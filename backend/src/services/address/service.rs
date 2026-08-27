//! Address Manager Service
//!
//! Handles HD address derivation and atomic allocation with encrypted xpub storage.
//! Aligned with docs/system_design.md

use crate::crypto::{decrypt_aes_gcm, encrypt_aes_gcm};
use crate::entity::{addresses, merchants, ChainFamily, Environment, Network};
use crate::services::address::hd_wallet;
use anyhow::Result;
use dashmap::DashSet;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    EntityTrait, Statement,
};
use std::sync::Arc;
use tracing::{debug, info, warn};

use super::key_provider::MasterKeyProviderBox;
use secrecy::{ExposeSecret, Secret};

pub struct AddressManager {
    db: DatabaseConnection,
    /// Encryption key for xpub storage (Data Encryption Key)
    /// stored as raw bytes to avoid repeated decoding
    encryption_key: Secret<Vec<u8>>,
    /// Provider for Master Key (Mnemonic or KMS)
    master_key_provider: MasterKeyProviderBox,
    /// In-progress replenishment tracking (debounce concurrent triggers)
    replenishing_merchants: Arc<DashSet<String>>,
}

/// Custom error for address allocation
#[derive(Debug, thiserror::Error)]
pub enum AddressAllocationError {
    #[error("Address pool exhausted for merchant {0}")]
    PoolExhausted(String),

    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("Encryption error: {0}")]
    Encryption(#[from] anyhow::Error),

    #[error("Task join error: {0}")]
    TaskJoin(#[from] tokio::task::JoinError),
}

impl AddressManager {
    pub fn new(
        db: DatabaseConnection,
        encryption_key_hex: Secret<String>,
        master_key_provider: MasterKeyProviderBox,
    ) -> Self {
        // Decode hex key once at initialization
        let key_bytes = hex::decode(encryption_key_hex.expose_secret())
            .expect("Encryption key must be valid hex (validated in config)");

        Self {
            db,
            encryption_key: Secret::new(key_bytes),
            master_key_provider,
            replenishing_merchants: Arc::new(DashSet::new()),
        }
    }

    /// Helper to get raw key bytes on demand
    fn get_key_bytes(&self) -> [u8; 32] {
        self.encryption_key
            .expose_secret()
            .as_slice()
            .try_into()
            .expect("FATAL: ENCRYPTION_KEY must be exactly 32 bytes for AES-256-GCM")
    }

    /// Atomic address allocation using single UPDATE ... RETURNING statement
    /// Returns (network, address) tuple on success
    pub async fn allocate_address(
        &self,
        merchant_id: &str,
        network: Network,
        _environment: Environment,
    ) -> Result<(String, String), AddressAllocationError> {
        let network_str = network.as_str().to_string();
        let result = self
            .db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
                UPDATE addresses
                SET status = 'Assigned',
                    updated_at = NOW()
                WHERE (network, address) = (
                    SELECT a.network, a.address
                    FROM addresses a
                    WHERE a.status = 'Idle'
                      AND a.merchant_id = $1
                      AND a.network = $2
                      AND NOT EXISTS (
                          SELECT 1 FROM addresses a2
                          WHERE a2.address = a.address
                            AND a2.status != 'Idle'
                            AND a2.network != a.network
                      )
                    LIMIT 1
                    FOR UPDATE SKIP LOCKED
                )
                RETURNING network, address
                "#,
                [merchant_id.into(), network_str.into()],
            ))
            .await?;

        match result {
            Some(row) => {
                let network: String = row.try_get("", "network")?;
                let address: String = row.try_get("", "address")?;
                info!(merchant_id, network = %network, address = %address, "Atomic address allocation");
                Ok((network, address))
            }
            None => Err(AddressAllocationError::PoolExhausted(
                merchant_id.to_string(),
            )),
        }
    }

    /// Release an address back to Idle (for expired sessions)
    pub async fn release_address(
        &self,
        network: Network,
        _environment: Environment,
        address: &str,
    ) -> Result<(), AddressAllocationError> {
        use chrono::Utc;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let now = Utc::now().fixed_offset();
        let network_str = network.as_str().to_string();

        // Find and update the address if it's currently Assigned
        let result = addresses::Entity::update_many()
            .col_expr(
                addresses::Column::Status,
                sea_orm::sea_query::Expr::value(addresses::AddressStatus::Idle),
            )
            .col_expr(
                addresses::Column::UpdatedAt,
                sea_orm::sea_query::Expr::value(now),
            )
            .filter(addresses::Column::Network.eq(network_str))
            .filter(addresses::Column::Address.eq(address))
            .filter(addresses::Column::Status.eq(addresses::AddressStatus::Assigned))
            .exec(&self.db)
            .await?;

        if result.rows_affected > 0 {
            info!(?network, address, "Address released back to Idle");
        }

        Ok(())
    }

    /// Pre-generate addresses for a merchant (called by background task)
    ///
    /// Uses SeaORM batch insert for optimal performance (single DB roundtrip)
    /// Offloads CPU-intensive HD derivation to blocking thread.
    ///
    /// # Arguments
    /// * `db` - Database connection or transaction
    /// * `merchant_id` - Merchant identifier
    /// * `merchant_xpub_encrypted` - Base64-encoded encrypted xpub (AES-256-GCM)
    /// * `network` - Network enum (e.g., Network::Tron, Network::Bsc)
    /// * `start_index` - Starting path_index
    /// * `count` - Number of addresses to generate
    pub async fn generate_addresses<'c, C>(
        &self,
        db: &'c C,
        merchant_id: &str,
        merchant_xpub_encrypted: &str,
        network: Network,
        _environment: Environment,
        start_index: u32,
        count: u32,
    ) -> Result<Vec<String>, AddressAllocationError>
    where
        C: ConnectionTrait,
    {
        use chrono::Utc;
        use sea_orm::InsertResult;

        // Decrypt xpub (fast, AES-NI)
        let key = self.get_key_bytes();
        let merchant_xpub = decrypt_aes_gcm(merchant_xpub_encrypted, &key)?;

        // Prepare data for closure
        let merchant_id_clone = merchant_id.to_string();
        let network_str = network.as_str().to_string();

        // Offload CPU-intensive HD derivation (PBKDF2/ECC) to blocking thread
        let network_for_derive = network.clone();
        let (models, addresses_created) = tokio::task::spawn_blocking(move || {
            let mut models: Vec<addresses::ActiveModel> = Vec::with_capacity(count as usize);
            let mut addresses_created: Vec<String> = Vec::with_capacity(count as usize);
            let now = Utc::now().fixed_offset();

            for i in start_index..(start_index + count) {
                let address = match hd_wallet::derive_address(
                    &merchant_xpub,
                    i,
                    network_for_derive.clone(),
                ) {
                    Ok(addr) => addr,
                    Err(e) => {
                        warn!(path_index = i, error = %e, "HD derivation failed, skipping");
                        continue;
                    }
                };

                models.push(addresses::ActiveModel {
                    network: Set(network_str.clone()),
                    address: Set(address.clone()),
                    merchant_id: Set(merchant_id_clone.clone()),
                    path_index: Set(i as i32),
                    native_balance: Set(0),
                    usdt_balance: Set(0),
                    usdc_balance: Set(0),
                    status: Set(addresses::AddressStatus::Idle),
                    error_reason: Set(None),
                    sweep_attempts: Set(0),
                    created_at: Set(now),
                    updated_at: Set(now),
                });
                addresses_created.push(address);
            }
            (models, addresses_created)
        })
        .await?;

        // Batch insert with ON CONFLICT DO NOTHING
        if !models.is_empty() {
            let _result: InsertResult<addresses::ActiveModel> =
                addresses::Entity::insert_many(models)
                    .on_conflict(
                        sea_orm::sea_query::OnConflict::columns([
                            addresses::Column::Network,
                            addresses::Column::Address,
                        ])
                        .do_nothing()
                        .to_owned(),
                    )
                    .exec(db)
                    .await?;
        }

        info!(
            merchant_id = %merchant_id,
            count = addresses_created.len(),
            "Generated addresses for merchant (batch insert)"
        );
        Ok(addresses_created)
    }

    /// Generate addresses for Solana (seed-based SLIP-0010 derivation).
    ///
    /// Solana uses Ed25519 keys derived via SLIP-0010, which requires seed access
    /// (no xpub cold derivation). This method delegates to `MasterKeyProvider::batch_derive_addresses`.
    ///
    /// Kept as a separate method to avoid modifying `generate_addresses()` signature,
    /// which would affect all TRON/EVM callers.
    pub async fn generate_addresses_solana<'c, C>(
        &self,
        db: &'c C,
        merchant_id: &str,
        account_index: u32,
        network: Network,
        start_index: u32,
        count: u32,
    ) -> Result<Vec<String>, AddressAllocationError>
    where
        C: ConnectionTrait,
    {
        use chrono::Utc;
        use sea_orm::InsertResult;

        let network_str = network.as_str().to_string();
        let merchant_id_str = merchant_id.to_string();

        // Derive addresses via MasterKeyProvider (seed access)
        let derived = self
            .master_key_provider
            .batch_derive_addresses(account_index, network.coin_type(), start_index, count)
            .await
            .map_err(|e| AddressAllocationError::Encryption(e))?;

        // Build ActiveModels for batch insert
        let now = Utc::now().fixed_offset();
        let mut models: Vec<addresses::ActiveModel> = Vec::with_capacity(derived.len());
        let mut addresses_created: Vec<String> = Vec::with_capacity(derived.len());

        for (path_index, address) in &derived {
            models.push(addresses::ActiveModel {
                network: Set(network_str.clone()),
                address: Set(address.clone()),
                merchant_id: Set(merchant_id_str.clone()),
                path_index: Set(*path_index as i32),
                native_balance: Set(0),
                usdt_balance: Set(0),
                usdc_balance: Set(0),
                status: Set(addresses::AddressStatus::Idle),
                error_reason: Set(None),
                sweep_attempts: Set(0),
                created_at: Set(now),
                updated_at: Set(now),
            });
            addresses_created.push(address.clone());
        }

        // Batch insert with ON CONFLICT DO NOTHING
        if !models.is_empty() {
            let _result: InsertResult<addresses::ActiveModel> =
                addresses::Entity::insert_many(models)
                    .on_conflict(
                        sea_orm::sea_query::OnConflict::columns([
                            addresses::Column::Network,
                            addresses::Column::Address,
                        ])
                        .do_nothing()
                        .to_owned(),
                    )
                    .exec(db)
                    .await?;
        }

        info!(
            merchant_id = %merchant_id,
            count = addresses_created.len(),
            "Generated Solana addresses for merchant (seed-based)"
        );
        Ok(addresses_created)
    }

    /// Encrypt merchant's Account xpub for storage
    pub fn encrypt_xpub(&self, xpub: &str) -> Result<String, AddressAllocationError> {
        let key = self.get_key_bytes();
        Ok(encrypt_aes_gcm(xpub, &key)?)
    }

    /// Decrypt merchant's Account xpub (for admin/debug operations)
    pub fn decrypt_xpub(&self, encrypted: &str) -> Result<String, AddressAllocationError> {
        let key = self.get_key_bytes();
        Ok(decrypt_aes_gcm(encrypted, &key)?)
    }

    /// Get count of effectively allocatable addresses for a merchant.
    ///
    /// Counts addresses that are both Idle on this network AND not blocked
    /// by the cross-chain exclusive lock (same address in non-Idle state on another network).
    pub async fn get_idle_count(
        &self,
        merchant_id: &str,
        network: Network,
        _environment: Environment,
    ) -> Result<u64> {
        let network_str = network.as_str().to_string();

        let result = self
            .db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
                SELECT COUNT(*) as cnt
                FROM addresses a
                WHERE a.status = 'Idle'
                  AND a.merchant_id = $1
                  AND a.network = $2
                  AND NOT EXISTS (
                      SELECT 1 FROM addresses a2
                      WHERE a2.address = a.address
                        AND a2.status != 'Idle'
                        AND a2.network != a.network
                  )
                "#,
                [merchant_id.into(), network_str.into()],
            ))
            .await?;

        let count: i64 = result
            .map(|row| row.try_get("", "cnt").unwrap_or(0))
            .unwrap_or(0);

        Ok(count as u64)
    }

    /// Async trigger address pool replenishment (fire-and-forget)
    ///
    /// Called after successful `allocate_address()` to proactively maintain pool levels.
    /// Non-blocking: spawns a background task and returns immediately.
    ///
    /// # Arguments
    /// * `merchant_id` - Merchant to replenish addresses for
    /// * `network` - Network context
    /// * `environment` - Environment context
    /// * `threshold` - Low watermark (default: 20)
    /// * `batch_size` - Number of addresses to generate (default: 50)
    pub fn trigger_replenish(
        self: Arc<Self>,
        merchant_id: String,
        network: Network,
        environment: Environment,
        threshold: u64,
        batch_size: u32,
    ) {
        tokio::spawn(async move {
            if let Err(e) = self
                .replenish_if_needed(&merchant_id, network, environment, threshold, batch_size)
                .await
            {
                warn!(merchant_id, error = %e, "Background replenish failed");
            }
        });
    }

    /// Internal: Check and replenish address pool if below threshold
    ///
    /// Uses DashSet for debounce to prevent thundering herd when multiple
    /// concurrent requests trigger replenishment simultaneously.
    async fn replenish_if_needed(
        &self,
        merchant_id: &str,
        network: Network,
        environment: Environment,
        threshold: u64,
        batch_size: u32,
    ) -> Result<(), AddressAllocationError> {
        use crate::entity::merchant_chain_accounts;
        use sea_orm::TransactionTrait;

        // Debounce: Skip if already replenishing for this merchant
        let debounce_key = format!("{}:{}:{:?}", merchant_id, network.as_str(), environment);
        if self.replenishing_merchants.contains(&debounce_key) {
            debug!(merchant_id, "Replenish already in progress, skipping");
            return Ok(());
        }

        // Double-check: Re-query idle count to handle race conditions
        let idle_count = self
            .get_idle_count(merchant_id, network.clone(), environment.clone())
            .await
            .map_err(|e| AddressAllocationError::Encryption(e))?;

        if idle_count >= threshold {
            debug!(
                merchant_id,
                idle_count, threshold, "Pool above threshold, skipping replenish"
            );
            return Ok(());
        }

        // Acquire debounce lock
        self.replenishing_merchants.insert(debounce_key.clone());

        // Ensure lock is released on exit (RAII pattern)
        let _guard = scopeguard::guard((), |_| {
            self.replenishing_merchants.remove(&debounce_key);
        });

        // Get chain account for xpub
        let chain_account = merchant_chain_accounts::Entity::find_by_id((
            merchant_id.to_string(),
            environment.clone(),
            network.clone(),
        ))
        .one(&self.db)
        .await?;

        let chain_account = match chain_account {
            Some(acc) => acc,
            None => {
                debug!(merchant_id, "No chain account found, skipping replenish");
                return Ok(());
            }
        };

        // Begin transaction for atomic update
        let txn = self.db.begin().await?;

        let start_index = (chain_account.last_path_index + 1) as u32;

        // Generate addresses — dispatch based on chain family
        let addresses = if network.chain_family() == ChainFamily::Solana {
            // Solana: need account_index from merchants table (seed-based derivation)
            let merchant = merchants::Entity::find_by_id(merchant_id)
                .one(&self.db)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Merchant not found for replenish"))?;
            let acct_idx: u32 = merchant
                .account_index
                .ok_or_else(|| anyhow::anyhow!("Merchant {} lacks account_index", merchant_id))?
                .try_into()
                .map_err(|_| anyhow::anyhow!("Invalid account_index"))?;
            self.generate_addresses_solana(
                &txn,
                merchant_id,
                acct_idx,
                network.clone(),
                start_index,
                batch_size,
            )
            .await?
        } else {
            self.generate_addresses(
                &txn,
                merchant_id,
                &chain_account.xpub_encrypted,
                network.clone(),
                environment.clone(),
                start_index,
                batch_size,
            )
            .await?
        };

        // Update last_path_index atomically
        if !addresses.is_empty() {
            let new_last_index = start_index + addresses.len() as u32 - 1;
            let mut active: merchant_chain_accounts::ActiveModel = chain_account.into();
            active.last_path_index = Set(new_last_index as i32);
            active.update(&txn).await?;
        }

        txn.commit().await?;

        info!(
            merchant_id,
            count = addresses.len(),
            idle_count,
            threshold,
            "Background replenish completed"
        );

        Ok(())
    }

    /// Initialize merchant addresses for a specific network context.
    ///
    /// This encapsulates the business logic for:
    /// 1. Deriving the merchant's Account xpub from master mnemonic (if not already set)
    /// 2. Encrypting and storing xpub in `merchant_chain_accounts`
    /// 3. Pre-generating payment addresses for the specified network
    ///
    /// # Arguments
    /// * `merchant_id` - Merchant identifier
    /// * `network` - Network context (from AuthenticatedMerchant)
    ///
    /// # Returns
    /// A JSON-like struct containing initialization stats
    pub async fn initialize_merchant_addresses(
        &self,
        merchant_id: &str,
        network: Network,
        environment: Environment,
    ) -> Result<InitializeAddressesResult, AddressAllocationError> {
        use crate::entity::merchant_chain_accounts;
        use sea_orm::TransactionTrait;

        // 1. Get merchant record (for account_index)
        let merchant_model = merchants::Entity::find_by_id(merchant_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                AddressAllocationError::Encryption(anyhow::anyhow!("Merchant not found"))
            })?;

        // 2. Check if Chain Account exists (using composite key)
        let chain_account_opt = merchant_chain_accounts::Entity::find_by_id((
            merchant_id.to_string(),
            environment.clone(),
            network,
        ))
        .one(&self.db)
        .await?;

        if let Some(chain_account) = chain_account_opt {
            // Already initialized, check idle count
            let idle_count = self
                .get_idle_count(merchant_id, network.clone(), environment.clone())
                .await
                .map_err(|e| AddressAllocationError::Encryption(e))?;

            if idle_count >= 10 {
                return Ok(InitializeAddressesResult {
                    already_initialized: true,
                    addresses_created: 0,
                    idle_count,
                    sample_address: None,
                });
            }

            // Begin Transaction
            let txn = self.db.begin().await?;

            let start_index = (chain_account.last_path_index + 1) as u32;
            let count = 20u32;

            // Generate more addresses (using txn)
            let addresses = if network.chain_family() == ChainFamily::Solana {
                // Solana: seed-based derivation (no xpub)
                let acct_idx: u32 = merchant_model
                    .account_index
                    .ok_or_else(|| {
                        AddressAllocationError::Encryption(anyhow::anyhow!(
                            "Merchant {} lacks account_index",
                            merchant_id
                        ))
                    })?
                    .try_into()
                    .map_err(|_| {
                        AddressAllocationError::Encryption(anyhow::anyhow!(
                            "Invalid account_index: must be non-negative"
                        ))
                    })?;
                self.generate_addresses_solana(
                    &txn,
                    merchant_id,
                    acct_idx,
                    network.clone(),
                    start_index,
                    count,
                )
                .await?
            } else {
                self.generate_addresses(
                    &txn,
                    merchant_id,
                    &chain_account.xpub_encrypted,
                    network.clone(),
                    environment.clone(),
                    start_index,
                    count,
                )
                .await?
            };

            // Update last_path_index atomically (using txn)
            if !addresses.is_empty() {
                let new_last_index = start_index + addresses.len() as u32 - 1;
                let mut active: merchant_chain_accounts::ActiveModel = chain_account.into();
                active.last_path_index = Set(new_last_index as i32);
                active.update(&txn).await?;
            }

            // Commit Transaction
            txn.commit().await?;

            // Re-check idle count (outside txn is fine)
            let new_idle_count = self
                .get_idle_count(merchant_id, network.clone(), environment.clone())
                .await
                .map_err(|e| AddressAllocationError::Encryption(e))?;

            return Ok(InitializeAddressesResult {
                already_initialized: true,
                addresses_created: addresses.len() as u32,
                idle_count: new_idle_count,
                sample_address: addresses.first().cloned(),
            });
        }

        // 3. Need to initialize — get merchant account_index
        let account_index_u32: u32 = merchant_model
            .account_index
            .ok_or_else(|| {
                AddressAllocationError::Encryption(anyhow::anyhow!(
                    "CRITICAL: Merchant {} lacks account_index (NULL). \
                     Every merchant must have a unique HD account index.",
                    merchant_id
                ))
            })?
            .try_into()
            .map_err(|_| {
                AddressAllocationError::Encryption(anyhow::anyhow!(
                    "Invalid account_index: must be non-negative"
                ))
            })?;

        let start_index = 0u32;
        let count = 20u32;

        // Begin Transaction for initial setup
        let txn = self.db.begin().await?;

        let (xpub_encrypted, addresses) = if network.chain_family() == ChainFamily::Solana {
            // Solana: seed-based derivation — no xpub, use encrypted sentinel
            let sentinel = self.encrypt_xpub("SOLANA_SEED_BASED")?;
            let addrs = self
                .generate_addresses_solana(
                    &txn,
                    merchant_id,
                    account_index_u32,
                    network.clone(),
                    start_index,
                    count,
                )
                .await?;
            (sentinel, addrs)
        } else {
            // TRON/EVM: xpub-based derivation
            let coin_type = network.coin_type();
            let account_xpub = self
                .master_key_provider
                .get_account_xpub_for_coin(account_index_u32, coin_type)
                .await
                .map_err(|e| AddressAllocationError::Encryption(e))?;
            let xpub_enc = self.encrypt_xpub(&account_xpub)?;
            let addrs = self
                .generate_addresses(
                    &txn,
                    merchant_id,
                    &xpub_enc,
                    network.clone(),
                    environment.clone(),
                    start_index,
                    count,
                )
                .await?;
            (xpub_enc, addrs)
        };

        // 7. Create Chain Account Row
        let new_last_index = if addresses.is_empty() {
            -1 // No addresses generated
        } else {
            (start_index + addresses.len() as u32 - 1) as i32
        };

        // This is a NEW row
        let new_account = merchant_chain_accounts::ActiveModel {
            merchant_id: Set(merchant_id.to_string()),
            environment: Set(environment),
            network: Set(network),
            xpub_encrypted: Set(xpub_encrypted),
            last_path_index: Set(new_last_index),
            collection_address: Set(None),
            usdt_balance: Set(0),
            usdc_balance: Set(0),
            created_at: Set(chrono::Utc::now().fixed_offset()),
            updated_at: Set(chrono::Utc::now().fixed_offset()),
        };

        // Use Entity::insert with ON CONFLICT DO NOTHING for idempotency
        merchant_chain_accounts::Entity::insert(new_account)
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns([
                    merchant_chain_accounts::Column::MerchantId,
                    merchant_chain_accounts::Column::Environment,
                    merchant_chain_accounts::Column::Network,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec(&txn)
            .await?;

        // Commit Transaction
        txn.commit().await?;

        Ok(InitializeAddressesResult {
            already_initialized: false,
            addresses_created: addresses.len() as u32,
            idle_count: addresses.len() as u64,
            sample_address: addresses.first().cloned(),
        })
    }
}

/// Result of address initialization
#[derive(Debug)]
pub struct InitializeAddressesResult {
    pub already_initialized: bool,
    pub addresses_created: u32,
    pub idle_count: u64,
    pub sample_address: Option<String>,
}
