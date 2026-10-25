# Development Setup

**Requirements:** Rust 1.85+ (stable). Install via [rustup](https://rustup.rs/).

## Clone and build

```sh
git clone https://github.com/ashlar-display/plumbob-async.git
cd plumbob-async
cargo build
```

## Running checks

```sh
cargo fmt --check
cargo clippy --features std -- -D warnings
cargo rustdoc --features std -- -D missing_docs
```

## Running tests

```sh
rustup target add thumbv7em-none-eabi                # once, for the no_std builds

cargo test --features std                            # std (default for tests)
cargo test --no-default-features                     # tests without alloc
cargo build --no-default-features --features alloc --target thumbv7em-none-eabi
cargo build --no-default-features --target thumbv7em-none-eabi
```

The `no_std` builds target `thumbv7em-none-eabi`, as CI does: on the host, `std` is
always available, so a host build cannot catch a dependency that enables it.

## Measuring coverage

Coverage requires [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov):

```sh
cargo install cargo-llvm-cov
cargo llvm-cov --features std
```

The current baseline is stored in `.coverage-baseline`. CI fails if coverage drops more
than 0.1% below it. On pushes to `main` or `develop`, an improvement automatically opens
a `ci/coverage-ratchet` PR to commit the new baseline.
