//! AML Service - 2-Layer Risk Checking
//!
//! L1: Local blacklist (DashSet in-memory cache)
//! L2: GoPlus API with 24h DB cache (multi-chain: TRON, BSC, etc.)
//!
//! Fail-open policy: API timeout/error → PASS (L1 already blocks critical addresses)

use super::entity::{self, api_cache};
use super::goplus::GoPlusClient;
use anyhow::Result;
use chrono::{Duration, Utc};
use dashmap::DashSet;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// AML check result
#[derive(Debug, Clone)]
pub enum RiskResult {
    /// Address is safe to process
    Safe,
    /// Address is blocked due to AML risk
    Blocked { reason: String },
}

/// AML service configuration
#[derive(Clone, Debug)]
pub struct AmlConfig {
    /// API timeout in seconds (default: 3)
    pub api_timeout_seconds: u64,
    /// Whether to use fail-open policy (default: true for MVP)
    pub fail_open: bool,
    /// Cache TTL in hours (default: 24)
    pub cache_ttl_hours: i64,
}

impl Default for AmlConfig {
    fn default() -> Self {
        Self {
            api_timeout_seconds: 3,
            fail_open: true,
            cache_ttl_hours: 24,
        }
    }
}

/// Map network string to GoPlus API chain_id.
///
/// GoPlus uses numeric chain IDs for EVM chains and "tron" for TRON.
/// See: https://docs.gopluslabs.io/reference/supported-blockchains
fn network_to_goplus_chain_id(network: &str) -> &'static str {
    let n = network.to_uppercase();
    if n.contains("BSC") || n.contains("BNB") {
        "56" // BNB Chain (BSC) mainnet
    } else if n.contains("ETH") {
        "1" // Ethereum mainnet
    } else {
        "tron" // TRON (default)
    }
}

/// AML Service with 2-layer risk checking
pub struct AmlService {
    /// L1: In-memory blacklist cache (O(1) lookup)
    blacklist_cache: Arc<DashSet<String>>,
    /// Database connection for L2 cache and blacklist loading
    db: DatabaseConnection,
    /// GoPlus API client
    goplus_client: GoPlusClient,
    /// Configuration
    config: AmlConfig,
}

impl AmlService {
    /// Create new AML service
    pub fn new(db: DatabaseConnection, config: AmlConfig) -> Self {
        let goplus_client = GoPlusClient::new(config.api_timeout_seconds);

        Self {
            blacklist_cache: Arc::new(DashSet::new()),
            db,
            goplus_client,
            config,
        }
    }

    /// Load blacklist from database into memory cache
    /// Call this at startup
    pub async fn load_blacklist_from_db(&self) -> Result<usize> {
        let records = entity::Entity::find().all(&self.db).await?;

        let count = records.len();
        for record in records {
            let normalized = Self::normalize_address(&record.address);
            self.blacklist_cache.insert(normalized);
        }

        info!("Loaded {} addresses into AML blacklist cache", count);
        Ok(count)
    }

    /// Main check: 2-layer risk screening
    ///
    /// 1. L1: Check in-memory blacklist (O(1))
    /// 2. L2: Check DB cache, then GoPlus API if cache miss/expired
    ///
    /// `network` is used to route the GoPlus API call to the correct chain
    /// (e.g. "TRON" → chain_id "tron", "BSC" → chain_id "56").
    pub async fn check_address(&self, address: &str, network: &str) -> Result<RiskResult> {
        let normalized = Self::normalize_address(address);

        // L1: Fast in-memory blacklist check
        if self.blacklist_cache.contains(&normalized) {
            debug!("L1 hit: {} is in blacklist", normalized);
            crate::services::metrics::inc_aml_check(network, "blocked");
            return Ok(RiskResult::Blocked {
                reason: "OFAC_SANCTIONS".to_string(),
            });
        }

        // L2: Check DB cache first
        if let Some(cached) = self.get_cached_result(&normalized).await? {
            debug!("L2 cache hit for {}", normalized);
            let label = match &cached {
                RiskResult::Safe => "safe",
                RiskResult::Blocked { .. } => "blocked",
            };
            crate::services::metrics::inc_aml_check(network, label);
            return Ok(cached);
        }

        // L2: Call GoPlus API with correct chain_id
        let chain_id = network_to_goplus_chain_id(network);
        match self.check_via_goplus(&normalized, chain_id).await {
            Ok(result) => {
                // Cache the result
                self.cache_result(&normalized, &result).await?;
                let label = match &result {
                    RiskResult::Safe => "safe",
                    RiskResult::Blocked { .. } => "blocked",
                };
                crate::services::metrics::inc_aml_check(network, label);
                Ok(result)
            }
            Err(e) => {
                warn!("GoPlus API failed for {}: {}", normalized, e);
                crate::services::metrics::inc_aml_check(network, "error");
                if self.config.fail_open {
                    // Fail-open: API error → PASS
                    // L1 already blocks critical OFAC addresses
                    Ok(RiskResult::Safe)
                } else {
                    // Fail-closed: API error → BLOCK
                    Ok(RiskResult::Blocked {
                        reason: "API_TIMEOUT".to_string(),
                    })
                }
            }
        }
    }

