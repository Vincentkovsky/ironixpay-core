//! Solana blockchain client (JSON-RPC).
//!
//! Provides JSON-RPC interface to Solana for SPL Token operations (USDT/USDC).
//! Uses reqwest for HTTP calls (same pattern as EvmClient) with failover support.
//!
//! # Key differences from EVM/TRON
//! - SPL Token balances require ATA (Associated Token Account) address derivation
//! - Solana has NO mempool — `sendTransaction` may be silently dropped
//! - Commitment levels replace block confirmations (processed/confirmed/finalized)
//! - Ed25519 signatures instead of secp256k1
//!
//! # Supported RPC Methods
//! - `getLatestBlockhash` (for transaction signing)
//! - `getBalance` (SOL balance)
//! - `getTokenAccountBalance` (SPL Token balance via ATA)
//! - `getSlot` + `getBlockTime` (block info)
//! - `getSignaturesForAddress` (for indexer)
//! - `getTransaction` (transaction details)
//! - `sendTransaction` (broadcast)
//! - `isBlockhashValid` (expiration check)

pub mod helius_manager;
pub mod indexer;
pub mod noop_scanner;
pub mod signing;
pub mod sweep_executor;
pub mod types;

use alloy_primitives::U256;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::Client;
use reqwest_middleware::ClientBuilder;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::debug;

use crate::entity::network::Network;
use crate::services::chain::traits::ChainClient;
use crate::services::chain::types::*;

use types::*;

// ─── Well-Known Program IDs ─────────────────────────────────────────────────

/// SPL Token program ID
pub const SPL_TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

/// SPL Associated Token Account program ID
pub const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

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

// ─── RPC Endpoint ───────────────────────────────────────────────────────────

struct RpcEndpoint {
    url: String,
}

impl RpcEndpoint {
    fn new(url: String) -> Self {
        Self { url }
    }

    /// Extract provider name from URL hostname.
    fn provider_name(&self) -> String {
        if self.url.contains("helius") {
            "Helius".to_string()
        } else if self.url.contains("quicknode") {
            "QuickNode".to_string()
        } else if self.url.contains("alchemy") {
            "Alchemy".to_string()
        } else if self.url.contains("solana.com") {
            "Solana Public".to_string()
        } else {
            self.url
                .split("//")
                .nth(1)
                .and_then(|s| s.split('/').next())
                .unwrap_or("Unknown")
                .to_string()
        }
    }

