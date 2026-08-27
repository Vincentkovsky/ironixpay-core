//! AML (Anti-Money Laundering) Service Module
//!
//! Provides address risk screening with a 2-layer model:
//! - L1: Local blacklist (OFAC SDN, known hacker addresses) - DashSet cache
//! - L2: GoPlus API with 24h DB cache - Dynamic risk scoring

pub mod entity;
pub mod goplus;
pub mod service;

pub use service::{AmlConfig, AmlService, RiskResult};
