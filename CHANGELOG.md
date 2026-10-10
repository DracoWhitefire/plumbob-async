# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Breaking changes

- **`FrlTrainer` runs plumbob's link training state machine.** Training now follows the
  HDMI 2.1 link training states (LTS:2 → LTS:3 → LTS:P, with LTS:4 and LTS:L) by running
  `plumbob::lts::run` over the async SCDC client and PHY, instead of a copy of plumbob's
  previous four-phase loop. Sync and async training are the same procedure, with the same
  outcomes, errors and traces; see plumbob's changelog for the procedure's changes.
- **`ScdcClient` has one method per register operation**, the async counterparts of
  plumbob's: `read_flt_ready`, `read_update_flags`, `clear_update_flags`,
  `read_ltp_requests`, `read_source_test_config`, `write_config_0_defaults`,
  `write_frl_config` and `read_ced`. `read_training_status` and the `TrainingStatus`
  re-export are removed.
- **`FrlTrainer::train(rates, config)`** trains over a list of rates, stepping down when
  the sink requests it; `train_at_rate` is `train` with one rate. `train_traced` joins
  `train_at_rate_traced`.
- **`TrainingOutcome::FallbackRequired` carries a `reason`** (`FallbackReason`, now
  re-exported), and `TrainingConfig`, `LtpReq`, `FfeLevels`, `FrlConfig` and
  `TrainingEvent` follow plumbob's new definitions.
- **`TrainingError` reports what happened to the link**, as in plumbob: an SCDC or PHY
  error is followed by LTS:L, the variants are `Scdc { error, exit }` and
  `Phy { error, exit }` with `exit` a `TmdsExit`, and a fallback whose LTS:L fails is
  `ExitFailed { reason, scdc, phy }`. Turn the exit after an error off with
  `TrainingConfig::exit_to_tmds_on_error`.
- **`train` and `train_at_rate` return `Trained`**, as in plumbob: the outcome with the
  attempt's `TrainingWarning`s (`Trained`, `TrainingWarning` and `MAX_WARNINGS` are
  re-exported).
- **The traced methods return the trace whatever the result**, as in plumbob:
  `(Result<Trained, TrainingError>, TrainingTrace)`.
- **`hdmi-hal-async` updated to 0.2**: `FrlTrainer`'s PHY implements hdmi-hal-async
  0.2's `HdmiPhy`. Implementations must take the per-lane `send_ltp(LanePatterns)`, add
  `set_frl_output(FrlOutput)`, use `LtpPattern`'s variants (HDMI 2.1 values; `new` is
  removed) and apply the TxFFE level now carried in `LaneEqParams`. See hdmi-hal-async's
  changelog.
- **`display-types` updated to 0.4**, as across the stack.

### Added

- **`FrlTrainer::phy`, `phy_mut`, `scdc` and `scdc_mut`**, as in plumbob.
- Re-exports of `HdmiForumFrl`, the async `HdmiPhy` and the PHY types, from
  `hdmi-hal-async`, so callers and PHY implementations need no direct dependency on it.
- **`FrlTrainer::exit_to_tmds`** — LTS:L on demand, as in plumbob: to take an FRL link
  down when the display is disabled or unplugged, before a mode change, or after a dropped
  `train` future.
- **Cancellation is documented**: `train` is not cancel-safe, since a dropped future
  cannot run LTS:L; the README and architecture document say to call `exit_to_tmds`
  afterwards and to prefer the poll limits over external timeouts.
- Re-exports of `FallbackReason`, `LtpRequests`, `UpdateFlags`, `SourceTestConfig`,
  `TmdsExit` and `ExitError`.
  `TrainingEvent` is re-exported without the `alloc` feature, and
  `FrlTrainer::train_with_events` delivers each event to a callback as it occurs, so
  targets without an allocator get the full reasoning too.

- **SLSA Build Level 2 provenance** — release artifacts are attested via
  `actions/attest-build-provenance` and verified with
  `gh attestation verify <file> --repo DracoWhitefire/plumbob-async`.

### Internal

- **CI builds for a `no_std` target** — the `Build (no_std)` and `Build (alloc only)`
  steps now build for `thumbv7em-none-eabi`. They previously built for the host, where
  `std` is always available, so they could not catch a dependency that enables `std` (as
  hdmi-hal did through `display-types`).
  The publish workflow runs the same build steps.
- **Tests compile without the `alloc` feature** — the tests that use the traced methods
  are gated on `alloc`, and CI and the publish workflow also run the tests with no
  features, so plain `cargo test` builds again.

## [0.1.0] - 2026-04-04

### Added

**Async FRL training state machine**

- `FrlTrainer<C, P>` — central type owning an async `ScdcClient` and async `HdmiPhy`;
  reusable across rate-fallback attempts without reconstruction between calls
- `async fn train_at_rate` — runs the full four-phase FRL training sequence, yielding to
  the executor between poll iterations rather than spinning
- `async fn train_at_rate_traced` — traced variant returning `(TrainingOutcome, TrainingTrace)`;
  requires the `alloc` feature

**`ScdcClient` trait**

- `ScdcClient` — async mirror of `plumbob::ScdcClient`. Methods:
  `async fn write_frl_config`, `async fn read_training_status`, `async fn read_ced`.
- Uses stable `async fn` in traits (AFIT, stabilised in Rust 1.75); no `async-trait`
  proc-macro dependency.

**Re-exports from `plumbob`**

All protocol and training types are re-exported at the crate root rather than duplicated:
`LtpReq`, `FfeLevels`, `FrlConfig`, `TrainingStatus`, `CedCount`, `CedCounters`,
`TrainingConfig`, `TrainingOutcome`, `TrainingError`. `TrainingEvent` and `TrainingTrace`
are re-exported under the `alloc` feature.

**Project infrastructure**

- `#![no_std]`, `#![forbid(unsafe_code)]`
- `#![allow(async_fn_in_trait)]` — targets single-threaded embedded executors (e.g.
  Embassy) where `Send` bounds on futures are not required, following the same
  decision made by `embedded-hal-async`.
- `alloc` feature enables `TrainingTrace` and `train_at_rate_traced`; `std` implies `alloc`.
- Full rustdoc coverage enforced via `cargo rustdoc --features std -- -D missing_docs`.
- 100% line coverage measured with `cargo-llvm-cov`; baseline stored in
  `.coverage-baseline`.
- Unit tests using `pollster` as the test executor; 49 tests covering all state machine
  branches, all error propagation paths, and all trace event sequences across both feature
  tiers.