    /// Build a masked endpoint for admin display.
    fn masked_endpoint(&self) -> String {
        let provider = self.provider_name();
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

// ─── Failover Constants ─────────────────────────────────────────────────────

/// Consecutive failures before triggering failover.
const FAILOVER_THRESHOLD: u32 = 2;

/// Seconds to wait before probing primary again after failover.
const RECOVERY_PROBE_INTERVAL_SECS: i64 = 60;

// ─── SolanaClient ───────────────────────────────────────────────────────────

/// Solana JSON-RPC client with failover support.
///
/// Follows the same architecture as `EvmClient`:
/// - Multiple RPC endpoints with automatic failover
/// - reqwest with retry middleware (3 HTTP-level retries per call)
/// - Atomic counters for thread-safe failover logic
pub struct SolanaClient {
    http: ClientWithMiddleware,
    probe_http: Client,
    endpoints: Vec<RpcEndpoint>,
    /// Index of the currently active endpoint (0 = primary)
    active: std::sync::atomic::AtomicUsize,
    /// Consecutive failure count for the active endpoint
    fail_count: std::sync::atomic::AtomicU32,
    /// Unix timestamp when failover was triggered
    failover_at: std::sync::atomic::AtomicI64,
    #[allow(dead_code)]
    network: Network,
}

impl SolanaClient {
    pub fn new(rpc_urls: Vec<String>, network: Network) -> Self {
        assert!(!rpc_urls.is_empty(), "At least one Solana RPC URL required");

        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        let base_client = Client::builder()
            .timeout(std::time::Duration::from_secs(15))
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
            network,
        }
    }

    // ─── Failover Logic (identical to EvmClient) ────────────────────────────

    fn active_url(&self) -> &str {
        let idx = self.active.load(std::sync::atomic::Ordering::Relaxed);
        &self.endpoints[idx].url
    }

    fn try_recover_primary(&self) {
        use std::sync::atomic::Ordering;
        let current = self.active.load(Ordering::Relaxed);
        if current == 0 {
            return;
        }
        let failover_time = self.failover_at.load(Ordering::Relaxed);
        let now = chrono::Utc::now().timestamp();
        if now - failover_time < RECOVERY_PROBE_INTERVAL_SECS {
            return;
        }
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
                "Solana RPC RECOVERY: switching back to primary"
            );
        }
    }

    fn on_success(&self) {
        if self.fail_count.load(std::sync::atomic::Ordering::Relaxed) > 0 {
            self.fail_count
                .store(0, std::sync::atomic::Ordering::Relaxed);
        }
        self.try_recover_primary();
    }

    fn on_failure(&self) {
        use std::sync::atomic::Ordering;
        let fails = self.fail_count.fetch_add(1, Ordering::SeqCst) + 1;
        if fails >= FAILOVER_THRESHOLD && self.endpoints.len() > 1 {
            let current = self.active.load(Ordering::Acquire);
            let next = current + 1;
            if next < self.endpoints.len() {
                if self
                    .active
                    .compare_exchange(current, next, Ordering::SeqCst, Ordering::Relaxed)
                    .is_ok()
                {
                    self.fail_count.store(0, Ordering::SeqCst);
                    self.failover_at
                        .store(chrono::Utc::now().timestamp(), Ordering::Relaxed);
                    tracing::warn!(
                        from = %self.endpoints[current].masked_endpoint(),
                        to = %self.endpoints[next].masked_endpoint(),
                        "Solana RPC FAILOVER: switched to backup"
                    );
                }
            } else if current > 0 {
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
                            from = %self.endpoints[current].masked_endpoint(),
                            to = %self.endpoints[0].masked_endpoint(),
                            "Solana RPC RECOVERY PROBE: retrying primary"
                        );
                    }
                }
            }
        }
    }

    // ─── JSON-RPC Transport ─────────────────────────────────────────────────

    /// Send a JSON-RPC request. Errors on RPC error or null result.
    async fn rpc_call<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T> {
        self.rpc_call_optional(method, params)
            .await?
            .ok_or_else(|| anyhow!("Solana RPC returned null for {}", method))
    }

    /// Send a JSON-RPC request, allowing null result (returns Ok(None)).
    async fn rpc_call_optional<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<Option<T>> {
        self.rpc_call_inner(method, params, false).await
    }

    /// Like `rpc_call_optional`, but tolerates specific RPC error codes.
    ///
    /// When `tolerate_not_found` is true, error code -32602 ("could not find account")
    /// returns `Ok(None)` WITHOUT triggering failover. This is critical for
    /// `getTokenAccountBalance` on addresses that haven't received tokens yet.
    async fn rpc_call_optional_tolerant<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<Option<T>> {
        self.rpc_call_inner(method, params, true).await
    }

    /// Core RPC transport. Shared by `rpc_call_optional` and `rpc_call_optional_tolerant`.
    async fn rpc_call_inner<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: serde_json::Value,
        tolerate_not_found: bool,
    ) -> Result<Option<T>> {
        let req = JsonRpcRequest::new(method, params);
        debug!(method = %req.method, "Solana RPC call");

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
            // -32602: "Invalid param: could not find account"
            // This is a normal business case (ATA doesn't exist yet), NOT an RPC failure.
            if tolerate_not_found && err.code == -32602 && err.message.contains("could not find") {
                self.on_success(); // RPC is healthy, just account missing
                return Ok(None);
            }
            self.on_failure();
            return Err(anyhow!("Solana RPC error {}: {}", err.code, err.message));
        }

        self.on_success();
        Ok(resp.result)
    }

    // ─── Solana-Specific RPC Methods ────────────────────────────────────────

    /// Get recent blockhash for transaction signing.
    ///
    /// CRITICAL: Uses `confirmed` commitment — NEVER use `processed`!
    /// `processed` may return a blockhash from a minority fork,
    /// causing validators to reject the signed tx with `BlockhashNotFound`.
    pub async fn get_latest_blockhash(&self) -> Result<(String, u64)> {
        let resp: RpcResponse<BlockhashResponse> = self
            .rpc_call(
                "getLatestBlockhash",
                serde_json::json!([{"commitment": "confirmed"}]),
            )
            .await?;
        Ok((resp.value.blockhash, resp.value.last_valid_block_height))
    }

    /// Current confirmed block height, used to prove blockhash expiry.
    pub async fn get_block_height(&self) -> Result<u64> {
        self.rpc_call(
            "getBlockHeight",
            serde_json::json!([{"commitment": "confirmed"}]),
        )
        .await
    }

    /// Query every configured endpoint and return the minimum confirmed height.
    /// Expiry is safe only when even the slowest healthy provider is past the
    /// transaction's last valid block height.
    pub async fn get_block_height_across_endpoints(&self) -> Result<u64> {
        let mut minimum = None;
        for endpoint in &self.endpoints {
            let request = JsonRpcRequest::new(
                "getBlockHeight",
                serde_json::json!([{"commitment": "confirmed"}]),
            );
            let response = self
                .probe_http
                .post(&endpoint.url)
                .json(&request)
                .send()
                .await
                .map_err(|error| {
                    anyhow!(
                        "Failed to query Solana block height on {}: {}",
                        endpoint.provider_name(),
                        error
                    )
                })?;
            let body: JsonRpcResponse<u64> = response.json().await?;
            if let Some(error) = body.error {
                return Err(anyhow!(
                    "Solana RPC error {} from {}: {}",
                    error.code,
                    endpoint.provider_name(),
                    error.message
                ));
            }
            let height = body
                .result
                .ok_or_else(|| anyhow!("Solana RPC returned null getBlockHeight result"))?;
            minimum = Some(minimum.map_or(height, |current: u64| current.min(height)));
        }
        minimum.ok_or_else(|| anyhow!("No Solana RPC endpoints configured"))
    }

    /// Get SOL balance in lamports.
    pub async fn get_sol_balance(&self, address: &str) -> Result<u64> {
        let resp: RpcResponse<u64> = self
            .rpc_call(
                "getBalance",
                serde_json::json!([address, {"commitment": "confirmed"}]),
            )
            .await?;
        Ok(resp.value)
    }

    /// Get SPL Token balance for a specific ATA address.
    ///
    /// Callers should first derive the ATA address using `derive_ata_address`.
    pub async fn get_token_account_balance(&self, ata_address: &str) -> Result<Option<u64>> {
        // Use tolerant variant: Solana returns RPC error -32602 (not null)
        // when the ATA doesn't exist. Without tolerance, this would trigger
        // false failover when scanning new addresses.
        let resp: Option<RpcResponse<TokenAmount>> = self
            .rpc_call_optional_tolerant(
                "getTokenAccountBalance",
                serde_json::json!([ata_address, {"commitment": "confirmed"}]),
            )
            .await?;
        match resp {
            Some(r) => {
                let amount = r
                    .value
                    .amount
                    .parse::<u64>()
                    .map_err(|e| anyhow!("Invalid token amount: {}", e))?;
                Ok(Some(amount))
            }
            None => Ok(None), // ATA doesn't exist
        }
    }

    /// Get SPL Token balance for a given owner + mint.
    ///
    /// Derives the ATA address internally, returns balance in microunits (e.g. 6 decimals).
    /// Returns 0 if the ATA does not exist.
    pub async fn get_spl_token_balance(&self, owner: &str, mint: &str) -> Result<i64> {
        let spl_token_program = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
        let ata = derive_ata_address(owner, mint, spl_token_program)?;
        match self.get_token_account_balance(&ata).await? {
            Some(amount) => Ok(amount as i64),
            None => Ok(0),
        }
    }

    /// Get current slot (equivalent to block number).
    pub async fn get_slot(&self) -> Result<u64> {
        self.rpc_call("getSlot", serde_json::json!([{"commitment": "confirmed"}]))
            .await
    }

    /// Get block time (Unix timestamp) for a given slot.
    pub async fn get_block_time(&self, slot: u64) -> Result<Option<i64>> {
        self.rpc_call_optional("getBlockTime", serde_json::json!([slot]))
            .await
    }

    /// Get transaction signatures for an address (for indexer scanning).
    ///
    /// Returns signatures in reverse chronological order.
    /// - `before`: pagination backward (pass the last/oldest signature from previous page)
    /// - `until`: stop-at cursor (only return signatures **newer** than this signature)
    pub async fn get_signatures_for_address(
        &self,
        address: &str,
        before: Option<&str>,
        until: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SignatureInfo>> {
        let mut config = serde_json::json!({
            "limit": limit,
            "commitment": "confirmed",
        });
        if let Some(before_sig) = before {
            config["before"] = serde_json::json!(before_sig);
        }
        if let Some(until_sig) = until {
            config["until"] = serde_json::json!(until_sig);
        }
        self.rpc_call(
            "getSignaturesForAddress",
            serde_json::json!([address, config]),
        )
        .await
    }

    /// Get transaction details by signature.
    ///
    /// Uses `jsonParsed` encoding for structured account key info.
    pub async fn get_transaction(&self, signature: &str) -> Result<Option<TransactionResponse>> {
        self.rpc_call_optional(
            "getTransaction",
            serde_json::json!([
                signature,
                {
                    "encoding": "jsonParsed",
                    "commitment": "confirmed",
                    "maxSupportedTransactionVersion": 0,
                }
            ]),
        )
        .await
    }

    /// Send a signed transaction (Base64-encoded).
    ///
    /// WARNING: Solana has NO mempool. This call may be silently dropped
    /// by the current Leader. For production use, `send_with_retry_loop`
    /// (Phase 2d) should be used instead.
    pub async fn send_transaction(&self, serialized_tx_base64: &str) -> Result<String> {
        self.rpc_call(
            "sendTransaction",
            serde_json::json!([
                serialized_tx_base64,
                {
                    "encoding": "base64",
                    "skipPreflight": false,
                    "preflightCommitment": "confirmed",
                }
            ]),
        )
        .await
    }

    /// Check if a blockhash is still valid.
    pub async fn is_blockhash_valid(&self, blockhash: &str) -> Result<bool> {
        let resp: RpcResponse<bool> = self
            .rpc_call(
                "isBlockhashValid",
                serde_json::json!([blockhash, {"commitment": "confirmed"}]),
            )
            .await?;
        Ok(resp.value)
    }

    /// Get signature statuses.
    pub async fn get_signature_statuses(
        &self,
        signatures: &[&str],
    ) -> Result<Vec<Option<SignatureStatus>>> {
        let resp: RpcResponse<Vec<Option<SignatureStatus>>> = self
            .rpc_call(
                "getSignatureStatuses",
                serde_json::json!([signatures, {"searchTransactionHistory": true}]),
            )
            .await?;
        Ok(resp.value)
    }

    /// Check every configured RPC, including historical transaction storage.
    /// `Ok(false)` means every endpoint answered successfully and none knew the signature.
    pub async fn signature_known_on_any_endpoint(&self, signature: &str) -> Result<bool> {
        for endpoint in &self.endpoints {
            let request = JsonRpcRequest::new(
                "getSignatureStatuses",
                serde_json::json!([[signature], {"searchTransactionHistory": true}]),
            );
            let response = self
                .probe_http
                .post(&endpoint.url)
                .json(&request)
                .send()
                .await
                .map_err(|error| {
                    anyhow!(
                        "Failed to query Solana signature on {}: {}",
                        endpoint.provider_name(),
                        error
                    )
                })?;
            let body: JsonRpcResponse<RpcResponse<Vec<Option<SignatureStatus>>>> =
                response.json().await?;
            if let Some(error) = body.error {
                return Err(anyhow!(
                    "Solana RPC error {} from {}: {}",
                    error.code,
                    endpoint.provider_name(),
                    error.message
                ));
            }
            let statuses = body
                .result
                .ok_or_else(|| anyhow!("Solana RPC returned null getSignatureStatuses result"))?;
            if statuses.value.first().is_some_and(Option::is_some) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Broadcast a signed transaction with aggressive retry.
    ///
    /// Solana has **no mempool** — `sendTransaction` may be silently dropped
    /// if the current Leader doesn't receive it. This method:
    /// 1. Sends every `RETRY_INTERVAL` until confirmed or expired
    /// 2. Checks `getSignatureStatuses` to detect confirmation
    /// 3. After `BLOCKHASH_CHECK_AFTER` attempts, checks blockhash validity
    ///
    /// # Arguments
    /// * `serialized_tx_base64` - Base64-encoded signed transaction
    /// * `signature` - Expected transaction signature (Base58, for status checking)
    /// * `blockhash` - The blockhash used in signing (for expiration checking)
    /// * `max_retries` - Maximum send attempts (default: 10)
    ///
    /// # Returns
    /// * `Ok(signature)` if confirmed
    /// * `Err` if blockhash expired or max retries exceeded
    pub async fn send_with_retry_loop(
        &self,
        serialized_tx_base64: &str,
        signature: &str,
        blockhash: &str,
        max_retries: usize,
    ) -> Result<String> {
        use tracing::{info, warn};

        const RETRY_INTERVAL_MS: u64 = 2000;
        const BLOCKHASH_CHECK_AFTER: usize = 5;

        for attempt in 1..=max_retries {
            // Send (or re-send) the transaction.
            // Errors here are usually transient (Leader unreachable) — log and retry.
            match self.send_transaction(serialized_tx_base64).await {
                Ok(_) => {
                    debug!(attempt, signature, "sendTransaction accepted");
                }
                Err(e) => {
                    warn!(attempt, signature, error = %e, "sendTransaction failed, will retry");
                }
            }

            // Wait before checking status
            tokio::time::sleep(std::time::Duration::from_millis(RETRY_INTERVAL_MS)).await;

            // Check if the transaction has been confirmed
            match self.get_signature_statuses(&[signature]).await {
                Ok(statuses) => {
                    if let Some(Some(status)) = statuses.first() {
                        // Check for on-chain error
                        if let Some(err) = &status.err {
                            return Err(anyhow!(
                                "Transaction {} failed on-chain: {}",
                                signature,
                                err
                            ));
                        }

                        // Check confirmation level
                        let confirmed = status
                            .confirmation_status
                            .as_deref()
                            .is_some_and(|s| s == "confirmed" || s == "finalized");

                        if confirmed {
                            info!(
                                attempt,
                                signature,
                                status = ?status.confirmation_status,
                                "Transaction confirmed"
                            );
                            return Ok(signature.to_string());
                        }
                    }
                }
                Err(e) => {
                    warn!(attempt, error = %e, "getSignatureStatuses failed");
                }
            }

            // After several attempts, check if blockhash is still valid.
            // If expired, the transaction can never be included — abort early.
            if attempt >= BLOCKHASH_CHECK_AFTER {
                match self.is_blockhash_valid(blockhash).await {
                    Ok(false) => {
                        return Err(anyhow!(
                            "Blockhash expired after {} attempts for tx {}. \
                             Caller must re-build and re-sign with a fresh blockhash.",
                            attempt,
                            signature,
                        ));
                    }
                    Ok(true) => {
                        debug!(attempt, "Blockhash still valid, continuing retry");
                    }
                    Err(e) => {
                        warn!(attempt, error = %e, "isBlockhashValid check failed");
                    }
                }
            }
        }

        Err(anyhow!(
            "Transaction {} not confirmed after {} attempts",
            signature,
            max_retries,
        ))
    }

    /// Check if an account (ATA) exists on-chain.
    ///
    /// Used to determine if CreateAssociatedTokenAccount instruction is needed
    /// before a transfer.
    pub async fn ata_exists(&self, ata_address: &str) -> Result<bool> {
        // getAccountInfo returns null for non-existent accounts
        let result: Option<serde_json::Value> = self
            .rpc_call_optional_tolerant(
                "getAccountInfo",
                serde_json::json!([ata_address, {"encoding": "base64", "commitment": "confirmed"}]),
            )
            .await?;

        match result {
            Some(resp) => {
                // Response has { context, value } — value is null if account doesn't exist
                let value = resp.get("value");
                Ok(value.is_some() && !value.unwrap().is_null())
            }
            None => Ok(false),
        }
    }

    /// Get recent priority fee estimates for ComputeBudget pricing.
    ///
    /// Calls `getRecentPrioritizationFees` and returns the median fee
    /// from the last few slots, with a minimum floor.
    pub async fn get_recent_priority_fee(&self) -> Result<u64> {
        let fees: Vec<PrioritizationFee> = self
            .rpc_call("getRecentPrioritizationFees", serde_json::json!([]))
            .await?;

        if fees.is_empty() {
            // No data — use a conservative default (1000 micro-lamports)
            return Ok(1000);
        }

        // Take the median of non-zero fees
        let mut non_zero: Vec<u64> = fees
            .iter()
            .map(|f| f.prioritization_fee)
            .filter(|&f| f > 0)
            .collect();

        if non_zero.is_empty() {
            return Ok(1000);
        }

        non_zero.sort_unstable();
        let median = non_zero[non_zero.len() / 2];

        // Floor at 1000 micro-lamports, cap at 1_000_000 (safety)
        Ok(median.max(1000).min(1_000_000))
    }

    /// Get priority fee with logged fallback.
    ///
    /// Wraps `get_recent_priority_fee()` with a warning log on failure
    /// and a safe default of 1000 micro-lamports.
    async fn get_priority_fee_or_default(&self) -> u64 {
        match self.get_recent_priority_fee().await {
            Ok(fee) => fee,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Failed to fetch priority fee, using default 1000 micro-lamports"
                );
                1000
            }
        }
    }

    /// Compile instructions into a serialized Solana Message with a fresh blockhash.
    ///
    /// Returns message bytes, signer count, blockhash, and its last valid height.
    async fn finalize_message(
        &self,
        instructions: &[solana_sdk::instruction::Instruction],
        fee_payer: &solana_sdk::pubkey::Pubkey,
    ) -> Result<(Vec<u8>, u8, String, u64)> {
        use solana_sdk::message::Message;

        let (blockhash, last_valid_block_height) = self.get_latest_blockhash().await?;
        let blockhash_parsed = solana_sdk::hash::Hash::from_str(&blockhash)
            .map_err(|e| anyhow!("Invalid blockhash: {}", e))?;

        let mut message = Message::new(instructions, Some(fee_payer));
        message.recent_blockhash = blockhash_parsed;

        let num_required_signatures = message.header.num_required_signatures;
        let message_bytes = message.serialize();

        Ok((
            message_bytes,
            num_required_signatures,
            blockhash,
            last_valid_block_height,
        ))
    }

    /// Build SPL Token sweep with fee payer delegation (dual-signer).
    ///
    /// Solana uniquely supports a fee payer that is different from the transaction
    /// authority. This allows sweeping tokens from deposit addresses WITHOUT
    /// injecting SOL for gas — the treasury pays all fees.
    ///
    /// # Signers (order matters!)
    /// 1. `fee_payer` (treasury) — pays gas + priority fee, first in signer list
    /// 2. `from` (deposit address) — authority for TransferChecked
    ///
    /// # Instructions
    /// - ComputeBudget::SetComputeUnitLimit + SetComputeUnitPrice
    /// - CreateAssociatedTokenAccountIdempotent (always included, no-op if exists)
    /// - SPL Token TransferChecked (from ATA → to ATA)
    /// - [optional] SPL Token CloseAccount (reclaim rent → fee_payer)
    pub async fn build_spl_sweep(
        &self,
        from: &str,
        to: &str,
        mint: &str,
        amount: u64,
        decimals: u8,
        fee_payer: &str,
        token_program_id: &str,
        close_ata: bool,
    ) -> Result<SolanaUnsignedTx> {
        // SDK-chain pubkey (v2) — used for Message::new()
        let fee_payer_pubkey = solana_pubkey(fee_payer)?;

        // SPL-chain pubkeys (v3) — used for spl-token / spl-ata instruction builders
        let from_pk = spl_pubkey(from)?;
        let to_pk = spl_pubkey(to)?;
        let mint_pk = spl_pubkey(mint)?;
        let fee_payer_pk = spl_pubkey(fee_payer)?;
        let token_prog = spl_pubkey(token_program_id)?;

        // Derive ATAs using the correct token program (SPL Token vs Token-2022)
        let from_ata_str = derive_ata_address(from, mint, token_program_id)?;
        let to_ata_str = derive_ata_address(to, mint, token_program_id)?;
        let from_ata_pk = spl_pubkey(&from_ata_str)?;
        let to_ata_pk = spl_pubkey(&to_ata_str)?;

        let mut instructions = Vec::with_capacity(5);

        // 1. ComputeBudget
        let priority_fee = self.get_priority_fee_or_default().await;
        let (limit_ix, price_ix) = compute_budget_instructions(100_000, priority_fee)?;
        instructions.push(limit_ix);
        instructions.push(price_ix);

        // 2. CreateATA (idempotent — no-op if already exists, avoids TOCTOU race)
        instructions.push(convert_spl_ix(
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &fee_payer_pk, // payer (treasury)
                &to_pk,        // wallet owner
                &mint_pk,
                &token_prog,
            ),
        ));

        // 3. TransferChecked: from_ata → to_ata, authority = from_pubkey
        instructions.push(convert_spl_ix(spl_token::instruction::transfer_checked(
            &token_prog,
            &from_ata_pk,
            &mint_pk,
            &to_ata_pk,
            &from_pk, // authority (deposit address owner)
            &[],
            amount,
            decimals,
        )?));

        // 4. CloseAccount for source ATA (reclaim ~0.002 SOL rent → fee_payer)
        //    Only for one-time addresses. Reusable deposit addresses must NOT close ATA.
        if close_ata {
            instructions.push(convert_spl_ix(spl_token::instruction::close_account(
                &token_prog,
                &from_ata_pk,  // account to close
                &fee_payer_pk, // rent destination (treasury)
                &from_pk,      // authority
                &[],
            )?));
        }

        // Finalize message
        let (message_bytes, num_required_signatures, blockhash, last_valid_block_height) = self
            .finalize_message(&instructions, &fee_payer_pubkey)
            .await?;

        Ok(SolanaUnsignedTx {
            message_bytes,
            recent_blockhash: blockhash,
            last_valid_block_height,
            num_required_signatures,
            // Fee payer MUST be first (Solana wire format requirement)
            signer_pubkeys: vec![fee_payer.to_string(), from.to_string()],
        })
    }

    /// Build an SPL Token transfer transaction (single-signer).
    ///
    /// Used for **payouts** (treasury → merchant). Unlike `build_spl_sweep`
    /// which requires dual signatures (fee_payer ≠ authority), here the
    /// `from` address is both the fee payer AND the token authority,
    /// so only **one Ed25519 signature** is needed.
    ///
    /// Instructions:
    /// 1. ComputeBudget (priority fee)
    /// 2. CreateATA idempotent (ensure recipient has an ATA)
    /// 3. TransferChecked: from_ata → to_ata
    pub async fn build_spl_transfer(
        &self,
        from: &str,
        to: &str,
        mint: &str,
        amount: u64,
        decimals: u8,
        token_program_id: &str,
    ) -> Result<SolanaUnsignedTx> {
        // `from` is both fee_payer and authority (single signer)
        let fee_payer_pubkey = solana_pubkey(from)?;

        let from_pk = spl_pubkey(from)?;
        let to_pk = spl_pubkey(to)?;
        let mint_pk = spl_pubkey(mint)?;
        let token_prog = spl_pubkey(token_program_id)?;

        // Derive ATAs
        let from_ata_str = derive_ata_address(from, mint, token_program_id)?;
        let to_ata_str = derive_ata_address(to, mint, token_program_id)?;
        let from_ata_pk = spl_pubkey(&from_ata_str)?;
        let to_ata_pk = spl_pubkey(&to_ata_str)?;

        let mut instructions = Vec::with_capacity(4);

        // 1. ComputeBudget
        let priority_fee = self.get_priority_fee_or_default().await;
        let (limit_ix, price_ix) = compute_budget_instructions(100_000, priority_fee)?;
        instructions.push(limit_ix);
        instructions.push(price_ix);

        // 2. CreateATA idempotent for recipient (no-op if exists)
        instructions.push(convert_spl_ix(
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &from_pk, // payer (treasury)
                &to_pk,   // wallet owner (merchant)
                &mint_pk,
                &token_prog,
            ),
        ));

        // 3. TransferChecked: from_ata → to_ata, authority = from (treasury)
        instructions.push(convert_spl_ix(spl_token::instruction::transfer_checked(
            &token_prog,
            &from_ata_pk,
            &mint_pk,
            &to_ata_pk,
            &from_pk, // authority = treasury (same as fee payer)
            &[],
            amount,
            decimals,
        )?));

        // Finalize message
        let (message_bytes, num_required_signatures, blockhash, last_valid_block_height) = self
            .finalize_message(&instructions, &fee_payer_pubkey)
            .await?;

        Ok(SolanaUnsignedTx {
            message_bytes,
            recent_blockhash: blockhash,
            last_valid_block_height,
            num_required_signatures,
            // Single signer: treasury only
            signer_pubkeys: vec![from.to_string()],
        })
    }

    /// Broadcast with retry loop (Solana-specific).
    ///
    /// Callers that have the blockhash should prefer this over `ChainClient::broadcast()`
    /// for reliable delivery on Solana's no-mempool network.
    pub async fn broadcast_solana(
        &self,
        solana_tx: &SolanaSignedTx,
        blockhash: &str,
    ) -> Result<ChainBroadcastResult> {
        let signature = self
            .send_with_retry_loop(
                &solana_tx.serialized_tx,
                &solana_tx.signature,
                blockhash,
                10, // Default max retries
            )
            .await?;

        Ok(ChainBroadcastResult {
            success: true,
            tx_hash: signature,
            message: None,
        })
    }
}

