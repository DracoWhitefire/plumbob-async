# plumbob-async

[![CI](https://github.com/DracoWhitefire/plumbob-async/actions/workflows/ci.yml/badge.svg)](https://github.com/DracoWhitefire/plumbob-async/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/plumbob-async.svg)](https://crates.io/crates/plumbob-async)
[![docs.rs](https://docs.rs/plumbob-async/badge.svg)](https://docs.rs/plumbob-async)
[![License: MPL-2.0](https://img.shields.io/badge/license-MPL--2.0-blue.svg)](LICENSE)
[![Rust 1.85+](https://img.shields.io/badge/rustc-1.85+-orange.svg)](https://blog.rust-lang.org/2025/02/20/Rust-1.85.0.html)
[![SLSA Level 2](https://slsa.dev/images/gh-badge-level2.svg)](https://slsa.dev)

Async FRL link training state machine for HDMI 2.1.

`plumbob-async` is the async companion to [`plumbob`](https://crates.io/crates/plumbob).
It runs plumbob's link training state machine — `plumbob::lts::run`, which performs no
I/O itself — over an async `ScdcClient` and an async `HdmiPhy`, `.await`ing each
operation, so training yields to the executor wherever the I/O waits. There is one state
machine for sync and async training: the outcomes, errors and traces are exactly
plumbob's. All shared protocol and training types are re-exported from `plumbob`.

## Usage

```sh
cargo add plumbob-async
```

Implement `ScdcClient` against your async SCDC transport, one method per register
operation (the async counterpart of `plumbob::ScdcClient`; see its documentation for what
each method must guarantee):

```rust
use plumbob_async::{
    CedCounters, ClearableUpdateFlags, FrlConfig, LtpRequests, ScdcClient, SourceTestConfig,
    UpdateFlags,
};

struct MyScdcClient { /* async I²C / DDC transport */ }

impl ScdcClient for MyScdcClient {
    type Error = MyError;

    // Polled in LTS:2 and LTS:3/LTS:P: await the poll interval (2 ms by default) here.
    async fn read_flt_ready(&mut self) -> Result<bool, MyError> { todo!() }
    async fn read_update_flags(&mut self) -> Result<UpdateFlags, MyError> { todo!() }
    async fn clear_update_flags(&mut self, flags: ClearableUpdateFlags) -> Result<(), MyError> { todo!() }
    async fn read_ltp_requests(&mut self) -> Result<LtpRequests, MyError> { todo!() }
    async fn read_source_test_config(&mut self) -> Result<SourceTestConfig, MyError> { todo!() }
    async fn write_config_0_defaults(&mut self) -> Result<(), MyError> { todo!() }
    async fn write_frl_config(&mut self, config: FrlConfig) -> Result<(), MyError> { todo!() }
    async fn read_ced(&mut self) -> Result<CedCounters, MyError> { todo!() }
}
```

Construct an `FrlTrainer` and train over the rates to try, highest first; the sink's
requests for a lower rate step down through the list:

```rust
use plumbob_async::{FrlTrainer, HdmiForumFrl, TrainingConfig, TrainingOutcome};

let mut trainer = FrlTrainer::new(scdc, phy);
let config = TrainingConfig::default();

let rates = [
    HdmiForumFrl::Rate12Gbps4Lanes,
    HdmiForumFrl::Rate10Gbps4Lanes,
    HdmiForumFrl::Rate6Gbps4Lanes,
];

let trained = trainer.train(&rates, &config).await?;
match trained.outcome {
    TrainingOutcome::Success { achieved_rate, .. } => println!("Trained at {achieved_rate:?}"),
    TrainingOutcome::FallbackRequired { reason, .. } => println!("No FRL link: {reason:?}"),
    _ => {}
}
for warning in trained.iter_warnings() {
    println!("Warning: {warning:?}");
}
```

After `Success`, start video on the PHY the trainer holds —
`trainer.phy_mut().set_frl_output(FrlOutput::Active).await` — or on the PHY `into_parts`
returns. `scdc`, `scdc_mut`, `phy` and `phy_mut` give access to both in between.

The `HdmiPhy` bound on `P` is `hdmi_hal_async::phy::HdmiPhy`. The `FrlConfig`,
`TrainingConfig`, `Trained`, `TrainingOutcome`, `TrainingWarning`, `TrainingError`, and
all protocol types are re-exported from `plumbob` and are identical between the sync and
async APIs: `train` returns the outcome with the warnings the attempt produced, such as
a sink requesting an undefined pattern value.

## Training procedure

The procedure is plumbob's — LTS:2 (prepare), LTS:3 (train), LTS:P (pass), with LTS:4
(lower rate) on the sink's request and LTS:L (exit to TMDS) before every
`FallbackRequired` and after an I/O error — because it is plumbob's code: `FrlTrainer` runs `plumbob::lts::run`
over its async SCDC client and PHY. See
[plumbob's README](https://github.com/DracoWhitefire/plumbob#training-procedure) and
architecture document for the states, the poll and retrain limits, and the per-lane rules.

`FallbackRequired { reason }` means no listed rate trained; `TrainingError` means a hard
I/O failure from the SCDC client or PHY. The two are kept distinct so a caller diagnosing
a failure knows whether it came from the protocol or the bus.

`trainer.exit_to_tmds().await` runs LTS:L on its own, to take a trained link down when
the display is disabled or unplugged, or before a mode change.

## Cancellation

`train` is not cancel-safe. Dropping its future — losing a `select!` against a
hot-unplug event, or hitting a `timeout` — stops training wherever it is, and a dropped
future cannot run LTS:L: the sink may be left configured for an FRL rate and the PHY
driving training patterns. After dropping one, return both ends to TMDS yourself:

```rust
select! {
    result = trainer.train(&rates, &config) => handle(result),
    _ = hot_unplug.wait() => {
        trainer.exit_to_tmds().await?;
    }
}
```

To bound how long training takes, prefer the poll limits in `TrainingConfig`: every
attempt that hits one ends in a defined state — with LTS:L already run, or, while the sink
sets `FLT_no_timeout` (compliance testing), with the link held for the test equipment
(`TrainingOutcome::NoTimeoutHold`).

## Diagnostics

Enable the `alloc` feature to get `train_traced` (and `train_at_rate_traced`), which
return a `TrainingTrace` alongside the result, whether that is an outcome or an error.
The trace and its `TrainingEvent`s are plumbob's: a sync trace and an async trace of the
same sink are identical.

```rust
let (result, trace) = trainer.train_traced(&rates, &config).await;

println!("Result: {result:?}");
for event in &trace.events {
    println!("  {event:?}");
}
```

Without an allocator, `train_with_events(&rates, &config, &mut |event| …).await` delivers
the same events as they occur, to a plain callback.

## Features

| Feature | Default | Description |
|---------|---------|-------------|
| `std`   | no      | Implies `alloc`; no additional API surface |
| `alloc` | no      | Enables `TrainingTrace`, `train_traced` and `train_at_rate_traced` |

No features are enabled by default.

## `no_std` builds

`plumbob-async` declares `#![no_std]` throughout. The bare crate provides full async
training without an allocator. The `alloc` feature adds `TrainingTrace`, `train_traced`
and `train_at_rate_traced`; `std` implies `alloc`.

```sh
# no_std, no heap
cargo add plumbob-async

# no_std + trace support
cargo add plumbob-async --features alloc

# std targets
cargo add plumbob-async --features std
```

No executor is assumed. The traits impose no constraint on which async runtime is used —
Embassy, Tokio, and async-std are all valid implementors.

## Stack position

```mermaid
flowchart LR
    dt["display-types"]
    hal["hdmi-hal"]
    hal_async["hdmi-hal-async"]
    plumbob["plumbob"]
    plumbob_async["plumbob-async"]

    dt --> hal --> hal_async --> plumbob_async
    dt --> plumbob --> plumbob_async
```

`plumbob-async` depends on `plumbob` for the state machine and all shared data types,
and on `hdmi-hal-async` for the async `HdmiPhy` trait. It does not depend on any executor
or runtime crate.

## Out of scope

- **The training procedure** — it is `plumbob`'s (`plumbob::lts::run`); this crate
  supplies async I/O to it.
- **Rate fallback policy** — the caller decides which rates to pass to `train` and what
  to do with a `FallbackRequired`.
- **SCDC register decoding** — plumbob-async reads typed values from `ScdcClient` and
  does not decode raw register bytes.
- **PHY vendor sequences** — plumbob-async calls `HdmiPhy` methods; the register
  sequences live in platform PHY backends.
- **Timing and inter-poll delay** — the poll limits in `TrainingConfig` are the only
  timeout mechanism; the interval belongs inside the polled `ScdcClient` methods, and
  awaiting it is what lets other tasks run: an `ScdcClient` whose futures are always
  ready keeps the executor for the whole `train` call (up to 7000 polls with the
  defaults; see plumbob's architecture document for the bound).
- **Executor dependency** — no runtime crate is in the dependency graph. The futures are
  not required to be `Send`: code generic over `ScdcClient` and `HdmiPhy` runs on a
  single-threaded executor or keeps the types concrete (see the architecture document).

## Documentation

- [`doc/architecture.md`](doc/architecture.md) — role, scope, the async trait and
  trainer, and design principles
- [`doc/model.md`](doc/model.md) — the re-exported types and the culvert-async boundary
- [`doc/testing.md`](doc/testing.md) — what the async driver's tests cover

## Verifying releases

Each release is built on GitHub Actions and attested with
[SLSA Build Level 2](https://slsa.dev) provenance. To verify a release
`.crate` against its signed provenance, install the
[GitHub CLI](https://cli.github.com/) and run:

```sh
gh attestation verify plumbob-async-X.Y.Z.crate --repo DracoWhitefire/plumbob-async
```

The attested `.crate` is attached to each
[GitHub release](https://github.com/DracoWhitefire/plumbob-async/releases).

## License

Licensed under the [Mozilla Public License 2.0](LICENSE).
