//! Chain-agnostic types using enum dispatch.
//!
//! Uses enums instead of trait objects for zero-overhead dispatch.
//! Each new chain adds a variant here (requires recompile, but compile-time safe).

/// RPC endpoint status for observability (admin console).
#[derive(Debug, Clone, serde::Serialize)]
pub struct RpcStatus {
    /// Provider name extracted from URL hostname (e.g., "Alchemy", "Ankr")
    pub provider: String,
    /// Whether currently using a fallback endpoint (active_index > 0)
    pub is_fallback: bool,
    /// Total configured endpoints
    pub endpoint_count: usize,
    /// Masked endpoint identifier, e.g. "Alchemy (…QCkJ)" or "mainnet.base.org"
    pub active_endpoint: String,
}

use crate::services::tron::interface;

/// Unsigned transaction envelope (chain-specific internals hidden behind enum).
#[derive(Debug, Clone)]
pub enum ChainUnsignedTx {
    Tron(interface::UnsignedTransaction),
    Evm(EvmUnsignedTx),
    Solana(SolanaUnsignedTx),
}

/// Signed transaction envelope.
#[derive(Debug, Clone)]
pub enum ChainSignedTx {
    Tron(interface::SignedTransaction),
    Evm(EvmSignedTx),
    Solana(SolanaSignedTx),
}

/// Solana unsigned transaction (pre-signing).
///
/// Contains the serialized message bytes and metadata needed for signing.
/// Unlike EVM (single signer), Solana supports multiple signers per tx
/// (e.g., sweep: from_address signs transfer authority, treasury signs as fee_payer).
#[derive(Debug, Clone)]
pub struct SolanaUnsignedTx {
    /// Serialized Solana Message bytes (for signing)
    pub message_bytes: Vec<u8>,
    /// Recent blockhash (validity window ~60s / ~150 slots)
    pub recent_blockhash: String,
    /// Last block height at which the signed transaction may be accepted.
    pub last_valid_block_height: u64,
    /// Number of required signatures
    pub num_required_signatures: u8,
    /// Ordered signer pubkeys — fee_payer MUST be first.
    /// For sweep: [treasury (fee_payer), from_address (authority)]
    /// For payout: [treasury]
    pub signer_pubkeys: Vec<String>,
}

/// Solana signed transaction (broadcast-ready).
#[derive(Debug, Clone)]
pub struct SolanaSignedTx {
    /// Transaction signature (Base58-encoded, also serves as tx_hash)
    pub signature: String,
    /// Serialized signed transaction (Base64)
    pub serialized_tx: String,
}

/// EVM unsigned transaction (pre-signing).
/// Contains all fields needed for EIP-155 RLP encoding.
#[derive(Debug, Clone)]
pub struct EvmUnsignedTx {
    pub from: String,
    pub to: String,
    pub data: String,  // Hex-encoded calldata (0x-prefixed)
    pub value: String, // Hex-encoded value in wei (0x-prefixed)
    pub nonce: u64,
    pub gas_price: u64, // In wei
    pub gas_limit: u64,
    pub chain_id: u64,
}

/// EVM signed transaction (ready to broadcast).
#[derive(Debug, Clone)]
pub struct EvmSignedTx {
    pub tx_hash: String,
    pub raw_tx_hex: String, // RLP-encoded signed tx (0x-prefixed)
}

/// Block information from any chain.
#[derive(Debug, Clone)]
pub struct ChainBlockInfo {
    pub number: u64,
    pub timestamp: i64,
}

/// Result of broadcasting a transaction.
#[derive(Debug, Clone)]
pub struct ChainBroadcastResult {
    pub success: bool,
    pub tx_hash: String,
    pub message: Option<String>,
}

/// On-chain transaction information.
#[derive(Debug, Clone)]
pub struct ChainTransactionInfo {
    pub tx_hash: String,
    pub block_number: i64,
    pub success: bool,
    /// Chain-specific status code (e.g., "OUT_OF_ENERGY" on TRON, revert reason on EVM)
    pub result: Option<String>,
    /// Fee burned in native token's smallest unit
    pub fee_burned: i64,
    /// Human-readable revert message (if failed)
    pub revert_message: Option<String>,
}

// ─── Conversion Helpers ─────────────────────────────────────────────────────

impl From<interface::BlockInfo> for ChainBlockInfo {
    fn from(b: interface::BlockInfo) -> Self {
        Self {
            number: b.number,
            timestamp: b.timestamp,
        }
    }
}

impl From<interface::BroadcastResult> for ChainBroadcastResult {
    fn from(r: interface::BroadcastResult) -> Self {
        Self {
            success: r.success,
            tx_hash: r.tx_hash,
            message: r.message,
        }
    }
}

impl From<interface::TransactionInfo> for ChainTransactionInfo {
    fn from(t: interface::TransactionInfo) -> Self {
        Self {
            tx_hash: t.tx_hash,
            block_number: t.block_number,
            success: t.success,
            result: t.result,
            fee_burned: t.fee_burned,
            revert_message: t.revert_message,
        }
    }
}