// SignatureStatus is defined in types.rs with other RPC response types.

// ─── ATA (Associated Token Account) Derivation ─────────────────────────────

/// Derive the Associated Token Account (ATA) address for a given owner and mint.
///
/// ATA = PDA(owner_pubkey, token_program_id, mint_pubkey) under the
/// Associated Token Account program.
///
/// This is a pure local computation (SHA-256 + Ed25519 curve check),
/// taking microseconds. No RPC calls needed.
///
/// # Arguments
/// * `owner` - Wallet address (Base58)
/// * `mint` - Token mint address (Base58)
/// * `token_program_id` - SPL Token program ID (Base58).
///   Use `SPL_TOKEN_PROGRAM_ID` for standard SPL tokens (USDT, USDC).
pub fn derive_ata_address(owner: &str, mint: &str, token_program_id: &str) -> Result<String> {
    let owner_bytes = bs58_decode_32(owner)?;
    let mint_bytes = bs58_decode_32(mint)?;
    let token_program_bytes = bs58_decode_32(token_program_id)?;
    let ata_program_bytes = bs58_decode_32(ASSOCIATED_TOKEN_PROGRAM_ID)?;

    let seeds: &[&[u8]] = &[&owner_bytes, &token_program_bytes, &mint_bytes];

    let (address_bytes, _bump) = find_program_address(seeds, &ata_program_bytes)
        .ok_or_else(|| anyhow!("Failed to find PDA for ATA derivation"))?;

    Ok(bs58::encode(&address_bytes).into_string())
}

