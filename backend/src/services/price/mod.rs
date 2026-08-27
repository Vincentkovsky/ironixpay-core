//! Price Oracle Module
//!
//! Provides cryptocurrency price feeds for cost calculations.

mod binance;

pub use binance::BinancePriceOracle;

use crate::entity::Network;
use anyhow::Result;
use async_trait::async_trait;

/// Trait for fetching cryptocurrency prices
#[async_trait]
pub trait PriceOracle: Send + Sync {
    /// Get the current TRX/USDT price
    async fn get_trx_usdt_price(&self) -> Result<rust_decimal::Decimal>;

    /// Get native token / USDT price for a given network.
    /// Maps each network to its correct Binance trading pair:
    /// - Tron → TRXUSDT
    /// - BSC → BNBUSDT
    /// - Polygon → MATICUSDT (POL)
    /// - Ethereum/Arbitrum/Base/Optimism → ETHUSDT
    async fn get_native_usdt_price(&self, network: Network) -> Result<rust_decimal::Decimal>;
}
