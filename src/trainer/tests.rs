//! Tests of the async driver. The state machine itself is tested in plumbob; these check
//! that `FrlTrainer` runs it over async I/O, including I/O that actually waits.

extern crate std;

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use std::collections::VecDeque;
use std::vec::Vec;

use display_types::cea861::hdmi_forum::HdmiForumFrl;
use hdmi_hal_async::LtpPattern;

use super::*;
use crate::{CedCounters, FallbackReason, LtpReq};

const RATE: HdmiForumFrl = HdmiForumFrl::Rate6Gbps4Lanes;

/// A future that is `Pending` once (and wakes itself) before completing.
struct WaitOnce(bool);

impl Future for WaitOnce {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            Poll::Ready(())
        } else {
            self.0 = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

/// Waits once when `waits` is set, then fails if `name` is the operation set to fail.
async fn operation(
    waits: bool,
    fail: Option<&'static str>,
    name: &'static str,
) -> Result<(), &'static str> {
    if waits {
        WaitOnce(false).await;
    }
    if fail == Some(name) {
        Err(name)
    } else {
        Ok(())
    }
}

fn all(req: LtpReq) -> LtpRequests {
    LtpRequests {
        lane0: req,
        lane1: req,
        lane2: req,
        lane3: req,
    }
}

/// A scripted sink: `FLT_ready` after `flt_ready_after` polls, then each set of requests
/// posted with `FLT_update` in turn once `Config_1` holds a rate, then `FRL_start`.
#[derive(Default)]
struct Sink {
    flt_ready_after: Option<u32>,
    flt_ready_polls: u32,
    rounds: VecDeque<LtpRequests>,
    frl_start: bool,
    configured: bool,
    flags: UpdateFlags,
    waits: bool,
    fail: Option<&'static str>,
}

impl Sink {
    /// A sink that raises `Source_Test_Update` before training.
    fn with_source_test(self) -> Self {
        Self {
            flags: UpdateFlags {
                source_test_update: true,
                ..UpdateFlags::default()
            },
            ..self
        }
    }

    fn training(rounds: &[LtpRequests]) -> Self {
        Self {
            flt_ready_after: Some(1),
            rounds: rounds.iter().copied().collect(),
            frl_start: true,
            ..Self::default()
        }
    }
}

impl ScdcClient for Sink {
    type Error = &'static str;

    async fn read_flt_ready(&mut self) -> Result<bool, &'static str> {
        operation(self.waits, self.fail, "read_flt_ready").await?;
        let ready = self
            .flt_ready_after
            .is_some_and(|n| self.flt_ready_polls >= n);
        self.flt_ready_polls += 1;
        Ok(ready)
    }

    async fn read_update_flags(&mut self) -> Result<UpdateFlags, &'static str> {
        operation(self.waits, self.fail, "read_update_flags").await?;
        if self.configured {
            if self.rounds.is_empty() {
                self.flags.frl_start |= self.frl_start;
            } else {
                self.flags.flt_update = true;
            }
        }
        Ok(self.flags)
    }

    async fn clear_update_flags(&mut self, flags: UpdateFlags) -> Result<(), &'static str> {
        operation(self.waits, self.fail, "clear_update_flags").await?;
        if flags.flt_update && self.flags.flt_update {
            self.flags.flt_update = false;
            self.rounds.pop_front();
        }
        self.flags.source_test_update &= !flags.source_test_update;
        if flags.frl_start && self.flags.frl_start {
            self.flags.frl_start = false;
            self.frl_start = false;
        }
        Ok(())
    }

    async fn read_ltp_requests(&mut self) -> Result<LtpRequests, &'static str> {
        operation(self.waits, self.fail, "read_ltp_requests").await?;
        Ok(self.rounds.front().copied().unwrap_or(all(LtpReq::None)))
    }

    async fn read_source_test_config(&mut self) -> Result<SourceTestConfig, &'static str> {
        operation(self.waits, self.fail, "read_source_test_config").await?;
        Ok(SourceTestConfig::default())
    }

    async fn write_config_0_defaults(&mut self) -> Result<(), &'static str> {
        operation(self.waits, self.fail, "write_config_0_defaults").await
    }

    async fn write_frl_config(&mut self, config: FrlConfig) -> Result<(), &'static str> {
        operation(self.waits, self.fail, "write_frl_config").await?;
        self.configured = config.rate != HdmiForumFrl::NotSupported;
        Ok(())
    }

    async fn read_ced(&mut self) -> Result<CedCounters, &'static str> {
        operation(self.waits, self.fail, "read_ced").await?;
        Ok(CedCounters {
            lane0: None,
            lane1: None,
            lane2: None,
            lane3: None,
        })
    }
}

