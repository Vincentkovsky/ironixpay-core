//! EVM blockchain client (BSC, Ethereum, etc.)
//!
//! Provides JSON-RPC interface to EVM-compatible chains for USDT operations.
//! Uses reqwest for HTTP calls (same pattern as TronClient) with
//! alloy-sol-types for ABI encoding.
//!
//! # Supported RPC Methods
//! - `eth_call` (ERC-20 balanceOf)
//! - `eth_getBalance` (native balance)
//! - `eth_blockNumber` + `eth_getBlockByNumber`
//! - `eth_getTransactionReceipt`
//! - `eth_getTransactionByHash`
//! - `eth_sendRawTransaction`
//! - `eth_getLogs` (for indexer)

pub mod gas_funder;
pub mod signing;

use alloy_primitives::{Address, U256};
use alloy_sol_types::{sol, SolCall};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use reqwest_middleware::ClientBuilder;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::services::chain::traits::ChainClient;
use crate::services::chain::types::*;

// ─── ABI Definitions ────────────────────────────────────────────────────────

sol! {
    function balanceOf(address account) external view returns (uint256);
    function transfer(address to, uint256 amount) external returns (bool);
}

// ─── JSON-RPC Types ─────────────────────────────────────────────────────────

#[derive(Serialize, Debug)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    method: String,
    params: serde_json::Value,
    id: u64,
}

impl JsonRpcRequest {
    fn new(method: &str, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0",
            method: method.to_string(),
            params,
            id: 1,
        }
    }
}

#[derive(Deserialize, Debug)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<JsonRpcError>,
    #[allow(dead_code)]
    id: Option<u64>,
}

#[derive(Deserialize, Debug)]
struct JsonRpcError {
    code: i64,
    message: String,
}

/// EVM transaction receipt from `eth_getTransactionReceipt`
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct EvmReceipt {
    transaction_hash: String,
    block_number: String,
    status: String, // "0x1" = success, "0x0" = revert
    gas_used: String,
    effective_gas_price: Option<String>,
}

/// EVM transaction from `eth_getTransactionByHash`
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct EvmTransaction {
    hash: String,
    block_number: Option<String>,
    from: String,
    to: Option<String>,
    value: String,
    input: String,
    /// Legacy gas price (always present, used as fallback when effectiveGasPrice is missing)
    gas_price: Option<String>,
}

/// EVM block from `eth_getBlockByNumber`
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct EvmBlock {
    number: String,
    timestamp: String,
}

/// EVM log entry from `eth_getLogs`
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EvmLog {
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
    /// Null when log is from a pending block or during brief reorg windows.
    pub block_number: Option<String>,
    /// Null when log is from a pending block.
    pub transaction_hash: Option<String>,
    /// Null when log is from a pending block.
    pub log_index: Option<String>,
    pub removed: Option<bool>,
}

// ─── EvmClient ──────────────────────────────────────────────────────────────

/// RPC endpoint with provider-specific limits.
struct RpcEndpoint {
    url: String,
    /// Max block range for `eth_getLogs` (Alchemy free=10, Ankr/public=1000)
    max_block_range: u16,
}

impl RpcEndpoint {
    fn new(url: String) -> Self {
        // Auto-detect max_block_range from URL hostname.
        // Alchemy free plan limits eth_getLogs to 10-block ranges.
        // Most other providers (Ankr, public RPCs) support 1000+.
        // TODO: Add explicit TOML override if new providers with different limits appear.
        let max_block_range = if url.contains("alchemy.com") {
            10
        } else {
            1000
        };
        Self {
            url,
            max_block_range,
        }
    }

    /// Extract a human-readable provider name from the URL hostname.
    fn provider_name(&self) -> String {
        if self.url.contains("alchemy.com") {
            "Alchemy".to_string()
        } else if self.url.contains("ankr.com") {
            "Ankr".to_string()
        } else if self.url.contains("infura.io") {
            "Infura".to_string()
        } else if self.url.contains("quicknode.com") {
            "QuickNode".to_string()
        } else {
            // Extract hostname as fallback
            self.url
                .split("//")
                .nth(1)
                .and_then(|s| s.split('/').next())
                .unwrap_or("Unknown")
                .to_string()
        }
    }

