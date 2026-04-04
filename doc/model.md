# Data Model

All protocol and training types used by plumbob-async are defined in `plumbob` and
re-exported here. This document describes the re-export structure, the two types that
are defined in plumbob-async itself, and the boundary at which culvert-async will
eventually convert its register-layer types into plumbob's vocabulary.

For full field-level documentation of each re-exported type, see
[`plumbob/doc/model.md`](../../plumbob/doc/model.md).

---

## Re-exported from `plumbob`

| Type | Kind | Notes |
|---|---|---|
| `LtpReq` | enum | Sink LTP request; `None` is the phase 4 exit condition |
| `FfeLevels` | enum | FFE level count advertised in Config_0 |
| `FrlConfig` | struct | Rate, FFE levels, and `dsc_frl_max` written in phase 1 |
| `TrainingStatus` | struct | `flt_ready`, `frl_start`, `ltp_req` from a single poll |
| `CedCount` | struct | 15-bit per-lane character error count |
| `CedCounters` | struct | Per-lane `Option<CedCount>`; `lane3` is `None` in 3-lane mode |
| `TrainingConfig` | struct | Per-attempt configuration with timeout iteration counts |
| `TrainingOutcome` | enum | `Success { achieved_rate }` or `FallbackRequired` |
| `TrainingError` | enum | Hard I/O failure: `Scdc(e)` or `Phy(e)` |
| `TrainingEvent` | enum | Single trace event (`alloc` feature) |
| `TrainingTrace` | struct | Full ordered event log for one attempt (`alloc` feature) |

The `From<LtpReq> for LtpPattern` conversion defined in `plumbob` is also used here
directly: `status.ltp_req.into()` produces an `hdmi_hal::phy::LtpPattern` for the
`HdmiPhy::send_ltp` call.

---

## Defined here

### `ScdcClient`

The only trait defined in this crate. Async mirror of `plumbob::ScdcClient` — identical
contract, `async fn` methods:

```rust
pub trait ScdcClient {
    type Error;

    async fn write_frl_config(&mut self, config: FrlConfig) -> Result<(), Self::Error>;
    async fn read_training_status(&mut self) -> Result<TrainingStatus, Self::Error>;
    async fn read_ced(&mut self) -> Result<CedCounters, Self::Error>;
}
```

The `ScdcClient` in plumbob-async is a distinct trait from `plumbob::ScdcClient`. A
type that implements the sync trait does not automatically implement the async one.
Implementors for async transports implement this trait; implementors for sync transports
implement `plumbob::ScdcClient`.

### `FrlTrainer<C, P>`

The central training type. Identical shape to `plumbob::FrlTrainer<C, P>` but with
different trait bounds:

| | plumbob | plumbob-async |
|---|---|---|
| `C` bound | `plumbob::ScdcClient` | `plumbob_async::ScdcClient` |
| `P` bound | `hdmi_hal::phy::HdmiPhy` | `hdmi_hal_async::phy::HdmiPhy` |
| `train_at_rate` | `fn` | `async fn` |

`new` and `into_parts` are not async; only the training methods are.

---

## `TrainingOutcome` vs. `TrainingError`

These are the same types as in `plumbob`, re-exported unchanged. The distinction is the
same:

- **`FallbackRequired`** — the link did not converge within the configured timeouts.
  Normal protocol outcome; the caller decides whether to retry at a lower rate or fall
  back to TMDS.
- **`TrainingError::Scdc(e)` / `TrainingError::Phy(e)`** — hard I/O failure from the
  async SCDC client or PHY. Unrelated to whether the link could have trained at this rate.

---

## The culvert-async boundary

When `culvert-async` is built, it will implement `plumbob_async::ScdcClient` for its
`Scdc<T>` via a `plumbob-async` cargo feature, mirroring the sync pattern in `culvert`.
The conversion at that impl boundary will be identical in structure:

```rust
// culvert-async, behind feature = "plumbob-async"
impl<T: ScdcTransport> plumbob_async::ScdcClient for Scdc<T> {
    async fn read_training_status(&mut self) -> Result<plumbob::TrainingStatus, ...> {
        let flags = self.read_status_flags().await?;
        Ok(plumbob::TrainingStatus {
            flt_ready: flags.flt_ready,
            frl_start: flags.frl_start,
            ltp_req: flags.ltp_req.into(), // culvert::LtpReq → plumbob::LtpReq
        })
    }
    // ...
}
```

The same reasoning applies as in the sync case: the `From` impls live in a feature-gated
module in `culvert-async`; `culvert-async`'s own types are unchanged; the conversion is
confined to the impl boundary.
