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

### CED-driven equalization

Mirrors the same open item in `plumbob`: `read_ced` is called on every LTP loop
iteration but the returned `CedCounters` are not yet acted on. Once `plumbob` drives
`adjust_equalization` from CED feedback, the same logic will be carried here.

### `culvert-async` integration

`culvert-async` will implement `plumbob_async::ScdcClient` for its `Scdc<T>` via a
`plumbob-async` cargo feature, mirroring the sync relationship between `culvert` and
`plumbob`. Until then, callers supply their own `ScdcClient` implementation.

### `LinkTrainer` trait

Once the integration layer defines an async `LinkTrainer` trait, `plumbob-async` will
implement it, decoupling the integration layer from this crate specifically.