    /// Build a masked endpoint identifier for admin display.
    ///
    /// For API-key-based providers: "Provider (…last4)" e.g. "Alchemy (…QCkJ)"
    /// For public RPCs: hostname e.g. "mainnet.base.org"
    fn masked_endpoint(&self) -> String {
        let provider = self.provider_name();
        // Check if URL has an API key path segment (e.g. /v2/KEY or /KEY)
        // Hostnames contain dots; real API keys never do.
        let key_suffix = self
            .url
            .rsplit('/')
            .next()
            .filter(|segment| segment.len() > 8 && !segment.contains('.'))
            .map(|key| {
                let last4 = &key[key.len().saturating_sub(4)..];
                format!(" (…{})", last4)
            })
            .unwrap_or_default();
        format!("{}{}", provider, key_suffix)
    }
}

/// Number of consecutive `rpc_call` failures before triggering failover.
/// Each failure already includes 3 reqwest-level retries, so 2 × 4 = 8 HTTP attempts.
const FAILOVER_THRESHOLD: u32 = 2;

/// After failover, wait this long before probing primary again.
/// The probe is transparent: the next regular rpc_call goes to primary.
/// If it fails, on_failure re-triggers failover within 2 calls.
const RECOVERY_PROBE_INTERVAL_SECS: i64 = 60;

#[cfg(not(test))]
const BROADCAST_AMBIGUITY_PROBE_ATTEMPTS: usize = 8;
#[cfg(test)]
const BROADCAST_AMBIGUITY_PROBE_ATTEMPTS: usize = 2;

#[cfg(not(test))]
const BROADCAST_AMBIGUITY_PROBE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
#[cfg(test)]
const BROADCAST_AMBIGUITY_PROBE_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1);

pub struct EvmClient {
    http: ClientWithMiddleware,
    /// Direct client without automatic retries, used to probe signed transaction hashes.
    probe_http: Client,
    endpoints: Vec<RpcEndpoint>,
    /// Index of the currently active endpoint (0 = primary)
    active: std::sync::atomic::AtomicUsize,
    /// Consecutive failure count for the active endpoint
    fail_count: std::sync::atomic::AtomicU32,
    /// Unix timestamp when failover was triggered (0 = on primary, never failed over)
    failover_at: std::sync::atomic::AtomicI64,
    chain_id: u64,
}