/// Solana's `find_program_address` — derives a Program Derived Address (PDA).
///
/// Iterates bump seeds from 255 down to 0, computing:
///   SHA-256(seed_0 || seed_1 || ... || [bump] || program_id || "ProgramDerivedAddress")
/// and returns the first result that is NOT on the Ed25519 curve.
fn find_program_address(seeds: &[&[u8]], program_id: &[u8; 32]) -> Option<([u8; 32], u8)> {
    use sha2::{Digest, Sha256};

    for bump in (0..=255u8).rev() {
        let mut hasher = Sha256::new();
        for seed in seeds {
            hasher.update(seed);
        }
        hasher.update([bump]);
        hasher.update(program_id);
        hasher.update(b"ProgramDerivedAddress");

        let hash = hasher.finalize();
        let hash_bytes: [u8; 32] = hash.into();

        // A valid PDA must NOT be on the Ed25519 curve.
        // If decompression fails → not on curve → valid PDA.
        if !is_on_ed25519_curve(&hash_bytes) {
            return Some((hash_bytes, bump));
        }
    }
    None
}

/// Check if 32 bytes represent a valid Ed25519 curve point.
///
/// Uses `curve25519-dalek` (via `ed25519-dalek`) to attempt point decompression.
/// If decompression succeeds, the bytes are on the curve (NOT a valid PDA).
fn is_on_ed25519_curve(bytes: &[u8; 32]) -> bool {
    use ed25519_dalek::VerifyingKey;
    // VerifyingKey::from_bytes performs curve point decompression + small subgroup check
    VerifyingKey::from_bytes(bytes).is_ok()
}

