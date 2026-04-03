# Architecture

## Role

`plumbob-async` is the async companion to `plumbob`. It mirrors `plumbob`'s FRL link
training state machine but exposes `async fn` methods, following the same design split as
`embedded-hal` / `embedded-hal-async` and `hdmi-hal` / `hdmi-hal-async`.

The crate defines an async `ScdcClient` trait and provides `FrlTrainer<C, P>` with an
`async fn train_at_rate`. Data types (`LtpReq`, `FfeLevels`, `FrlConfig`, `TrainingStatus`,
`CedCounters`, `TrainingConfig`, `TrainingOutcome`, `TrainingError`, `TrainingEvent`,
`TrainingTrace`) are defined in `plumbob` and re-exported here; they are not duplicated.

The async training loop yields between poll iterations instead of spinning, making it
suitable for cooperative async runtimes on both embedded targets (Embassy) and OS targets
(Tokio, async-std).

---

## Scope

plumbob-async covers:

- `ScdcClient`: the async typed SCDC interface trait, mirroring `plumbob::ScdcClient`,
- `FrlTrainer<C, P>`: the central type, owning an async `ScdcClient` and an async `HdmiPhy`,
- `async fn train_at_rate`: the full four-phase FRL training sequence, yielding between polls,
- re-exports of all shared protocol and training types from `plumbob`,
- simulation support: the training procedure is fully exercisable without real hardware
  using simulated async implementations of `ScdcClient` and `HdmiPhy`.

The following are out of scope, for the same reasons as in `plumbob`:

- **Rate fallback policy** — `train_at_rate` returns `FallbackRequired`; the caller
  decides the retry sequence.
- **SCDC register decoding** — plumbob-async reads typed values from `ScdcClient` and
  does not decode raw register bytes.
- **PHY vendor sequences** — plumbob-async calls `HdmiPhy` methods; the register sequences
  are in platform PHY backends.
- **Timing and sleep** — plumbob-async does not insert delays between polls. The
  iteration limits in `TrainingConfig` remain the only timeout mechanism. If a caller
  needs a delay between polls, it wraps its `ScdcClient` implementation to insert one.
- **TMDS link setup** — FRL training only.
- **Executor dependency** — the traits impose no constraint on which async runtime is used.

---

## Dependencies

```
display-types    ─┐
hdmi-hal         ─┼─►  hdmi-hal-async  ─┐
                  │                      │
plumbob          ─┴──────────────────────┴─►  plumbob-async
```

- `plumbob` — all shared data types, re-exported from here
- `hdmi-hal-async` — `HdmiPhy` (async version)

plumbob-async does not depend on `culvert-async`. When `culvert-async` is built, it will
implement `plumbob_async::ScdcClient` for its `Scdc<T>` behind a `plumbob-async` cargo
feature, mirroring the sync pattern.

---

## Async `ScdcClient`

Async mirror of `plumbob::ScdcClient`. Identical semantics; `fn` becomes `async fn`.

```rust
pub trait ScdcClient {
    type Error;

    async fn write_frl_config(&mut self, config: FrlConfig) -> Result<(), Self::Error>;
    async fn read_training_status(&mut self) -> Result<TrainingStatus, Self::Error>;
    async fn read_ced(&mut self) -> Result<CedCounters, Self::Error>;
}
```

MSRV is `1.85`, which includes stable `async fn` in traits (AFIT, stabilised in `1.75`).
The `async-trait` proc-macro crate is not needed. AFIT-generated futures are not
object-safe, which is the same trade-off `embedded-hal-async` accepts.

---

## `FrlTrainer<C, P>`

The central training type. Owns an async `ScdcClient` and an async `HdmiPhy`.

```rust
pub struct FrlTrainer<C, P> { ... }

impl<C: ScdcClient, P: HdmiPhy> FrlTrainer<C, P> {
    pub fn new(scdc: C, phy: P) -> Self;
    pub fn into_parts(self) -> (C, P);

    /// Run the full four-phase training sequence at the given FRL rate.
    pub async fn train_at_rate(
        &mut self,
        rate: HdmiForumFrl,
        config: &TrainingConfig,
    ) -> Result<TrainingOutcome, TrainingError<C::Error, P::Error>>;

    /// Like `train_at_rate`, but also returns a `TrainingTrace`. Requires `alloc`.
    #[cfg(feature = "alloc")]
    pub async fn train_at_rate_traced(
        &mut self,
        rate: HdmiForumFrl,
        config: &TrainingConfig,
    ) -> Result<(TrainingOutcome, TrainingTrace), TrainingError<C::Error, P::Error>>;
}
```

`HdmiPhy` here is `hdmi_hal_async::phy::HdmiPhy`. The `FrlTrainer` type alias re-uses all
training outcome and config types from `plumbob`; there is nothing new to define there.

---

## Training Procedure

The four-phase sequence is identical to `plumbob`'s. Each phase that polls the sink
becomes an `async` loop that `.await`s each `read_training_status` call, yielding to the
executor between iterations. No blocking spin loops anywhere in the path.

### Phase 1 — Configuration

1. `await` `ScdcClient::write_frl_config` with the target rate and FFE level count.
2. `await` `HdmiPhy::set_frl_rate` to configure the physical lanes for this rate.

### Phase 2 — Readiness

`await` `ScdcClient::read_training_status` in a loop until `flt_ready` asserts. Terminate
with `FallbackRequired` after `TrainingConfig::flt_ready_timeout` iterations.

