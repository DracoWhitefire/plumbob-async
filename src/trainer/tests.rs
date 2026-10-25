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
use crate::{
    FallbackReason, FfeLevels, LtpReq, SendHdmiPhy, SendScdcClient, TrainingOutcome,
    TrainingWarning, TxFfeRestrictions,
};

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
    LtpRequests::new(req, req, req, req)
}

/// A scripted sink: `FLT_ready` after `flt_ready_after` polls, then each set of requests
/// posted with `FLT_update` in turn once `Config_1` holds a rate, then `FRL_start`. A
/// `source_test_change` is applied, with `Source_Test_Update`, on the first poll after that.
/// It records the flags it is asked to clear and the writes it receives.
#[derive(Default)]
struct Sink {
    flt_ready_after: Option<u32>,
    flt_ready_polls: u32,
    rounds: VecDeque<LtpRequests>,
    frl_start: bool,
    configured: bool,
    flags: UpdateFlags,
    source_test: SourceTestConfig,
    source_test_change: Option<SourceTestConfig>,
    cleared: Vec<ClearableUpdateFlags>,
    config_0_writes: u32,
    frl_configs: Vec<FrlConfig>,
    waits: bool,
    fail: Option<&'static str>,
}

// The flags plumbob clears, one at a time: `Source_Test_Update`, `FRL_start`, `FLT_update`.
const SOURCE_TEST_UPDATE: ClearableUpdateFlags =
    ClearableUpdateFlags::new(false, false, true, false, false, false);
const FRL_START: ClearableUpdateFlags =
    ClearableUpdateFlags::new(false, false, false, true, false, false);
const FLT_UPDATE: ClearableUpdateFlags =
    ClearableUpdateFlags::new(false, false, false, false, true, false);

impl Sink {
    /// A sink that raises `Source_Test_Update` before training.
    fn with_source_test(self) -> Self {
        let mut flags = UpdateFlags::default();
        flags.source_test_update = true;
        Self { flags, ..self }
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
            if let Some(config) = self.source_test_change.take() {
                self.source_test = config;
                self.flags.source_test_update = true;
            }
            if self.rounds.is_empty() {
                self.flags.frl_start |= self.frl_start;
            } else {
                self.flags.flt_update = true;
            }
        }
        Ok(self.flags)
    }

    async fn clear_update_flags(
        &mut self,
        flags: ClearableUpdateFlags,
    ) -> Result<(), &'static str> {
        operation(self.waits, self.fail, "clear_update_flags").await?;
        self.cleared.push(flags);
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
        Ok(self.source_test)
    }

    async fn write_config_0_defaults(&mut self) -> Result<(), &'static str> {
        operation(self.waits, self.fail, "write_config_0_defaults").await?;
        self.config_0_writes += 1;
        Ok(())
    }

    async fn write_frl_config(&mut self, config: FrlConfig) -> Result<(), &'static str> {
        operation(self.waits, self.fail, "write_frl_config").await?;
        self.frl_configs.push(config);
        self.configured = config.frl_rate != HdmiForumFrl::NotSupported;
        Ok(())
    }
}

/// A PHY that records the rates and patterns it is given.
#[derive(Default)]
struct Phy {
    rates: Vec<HdmiForumFrl>,
    patterns: Vec<LanePatterns>,
    outputs: Vec<FrlOutput>,
    equalization: Vec<EqParams>,
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

    async fn set_frl_output(&mut self, output: FrlOutput) -> Result<(), &'static str> {
        operation(self.waits, self.fail, "set_frl_output").await?;
        self.outputs.push(output);
        Ok(())
    }

    async fn adjust_equalization(&mut self, params: EqParams) -> Result<(), &'static str> {
        operation(self.waits, self.fail, "adjust_equalization").await?;
        self.equalization.push(params);
        Ok(())
    }

    async fn set_scrambling(&mut self, _enabled: bool) -> Result<(), &'static str> {
        operation(self.waits, self.fail, "set_scrambling").await
    }
}

fn per_lane() -> LtpRequests {
    LtpRequests::new(LtpReq::Lfsr0, LtpReq::Lfsr1, LtpReq::Lfsr2, LtpReq::Lfsr3)
}

