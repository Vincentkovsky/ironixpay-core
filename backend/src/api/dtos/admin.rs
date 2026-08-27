//! Admin Portal DTOs
//!
//! Response structures for the admin-only API endpoints.
//! These are NOT merchant-scoped — they provide platform-wide views.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use validator::Validate;

/// Dashboard overview statistics — platform-wide aggregates.
#[derive(Serialize)]
pub struct AdminDashboardStats {
    pub total_merchants: u64,
    pub active_merchants: u64,
    pub active_sessions: u64,
    pub total_volume_24h: String,
    pub global_liability: String,
    pub pending_exceptions: u64,
    pub pending_withdrawals: u64,
    pub pending_payouts: u64,
    pub treasury_balance: Option<String>,
    pub treasury_address: Option<String>,
}

/// Extended system health check — beyond basic /health.
#[derive(Serialize)]
pub struct AdminSystemHealth {
    pub database: bool,
    /// Per-chain RPC health: e.g. {"TRON": true, "BSC": true, "ETHEREUM": false}
    pub chain_rpc: HashMap<String, bool>,
    pub indexer: Vec<IndexerProgress>,
    pub address_pool: HashMap<String, AddressPoolStats>,
    /// Background service heartbeat status: e.g. {"tron_sweeper": "healthy"}
    pub services: HashMap<String, String>,
}

/// Indexer block sync progress per network.
#[derive(Serialize)]
pub struct IndexerProgress {
    pub network: String,
    pub last_processed_block: i64,
    pub chain_head_block: Option<i64>,
    pub blocks_behind: Option<i64>,
    pub updated_at: String,
    /// Active RPC provider name (e.g., "Alchemy", "Ankr"). None for TRON.
    pub active_rpc: Option<String>,
    /// Whether currently on a fallback RPC endpoint
    pub is_fallback: Option<bool>,
    /// Masked endpoint identifier, e.g. "Alchemy (…QCkJ)". None for TRON.
    pub active_endpoint: Option<String>,
}

/// Global address pool statistics (all merchants combined).
#[derive(Serialize)]
pub struct AddressPoolStats {
    pub total: u64,
    pub idle: u64,
    pub assigned: u64,
    pub detected: u64,
    pub sweeping: u64,
    pub cooling: u64,
    pub locked: u64,
    pub error: u64,
}

/// Summary of a merchant (organization) for list views.
#[derive(Serialize)]
pub struct MerchantSummary {
    pub id: String,
    /// Organization display name
    pub name: String,
    /// Owner user's email
    pub email: String,
    /// Owner user's display name
    pub owner_name: Option<String>,
    pub status: String,
    pub is_totp_enabled: bool,
    pub email_verified: bool,
    /// Number of active members in this organization
    pub member_count: u64,
    pub created_at: String,
}

/// Per-chain account balance summary.
#[derive(Serialize)]
pub struct ProfileSummary {
    pub environment: String,
    pub network: String,
    pub balance: String,
}

/// Organization member info for admin detail view.
#[derive(Serialize)]
pub struct MemberInfo {
    /// Membership ID (unique per org_members row)
    pub id: String,
    pub user_id: Option<String>,
    /// User email (accepted) or invited_email (pending)
    pub email: String,
    pub name: Option<String>,
    pub role: String,
    pub status: String,
    pub joined_at: Option<String>,
}

/// Detailed merchant view (single merchant panorama).
#[derive(Serialize)]
pub struct AdminMerchantDetail {
    pub merchant: MerchantSummary,
    pub profiles: Vec<ProfileSummary>,
    pub api_key_count: u64,
    pub address_stats: AddressPoolStats,
    pub total_sessions: u64,
    pub active_sessions: u64,
    /// Custom fee percentage (decimal fraction). None = global default.
    /// e.g. "0.0050" = 0.5%
    pub custom_fee_percentage: Option<String>,
    /// Effective fee percentage as a display string (e.g. "0.10" for 0.1%).
    /// Resolved by backend: custom if set, otherwise global default.
    pub effective_fee_percentage: String,
    /// All organization members
    pub members: Vec<MemberInfo>,
}

/// Request to update a merchant's custom fee percentage.
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateMerchantFeeRequest {
    /// Custom fee percentage as decimal fraction (0.005 = 0.5%).
    /// Set to null to revert to global default.
    pub custom_fee_percentage: Option<f64>,
}

/// Generic list response for admin entity browsing.
#[derive(Serialize)]
pub struct AdminSweepResponse {
    pub id: String,
    pub merchant_id: String,
    pub session_id: Option<String>,
    pub sweep_type: String,
    pub network: String,
    pub from_address: String,
    pub to_address: String,
    pub tx_hash: Option<String>,
    pub amount: String,
    pub cost_in_usdt: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: String,
}

/// Admin withdrawal list item.
#[derive(Serialize)]
pub struct AdminWithdrawalResponse {
    pub id: String,
    pub merchant_id: String,
    pub environment: String,
    pub network: String,
    pub amount: String,
    pub network_fee: String,
    pub net_amount: String,
    pub to_address: String,
    pub status: String,
    pub tx_hash: Option<String>,
    pub error_reason: Option<String>,
    pub currency: String,
    pub created_at: String,
}

