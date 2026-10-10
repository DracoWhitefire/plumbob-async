use display_types::cea861::hdmi_forum::HdmiForumFrl;
use hdmi_hal_async::phy::HdmiPhy;
use hdmi_hal_async::{EqParams, FrlOutput, LanePatterns};
use plumbob::lts::{self, TrainingIo};

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::client::ScdcClient;
use crate::{
    ExitError, FrlConfig, LtpRequests, SourceTestConfig, TrainingConfig, TrainingError,
    TrainingEvent, TrainingOutcome, UpdateFlags,
};

#[cfg(feature = "alloc")]
use crate::TrainingTrace;

type Error<C, P> = TrainingError<<C as ScdcClient>::Error, <P as HdmiPhy>::Error>;

/// The central async training type. Owns an async `ScdcClient` and an async `HdmiPhy`.
///
/// The async counterpart to [`plumbob::FrlTrainer`]: it runs plumbob's state machine,
/// [`plumbob::lts::run`], awaiting each SCDC and PHY operation, so it yields to the
/// executor wherever the I/O waits. Reusable across training attempts; use
/// [`into_parts`](Self::into_parts) to recover the SCDC client and PHY.
///
/// # Cancellation
///
/// [`train`](Self::train) is not cancel-safe. Dropping its future — a `select!` against
/// a hot-unplug, a `timeout` — stops training wherever it is: the sink may be configured
/// for an FRL rate and the PHY driving training patterns, and no LTS:L runs, since a
/// future cannot await anything once dropped. Call [`exit_to_tmds`](Self::exit_to_tmds)
/// afterwards to return both ends to TMDS. Bounding training is better left to the poll
/// limits in [`TrainingConfig`], which end every attempt in a defined state.
pub struct FrlTrainer<C, P> {
    scdc: C,
    phy: P,
}

impl<C: ScdcClient, P: HdmiPhy> FrlTrainer<C, P> {
    /// Constructs a new `FrlTrainer` owning the given async SCDC client and PHY.
    pub fn new(scdc: C, phy: P) -> Self {
        Self { scdc, phy }
    }

    /// Consumes the trainer and returns the SCDC client and PHY.
    pub fn into_parts(self) -> (C, P) {
        (self.scdc, self.phy)
    }

    /// LTS:L on demand: stops the training patterns, returns the PHY to TMDS, turns FRL
    /// off in `Config_1` and clears `FLT_update` if it is set.
    ///
    /// The async counterpart to [`plumbob::FrlTrainer::exit_to_tmds`]: for taking an FRL
    /// link down when the display is disabled or unplugged, before a mode change, after
    /// an error with [`TrainingConfig::exit_to_tmds_on_error`] off, and after a dropped
    /// [`train`](Self::train) future (see [Cancellation](Self#cancellation)). Safe to call
    /// in any state, and again. Every step is attempted even when one fails.
    pub async fn exit_to_tmds(&mut self) -> Result<(), ExitError<C::Error, P::Error>> {
        let mut io = AsyncIo {
            scdc: &mut self.scdc,
            phy: &mut self.phy,
        };
        lts::exit_to_tmds(&mut io, &mut |_| {}).await
    }

    /// Trains at a single rate: `train(&[rate], config)`.
    pub async fn train_at_rate(
        &mut self,
        rate: HdmiForumFrl,
        config: &TrainingConfig,
    ) -> Result<TrainingOutcome, Error<C, P>> {
        self.train(&[rate], config).await
    }

    /// Trains over `rates` in order, stepping down when the sink requests it.
    ///
    /// The async counterpart to [`plumbob::FrlTrainer::train`], with the same outcomes and
    /// errors. Not cancel-safe: see [Cancellation](Self#cancellation).
    pub async fn train(
        &mut self,
        rates: &[HdmiForumFrl],
        config: &TrainingConfig,
    ) -> Result<TrainingOutcome, Error<C, P>> {
        self.run(rates, config, &mut |_| {}).await
    }

    /// Like [`train_at_rate`](Self::train_at_rate), and also returns a [`TrainingTrace`]
    /// of the attempt, whatever its result.
    #[cfg(feature = "alloc")]
    pub async fn train_at_rate_traced(
        &mut self,
        rate: HdmiForumFrl,
        config: &TrainingConfig,
    ) -> (Result<TrainingOutcome, Error<C, P>>, TrainingTrace) {
        self.train_traced(&[rate], config).await
    }

    /// Like [`train`](Self::train), and also returns a [`TrainingTrace`] of the attempt.
    ///
    /// The trace is returned whatever the result, so an attempt that ended in a
    /// [`TrainingError`] can be explained as well.
    #[cfg(feature = "alloc")]
    pub async fn train_traced(
        &mut self,
        rates: &[HdmiForumFrl],
        config: &TrainingConfig,
    ) -> (Result<TrainingOutcome, Error<C, P>>, TrainingTrace) {
        let mut events = Vec::new();
        let result = self
            .run(rates, config, &mut |event| events.push(event))
            .await;
        (result, TrainingTrace::new(rates.to_vec(), *config, events))
    }

    /// Runs [`lts::run`] over this trainer's SCDC client and PHY.
    async fn run<F: FnMut(TrainingEvent)>(
        &mut self,
        rates: &[HdmiForumFrl],
        config: &TrainingConfig,
        record: &mut F,
    ) -> Result<TrainingOutcome, Error<C, P>> {
        let mut io = AsyncIo {
            scdc: &mut self.scdc,
            phy: &mut self.phy,
        };
        lts::run(&mut io, rates, config, record).await
    }
}

/// [`TrainingIo`] over an async `ScdcClient` and `HdmiPhy`.
struct AsyncIo<'a, C, P> {
    scdc: &'a mut C,
    phy: &'a mut P,
}

impl<C: ScdcClient, P: HdmiPhy> TrainingIo for AsyncIo<'_, C, P> {
    type ScdcError = C::Error;
    type PhyError = P::Error;

    async fn read_flt_ready(&mut self) -> Result<bool, C::Error> {
        self.scdc.read_flt_ready().await
    }

    async fn read_update_flags(&mut self) -> Result<UpdateFlags, C::Error> {
        self.scdc.read_update_flags().await
    }

    async fn clear_update_flags(&mut self, flags: UpdateFlags) -> Result<(), C::Error> {
        self.scdc.clear_update_flags(flags).await
    }

    async fn read_ltp_requests(&mut self) -> Result<LtpRequests, C::Error> {
        self.scdc.read_ltp_requests().await
    }

    async fn read_source_test_config(&mut self) -> Result<SourceTestConfig, C::Error> {
        self.scdc.read_source_test_config().await
    }

    async fn write_config_0_defaults(&mut self) -> Result<(), C::Error> {
        self.scdc.write_config_0_defaults().await
    }

    async fn write_frl_config(&mut self, config: FrlConfig) -> Result<(), C::Error> {
        self.scdc.write_frl_config(config).await
    }

    async fn set_frl_rate(&mut self, rate: HdmiForumFrl) -> Result<(), P::Error> {
        self.phy.set_frl_rate(rate).await
    }

    async fn send_ltp(&mut self, patterns: LanePatterns) -> Result<(), P::Error> {
        self.phy.send_ltp(patterns).await
    }

    async fn set_frl_output(&mut self, output: FrlOutput) -> Result<(), P::Error> {
        self.phy.set_frl_output(output).await
    }

    async fn adjust_equalization(&mut self, params: EqParams) -> Result<(), P::Error> {
        self.phy.adjust_equalization(params).await
    }
}

#[cfg(test)]
mod tests;
