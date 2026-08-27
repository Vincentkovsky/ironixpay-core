//! Billing Service Implementation
//!
//! Handles accounting logic for merchants: balance credits and refunds.
//! Acts as the "Accountant" of the system (Ledger mode).
//!
//! Aligned with docs/1_mvp_must_have/1.10 ledger-mode-design.md

use anyhow::{anyhow, Result};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, QuerySelect, Set};
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::entity::billing_logs;

#[derive(Clone)]
pub struct BillingService {}

impl BillingService {
    pub fn new() -> Self {
        Self {}
    }

    /// Lock chain account row for per-chain balance operations (SELECT ... FOR UPDATE).
    ///
    /// Locks (merchant_id, environment, network) row in `merchant_chain_accounts`
    /// for atomic balance read-modify-write.
    pub async fn get_chain_balance_lock<C>(
        &self,
        txn: &C,
        merchant_id: &str,
        environment: crate::entity::Environment,
        network: crate::entity::Network,
    ) -> Result<crate::entity::merchant_chain_accounts::Model>
    where
        C: ConnectionTrait,
    {
        use crate::entity::merchant_chain_accounts;

        let account_opt = merchant_chain_accounts::Entity::find_by_id((
            merchant_id.to_string(),
            environment.clone(),
            network.clone(),
        ))
        .lock_exclusive()
        .one(txn)
        .await
        .map_err(|e| {
            error!(
                merchant_id,
                environment = ?environment,
                network = ?network,
                error = %e,
                "Failed to acquire chain account lock for billing operation"
            );
            e
        })?;

        account_opt.ok_or_else(|| {
            anyhow!(
                "Chain account not found for merchant {} env {:?} network {:?}",
                merchant_id,
                environment,
                network
            )
        })
    }

    /// Process a deposit (Credit) to a merchant's per-chain balance.
    ///
    /// Locks the `merchant_chain_accounts` row for the given (merchant, env, network)
    /// and credits the balance atomically.
    pub async fn process_deposit<C>(
        &self,
        txn: &C,
        merchant_id: &str,
        amount: i64,
        external_ref_id: Option<String>,
        description: Option<String>,
        network: crate::entity::Network,
        environment: crate::entity::Environment,
        token: &str,
        // Pre-fee gross amount (microunits). For agent commission tracking.
        gross_amount: Option<i64>,
        // Actual fee charged (microunits). For agent commission tracking.
        fee_amount: Option<i64>,
    ) -> Result<billing_logs::Model>
    where
        C: ConnectionTrait,
    {
        use crate::entity::merchant_chain_accounts;

        if amount < 0 {
            return Err(anyhow!("Deposit amount must not be negative"));
        }

        // 1. Pessimistic Lock on chain account
        let chain_account = self
            .get_chain_balance_lock(txn, merchant_id, environment.clone(), network.clone())
            .await?;

        // 2. Update Balance (skip for zero-net dust payments — audit log only)
        // Select the correct balance column based on token
        let previous_balance = if token == "USDC" {
            chain_account.usdc_balance
        } else {
            chain_account.usdt_balance
        };
        let new_balance = previous_balance + amount;

        if amount > 0 {
            let mut account_active: merchant_chain_accounts::ActiveModel = chain_account.into();
            if token == "USDC" {
                account_active.usdc_balance = Set(new_balance);
            } else {
                account_active.usdt_balance = Set(new_balance);
            }
            account_active.updated_at = Set(Utc::now().into());
            let _updated = account_active.update(txn).await.map_err(|e| {
                error!(
                    merchant_id,
                    environment = ?environment,
                    network = ?network,
                    deposit_amount = amount,
                    error = %e,
                    "Failed to update chain account balance during deposit"
                );
                e
            })?;
        }

        // 3. Create Billing Log
        let log = billing_logs::ActiveModel {
            id: Set(format!("bl_{}", Uuid::new_v4().simple())),
            environment: Set(environment.clone()),
            network: Set(network.as_str().to_string()),
            merchant_id: Set(merchant_id.to_string()),
            session_id: Set(None),
            external_ref_id: Set(external_ref_id),
            billing_type: Set(billing_logs::BillingType::PaymentCredit),
            previous_balance: Set(previous_balance),
            amount_change: Set(amount),
            balance_after: Set(new_balance),
            description: Set(description),
            token: Set(token.to_string()),
            gross_amount: Set(gross_amount),
            fee_amount: Set(fee_amount),
            created_at: Set(Utc::now().into()),
        };

        let saved_log = log.insert(txn).await.map_err(|e| {
            error!(
                merchant_id,
                environment = ?environment,
                deposit_amount = amount,
                error = %e,
                "Failed to insert billing log during deposit"
            );
            e
        })?;

        info!(
            merchant_id,
            environment = ?environment,
            network = ?network,
            deposit_amount = amount,
            new_balance = new_balance,
            token,
            "Processed deposit to chain account"
        );

        Ok(saved_log)
    }

