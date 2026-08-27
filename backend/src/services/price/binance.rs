//! Binance Price Oracle Implementation
//!
//! Fetches real-time cryptocurrency prices from Binance API.
//! Supports multiple trading pairs with per-symbol caching.

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tracing::debug;

use super::PriceOracle;

const BINANCE_API_BASE: &str = "https://api.binance.com/api/v3/ticker/price";

/// Binance API response for ticker price
#[derive(Debug, Deserialize)]
struct BinanceTickerPrice {
    symbol: String,
    price: String,
}

/// Cached price entry
struct CachedPrice {
    price: Decimal,
    fetched_at: Instant,
}

/// Binance price oracle with per-symbol caching
pub struct BinancePriceOracle {
    client: Client,
    /// Cache TTL - how long to use cached price before refreshing
    cache_ttl: Duration,
    /// Per-symbol price cache
    price_cache: Arc<RwLock<HashMap<String, CachedPrice>>>,
    /// Per-symbol fetch locks to prevent thundering herd
    fetch_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
    /// API base URL (configurable for testing)
    api_base: String,
}

impl BinancePriceOracle {
    /// Create a new Binance price oracle
    ///
    /// # Arguments
    /// * `cache_ttl_seconds` - How long to cache prices (default: 60 seconds)
    pub fn try_new(cache_ttl_seconds: Option<u64>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .context("Failed to create HTTP client for Binance Oracle")?;

        Ok(Self {
            client,
            cache_ttl: Duration::from_secs(cache_ttl_seconds.unwrap_or(60)),
            price_cache: Arc::new(RwLock::new(HashMap::new())),
            fetch_locks: Arc::new(Mutex::new(HashMap::new())),
            api_base: BINANCE_API_BASE.to_string(),
        })
    }

    /// Set custom API base URL (for testing)
    #[cfg(test)]
    pub fn with_api_url(mut self, url: String) -> Self {
        self.api_base = url;
        self
    }

    /// Get or create a per-symbol fetch lock (prevents thundering herd)
    async fn get_fetch_lock(&self, symbol: &str) -> Arc<Mutex<()>> {
        let mut locks = self.fetch_locks.lock().await;
        locks
            .entry(symbol.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// Fetch price from Binance API for a given symbol
    async fn fetch_price(&self, symbol: &str) -> Result<Decimal> {
        let url = format!("{}?symbol={}", self.api_base, symbol);
        let response: BinanceTickerPrice = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to send request to Binance")?
            .error_for_status()
            .context("Binance API returned error")?
            .json()
            .await
            .context("Failed to parse Binance response")?;

        let price: Decimal = response
            .price
            .parse()
            .context("Failed to parse price string as Decimal")?;

        if price <= Decimal::ZERO {
            return Err(anyhow!("Invalid price from Binance: {}", price));
        }

        debug!(symbol = %response.symbol, %price, "Fetched price from Binance");

        Ok(price)
    }

    /// Get price for a symbol with caching and request coalescing
    async fn get_cached_price(&self, symbol: &str) -> Result<Decimal> {
        // 1. Check cache first (Fast Path)
        {
            let cache = self.price_cache.read().await;
            if let Some(cached) = cache.get(symbol) {
                if cached.fetched_at.elapsed() < self.cache_ttl {
                    debug!(
                        %symbol,
                        %cached.price,
                        age_secs = cached.fetched_at.elapsed().as_secs(),
                        "Using cached price"
                    );
                    return Ok(cached.price);
                }
            }
        }

        // 2. Acquire per-symbol fetch lock (Request Coalescing)
        let lock = self.get_fetch_lock(symbol).await;
        let _guard = lock.lock().await;

        // 3. Double Check Cache (after acquiring lock)
        {
            let cache = self.price_cache.read().await;
            if let Some(cached) = cache.get(symbol) {
                if cached.fetched_at.elapsed() < self.cache_ttl {
                    debug!(
                        %symbol,
                        %cached.price,
                        age_secs = cached.fetched_at.elapsed().as_secs(),
                        "Using cached price (coalesced)"
                    );
                    return Ok(cached.price);
                }
            }
        }

        // 4. Fetch from API (Slow Path)
        let price = self.fetch_price(symbol).await?;

        // 5. Update cache
        {
            let mut cache = self.price_cache.write().await;
            cache.insert(
                symbol.to_string(),
                CachedPrice {
                    price,
                    fetched_at: Instant::now(),
                },
            );
        }

        Ok(price)
    }
}

use crate::entity::Network;

#[async_trait]
impl PriceOracle for BinancePriceOracle {
    async fn get_trx_usdt_price(&self) -> Result<Decimal> {
        self.get_cached_price("TRXUSDT").await
    }

    async fn get_native_usdt_price(&self, network: Network) -> Result<Decimal> {
        let symbol = match network {
            Network::Tron => "TRXUSDT",
            Network::Bsc => "BNBUSDT",
            Network::Polygon => "MATICUSDT",
            // ETH L1 and L2s all use ETH as native gas token
            Network::Ethereum | Network::Arbitrum | Network::Base | Network::Optimism => "ETHUSDT",
            Network::Solana => "SOLUSDT",
        };
        self.get_cached_price(symbol).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_binance_price_fetch_mock() {
        // Ensure we bypass any local proxy for the mock server
        std::env::set_var("NO_PROXY", "127.0.0.1,localhost");

        // Start a mock server
        let mock_server = MockServer::start().await;

        // Mock response
        Mock::given(method("GET"))
            .and(path("/api/v3/ticker/price"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "symbol": "TRXUSDT",
                "price": "0.35000000"
            })))
            .mount(&mock_server)
            .await;

        // Configure oracle to use mock server
        let mock_url = format!("{}/api/v3/ticker/price", mock_server.uri());
        let oracle = BinancePriceOracle::try_new(Some(60))
            .unwrap()
            .with_api_url(mock_url);

        let price = oracle.get_trx_usdt_price().await.unwrap();

        // Validations
        assert_eq!(price, Decimal::new(35, 2)); // 0.35
    }

    #[tokio::test]
    async fn test_binance_fetch_failure() {
        // Start a mock server
        let mock_server = MockServer::start().await;

        // Mock 500 error
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let mock_url = format!("{}/api/v3/ticker/price", mock_server.uri());
        let oracle = BinancePriceOracle::try_new(Some(60))
            .unwrap()
            .with_api_url(mock_url);

        let result = oracle.get_trx_usdt_price().await;

        // Should return Error, not fallback
        assert!(result.is_err());
    }
}
