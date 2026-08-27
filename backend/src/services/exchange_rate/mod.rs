//! Exchange Rate Service
//!
//! Syncs crypto/fiat rates from CoinGecko, caches in DashMap + persists to DB.
//! Background task runs on CancellationToken, same pattern as sweeper/indexer.

use anyhow::{anyhow, Result};
use dashmap::DashMap;
use rust_decimal::Decimal;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// Cached rate entry with timestamp
struct CachedRate {
    rate: Decimal,
    updated_at: std::time::Instant,
}

/// Supported fiat currencies (Phase 1)
const SUPPORTED_FIATS: &[&str] = &[
    "usd", "eur", "gbp", "cny", "jpy", "krw", "sgd", "hkd", "twd", "rub", "aud", "nzd", "cad",
    "chf", "sek", "nok", "dkk", "aed", "zar", "mxn", "inr",
];

/// Exchange Rate Service — CoinGecko sync + DashMap cache
pub struct ExchangeRateService {
    db: DatabaseConnection,
    http_client: reqwest::Client,
    /// Cache: (CRYPTO, FIAT) → CachedRate (e.g., ("USDT", "USD") → 1.0005)
    cache: Arc<DashMap<(String, String), CachedRate>>,
    coingecko_api_key: Option<String>,
    sync_interval_secs: u64,
    /// Max age before cache is considered stale and rejected
    staleness_threshold_secs: u64,
    /// Delay before first sync — stagger prod/sandbox to avoid CoinGecko 429
    startup_delay_secs: u64,
}

impl ExchangeRateService {
    pub fn new(
        db: DatabaseConnection,
        coingecko_api_key: Option<String>,
        sync_interval_secs: u64,
        startup_delay_secs: u64,
    ) -> Self {
        Self {
            db,
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .user_agent("IronixPay/1.0 (exchange-rate-sync)")
                .build()
                .expect("Failed to build HTTP client"),
            cache: Arc::new(DashMap::new()),
            coingecko_api_key,
            sync_interval_secs,
            staleness_threshold_secs: 3600, // 1 hour max staleness
            startup_delay_secs,
        }
    }

