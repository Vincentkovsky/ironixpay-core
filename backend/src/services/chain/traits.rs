//! Chain-agnostic trait definitions.
//!
//! `ChainClient` — blockchain read/write operations (query, build, broadcast).
//! `ChainSigner` — pure cryptographic signing (chain-agnostic secp256k1).
//!
//! Design decisions:
//! - `sign_transaction` is NOT on ChainClient (signing is a separate concern)
//! - Amounts use `U256` to safely handle 18-decimal EVM tokens (BSC USDT)
//! - `token_address` is explicit to support multi-token (USDT + USDC) in future

use alloy_primitives::U256;
use anyhow::Result;
use async_trait::async_trait;

use super::types::*;

/// Chain-agnostic blockchain client interface.
///
/// Each chain implementation (TRON, EVM, Solana) implements this trait.
/// Handles all network interactions: querying state, building transactions,
/// and broadcasting signed transactions.
///
/// # Responsibilities
/// - Query token/native balances
/// - Build unsigned transactions
/// - Broadcast signed transactions
/// - Query block and transaction status
///
/// # NOT Responsible For
/// - Signing transactions (see `TransactionSigner` in `key_provider.rs`)
/// - Resource estimation (chain-specific: EnergyManager for TRON, GasEstimator for EVM)
#[async_trait]
pub trait ChainClient: Send + Sync {
    /// Get token balance for an address.
    ///
    /// Returns balance in the token's smallest unit.
    /// For TRON USDT (6 decimals): 1 USDT = 1_000_000
    /// For BSC USDT (18 decimals): 1 USDT = 1_000_000_000_000_000_000
    async fn get_token_balance(&self, address: &str, token_address: &str) -> Result<U256>;

    /// Get native token balance (TRX / ETH / BNB).
    ///
    /// Returns balance in the chain's smallest unit (SUN / Wei).
    async fn get_native_balance(&self, address: &str) -> Result<U256>;

    /// Build an unsigned token transfer transaction.
    async fn build_token_transfer(
        &self,
        from: &str,
        to: &str,
        token_address: &str,
        amount: U256,
    ) -> Result<ChainUnsignedTx>;

    /// Build an unsigned native token transfer (for gas funding).
    async fn build_native_transfer(
        &self,
        from: &str,
        to: &str,
        amount: U256,
    ) -> Result<ChainUnsignedTx>;

    /// Broadcast a signed transaction to the network.
    async fn broadcast(&self, tx: &ChainSignedTx) -> Result<ChainBroadcastResult>;

    /// Get current block info (number + timestamp).
    async fn get_current_block(&self) -> Result<ChainBlockInfo>;

    /// Get on-chain transaction info (confirmation status, success/failure).
    ///
    /// Returns `None` if the transaction is not found.
    async fn get_transaction_info(&self, tx_hash: &str) -> Result<Option<ChainTransactionInfo>>;

    /// Check if a signed transaction exists (in mempool or on chain).
    ///
    /// Returns `None` if the transaction is not found anywhere.
    async fn get_transaction_by_id(&self, tx_hash: &str) -> Result<Option<ChainSignedTx>>;

    /// Get RPC endpoint status (active provider, failover state).
    /// Returns None for clients without failover support (e.g., TRON).
    fn rpc_status(&self) -> Option<RpcStatus> {
        None
    }

    /// Validate that a transaction is a token transfer TO the expected address.
    ///
    /// Used by the `notify-payment` endpoint for anti-griefing: prevents
    /// malicious users from faking payment detection by submitting arbitrary
    /// transaction hashes. Verifies:
    /// 1. Transaction exists on-chain
    /// 2. It interacts with the correct token contract
    /// 3. The decoded `to` address matches `expected_pay_address`
    ///
    /// Default: returns `false` (unimplemented chains reject all notifications).
    async fn validate_payment_tx(
        &self,
        tx_hash: &str,
        expected_pay_address: &str,
        token_contract: &str,
    ) -> bool {
        let _ = (tx_hash, expected_pay_address, token_contract);
        false
    }
}

/// Chain-agnostic transaction signer.
///
/// Pure cryptographic operation: takes a 32-byte message digest and
/// produces an ECDSA signature. The hash algorithm (SHA-256 for TRON,
/// Keccak-256 for EVM) is the caller's responsibility.
///
/// This trait is defined here for documentation purposes but the active
/// implementation lives in `services::address::key_provider::TransactionSigner`.
/// It will be migrated to use `message_hash` instead of `raw_bytes` in Phase 2
/// when we have two chain implementations to validate the design.
#[async_trait]
pub trait ChainSigner: Send + Sync {
    /// Sign a 32-byte message digest with the key at the given derivation path.
    ///
    /// # Arguments
    /// * `message_hash` - 32-byte digest (SHA-256 for TRON, Keccak-256 for EVM)
    /// * `account_index` - Merchant's account index (hardened derivation)
    /// * `path_index` - Address index within merchant
    /// * `coin_type` - BIP44 coin type (195 for TRON, 60 for EVM)
    ///
    /// # Returns
    /// 65-byte signature (R || S || V)
    async fn sign_digest(
        &self,
        message_hash: &[u8; 32],
        account_index: i32,
        path_index: u32,
        coin_type: u32,
    ) -> Result<Vec<u8>>;
}
