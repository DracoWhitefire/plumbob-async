# plumbob-async

[![CI](https://github.com/ashlar-display/plumbob-async/actions/workflows/ci.yml/badge.svg)](https://github.com/ashlar-display/plumbob-async/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/plumbob-async.svg)](https://crates.io/crates/plumbob-async)
[![docs.rs](https://docs.rs/plumbob-async/badge.svg)](https://docs.rs/plumbob-async)
[![License: MPL-2.0](https://img.shields.io/badge/license-MPL--2.0-blue.svg)](LICENSE)
[![Rust 1.85+](https://img.shields.io/badge/rustc-1.85+-orange.svg)](https://blog.rust-lang.org/2025/02/20/Rust-1.85.0.html)
[![SLSA Level 2](https://slsa.dev/images/gh-badge-level2.svg)](https://slsa.dev)
[![trusted publishing only](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fcrates.io%2Fapi%2Fv1%2Fcrates%2Fplumbob-async&query=%24.crate.trustpub_only&label=trusted%20publishing%20only)](#verifying-releases)

> **Part of [Ashlar](https://ashlar-display.org)** — a display connection stack in Rust,
> HDMI 2.1 first: from EDID parsing through mode negotiation to FRL link training.
>
> [display-types](https://github.com/DracoWhitefire/display-types) ·
> [piaf](https://github.com/DracoWhitefire/piaf) ·
> [concordance](https://github.com/DracoWhitefire/concordance) ·
> [cartouche](https://github.com/DracoWhitefire/cartouche) ·
> [culvert](https://github.com/DracoWhitefire/culvert) ·
> [plumbob](https://github.com/DracoWhitefire/plumbob) ·
> **plumbob-async** ·
> [hdmi-hal](https://github.com/DracoWhitefire/hdmi-hal) ·
> [hdmi-hal-i2c-dev](https://github.com/DracoWhitefire/hdmi-hal-i2c-dev)

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
    ClearableUpdateFlags, FrlConfig, LtpRequests, ScdcClient, SourceTestConfig, UpdateFlags,
};

struct MyScdcClient { /* async I²C / DDC transport */ }

impl ScdcClient for MyScdcClient {
    type Error = MyError;

    // Status_Flags_0 bit 6. Polled in LTS:2: await the poll interval (2 ms by default) here.
    async fn read_flt_ready(&mut self) -> Result<bool, MyError> { todo!() }
    // Update_0, all seven flags. Polled in LTS:3 and LTS:P: await the poll interval here too.
    async fn read_update_flags(&mut self) -> Result<UpdateFlags, MyError> { todo!() }
    async fn clear_update_flags(&mut self, flags: ClearableUpdateFlags) -> Result<(), MyError> { todo!() }
    async fn read_ltp_requests(&mut self) -> Result<LtpRequests, MyError> { todo!() }
    async fn read_source_test_config(&mut self) -> Result<SourceTestConfig, MyError> { todo!() }
    async fn write_config_0_defaults(&mut self) -> Result<(), MyError> { todo!() }
    async fn write_frl_config(&mut self, config: FrlConfig) -> Result<(), MyError> { todo!() }
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
`trainer.phy_mut().set_frl_output(FrlOutput::Active).await`, with `FrlOutput` and the
`HdmiPhy` trait imported from `plumbob_async` — or on the PHY `into_parts` returns. `scdc`, `scdc_mut`, `phy` and `phy_mut` give access to both in between.

The `HdmiPhy` bound on `P` is `hdmi_hal_async::phy::HdmiPhy`. The `FrlConfig`,
`TrainingConfig`, `Trained`, `TrainingOutcome`, `TrainingWarning`, `TrainingError`, and
all protocol types are re-exported from `plumbob` and are identical between the sync and
async APIs: `train` returns the outcome with the warnings the attempt produced, such as
a sink requesting an undefined pattern value.

### Multi-threaded executors

The plain traits promise nothing about their futures, so code generic over `ScdcClient`
and `HdmiPhy` cannot spawn training on a multi-threaded executor. For that, implement the
`Send` twins, `SendScdcClient` and hdmi-hal-async's `SendHdmiPhy` (re-exported): the same
methods, returning futures that are `Send`, with errors that are `Send`. With both,
the futures of `train`, the other training methods and `exit_to_tmds` are `Send` in
generic code (`train_with_events` given a `Send` callback). Bound the code by the twins
alone, as below: adding `+ ScdcClient` or `+ HdmiPhy`, which the twins imply anyway, makes
the compiler use that bound instead of the blanket implementation, and the futures are no
longer known to be `Send`:

```rust
use plumbob_async::{FrlTrainer, HdmiForumFrl, SendHdmiPhy, SendScdcClient, TrainingConfig};

impl SendScdcClient for MyThreadSafeScdcClient {
    type Error = MyError;

    async fn read_flt_ready(&mut self) -> Result<bool, MyError> { todo!() }
    // … the other six methods, as for `ScdcClient`
}

fn spawn_training<C, P>(mut trainer: FrlTrainer<C, P>, rates: &'static [HdmiForumFrl])
where
    C: SendScdcClient + 'static,
    P: SendHdmiPhy + 'static,
{
    tokio::spawn(async move {
        match trainer.train(rates, &TrainingConfig::default()).await {
            Ok(trained) => println!("{:?}", trained.outcome),
            Err(_) => println!("training failed"),
        }
    });
}
```

A twin implementation is also an implementation of the plain trait, so it works with
code written against either. Import the trait your code is bounded by: with both in
scope, method calls on a twin implementation are ambiguous. Code that keeps the types
concrete needs no twins: its `train` future is `Send` when the client's and PHY's futures
and error types are.

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

For a wall-clock deadline, or to stop on a signal without dropping the future, fail one
polled `ScdcClient` read with an error of your own and let every later call through:
training stops, LTS:L returns both ends to TMDS, and `train` returns
`TrainingError::Scdc` with your error. Never fail a read made after LTS:L has begun
(after a `write_frl_config` with `HdmiForumFrl::NotSupported`), until a
`write_frl_config` with an FRL rate starts training again. LTS:L is skipped with
`TrainingConfig::exit_to_tmds_on_error` off, and the link is held while the sink sets
`FLT_no_timeout`; `plumbob::ScdcClient`'s "Stopping early" has the details.

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
  timeout mechanism, and a wall-clock deadline is the caller's (see "Cancellation"); the
  interval belongs inside the polled `ScdcClient` methods, and awaiting it is what lets
  other tasks run: an `ScdcClient` whose futures are always ready keeps the executor for
  the whole `train` call (up to 7000 polls with the defaults; see plumbob's architecture
  document for the bound).
- **Executor dependency** — no runtime crate is in the dependency graph. The plain
  traits' futures are not required to be `Send`; for a multi-threaded executor, implement
  the `Send` twins (see "Multi-threaded executors").

## Documentation

- [`doc/architecture.md`](doc/architecture.md) — role, scope, the async trait and
  trainer, and design principles
- [`doc/model.md`](doc/model.md) — the re-exported types and the culvert-async boundary
- [`doc/testing.md`](doc/testing.md) — what the async driver's tests cover

## Verifying releases

Releases are built on GitHub Actions and attested with
[SLSA Build Level 2](https://slsa.dev) provenance (from 0.2.0). To verify a release
`.crate` against its signed provenance, install the
[GitHub CLI](https://cli.github.com/) and run:

```sh
gh attestation verify plumbob-async-X.Y.Z.crate --repo ashlar-display/plumbob-async
```

The release from before the repository moved to the `ashlar-display` organization (0.2.0)
was attested under its previous owner, and verifies against it:

```sh
gh attestation verify plumbob-async-0.2.0.crate --owner DracoWhitefire \
  --signer-repo DracoWhitefire/plumbob-async
```

The attested `.crate` is attached to each
[GitHub release](https://github.com/ashlar-display/plumbob-async/releases).

Releases from 0.2.0 are uploaded through crates.io
[trusted publishing](https://rust-lang.github.io/rfcs/3691-trusted-publishing-cratesio.html): the publish
workflow obtains a short-lived token via OpenID Connect instead of using a stored
API token. crates.io can also reject uploads made with an API token for this crate;
the badge above shows whether that setting is on. Each version's publishing details,
including the workflow run and commit, are available from the crates.io API:

```sh
curl -A "your-tool (contact)" https://crates.io/api/v1/crates/plumbob-async/X.Y.Z
# see .version.trustpub_data
```

0.1.0 was uploaded by hand with an API token; it carries no provenance and has no GitHub
release.

## License

Licensed under the [Mozilla Public License 2.0](LICENSE).