    /// Start background sync loop (same pattern as sweeper/indexer)
    pub async fn start(self: Arc<Self>, token: CancellationToken) -> Result<()> {
        info!(
            interval_secs = self.sync_interval_secs,
            startup_delay_secs = self.startup_delay_secs,
            "ExchangeRateService started"
        );

        // Stagger startup to avoid CoinGecko 429 when prod+sandbox restart together
        if self.startup_delay_secs > 0 {
            info!(
                delay_secs = self.startup_delay_secs,
                "Delaying initial rate sync to stagger CoinGecko requests"
            );
            tokio::time::sleep(Duration::from_secs(self.startup_delay_secs)).await;
        }

        // Initial sync on startup
        if let Err(e) = self.sync_rates().await {
            warn!(error = %e, "Initial rate sync failed — will retry on next interval");
        }

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    info!("ExchangeRateService shutting down");
                    break;
                }
                _ = tokio::time::sleep(Duration::from_secs(self.sync_interval_secs)) => {
                    if let Err(e) = self.sync_rates().await {
                        warn!(error = %e, "Rate sync failed — using cached rates");
                    }
                }
            }
        }

        Ok(())
    }

    /// Sync rates from CoinGecko for all supported crypto/fiat pairs
    async fn sync_rates(&self) -> Result<()> {
        let ids = "tether,usd-coin";
        let vs_currencies = SUPPORTED_FIATS.join(",");

        let mut url = format!(
            "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies={}",
            ids, vs_currencies
        );

        // Add API key as header or query param based on CoinGecko demo key format
        if let Some(ref key) = self.coingecko_api_key {
            url.push_str(&format!("&x_cg_demo_api_key={}", key));
        }

        debug!("Syncing exchange rates from CoinGecko");

        let resp = self.http_client.get(&url).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!(
                "CoinGecko API error: status={} body={}",
                status,
                body
            ));
        }

        // Response format: { "tether": { "usd": 1.0005, "cny": 7.25 }, "usd-coin": { ... } }
        let data: serde_json::Value = resp.json().await?;

        let mut count = 0;
        let now = std::time::Instant::now();
        let crypto_pairs = [("tether", "USDT"), ("usd-coin", "USDC")];

        for (cg_id, crypto_symbol) in &crypto_pairs {
            if let Some(prices) = data.get(cg_id).and_then(|v| v.as_object()) {
                for (fiat, rate_val) in prices {
                    if let Some(rate_f64) = rate_val.as_f64() {
                        let rate =
                            Decimal::from_str(&format!("{:.8}", rate_f64)).unwrap_or_default();
                        let fiat_upper = fiat.to_uppercase();

                        // Update DashMap cache
                        self.cache.insert(
                            (crypto_symbol.to_string(), fiat_upper.clone()),
                            CachedRate {
                                rate,
                                updated_at: now,
                            },
                        );

                        // Upsert to DB — keep only latest rate per (crypto, fiat) pair
                        let upsert_sql = r#"
                            INSERT INTO exchange_rates (crypto, fiat, rate, source, created_at, updated_at)
                            VALUES ($1, $2, $3, 'coingecko', NOW(), NOW())
                            ON CONFLICT (crypto, fiat) DO UPDATE
                            SET rate = $3, updated_at = NOW()
                        "#;
                        if let Err(e) = self
                            .db
                            .execute(Statement::from_sql_and_values(
                                DbBackend::Postgres,
                                upsert_sql,
                                [
                                    crypto_symbol.to_string().into(),
                                    fiat_upper.into(),
                                    rate.into(),
                                ],
                            ))
                            .await
                        {
                            warn!(error = %e, "Failed to upsert exchange rate to DB");
                        }

                        count += 1;
                    }
                }
            }
        }

        info!(rates_synced = count, "Exchange rates synced from CoinGecko");
        Ok(())
    }

    /// Get cached rate for a crypto/fiat pair.
    ///
    /// Returns error if rate is not cached or is stale (>staleness_threshold).
    ///
    /// Special case: USD ↔ USDT/USDC uses a hardcoded 1:1 rate since both are
    /// USD-pegged stablecoins. This avoids ugly micro-differences from CoinGecko
    /// (e.g. 0.99998) and eliminates API dependency for the most common case.
    pub fn get_rate(&self, crypto: &str, fiat: &str) -> Result<Decimal> {
        // Stablecoin ↔ USD: hardcoded 1:1, no CoinGecko needed
        if fiat.eq_ignore_ascii_case("USD")
            && matches!(crypto.to_uppercase().as_str(), "USDT" | "USDC")
        {
            return Ok(Decimal::ONE);
        }

        let key = (crypto.to_uppercase(), fiat.to_uppercase());

        match self.cache.get(&key) {
            Some(entry) => {
                let age = entry.updated_at.elapsed();
                if age > Duration::from_secs(self.staleness_threshold_secs) {
                    Err(anyhow!(
                        "Exchange rate for {}/{} is stale ({:.0}s old, max {}s). Cannot create fiat-priced session.",
                        crypto, fiat, age.as_secs_f64(), self.staleness_threshold_secs
                    ))
                } else {
                    Ok(entry.rate)
                }
            }
            None => Err(anyhow!(
                "No exchange rate cached for {}/{}. Rate sync may not have completed yet.",
                crypto,
                fiat
            )),
        }
    }

    /// Get all supported fiat currency codes (uppercase)
    pub fn supported_fiats() -> Vec<&'static str> {
        SUPPORTED_FIATS
            .iter()
            .map(|f| match *f {
                "usd" => "USD",
                "cny" => "CNY",
                "eur" => "EUR",
                "gbp" => "GBP",
                "jpy" => "JPY",
                "krw" => "KRW",
                "sgd" => "SGD",
                "hkd" => "HKD",
                "twd" => "TWD",
                "rub" => "RUB",
                "aud" => "AUD",
                "nzd" => "NZD",
                "cad" => "CAD",
                "chf" => "CHF",
                "sek" => "SEK",
                "nok" => "NOK",
                "dkk" => "DKK",
                "aed" => "AED",
                "zar" => "ZAR",
                "mxn" => "MXN",
                "inr" => "INR",
                other => other,
            })
            .collect()
    }

    /// Check if a currency code is a supported fiat currency
    pub fn is_supported_fiat(currency: &str) -> bool {
        let upper = currency.to_uppercase();
        Self::supported_fiats().contains(&upper.as_str())
    }

    /// Check if a currency code is a crypto (USDT/USDC)
    pub fn is_crypto(currency: &str) -> bool {
        matches!(currency.to_uppercase().as_str(), "USDT" | "USDC")
    }
}
