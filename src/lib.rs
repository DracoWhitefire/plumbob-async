//! Async FRL link training state machine for HDMI 2.1.
//!
//! `plumbob-async` is the async companion to [`plumbob`]. Its [`FrlTrainer`] runs
//! plumbob's state machine, [`plumbob::lts::run`], over an async [`ScdcClient`] and an
//! async `HdmiPhy`, so sync and async training are the same procedure. All shared
//! protocol and training types are re-exported from `plumbob`.
//!
//! The futures of the plain traits are not required to be `Send`. To spawn training on a
//! multi-threaded executor from code generic over the client and PHY, implement their
//! `Send` twins, [`SendScdcClient`] and [`SendHdmiPhy`]; every twin is also an
//! implementation of the plain trait.
//!
//! # Features
//!
//! - **`alloc`** — enables `TrainingTrace`, `FrlTrainer::train_traced` and
//!   `FrlTrainer::train_at_rate_traced`.
//! - **`std`** — implies `alloc`; no additional API surface.

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(async_fn_in_trait)]

#[cfg(feature = "alloc")]
extern crate alloc;

mod client;
mod trainer;

pub use client::{ScdcClient, SendScdcClient};
pub use trainer::FrlTrainer;

pub use plumbob::{
    ClearableUpdateFlags, ExitError, FallbackReason, FfeLevels, FrlConfig, LtpReq, LtpRequests,
    MAX_WARNINGS, SourceTestConfig, TmdsExit, Trained, TrainingConfig, TrainingError,
    TrainingEvent, TrainingOutcome, TrainingWarning, UpdateFlags,
};

#[cfg(feature = "alloc")]
pub use plumbob::TrainingTrace;

// The FRL rate and the async PHY interface `FrlTrainer` drives (and its `Send` twin),
// re-exported from `hdmi-hal-async` so that callers and PHY implementations use the same
// versions plumbob-async does without depending on that crate themselves. A plain comment:
// a doc comment here would head the inlined documentation of every item re-exported.
pub use hdmi_hal_async::phy::{HdmiPhy, SendHdmiPhy};
pub use hdmi_hal_async::{
    EqParams, FrlOutput, HdmiForumFrl, LaneEqParams, LanePatterns, LtpPattern, TxFfeLevel,
    TxFfeRestrictions,
};
