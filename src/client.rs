use crate::{
    CedCounters, ClearableUpdateFlags, FrlConfig, LtpRequests, SourceTestConfig, UpdateFlags,
};

/// Async typed SCDC interface required by the FRL training state machine.
///
/// This is the async counterpart to [`plumbob::ScdcClient`]: the same methods with the
/// same meaning, each an `async fn`. See that trait for what implementations must
/// guarantee, including where the poll interval belongs (inside the polled methods; an
/// async implementation can await a timer there). `culvert-async` implements it for its
/// `Scdc<T>`.
pub trait ScdcClient {
    /// Error type returned by SCDC operations.
    type Error;

    /// Read `FLT_ready` (`Status_Flags_0` bit 6).
    ///
    /// Async counterpart to [`plumbob::ScdcClient::read_flt_ready`].
    async fn read_flt_ready(&mut self) -> Result<bool, Self::Error>;

    /// Read the `Update_0` flags the state machine uses.
    ///
    /// Async counterpart to [`plumbob::ScdcClient::read_update_flags`].
    async fn read_update_flags(&mut self) -> Result<UpdateFlags, Self::Error>;

    /// Clear the given `Update_0` flags (write 1 to clear).
    ///
    /// Async counterpart to [`plumbob::ScdcClient::clear_update_flags`].
    async fn clear_update_flags(&mut self, flags: ClearableUpdateFlags) -> Result<(), Self::Error>;

    /// Read the per-lane link training requests from `Status_Flags_1/2`.
    ///
    /// Async counterpart to [`plumbob::ScdcClient::read_ltp_requests`].
    async fn read_ltp_requests(&mut self) -> Result<LtpRequests, Self::Error>;

    /// Read `Source_Test_Configuration` (0x35).
    ///
    /// Async counterpart to [`plumbob::ScdcClient::read_source_test_config`].
    async fn read_source_test_config(&mut self) -> Result<SourceTestConfig, Self::Error>;

    /// Write `Config_0` with read requests disabled and `FLT_no_retrain` clear.
    ///
    /// Async counterpart to [`plumbob::ScdcClient::write_config_0_defaults`].
    async fn write_config_0_defaults(&mut self) -> Result<(), Self::Error>;

    /// Write `Config_1`: the FRL rate and FFE levels.
    ///
    /// Async counterpart to [`plumbob::ScdcClient::write_frl_config`].
    async fn write_frl_config(&mut self, config: FrlConfig) -> Result<(), Self::Error>;

    /// Read the per-lane character error counters, for diagnostics.
    ///
    /// Async counterpart to [`plumbob::ScdcClient::read_ced`].
    async fn read_ced(&mut self) -> Result<CedCounters, Self::Error>;
}
