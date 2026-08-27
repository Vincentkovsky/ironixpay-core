//! Chain Abstraction Layer
//!
//! Provides chain-agnostic traits and types for multi-chain support.
//! Each blockchain implementation (TRON, EVM, Solana) implements `ChainClient`.

pub mod traits;
pub mod types;

pub use traits::{ChainClient, ChainSigner};
pub use types::*;
