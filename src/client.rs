use crate::{
    CedCounters, ClearableUpdateFlags, FrlConfig, LtpRequests, SourceTestConfig, UpdateFlags,
};

/// Async typed SCDC interface required by the FRL training state machine.
///
/// This is the async counterpart to [`plumbob::ScdcClient`]: the same methods with the
/// same meaning, each an `async fn`. See that trait for what implementations must
/// guarantee, including where the poll interval belongs (inside the polled methods; an
/// async implementation can await a timer there) and how to stop training early ("Stopping
/// early"): for a deadline or a cancel signal, return one error of your own from a polled
/// read and let every later call through. Training then ends through LTS:L, and `train`
/// returns [`TrainingError::Scdc`](crate::TrainingError::Scdc) with that error; unlike a
/// dropped `train` future, this leaves both ends in TMDS (unless
/// `exit_to_tmds_on_error` is off or the sink sets `FLT_no_timeout`). Never fail a read
/// made after LTS:L has begun (after a `write_frl_config` with `NotSupported`), until a
/// `write_frl_config` with an FRL rate starts training again. `culvert-async` implements
/// it for its `Scdc<T>`.
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

/// [`ScdcClient`] with `Send` futures, for code that spawns link training on a
/// multi-threaded executor.
///
/// `ScdcClient`'s `async fn`s promise nothing about their futures, so code generic over
/// `C: ScdcClient` cannot spawn a future that calls them, however `C` is bounded. This
/// trait has the same methods and contract, each returning a future that is `Send`, and
/// errors that are `Send` too: a spawned task returns them. Implementations still write
/// `async fn`; the compiler checks that each future is `Send`. With a `SendScdcClient` and
/// an `hdmi_hal_async::phy::SendHdmiPhy`, the futures of `FrlTrainer`'s training methods
/// and `exit_to_tmds` are `Send` in generic code too (`train_with_events` given a `Send`
/// callback).
///
/// Bound generic code by `SendScdcClient` alone. Adding `+ ScdcClient`, which it implies
/// anyway, makes the compiler use that bound instead of the blanket implementation, and
/// the futures are no longer known to be `Send`; the same holds for `SendHdmiPhy` and
/// `+ HdmiPhy`.
///
/// Every `SendScdcClient` is an `ScdcClient`, through a blanket implementation, so it
/// works with everything written against `ScdcClient` as well; a type implements one of
/// the two (implementing both conflicts with the blanket implementation). A
/// `SendScdcClient` type has both traits' methods, so with both traits in scope a method
/// call on it is ambiguous: import only the trait the code is bounded by, or call
/// `SendScdcClient::read_flt_ready(…)`.
pub trait SendScdcClient: Send {
    /// Error type returned by SCDC operations.
    type Error: Send;

    /// See [`ScdcClient::read_flt_ready`].
    fn read_flt_ready(&mut self) -> impl Future<Output = Result<bool, Self::Error>> + Send;

    /// See [`ScdcClient::read_update_flags`].
    fn read_update_flags(
        &mut self,
    ) -> impl Future<Output = Result<UpdateFlags, Self::Error>> + Send;

    /// See [`ScdcClient::clear_update_flags`].
    fn clear_update_flags(
        &mut self,
        flags: ClearableUpdateFlags,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// See [`ScdcClient::read_ltp_requests`].
    fn read_ltp_requests(
        &mut self,
    ) -> impl Future<Output = Result<LtpRequests, Self::Error>> + Send;

    /// See [`ScdcClient::read_source_test_config`].
    fn read_source_test_config(
        &mut self,
    ) -> impl Future<Output = Result<SourceTestConfig, Self::Error>> + Send;

    /// See [`ScdcClient::write_config_0_defaults`].
    fn write_config_0_defaults(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// See [`ScdcClient::write_frl_config`].
    fn write_frl_config(
        &mut self,
        config: FrlConfig,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// See [`ScdcClient::read_ced`].
    fn read_ced(&mut self) -> impl Future<Output = Result<CedCounters, Self::Error>> + Send;
}

impl<T: SendScdcClient> ScdcClient for T {
    type Error = <T as SendScdcClient>::Error;

    fn read_flt_ready(&mut self) -> impl Future<Output = Result<bool, Self::Error>> {
        <T as SendScdcClient>::read_flt_ready(self)
    }

    fn read_update_flags(&mut self) -> impl Future<Output = Result<UpdateFlags, Self::Error>> {
        <T as SendScdcClient>::read_update_flags(self)
    }

    fn clear_update_flags(
        &mut self,
        flags: ClearableUpdateFlags,
    ) -> impl Future<Output = Result<(), Self::Error>> {
        <T as SendScdcClient>::clear_update_flags(self, flags)
    }

    fn read_ltp_requests(&mut self) -> impl Future<Output = Result<LtpRequests, Self::Error>> {
        <T as SendScdcClient>::read_ltp_requests(self)
    }

    fn read_source_test_config(
        &mut self,
    ) -> impl Future<Output = Result<SourceTestConfig, Self::Error>> {
        <T as SendScdcClient>::read_source_test_config(self)
    }

    fn write_config_0_defaults(&mut self) -> impl Future<Output = Result<(), Self::Error>> {
        <T as SendScdcClient>::write_config_0_defaults(self)
    }

    fn write_frl_config(
        &mut self,
        config: FrlConfig,
    ) -> impl Future<Output = Result<(), Self::Error>> {
        <T as SendScdcClient>::write_frl_config(self, config)
    }

    fn read_ced(&mut self) -> impl Future<Output = Result<CedCounters, Self::Error>> {
        <T as SendScdcClient>::read_ced(self)
    }
}
