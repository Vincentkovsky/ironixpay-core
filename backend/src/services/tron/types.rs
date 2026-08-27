//! Tron API request/response types
//!
//! Based on official documentation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Common
// ============================================================================

/// API result wrapper
#[derive(Debug, Clone, Deserialize)]
pub struct ApiResult {
    pub result: bool,
    pub message: Option<String>,
    pub code: Option<String>,
}

/// Transaction raw data (simplified)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransactionRawData {
    pub contract: Vec<TransactionContract>,
    pub ref_block_bytes: String,
    pub ref_block_hash: String,
    pub expiration: i64,
    pub timestamp: Option<i64>,
    pub fee_limit: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransactionContract {
    pub parameter: ContractParameter,
    #[serde(rename = "type")]
    pub contract_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContractParameter {
    pub value: serde_json::Value,
    pub type_url: String,
}

// ============================================================================
// Wallet API
// ============================================================================

// --- GetAccount ---

#[derive(Debug, Clone, Serialize)]
pub struct GetAccountRequest {
    pub address: String,
    pub visible: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetAccountResponse {
    #[serde(default)]
    pub balance: i64,
    #[serde(default)]
    pub address: String,
    // Add other fields as needed
}

// --- TriggerConstantContract ---

#[derive(Debug, Clone, Serialize)]
pub struct TriggerConstantContractRequest {
    pub owner_address: String,
    pub contract_address: String,
    pub function_selector: String,
    pub parameter: String,
    pub visible: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TriggerConstantContractResponse {
    pub result: ApiResult,
    #[serde(default)]
    pub energy_used: i64,
    #[serde(default)]
    pub energy_penalty: i64,
    #[serde(default)]
    pub constant_result: Vec<String>,
}

// --- TriggerSmartContract ---

#[derive(Debug, Clone, Serialize)]
pub struct TriggerSmartContractRequest {
    pub owner_address: String,
    pub contract_address: String,
    pub function_selector: String,
    pub parameter: String,
    pub fee_limit: i64,
    pub call_value: i64,
}

// --- CreateTransaction (TRX Transfer) ---

#[derive(Debug, Clone, Serialize)]
pub struct CreateTransactionRequest {
    pub owner_address: String,
    pub to_address: String,
    pub amount: u64,
    pub visible: Option<bool>,
}

// --- Broadcast ---

#[derive(Debug, Clone, Serialize)]
pub struct BroadcastTransactionRequest {
    pub raw_data_hex: String,
    pub signature: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BroadcastTransactionResponse {
    pub result: Option<bool>,
    pub txid: Option<String>,
    pub code: Option<String>,
    pub message: Option<String>,
}

// --- TransactionInfo ---

#[derive(Debug, Clone, Deserialize)]
pub struct TransactionInfoResponse {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub fee: i64,
    #[serde(rename = "blockNumber", default)]
    pub block_number: i64,
    #[serde(rename = "blockTimeStamp", default)]
    pub block_timestamp: i64,
    #[serde(rename = "contractResult")]
    pub contract_result: Option<Vec<String>>,
    #[serde(default)]
    pub receipt: ResourceReceipt,
    pub log: Option<Vec<TransactionLog>>,
    pub result: Option<String>, // "FAILED" or null
    #[serde(rename = "resMessage")]
    pub res_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ResourceReceipt {
    pub energy_usage: Option<i64>,
    pub energy_fee: Option<i64>,
    pub net_usage: Option<i64>,
    pub net_fee: Option<i64>,
    pub result: Option<String>, // "SUCCESS" or "OUT_OF_ENERGY" etc.
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransactionLog {
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
}

// --- Account Resource ---

/// TronGrid returns asset net entries as [{key, value}] arrays, not maps.
#[derive(Debug, Clone, Deserialize)]
pub struct AssetNetEntry {
    pub key: String,
    pub value: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetAccountResourceResponse {
    #[serde(rename = "freeNetUsed", default)]
    pub free_net_used: i64,
    #[serde(rename = "freeNetLimit", default)]
    pub free_net_limit: i64,
    #[serde(rename = "NetUsed", default)]
    pub net_used: i64,
    #[serde(rename = "NetLimit", default)]
    pub net_limit: i64,
    #[serde(rename = "EnergyUsed", default)]
    pub energy_used: i64,
    #[serde(rename = "EnergyLimit", default)]
    pub energy_limit: i64,

    // Global limits (useful for calculating burned resource cost)
    #[serde(rename = "TotalNetLimit", default)]
    pub total_net_limit: i64,
    #[serde(rename = "TotalNetWeight", default)]
    pub total_net_weight: i64,
    #[serde(rename = "TotalEnergyLimit", default)]
    pub total_energy_limit: i64,
    #[serde(rename = "TotalEnergyWeight", default)]
    pub total_energy_weight: i64,

    /// TronGrid returns [{key: "token_id", value: N}], not a flat map.
    #[serde(rename = "assetNetUsed", default)]
    pub asset_net_used: Vec<AssetNetEntry>,

    #[serde(rename = "assetNetLimit", default)]
    pub asset_net_limit: Vec<AssetNetEntry>,
}

// --- Estimate Energy ---

#[derive(Debug, Clone, Serialize)]
pub struct EstimateEnergyRequest {
    pub owner_address: String,
    pub contract_address: String,
    pub function_selector: String,
    pub parameter: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EstimateEnergyResponse {
    pub result: ApiResult,
    pub energy_required: i64,
}

// ============================================================================
// TronGrid / Explorer API
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct TRC20TransactionsResponse {
    #[serde(default)]
    pub data: Vec<TRC20Transaction>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TRC20Transaction {
    pub transaction_id: String,
    pub block_timestamp: i64,
    pub from: String,
    pub to: String,
    pub value: String,
    #[serde(rename = "type")]
    pub tx_type: Option<String>,
    pub token_info: Option<TokenInfo>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TokenInfo {
    pub symbol: String,
    pub address: String,
    pub decimals: i32,
    pub name: String,
}

// ============================================================================
// Block API
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct BlockResponse {
    pub block_header: BlockHeader,
}

#[derive(Debug, Deserialize)]
pub struct BlockHeader {
    pub raw_data: BlockRawData,
}

#[derive(Debug, Deserialize)]
pub struct BlockRawData {
    pub number: i64,
    pub timestamp: i64,
}

#[derive(Debug, Deserialize)]
pub struct BlockTransactionsResponse {
    #[serde(default)]
    pub transactions: Option<Vec<RawTransaction>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RawTransaction {
    #[serde(rename = "txID")]
    pub tx_id: String,
    pub raw_data: RawData,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RawData {
    pub contract: Vec<Contract>,
    pub timestamp: Option<i64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Contract {
    pub parameter: Parameter,
    #[serde(rename = "type")]
    pub contract_type: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Parameter {
    pub value: serde_json::Value,
}

// ============================================================================
// Block Events API (TronGrid)
// ============================================================================

/// Response from GET /v1/blocks/{block_number}/events
#[derive(Debug, Deserialize)]
pub struct BlockEventsResponse {
    pub success: bool,
    #[serde(default)]
    pub data: Vec<BlockEvent>,
    pub meta: Option<EventsMeta>,
}

/// Individual event from a block
#[derive(Debug, Deserialize, Clone)]
pub struct BlockEvent {
    pub block_number: i64,
    pub block_timestamp: i64,
    /// Contract address that emitted the event (hex format, e.g., "41...")
    pub contract_address: String,
    /// Event name (e.g., "Transfer")
    pub event_name: String,
    /// Event index within the transaction
    pub event_index: i32,
    /// Transaction hash
    pub transaction_id: String,
    /// Event parameters: {from, to, value} for Transfer events
    /// Note: Addresses may be in hex or base58 format depending on TronGrid version
    #[serde(default)]
    pub result: HashMap<String, String>,
    /// Whether the transaction is unconfirmed
    #[serde(rename = "_unconfirmed", default)]
    pub unconfirmed: bool,
}

/// Pagination metadata for events API
#[derive(Debug, Deserialize, Clone)]
pub struct EventsMeta {
    pub page_size: Option<i32>,
    /// Fingerprint for next page, None if no more pages
    pub fingerprint: Option<String>,
    /// Response timestamp (milliseconds)
    pub at: Option<i64>,
}