impl EvmClient {
    pub fn new(rpc_urls: Vec<String>, chain_id: u64) -> Self {
        assert!(!rpc_urls.is_empty(), "At least one RPC URL is required");

        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        let base_client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("Failed to build HTTP client");
        let probe_http = base_client.clone();
        let http = ClientBuilder::new(base_client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        let endpoints: Vec<RpcEndpoint> = rpc_urls.into_iter().map(RpcEndpoint::new).collect();

        Self {
            http,
            probe_http,
            endpoints,
            active: std::sync::atomic::AtomicUsize::new(0),
            fail_count: std::sync::atomic::AtomicU32::new(0),
            failover_at: std::sync::atomic::AtomicI64::new(0),
            chain_id,
        }
    }

    /// Current active endpoint's max eth_getLogs block range.
    /// Changes dynamically when failover occurs (e.g., Alchemy 10 → Ankr 1000).
    pub fn max_block_range(&self) -> u16 {
        let idx = self.active.load(std::sync::atomic::Ordering::Relaxed);
        self.endpoints[idx].max_block_range
    }

    /// Get the URL of the currently active endpoint.
    fn active_url(&self) -> &str {
        let idx = self.active.load(std::sync::atomic::Ordering::Relaxed);
        &self.endpoints[idx].url
    }

    /// If on a fallback endpoint and enough time has passed, switch back to
    /// primary. Called from on_success() so recovery happens AFTER a successful
    /// fallback call. This ensures the next scan_new_blocks() reads
    /// max_block_range() with primary's value before constructing requests.
    fn try_recover_primary(&self) {
        use std::sync::atomic::Ordering;

        let current = self.active.load(Ordering::Relaxed);
        if current == 0 {
            return; // already on primary
        }

        let failover_time = self.failover_at.load(Ordering::Relaxed);
        let now = chrono::Utc::now().timestamp();
        if now - failover_time < RECOVERY_PROBE_INTERVAL_SECS {
            return; // too soon
        }

        // CAS: only one thread wins the recovery attempt
        if self
            .active
            .compare_exchange(current, 0, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
        {
            self.fail_count.store(0, Ordering::SeqCst);
            // Update failover_at so next probe waits another interval if recovery fails
            self.failover_at.store(now, Ordering::Relaxed);
            tracing::info!(
                from = %self.endpoints[current].url,
                to = %self.endpoints[0].url,
                "RPC RECOVERY: switching back to primary endpoint"
            );
        }
    }

    /// Record a successful RPC call — reset failure counter and check recovery.
    fn on_success(&self) {
        if self.fail_count.load(std::sync::atomic::Ordering::Relaxed) > 0 {
            self.fail_count
                .store(0, std::sync::atomic::Ordering::Relaxed);
        }
        // After a successful fallback call, check if it's time to switch back
        self.try_recover_primary();
    }

    /// Record a failed RPC call — potentially trigger failover.
    fn on_failure(&self) {
        use std::sync::atomic::Ordering;

        let fails = self.fail_count.fetch_add(1, Ordering::SeqCst) + 1;
        if fails >= FAILOVER_THRESHOLD && self.endpoints.len() > 1 {
            let current = self.active.load(Ordering::Acquire);
            let next = current + 1;
            if next < self.endpoints.len() {
                // CAS: only one thread wins the switch
                if self
                    .active
                    .compare_exchange(current, next, Ordering::SeqCst, Ordering::Relaxed)
                    .is_ok()
                {
                    self.fail_count.store(0, Ordering::SeqCst);
                    self.failover_at
                        .store(chrono::Utc::now().timestamp(), Ordering::Relaxed);
                    tracing::warn!(
                        from = %self.endpoints[current].url,
                        to = %self.endpoints[next].url,
                        max_block_range = self.endpoints[next].max_block_range,
                        "RPC FAILOVER: switched to backup endpoint"
                    );
                }
            } else if current > 0 {
                // On last endpoint with sustained failures.
                // Probe primary if enough time has passed — avoids permanent
                // deadlock when primary recovers but last backup stays down.
                let failover_time = self.failover_at.load(Ordering::Relaxed);
                let now = chrono::Utc::now().timestamp();
                if now - failover_time >= RECOVERY_PROBE_INTERVAL_SECS {
                    if self
                        .active
                        .compare_exchange(current, 0, Ordering::SeqCst, Ordering::Relaxed)
                        .is_ok()
                    {
                        self.fail_count.store(0, Ordering::SeqCst);
                        self.failover_at.store(now, Ordering::Relaxed);
                        tracing::info!(
                            from = %self.endpoints[current].url,
                            to = %self.endpoints[0].url,
                            "RPC RECOVERY PROBE: last endpoint failing, retrying primary"
                        );
                    }
                }
            }
        }
    }

    /// Send a JSON-RPC request and parse the response.
    /// Errors if the RPC returns an error or null result.
    async fn rpc_call<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T> {
        self.rpc_call_optional(method, params)
            .await?
            .ok_or_else(|| anyhow!("RPC returned null result for {}", method))
    }

    /// Send a JSON-RPC request, allowing null result (returns `Ok(None)`).
    /// Propagates network errors and RPC errors; only null result → None.
    async fn rpc_call_optional<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<Option<T>> {
        let req = JsonRpcRequest::new(method, params);
        debug!(method = %req.method, "EVM RPC call");

        let result = self.http.post(self.active_url()).json(&req).send().await;

        let resp_body = match result {
            Ok(resp) => resp,
            Err(e) => {
                self.on_failure();
                return Err(e.into());
            }
        };

        let resp: JsonRpcResponse<T> = match resp_body.json().await {
            Ok(r) => r,
            Err(e) => {
                self.on_failure();
                return Err(e.into());
            }
        };

        if let Some(err) = resp.error {
            self.on_failure();
            return Err(anyhow!("RPC error {}: {}", err.code, err.message));
        }

        self.on_success();
        Ok(resp.result)
    }

    /// Check every configured endpoint for a locally signed transaction hash.
    ///
    /// `eth_sendRawTransaction` is not atomic with its HTTP response: a node may
    /// accept and propagate a transaction even if the client later sees a timeout
    /// or a JSON-RPC error from a retry. The signed EVM transaction hash is already
    /// deterministic, so use it to resolve that ambiguity before reporting failure.
    pub async fn transaction_known_on_any_endpoint(&self, tx_hash: &str) -> Result<bool> {
        let mut endpoint_responded = vec![false; self.endpoints.len()];

        for attempt in 0..BROADCAST_AMBIGUITY_PROBE_ATTEMPTS {
            for (index, endpoint) in self.endpoints.iter().enumerate() {
                let req =
                    JsonRpcRequest::new("eth_getTransactionByHash", serde_json::json!([tx_hash]));
                let response = self.probe_http.post(&endpoint.url).json(&req).send().await;

                let known = match response {
                    Ok(response) => {
                        match response.json::<JsonRpcResponse<serde_json::Value>>().await {
                            Ok(body) if body.error.is_none() => {
                                endpoint_responded[index] = true;
                                body.result.is_some()
                            }
                            _ => false,
                        }
                    }
                    Err(_) => false,
                };

                if known {
                    warn!(
                        tx_hash,
                        provider = %endpoint.provider_name(),
                        attempt = attempt + 1,
                        "Recovered ambiguous EVM broadcast by local transaction hash"
                    );
                    return Ok(true);
                }
            }

            if attempt + 1 < BROADCAST_AMBIGUITY_PROBE_ATTEMPTS {
                tokio::time::sleep(BROADCAST_AMBIGUITY_PROBE_INTERVAL).await;
            }
        }

        if endpoint_responded.iter().all(|responded| *responded) {
            Ok(false)
        } else {
            Err(anyhow!(
                "Could not establish transaction absence across every configured EVM RPC"
            ))
        }
    }

    /// Parse a hex string (0x-prefixed) to u64.
    /// Returns 0 for empty/"0x" input (EVM returns "0x" for non-existent data).
    fn parse_hex_u64(s: &str) -> Result<u64> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        if s.is_empty() {
            return Ok(0);
        }
        u64::from_str_radix(s, 16).map_err(|e| anyhow!("Invalid hex u64 '{}': {}", s, e))
    }

    /// Parse a hex string (0x-prefixed) to i64.
    /// Returns 0 for empty/"0x" input.
    pub fn parse_hex_i64(s: &str) -> Result<i64> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        if s.is_empty() {
            return Ok(0);
        }
        i64::from_str_radix(s, 16).map_err(|e| anyhow!("Invalid hex i64 '{}': {}", s, e))
    }