/// A PHY that records the rates and patterns it is given.
#[derive(Default)]
struct Phy {
    rates: Vec<HdmiForumFrl>,
    patterns: Vec<LanePatterns>,
    waits: bool,
    fail: Option<&'static str>,
}

impl HdmiPhy for Phy {
    type Error = &'static str;

    async fn set_frl_rate(&mut self, rate: HdmiForumFrl) -> Result<(), &'static str> {
        operation(self.waits, self.fail, "set_frl_rate").await?;
        self.rates.push(rate);
        Ok(())
    }

    async fn send_ltp(&mut self, patterns: LanePatterns) -> Result<(), &'static str> {
        operation(self.waits, self.fail, "send_ltp").await?;
        self.patterns.push(patterns);
        Ok(())
    }

    async fn set_frl_output(&mut self, _output: FrlOutput) -> Result<(), &'static str> {
        operation(self.waits, self.fail, "set_frl_output").await
    }

    async fn adjust_equalization(&mut self, _params: EqParams) -> Result<(), &'static str> {
        operation(self.waits, self.fail, "adjust_equalization").await
    }

    async fn set_scrambling(&mut self, _enabled: bool) -> Result<(), &'static str> {
        operation(self.waits, self.fail, "set_scrambling").await
    }
}

fn per_lane() -> LtpRequests {
    LtpRequests {
        lane0: LtpReq::Lfsr0,
        lane1: LtpReq::Lfsr1,
        lane2: LtpReq::Lfsr2,
        lane3: LtpReq::Lfsr3,
    }
}

#[pollster::test]
async fn trains_through_async_io() {
    let sink = Sink::training(&[per_lane(), all(LtpReq::None)]);
    let mut trainer = FrlTrainer::new(sink, Phy::default());
    let outcome = trainer.train(&[RATE], &TrainingConfig::default()).await;
    assert_eq!(
        outcome,
        Ok(TrainingOutcome::Success {
            achieved_rate: RATE
        })
    );
    let (_, phy) = trainer.into_parts();
    assert_eq!(phy.rates, [RATE]);
    assert!(phy.patterns.contains(&LanePatterns {
        lane0: Some(LtpPattern::Lfsr0),
        lane1: Some(LtpPattern::Lfsr1),
        lane2: Some(LtpPattern::Lfsr2),
        lane3: Some(LtpPattern::Lfsr3),
    }));
}

#[cfg(feature = "alloc")]
#[pollster::test]
async fn io_that_waits_gives_the_same_result() {
    let rounds = [per_lane(), all(LtpReq::None)];
    let mut sync_like = FrlTrainer::new(Sink::training(&rounds), Phy::default());
    let mut waiting = FrlTrainer::new(
        Sink {
            waits: true,
            ..Sink::training(&rounds)
        },
        Phy {
            waits: true,
            ..Phy::default()
        },
    );
    let config = TrainingConfig::default();
    let expected = sync_like.train_traced(&[RATE], &config).await;
    let actual = waiting.train_traced(&[RATE], &config).await;
    assert!(expected.is_ok());
    assert_eq!(actual, expected);
}

