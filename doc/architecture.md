# Architecture

## Role

`plumbob-async` is the async companion to `plumbob`: FRL link training for async SCDC and
PHY implementations. It does not have a training state machine of its own. It runs
plumbob's — [`plumbob::lts::run`], an `async fn` that performs no I/O itself — over an
async `ScdcClient` and an async `HdmiPhy`, `.await`ing each operation, so training yields
to the executor wherever the I/O waits.

Because there is one state machine, sync and async training behave identically and every
fix reaches both. The stack design document's "Sync and Async Companions" section records
why the state machine lives in `plumbob` rather than being copied here.

The crate is suitable for cooperative async runtimes on embedded targets (Embassy) and OS
targets (Tokio, async-std); it depends on no runtime.

---

## Scope

plumbob-async covers:

- `ScdcClient`: the async typed SCDC interface trait, the counterpart of
  `plumbob::ScdcClient`,
- `FrlTrainer<C, P>`: owns an async `ScdcClient` and an async `HdmiPhy` and runs
  `plumbob::lts::run` over them (`train`, `train_at_rate` and their traced forms),
- re-exports of the shared protocol and training types from `plumbob`.

The following are out of scope:

- **The training procedure itself** — LTS:2 → LTS:3 → LTS:P, LTS:4, LTS:L, the poll
  limits and the retrain limit are `plumbob`'s; see its architecture document.
- **Rate fallback policy** — the caller decides which rates to pass to `train` and what
  to do with a `FallbackRequired`.
- **SCDC register decoding** — plumbob-async reads typed values from `ScdcClient` and
  does not decode raw register bytes.
- **PHY vendor sequences** — plumbob-async calls `HdmiPhy` methods; the register sequences
  are in platform PHY backends.
- **Timing** — the poll limits in `TrainingConfig` assume one poll every 2 ms by default.
  The interval belongs inside the polled `ScdcClient` methods, where an async
  implementation can await a timer.
- **Executor dependency** — the traits impose no constraint on which async runtime is used.

---

## Dependencies

```
display-types    ─┐
hdmi-hal         ─┼─►  hdmi-hal-async  ─┐
                  │                      │
plumbob          ─┴──────────────────────┴─►  plumbob-async
```

- `plumbob` — the state machine (`plumbob::lts`) and all shared types, re-exported here
- `hdmi-hal-async` — the async `HdmiPhy` and the `hdmi-hal` data types it re-exports
- `display-types` — `HdmiForumFrl`

`culvert-async` implements `plumbob_async::ScdcClient` for its `Scdc<T>` behind a
`plumbob-async` cargo feature, as `culvert` does for `plumbob::ScdcClient`.

---

## Async `ScdcClient`

The async counterpart to `plumbob::ScdcClient`: the same eight operations with the same
meaning and implementer obligations, each an `async fn`.

```rust
pub trait ScdcClient {
    type Error;

    async fn read_flt_ready(&mut self) -> Result<bool, Self::Error>;
    async fn read_update_flags(&mut self) -> Result<UpdateFlags, Self::Error>;
    async fn clear_update_flags(&mut self, flags: UpdateFlags) -> Result<(), Self::Error>;
    async fn read_ltp_requests(&mut self) -> Result<LtpRequests, Self::Error>;
    async fn read_source_test_config(&mut self) -> Result<SourceTestConfig, Self::Error>;
    async fn write_config_0_defaults(&mut self) -> Result<(), Self::Error>;
    async fn write_frl_config(&mut self, config: FrlConfig) -> Result<(), Self::Error>;
    async fn read_ced(&mut self) -> Result<CedCounters, Self::Error>;
}
```

MSRV is `1.85`, which includes stable `async fn` in traits (stabilised in `1.75`); the
`async-trait` crate is not needed. The futures are not required to be `Send` and the
trait is not object-safe — the same trade-offs `embedded-hal-async` and `hdmi-hal-async`
accept.

In practice: with concrete `ScdcClient` and `HdmiPhy` types whose futures are `Send`, the
`train` future is `Send` and can be spawned on a multi-threaded Tokio runtime. A caller
generic over `ScdcClient` and `HdmiPhy` cannot spawn it, whatever it bounds the types by;
it runs on a single-threaded executor (Embassy, Tokio's current-thread runtime or
`LocalSet`) or keeps the types concrete. `Send` variants of the traits, which lift this,
are designed and proven in the stack design document ("Sync and Async Companions", "`Send` futures for multi-threaded executors") and deferred until the integration layer needs
them.

---

## `FrlTrainer<C, P>`