    /// Parse a hex string (0x-prefixed) to U256.
    /// Returns U256::ZERO for empty/"0x" input (common EVM edge case:
    /// eth_call to non-contract address or self-destructed contract).
    fn parse_hex_u256(s: &str) -> Result<U256> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        if s.is_empty() {
            return Ok(U256::ZERO);
        }
        U256::from_str_radix(s, 16).map_err(|e| anyhow!("Invalid hex U256 '{}': {}", s, e))
    }

    // ─── Public EVM-specific Methods (for Indexer) ──────────────────────────

    /// Fetch logs for a block range, filtered by contract(s) + Transfer topic.
    ///
    /// Used by EVM Indexer to scan for ERC-20 token transfers.
    /// Accepts multiple contract addresses (e.g., USDT + USDC) in a single RPC call.
    pub async fn get_logs(
        &self,
        from_block: u64,
        to_block: u64,
        contract_addresses: &[&str],
    ) -> Result<Vec<EvmLog>> {
        // Transfer(address indexed from, address indexed to, uint256 value)
        let transfer_topic = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

        // eth_getLogs `address` field natively supports a JSON array
        let address_filter: serde_json::Value = if contract_addresses.len() == 1 {
            serde_json::json!(contract_addresses[0])
        } else {
            serde_json::json!(contract_addresses)
        };

        let params = serde_json::json!([{
            "fromBlock": format!("0x{:x}", from_block),
            "toBlock": format!("0x{:x}", to_block),
            "address": address_filter,
            "topics": [transfer_topic]
        }]);

        self.rpc_call("eth_getLogs", params).await
    }

    /// Fetch the pending nonce for an address (eth_getTransactionCount).
    pub async fn get_nonce(&self, address: &str) -> Result<u64> {
        let hex: String = self
            .rpc_call(
                "eth_getTransactionCount",
                serde_json::json!([address, "pending"]),
            )
            .await?;
        Self::parse_hex_u64(&hex)
    }

    /// Fetch the confirmed nonce. If this is greater than a missing signed
    /// transaction's nonce, another transaction consumed that nonce.
    pub async fn get_latest_nonce(&self, address: &str) -> Result<u64> {
        let hex: String = self
            .rpc_call(
                "eth_getTransactionCount",
                serde_json::json!([address, "latest"]),
            )
            .await?;
        Self::parse_hex_u64(&hex)
    }

    /// Fetch the confirmed nonce from every configured endpoint. The minimum
    /// is the only safe replacement proof because providers may lag each other.
    pub async fn get_latest_nonce_across_endpoints(&self, address: &str) -> Result<u64> {
        let mut minimum = None;
        for endpoint in &self.endpoints {
            let request = JsonRpcRequest::new(
                "eth_getTransactionCount",
                serde_json::json!([address, "latest"]),
            );
            let response = self
                .probe_http
                .post(&endpoint.url)
                .json(&request)
                .send()
                .await
                .with_context(|| {
                    format!(
                        "Failed to query confirmed nonce from {}",
                        endpoint.provider_name()
                    )
                })?;
            let body: JsonRpcResponse<String> = response.json().await.with_context(|| {
                format!(
                    "Invalid confirmed nonce response from {}",
                    endpoint.provider_name()
                )
            })?;
            if let Some(error) = body.error {
                return Err(anyhow!(
                    "Confirmed nonce RPC error from {}: {}",
                    endpoint.provider_name(),
                    error.message
                ));
            }
            let nonce = Self::parse_hex_u64(
                body.result
                    .as_deref()
                    .ok_or_else(|| anyhow!("Confirmed nonce RPC returned null"))?,
            )?;
            minimum = Some(minimum.map_or(nonce, |current: u64| current.min(nonce)));
        }

        minimum.ok_or_else(|| anyhow!("No EVM RPC endpoints configured"))
    }

    /// Fetch current gas price in wei (eth_gasPrice).
    pub async fn get_gas_price(&self) -> Result<u64> {
        let hex: String = self.rpc_call("eth_gasPrice", serde_json::json!([])).await?;
        Self::parse_hex_u64(&hex)
    }

    /// Estimate gas for a transaction via `eth_estimateGas`.
    ///
    /// Returns estimated gas units. Callers should apply a buffer (e.g., 1.2x).
    pub async fn estimate_gas(&self, tx: &serde_json::Value) -> Result<u64> {
        let params = serde_json::json!([tx, "latest"]);
        let hex: String = self.rpc_call("eth_estimateGas", params).await?;
        Self::parse_hex_u64(&hex)
    }

    /// Estimate gas needed for an ERC-20 token transfer.
    ///
    /// Builds the calldata for `transfer(to, amount)` and calls `eth_estimateGas`.
    /// Returns estimated gas with 20% buffer, minimum 65k.
    ///
    /// Used by both `build_token_transfer` (for the actual tx) and callers
    /// like `EvmSweepExecutor`/`EvmPayoutExecutor` who need to fund gas before building.
    pub async fn estimate_token_transfer_gas(
        &self,
        from: &str,
        to: &str,
        token_address: &str,
        amount: U256,
    ) -> u64 {
        let to_addr: Address = match to.parse() {
            Ok(a) => a,
            Err(_) => return 65_000,
        };
        let call = transferCall {
            to: to_addr,
            amount,
        };
        let calldata = hex::encode(call.abi_encode());
        let estimate_tx = serde_json::json!({
            "from": from,
            "to": token_address,
            "data": format!("0x{}", calldata),
        });
        let estimated = self.estimate_gas(&estimate_tx).await.unwrap_or(65_000);
        std::cmp::max(estimated * 12 / 10, 65_000)
    }

    /// Get the chain ID configured for this client.
    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }
}

