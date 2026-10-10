# Data Model

All protocol and training types used by plumbob-async are defined in `plumbob` and
re-exported here. This document lists the re-exports, the two types defined in
plumbob-async itself, and the boundary at which `culvert-async` converts its
register-layer types into plumbob's vocabulary.

For field-level documentation of each re-exported type, see
[`plumbob/doc/model.md`](../../plumbob/doc/model.md).

---

## Re-exported from `plumbob`

| Type | Kind | Notes |
|---|---|---|
| `LtpReq` | enum | One lane's request: 0x0 (trained), 0x1–0x8 (patterns), 0xE, 0xF, or `Reserved` (0x9–0xD) |
| `LtpRequests` | struct | The requests for all four lanes |
| `FfeLevels` | struct | Highest TxFFE level index (0–7), written to `Config_1` |
| `FrlConfig` | struct | `Config_1`: the rate and FFE levels |
| `UpdateFlags` | struct | The `Update_0` training flags read and cleared |
| `SourceTestConfig` | struct | `FLT_no_timeout` from `Source_Test_Configuration` |
| `CedCount` | struct | 15-bit per-lane character error count |
| `CedCounters` | struct | Per-lane `Option<CedCount>`; `lane3` is `None` in 3-lane mode |
| `TrainingConfig` | struct | Per-attempt configuration: FFE levels, poll limits, retrain limit |
| `Trained` | struct | What `train` returns: the `outcome` and the attempt's `TrainingWarning`s |
| `TrainingOutcome` | enum | `Success { achieved_rate }`, `FallbackRequired { reason }`, or `NoTimeoutHold { rate }` |
| `TrainingWarning` | enum | A non-fatal anomaly, such as an undefined LTP request; repeats merged |
| `FallbackReason` | enum | Why an attempt ended without a link |
| `TrainingError` | enum | Hard I/O failure: `Scdc { error, exit }` or `Phy { error, exit }`; or `ExitFailed` |
| `TmdsExit` | enum | What LTS:L did after an I/O failure: `Exited`, `Failed(ExitError)`, or `Skipped` |
| `ExitError` | struct | A failed LTS:L: each end's first error (`scdc`, `phy`) |
| `TrainingEvent` | enum | One trace event |
| `TrainingTrace` | struct | The rates, config and ordered events of an attempt (`alloc` feature) |

---

## Defined here

### `ScdcClient`

The only trait defined in this crate: the async counterpart of `plumbob::ScdcClient`,
with the same eight operations, meaning and implementer obligations, each an `async fn`.
It is a distinct trait: a type implementing the sync trait does not implement this one.
Async transports implement this trait; sync transports implement `plumbob::ScdcClient`.

### `FrlTrainer<C, P>`

The same shape as `plumbob::FrlTrainer<C, P>`, with different trait bounds:

| | plumbob | plumbob-async |
|---|---|---|
| `C` bound | `plumbob::ScdcClient` | `plumbob_async::ScdcClient` |
| `P` bound | `hdmi_hal::phy::HdmiPhy` | `hdmi_hal_async::phy::HdmiPhy` |
| Training methods | `fn` | `async fn` |
| State machine | `plumbob::lts::run` | `plumbob::lts::run` |

`new` and `into_parts` are not async; only the training methods are.

---

## `TrainingOutcome` vs. `TrainingError`

These are `plumbob`'s types, re-exported unchanged:

- **`FallbackRequired { reason }`** — no listed rate trained; `reason` says why, and the
  sink and PHY are back in TMDS. A normal protocol outcome; the caller decides what to do
  next.
- **`NoTimeoutHold { rate }`** — the sink is under compliance test (`FLT_no_timeout`) and
  the poll cap ran out; the link is left as it is for the test equipment.
- **`TrainingError::Scdc { error, exit }` / `TrainingError::Phy { error, exit }`** — a
  hard I/O failure from the async SCDC client or PHY, unrelated to whether the link could
  have trained. LTS:L still runs afterwards, unless `exit_to_tmds_on_error` is off, and
  `exit` says whether both ends reached TMDS. `TrainingError::ExitFailed` is a fallback
  whose LTS:L failed. See plumbob's `model.md` for the details.

---

## The culvert-async boundary

`culvert-async` implements `plumbob_async::ScdcClient` for its `Scdc<T>` behind a
`plumbob-async` cargo feature, as `culvert` does for `plumbob::ScdcClient`: each trait
method calls one `culvert-async` method and converts culvert's register types to
plumbob's (`culvert::LtpRequests` → `plumbob::LtpRequests`, and so on). The conversions
are the same as culvert's, since both crates use culvert's types; `culvert-async`'s own
types are unchanged.
