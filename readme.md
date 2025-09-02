# atomics
Basic utils for concurrent programming.

## Installation
```toml 
[dependencies]
atomics = "0.1"
```

## Types

### Backoff

Basic exponential backoff implementaiton.

- If its generic param is 0, it will always execute `thread::yield_now()`.
- If its generic params is positive, it will execute a number of `hint::spin_loop()` before it starts to `thread::yield_now()`.
- If its generic param is negative, it will just execute `hint::spin_loop()` without ever yielding.

### atomic_t::AtomicT{Usize,64,32,16,8}

Wrapps the type in atomic. Type size must match the size of the atomic.
Type must be `Copy` and have no uninit bytes.

`new` constructior is unsafe, since you need to guarantee that the type contains no uninit bytes.

With `bytemuck` feature, there is a safe constructor: `new_no_uninit`.

### atomic_t_mut::AtomicT{Usize,64,32,16,8}

Alsmost the same as `atomic_t::*`, but uses `atomic_maybe_uninit` crate to support types that have uninit bytes.

This makes `new` constructor safe.

Downside is that `atomic_maybe_uninit` crate uses inline assembly to support this, which means you cannot use `miri` to test programs that use it.


### SpinMutex

- Default type `SpinMutex` used `Backoff<6>`.
- You can use `SpinSeqLockEx` with a custom backoff param.

### SpinRwLock

- Default type `SpinRwLock` used `Backoff<6>`.
- You can use `SpinRwLockEx` with a custom backoff param.

### SpinSeqLock

Reference implementation: [Amanieu/seqlock](https://github.com/Amanieu/seqlock)

- Default type `SpinSeqLock` used `Backoff<6>`.
- You can use `SpinSeqLockEx` with a custom backoff param.

Sequence locks support "optimistic reading" that can `load()` `Copy` types without writing to shared memory.

Downside is that "optimistic reading" is technically UB under Rust/C++ memory model. It is a well known "hole" in the model, but people have been using it in both Rust/C/C++ without issues (citation needed!). 

Actual LLVM memory model allows for this use case, which might be part of the reason reason why things dont blow up.

Use it with caution!

NOTE: Since `miri` will recognize it as UB, optimistic reads are disabled for `miri`.


## Features
- `std` - Enables `thread::yield_now()` for `Backoff`, otherwise it will awalys use just `hint::spin_loop()`.
- `serde` - Enables `Serialize` and `Deserialize` for `SpinSeqLock`. TODO: support `serde` for other types!
- `bytemuck` - Enables safe constructor for `atomic_t::*` types. 