#[pollster::test]
async fn trains_through_async_io() {
    let sink = Sink::training(&[per_lane(), all(LtpReq::FfeChange), all(LtpReq::None)]);
    let mut trainer = FrlTrainer::new(sink, Phy::default());
    let outcome = trainer.train(&[RATE], &TrainingConfig::default()).await;
    assert!(matches!(
        outcome.map(|trained| trained.outcome),
        Ok(TrainingOutcome::Success {
            achieved_rate: RATE,
            ..
        })
    ));
    let (sink, phy) = trainer.into_parts();
    // The flags reach the sink exactly as plumbob clears them: `FLT_update` in LTS:2 and
    // after each round of requests, then `FRL_start`.
    assert_eq!(
        sink.cleared,
        [FLT_UPDATE, FLT_UPDATE, FLT_UPDATE, FLT_UPDATE, FRL_START]
    );
    assert!(!sink.flags.frl_start);
    assert_eq!(sink.config_0_writes, 1);
    assert_eq!(
        sink.frl_configs,
        [FrlConfig::new(RATE, FfeLevels::new(3).unwrap())]
    );
    assert_eq!(phy.rates, [RATE]);
    assert!(phy.patterns.contains(&LanePatterns {
        lane0: Some(LtpPattern::Lfsr0),
        lane1: Some(LtpPattern::Lfsr1),
        lane2: Some(LtpPattern::Lfsr2),
        lane3: Some(LtpPattern::Lfsr3),
    }));
    // Gap characters in LTS:2 and again in LTS:P.
    assert_eq!(phy.outputs, [FrlOutput::GapOnly, FrlOutput::GapOnly]);
    // LTS:2 resets every lane to level 0; the 0xE request raises every lane to 1.
    let levels = |eq: &EqParams| {
        (
            [
                eq.lane0.tx_ffe_level.value(),
                eq.lane1.tx_ffe_level.value(),
                eq.lane2.tx_ffe_level.value(),
            ],
            eq.lane3.map(|lane| lane.tx_ffe_level.value()),
        )
    };
    let sent: Vec<_> = phy.equalization.iter().map(levels).collect();
    assert_eq!(sent, [([0, 0, 0], Some(0)), ([1, 1, 1], Some(1))]);
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
    assert!(expected.0.is_ok());
    assert_eq!(actual, expected);
}

#[pollster::test]
async fn a_source_test_update_during_training_reaches_the_phy() {
    let mut no_ffe = SourceTestConfig::default();
    no_ffe.txffe_no_ffe = true;
    let sink = Sink {
        source_test_change: Some(no_ffe),
        ..Sink::training(&[per_lane(), all(LtpReq::None)])
    };
    let mut trainer = FrlTrainer::new(sink, Phy::default());
    let mut reads = Vec::new();
    let outcome = trainer
        .train_with_events(&[RATE], &TrainingConfig::default(), &mut |event| {
            if let TrainingEvent::SourceTestConfigRead { config, .. } = event {
                reads.push(config);
            }
        })
        .await;
    assert!(matches!(
        outcome.map(|trained| trained.outcome),
        Ok(TrainingOutcome::Success {
            achieved_rate: RATE,
            ..
        })
    ));
    assert_eq!(reads, [SourceTestConfig::default(), no_ffe]);
    let (sink, phy) = trainer.into_parts();
    assert!(!sink.flags.source_test_update);
    assert!(sink.cleared.contains(&SOURCE_TEST_UPDATE));
    // The equalization of LTS:2's rate setup, then the re-send with the new restrictions.
    let restrictions: Vec<TxFfeRestrictions> = phy
        .equalization
        .iter()
        .map(|eq| eq.tx_ffe_restrictions)
        .collect();
    assert_eq!(
        restrictions,
        [
            TxFfeRestrictions::default(),
            TxFfeRestrictions::new(false, false, true)
        ]
    );
}

