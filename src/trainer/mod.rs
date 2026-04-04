use display_types::cea861::hdmi_forum::HdmiForumFrl;
use hdmi_hal_async::phy::HdmiPhy;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::client::ScdcClient;
use crate::{FrlConfig, LtpReq, TrainingConfig, TrainingError, TrainingOutcome};

#[cfg(feature = "alloc")]
use crate::{TrainingEvent, TrainingTrace};

/// The central async training type. Owns an async `ScdcClient` and an async `HdmiPhy`.
///
/// `FrlTrainer` is reusable across multiple `train_at_rate` calls. A caller
/// performing rate fallback calls `train_at_rate` repeatedly on the same trainer,
/// stepping down through FRL tiers, without reconstructing it between attempts.
/// Use `into_parts` to recover the SCDC client and PHY when training is finished.
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

    /// Runs the full four-phase FRL training sequence at the given rate.
    ///
    /// Each SCDC and PHY call is awaited, yielding to the executor between
    /// iterations. Returns [`TrainingOutcome::Success`] when all lanes satisfy
    /// their LTP requests, or [`TrainingOutcome::FallbackRequired`] if any phase
    /// times out. A [`TrainingError`] is returned only on hard I/O failures.
    pub async fn train_at_rate(
        &mut self,
        rate: HdmiForumFrl,
        config: &TrainingConfig,
    ) -> Result<TrainingOutcome, TrainingError<C::Error, P::Error>> {
        // Phase 1 — Configuration
        self.scdc
            .write_frl_config(FrlConfig {
                rate,
                ffe_levels: config.ffe_levels,
                dsc_frl_max: config.dsc_frl_max,
            })
            .await
            .map_err(TrainingError::Scdc)?;
        self.phy
            .set_frl_rate(rate)
            .await
            .map_err(TrainingError::Phy)?;

        // Phase 2 — Readiness: poll until the sink asserts flt_ready.
        let mut i = 0u32;
        loop {
            let status = self
                .scdc
                .read_training_status()
                .await
                .map_err(TrainingError::Scdc)?;
            if status.flt_ready {
                break;
            }
            i += 1;
            if i >= config.flt_ready_timeout {
                return Ok(TrainingOutcome::FallbackRequired);
            }
        }

        // Phase 3 — Initiation: poll until the sink asserts frl_start.
        let mut i = 0u32;
        loop {
            let status = self
                .scdc
                .read_training_status()
                .await
                .map_err(TrainingError::Scdc)?;
            if status.frl_start {
                break;
            }
            i += 1;
            if i >= config.frl_start_timeout {
                return Ok(TrainingOutcome::FallbackRequired);
            }
        }

        // Phase 4 — LTP loop.
        let mut i = 0u32;
        loop {
            let status = self
                .scdc
                .read_training_status()
                .await
                .map_err(TrainingError::Scdc)?;
            if status.ltp_req == LtpReq::None {
                return Ok(TrainingOutcome::Success {
                    achieved_rate: rate,
                });
            }
            let _ced = self.scdc.read_ced().await.map_err(TrainingError::Scdc)?;
            self.phy
                .send_ltp(status.ltp_req.into())
                .await
                .map_err(TrainingError::Phy)?;
            i += 1;
            if i >= config.ltp_timeout {
                return Ok(TrainingOutcome::FallbackRequired);
            }
        }
    }

    /// Like [`FrlTrainer::train_at_rate`], but also returns a [`TrainingTrace`]
    /// recording the full event sequence. Requires the `alloc` feature.
    #[cfg(feature = "alloc")]
    pub async fn train_at_rate_traced(
        &mut self,
        rate: HdmiForumFrl,
        config: &TrainingConfig,
    ) -> Result<(TrainingOutcome, TrainingTrace), TrainingError<C::Error, P::Error>> {
        let mut events: Vec<TrainingEvent> = Vec::new();

        // Phase 1 — Configuration
        self.scdc
            .write_frl_config(FrlConfig {
                rate,
                ffe_levels: config.ffe_levels,
                dsc_frl_max: config.dsc_frl_max,
            })
            .await
            .map_err(TrainingError::Scdc)?;
        self.phy
            .set_frl_rate(rate)
            .await
            .map_err(TrainingError::Phy)?;
        events.push(TrainingEvent::RateConfigured {
            rate,
            ffe_levels: config.ffe_levels,
        });

        // Phase 2 — Readiness: poll until the sink asserts flt_ready.
        let mut i = 0u32;
        let timed_out = loop {
            let status = self
                .scdc
                .read_training_status()
                .await
                .map_err(TrainingError::Scdc)?;
            if status.flt_ready {
                events.push(TrainingEvent::FltReadyReceived {
                    after_iterations: i,
                });
                break false;
            }
            i += 1;
            if i >= config.flt_ready_timeout {
                events.push(TrainingEvent::FltReadyTimeout {
                    iterations_elapsed: i,
                });
                break true;
            }
        };
        if timed_out {
            return Ok((
                TrainingOutcome::FallbackRequired,
                TrainingTrace::new(rate, *config, events),
            ));
        }

        // Phase 3 — Initiation: poll until the sink asserts frl_start.
        let mut i = 0u32;
        let timed_out = loop {
            let status = self
                .scdc
                .read_training_status()
                .await
                .map_err(TrainingError::Scdc)?;
            if status.frl_start {
                events.push(TrainingEvent::FrlStartReceived {
                    after_iterations: i,
                });
                break false;
            }
            i += 1;
            if i >= config.frl_start_timeout {
                events.push(TrainingEvent::FrlStartTimeout {
                    iterations_elapsed: i,
                });
                break true;
            }
        };
        if timed_out {
            return Ok((
                TrainingOutcome::FallbackRequired,
                TrainingTrace::new(rate, *config, events),
            ));
        }

        // Phase 4 — LTP loop.
        let mut i = 0u32;
        let mut last_ltp: Option<LtpReq> = None;
        let outcome = loop {
            let status = self
                .scdc
                .read_training_status()
                .await
                .map_err(TrainingError::Scdc)?;
            if status.ltp_req == LtpReq::None {
                events.push(TrainingEvent::AllLanesSatisfied {
                    after_iterations: i,
                });
                break TrainingOutcome::Success {
                    achieved_rate: rate,
                };
            }
            if Some(status.ltp_req) != last_ltp {
                events.push(TrainingEvent::LtpPatternRequested {
                    pattern: status.ltp_req,
                });
                last_ltp = Some(status.ltp_req);
            }
            let _ced = self.scdc.read_ced().await.map_err(TrainingError::Scdc)?;
            self.phy
                .send_ltp(status.ltp_req.into())
                .await
                .map_err(TrainingError::Phy)?;
            i += 1;
            if i >= config.ltp_timeout {
                events.push(TrainingEvent::LtpLoopTimeout {
                    iterations_elapsed: i,
                });
                break TrainingOutcome::FallbackRequired;
            }
        };
        Ok((outcome, TrainingTrace::new(rate, *config, events)))
    }
}

#[cfg(test)]
mod sim;

#[cfg(test)]
mod tests;
