use crate::{CedCounters, FrlConfig, TrainingStatus};

/// Async typed SCDC interface required by the FRL training state machine.
///
/// This is the async counterpart to [`plumbob::ScdcClient`]. Identical semantics;
/// each method is `async fn`. Implementations are provided by SCDC crates — for
/// example, `culvert-async` will implement this for its `Scdc<T>` type.
///
/// The trait is intentionally minimal: it exposes only the register operations the
/// training state machine needs. Callers that need lower-level SCDC access should
/// use the underlying transport directly.
pub trait ScdcClient {
    /// The error type returned by transport operations.
    type Error;

    /// Write FRL rate and configuration to Config_0.
    ///
    /// Async counterpart to [`plumbob::ScdcClient::write_frl_config`].
    async fn write_frl_config(&mut self, config: FrlConfig) -> Result<(), Self::Error>;

    /// Read `flt_ready`, `frl_start`, and `ltp_req` from Status_Flags.
    ///
    /// Async counterpart to [`plumbob::ScdcClient::read_training_status`].
    async fn read_training_status(&mut self) -> Result<TrainingStatus, Self::Error>;

    /// Read per-lane character error counts for equalization feedback.
    ///
    /// Async counterpart to [`plumbob::ScdcClient::read_ced`].
    async fn read_ced(&mut self) -> Result<CedCounters, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CedCount, LtpReq};

    struct MockScdc {
        status: TrainingStatus,
    }

    impl ScdcClient for MockScdc {
        type Error = core::convert::Infallible;

        async fn write_frl_config(&mut self, _config: FrlConfig) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn read_training_status(&mut self) -> Result<TrainingStatus, Self::Error> {
            Ok(self.status)
        }

        async fn read_ced(&mut self) -> Result<CedCounters, Self::Error> {
            Ok(CedCounters {
                lane0: Some(CedCount::new(0)),
                lane1: Some(CedCount::new(0)),
                lane2: Some(CedCount::new(0)),
                lane3: None,
            })
        }
    }

    // Verify the trait is well-formed and that a mock can implement it.
    #[test]
    fn mock_scdc_implements_trait() {
        let _ = MockScdc {
            status: TrainingStatus {
                flt_ready: false,
                frl_start: false,
                ltp_req: LtpReq::None,
            },
        };
    }
}