#[pollster::test]
async fn a_fallback_returns_the_phy_to_tmds() {
    let mut trainer = FrlTrainer::new(Sink::default(), Phy::default());
    let mut config = TrainingConfig::default();
    config.flt_ready_polls = 3;
    let outcome = trainer.train_at_rate(RATE, &config).await;
    assert!(matches!(
        outcome.map(|trained| trained.outcome),
        Ok(TrainingOutcome::FallbackRequired {
            reason: FallbackReason::FltReadyTimeout,
            ..
        })
    ));

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
    // Not the default, so the trace shows the config it was given.
    let mut config = TrainingConfig::default();
    config.flt_ready_polls = 49;
    let mut trainer = FrlTrainer::new(Sink::training(&[all(LtpReq::None)]), Phy::default());
    let (outcome, trace) = trainer.train_at_rate_traced(RATE, &config).await;
    assert!(matches!(
        outcome.map(|trained| trained.outcome),
        Ok(TrainingOutcome::Success {
            achieved_rate: RATE,
            ..
        })
    ));
    assert_eq!(trace.rates, [RATE]);
    assert_eq!(trace.config, config);
    assert!(matches!(
        trace.events[..2],
        [
            TrainingEvent::SourceTestConfigRead { config, .. },
            TrainingEvent::FltReady { after_polls: 2, .. },
        ] if config == SourceTestConfig::default()
    ));
    assert!(matches!(
        trace.events.last(),
        Some(TrainingEvent::FrlStart { .. })
    ));
}

#[pollster::test]
async fn the_test_doubles_answer_the_operations_training_does_not_use() {
    // Covers the trait methods the state machine never calls.
    let mut sink = Sink::default();
    assert!(sink.read_source_test_config().await.is_ok());
    assert!(Phy::default().set_scrambling(true).await.is_ok());
}

#[pollster::test]
async fn exit_to_tmds_performs_lts_l_over_async_io() {
    let sink = Sink {
        configured: true,
        waits: true,
        ..Sink::default()
    };
    let phy = Phy {
        waits: true,
        ..Phy::default()
    };
    let mut trainer = FrlTrainer::new(sink, phy);
    assert_eq!(trainer.exit_to_tmds().await, Ok(()));
    let (sink, phy) = trainer.into_parts();
    assert!(!sink.configured);
    assert_eq!(phy.rates, [HdmiForumFrl::NotSupported]);
    assert_eq!(phy.patterns, [LanePatterns::default()]);
}

#[pollster::test]
async fn exit_to_tmds_reports_each_ends_error() {
    let sink = Sink {
        fail: Some("write_frl_config"),
        ..Sink::default()
    };
    let mut trainer = FrlTrainer::new(sink, Phy::default());
    let result = trainer.exit_to_tmds().await;
    assert!(
        matches!(
            result,
            Err(ExitError {
                scdc: Some("write_frl_config"),
                phy: None,
                ..
            })
        ),
        "{result:?}"
    );
}

#[pollster::test]
async fn exit_to_tmds_recovers_a_dropped_train_future() {
    // Training that waits on every operation and never passes, so it is still running
    // when it is dropped.
    let sink = Sink {
        waits: true,
        ..Sink::training(&[per_lane(); 64])
    };
    let phy = Phy {
        waits: true,
        ..Phy::default()
    };
    let mut trainer = FrlTrainer::new(sink, phy);
    let config = TrainingConfig::default();
    {
        let mut train = core::pin::pin!(trainer.train(&[RATE], &config));
        let mut cx = Context::from_waker(core::task::Waker::noop());
        for _ in 0..40 {
            assert!(train.as_mut().poll(&mut cx).is_pending());
        }
    }
    assert!(
        trainer.scdc.configured,
        "the dropped future left the sink in FRL"
    );

    assert_eq!(trainer.exit_to_tmds().await, Ok(()));
    let (sink, phy) = trainer.into_parts();
    assert!(!sink.configured);
    assert_eq!(phy.rates.last(), Some(&HdmiForumFrl::NotSupported));
}

#[pollster::test]
async fn undefined_requests_come_back_as_warnings() {
    let mut reserved = per_lane();
    reserved.lane2 = LtpReq::Reserved(0xC);
    let sink = Sink {
        waits: true,
        ..Sink::training(&[reserved, all(LtpReq::None)])
    };
    let trained = FrlTrainer::new(sink, Phy::default())
        .train(&[RATE], &TrainingConfig::default())
        .await
        .unwrap();
    assert!(matches!(
        trained.warnings(),
        [TrainingWarning::UndefinedLtpRequest {
            lane: 2,
            value: 0xC,
            count: 1,
            in_use: true,
            ..
        }]
    ));
}