/// Decode a Base58 string to exactly 32 bytes (Solana pubkey).
fn bs58_decode_32(s: &str) -> Result<[u8; 32]> {
    let bytes = bs58::decode(s)
        .into_vec()
        .map_err(|e| anyhow!("Invalid Base58 '{}': {}", s, e))?;
    if bytes.len() != 32 {
        return Err(anyhow!(
            "Expected 32 bytes for '{}', got {}",
            s,
            bytes.len()
        ));
    }
    let mut result = [0u8; 32];
    result.copy_from_slice(&bytes);
    Ok(result)
}

/// Convert a Base58-encoded Solana address to `solana_sdk::pubkey::Pubkey` (v2 chain).
fn solana_pubkey(s: &str) -> Result<solana_sdk::pubkey::Pubkey> {
    solana_sdk::pubkey::Pubkey::from_str(s)
        .map_err(|e| anyhow!("Invalid Solana pubkey '{}': {}", s, e))
}

/// Convert a Base58-encoded Solana address to SPL-compatible `Pubkey` (v3 chain).
///
/// `spl-token v9` / `spl-associated-token-account v8` use `solana_pubkey v3`,
/// while `solana-sdk v2` uses `solana_pubkey v2`. This helper returns the v3
/// variant for use with SPL instruction builders.
fn spl_pubkey(s: &str) -> Result<spl_token::solana_program::pubkey::Pubkey> {
    spl_token::solana_program::pubkey::Pubkey::from_str(s)
        .map_err(|e| anyhow!("Invalid Solana pubkey '{}': {}", s, e))
}

