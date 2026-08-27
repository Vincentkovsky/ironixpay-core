//! Tron blockchain client
//!
//! Provides interface to Tron network for USDT operations and transaction monitoring.

pub mod address;
pub mod interface; // Added interface module
pub mod types;

use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use tracing::{debug, error};

// Use types from local interface instead of wallet
use interface::{
    BlockInfo, BroadcastResult, SignedTransaction, TransactionInfo, TronBroadcaster,
    UnsignedTransaction,
};

pub struct TronClient {
    http: ClientWithMiddleware,
    full_node: String,
    full_nodes: Vec<String>,
    usdt_contract: String,
}

impl TronClient {
    pub fn new(full_node: String, usdt_contract: String, api_key: Option<String>) -> Self {
        Self::new_with_endpoints(vec![full_node], usdt_contract, api_key)
    }

    pub fn new_with_endpoints(
        mut full_nodes: Vec<String>,
        usdt_contract: String,
        api_key: Option<String>,
    ) -> Self {
        assert!(
            !full_nodes.is_empty(),
            "At least one TRON RPC endpoint is required"
        );
        full_nodes.dedup();
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(key) = &api_key {
            headers.insert(
                "TRON-PRO-API-KEY",
                reqwest::header::HeaderValue::from_str(key)
                    .expect("Invalid TRONGRID_API_KEY value"),
            );
        }

        let base_client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .default_headers(headers)
            .build()
            .expect("Failed to build HTTP client");

        // Retry policy: 3 attempts with exponential backoff (1s base)
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        let http = ClientBuilder::new(base_client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Self {
            http,
            full_node: full_nodes[0].clone(),
            full_nodes,
            usdt_contract,
        }
    }

    /// Get USDT balance for an address (returns amount in smallest unit, 6 decimals).
    /// Convenience wrapper around `get_trc20_balance` using `self.usdt_contract`.
    pub async fn get_usdt_balance(&self, address: &str) -> Result<i64> {
        self.get_trc20_balance(address, &self.usdt_contract.clone())
            .await
    }

    /// Get any TRC-20 token balance for an address (returns raw on-chain amount).
    /// `token_contract` is the Base58 contract address of the TRC-20 token.
    pub async fn get_trc20_balance(&self, address: &str, token_contract: &str) -> Result<i64> {
        use alloy_primitives::Address as EvmAddress;
        use alloy_sol_types::{sol, SolCall};
        use types::TriggerConstantContractResponse;

        sol! {
            function balanceOf(address account) external view returns (uint256);
        }

        let url = format!("{}/wallet/triggerconstantcontract", self.full_node);

        // Convert TRON address to EVM address for ABI encoding
        let evm_bytes = address::tron_to_evm(address)?;
        let evm_addr = EvmAddress::from(evm_bytes);

        // Build ABI-encoded parameter (without selector)
        let call = balanceOfCall { account: evm_addr };
        let encoded = call.abi_encode();
        assert!(encoded.len() >= 4, "ABI encoded data too short");
        let param_hex = hex::encode(&encoded[4..]); // Skip selector

        let owner_hex = address::to_hex(address)?;
        let contract_hex = address::to_hex(token_contract)?;

        let req = types::TriggerConstantContractRequest {
            owner_address: owner_hex,
            contract_address: contract_hex,
            function_selector: "balanceOf(address)".to_string(),
            parameter: param_hex,
            visible: Some(false),
        };

        debug!(
            "get_trc20_balance request: url={} contract={} req={:?}",
            url, token_contract, req
        );

        let resp: TriggerConstantContractResponse =
            self.http.post(&url).json(&req).send().await?.json().await?;

        if !resp.result.result {
            return Ok(0);
        }

        if let Some(result) = resp.constant_result.first() {
            let balance = i64::from_str_radix(result, 16).unwrap_or(0);
            return Ok(balance);
        }
        Ok(0)
    }

    /// Check if address has any transactions in the last `seconds` seconds
    pub async fn has_recent_transactions(&self, address: &str, seconds: u64) -> Result<bool> {
        use types::TRC20TransactionsResponse;

        let url = format!(
            "{}/v1/accounts/{}/transactions/trc20?limit=1&min_timestamp={}",
            self.full_node,
            address,
            chrono::Utc::now().timestamp_millis() - (seconds as i64 * 1000)
        );

        let resp: TRC20TransactionsResponse = self.http.get(&url).send().await?.json().await?;
        Ok(!resp.data.is_empty())
    }

    /// Get TRC20 transactions for an address
    pub async fn get_trc20_transactions(
        &self,
        address: &str,
        limit: u32,
        min_timestamp: Option<i64>,
    ) -> Result<Vec<types::TRC20Transaction>> {
        use types::TRC20TransactionsResponse;

        let mut url = format!(
            "{}/v1/accounts/{}/transactions/trc20?limit={}&only_to=true&contract_address={}",
            self.full_node, address, limit, self.usdt_contract
        );

        if let Some(ts) = min_timestamp {
            url.push_str(&format!("&min_timestamp={}", ts));
        }

        debug!(url = %url, "Fetching TRC20 transactions");

        let resp: TRC20TransactionsResponse = self.http.get(&url).send().await?.json().await?;
        Ok(resp.data)
    }

    /// Get total TRX balance in SUN
    pub async fn get_trx_balance(&self, address: &str) -> Result<u64> {
        use types::{GetAccountRequest, GetAccountResponse};

        let url = format!("{}/wallet/getaccount", self.full_node);

        let req = GetAccountRequest {
            address: address::to_hex(address)?,
            visible: Some(false),
        };

        let resp: GetAccountResponse = self.http.post(&url).json(&req).send().await?.json().await?;

        // Safe conversion: balance should be non-negative, but guard against API weirdness
        if resp.balance < 0 {
            anyhow::bail!(
                "Unexpected negative balance from TRON API: {}",
                resp.balance
            );
        }
        Ok(resp.balance as u64)
    }

    /// Get current block number and timestamp
    pub async fn get_current_block(&self) -> Result<BlockInfo> {
        use types::BlockResponse;

        let url = format!("{}/wallet/getnowblock", self.full_node);
        let resp: BlockResponse = self.http.get(&url).send().await?.json().await?;
        Ok(BlockInfo {
            number: resp.block_header.raw_data.number as u64,
            timestamp: resp.block_header.raw_data.timestamp,
        })
    }

    /// Get details of an on-chain transaction
    pub async fn get_transaction_info(&self, tx_hash: &str) -> Result<Option<TransactionInfo>> {
        use types::TransactionInfoResponse;

        let url = format!("{}/wallet/gettransactioninfobyid", self.full_node);
        // Note: The API returns an EMPTY JSON object `{}` if not found, or maybe just empty body?
        // documentation implies return is the object.
        // `reqwest` json() might fail if body is empty.
        // But TRON usually returns {} for not found on some endpoints, or specific error.
        // Let's keep it safe with `serde_json::Value` check first ONLY if we suspect weirdness,
        // but strong typing is the goal.
        // Actually, for getTransactionInfoById, if tx not found, it returns empty object `{}`.
        // Our strong struct `TransactionInfoResponse` has required fields like `id`, `block_number`.
        // So direct parsing will FAIL if it's `{}`.
        // We need to handle this.
        let resp_value: serde_json::Value = self
            .http
            .post(&url)
            .json(&serde_json::json!({ "value": tx_hash }))
            .send()
            .await?
            .json()
            .await?;

        // Check for specific API error (e.g. invalid hash, internal error)
        if let Some(error) = resp_value.get("Error") {
            anyhow::bail!("Tron node error: {}", error);
        }

        if resp_value.as_object().map(|o| o.is_empty()).unwrap_or(true) {
            return Ok(None);
        }

        let resp: TransactionInfoResponse = serde_json::from_value(resp_value)?;

        if resp.block_number == 0 {
            return Ok(None);
        }

        // Check receipt result (usually "SUCCESS" or "OUT_OF_ENERGY")
        // Also check top-level result (sometimes used for "FAILED")
        let is_success = if let Some(res) = &resp.receipt.result {
            if res != "SUCCESS" {
                false
            } else {
                // If receipt says SUCCESS, double check top level isn't FAILED
                resp.result.as_deref() != Some("FAILED")
            }
        } else {
            // No receipt result? Fallback to top level
            resp.result.as_deref() == Some("SUCCESS") || resp.result.is_none()
        };

        let fee_burned = resp.fee;

        let revert_message = if !is_success {
            resp.res_message.map(|s| {
                if let Ok(decoded) = hex::decode(&s) {
                    String::from_utf8_lossy(&decoded).to_string()
                } else {
                    s
                }
            })
        } else {
            None
        };

        let tx_result = resp.receipt.result.clone().or_else(|| resp.result.clone());

        Ok(Some(TransactionInfo {
            tx_hash: tx_hash.to_string(),
            block_number: resp.block_number,
            success: is_success,
            result: tx_result,
            fee_burned,
            revert_message,
        }))
    }

    /// Get transaction by ID (checks mempool and chain)
    pub async fn get_transaction_by_id(&self, tx_hash: &str) -> Result<Option<SignedTransaction>> {
        let url = format!("{}/wallet/gettransactionbyid", self.full_node);

        let resp_value: serde_json::Value = self
            .http
            .post(&url)
            .json(&serde_json::json!({ "value": tx_hash }))
            .send()
            .await?
            .json()
            .await?;

        // Check for specific API error
        if let Some(error) = resp_value.get("Error") {
            anyhow::bail!("Tron node error: {}", error);
        }

        if resp_value.as_object().map(|o| o.is_empty()).unwrap_or(true) {
            return Ok(None);
        }

        let tx_id = resp_value
            .get("txID")
            .and_then(|v| v.as_str())
            .unwrap_or(tx_hash)
            .to_string();

        let raw_data_hex = resp_value.get("raw_data_hex").and_then(|v| v.as_str());

        let raw_data = if let Some(hex_str) = raw_data_hex {
            hex::decode(hex_str)?
        } else {
            // If raw_data_hex is missing, try to treat it as empty or fail?
            // Some API responses might behave differently.
            // For now, if we can't get raw bytes, we assume something is wrong or it's not a standard return.
            return Ok(None);
        };

        let signature = if let Some(sigs) = resp_value.get("signature").and_then(|v| v.as_array()) {
            if let Some(first) = sigs.first().and_then(|s| s.as_str()) {
                hex::decode(first).unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let expiration = resp_value
            .get("raw_data")
            .and_then(|v| v.get("expiration"))
            .and_then(|v| v.as_i64());

        Ok(Some(SignedTransaction {
            tx_id,
            raw_data,
            signature,
            raw_data_json: resp_value.get("raw_data").cloned(),
            expiration,
        }))
    }

    /// Query every configured full node before declaring a deterministic hash absent.
    pub async fn transaction_known_on_any_endpoint(&self, tx_hash: &str) -> Result<bool> {
        for full_node in &self.full_nodes {
            let url = format!("{}/wallet/gettransactionbyid", full_node);
            let response: serde_json::Value = self
                .http
                .post(&url)
                .json(&serde_json::json!({ "value": tx_hash }))
                .send()
                .await?
                .json()
                .await?;
            if let Some(error) = response.get("Error") {
                anyhow::bail!("TRON node {} returned error: {}", full_node, error);
            }
            if response
                .get("txID")
                .and_then(|value| value.as_str())
                .is_some_and(|value| value.eq_ignore_ascii_case(tx_hash))
            {
                return Ok(true);
            }
            if !response
                .as_object()
                .map(|value| value.is_empty())
                .unwrap_or(true)
            {
                anyhow::bail!(
                    "TRON node {} returned an unrecognized transaction lookup response",
                    full_node
                );
            }
        }
        Ok(false)
    }

    pub async fn build_trc20_transfer(
        &self,
        from: &str,
        to: &str,
        amount: u64,
        contract_address: &str,
    ) -> Result<UnsignedTransaction> {
        use alloy_primitives::{Address as EvmAddress, U256};
        use alloy_sol_types::{sol, SolCall};
        use types::TriggerSmartContractRequest;

        sol! {
            function transfer(address to, uint256 amount) external returns (bool);
        }

        let url = format!("{}/wallet/triggersmartcontract", self.full_node);

        // Convert TRON address to EVM address for ABI encoding
        let to_evm_bytes = address::tron_to_evm(to)?;
        let to_evm = EvmAddress::from(to_evm_bytes);

        // Build ABI-encoded parameter (without selector)
        let call = transferCall {
            to: to_evm,
            amount: U256::from(amount),
        };
        let encoded = call.abi_encode();
        assert!(encoded.len() >= 4, "ABI encoded data too short");
        // Skip first 4 bytes (selector) as TRON API uses function_selector separately
        let param_hex = hex::encode(&encoded[4..]);

        let from_hex = address::to_hex(from)?;
        let contract_hex = address::to_hex(contract_address)?;

        let req = TriggerSmartContractRequest {
            owner_address: from_hex,
            contract_address: contract_hex,
            function_selector: "transfer(address,uint256)".to_string(),
            parameter: param_hex,
            // TODO: Consider making this dynamic or configurable (e.g. 100 TRX) for safety
            fee_limit: 50_000_000,
            call_value: 0,
        };

        debug!("Building transfer tx: {:?}", req);

        let resp: serde_json::Value = self.http.post(&url).json(&req).send().await?.json().await?;

        if let Some(error) = resp.get("Error") {
            anyhow::bail!("Tron node error: {}", error);
        }

        if let Some(result) = resp.get("result") {
            if result["result"].as_bool() != Some(true) {
                let msg = result["message"].as_str().unwrap_or("Unknown error");
                anyhow::bail!("Contract trigger failed: {}", msg);
            }
        }

        let tx = resp
            .get("transaction")
            .ok_or_else(|| anyhow::anyhow!("No transaction in response"))?;
        let raw_data_hex = tx["raw_data_hex"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No raw_data_hex"))?
            .to_string();

        let tx_data = tx.get("raw_data");
        let expiration = tx_data
            .and_then(|v| v.get("expiration"))
            .and_then(|v| v.as_i64());

        Ok(UnsignedTransaction {
            raw_data: hex::decode(&raw_data_hex)?,
            raw_data_hex,
            raw_data_json: tx.get("raw_data").cloned(),
            expiration,
        })
    }

    pub fn sign_transaction(
        &self,
        tx: &UnsignedTransaction,
        private_key: &[u8],
    ) -> Result<SignedTransaction> {
        use bitcoin::secp256k1::{Message, Secp256k1, SecretKey};
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(&tx.raw_data);
        let tx_hash = hasher.finalize();
        let tx_id_hex = hex::encode(tx_hash);

        let secp = Secp256k1::new();
        let sk = SecretKey::from_slice(private_key)
            .map_err(|e| anyhow::anyhow!("Invalid private key: {}", e))?;
        let msg = Message::from_digest_slice(&tx_hash).expect("Hash is 32 bytes");

        let sig = secp.sign_ecdsa_recoverable(&msg, &sk);
        let (rec_id, sig_bytes) = sig.serialize_compact();

        let mut full_sig = Vec::new();
        full_sig.extend_from_slice(&sig_bytes);
        full_sig.push(rec_id.to_i32() as u8);

        Ok(SignedTransaction {
            tx_id: tx_id_hex,
            raw_data: tx.raw_data.clone(),
            signature: full_sig,
            raw_data_json: tx.raw_data_json.clone(),
            expiration: tx.expiration,
        })
    }

    pub async fn broadcast(&self, tx: &SignedTransaction) -> Result<BroadcastResult> {
        use types::{BroadcastTransactionRequest, BroadcastTransactionResponse};

        let url = format!("{}/wallet/broadcasttransaction", self.full_node);

        let req = BroadcastTransactionRequest {
            raw_data_hex: hex::encode(&tx.raw_data),
            signature: vec![hex::encode(&tx.signature)],
            raw_data: tx.raw_data_json.clone(),
        };

        debug!(tx_id=%tx.tx_id, "Broadcasting transaction");

        let resp: BroadcastTransactionResponse =
            self.http.post(&url).json(&req).send().await?.json().await?;

        let success = resp.result.unwrap_or(false);

        if success {
            Ok(BroadcastResult {
                success: true,
                tx_hash: tx.tx_id.clone(),
                message: resp.message,
            })
        } else {
            error!(?resp, "Broadcast failed raw response");

            let msg = if let Some(hex_msg) = &resp.message {
                if let Ok(decoded) = hex::decode(hex_msg) {
                    String::from_utf8_lossy(&decoded).to_string()
                } else {
                    hex_msg.clone()
                }
            } else {
                "Unknown message".to_string()
            };

            let code = resp.code.unwrap_or_else(|| "UNKNOWN".to_string());
            anyhow::bail!("Broadcast failed: {:?} {}", code, msg);
        }
    }

    pub async fn build_trx_transfer(
        &self,
        from: &str,
        to: &str,
        amount_sun: u64,
    ) -> Result<UnsignedTransaction> {
        use types::CreateTransactionRequest;

        let url = format!("{}/wallet/createtransaction", self.full_node);

        let from_hex = address::to_hex(from)?;
        let to_hex = address::to_hex(to)?;

        let req = CreateTransactionRequest {
            owner_address: from_hex,
            to_address: to_hex,
            amount: amount_sun,
            visible: Some(false),
        };

        debug!(
            "Building TRX transfer: {} -> {} amount={} SUN",
            from, to, amount_sun
        );

        let response = self
            .http
            .post(&url)
            .json(&req)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if let Some(error) = response.get("Error") {
            return Err(anyhow::anyhow!("TRON API error: {}", error));
        }

        let raw_data_hex = response["raw_data_hex"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing raw_data_hex in response"))?;

        let raw_data = hex::decode(raw_data_hex)?;

        let expiration = response
            .get("raw_data")
            .and_then(|v| v.get("expiration"))
            .and_then(|v| v.as_i64());

        Ok(UnsignedTransaction {
            raw_data,
            raw_data_hex: raw_data_hex.to_string(),
            raw_data_json: response.get("raw_data").cloned(),
            expiration,
        })
    }

    pub async fn get_account_resources(&self, address: &str) -> Result<interface::AccountResource> {
        use types::{GetAccountRequest, GetAccountResourceResponse};

        let url = format!("{}/wallet/getaccountresource", self.full_node);

        let req = GetAccountRequest {
            address: address::to_hex(address)?,
            visible: Some(false),
        };

        let resp: GetAccountResourceResponse =
            self.http.post(&url).json(&req).send().await?.json().await?;

        let asset_net_used = resp
            .asset_net_used
            .into_iter()
            .map(|e| interface::AssetNetUsed {
                key: e.key,
                value: e.value,
            })
            .collect();

        Ok(interface::AccountResource {
            free_net_used: resp.free_net_used,
            free_net_limit: resp.free_net_limit,
            net_limit: resp.net_limit,
            asset_net_used,
            net_used: resp.net_used,
            energy_limit: resp.energy_limit,
            energy_used: resp.energy_used,
        })
    }

    /// Estimate energy for a contract call
    pub async fn estimate_energy(
        &self,
        owner_address: &str,
        contract_address: &str,
        function_selector: &str,
        parameter: &str,
    ) -> Result<i64> {
        use types::{EstimateEnergyRequest, EstimateEnergyResponse};

        let url = format!("{}/wallet/estimateenergy", self.full_node);

        let req = EstimateEnergyRequest {
            owner_address: address::to_hex(owner_address)?,
            contract_address: address::to_hex(contract_address)?,
            function_selector: function_selector.to_string(),
            parameter: parameter.to_string(),
            visible: Some(false),
        };

        let resp: EstimateEnergyResponse =
            self.http.post(&url).json(&req).send().await?.json().await?;

        if resp.result.result {
            Ok(resp.energy_required)
        } else {
            let msg = resp.result.message.as_deref().unwrap_or("Unknown error");
            // Try to decode hex message if possible
            let decoded_msg = if let Ok(decoded) = hex::decode(msg) {
                String::from_utf8_lossy(&decoded).to_string()
            } else {
                msg.to_string()
            };
            anyhow::bail!("Estimate energy failed: {}", decoded_msg);
        }
    }

    /// Get all transactions from a specific block
    pub async fn get_block_transactions(
        &self,
        block_num: i64,
    ) -> Result<Vec<types::RawTransaction>> {
        use types::BlockTransactionsResponse;

        let url = format!("{}/wallet/getblockbynum", self.full_node);
        let resp: BlockTransactionsResponse = self
            .http
            .post(&url)
            .json(&serde_json::json!({ "num": block_num }))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.transactions.unwrap_or_default())
    }

    /// Parse TRC20 transfer data (ABI encoded)
    /// Returns (to_address, amount) if successful
    pub fn parse_trc20_transfer_data(&self, data_hex: &str) -> Option<(String, u64)> {
        use alloy_sol_types::{sol, SolCall};

        sol! {
            function transfer(address to, uint256 amount) external returns (bool);
        }

        let data = hex::decode(data_hex).ok()?;
        let call = transferCall::abi_decode(&data, true).ok()?;

        // Convert EVM address to TRON Base58
        let evm_bytes: [u8; 20] = call.to.0 .0;
        let tron_addr = address::evm_to_tron(&evm_bytes);

        // Convert U256 to u64 (safe for USDT which fits in u64)
        let amount = call.amount.try_into().ok()?;

        Some((tron_addr, amount))
    }

    /// Get all events from a specific block (with pagination)
    ///
    /// Fetches all events from the block, handling pagination via fingerprint.
    /// Returns only confirmed events by default.
    pub async fn get_block_events(
        &self,
        block_number: i64,
        only_confirmed: bool,
    ) -> Result<Vec<types::BlockEvent>> {
        use types::BlockEventsResponse;

        let mut all_events = Vec::new();
        let mut fingerprint: Option<String> = None;

        loop {
            // Build URL with pagination
            let mut url = format!(
                "{}/v1/blocks/{}/events?limit=200&only_confirmed={}",
                self.full_node, block_number, only_confirmed
            );

            if let Some(ref fp) = fingerprint {
                url.push_str(&format!("&fingerprint={}", fp));
            }

            debug!(url = %url, "Fetching block events");

            let resp: BlockEventsResponse = self.http.get(&url).send().await?.json().await?;

            if !resp.success {
                anyhow::bail!(
                    "Block events API returned success=false for block {}",
                    block_number
                );
            }

            all_events.extend(resp.data);

            // Check for next page
            fingerprint = resp.meta.and_then(|m| m.fingerprint);
            if fingerprint.is_none() {
                break;
            }
        }

        // 🚿 Normalize all addresses to Base58 (ACL Pattern)
        for event in &mut all_events {
            // 1. Normalize contract address
            if let Some(clean) = address::normalize_to_base58(&event.contract_address) {
                event.contract_address = clean;
            }

            // 2. Normalize result fields (to, from)
            // Use local scope to control borrow
            let mut updates = Vec::new();

            for (key, val) in &event.result {
                if key == "to" || key == "from" {
                    // Skip non-address values (e.g. numeric params from non-Transfer events)
                    // Valid TRON hex addresses are 40-42 chars; Base58 are 34 chars starting with 'T'
                    let is_potential_address =
                        (val.len() == 40 || val.len() == 42 || val.len() == 34)
                            && !val.chars().all(|c| c.is_ascii_digit());
                    if !is_potential_address {
                        continue;
                    }
                    debug!(
                        tx = %event.transaction_id,
                        field = %key,
                        original = %val,
                        "Normalizing address field"
                    );
                    if let Some(clean) = address::normalize_to_base58(val) {
                        debug!(
                            tx = %event.transaction_id,
                            field = %key,
                            original = %val,
                            normalized = %clean,
                            "Address normalized successfully"
                        );
                        updates.push((key.clone(), clean));
                    } else {
                        debug!(
                            tx = %event.transaction_id,
                            field = %key,
                            value = %val,
                            "Skipping non-address value in result field (normalize_to_base58 returned None)"
                        );
                    }
                }
            }

            for (key, clean) in updates {
                event.result.insert(key, clean);
            }
        }

        debug!(
            block = block_number,
            events_count = all_events.len(),
            "Fetched block events"
        );

        Ok(all_events)
    }
}

#[async_trait]
impl TronBroadcaster for TronClient {
    async fn get_usdt_balance(&self, address: &str) -> Result<u64> {
        self.get_usdt_balance(address).await.map(|b| b as u64)
    }

    async fn get_trc20_balance(&self, address: &str, token_contract: &str) -> Result<i64> {
        self.get_trc20_balance(address, token_contract).await
    }

    async fn get_trx_balance(&self, address: &str) -> Result<u64> {
        self.get_trx_balance(address).await
    }

    async fn build_trc20_transfer(
        &self,
        from: &str,
        to: &str,
        amount: u64,
        contract_address: &str,
    ) -> Result<UnsignedTransaction> {
        self.build_trc20_transfer(from, to, amount, contract_address)
            .await
    }

    fn sign_transaction(
        &self,
        tx: &UnsignedTransaction,
        private_key: &[u8],
    ) -> Result<SignedTransaction> {
        self.sign_transaction(tx, private_key)
    }

    async fn broadcast(&self, tx: &SignedTransaction) -> Result<BroadcastResult> {
        self.broadcast(tx).await
    }

    async fn build_trx_transfer(
        &self,
        from: &str,
        to: &str,
        amount_sun: u64,
    ) -> Result<UnsignedTransaction> {
        self.build_trx_transfer(from, to, amount_sun).await
    }

    async fn get_transaction_info(&self, tx_hash: &str) -> Result<Option<TransactionInfo>> {
        self.get_transaction_info(tx_hash).await
    }

    async fn get_transaction_by_id(&self, tx_hash: &str) -> Result<Option<SignedTransaction>> {
        self.get_transaction_by_id(tx_hash).await
    }

    async fn transaction_known_on_any_endpoint(&self, tx_hash: &str) -> Result<bool> {
        self.transaction_known_on_any_endpoint(tx_hash).await
    }

    async fn get_current_block(&self) -> Result<BlockInfo> {
        self.get_current_block().await
    }

    async fn get_account_resources(&self, address: &str) -> Result<interface::AccountResource> {
        self.get_account_resources(address).await
    }

    async fn estimate_energy(
        &self,
        owner_address: &str,
        contract_address: &str,
        function_selector: &str,
        parameter: &str,
    ) -> Result<i64> {
        self.estimate_energy(
            owner_address,
            contract_address,
            function_selector,
            parameter,
        )
        .await
    }
}

// ─── ChainClient Implementation ────────────────────────────────────────────
//
// Thin conversion layer: delegates to existing TronClient methods,
// wrapping return values in chain-agnostic types.

use crate::services::chain::traits::ChainClient;
use crate::services::chain::types::*;
use alloy_primitives::U256;

#[async_trait]
impl ChainClient for TronClient {
    async fn get_token_balance(&self, address: &str, _token_address: &str) -> Result<U256> {
        // TronClient.get_usdt_balance ignores token_address (uses self.usdt_contract)
        // Phase 2+: pass token_address for multi-token support
        let balance = self.get_usdt_balance(address).await?;
        // Clamp: i64 can theoretically be negative from malformed RPC response
        Ok(U256::from(balance.max(0) as u64))
    }

    async fn get_native_balance(&self, address: &str) -> Result<U256> {
        let balance = self.get_trx_balance(address).await?;
        Ok(U256::from(balance))
    }

    async fn build_token_transfer(
        &self,
        from: &str,
        to: &str,
        token_address: &str,
        amount: U256,
    ) -> Result<ChainUnsignedTx> {
        // Convert U256 to u64 (safe for TRON USDT: 6 decimals, max ~18.4 quintillion)
        let amount_u64: u64 = amount
            .try_into()
            .map_err(|_| anyhow::anyhow!("Amount {} exceeds u64 range for TRON", amount))?;
        let tx = self
            .build_trc20_transfer(from, to, amount_u64, token_address)
            .await?;
        Ok(ChainUnsignedTx::Tron(tx))
    }

    async fn build_native_transfer(
        &self,
        from: &str,
        to: &str,
        amount: U256,
    ) -> Result<ChainUnsignedTx> {
        let amount_u64: u64 = amount
            .try_into()
            .map_err(|_| anyhow::anyhow!("Amount {} exceeds u64 range for TRON", amount))?;
        let tx = self.build_trx_transfer(from, to, amount_u64).await?;
        Ok(ChainUnsignedTx::Tron(tx))
    }

    async fn broadcast(&self, tx: &ChainSignedTx) -> Result<ChainBroadcastResult> {
        match tx {
            ChainSignedTx::Tron(signed) => {
                let result = self.broadcast(signed).await?;
                Ok(result.into())
            }
            #[allow(unreachable_patterns)]
            _ => Err(anyhow::anyhow!(
                "TronClient cannot broadcast non-TRON transactions"
            )),
        }
    }

    async fn get_current_block(&self) -> Result<ChainBlockInfo> {
        let block = self.get_current_block().await?;
        Ok(block.into())
    }

    async fn get_transaction_info(&self, tx_hash: &str) -> Result<Option<ChainTransactionInfo>> {
        let info = self.get_transaction_info(tx_hash).await?;
        Ok(info.map(|i| i.into()))
    }

    async fn get_transaction_by_id(&self, tx_hash: &str) -> Result<Option<ChainSignedTx>> {
        let tx = self.get_transaction_by_id(tx_hash).await?;
        Ok(tx.map(ChainSignedTx::Tron))
    }

    async fn validate_payment_tx(
        &self,
        tx_hash: &str,
        expected_pay_address: &str,
        token_contract: &str,
    ) -> bool {
        // 1. Fetch the raw transaction
        let tx = match self.get_transaction_by_id(tx_hash).await {
            Ok(Some(tx)) => tx,
            _ => return false,
        };

        // 2. Parse raw_data_json to extract contract call info
        let raw_data = match &tx.raw_data_json {
            Some(v) => v,
            None => return false,
        };

        // Navigate: raw_data.contract[0].parameter.value
        let value = match raw_data
            .get("contract")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("parameter"))
            .and_then(|p| p.get("value"))
        {
            Some(v) => v,
            None => return false,
        };

        // 3. Verify contract_address matches USDT contract (hex form)
        let contract_hex = match value.get("contract_address").and_then(|v| v.as_str()) {
            Some(addr) => addr,
            None => return false,
        };
        // Convert token_contract (Base58) to hex for comparison
        let expected_hex = match address::to_hex(token_contract) {
            Ok(hex) => hex,
            Err(_) => return false,
        };
        if !contract_hex.eq_ignore_ascii_case(&expected_hex) {
            return false;
        }

        // 4. Decode ABI data to extract the `to` address
        let data_hex = match value.get("data").and_then(|v| v.as_str()) {
            Some(d) => d,
            None => return false,
        };
        let (to_address, _amount) = match self.parse_trc20_transfer_data(data_hex) {
            Some(result) => result,
            None => return false,
        };

        // 5. Compare decoded to_address with expected pay_address
        to_address == expected_pay_address
    }
}