#[pollster::test]
async fn train_with_events_reports_each_event_over_async_io() {
    let sink = Sink {
        waits: true,
        ..Sink::training(&[per_lane(), all(LtpReq::None)])
    };
    let mut events = Vec::new();
    let trained = FrlTrainer::new(sink, Phy::default())
        .train_with_events(&[RATE], &TrainingConfig::default(), &mut |event| {
            events.push(event)
        })
        .await
        .unwrap();
    assert!(matches!(
        trained.outcome,
        TrainingOutcome::Success {
            achieved_rate: RATE,
            ..
        }
    ));
    assert!(matches!(
        events[..2],
        [
            TrainingEvent::SourceTestConfigRead { config, .. },
            TrainingEvent::FltReady { after_polls: 2, .. },
        ] if config == SourceTestConfig::default()
    ));
    assert!(matches!(
        events.last(),
        Some(TrainingEvent::FrlStart { .. })
    ));
}

#[pollster::test]
async fn the_trainer_gives_access_to_its_client_and_phy() {
    let mut trainer = FrlTrainer::new(Sink::default(), Phy::default());
    trainer
        .phy_mut()
        .set_frl_output(FrlOutput::Active)
        .await
        .unwrap();
    trainer.scdc_mut().read_flt_ready().await.unwrap();
    assert_eq!(trainer.phy().outputs, [FrlOutput::Active]);
    assert_eq!(trainer.scdc().flt_ready_polls, 1);
}

/// `Sink` behind the `Send` twin: each method forwards to the plain `ScdcClient`.
struct SendSink(Sink);

impl SendScdcClient for SendSink {
    type Error = &'static str;

    async fn read_flt_ready(&mut self) -> Result<bool, &'static str> {
        ScdcClient::read_flt_ready(&mut self.0).await
    }

    async fn read_update_flags(&mut self) -> Result<UpdateFlags, &'static str> {
        ScdcClient::read_update_flags(&mut self.0).await
    }

    async fn clear_update_flags(
        &mut self,
        flags: ClearableUpdateFlags,
    ) -> Result<(), &'static str> {
        ScdcClient::clear_update_flags(&mut self.0, flags).await
    }

    async fn read_ltp_requests(&mut self) -> Result<LtpRequests, &'static str> {
        ScdcClient::read_ltp_requests(&mut self.0).await
    }

    async fn read_source_test_config(&mut self) -> Result<SourceTestConfig, &'static str> {
        ScdcClient::read_source_test_config(&mut self.0).await
    }

    async fn write_config_0_defaults(&mut self) -> Result<(), &'static str> {
        ScdcClient::write_config_0_defaults(&mut self.0).await
    }

    async fn write_frl_config(&mut self, config: FrlConfig) -> Result<(), &'static str> {
        ScdcClient::write_frl_config(&mut self.0, config).await
    }
}

/// `Phy` behind hdmi-hal-async's `Send` twin.
struct SendPhy(Phy);

impl SendHdmiPhy for SendPhy {
    type Error = &'static str;

    async fn set_frl_rate(&mut self, rate: HdmiForumFrl) -> Result<(), &'static str> {
        HdmiPhy::set_frl_rate(&mut self.0, rate).await
    }

    async fn send_ltp(&mut self, patterns: LanePatterns) -> Result<(), &'static str> {
        HdmiPhy::send_ltp(&mut self.0, patterns).await
    }

    async fn set_frl_output(&mut self, output: FrlOutput) -> Result<(), &'static str> {
        HdmiPhy::set_frl_output(&mut self.0, output).await
    }

    async fn adjust_equalization(&mut self, params: EqParams) -> Result<(), &'static str> {
        HdmiPhy::adjust_equalization(&mut self.0, params).await
    }

    async fn set_scrambling(&mut self, enabled: bool) -> Result<(), &'static str> {
        HdmiPhy::set_scrambling(&mut self.0, enabled).await
    }
}

type SendResults<C, P> = (
    Result<Trained, TrainingError<C, P>>,
    Result<(), ExitError<C, P>>,
    Result<Trained, TrainingError<C, P>>,
);

/// Trains, exits, trains again with events and, with `alloc`, traced.
async fn train_exit_train<C: ScdcClient, P: HdmiPhy>(
    trainer: &mut FrlTrainer<C, P>,
    config: &TrainingConfig,
    events: &mut usize,
) -> SendResults<C::Error, P::Error> {
    let trained = trainer.train(&[RATE], config).await;
    let exit = trainer.exit_to_tmds().await;
    let with_events = trainer
        .train_with_events(&[RATE], config, &mut |_| *events += 1)
        .await;
    #[cfg(feature = "alloc")]
    {
        let (_, trace) = trainer.train_traced(&[RATE], config).await;
        *events += trace.events.len();
    }
    (trained, exit, with_events)
}

