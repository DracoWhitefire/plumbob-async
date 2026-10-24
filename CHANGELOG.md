# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Breaking changes

- **`read_ced` is removed from `ScdcClient` and `SendScdcClient`, and the `CedCount` and
  `CedCounters` re-exports with it**, as in plumbob. Training never read the CED counters;
  a diagnostics method on the link training trait only made every implementation carry
  one. Implementations delete their `read_ced`. To read the counters, use the SCDC
  implementation directly (culvert-async's `read_ced`), or a trait of your own in the
  integration layer.
- **Requires the plumbob and hdmi-hal-async releases on hdmi-hal 0.6.** The PHY interface
  re-exported from hdmi-hal-async (`HdmiPhy`, `SendHdmiPhy`, `EqParams`, `FrlOutput`,
  `LaneEqParams`, `LanePatterns`, `LtpPattern`, `TxFfeLevel`, `TxFfeRestrictions`) is now
  built on hdmi-hal 0.6's types: a PHY implementing the previous hdmi-hal-async's
  `HdmiPhy` no longer fits `FrlTrainer`. Move plumbob, hdmi-hal-async and hdmi-hal
  alongside this crate; the traits themselves are unchanged. `HdmiForumFrl` is
  display-types' and is unaffected.

### Changed

- **`UpdateFlags::ced_update` and `rsed_update` (re-exported from plumbob) are documented
  as what they are seen to be:** set by the sink and kept until the source clears them, a
  hint at most. They no longer say the counters "have been updated".
- **The docs count seven `ScdcClient` operations**, not eight, now that `read_ced` is gone,
  and the test sink no longer claims to return CED counts.

- **Part of Ashlar** — the README now opens with a block naming the
  [Ashlar](https://ashlar-display.org) stack and linking the other crates in it. Keywords
  are aligned across the stack, with `ashlar` and `hdmi` on every crate; `ashlar` and
  `async` are added, `frl` and `embedded` are dropped. Categories gain `no-std::no-alloc`.
  The crate's `homepage` is now the Ashlar website.

## [0.2.0] - 2026-10-17

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
- **`TrainingOutcome::FallbackRequired` carries a `reason`** (a `FallbackReason`), and
  `TrainingConfig`, `LtpReq`, `FfeLevels`, `FrlConfig` and `TrainingEvent` follow
  plumbob's new definitions.
- **`TrainingError` reports what happened to the link**, as in plumbob: an SCDC or PHY
  error is followed by LTS:L, the variants are `Scdc { error, exit }` and
  `Phy { error, exit }` with `exit` a `TmdsExit`, and a fallback whose LTS:L fails is
  `ExitFailed { reason, error }`. Turn the exit after an error off with
  `TrainingConfig::exit_to_tmds_on_error`; while the sink sets `FLT_no_timeout` an error
  holds the link instead (`TmdsExit::HeldUnderTest`). A rate list the procedure cannot run
  is rejected before any I/O with `InvalidRates { index, rate }` or `NoRates`, and
  `TrainingError` is now `#[non_exhaustive]`.
- **`TrainingOutcome::NoTimeoutHold { rate }`**, as in plumbob: while the sink sets
  `FLT_no_timeout`, a poll cap that runs out leaves the link as it is for the test
  equipment instead of falling back. The enum was already `#[non_exhaustive]`, so a match
  compiles unchanged and reaches its wildcard arm; handle the variant explicitly, since
  the link is left in FRL.
- **`train` and `train_at_rate` return `Trained`**, as in plumbob: the outcome with the
  attempt's `TrainingWarning`s, read with `warnings()` (a slice) or `iter_warnings()`, the
  same API with and without `alloc` (`Trained`, `TrainingWarning` and `MAX_WARNINGS` are
  re-exported).
- **The traced methods return the trace whatever the result**, as in plumbob:
  `(Result<Trained, TrainingError>, TrainingTrace)` instead of
  `Result<(TrainingOutcome, TrainingTrace), TrainingError>`.
- **`TrainingTrace` records the list of rates**, as in plumbob: its `rate` field is
  replaced by `rates`, and `TrainingTrace::new` takes the rates.
- **Every struct variant of `TrainingOutcome`, `TrainingError`, `TrainingWarning` and
  `TrainingEvent` is `#[non_exhaustive]`**, as in plumbob, so a later release can add a
  field to one without breaking callers. Patterns need `..` (`TrainingOutcome::Success {
  achieved_rate, .. }`), and code outside plumbob cannot build these variants: tests that
  compare an outcome, error or trace with `assert_eq!` against a constructed value match
  on it instead (`matches!`, or a pattern with `..`).
- **The SCDC types mirror culvert's**, as in plumbob. `FrlConfig::rate` is renamed
  `frl_rate`. `FrlConfig`, `LtpRequests`, `CedCounters`, `UpdateFlags` and
  `SourceTestConfig` are `#[non_exhaustive]`: build `FrlConfig`, `LtpRequests` and
  `CedCounters` with `new(…)`, and `UpdateFlags` and `SourceTestConfig` with `new(…)` or
  by setting fields on the default. `UpdateFlags` has all seven `Update_0` flags and
  `SourceTestConfig` all six `Source_Test_Configuration` bits, and
  `TrainingEvent::SourceTestConfigRead` carries the register as read (`config`). An
  `ScdcClient` fills every field in `read_update_flags` and `read_source_test_config`, and
  its `clear_update_flags` takes a `ClearableUpdateFlags`: every flag but `RR_Test`, which
  the source never clears.
- **`plumbob` updated to 0.3.1**: `FrlTrainer` runs plumbob 0.3's state machine and
  re-exports its types; the entries marked "as in plumbob" come from it. See plumbob's
  changelog. 0.3.1 is needed for `Send` futures: plumbob 0.2.0 and 0.3.0 kept state in the
  training future that is not `Send`.
- **`hdmi-hal-async` updated to 0.2.1**: `FrlTrainer`'s PHY implements hdmi-hal-async
  0.2's `HdmiPhy`. Implementations must take the per-lane `send_ltp(LanePatterns)`, add
  `set_frl_output(FrlOutput)`, use `LtpPattern`'s variants (HDMI 2.1 values; `new` is
  removed) and apply the TxFFE level now carried in `LaneEqParams`. See hdmi-hal-async's
  changelog.
- **`display-types` updated to 0.4**, as across the stack.

### Added

- **`SendScdcClient`, the `Send` twin of `ScdcClient`**, for spawning training on a
  multi-threaded executor from code generic over the client and PHY: the same methods,
  each returning a future that is `Send`, with a `Send` error. Every `SendScdcClient` is
  an `ScdcClient` through a blanket implementation; implement one of the two. With it
  and hdmi-hal-async's `SendHdmiPhy` (now re-exported), the futures of every training
  method and of `exit_to_tmds` are `Send` in generic code (`train_with_events` given a
  `Send` callback). Bound the code by the twins alone: an added `+ ScdcClient` or
  `+ HdmiPhy` bound makes the futures opaque again. Import only the trait the code is
  bounded by: with both in scope, method calls on a twin implementation are ambiguous.
- **The tester's TxFFE restrictions reach the PHY**, as in plumbob. The three TxFFE bits
  of `Source_Test_Configuration` (pre-shoot only, de-emphasis only, no FFE) go to
  `HdmiPhy::adjust_equalization` as `TxFfeRestrictions` (re-exported), with every
  equalization of the attempt; when a `Source_Test_Update` seen on any LTS:3 or LTS:P poll
  changes them, the lanes' levels are sent again with the new restrictions at once. A
  `Source_Test_Update` is handled on every such poll, also without `FLT_update`. How to
  combine the bits is the PHY's (see hdmi-hal-async 0.2.1 and hdmi-hal 0.5.1).
- **`FrlTrainer::train(rates, config)`** trains over a list of rates, stepping down when
  the sink requests it; `train_at_rate` is `train` with one rate. `train_traced` joins
  `train_at_rate_traced`.
- **`FrlTrainer::phy`, `phy_mut`, `scdc` and `scdc_mut`**, as in plumbob.
- Re-exports of `HdmiForumFrl`, the async `HdmiPhy` and the PHY types, from
  `hdmi-hal-async`, so callers and PHY implementations need no direct dependency on it.
- **`FrlTrainer::exit_to_tmds`** — LTS:L on demand, as in plumbob: to take an FRL link
  down when the display is disabled or unplugged, before a mode change, or after a dropped
  `train` future.
- **Cancellation is documented**: `train` is not cancel-safe, since a dropped future
  cannot run LTS:L; the README and architecture document say to call `exit_to_tmds`
  afterwards and to prefer the poll limits over external timeouts. For a wall-clock
  deadline or a cancel signal, the documented way is plumbob's: the `ScdcClient` fails one
  polled read with its own error, so training stops through LTS:L and `train` returns
  `TrainingError::Scdc`, with both ends in TMDS (unless `exit_to_tmds_on_error` is off or
  the sink sets `FLT_no_timeout`).
- Re-exports of `FallbackReason`, `LtpRequests`, `UpdateFlags`, `ClearableUpdateFlags`,
  `SourceTestConfig`, `TmdsExit` and `ExitError`, so an `ScdcClient` implementation needs
  no direct dependency on plumbob.
- **`FrlTrainer::train_with_events`** delivers each `TrainingEvent` to a callback as it
  occurs, and `TrainingEvent` is re-exported without the `alloc` feature, so targets
  without an allocator get the full reasoning too.
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
- **A test covers a `Source_Test_Update` during training** — the test sink can change
  `Source_Test_Configuration` in LTS:3, and the test checks that the new register is read,
  the flag cleared and the new TxFFE restrictions sent to the PHY. The test it replaces
  passed whether or not the flag was raised.
- **Publishing uses crates.io trusted publishing** — `publish.yml` obtains a short-lived
  token through OIDC (`rust-lang/crates-io-auth-action`, revoked at the end of the job, and
  short-lived regardless) instead of the long-lived `CARGO_REGISTRY_TOKEN` secret. The
  upload job runs in the `release` environment, which only `v*` tags can deploy to and
  which requires a maintainer's approval. Quality gates moved to a separate `verify` job,
  so no dependency build scripts run and no cache is restored where publishing credentials
  exist. The attested `.crate` is kept as a separate copy; after upload the job fails if
  crates.io's checksum differs from it, and that same copy is attached to the GitHub
  release. Re-running a partially failed publish reuses an existing GitHub release.
  `publish.yml` refuses tags that do not match the `Cargo.toml` version, and gains a
  `workflow_dispatch` trigger for `release-tag` (tags pushed with `GITHUB_TOKEN` start no
  push-triggered workflows); dispatches against a non-tag ref are skipped.
- **Release summary before approval** — `publish.yml` gains a `summary` job that shows the
  crate, version and commit, changes since the previous published release (ignoring
  pre-release or malformed tags, and flagging a baseline that is not on crates.io), changes
  to workflow, toolchain and build configuration, manifest changes, the changelog section,
  and the `.crate` sha256 with a command to reproduce it, on the run page while the publish
  job waits for approval. The publish job refuses to attest a package whose sha256 differs
  from the reviewed one.
- **Release workflows** — `release-prep` picks the version bump with `cargo-semver-checks`
  (`auto`, or checks an explicit bump), stamps the changelog, refreshes `Cargo.lock` and
  opens a `Release vX.Y.Z` PR. `release-tag` checks that CI passed on `develop` (and says so
  when the run has not finished yet), promotes `develop` to `main` through a
  `release-promote/v*` branch and PR, fails if that merge fails, tags only when `main` holds
  exactly the promoted tree, and dispatches `publish.yml` for the tag; it also accepts a
  squash-merged release commit (`Release vX.Y.Z (#N)`). Both fail when started from a branch
  other than `develop`. Releases were previously tagged by hand.
- **No CI run on promotion PRs** — CI no longer runs on pull requests into `main`. The only
  ones are `release-tag`'s `release-promote/*` PRs, whose content already passed CI on
  `develop`; opened by `github-actions[bot]`, their runs would be held for approval and
  listed as `action_required` or failed.
- **Supply-chain hardening for CI** — every third-party action is pinned to a commit SHA
  of a release tag, with `dependabot.yml` keeping the pins and `Cargo.lock` current
  (lockfile-only: version requirements stay a deliberate choice). The Rust toolchain itself
  still follows `stable`. `cargo-semver-checks` and `cargo-llvm-cov` are installed as
  checksum-verified prebuilt binaries by the pinned `install-action`. The security audit
  also runs weekly, with read-only repository permissions.
- **README documents release provenance** — a badge shows whether crates.io requires trusted
  publishing for this crate, and "Verifying releases" now states which versions carry SLSA
  provenance and are uploaded through trusted publishing (from 0.2.0; it previously claimed
  every release was attested), that 0.1.0 was uploaded by hand with an API token, and how
  to look up a version's publishing details on crates.io.

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
