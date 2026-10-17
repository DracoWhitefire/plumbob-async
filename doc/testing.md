# Testing Strategy

The training state machine is `plumbob::lts::run`, and plumbob's test suite covers it:
every link training state, poll limit, per-lane rule, fallback and trace event.
plumbob-async adds no training logic, so its tests check the part it does add — driving
that state machine through async I/O. No real hardware, network or async runtime is
required beyond the executor used to drive the tests.

## Executor

Tests use [`pollster`](https://docs.rs/pollster) via `#[pollster::test]`. Pollster is a
minimal single-threaded executor that blocks on a future inline — it has no scheduler,
no I/O reactor, and no thread pool. It is a dev-dependency only and does not appear in
the library's dependency graph.

The library itself imposes no executor constraint. Any runtime that can drive an `async fn`
(Embassy, Tokio, async-std) can use `FrlTrainer`.

## Test structure (`src/trainer/tests.rs`)

- `Sink` — a scripted async `ScdcClient`: `FLT_ready` after N polls, then each set of
  per-lane requests posted with `FLT_update` once `Config_1` holds an FRL rate, then
  `FRL_start`. It can raise `Source_Test_Update` before training, change
  `Source_Test_Configuration` (raising `Source_Test_Update`) once LTS:3 begins, fail any
  one operation, and make every operation wait.
- `Phy` — an async `HdmiPhy` that records the rates, patterns, output modes and
  equalization it is given, with the same failure and waiting options.
- Waiting is real: `WaitOnce` is a future that returns `Pending` once and wakes itself
  before completing, so the executor has to poll the training future again.

The tests check:

- A full training run through async I/O, with the per-lane patterns, the gap-only output
  and the TxFFE reset and raise reaching the PHY
- **I/O that waits gives the same result** as I/O that does not, compared trace for trace
  (requires `alloc`)
- A `Source_Test_Update` during LTS:3: the new register read, the flag cleared, and the
  new `TxFfeRestrictions` sent to the PHY with the lanes' levels
- A fallback returning the PHY to TMDS
- `TrainingError::Scdc` and `TrainingError::Phy` carrying the error from every SCDC and
  PHY operation the state machine uses
- The traced methods recording the rates, the config and the events (requires `alloc`)
- `train_with_events` reporting each event as it occurs over waiting I/O, without `alloc`
- `exit_to_tmds` over async I/O: the LTS:L steps reaching both ends, each end's error on
  failure, and recovering a `train` future dropped mid-training (polled by hand over
  waiting I/O, then dropped with the sink still configured for FRL)

The tests that compare or inspect traces are compiled only with the `alloc` feature. CI
runs the suite with the `std` feature and again with no features, so it builds in both.

## Coverage

CI measures line coverage with `cargo-llvm-cov` over the `std` feature set. The baseline
is stored in `.coverage-baseline`; CI fails if coverage drops more than 0.1% below it.

## Philosophy

The state machine runs identically against simulated and real `ScdcClient` / `HdmiPhy`
implementations, and identically sync and async, because it is the same function. Tests
of training behaviour belong in `plumbob`; tests here cover the async driver. Hardware is
never a test dependency.
