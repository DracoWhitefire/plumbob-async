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
- **`display-types` updated to 0.4**, as across the stack.

### Added

- Re-exports of `FallbackReason`, `LtpRequests`, `UpdateFlags` and `SourceTestConfig`.
  `TrainingEvent` is re-exported without the `alloc` feature.

- **SLSA Build Level 2 provenance** — release artifacts are attested via
  `actions/attest-build-provenance` and verified with
  `gh attestation verify <file> --repo DracoWhitefire/plumbob-async`.

### Internal

- **CI builds for a `no_std` target** — the `Build (no_std)` and `Build (alloc only)`
  steps now build for `thumbv7em-none-eabi`. They previously built for the host, where
  `std` is always available, so they could not catch a dependency that enables `std` (as
  hdmi-hal did through `display-types`).
  The publish workflow runs the same build steps.

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