/// [`train_exit_train`] generic over the `Send` twins, as a future that must be `Send`:
/// this compiles only if every training future is `Send` in generic code, so it could be
/// spawned on a multi-threaded executor.
fn spawnable<'a, C: SendScdcClient, P: SendHdmiPhy>(
    trainer: &'a mut FrlTrainer<C, P>,
    config: &'a TrainingConfig,
    events: &'a mut usize,
) -> impl Future<Output = SendResults<C::Error, P::Error>> + Send + 'a {
    train_exit_train(trainer, config, events)
}

#[pollster::test]
async fn the_send_twins_train_in_generic_send_code() {
    let sink = Sink {
        waits: true,
        ..Sink::training(&[per_lane(), all(LtpReq::None)])
    };
    let mut trainer = FrlTrainer::new(SendSink(sink), SendPhy(Phy::default()));
    let mut config = TrainingConfig::default();
    // The second attempt finds no more requests; keep its timeout short.
    config.ltp_polls = 10;
    let mut events = 0;
    let (trained, exit, with_events) = spawnable(&mut trainer, &config, &mut events).await;
    assert!(matches!(
        trained.map(|trained| trained.outcome),
        Ok(TrainingOutcome::Success {
            achieved_rate: RATE,
            ..
        })
    ));
    assert_eq!(exit, Ok(()));
    assert!(with_events.is_ok());
    assert!(events > 0);
    // The operation training does not use, through the blanket implementation.
    let (_, mut phy) = trainer.into_parts();
    assert_eq!(HdmiPhy::set_scrambling(&mut phy, true).await, Ok(()));
}

/// Runs `trainer` with events over the default config, and returns the result, the
/// events and both ends.
async fn run_with_events<C: ScdcClient, P: HdmiPhy>(
    mut trainer: FrlTrainer<C, P>,
) -> (
    Result<Trained, TrainingError<C::Error, P::Error>>,
    Vec<TrainingEvent>,
    C,
    P,
) {
    let mut events = Vec::new();
    let result = trainer
        .train_with_events(&[RATE], &TrainingConfig::default(), &mut |event| {
            events.push(event)
        })
        .await;
    let (scdc, phy) = trainer.into_parts();
    (result, events, scdc, phy)
}

/// The scripted sink both runs of the twin comparison use: it waits, changes its source
/// test configuration in LTS:3 and asks for an FFE change.
fn comparison_sink() -> Sink {
    let mut no_ffe = SourceTestConfig::default();
    no_ffe.txffe_no_ffe = true;
    Sink {
        waits: true,
        source_test_change: Some(no_ffe),
        ..Sink::training(&[per_lane(), all(LtpReq::FfeChange), all(LtpReq::None)])
    }
}

#[pollster::test]
async fn the_send_twin_forwards_every_operation_unchanged() {
    // The same sink through the plain client and through the twin: every value the
    // blanket implementation forwards shows up in the events or in what the ends recorded.
    let (plain, plain_events, plain_sink, plain_phy) =
        run_with_events(FrlTrainer::new(comparison_sink(), Phy::default())).await;
    let (twin, twin_events, SendSink(twin_sink), SendPhy(twin_phy)) = run_with_events(
        FrlTrainer::new(SendSink(comparison_sink()), SendPhy(Phy::default())),
    )
    .await;
    assert!(plain.is_ok());
    assert_eq!(twin, plain);
    assert_eq!(twin_events, plain_events);
    assert_eq!(twin_sink.cleared, plain_sink.cleared);
    assert_eq!(twin_sink.config_0_writes, plain_sink.config_0_writes);
    assert_eq!(twin_sink.frl_configs, plain_sink.frl_configs);
    assert_eq!(twin_phy.rates, plain_phy.rates);
    assert_eq!(twin_phy.patterns, plain_phy.patterns);
    assert_eq!(twin_phy.outputs, plain_phy.outputs);
    assert_eq!(twin_phy.equalization, plain_phy.equalization);
}