// ─── ChainClient Implementation ────────────────────────────────────────────

#[async_trait]
impl ChainClient for EvmClient {
    async fn get_token_balance(&self, address: &str, token_address: &str) -> Result<U256> {
        let addr: Address = address
            .parse()
            .map_err(|e| anyhow!("Invalid address: {}", e))?;
        let call = balanceOfCall { account: addr };
        let calldata = hex::encode(call.abi_encode());

        let params = serde_json::json!([
            { "to": token_address, "data": format!("0x{}", calldata) },
            "latest"
        ]);

        let result: String = self.rpc_call("eth_call", params).await?;
        Self::parse_hex_u256(&result)
    }

    async fn get_native_balance(&self, address: &str) -> Result<U256> {
        let params = serde_json::json!([address, "latest"]);
        let result: String = self.rpc_call("eth_getBalance", params).await?;
        Self::parse_hex_u256(&result)
    }

    async fn build_token_transfer(
        &self,
        from: &str,
        to: &str,
        token_address: &str,
        amount: U256,
    ) -> Result<ChainUnsignedTx> {
        let to_addr: Address = to
            .parse()
            .map_err(|e| anyhow!("Invalid to address: {}", e))?;
        let call = transferCall {
            to: to_addr,
            amount,
        };
        let calldata = hex::encode(call.abi_encode());
        let nonce = self.get_nonce(from).await?;
        let gas_price = self.get_gas_price().await?;
        // +20% buffer: prevents rejection when baseFee rises between estimation and broadcast.
        // On L2s (Arbitrum/OP/Base) this costs <$0.001 extra per tx.
        let gas_price = gas_price + gas_price / 5;
        let gas_limit = self
            .estimate_token_transfer_gas(from, to, token_address, amount)
            .await;

        Ok(ChainUnsignedTx::Evm(EvmUnsignedTx {
            from: from.to_string(),
            to: token_address.to_string(),
            data: format!("0x{}", calldata),
            value: "0x0".to_string(),
            nonce,
            gas_price,
            gas_limit,
            chain_id: self.chain_id,
        }))
    }

