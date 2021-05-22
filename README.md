# multi-semaphore

[![Build Status](https://travis-ci.com/lefth/multi-semaphore.svg?branch=master)](https://travis-ci.com/lefth/multi-semaphore)

[Documentation (master)](https://lefth.github.io/multi-semaphore)

A counting, blocking semaphore extracted from rust 1.7.0.

Semaphores are a form of atomic counter where access is only granted if the
counter is a positive value.  This library allows getting a count greater than
1--if a thread acquires 4 resources, the thread will block until the counter is
4 or greater.  Each release will increment the counter and unblock any threads
if necessary.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
multi-semaphore = "0.1"
```

and if you are using an older version of rust, add this to your crate root:

```rust
extern crate multi_semaphore;
```

## Examples

```rust
use multi_semaphore::Semaphore;

// Create a semaphore that represents 5 resources
let sem = Semaphore::new(5);

// Acquire one of the resources
sem.acquire();

// Acquire one of the resources for a limited period of time
{
    let _guard = sem.access();
	// or
    let _guard = sem.access_many(8);
    // ...
} // resources are released here

// Release our initially acquired resource
sem.release();
// or
sem.release_many(8);
```

## License

Unless otherwise noted, all code, tests, and docs are © 2014 The Rust Project Developers and dual-licensed under the Apache 2.0 and MIT licenses.
See the copyright declaration at the top of [src/lib.rs](src/lib.rs) for more.
