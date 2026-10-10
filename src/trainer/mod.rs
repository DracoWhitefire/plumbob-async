use display_types::cea861::hdmi_forum::HdmiForumFrl;
use hdmi_hal_async::phy::HdmiPhy;
use hdmi_hal_async::{EqParams, FrlOutput, LanePatterns};
use plumbob::lts::{self, TrainingIo};

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::client::ScdcClient;
use crate::{
    FrlConfig, LtpRequests, SourceTestConfig, TrainingConfig, TrainingError, TrainingEvent,
    TrainingOutcome, UpdateFlags,
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
    /// errors.
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