    async fn build_native_transfer(
        &self,
        from: &str,
        to: &str,
        amount: U256,
    ) -> Result<ChainUnsignedTx> {
        let nonce = self.get_nonce(from).await?;
        let gas_price = self.get_gas_price().await?;
        // +20% buffer: prevents rejection when baseFee rises between estimation and broadcast
        let gas_price = gas_price + gas_price / 5;

        // Dynamic gas estimation with 21k floor (handles smart contract wallets).
        // Currently only used for EOA gas funding, but future-proofs for contract targets.
        let value_hex = format!("0x{:x}", amount);
        let estimate_tx = serde_json::json!({
            "from": from,
            "to": to,
            "value": &value_hex,
        });
        let estimated = self.estimate_gas(&estimate_tx).await.unwrap_or(21_000);
        let gas_limit = std::cmp::max(estimated * 12 / 10, 21_000);

        Ok(ChainUnsignedTx::Evm(EvmUnsignedTx {
            from: from.to_string(),
            to: to.to_string(),
            data: "0x".to_string(),
            value: value_hex,
            nonce,
            gas_price,
            gas_limit,
            chain_id: self.chain_id,
        }))
    }

    async fn broadcast(&self, tx: &ChainSignedTx) -> Result<ChainBroadcastResult> {
        match tx {
            ChainSignedTx::Evm(raw_tx) => {
                let local_tx_hash = raw_tx.tx_hash.clone();
                let params = serde_json::json!([&raw_tx.raw_tx_hex]);
                match self
                    .rpc_call::<String>("eth_sendRawTransaction", params)
                    .await
                {
                    Ok(rpc_tx_hash) => {
                        if !rpc_tx_hash.eq_ignore_ascii_case(&local_tx_hash) {
                            return Err(anyhow!(
                                "RPC returned transaction hash {} for signed hash {}",
                                rpc_tx_hash,
                                local_tx_hash
                            ));
                        }
                        Ok(ChainBroadcastResult {
                            success: true,
                            tx_hash: local_tx_hash,
                            message: None,
                        })
                    }
                    Err(error) => {
                        warn!(
                            tx_hash = %local_tx_hash,
                            error = %error,
                            "EVM broadcast response was ambiguous; probing transaction hash"
                        );

                        if matches!(
                            self.transaction_known_on_any_endpoint(&local_tx_hash).await,
                            Ok(true)
                        ) {
                            return Ok(ChainBroadcastResult {
                                success: true,
                                tx_hash: local_tx_hash,
                                message: Some(
                                    "RPC response was ambiguous; transaction recovered by local hash"
                                        .to_string(),
                                ),
                            });
                        }

                        Err(error).context(format!(
                            "EVM broadcast failed and transaction {} was not found",
                            local_tx_hash
                        ))
                    }
                }
            }
            #[allow(unreachable_patterns)]
            _ => Err(anyhow!("EvmClient cannot broadcast non-EVM transactions")),
        }
    }

