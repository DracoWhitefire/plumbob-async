# Testing Strategy

plumbob-async's test suite mirrors plumbob's: all tests run against scripted async
implementations of `ScdcClient` and `HdmiPhy`. No real hardware, no network, and no
async runtime are required beyond the executor used to drive the tests.

## Executor

Tests use [`pollster`](https://docs.rs/pollster) via `#[pollster::test]`. Pollster is a
minimal single-threaded executor that blocks on a future inline — it has no scheduler,
no I/O reactor, and no thread pool. It is a dev-dependency only and does not appear in
the library's dependency graph.

The library itself imposes no executor constraint. Any runtime that can drive an `async fn`
(Embassy, Tokio, async-std) can use `FrlTrainer`.

## Test structure

All tests live in `src/trainer/`, alongside the code they cover.

### Sim harness (`src/trainer/sim.rs`)

Two types implement the async traits under `#[cfg(test)]`:

- `SimScdc` — a scripted `ScdcClient` backed by a `VecDeque`. Each `.await` on
  `read_training_status` pops the next entry, simulating a sink's phase-by-phase responses.
  Errors are injected by pushing a scripted error value or setting `fail_write_frl_config`
  or `fail_read_ced`.
- `MockPhy` — a minimal `HdmiPhy` that records the last `set_frl_rate` call and the last
  `send_ltp` pattern, and supports injectable errors via `fail_set_frl_rate` and
  `fail_send_ltp`.

Helper functions (`not_ready`, `flt_ready`, `frl_started`, `ltp`, `cfg`) construct
`TrainingStatus` and `TrainingConfig` fixtures to keep individual tests concise.

### State machine tests (`src/trainer/tests.rs`)

Every branch in `train_at_rate` has a corresponding test:

- Successful training: `flt_ready` and `frl_start` assert after N polls; `ltp_req`
  transitions through one or more patterns before reaching `None`
- Each phase timing out independently (phases 2, 3, and 4)
- Each phase succeeding on the first iteration (`after_iterations: 0`)
- `TrainingError::Scdc` propagating from each of the three `ScdcClient` methods
- `TrainingError::Phy` propagating from `set_frl_rate` and `send_ltp`
- `into_parts` recovering the SCDC client and PHY after a completed attempt
- `send_ltp` receiving the correct `LtpPattern` raw value for each `LtpReq` variant

### Trace tests (requires `alloc` feature)

The `alloc`-gated `trace_tests` sub-module exercises `train_at_rate_traced` and asserts on:

- Event order and variant presence for all terminal states (success, three timeout variants)
- `LtpPatternRequested` emitted only on `ltp_req` transitions, not once per poll
- `after_iterations` / `iterations_elapsed` counts matching the scripted queue
- `TrainingTrace.config` matching the `TrainingConfig` passed to `train_at_rate_traced`,
  so that timeout counts in events are interpretable against the configured limits

## Coverage

CI measures line coverage with `cargo-llvm-cov` over the `std` feature set. The baseline
is stored in `.coverage-baseline`; CI fails if coverage drops more than 0.1% below it.
New logic without tests will likely trip this.

## Philosophy

The training loop runs identically against simulated and real `ScdcClient` / `HdmiPhy`
implementations. The only difference from the sync tests in `plumbob` is the `async`
keyword on each test function and the `.await` on each trainer call — the scripted inputs,
the assertions, and the coverage expectations are the same. A test that cannot run with
a scripted `SimScdc` does not belong in this repository. Hardware is never a test
dependency.