### Phase 3 — Initiation

`await` `ScdcClient::read_training_status` in a loop until `frl_start` asserts. Terminate
with `FallbackRequired` after `TrainingConfig::frl_start_timeout` iterations.

### Phase 4 — LTP Loop

On each iteration, `await` `read_training_status`. If `LtpReq::None`, training succeeded.
Otherwise, `await` `HdmiPhy::send_ltp` with the requested pattern and iterate. Terminate
with `FallbackRequired` after `TrainingConfig::ltp_timeout` iterations.

---

## Re-exports

All consumer-facing types from `plumbob` are re-exported so callers have a single import
path:

```rust
pub use plumbob::{
    CedCount, CedCounters, FfeLevels, FrlConfig, LtpReq,
    TrainingConfig, TrainingError, TrainingEvent, TrainingOutcome, TrainingTrace,
    TrainingStatus,
};
```

`HdmiForumFrl` comes from `display-types`; callers import it from there or from `hdmi-hal`
directly.

---

## Module Structure

```
src/
  lib.rs          — #![no_std], re-exports, module declarations, crate-level docs
  client.rs       — async ScdcClient trait
  trainer.rs      — FrlTrainer<C, P> and train_at_rate implementation
```

---

## `no_std`, `alloc`, and Features

plumbob-async declares `#![no_std]` and `#![forbid(unsafe_code)]`. The feature tiers
mirror `plumbob` exactly:

| Feature | Adds |
|---|---|
| *(none)* | Full async training loop, no heap use |
| `alloc` | `TrainingTrace` re-export and `train_at_rate_traced` |
| `std` | Implies `alloc` |

The `alloc` and `std` features gate `train_at_rate_traced` in the same way as `plumbob`,
and they must also enable the corresponding features on the `plumbob` dependency so that
`TrainingTrace` is available to re-export.

---

## Design Principles

The principles are the same as `plumbob`'s:

- **Interfaces owned by consumers.** plumbob-async defines the interface its dependencies
  must satisfy (`ScdcClient`) rather than depending on a concrete implementation.
- **Deterministic and testable.** The training procedure runs identically against a
  simulated async `ScdcClient` and real hardware. No hardware required for any test.
- **Policy at the right layer.** plumbob-async implements the spec, not strategy.
- **No unsafe code.** `#![forbid(unsafe_code)]`.
- **Type reuse over duplication.** All protocol and training types live in `plumbob` and
  are re-exported here. There is no parallel type hierarchy.

---

## Implementation Plan

### Step 1 — Cargo.toml

Add dependencies:

```toml
[dependencies]
plumbob = { version = "0.1", default-features = false }
hdmi-hal-async = { version = "0.1", default-features = false }

[features]
alloc = ["plumbob/alloc"]
std = ["alloc", "plumbob/std"]
```

`hdmi-hal-async` is a path dependency (`path = "../hdmi-hal-async"`) until published on
crates.io; switch to a version requirement after the first release.

### Step 2 — `src/lib.rs`

Replace the placeholder content with:

```rust
#![no_std]
#![forbid(unsafe_code)]

mod client;
mod trainer;

pub use client::ScdcClient;
pub use trainer::FrlTrainer;

pub use plumbob::{
    CedCount, CedCounters, FfeLevels, FrlConfig, LtpReq,
    TrainingConfig, TrainingError, TrainingEvent, TrainingOutcome, TrainingStatus,
};

#[cfg(feature = "alloc")]
pub use plumbob::TrainingTrace;
```

### Step 3 — `src/client.rs`

Define the async `ScdcClient` trait. Doc comments mirror `plumbob::ScdcClient` verbatim,
with a note that each method is the async counterpart.

Add a `#[cfg(test)]` block with a minimal mock implementor (`struct MockScdc`) and
compile-time verification that the trait is well-formed.

### Step 4 — `src/trainer.rs`

Implement `FrlTrainer<C, P>`:

1. `new` and `into_parts` — identical to the sync version, no async needed.
2. `async fn train_at_rate` — four-phase loop mirroring `plumbob`'s `train_at_rate`,
   with every trait method call replaced by `.await`.
3. `async fn train_at_rate_traced` (behind `#[cfg(feature = "alloc")]`) — same as above
   but accumulates `TrainingEvent`s into a `TrainingTrace`. Since `TrainingEvent` and
   `TrainingTrace` are re-exported from `plumbob`, no new event types are needed; the
   event production logic is copied from `plumbob`.

### Step 5 — Tests

Each module's `#[cfg(test)]` block:

- `client.rs`: mock `ScdcClient` implementor; verify the trait compiles and that an
  `async fn` returning `Result<TrainingStatus, _>` satisfies the bound.
- `trainer.rs`: `MockScdc` + `MockPhy` (implementing `hdmi_hal_async::phy::HdmiPhy`)
  with pre-scripted responses covering all four terminal states:
  - successful training (phases 1–4 complete)
  - `FltReadyTimeout`
  - `FrlStartTimeout`
  - `LtpLoopTimeout`

Tests run under `tokio::test` (or `embassy_futures::block_on` for a `no_std`-compatible
alternative) in dev-dependencies, without pulling an executor into the library.

### Step 6 — README and doc

The README should follow the same structure as `plumbob`'s: crate role, quick-start
snippet, feature flags, and a pointer to this file for full documentation.