    async fn get_current_block(&self) -> Result<ChainBlockInfo> {
        // Single RPC call using "latest" tag to avoid race condition:
        // Previously we called eth_blockNumber then eth_getBlockByNumber(N),
        // but load balancers could route the second call to a node that
        // hadn't synced block N yet, returning null and triggering
        // chain health degradation → /ready 503 → Sentry noise.
        let params = serde_json::json!(["latest", false]);
        let block: EvmBlock = self.rpc_call("eth_getBlockByNumber", params).await?;

        let block_number = Self::parse_hex_u64(&block.number)?;
        let timestamp = Self::parse_hex_i64(&block.timestamp)?;

        Ok(ChainBlockInfo {
            number: block_number,
            timestamp,
        })
    }

    async fn get_transaction_info(&self, tx_hash: &str) -> Result<Option<ChainTransactionInfo>> {
        let params = serde_json::json!([tx_hash]);
        let receipt: EvmReceipt = match self
            .rpc_call_optional("eth_getTransactionReceipt", params)
            .await?
        {
            Some(r) => r,
            None => return Ok(None),
        };

        let block_number = Self::parse_hex_i64(&receipt.block_number)?;
        let gas_used = Self::parse_hex_i64(&receipt.gas_used)?;
        let mut gas_price = receipt
            .effective_gas_price
            .as_deref()
            .and_then(|p| Self::parse_hex_i64(p).ok())
            .unwrap_or(0);

        // Fallback: if effectiveGasPrice is missing (non-EIP-1559 chains),
        // fetch the original transaction's gasPrice field.
        if gas_price == 0 {
            let tx_params = serde_json::json!([tx_hash]);
            if let Ok(Some(tx)) = self
                .rpc_call_optional::<EvmTransaction>("eth_getTransactionByHash", tx_params)
                .await
            {
                gas_price = tx
                    .gas_price
                    .as_deref()
                    .and_then(|p| Self::parse_hex_i64(p).ok())
                    .unwrap_or(0);
            }
        }

        let fee_burned = gas_used.saturating_mul(gas_price);

        let success = receipt.status == "0x1";

        Ok(Some(ChainTransactionInfo {
            tx_hash: receipt.transaction_hash,
            block_number,
            success,
            result: if success {
                None
            } else {
                Some("REVERT".to_string())
            },
            fee_burned,
            revert_message: if success {
                None
            } else {
                Some("Transaction reverted".to_string())
            },
        }))
    }

    async fn get_transaction_by_id(&self, tx_hash: &str) -> Result<Option<ChainSignedTx>> {
        let params = serde_json::json!([tx_hash]);
        let tx: EvmTransaction = match self
            .rpc_call_optional("eth_getTransactionByHash", params)
            .await?
        {
            Some(t) => t,
            None => return Ok(None),
        };

        Ok(Some(ChainSignedTx::Evm(EvmSignedTx {
            tx_hash: tx.hash,
            raw_tx_hex: String::new(), // Not available from getTransactionByHash
        })))
    }

    fn rpc_status(&self) -> Option<RpcStatus> {
        let idx = self.active.load(std::sync::atomic::Ordering::Relaxed);
        Some(RpcStatus {
            provider: self.endpoints[idx].provider_name(),
            is_fallback: idx > 0,
            endpoint_count: self.endpoints.len(),
            active_endpoint: self.endpoints[idx].masked_endpoint(),
        })
    }

