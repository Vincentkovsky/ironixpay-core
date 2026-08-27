//! Solana JSON-RPC response types for deserialization.
//!
//! These types map directly to the Solana JSON-RPC API responses.
//! Reference: https://solana.com/docs/rpc

use serde::Deserialize;

// ─── Generic RPC Wrapper ────────────────────────────────────────────────────

/// Standard Solana RPC response envelope.
/// Most RPC methods return `{ "context": { "slot": N }, "value": T }`.
#[derive(Deserialize, Debug)]
pub struct RpcResponse<T> {
    pub context: RpcContext,
    pub value: T,
}

#[derive(Deserialize, Debug)]
pub struct RpcContext {
    pub slot: u64,
}

// ─── Balance Types ──────────────────────────────────────────────────────────

/// Response from `getTokenAccountBalance`.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TokenAmount {
    /// Raw amount as string (to avoid u64 overflow in JSON)
    pub amount: String,
    /// Token decimals
    pub decimals: u8,
    /// Human-readable amount (e.g., "1.5")
    #[allow(dead_code)]
    pub ui_amount: Option<f64>,
}

/// Token account info from `getTokenAccountsByOwner` (fallback usage).
#[derive(Deserialize, Debug)]
pub struct TokenAccountInfo {
    pub pubkey: String,
    pub account: TokenAccountData,
}

#[derive(Deserialize, Debug)]
pub struct TokenAccountData {
    pub data: TokenAccountParsed,
}

#[derive(Deserialize, Debug)]
pub struct TokenAccountParsed {
    pub parsed: TokenAccountParsedInfo,
}

#[derive(Deserialize, Debug)]
pub struct TokenAccountParsedInfo {
    pub info: TokenAccountDetails,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TokenAccountDetails {
    pub mint: String,
    pub owner: String,
    pub token_amount: TokenAmount,
}

// ─── Signature Types ────────────────────────────────────────────────────────

/// Entry from `getSignaturesForAddress`.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SignatureInfo {
    /// Transaction signature (Base58)
    pub signature: String,
    /// Slot the transaction was processed in
    pub slot: u64,
    /// Block time (Unix timestamp), None if not available
    pub block_time: Option<i64>,
    /// Error info, None if transaction succeeded
    pub err: Option<serde_json::Value>,
    /// Optional memo
    pub memo: Option<String>,
    /// Confirmation status
    pub confirmation_status: Option<String>,
}

/// Signature status from `getSignatureStatuses`.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SignatureStatus {
    pub slot: u64,
    pub confirmations: Option<u64>,
    pub err: Option<serde_json::Value>,
    pub confirmation_status: Option<String>,
}

// ─── Transaction Types ──────────────────────────────────────────────────────

/// Response from `getTransaction`.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TransactionResponse {
    /// Slot the transaction was processed in
    pub slot: u64,
    /// Block time (Unix timestamp)
    pub block_time: Option<i64>,
    /// Transaction metadata
    pub meta: Option<TransactionMeta>,
    /// The transaction itself
    pub transaction: TransactionData,
}

/// Transaction metadata (fees, balances, logs).
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TransactionMeta {
    /// Transaction fee in lamports
    pub fee: u64,
    /// Error info, None if successful
    pub err: Option<serde_json::Value>,
    /// Pre-execution token balances
    pub pre_token_balances: Option<Vec<TokenBalance>>,
    /// Post-execution token balances
    pub post_token_balances: Option<Vec<TokenBalance>>,
    /// Log messages
    pub log_messages: Option<Vec<String>>,
}

/// Token balance entry in transaction metadata.
/// Used by Indexer to detect SPL Token transfers by comparing
/// pre/post balances for monitored addresses.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TokenBalance {
    /// Index into the transaction's account keys array
    pub account_index: u8,
    /// Token mint address
    pub mint: String,
    /// Owner of the token account
    pub owner: Option<String>,
    /// Token amount
    pub ui_token_amount: TokenAmount,
}

/// Transaction data (can be JSON-parsed or binary).
/// We request `jsonParsed` encoding for human-readable output.
#[derive(Deserialize, Debug)]
pub struct TransactionData {
    /// Transaction signatures (first = canonical tx signature)
    pub signatures: Vec<String>,
    /// The transaction message
    pub message: TransactionMessage,
}

/// Transaction message with account keys and instructions.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TransactionMessage {
    /// Account keys involved in the transaction
    pub account_keys: Vec<AccountKey>,
}

/// Account key in a transaction message.
/// When using `jsonParsed` encoding, this is an object with pubkey + signer/writable flags.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AccountKey {
    pub pubkey: String,
    pub signer: bool,
    pub writable: bool,
}

// ─── Blockhash Types ────────────────────────────────────────────────────────

/// Response value from `getLatestBlockhash`.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BlockhashResponse {
    pub blockhash: String,
    pub last_valid_block_height: u64,
}

// ─── Priority Fee Types ─────────────────────────────────────────────────────

/// Response from `getRecentPrioritizationFees`.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PrioritizationFee {
    /// Slot of the fee sample
    pub slot: u64,
    /// Priority fee in micro-lamports per compute unit
    pub prioritization_fee: u64,
}