    /// Normalize address format for consistent cache keys
    fn normalize_address(address: &str) -> String {
        // TRON: Base58Check (case-sensitive), EVM: 0x hex (case-insensitive).
        // Trim whitespace for edge case defense. We do NOT lowercase here because
        // TRON addresses are case-sensitive and DB cache keys must be exact.
        address.trim().to_string()
    }

    /// Check DB cache for recent API result
    async fn get_cached_result(&self, address: &str) -> Result<Option<RiskResult>> {
        let cached = api_cache::Entity::find_by_id(address).one(&self.db).await?;

        if let Some(record) = cached {
            // Check if cache is still valid (within TTL)
            let cache_age = Utc::now().signed_duration_since(record.checked_at.with_timezone(&Utc));
            if cache_age < Duration::hours(self.config.cache_ttl_hours) {
                if record.is_risky {
                    return Ok(Some(RiskResult::Blocked {
                        reason: record
                            .risk_reason
                            .unwrap_or_else(|| "GOPLUS_RISK".to_string()),
                    }));
                } else {
                    return Ok(Some(RiskResult::Safe));
                }
            }
            // Cache expired, will be refreshed
        }

        Ok(None)
    }

    /// Call GoPlus API for risk check
    async fn check_via_goplus(&self, address: &str, chain_id: &str) -> Result<RiskResult> {
        let result = self.goplus_client.check_address(address, chain_id).await?;

        match result {
            Some(security_result) => {
                if security_result.is_high_risk() {
                    Ok(RiskResult::Blocked {
                        reason: security_result
                            .risk_reason()
                            .unwrap_or_else(|| "GOPLUS_RISK".to_string()),
                    })
                } else {
                    Ok(RiskResult::Safe)
                }
            }
            None => {
                // No data from GoPlus - treat as safe
                debug!("No GoPlus data for {}, treating as safe", address);
                Ok(RiskResult::Safe)
            }
        }
    }

    /// Cache API result to database
    async fn cache_result(&self, address: &str, result: &RiskResult) -> Result<()> {
        let (is_risky, risk_reason) = match result {
            RiskResult::Safe => (false, None),
            RiskResult::Blocked { reason } => (true, Some(reason.clone())),
        };

        let now = Utc::now();

        // Upsert: update if exists, insert if not
        let existing = api_cache::Entity::find_by_id(address).one(&self.db).await?;

        if existing.is_some() {
            // Update existing record
            api_cache::ActiveModel {
                address: Set(address.to_string()),
                is_risky: Set(is_risky),
                risk_reason: Set(risk_reason),
                checked_at: Set(now.into()),
            }
            .update(&self.db)
            .await?;
        } else {
            // Insert new record
            api_cache::ActiveModel {
                address: Set(address.to_string()),
                is_risky: Set(is_risky),
                risk_reason: Set(risk_reason),
                checked_at: Set(now.into()),
            }
            .insert(&self.db)
            .await?;
        }

        debug!("Cached AML result for {}: is_risky={}", address, is_risky);
        Ok(())
    }

    /// Add address to runtime blacklist (does not persist to DB)
    /// Use for dynamic blocking during runtime
    pub fn add_to_blacklist(&self, address: &str) {
        let normalized = Self::normalize_address(address);
        self.blacklist_cache.insert(normalized);
    }

    /// Get current blacklist size (for monitoring)
    pub fn blacklist_size(&self) -> usize {
        self.blacklist_cache.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_address() {
        assert_eq!(AmlService::normalize_address("  TAbc123  "), "TAbc123");
    }
}
