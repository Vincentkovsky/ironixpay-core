//! Sweeper Service Module
//!
//! Handles automatic USDT collection (sweeping) from derived addresses.

pub mod executor;
pub mod service;

pub use executor::SweepExecutor;
pub use service::{SweeperConfig, SweeperService};