```rust
impl<C: ScdcClient, P: HdmiPhy> FrlTrainer<C, P> {
    pub fn new(scdc: C, phy: P) -> Self;
    pub fn into_parts(self) -> (C, P);
    pub async fn exit_to_tmds(&mut self) -> Result<(), ExitError<C::Error, P::Error>>;

    pub async fn train(&mut self, rates: &[HdmiForumFrl], config: &TrainingConfig)
        -> Result<Trained, TrainingError<C::Error, P::Error>>;
    pub async fn train_at_rate(&mut self, rate: HdmiForumFrl, config: &TrainingConfig)
        -> Result<Trained, TrainingError<C::Error, P::Error>>;

    #[cfg(feature = "alloc")]
    pub async fn train_traced(&mut self, rates: &[HdmiForumFrl], config: &TrainingConfig)
        -> (Result<Trained, TrainingError<C::Error, P::Error>>, TrainingTrace);
    #[cfg(feature = "alloc")]
    pub async fn train_at_rate_traced(&mut self, rate: HdmiForumFrl, config: &TrainingConfig)
        -> (Result<Trained, TrainingError<C::Error, P::Error>>, TrainingTrace);
    pub async fn train_with_events<F: FnMut(TrainingEvent)>(&mut self,
        rates: &[HdmiForumFrl], config: &TrainingConfig, record: &mut F)
        -> Result<Trained, TrainingError<C::Error, P::Error>>;
}
```

`HdmiPhy` is `hdmi_hal_async::phy::HdmiPhy`. The API matches `plumbob::FrlTrainer`, with
the training methods `async`.

Each training method runs `plumbob::lts::run` over a private adapter that implements
`plumbob::lts::TrainingIo` by `.await`ing the trainer's `ScdcClient` and `HdmiPhy`. The
outcomes, errors and `TrainingEvent`s are therefore exactly plumbob's: a sync trace and an
async trace of the same sink are identical. `exit_to_tmds` runs `plumbob::lts::exit_to_tmds`
over the same adapter.

### Cancellation

`train` is not cancel-safe, and cannot be made so. Dropping its future stops the state
machine at whichever `.await` it was suspended on — possibly with `Config_1` holding an
FRL rate and the PHY driving per-lane patterns — and a dropped future cannot await
anything, so no drop guard can run LTS:L; async I/O has no synchronous fallback to call
from `Drop`. A transport transaction in flight may also be abandoned part-way.

The trainer itself is unaffected: it holds no state between calls, so it can train again
or run `exit_to_tmds` straight away. The documented recovery is that a caller who drops
a `train` future calls `exit_to_tmds()` before anything else. Bounding training with an
external timeout is discouraged in favour of `TrainingConfig`'s poll limits, which end
every attempt in a defined state.

---

## Re-exports

All consumer-facing types from `plumbob` are re-exported so callers have a single import
path:

```rust
pub use plumbob::{
    CedCount, CedCounters, ExitError, FallbackReason, FfeLevels, FrlConfig, LtpReq,
    LtpRequests, SourceTestConfig, TmdsExit, Trained, TrainingConfig, TrainingError,
    TrainingEvent, TrainingOutcome, TrainingWarning, UpdateFlags, MAX_WARNINGS,
};
#[cfg(feature = "alloc")]
pub use plumbob::TrainingTrace;
```

`HdmiForumFrl` comes from `display-types`; the `hdmi-hal` data types (`LanePatterns`,
`FrlOutput`, `EqParams`, …) from `hdmi-hal-async`.

---

## Module Structure

```
src/
  lib.rs              — #![no_std], re-exports, module declarations, crate-level docs
  client.rs           — the async ScdcClient trait
  trainer/mod.rs      — FrlTrainer<C, P> and its TrainingIo adapter
  trainer/tests.rs    — tests of the async driver
```

---

## `no_std`, `alloc`, and Features

plumbob-async declares `#![no_std]` and `#![forbid(unsafe_code)]`. The feature tiers
mirror `plumbob`'s:

| Feature | Adds |
|---|---|
| *(none)* | Full async training, no heap use |
| `alloc` | `TrainingTrace`, `train_traced` and `train_at_rate_traced` |
| `std` | Implies `alloc` |

The `alloc` and `std` features enable the corresponding features on the `plumbob`
dependency, which provides `TrainingTrace`. CI builds the crate with no features and with
`alloc` for `thumbv7em-none-eabi`, a target without `std`.

---

## Design Principles

The principles are `plumbob`'s:

- **Interfaces owned by consumers.** plumbob-async defines the interface its dependencies
  must satisfy (`ScdcClient`) rather than depending on a concrete implementation.
- **One implementation, sync and async.** The state machine is `plumbob::lts::run`; this
  crate supplies async I/O to it and adds no training logic, so the two cannot diverge.
- **Deterministic and testable.** The state machine is tested in `plumbob`; this crate's
  tests drive it through async I/O, including I/O that actually waits. No hardware is
  required for any test.
- **Policy at the right layer.** plumbob-async implements the spec, not strategy.
- **No unsafe code.** `#![forbid(unsafe_code)]`.
- **Type reuse over duplication.** All protocol and training types live in `plumbob` and
  are re-exported here. There is no parallel type hierarchy.
- **Attested releases.** Every release is published through a GitHub Actions workflow
  that signs the `.crate` package with [SLSA Build Level 2](https://slsa.dev) provenance.
  Verify with `gh attestation verify <file> --repo DracoWhitefire/plumbob-async`.

[`plumbob::lts::run`]: https://docs.rs/plumbob/latest/plumbob/lts/fn.run.html
