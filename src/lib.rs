//! Async FRL link training state machine for HDMI 2.1.
//!
//! `plumbob-async` is the async companion to [`plumbob`], following the same design split
//! as `embedded-hal` / `embedded-hal-async`. It mirrors [`plumbob::ScdcClient`] and
//! [`plumbob::FrlTrainer`] with `async fn` methods and re-exports all shared protocol and
//! training types from `plumbob`.
//!
//! # Features
//!
//! - **`alloc`** — enables `TrainingTrace` and `FrlTrainer::train_at_rate_traced`.
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
    CedCount, CedCounters, FfeLevels, FrlConfig, LtpReq, TrainingConfig, TrainingError,
    TrainingOutcome, TrainingStatus,
};

#[cfg(feature = "alloc")]
pub use plumbob::{TrainingEvent, TrainingTrace};