// ─── ComputeBudget Instructions ─────────────────────────────────────────────
//
// solana-sdk v2 decomposed the crate: `compute_budget` only re-exports the
// interface (program ID + instruction discriminants), NOT the higher-level
// instruction builders. We construct them manually here.
//
// Reference: https://docs.rs/solana-compute-budget-interface/

/// ComputeBudget program ID
const COMPUTE_BUDGET_PROGRAM_ID: &str = "ComputeBudget111111111111111111111111111111";

/// Build ComputeBudget instructions (SetComputeUnitLimit + SetComputeUnitPrice).
///
/// Returns a pair of `Instruction`s ready to be included in a Message.
fn compute_budget_instructions(
    unit_limit: u32,
    unit_price: u64,
) -> Result<(
    solana_sdk::instruction::Instruction,
    solana_sdk::instruction::Instruction,
)> {
    use solana_sdk::instruction::Instruction;

    let program_id = solana_pubkey(COMPUTE_BUDGET_PROGRAM_ID)?;

    // SetComputeUnitLimit: discriminant 0x02 + u32 LE
    let mut limit_data = Vec::with_capacity(5);
    limit_data.push(0x02);
    limit_data.extend_from_slice(&unit_limit.to_le_bytes());

    let limit_ix = Instruction {
        program_id,
        accounts: vec![],
        data: limit_data,
    };

    // SetComputeUnitPrice: discriminant 0x03 + u64 LE
    let mut price_data = Vec::with_capacity(9);
    price_data.push(0x03);
    price_data.extend_from_slice(&unit_price.to_le_bytes());

    let price_ix = Instruction {
        program_id,
        accounts: vec![],
        data: price_data,
    };

    Ok((limit_ix, price_ix))
}

// ─── Instruction Version Bridge ─────────────────────────────────────────────
//
// `spl-token v9` uses `solana-instruction v3` while `solana-sdk v2` uses
// `solana-instruction v2`. Both have identical binary layout but are distinct
// Rust types. This helper bridges them via raw byte conversion.

/// Convert an SPL Token instruction (solana-instruction v3) to solana-sdk's
/// instruction type (v2) by byte-copying fields.
///
/// Both `Instruction` types have identical fields:
///   `program_id: Pubkey([u8; 32])`, `accounts: Vec<AccountMeta>`, `data: Vec<u8>`
fn convert_spl_ix(
    ix: spl_token::solana_program::instruction::Instruction,
) -> solana_sdk::instruction::Instruction {
    use solana_sdk::instruction::{AccountMeta, Instruction};
    use solana_sdk::pubkey::Pubkey;

    let program_id = Pubkey::new_from_array(ix.program_id.to_bytes());

    let accounts = ix
        .accounts
        .into_iter()
        .map(|am| AccountMeta {
            pubkey: Pubkey::new_from_array(am.pubkey.to_bytes()),
            is_signer: am.is_signer,
            is_writable: am.is_writable,
        })
        .collect();

    Instruction {
        program_id,
        accounts,
        data: ix.data,
    }
}

// ─── ChainClient Implementation ────────────────────────────────────────────

#[async_trait]
impl ChainClient for SolanaClient {
    async fn get_token_balance(&self, address: &str, token_address: &str) -> Result<U256> {
        // Derive ATA address locally, then query balance
        let ata = derive_ata_address(address, token_address, SPL_TOKEN_PROGRAM_ID)?;
        match self.get_token_account_balance(&ata).await? {
            Some(balance) => Ok(U256::from(balance)),
            None => Ok(U256::ZERO), // ATA doesn't exist → zero balance
        }
    }

    async fn get_native_balance(&self, address: &str) -> Result<U256> {
        let lamports = self.get_sol_balance(address).await?;
        Ok(U256::from(lamports))
    }