/// Admin payout list item.
#[derive(Serialize)]
pub struct AdminPayoutResponse {
    pub id: String,
    pub merchant_id: String,
    pub environment: String,
    pub network: String,
    pub amount: String,
    pub fee: String,
    pub net_amount: String,
    pub to_address: String,
    pub status: String,
    pub tx_hash: Option<String>,
    pub description: Option<String>,
    pub error_reason: Option<String>,
    pub currency: String,
    pub created_at: String,
    pub completed_at: Option<String>,
}

/// Admin billing log item.
#[derive(Serialize)]
pub struct AdminBillingLogResponse {
    pub id: String,
    pub environment: String,
    pub network: String,
    pub merchant_id: String,
    pub session_id: Option<String>,
    pub external_ref_id: Option<String>,
    pub billing_type: String,
    pub previous_balance: String,
    pub amount_change: String,
    pub balance_after: String,
    pub description: Option<String>,
    pub created_at: String,
}

/// Active database query from pg_stat_activity.
#[derive(Serialize)]
pub struct ActiveQuery {
    pub pid: i32,
    pub duration_seconds: f64,
    pub duration_display: String,
    pub state: String,
    pub query: String,
    pub client_addr: Option<String>,
    pub application_name: String,
    pub wait_event_type: Option<String>,
}

/// Result of a query kill operation.
#[derive(Serialize)]
pub struct KillQueryResponse {
    pub pid: i32,
    pub terminated: bool,
}

/// Admin transaction list/detail item.
#[derive(Serialize)]
pub struct AdminTransactionResponse {
    pub network: String,
    pub tx_hash: String,
    pub log_index: i32,
    pub session_id: Option<String>,
    pub merchant_id: String,
    pub currency_symbol: String,
    pub from_address: String,
    pub to_address: String,
    pub amount: String,
    pub status: String,
    pub confirmations_count: i32,
    pub block_number: i64,
    pub block_timestamp: String,
    pub is_credited: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Admin payment event list/detail item.
#[derive(Serialize)]
pub struct AdminPaymentEventResponse {
    pub id: String,
    pub event_type: String,
    pub session_id: String,
    pub tx_network: String,
    pub tx_hash: String,
    pub tx_log_index: i32,
    pub amount: String,
    pub status: String,
    pub attempt_count: i32,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub processed_at: Option<String>,
}

/// Admin address list item.
#[derive(Serialize)]
pub struct AdminAddressResponse {
    pub network: String,
    pub address: String,
    pub merchant_id: String,
    pub status: String,
    pub usdt_balance: String,
    pub usdc_balance: String,
    pub native_balance: String,
    pub sweep_attempts: i32,
    pub error_reason: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Full treasury overview — balance, reconciliation, alerting, and history.
#[derive(Serialize)]
pub struct TreasuryOverview {
    /// On-chain USDT balance (formatted, e.g. "1234.56")
    pub balance: Option<String>,
    /// Platform treasury address
    pub address: String,
    /// Whether the balance is below the configured threshold
    pub low_balance_alert: bool,
    /// Configured low-balance threshold (formatted USDT)
    pub threshold: String,
    // ── Reconciliation ──
    /// Sum of all confirmed sweep amounts (USDT into treasury)
    pub total_swept_in: String,
    /// Sum of all completed withdrawal net_amounts (USDT out of treasury)
    pub total_paid_out: String,
    /// Expected balance = swept_in - paid_out
    pub expected_balance: String,
    /// Discrepancy = expected - actual, or "N/A" if balance unavailable
    pub discrepancy: String,
    // ── Recent history ──
    /// Last N inflow/outflow transactions
    pub recent_transactions: Vec<TreasuryTransaction>,
}

/// A single treasury inflow or outflow record.
#[derive(Serialize)]
pub struct TreasuryTransaction {
    /// "in" (sweep → treasury) or "out" (treasury → merchant)
    pub direction: String,
    /// e.g. "auto_sweep", "manual_sweep", "withdrawal"
    pub tx_type: String,
    /// Formatted USDT amount
    pub amount: String,
    /// Merchant ID associated with the transaction
    pub merchant_id: String,
    /// On-chain tx hash (if available)
    pub tx_hash: Option<String>,
    /// ISO 8601 timestamp
    pub created_at: String,
}

/// Platform wallets overview — treasury and gas sponsor per chain.
#[derive(Serialize)]
pub struct PlatformWalletsResponse {
    pub wallets: Vec<ChainWallet>,
}

/// Single chain's platform wallet info.
#[derive(Serialize)]
pub struct ChainWallet {
    /// Chain identifier (e.g. "TRON", "BSC")
    pub chain: String,
    /// Treasury address for this chain
    pub treasury_address: String,
    /// USDT balance of treasury (formatted, e.g. "1234.560000")
    pub treasury_usdt_balance: Option<String>,
    /// USDC balance of treasury (formatted, e.g. "567.890000"). None if USDC not supported on chain.
    pub treasury_usdc_balance: Option<String>,
    /// Gas sponsor address for this chain
    pub gas_sponsor_address: String,
    /// Native token balance of gas sponsor (formatted, e.g. "450.123456")
    pub gas_sponsor_native_balance: Option<String>,
    /// Native token symbol (e.g. "TRX", "BNB")
    pub native_symbol: String,
    /// Whether gas sponsor balance is below threshold (computed server-side)
    pub gas_sponsor_low_balance: bool,
}
