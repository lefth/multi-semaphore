// Copyright 2014 The Rust Project Developers. See
// http://rust-lang.org/COPYRIGHT.
// Copyright 2021 Daniel Zwell.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::ops::Drop;
use std::sync::{Condvar, Mutex};

/// A counting, blocking, semaphore.
///
/// Semaphores are a form of atomic counter where access is only granted if the
/// counter is a positive value. Each acquisition will block the calling thread
/// until the counter is positive, and each release will increment the counter
/// and unblock any threads if necessary. This library allows getting a count
/// greater than 1. If a thread acquires 4 resources, the thread will block until
/// the counter is 4 or greater.  Each release will increment the counter and
/// unblock any threads if necessary.

///
/// # Examples
///
/// ```
/// use multi_semaphore::Semaphore;
///
/// // Create a semaphore that represents 6 resources
/// let sem = Semaphore::new(6);
///
/// // Acquire one or more of the resources
/// sem.acquire();
/// sem.acquire_many(2);
///
/// // Acquire one or more of the resources for a limited period of time
/// {
///     let _guard = sem.access();
///     // ...
/// } // resource is released here

/// {
///     let _guard = sem.access_many(3);
///     // ...
/// } // resources are released here
///
/// // Release our initially acquired resource
/// sem.release();
/// // Take care to relase the number of resources you intend, rather than too few or too many.
/// // Using guards from `access_many(n)` is preferred.
/// sem.release_many(2);
/// ```
pub struct Semaphore {
    lock: Mutex<isize>,
    cvar: Condvar,
}

/// An RAII guard which will release one or more resources acquired from a semaphore when
/// dropped.
pub struct SemaphoreGuard<'a> {
    sem: &'a Semaphore,
    amount: isize,
}

impl Semaphore {
    /// Creates a new semaphore with the initial count specified.
    ///
    /// The count specified can be thought of as a number of resources, and a
    /// call to `acquire` or `access` will block until at least one resource is
    /// available. It is valid to initialize a semaphore with a negative count.
    pub fn new(count: isize) -> Semaphore {
        Semaphore {
            lock: Mutex::new(count),
            cvar: Condvar::new(),
        }
    }

    /// Acquires a resource of this semaphore, blocking the current thread until
    /// it can do so.
    ///
    /// This method will block until the internal count of the semaphore is at
    /// least 1.
    pub fn acquire(&self) {
        let mut count = self.lock.lock().unwrap();
        while *count <= 0 {
            count = self.cvar.wait(count).unwrap();
        }
        *count -= 1;
    }

    /// Acquires one or more resources of this semaphore, blocking the current thread until
    /// it can do so.
    ///
    /// This method will block until the internal count of the semaphore is at
    /// least `amount`.
    pub fn acquire_many(&self, amount: isize) {
        if amount == 0 {
            return;
        }
        let mut count = self.lock.lock().unwrap();
        while *count < amount {
            count = self.cvar.wait(count).unwrap();
        }
        *count -= amount;
    }

    /// Release a resource from this semaphore.
    ///
    /// This will increment the number of resources in this semaphore by 1 and
    /// will notify any pending waiters in `acquire` or `access` if necessary.
    pub fn release(&self) {
        *self.lock.lock().unwrap() += 1;
        self.cvar.notify_all();
    }

    /// Release one or more resources from this semaphore.
    ///
    /// This will increment the number of resources in this semaphore by 1 and
    /// will notify any pending waiters in `acquire` or `access` if necessary.
    pub fn release_many(&self, amount: isize) {
        if amount == 0 {
            return;
        }
        *self.lock.lock().unwrap() += amount;
        self.cvar.notify_all();
    }

    /// Acquires a resource of this semaphore, returning an RAII guard to
    /// release the semaphore when dropped.
    ///
    /// This function is semantically equivalent to an `acquire` followed by a
    /// `release` when the guard returned is dropped.
    pub fn access(&self) -> SemaphoreGuard {
        self.acquire();
        SemaphoreGuard {
            sem: self,
            amount: 1,
        }
    }

    /// Acquires one or more resources of this semaphore, returning an RAII guard to
    /// release the semaphore when dropped.
    ///
    /// This function is semantically equivalent to an `acquire_many(n)` followed by a
    /// `release_many(n)` when the guard returned is dropped.
    pub fn access_many(&self, amount: isize) -> SemaphoreGuard {
        self.acquire_many(amount);
        SemaphoreGuard {
            sem: self,
            amount: amount,
        }
    }
}

impl<'a> Drop for SemaphoreGuard<'a> {
    fn drop(&mut self) {
        if self.amount == 0 {
            return;
        }
        self.sem.release_many(self.amount);
    }
}

#[cfg(test)]
mod tests {
    use std::prelude::v1::*;

    use super::Semaphore;
    use std::sync::mpsc::channel;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_sem_acquire_release() {
        let s = Semaphore::new(1);
        s.acquire();
        s.release();
        s.acquire();
    }

    #[test]
    fn test_sem_basic() {
        let s = Semaphore::new(1);
        let _g = s.access();
    }

    #[test]
    fn test_sem_as_mutex() {
        let s = Arc::new(Semaphore::new(1));
        let s2 = s.clone();
        let _t = thread::spawn(move || {
            let _g = s2.access();
        });
        let _g = s.access();
    }

    #[test]
    fn test_sem_as_cvar() {
        // Child waits and parent signals
        let (tx, rx) = channel();
        let s = Arc::new(Semaphore::new(0));
        let s2 = s.clone();
        let _t = thread::spawn(move || {
            s2.acquire();
            tx.send(()).unwrap();
        });
        s.release();
        let _ = rx.recv();

        // Parent waits and child signals
        let (tx, rx) = channel();
        let s = Arc::new(Semaphore::new(0));
        let s2 = s.clone();
        let _t = thread::spawn(move || {
            s2.release();
            let _ = rx.recv();
        });
        s.acquire();
        tx.send(()).unwrap();
    }

    #[test]
    fn test_sem_multi_resource() {
        // Parent and child both get in the critical section at the same
        // time, and shake hands.
        let s = Arc::new(Semaphore::new(2));
        let s2 = s.clone();
        let (tx1, rx1) = channel();
        let (tx2, rx2) = channel();
        let _t = thread::spawn(move || {
            let _g = s2.access();
            let _ = rx2.recv();
            tx1.send(()).unwrap();
        });
        let _g = s.access();
        tx2.send(()).unwrap();
        rx1.recv().unwrap();
    }

    #[test]
    fn test_sem_runtime_friendly_blocking() {
        let s = Arc::new(Semaphore::new(1));
        let s2 = s.clone();
        let (tx, rx) = channel();
        {
            let _g = s.access();
            thread::spawn(move || {
                tx.send(()).unwrap();
                drop(s2.access());
                tx.send(()).unwrap();
            });
            rx.recv().unwrap(); // wait for child to come alive
        }
        rx.recv().unwrap(); // wait for child to be done
    }
}