#[cfg(feature = "alloc")]
#[pollster::test]
async fn a_source_test_configuration_is_read() {
    let sink = Sink::training(&[all(LtpReq::None)]).with_source_test();
    let mut trainer = FrlTrainer::new(sink, Phy::default());
    let (outcome, trace) = trainer
        .train_traced(&[RATE], &TrainingConfig::default())
        .await
        .unwrap();
    assert_eq!(
        outcome,
        TrainingOutcome::Success {
            achieved_rate: RATE
        }
    );
    assert_eq!(
        trace.events.first(),
        Some(&TrainingEvent::SourceTestConfigRead {
            flt_no_timeout: false
        })
    );
}

#[pollster::test]
async fn a_fallback_returns_the_phy_to_tmds() {
    let mut trainer = FrlTrainer::new(Sink::default(), Phy::default());
    let mut config = TrainingConfig::default();
    config.flt_ready_polls = 3;
    let outcome = trainer.train_at_rate(RATE, &config).await;
    assert_eq!(
        outcome,
        Ok(TrainingOutcome::FallbackRequired {
            reason: FallbackReason::FltReadyTimeout
        })
    );
    let (_, phy) = trainer.into_parts();
    assert_eq!(phy.rates.last(), Some(&HdmiForumFrl::NotSupported));
}

#[pollster::test]
async fn scdc_and_phy_errors_are_returned() {
    let rounds = [per_lane(), all(LtpReq::None)];
    for op in [
        "read_flt_ready",
        "read_update_flags",
        "clear_update_flags",
        "read_ltp_requests",
        "read_source_test_config",
        "write_config_0_defaults",
        "write_frl_config",
    ] {
        let sink = Sink {
            fail: Some(op),
            ..Sink::training(&rounds).with_source_test()
        };
        let outcome = FrlTrainer::new(sink, Phy::default())
            .train(&[RATE], &TrainingConfig::default())
            .await;
        assert!(
            matches!(outcome, Err(TrainingError::Scdc { error, .. }) if error == op),
            "{op}"
        );
    }
    for op in [
        "set_frl_rate",
        "send_ltp",
        "set_frl_output",
        "adjust_equalization",
    ] {
        let phy = Phy {
            fail: Some(op),
            ..Phy::default()
        };
        let outcome = FrlTrainer::new(Sink::training(&rounds), phy)
            .train(&[RATE], &TrainingConfig::default())
            .await;
        assert!(
            matches!(outcome, Err(TrainingError::Phy { error, .. }) if error == op),
            "{op}"
        );
    }
}

#[cfg(feature = "alloc")]
#[pollster::test]
async fn traced_training_records_the_attempt() {
    let config = TrainingConfig::default();
    let mut trainer = FrlTrainer::new(Sink::training(&[all(LtpReq::None)]), Phy::default());
    let (outcome, trace) = trainer.train_at_rate_traced(RATE, &config).await.unwrap();
    assert_eq!(
        outcome,
        TrainingOutcome::Success {
            achieved_rate: RATE
        }
    );
    assert_eq!(trace.rates, [RATE]);
    assert_eq!(trace.config, config);
    assert_eq!(
        trace.events.first(),
        Some(&TrainingEvent::FltReady { after_polls: 2 })
    );
    assert!(matches!(
        trace.events.last(),
        Some(TrainingEvent::FrlStart { .. })
    ));
}

#[pollster::test]
async fn the_test_doubles_answer_the_operations_training_does_not_use() {
    // Covers the trait methods the state machine never calls.
    let mut sink = Sink::default();
    assert!(sink.read_ced().await.is_ok());
    assert!(sink.read_source_test_config().await.is_ok());
    let mut sink = Sink {
        fail: Some("read_ced"),
        ..Sink::default()
    };
    assert_eq!(sink.read_ced().await, Err("read_ced"));
    assert!(Phy::default().set_scrambling(true).await.is_ok());
}
