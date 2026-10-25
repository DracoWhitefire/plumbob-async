# Data Model

All protocol and training types used by plumbob-async are defined in `plumbob` and
re-exported here. This document lists the re-exports, the traits and type defined in
plumbob-async itself (`ScdcClient`, `SendScdcClient`, `FrlTrainer`), and the boundary at which `culvert-async` converts its
register-layer types into plumbob's vocabulary.

For field-level documentation of each re-exported type, see
[`plumbob`'s `doc/model.md`](https://github.com/ashlar-display/plumbob/blob/main/doc/model.md).

---

## Re-exported from `plumbob`

| Type                   | Kind   | Notes                                                                                                     |
|------------------------|--------|-----------------------------------------------------------------------------------------------------------|
| `LtpReq`               | enum   | One lane's request: 0x0 (trained), 0x1–0x8 (patterns), 0xE, 0xF, or `Reserved` (0x9–0xD)                  |
| `LtpRequests`          | struct | The requests for all four lanes; `new(…)` takes them in lane order                                        |
| `FfeLevels`            | struct | Highest TxFFE level index (0–7), written to `Config_1`                                                    |
| `FrlConfig`            | struct | `Config_1`: the rate (`frl_rate`) and FFE levels; `new(…)`                                                |
| `UpdateFlags`          | struct | All seven `Update_0` flags; `new(…)` (bit order) or set fields on the default                             |
| `ClearableUpdateFlags` | struct | The flags `clear_update_flags` takes: all but `RR_Test`                                                   |
| `SourceTestConfig`     | struct | All six `Source_Test_Configuration` bits; `new(…)` or set fields on the default                           |
| `TrainingConfig`       | struct | Per-attempt configuration: FFE levels, poll limits, retrain limit                                         |
| `Trained`              | struct | What `train` returns: the `outcome` and the attempt's warnings (`warnings()`)                             |
| `TrainingOutcome`      | enum   | `Success { achieved_rate, .. }`, `FallbackRequired { reason, .. }`, or `NoTimeoutHold { rate, .. }`       |
| `TrainingWarning`      | enum   | A non-fatal anomaly, such as an undefined LTP request; repeats merged                                     |
| `FallbackReason`       | enum   | Why an attempt ended without a link                                                                       |
| `TrainingError`        | enum   | Hard failure: `Scdc { error, exit }` or `Phy { error, exit }`; `ExitFailed`; `InvalidRates`; or `NoRates` |
| `TmdsExit`             | enum   | What LTS:L did after an I/O failure: `Exited`, `Failed(ExitError)`, `Skipped` or `HeldUnderTest`          |
| `ExitError`            | struct | A failed LTS:L: each end's first error (`scdc`, `phy`)                                                    |
| `TrainingEvent`        | enum   | One trace event                                                                                           |
| `TrainingTrace`        | struct | The rates, config and ordered events of an attempt (`alloc` feature)                                      |

---

## Defined here

### `ScdcClient`

The trait defined in this crate: the async counterpart of `plumbob::ScdcClient`, with
the same seven operations, meaning and implementer obligations, each an `async fn`.
It is a distinct trait: a type implementing the sync trait does not implement this one.
Async transports implement this trait; sync transports implement `plumbob::ScdcClient`.

### `SendScdcClient`

`ScdcClient`'s `Send` twin: the same operations, each returning a future that is `Send`,
with a `Send` error. Every `SendScdcClient` is an `ScdcClient` through a blanket
implementation; a type implements one of the two. Implement it, with hdmi-hal-async's
`SendHdmiPhy`, to spawn training on a multi-threaded executor from generic code.

### `FrlTrainer<C, P>`

The same shape as `plumbob::FrlTrainer<C, P>`, with different trait bounds:

|                  | plumbob                  | plumbob-async                  |
|------------------|--------------------------|--------------------------------|
| `C` bound        | `plumbob::ScdcClient`    | `plumbob_async::ScdcClient`    |
| `P` bound        | `hdmi_hal::phy::HdmiPhy` | `hdmi_hal_async::phy::HdmiPhy` |
| Training methods | `fn`                     | `async fn`                     |
| State machine    | `plumbob::lts::run`      | `plumbob::lts::run`            |

`new`, `into_parts` and the accessors (`scdc`, `scdc_mut`, `phy`, `phy_mut`) are not
async; the training methods and `exit_to_tmds` are.

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
  have trained. LTS:L still runs afterwards, unless `exit_to_tmds_on_error` is off or
  the sink had set `FLT_no_timeout` (`HeldUnderTest`), and
  `exit` says whether both ends reached TMDS. `TrainingError::ExitFailed` is a fallback
  whose LTS:L failed. See plumbob's `model.md` for the details.

---

## The culvert-async boundary

`culvert-async` implements `plumbob_async::ScdcClient` for its `Scdc<T>` behind a
`plumbob-async` cargo feature, as `culvert` does for `plumbob::ScdcClient`: each trait
method calls one `culvert-async` method and converts culvert's register types to
plumbob's (`culvert::LtpRequests` → `plumbob::LtpRequests`, and so on). plumbob's SCDC
types mirror culvert's field for field, so each conversion is one-to-one, and the
conversions are the same as culvert's, since both crates use culvert's types;
`culvert-async`'s own types are unchanged.