    async fn validate_payment_tx(
        &self,
        tx_hash: &str,
        expected_pay_address: &str,
        token_contract: &str,
    ) -> bool {
        // 1. Fetch transaction
        let params = serde_json::json!([tx_hash]);
        let tx: EvmTransaction = match self
            .rpc_call_optional("eth_getTransactionByHash", params)
            .await
        {
            Ok(Some(t)) => t,
            _ => return false,
        };

        // 2. Verify tx.to matches USDT contract
        let tx_to = match &tx.to {
            Some(to) => to,
            None => return false, // Contract creation tx
        };
        if !tx_to.eq_ignore_ascii_case(token_contract) {
            return false;
        }

        // 3. Parse calldata: 4-byte selector + 32-byte padded address
        // ERC20 transfer(address,uint256) selector = 0xa9059cbb
        let input = tx.input.strip_prefix("0x").unwrap_or(&tx.input);
        if input.len() < 136 || !input[..8].eq_ignore_ascii_case("a9059cbb") {
            return false;
        }

        // Extract to address: bytes [8+24..8+64] = 20-byte address (skip 12-byte zero padding)
        let to_hex = &input[8 + 24..8 + 64];
        let decoded_to = format!("0x{}", to_hex);

        // 4. Compare with expected pay address
        decoded_to.eq_ignore_ascii_case(expected_pay_address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        matchers::{body_partial_json, method},
        Mock, MockServer, ResponseTemplate,
    };

    const LOCAL_TX_HASH: &str =
        "0x8ba3b8d37ee2c628dbeb7bb13d58d9e0ddc1ed26d09e0f2c025f7373044157bc";

    fn signed_transaction() -> ChainSignedTx {
        ChainSignedTx::Evm(EvmSignedTx {
            tx_hash: LOCAL_TX_HASH.to_string(),
            raw_tx_hex:
                "0xf86c01843b9aca0082520894000000000000000000000000000000000000000080801ba0"
                    .to_string(),
        })
    }

    async fn mount_broadcast_error(server: &MockServer) {
        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({
                "method": "eth_sendRawTransaction"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "error": {
                    "code": -32003,
                    "message": "insufficient funds for gas * price + value"
                }
            })))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn broadcast_recovers_rpc_error_when_local_hash_is_known() {
        let primary = MockServer::start().await;
        let fallback = MockServer::start().await;
        mount_broadcast_error(&primary).await;
        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({
                "method": "eth_getTransactionByHash"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": null
            })))
            .mount(&primary)
            .await;
        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({
                "method": "eth_getTransactionByHash"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": { "hash": LOCAL_TX_HASH }
            })))
            .mount(&fallback)
            .await;

        let client = EvmClient::new(vec![primary.uri(), fallback.uri()], 1);
        let result = ChainClient::broadcast(&client, &signed_transaction())
            .await
            .expect("known transaction hash should recover ambiguous response");

        assert!(result.success);
        assert_eq!(result.tx_hash, LOCAL_TX_HASH);
        assert!(result.message.is_some());
    }

    #[tokio::test]
    async fn broadcast_preserves_error_when_local_hash_is_not_found() {
        let server = MockServer::start().await;
        mount_broadcast_error(&server).await;
        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({
                "method": "eth_getTransactionByHash"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": null
            })))
            .mount(&server)
            .await;

        let client = EvmClient::new(vec![server.uri()], 1);
        let error = ChainClient::broadcast(&client, &signed_transaction())
            .await
            .expect_err("unknown transaction hash must remain a broadcast failure");
        let message = error.to_string();

        assert!(message.contains(LOCAL_TX_HASH));
        assert!(message.contains("not found"));
    }

    #[tokio::test]
    async fn unavailable_endpoint_does_not_prove_transaction_absence() {
        let available = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({
                "method": "eth_getTransactionByHash"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": null
            })))
            .mount(&available)
            .await;

        let client = EvmClient::new(vec![available.uri(), "http://127.0.0.1:1".to_string()], 1);
        let result = client
            .transaction_known_on_any_endpoint(LOCAL_TX_HASH)
            .await;

        assert!(result.is_err());
    }
}
