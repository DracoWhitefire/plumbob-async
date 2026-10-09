//! Async FRL link training state machine for HDMI 2.1.
//!
//! `plumbob-async` is the async companion to [`plumbob`]. Its [`FrlTrainer`] runs
//! plumbob's state machine, [`plumbob::lts::run`], over an async [`ScdcClient`] and an
//! async `HdmiPhy`, so sync and async training are the same procedure. All shared
//! protocol and training types are re-exported from `plumbob`.
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

pub use client::ScdcClient;
pub use trainer::FrlTrainer;

pub use plumbob::{
    CedCount, CedCounters, FallbackReason, FfeLevels, FrlConfig, LtpReq, LtpRequests,
    SourceTestConfig, TrainingConfig, TrainingError, TrainingEvent, TrainingOutcome, UpdateFlags,
};

#[cfg(feature = "alloc")]
pub use plumbob::TrainingTrace;