    /// Process a refund (Credit) to a merchant's per-chain balance.
    ///
    /// Used when a sweep operation fails (e.g. broadcast failure) after money was deducted.
    ///
    /// **Idempotency**: If a refund already exists for the given `external_ref_id` (tx_hash),
    /// this function returns the existing log without creating a duplicate.
    pub async fn refund_cost<C>(
        &self,
        txn: &C,
        merchant_id: &str,
        session_id: Option<String>,
        amount: i64,
        external_ref_id: Option<String>,
        description: Option<String>,
        network: crate::entity::Network,
        environment: crate::entity::Environment,
        token: &str,
    ) -> Result<billing_logs::Model>
    where
        C: ConnectionTrait,
    {
        use crate::entity::merchant_chain_accounts;
        use sea_orm::{ColumnTrait, QueryFilter};

        if amount <= 0 {
            return Err(anyhow!("Refund amount must be positive"));
        }

        // ============================================================
        // HIGH-6 FIX: Idempotency Check
        // ============================================================
        if let Some(ref ref_id) = external_ref_id {
            let existing_refund = billing_logs::Entity::find()
                .filter(billing_logs::Column::ExternalRefId.eq(Some(ref_id.clone())))
                .filter(billing_logs::Column::BillingType.eq(billing_logs::BillingType::Refund))
                .filter(billing_logs::Column::MerchantId.eq(merchant_id))
                .one(txn)
                .await?;

            if let Some(existing) = existing_refund {
                debug!(
                    merchant_id,
                    external_ref_id = %ref_id,
                    "Refund already processed for this transaction, skipping duplicate"
                );
                return Ok(existing);
            }
        }

        // 1. Lock chain account
        let chain_account = self
            .get_chain_balance_lock(txn, merchant_id, environment.clone(), network.clone())
            .await?;

        // 2. Update Balance — select correct column based on token
        let previous_balance = if token == "USDC" {
            chain_account.usdc_balance
        } else {
            chain_account.usdt_balance
        };
        let new_balance = previous_balance + amount;

        let mut account_active: merchant_chain_accounts::ActiveModel = chain_account.into();
        if token == "USDC" {
            account_active.usdc_balance = Set(new_balance);
        } else {
            account_active.usdt_balance = Set(new_balance);
        }
        account_active.updated_at = Set(Utc::now().into());
        account_active.update(txn).await.map_err(|e| {
            error!(
                merchant_id,
                environment = ?environment,
                network = ?network,
                refund_amount = amount,
                error = %e,
                "Failed to update chain account balance during refund"
            );
            e
        })?;

        // 3. Create Billing Log
        let log = billing_logs::ActiveModel {
            id: Set(format!("bl_{}", Uuid::new_v4().simple())),
            environment: Set(environment.clone()),
            network: Set(network.as_str().to_string()),
            merchant_id: Set(merchant_id.to_string()),
            session_id: Set(session_id),
            external_ref_id: Set(external_ref_id),
            billing_type: Set(billing_logs::BillingType::Refund),
            previous_balance: Set(previous_balance),
            amount_change: Set(amount),
            balance_after: Set(new_balance),
            description: Set(description),
            token: Set(token.to_string()),
            gross_amount: Set(None),
            fee_amount: Set(None),
            created_at: Set(Utc::now().into()),
        };

        let saved_log = log.insert(txn).await.map_err(|e| {
            error!(
                merchant_id,
                environment = ?environment,
                refund_amount = amount,
                error = %e,
                "Failed to insert billing log during refund"
            );
            e
        })?;

        info!(
            merchant_id,
            environment = ?environment,
            network = ?network,
            refund_amount = amount,
            new_balance = new_balance,
            "Processed refund to chain account"
        );

        Ok(saved_log)
    }
}