    async fn build_token_transfer(
        &self,
        from: &str,
        to: &str,
        token_address: &str,
        amount: U256,
    ) -> Result<ChainUnsignedTx> {
        let amount_u64 = amount
            .try_into()
            .map_err(|_| anyhow!("Amount overflow for Solana u64: {}", amount))?;

        // SDK-chain pubkey (v2) — for Message::new()
        let from_pubkey = solana_pubkey(from)?;

        // SPL-chain pubkeys (v3) — for spl-token / spl-ata instruction builders
        let from_pk = spl_pubkey(from)?;
        let to_pk = spl_pubkey(to)?;
        let mint_pk = spl_pubkey(token_address)?;
        let token_prog = spl_pubkey(SPL_TOKEN_PROGRAM_ID)?;

        // Derive ATAs
        let from_ata_str = derive_ata_address(from, token_address, SPL_TOKEN_PROGRAM_ID)?;
        let to_ata_str = derive_ata_address(to, token_address, SPL_TOKEN_PROGRAM_ID)?;
        let from_ata_pk = spl_pubkey(&from_ata_str)?;
        let to_ata_pk = spl_pubkey(&to_ata_str)?;

        // Build instruction list
        let mut instructions = Vec::with_capacity(4);

        // 1. ComputeBudget
        let priority_fee = self.get_priority_fee_or_default().await;
        let (limit_ix, price_ix) = compute_budget_instructions(50_000, priority_fee)?;
        instructions.push(limit_ix);
        instructions.push(price_ix);

        // 2. CreateATA (idempotent — no-op if already exists, avoids TOCTOU race)
        instructions.push(convert_spl_ix(
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &from_pk, // payer (also the sender for payout)
                &to_pk,   // wallet owner
                &mint_pk, // token mint
                &token_prog,
            ),
        ));

        // 3. SPL Token TransferChecked (includes decimals for safety)
        // NOTE: Decimals are hardcoded to 6 (USDT/USDC on Solana).
        //       ChainClient trait doesn't pass decimals; refactor when adding
        //       non-6-decimal tokens (e.g., PYUSD via Token-2022).
        instructions.push(convert_spl_ix(spl_token::instruction::transfer_checked(
            &token_prog,
            &from_ata_pk, // source ATA
            &mint_pk,     // mint
            &to_ata_pk,   // destination ATA
            &from_pk,     // authority (owner of source ATA)
            &[],          // no multisig signers
            amount_u64,
            6, // USDC/USDT decimals on Solana
        )?));

        // Finalize message
        let (message_bytes, num_required_signatures, blockhash, last_valid_block_height) =
            self.finalize_message(&instructions, &from_pubkey).await?;

