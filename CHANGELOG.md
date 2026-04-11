# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **SLSA Build Level 2 provenance** — release artifacts are attested via
  `actions/attest-build-provenance` and verified with
  `gh attestation verify <file> --repo DracoWhitefire/plumbob-async`.

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
