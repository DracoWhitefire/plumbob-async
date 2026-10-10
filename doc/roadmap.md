# Roadmap

## Released

### 0.1.0

- Async four-phase FRL training state machine (`FrlTrainer`, `train_at_rate`)
- `ScdcClient` trait — async typed SCDC interface for the training procedure
- `TrainingConfig`, `TrainingOutcome`, `TrainingError` — re-exported from `plumbob`
- Owned protocol types re-exported from `plumbob`: `LtpReq`, `FfeLevels`, `FrlConfig`,
  `TrainingStatus`, `CedCount`, `CedCounters`
- `TrainingTrace` and `train_at_rate_traced` (`alloc` feature)
- `no_std` support with `alloc` and `std` feature flags

## Planned

### `LinkTrainer` trait

Once the integration layer defines an async `LinkTrainer` trait, `plumbob-async` will
implement it, decoupling the integration layer from this crate specifically.
