use anyhow::Result;
use async_trait::async_trait;

/// Tron Blockchain Client Trait
///
/// Abstracts Tron network operations for testability.
#[async_trait]
pub trait TronBroadcaster: Send + Sync {
    /// Get TRC20 USDT balance for an address
    async fn get_usdt_balance(&self, address: &str) -> Result<u64>;

    /// Get a TRC20 token balance for an address using the requested contract.
    async fn get_trc20_balance(&self, address: &str, token_contract: &str) -> Result<i64>;

    /// Get TRX balance for an address
    async fn get_trx_balance(&self, address: &str) -> Result<u64>;

    /// Build a TRC20 transfer transaction
    async fn build_trc20_transfer(
        &self,
        from: &str,
        to: &str,
        amount: u64,
        contract_address: &str,
    ) -> Result<UnsignedTransaction>;

    /// Sign a transaction with the given private key
    fn sign_transaction(
        &self,
        tx: &UnsignedTransaction,
        private_key: &[u8],
    ) -> Result<SignedTransaction>;

    /// Broadcast a signed transaction to the network
    async fn broadcast(&self, tx: &SignedTransaction) -> Result<BroadcastResult>;

    /// Build a native TRX transfer transaction (for testnet gas sponsorship)
    async fn build_trx_transfer(
        &self,
        from: &str,
        to: &str,
        amount_sun: u64, // Amount in SUN (1 TRX = 1,000,000 SUN)
    ) -> Result<UnsignedTransaction>;

    /// Get current block number and timestamp
    async fn get_current_block(&self) -> Result<BlockInfo>;

    /// Get details of an on-chain transaction
    async fn get_transaction_info(&self, tx_hash: &str) -> Result<Option<TransactionInfo>>;

    /// Get account energy and bandwidth resources
    async fn get_account_resources(&self, address: &str) -> Result<AccountResource>;

    /// Get raw transaction by ID (checks existence in mempool/chain)
    async fn get_transaction_by_id(&self, tx_hash: &str) -> Result<Option<SignedTransaction>>;

    /// Establish transaction presence or absence across all configured providers.
    /// Test implementations and single-provider clients inherit the safe fallback.
    async fn transaction_known_on_any_endpoint(&self, tx_hash: &str) -> Result<bool> {
        Ok(self.get_transaction_by_id(tx_hash).await?.is_some())
    }

    /// Estimate energy for a contract call
    async fn estimate_energy(
        &self,
        owner_address: &str,
        contract_address: &str,
        function_selector: &str,
        parameter: &str,
    ) -> Result<i64>;
}

/// Account resource information (Energy & Bandwidth)
#[derive(Debug, Clone, Default)]
pub struct AccountResource {
    pub free_net_used: i64,
    pub free_net_limit: i64,
    pub net_limit: i64,
    pub asset_net_used: Vec<AssetNetUsed>,
    pub net_used: i64,
    pub energy_limit: i64,
    pub energy_used: i64,
}

#[derive(Debug, Clone, Default)]
pub struct AssetNetUsed {
    pub key: String,
    pub value: i64,
}

/// Block information from the chain
#[derive(Debug, Clone)]
pub struct BlockInfo {
    pub number: u64,
    pub timestamp: i64,
}

#[derive(Debug, Clone)]
pub struct TransactionInfo {
    pub tx_hash: String,
    pub block_number: i64,
    pub success: bool,
    pub result: Option<String>, // TRON status code (e.g., "OUT_OF_ENERGY", "REVERT")
    pub fee_burned: i64,        // Total fee in Sun
    pub revert_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UnsignedTransaction {
    pub raw_data: Vec<u8>,
    pub raw_data_hex: String,
    pub raw_data_json: Option<serde_json::Value>,
    pub expiration: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct SignedTransaction {
    pub tx_id: String,
    pub raw_data: Vec<u8>,
    pub signature: Vec<u8>,
    pub raw_data_json: Option<serde_json::Value>,
    pub expiration: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct BroadcastResult {
    pub success: bool,
    pub tx_hash: String,
    pub message: Option<String>,
}