        Ok(ChainUnsignedTx::Solana(SolanaUnsignedTx {
            message_bytes,
            recent_blockhash: blockhash,
            last_valid_block_height,
            num_required_signatures,
            signer_pubkeys: vec![from.to_string()],
        }))
    }

    async fn build_native_transfer(
        &self,
        from: &str,
        to: &str,
        amount: U256,
    ) -> Result<ChainUnsignedTx> {
        let amount_lamports: u64 = amount
            .try_into()
            .map_err(|_| anyhow!("Amount overflow for Solana u64: {}", amount))?;

        let from_pubkey = solana_pubkey(from)?;
        let to_pubkey = solana_pubkey(to)?;

        let mut instructions = Vec::with_capacity(3);

        // 1. ComputeBudget
        let priority_fee = self.get_priority_fee_or_default().await;
        let (limit_ix, price_ix) = compute_budget_instructions(5_000, priority_fee)?;
        instructions.push(limit_ix);
        instructions.push(price_ix);

        // 2. System::Transfer
        instructions.push(solana_sdk::system_instruction::transfer(
            &from_pubkey,
            &to_pubkey,
            amount_lamports,
        ));

        // Finalize message
        let (message_bytes, num_required_signatures, blockhash, last_valid_block_height) =
            self.finalize_message(&instructions, &from_pubkey).await?;

        Ok(ChainUnsignedTx::Solana(SolanaUnsignedTx {
            message_bytes,
            recent_blockhash: blockhash,
            last_valid_block_height,
            num_required_signatures,
            signer_pubkeys: vec![from.to_string()],
        }))
    }

    async fn broadcast(&self, tx: &ChainSignedTx) -> Result<ChainBroadcastResult> {
        match tx {
            ChainSignedTx::Solana(solana_tx) => {
                // Fallback: If the signed tx doesn't carry the blockhash separately,
                // do a simple send (backward compat for tests).
                // The caller should use `broadcast_solana()` directly for retry loop.
                let signature = self.send_transaction(&solana_tx.serialized_tx).await?;
                Ok(ChainBroadcastResult {
                    success: true,
                    tx_hash: signature,
                    message: None,
                })
            }
            #[allow(unreachable_patterns)]
            _ => Err(anyhow!(
                "SolanaClient cannot broadcast non-Solana transactions"
            )),
        }
    }

    async fn get_current_block(&self) -> Result<ChainBlockInfo> {
        let slot = self.get_slot().await?;
        let timestamp = self
            .get_block_time(slot)
            .await?
            .unwrap_or_else(|| chrono::Utc::now().timestamp());

        Ok(ChainBlockInfo {
            number: slot,
            timestamp,
        })
    }

    async fn get_transaction_info(&self, tx_hash: &str) -> Result<Option<ChainTransactionInfo>> {
        let tx = match self.get_transaction(tx_hash).await? {
            Some(t) => t,
            None => return Ok(None),
        };

        let meta = tx.meta.as_ref();
        let success = meta.is_some_and(|m| m.err.is_none());
        // Cap at i64::MAX to prevent silent truncation from u64 → i64.
        // Normal fees are ~5000 lamports, but priority fees have no upper bound.
        let fee_burned = meta.map(|m| m.fee.min(i64::MAX as u64) as i64).unwrap_or(0);

        Ok(Some(ChainTransactionInfo {
            tx_hash: tx
                .transaction
                .signatures
                .first()
                .cloned()
                .unwrap_or_default(),
            block_number: tx.slot as i64,
            success,
            result: if success {
                None
            } else {
                meta.and_then(|m| m.err.as_ref()).map(|e| format!("{}", e))
            },
            fee_burned,
            revert_message: if success {
                None
            } else {
                Some("Transaction failed".to_string())
            },
        }))
    }

    async fn get_transaction_by_id(&self, tx_hash: &str) -> Result<Option<ChainSignedTx>> {
        let tx = match self.get_transaction(tx_hash).await? {
            Some(t) => t,
            None => return Ok(None),
        };

        let signature = tx
            .transaction
            .signatures
            .first()
            .cloned()
            .unwrap_or_default();

        Ok(Some(ChainSignedTx::Solana(SolanaSignedTx {
            signature,
            serialized_tx: String::new(), // Not available from getTransaction
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
        // Fetch the transaction and verify it contains an SPL Token transfer
        // to the expected pay address for the correct token mint.
        let tx = match self.get_transaction(tx_hash).await {
            Ok(Some(t)) => t,
            _ => return false,
        };

        let meta = match &tx.meta {
            Some(m) => m,
            None => return false,
        };

        // Transaction must have succeeded
        if meta.err.is_some() {
            return false;
        }

        let post_balances = match &meta.post_token_balances {
            Some(b) => b,
            None => return false,
        };

        let pre_balances = meta.pre_token_balances.as_deref().unwrap_or(&[]);

        // Derive the expected ATA for the pay address + token mint
        let expected_ata =
            match derive_ata_address(expected_pay_address, token_contract, SPL_TOKEN_PROGRAM_ID) {
                Ok(ata) => ata,
                Err(_) => return false,
            };

        // Check if any post-token-balance entry matches:
        // 1. The correct mint (token_contract)
        // 2. Owner is the expected pay address OR account maps to expected ATA
        // 3. Balance actually INCREASED (delta > 0) vs pre-balance
        for balance in post_balances {
            let mint_matches = balance.mint == token_contract;
            let owner_matches = balance
                .owner
                .as_deref()
                .is_some_and(|o| o == expected_pay_address);

            // Also check if we can resolve the account key at this index
            let account_key_matches = (balance.account_index as usize)
                < tx.transaction.message.account_keys.len()
                && tx.transaction.message.account_keys[balance.account_index as usize].pubkey
                    == expected_ata;

            if mint_matches && (owner_matches || account_key_matches) {
                // Compare pre vs post to verify actual token inflow.
                // Post-balance > 0 alone is insufficient — the ATA may have
                // residual balance from a previous unswepped deposit.
                let post_amount: u64 = balance.ui_token_amount.amount.parse().unwrap_or(0);

                let pre_amount: u64 = pre_balances
                    .iter()
                    .find(|b| b.account_index == balance.account_index)
                    .map(|b| b.ui_token_amount.amount.parse::<u64>().unwrap_or(0))
                    .unwrap_or(0); // 0 if ATA was just created in this tx

                if post_amount > pre_amount {
                    return true;
                }
            }
        }

        false
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        matchers::{body_partial_json, method},
        Mock, MockServer, ResponseTemplate,
    };

    #[tokio::test]
    async fn signature_status_lookup_searches_transaction_history() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({
                "method": "getSignatureStatuses",
                "params": [["test-signature"], {"searchTransactionHistory": true}]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "context": {"slot": 1},
                    "value": [null]
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = SolanaClient::new(vec![server.uri()], Network::Solana);
        let statuses = client
            .get_signature_statuses(&["test-signature"])
            .await
            .unwrap();

        assert_eq!(statuses.len(), 1);
        assert!(statuses[0].is_none());
    }

    // ─── ATA Derivation Tests ───────────────────────────────────────────────

    #[test]
    fn test_ata_derivation_known_address() {
        // USDC on Solana mainnet
        // Mint: EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
        // A well-known wallet for testing
        let owner = "vines1vzrYbzLMRdu58ou5XTby4qAqVRLmqo36NKPTg";
        let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

        let ata = derive_ata_address(owner, usdc_mint, SPL_TOKEN_PROGRAM_ID).unwrap();

        // ATA should be a valid Solana address (32-44 chars, valid Base58)
        assert!(
            ata.len() >= 32 && ata.len() <= 44,
            "ATA length: {}",
            ata.len()
        );
        let decoded = bs58::decode(&ata).into_vec().unwrap();
        assert_eq!(decoded.len(), 32, "Decoded ATA should be 32 bytes");
    }

    #[test]
    fn test_ata_derivation_deterministic() {
        let owner = "11111111111111111111111111111111";
        let mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

        let ata1 = derive_ata_address(owner, mint, SPL_TOKEN_PROGRAM_ID).unwrap();
        let ata2 = derive_ata_address(owner, mint, SPL_TOKEN_PROGRAM_ID).unwrap();
        assert_eq!(ata1, ata2, "ATA derivation must be deterministic");
    }

    #[test]
    fn test_ata_different_owners_different_atas() {
        let mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        let owner1 = "vines1vzrYbzLMRdu58ou5XTby4qAqVRLmqo36NKPTg";
        let owner2 = "11111111111111111111111111111111";

        let ata1 = derive_ata_address(owner1, mint, SPL_TOKEN_PROGRAM_ID).unwrap();
        let ata2 = derive_ata_address(owner2, mint, SPL_TOKEN_PROGRAM_ID).unwrap();
        assert_ne!(ata1, ata2, "Different owners should produce different ATAs");
    }

    #[test]
    fn test_ata_different_mints_different_atas() {
        let owner = "vines1vzrYbzLMRdu58ou5XTby4qAqVRLmqo36NKPTg";
        let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        let usdt_mint = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";

        let ata_usdc = derive_ata_address(owner, usdc_mint, SPL_TOKEN_PROGRAM_ID).unwrap();
        let ata_usdt = derive_ata_address(owner, usdt_mint, SPL_TOKEN_PROGRAM_ID).unwrap();
        assert_ne!(
            ata_usdc, ata_usdt,
            "Different mints should produce different ATAs"
        );
    }

    #[test]
    fn test_ata_invalid_base58_fails() {
        let result = derive_ata_address("invalid!!", "also-invalid", SPL_TOKEN_PROGRAM_ID);
        assert!(result.is_err());
    }

    #[test]
    fn test_ata_wrong_length_fails() {
        // Valid Base58 but wrong length (not 32 bytes)
        let short = "1111"; // decodes to < 32 bytes
        let result = derive_ata_address(
            short,
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            SPL_TOKEN_PROGRAM_ID,
        );
        assert!(result.is_err());
    }

    // ─── find_program_address Tests ─────────────────────────────────────────

    #[test]
    fn test_find_program_address_returns_valid_pda() {
        let seed = b"hello";
        let program_id = [1u8; 32];
        let result = find_program_address(&[seed.as_ref()], &program_id);
        assert!(result.is_some(), "Should find a valid PDA");

        let (pda, _bump) = result.unwrap();
        // PDA must NOT be on the Ed25519 curve
        assert!(!is_on_ed25519_curve(&pda));
    }

    #[test]
    fn test_find_program_address_deterministic() {
        let seed = b"deterministic_test";
        let program_id = [42u8; 32];

        let (pda1, bump1) = find_program_address(&[seed.as_ref()], &program_id).unwrap();
        let (pda2, bump2) = find_program_address(&[seed.as_ref()], &program_id).unwrap();

        assert_eq!(pda1, pda2);
        assert_eq!(bump1, bump2);
    }

    // ─── bs58_decode_32 Tests ───────────────────────────────────────────────

    #[test]
    fn test_bs58_decode_32_valid() {
        // System program = all zeros = "11111111111111111111111111111111"
        let result = bs58_decode_32("11111111111111111111111111111111");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), [0u8; 32]);
    }

    #[test]
    fn test_bs58_decode_32_invalid() {
        assert!(bs58_decode_32("invalid!!").is_err());
        assert!(bs58_decode_32("1111").is_err()); // too short
    }
}
