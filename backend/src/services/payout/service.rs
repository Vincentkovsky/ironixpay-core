//! Payout Service Implementation
//!
//! Handles merchant withdrawals: balance debit → on-chain transfer → confirm/rollback.
//! Uses Semaphore to rate-limit concurrent **broadcasts** only (not confirmations).
//! Chain-agnostic: all chain-specific operations delegated to PayoutExecutor.
//!
//! Aligned with docs/1_mvp_must_have/1.10 ledger-mode-design.md

use super::executor::PayoutExecutor;
use super::PayoutError;
use crate::services::aml::{service::RiskResult, AmlService};
use anyhow::{anyhow, Result};
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection,
    EntityTrait, QueryFilter, QuerySelect, Set, TransactionTrait,
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use tracing::{debug, error, info, warn};

use crate::entity::{
    billing_logs, merchant_chain_accounts, network::Network, outbound_transactions, webhook_events,
    withdrawals, withdrawals::WithdrawalStatus,
};
use crate::services::alerting::{AlertLevel, AlertingService};
use crate::services::billing::fee_config::FeeConfig;
use crate::services::billing::BillingService;
use crate::services::webhook::WebhookService;

/// Webhook payload for payout/withdrawal events.
/// All amounts are human-readable decimal strings (e.g., "10.5" = 10.5 USDT).
#[derive(Serialize)]
struct PayoutEventPayload {
    /// Object type identifier (always "payout")
    object: &'static str,
    id: String,
    merchant_id: String,
    livemode: bool,
    status: String,
    amount: String,
    fee: String,
    net_amount: String,
    currency: String,
    network: String,
    to_address: String,
    tx_hash: Option<String>,
    idempotency_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_reason: Option<String>,
    created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    completed_at: Option<i64>,
}

/// Treasury HD derivation: account_index=0, path_index=0.
/// Dedicated to platform treasury — never collides with merchant addresses (account_index ≥ 1).
const TREASURY_ACCOUNT_INDEX: i32 = 0;
const TREASURY_PATH_INDEX: u32 = 0;

/// Maximum concurrent on-chain **broadcasts** (confirmations run outside this limit).
const MAX_CONCURRENT_BROADCASTS: usize = 5;

/// Required confirmation blocks.
const CONFIRM_BLOCKS: u64 = 30;

/// Max number of Processing records to check per confirmation cycle.
const CONFIRM_BATCH_SIZE: u64 = 50;

#[derive(Clone)]
pub struct PayoutService {
    db: DatabaseConnection,
    billing_service: Arc<BillingService>,
    fee_config: Arc<FeeConfig>,
    /// Chain-specific payout executors, keyed by Network
    executors: HashMap<Network, Arc<dyn PayoutExecutor>>,
    /// Treasury addresses per network
    treasury_addresses: HashMap<Network, String>,
    broadcast_semaphore: Arc<Semaphore>,
    /// Per-network broadcast lock for nonce serialization.
    /// Ensures nonce query → sign → broadcast is atomic per treasury address per chain.
    /// TODO: If scaling backend instances > 1, replace with Postgres advisory lock.
    broadcast_locks: HashMap<Network, Arc<Mutex<()>>>,
    alerting_service: Arc<AlertingService>,
    /// Per-network outbound fee overrides (USDT microunits), from chains.toml.
    /// Keyed by Network (not ChainFamily) so ETH and L2s can have different fees.
    /// Falls back to FeeConfig::flat_payout_fee if network not present.
    chain_outbound_fees: HashMap<Network, i64>,
    /// AML service for checking destination addresses (mandatory for payouts)
    aml_service: Arc<AmlService>,
    /// Webhook service for notifying merchants of payout/withdrawal status changes
    webhook_service: Arc<WebhookService>,
    outbound_store: Arc<crate::services::outbound::OutboundTransactionStore>,
    /// Optional heartbeat reporter for /ready and admin health monitoring.
    service_health: Option<(
        crate::services::service_health::ServiceHealthRegistry,
        String,
    )>,
}

impl PayoutService {
    pub fn new(
        db: DatabaseConnection,
        billing_service: Arc<BillingService>,
        fee_config: Arc<FeeConfig>,
        executors: HashMap<Network, Arc<dyn PayoutExecutor>>,
        treasury_addresses: HashMap<Network, String>,
        broadcast_locks: HashMap<Network, Arc<Mutex<()>>>,
        alerting_service: Arc<AlertingService>,
        chain_outbound_fees: HashMap<Network, i64>,
        aml_service: Arc<AmlService>,
        webhook_service: Arc<WebhookService>,
        outbound_store: Arc<crate::services::outbound::OutboundTransactionStore>,
    ) -> Self {
        Self {
            db,
            billing_service,
            fee_config,
            executors,
            treasury_addresses,
            broadcast_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_BROADCASTS)),
            broadcast_locks,
            alerting_service,
            chain_outbound_fees,
            aml_service,
            webhook_service,
            outbound_store,
            service_health: None,
        }
    }

    /// Attach heartbeat reporting for service health monitoring.
    pub fn with_health(
        mut self,
        registry: crate::services::service_health::ServiceHealthRegistry,
        service_name: String,
    ) -> Self {
        self.service_health = Some((registry, service_name));
        self
    }

    /// Build a webhook payload from a payout model.
    ///
    /// Fields that may be stale on the `po` model (read before DB updates in the
    /// same transaction) are passed explicitly: `completed_at`, `error_reason`, `tx_hash`.
    fn build_webhook_payload(
        po: &crate::entity::payouts::Model,
        status: &str,
        completed_at: Option<i64>,
        error_reason: Option<String>,
        tx_hash: Option<String>,
    ) -> PayoutEventPayload {
        use crate::api::dtos::checkout::from_micro;
        let currency = &po.currency;
        PayoutEventPayload {
            object: "payout",
            id: po.id.clone(),
            merchant_id: po.merchant_id.clone(),
            livemode: Network::is_livemode_env(&po.environment),
            status: status.to_string(),
            amount: from_micro(po.amount, currency),
            fee: from_micro(po.fee, currency),
            net_amount: from_micro(po.net_amount, currency),
            currency: currency.clone(),
            network: po.network.clone(),
            to_address: po.to_address.clone(),
            tx_hash,
            idempotency_key: Some(po.idempotency_key.clone()),
            description: po.description.clone(),
            metadata: po.metadata.clone(),
            error_reason,
            created_at: po.created_at.timestamp(),
            completed_at,
        }
    }

    /// Determine the webhook event type based on whether this is a payout (po_) or withdrawal (wd_).
    ///
    /// NOTE: Currently only payouts (po_) trigger webhooks. Withdrawals are initiated by
    /// merchants via the Dashboard, so they can observe status changes directly in the UI.
    /// The `wd_` branches are retained for forward-compatibility — if we later decide to
    /// add webhook notifications for withdrawals, no changes are needed here.
    fn completed_event_type(id: &str) -> &'static str {
        if id.starts_with("wd_") {
            webhook_events::EVENT_WITHDRAWAL_COMPLETED
        } else {
            webhook_events::EVENT_PAYOUT_COMPLETED
        }
    }

    fn failed_event_type(id: &str) -> &'static str {
        if id.starts_with("wd_") {
            webhook_events::EVENT_WITHDRAWAL_FAILED
        } else {
            webhook_events::EVENT_PAYOUT_FAILED
        }
    }

    // =========================================================================
    // Public API: request_withdrawal
    // =========================================================================

    /// Request a withdrawal from merchant balance.
    ///
    /// Executes inside a transaction with FOR UPDATE lock to prevent double-spend:
    /// 1. Lock merchant profile row
    /// 2. Verify sufficient balance
    /// 3. Compute payout fee (No-Loss policy)
    /// 4. Debit balance atomically
    /// 5. Create billing log (Withdrawal) + backfill external_ref_id
    /// 6. Insert withdrawal record (Pending or PendingApproval based on risk rules)
    ///
    /// # Risk Control
    /// Non-Owner withdrawals may be routed to PendingApproval
    /// based on new-address and amount-threshold rules.
    pub async fn request_withdrawal(
        &self,
        merchant_id: &str,
        amount: i64,
        environment: crate::entity::Environment,
        network: crate::entity::Network,
        currency: &str,
        requested_by: Option<&str>,
        skip_risk_control: bool,
    ) -> Result<withdrawals::Model, PayoutError> {
        if amount <= 0 {
            return Err(PayoutError::InvalidAmount(
                "Withdrawal amount must be positive".into(),
            ));
        }

        // Compute outbound fee (per-chain override → global fallback)
        let chain_fee = self.chain_outbound_fees.get(&network).copied();
        let payout_fee = self.fee_config.outbound_fee(amount, chain_fee);
        let net_amount = amount - payout_fee;

        if net_amount <= 0 {
            let amount_str = crate::api::dtos::checkout::from_micro(amount, currency);
            let fee_str = crate::api::dtos::checkout::from_micro(payout_fee, currency);
            return Err(PayoutError::InvalidAmount(format!(
                "Withdrawal amount ({} {}) is too small to cover payout fee ({} {})",
                amount_str, currency, fee_str, currency
            )));
        }

        let merchant_id_owned = merchant_id.to_string();
        let billing_service = self.billing_service.clone();

        // Look up collection address for the requested network
        let chain_account = merchant_chain_accounts::Entity::find()
            .filter(merchant_chain_accounts::Column::MerchantId.eq(merchant_id))
            .filter(merchant_chain_accounts::Column::Environment.eq(environment.clone()))
            .filter(merchant_chain_accounts::Column::Network.eq(network.clone()))
            .one(&self.db)
            .await?
            .ok_or_else(|| PayoutError::NoChainAccount {
                merchant_id: merchant_id.to_string(),
                environment: format!("{:?}", environment),
            })?;

        let to_address = chain_account
            .collection_address
            .ok_or(PayoutError::NoCollectionAddress)?;

        // Validate destination address (network-aware)
        match network.chain_family() {
            crate::entity::ChainFamily::Tron => {
                crate::services::tron::address::validate_address(&to_address).map_err(|e| {
                    PayoutError::InvalidAddress {
                        message: format!("Invalid TRON collection address: {}", e),
                        param: "collection_address".into(),
                    }
                })?;
            }
            crate::entity::ChainFamily::Evm => {
                // Basic EVM address format check (0x + 40 hex chars)
                if !to_address.starts_with("0x") || to_address.len() != 42 {
                    return Err(PayoutError::InvalidAddress {
                        message: "Invalid EVM address: must be 0x-prefixed 42-char hex".into(),
                        param: "collection_address".into(),
                    });
                }
                // Validate hex characters (strip 0x prefix, decode remaining 40 hex chars)
                if hex::decode(&to_address[2..]).is_err() {
                    return Err(PayoutError::InvalidAddress {
                        message: "Invalid EVM address: contains non-hex characters".into(),
                        param: "collection_address".into(),
                    });
                }
            }
            crate::entity::ChainFamily::Solana => {
                crate::entity::network::validate_solana_address(&to_address).map_err(|e| {
                    PayoutError::InvalidAddress {
                        message: format!("Invalid Solana collection address: {}", e),
                        param: "collection_address".into(),
                    }
                })?;
            }
        }

        // P-1 FIX: Guard against self-transfer (treasury → treasury)
        let treasury_address = self
            .treasury_addresses
            .get(&network)
            .cloned()
            .unwrap_or_default();
        let is_self_transfer = match network.chain_family() {
            crate::entity::ChainFamily::Solana => to_address == treasury_address, // Base58 is case-sensitive
            _ => to_address.to_lowercase() == treasury_address.to_lowercase(),
        };
        if is_self_transfer {
            return Err(PayoutError::SelfTransfer {
                message: "Collection address cannot be the platform treasury address".into(),
                param: "collection_address".into(),
            });
        }

        // Execute in transaction with FOR UPDATE lock
        // SeaORM requires DbErr, so we map anyhow errors via DbErr::Custom
        let currency_owned = currency.to_string();
        let requested_by_owned = requested_by.map(|s| s.to_string());

        // ── Risk Control: determine initial status ──
        // NOTE (TOCTOU): This runs outside the transaction. In theory, settings could change
        // between this check and the transaction. The practical risk is negligible (millisecond
        // window, settings changes are rare manual operations). Moving inside the transaction
        // would require restructuring the async closure.
        let initial_status = if skip_risk_control {
            WithdrawalStatus::Pending
        } else {
            self.determine_withdrawal_status(
                merchant_id,
                &network,
                &to_address,
                amount,
                requested_by,
            )
            .await
        };
        let log_status = initial_status.clone();

        let withdrawal = self
            .db
            .transaction::<_, withdrawals::Model, sea_orm::DbErr>(|txn| {
                let merchant_id = merchant_id_owned.clone();
                let billing_service = billing_service.clone();
                let to_address = to_address.clone();
                let env = environment.clone();
                let net = network.clone();
                let network_str = network.as_str().to_string();
                let currency = currency_owned.clone();

                Box::pin(async move {
                    // 1. Lock chain account row (SELECT ... FOR UPDATE)
                    let chain_account = billing_service
                        .get_chain_balance_lock(txn, &merchant_id, env.clone(), net.clone())
                        .await
                        .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

                    // 2. Balance check (dynamic: USDT or USDC)
                    let current_balance = if currency == "USDC" {
                        chain_account.usdc_balance
                    } else {
                        chain_account.usdt_balance
                    };
                    if current_balance < amount {
                        return Err(sea_orm::DbErr::Custom(format!(
                            "INSUFFICIENT_BALANCE:{}:{}:{}",
                            current_balance, amount, currency
                        )));
                    }

                    // 3. Debit balance
                    let previous_balance = current_balance;
                    let new_balance = current_balance - amount;
                    let mut account_active: crate::entity::merchant_chain_accounts::ActiveModel =
                        chain_account.into();
                    if currency == "USDC" {
                        account_active.usdc_balance = Set(new_balance);
                    } else {
                        account_active.usdt_balance = Set(new_balance);
                    }
                    account_active.updated_at = Set(chrono::Utc::now().into());
                    account_active.update(txn).await?;

                    // 4. Generate withdrawal ID first (for billing log cross-reference)
                    let wd_id = format!("wd_{}", uuid::Uuid::new_v4().simple());

                    // 5. Create billing log (Withdrawal — negative amount_change)
                    // BUG-3 FIX: Set external_ref_id = wd_id for audit trail
                    let log = billing_logs::ActiveModel {
                        id: Set(format!("bl_{}", uuid::Uuid::new_v4().simple())),
                        environment: Set(env),
                        network: Set(network_str.clone()),
                        merchant_id: Set(merchant_id.clone()),
                        session_id: Set(None),
                        external_ref_id: Set(Some(wd_id.clone())),
                        billing_type: Set(billing_logs::BillingType::Withdrawal),
                        previous_balance: Set(previous_balance),
                        amount_change: Set(-amount),
                        balance_after: Set(new_balance),
                        description: Set(Some(format!(
                            "Withdrawal: gross={} {} fee={} {} net={} {} to={}",
                            amount as f64 / 1_000_000.0,
                            currency,
                            payout_fee as f64 / 1_000_000.0,
                            currency,
                            net_amount as f64 / 1_000_000.0,
                            currency,
                            to_address
                        ))),
                        token: Set(currency.clone()),
                        gross_amount: Set(None),
                        fee_amount: Set(None),
                        created_at: Set(chrono::Utc::now().into()),
                    };
                    log.insert(txn).await?;

                    // 6. Insert withdrawal record
                    let withdrawal = withdrawals::ActiveModel {
                        id: Set(wd_id),
                        merchant_id: Set(merchant_id),
                        environment: Set(env),
                        network: Set(network_str),
                        amount: Set(amount),
                        network_fee: Set(payout_fee),
                        net_amount: Set(net_amount),
                        to_address: Set(to_address),
                        status: Set(initial_status),
                        tx_hash: Set(None),
                        error_reason: Set(None),
                        currency: Set(currency),
                        created_at: Set(chrono::Utc::now().into()),
                        updated_at: Set(chrono::Utc::now().into()),
                        completed_at: Set(None),
                        requested_by: Set(requested_by_owned),
                        reviewed_by: Set(None),
                        reviewed_at: Set(None),
                    };

                    let saved = withdrawal.insert(txn).await?;

                    info!(
                        withdrawal_id = %saved.id,
                        merchant_id = %saved.merchant_id,
                        amount = saved.amount,
                        net_amount = saved.net_amount,
                        status = ?log_status,
                        "Withdrawal requested"
                    );

                    Ok(saved)
                })
            })
            .await
            .map_err(|e| {
                let msg = e.to_string();
                // Parse structured balance error from transaction closure
                if let Some(rest) = msg.strip_prefix("Custom Error: INSUFFICIENT_BALANCE:") {
                    let parts: Vec<&str> = rest.splitn(3, ':').collect();
                    if parts.len() >= 2 {
                        let have = parts[0].parse::<i64>().unwrap_or(0);
                        let need = parts[1].parse::<i64>().unwrap_or(0);
                        let currency = parts.get(2).unwrap_or(&"USDT").to_string();
                        return PayoutError::InsufficientBalance {
                            have,
                            need,
                            currency,
                        };
                    }
                }
                match e {
                    sea_orm::TransactionError::Transaction(db_err) => PayoutError::Database(db_err),
                    sea_orm::TransactionError::Connection(db_err) => PayoutError::Database(db_err),
                }
            })?;

        Ok(withdrawal)
    }

    // =========================================================================
    // Worker Loop
    // =========================================================================

    /// Start the payout worker loop. Runs until cancellation.
    ///
    /// Each 30s tick runs two phases:
    /// 1. **Confirm**: Scan Processing records with tx_hash, check on-chain status
    /// 2. **Broadcast**: Pick up Pending records, broadcast on-chain
    ///
    /// This persistent scan pattern ensures no payout/withdrawal is ever lost,
    /// even across app restarts — the DB is the single source of truth.
    pub async fn start(&self, cancel_token: tokio_util::sync::CancellationToken) -> Result<()> {
        info!("PayoutService worker starting...");

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    info!("PayoutService worker shutting down");
                    break;
                }
                _ = interval.tick() => {
                    // Phase 0: Auto-expire stale PendingApproval records (24h timeout)
                    self.auto_expire_stale_approvals().await;

                    // Phase 1: Confirm — check on-chain status of Processing records
                    self.recover_auxiliary_outbounds().await;
                    self.confirm_processing_withdrawals().await;
                    self.confirm_processing_payouts().await;

                    // Phase 2: Broadcast — pick up new Pending records
                    if let Err(e) = self.process_pending_withdrawals().await {
                        error!(error = %e, "Error processing pending withdrawals");
                    }
                    if let Err(e) = self.process_pending_payouts().await {
                        error!(error = %e, "Error processing pending payouts");
                    }

                    // Heartbeat after each tick (proves worker is alive and looping)
                    if let Some((ref reg, ref name)) = self.service_health {
                        reg.heartbeat(name);
                    }
                }
            }
        }

        Ok(())
    }

    /// Recover gas-funding child transactions independently from their business parent.
    async fn recover_auxiliary_outbounds(&self) {
        use crate::entity::transactions::ChainTxState;

        let records = match outbound_transactions::Entity::find()
            .filter(outbound_transactions::Column::Purpose.is_in([
                outbound_transactions::OutboundPurpose::GasFunding,
                outbound_transactions::OutboundPurpose::EnergyFunding,
            ]))
            .filter(outbound_transactions::Column::State.is_in([
                outbound_transactions::OutboundState::Signed,
                outbound_transactions::OutboundState::BroadcastUnknown,
                outbound_transactions::OutboundState::Pending,
            ]))
            .limit(CONFIRM_BATCH_SIZE)
            .all(&self.db)
            .await
        {
            Ok(records) => records,
            Err(error) => {
                warn!(error = %error, "Failed to load auxiliary outbound transactions");
                return;
            }
        };

        for outbound in records {
            let Some(network) = Network::from_str_lenient(&outbound.network) else {
                continue;
            };
            let Some(executor) = self.executors.get(&network) else {
                continue;
            };
            let Some(tx_hash) = outbound.tx_hash.as_deref() else {
                continue;
            };

            match executor.check_tx_status(tx_hash, CONFIRM_BLOCKS).await {
                Ok(ChainTxState::Confirmed) => {
                    let _ = self
                        .outbound_store
                        .mark_state(
                            &outbound.id,
                            outbound_transactions::OutboundState::Confirmed,
                            None,
                        )
                        .await;
                }
                Ok(ChainTxState::Failed) => {
                    let _ = self
                        .outbound_store
                        .mark_state(
                            &outbound.id,
                            outbound_transactions::OutboundState::Reverted,
                            Some("Auxiliary transaction reverted on-chain".into()),
                        )
                        .await;
                }
                Ok(ChainTxState::Pending | ChainTxState::Unconfirmed) => {
                    let _ = self
                        .outbound_store
                        .mark_state(
                            &outbound.id,
                            outbound_transactions::OutboundState::Pending,
                            None,
                        )
                        .await;
                }
                Ok(ChainTxState::NotFound) => {
                    if outbound.purpose == outbound_transactions::OutboundPurpose::EnergyFunding
                        && outbound.signed_payload_encrypted.is_none()
                    {
                        warn!(
                            outbound_id = %outbound.id,
                            provider_reference = ?outbound.provider_reference,
                            "Energy funding transaction is not yet visible on-chain; retaining it for reconciliation"
                        );
                        continue;
                    }
                    let payload = match self.outbound_store.decrypt_payload(&outbound) {
                        Ok(payload) => payload,
                        Err(error) => {
                            error!(outbound_id = %outbound.id, error = %error, "Cannot decrypt auxiliary signed payload");
                            continue;
                        }
                    };
                    match executor.recover_broadcast(&payload).await {
                        Ok(crate::services::outbound::RecoveryDisposition::Pending) => {
                            let _ = self
                                .outbound_store
                                .mark_broadcast(
                                    &outbound.id,
                                    crate::services::outbound::BroadcastDisposition::Accepted,
                                    None,
                                )
                                .await;
                        }
                        Ok(crate::services::outbound::RecoveryDisposition::BroadcastUnknown(
                            error,
                        )) => {
                            let _ = self
                                .outbound_store
                                .mark_broadcast(
                                    &outbound.id,
                                    crate::services::outbound::BroadcastDisposition::Unknown,
                                    Some(error),
                                )
                                .await;
                        }
                        Ok(
                            disposition @ (crate::services::outbound::RecoveryDisposition::Expired
                            | crate::services::outbound::RecoveryDisposition::Replaced),
                        ) => {
                            let state = match disposition {
                                crate::services::outbound::RecoveryDisposition::Expired => {
                                    outbound_transactions::OutboundState::Expired
                                }
                                crate::services::outbound::RecoveryDisposition::Replaced => {
                                    outbound_transactions::OutboundState::Replaced
                                }
                                _ => unreachable!(),
                            };
                            let reason = "Auxiliary transaction proven unable to land";
                            match self
                                .outbound_store
                                .stage_terminal_evidence(&outbound.id, state.clone(), reason)
                                .await
                            {
                                Ok(crate::services::outbound::TerminalEvidence::Ready) => {
                                    let _ = self
                                        .outbound_store
                                        .mark_state(&outbound.id, state, Some(reason.into()))
                                        .await;
                                }
                                Ok(crate::services::outbound::TerminalEvidence::Staged)
                                | Ok(crate::services::outbound::TerminalEvidence::Conflict) => {}
                                Err(error) => {
                                    warn!(outbound_id = %outbound.id, error = %error, "Failed to stage auxiliary terminal evidence");
                                }
                            }
                        }
                        Err(error) => {
                            warn!(outbound_id = %outbound.id, error = %error, "Auxiliary transaction recovery failed");
                        }
                    }
                }
                Err(error) => {
                    warn!(outbound_id = %outbound.id, error = %error, "Auxiliary transaction status check failed");
                }
            }
        }
    }

    /// Process all pending withdrawals with semaphore-limited concurrency.
    ///
    /// Broadcast only — confirmation is handled by `confirm_processing_withdrawals()`
    /// in the main loop's next tick.
    async fn process_pending_withdrawals(&self) -> Result<()> {
        let pending = withdrawals::Entity::find()
            .filter(withdrawals::Column::Status.eq(WithdrawalStatus::Pending))
            .all(&self.db)
            .await?;

        if pending.is_empty() {
            return Ok(());
        }

        info!(count = pending.len(), "Processing pending withdrawals");

        for wd in pending {
            let permit = self
                .broadcast_semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| anyhow!("Semaphore closed: {}", e))?;

            let svc = self.clone();
            let wd_id = wd.id.clone();

            tokio::spawn(async move {
                let _permit = permit;
                if let Err(e) = svc.execute_broadcast(wd).await {
                    error!(withdrawal_id = %wd_id, error = %e, "Withdrawal broadcast failed");
                }
                // Confirmation handled by confirm_processing_withdrawals() in next cycle
            });
        }

        Ok(())
    }

    // =========================================================================
    // Broadcast Phase (holds semaphore permit)
    // =========================================================================

    /// Execute the broadcast phase of a withdrawal: CAS → executor dispatch → broadcast.
    /// Returns the tx_hash on success for confirmation polling.
    async fn execute_broadcast(&self, wd: withdrawals::Model) -> Result<String> {
        let wd_id = wd.id.clone();
        debug!(withdrawal_id = %wd_id, network = %wd.network, "Starting withdrawal broadcast");

        // Resolve chain family from network string
        let net_enum = match Network::from_str_lenient(&wd.network) {
            Some(n) => n,
            None => {
                error!(withdrawal_id = %wd_id, network = %wd.network, "Invalid network on withdrawal");
                return Err(anyhow!("Invalid network on withdrawal: {}", wd.network));
            }
        };
        // Look up executor for this network
        let executor = match self.executors.get(&net_enum) {
            Some(e) => e,
            None => {
                error!(withdrawal_id = %wd_id, network = %wd.network, "No payout executor configured");
                return Err(anyhow!(
                    "No payout executor configured for network: {}",
                    wd.network
                ));
            }
        };

        // Look up treasury address for this network
        let treasury = match self.treasury_addresses.get(&net_enum) {
            Some(t) => t,
            None => {
                error!(withdrawal_id = %wd_id, network = %wd.network, "No treasury address configured");
                return Err(anyhow!(
                    "No treasury address configured for network: {}",
                    wd.network
                ));
            }
        };

        // 1. CAS: Mark as Processing (only if still Pending)
        let update_res = withdrawals::Entity::update_many()
            .col_expr(
                withdrawals::Column::Status,
                Expr::value(WithdrawalStatus::Processing),
            )
            .col_expr(
                withdrawals::Column::UpdatedAt,
                Expr::value(chrono::Utc::now()),
            )
            .filter(withdrawals::Column::Id.eq(&wd_id))
            .filter(withdrawals::Column::Status.eq(WithdrawalStatus::Pending))
            .exec(&self.db)
            .await?;

        if update_res.rows_affected == 0 {
            debug!(withdrawal_id = %wd_id, "Withdrawal no longer pending, skipping");
            return Err(anyhow!("Withdrawal no longer pending"));
        }

        // 2. Convert net_amount to u64 (should never fail — validated in request_withdrawal)
        let amount = match u64::try_from(wd.net_amount) {
            Ok(a) => a,
            Err(_) => {
                let reason = format!(
                    "net_amount {} is negative — refusing to broadcast",
                    wd.net_amount
                );
                error!(withdrawal_id = %wd_id, net_amount = wd.net_amount, "Negative net_amount after CAS");
                self.fail_and_refund(&wd, &reason, None).await;
                return Err(anyhow!("{}", reason));
            }
        };

        // 3. Execute payout via chain-specific executor
        // Narrow lock scope: only nonce query → sign → broadcast.
        // CAS and DB writes intentionally stay outside to avoid lock contention.
        // Resolve token contract + decimals from ChainConfig (dynamic: USDT or USDC)
        let chain_config = net_enum.chain_config(&wd.environment);
        let (token_contract, token_decimals) = if wd.currency == "USDC" {
            let contract = chain_config.usdc_contract.clone().unwrap_or_default();
            let decimals = chain_config.usdc_decimals.unwrap_or(6);
            (contract, decimals)
        } else {
            (
                chain_config.usdt_contract.clone(),
                chain_config.usdt_decimals,
            )
        };
        let outbound_id = crate::services::outbound::new_id();
        let mut outbound = crate::services::outbound::preparing_model(
            outbound_id.clone(),
            wd.merchant_id.clone(),
            wd.environment,
            outbound_transactions::OutboundOperationType::Withdrawal,
            wd.network.clone(),
            treasury.clone(),
            wd.to_address.clone(),
            wd.net_amount,
            wd.currency.clone(),
        );
        outbound.withdrawal_id = Set(Some(wd_id.clone()));
        if let Err(error) = crate::services::outbound::create_attempt(&self.db, outbound).await {
            self.fail_and_refund(
                &wd,
                &format!("Failed to create outbound journal: {error}"),
                None,
            )
            .await;
            return Err(error);
        }
        let payout_result = {
            let _lock = if let Some(lock) = self.broadcast_locks.get(&net_enum) {
                Some(lock.lock().await)
            } else {
                None
            };
            executor
                .execute_payout(
                    treasury,
                    &wd.to_address,
                    amount,
                    TREASURY_ACCOUNT_INDEX,
                    TREASURY_PATH_INDEX,
                    &token_contract,
                    token_decimals,
                    &outbound_id,
                    &self.outbound_store,
                )
                .await
            // _lock dropped here
        };
        let payout_result = match payout_result {
            Ok(result) => result,
            Err(e) => {
                error!(withdrawal_id = %wd_id, error = %e, "Payout execution failed");
                let root_failed = self
                    .outbound_store
                    .mark_preparing_failed(&outbound_id, e.to_string())
                    .await;
                crate::services::metrics::inc_payout_broadcast(&wd.network, "failed", "withdrawal");
                match root_failed {
                    Ok(true) => {
                        self.fail_and_refund(&wd, &format!("Payout failed: {}", e), None)
                            .await;
                    }
                    Ok(false) => {
                        warn!(
                            withdrawal_id = %wd_id,
                            outbound_id = %outbound_id,
                            "Withdrawal failure occurred after signing; retaining Processing state for recovery"
                        );
                    }
                    Err(error) => {
                        error!(
                            withdrawal_id = %wd_id,
                            outbound_id = %outbound_id,
                            error = %error,
                            "Cannot establish whether withdrawal failed before signing; refusing refund"
                        );
                    }
                }
                return Err(anyhow!("Payout failed: {}", e));
            }
        };

        if !self
            .outbound_store
            .adopt_executor_result(
                &outbound_id,
                &payout_result.tx_hash,
                payout_result.broadcast_disposition.clone(),
            )
            .await?
        {
            return Err(anyhow!(
                "Withdrawal outbound handoff conflicted for {}",
                outbound_id
            ));
        }

        let tx_hash = payout_result.tx_hash;

        if let Some(funding_tx_hash) = payout_result.funding_tx_hash.clone() {
            let _ = outbound_transactions::Entity::update_many()
                .col_expr(
                    outbound_transactions::Column::FundingTxHash,
                    Expr::value(Some(funding_tx_hash)),
                )
                .filter(outbound_transactions::Column::Id.eq(&outbound_id))
                .exec(&self.db)
                .await;
        }

        // 4. Record tx_hash (so reset_on_startup knows it was sent)
        let business_handoff = withdrawals::Entity::update_many()
            .col_expr(
                withdrawals::Column::TxHash,
                Expr::value(Some(tx_hash.clone())),
            )
            .filter(withdrawals::Column::Id.eq(&wd_id))
            .filter(withdrawals::Column::Status.eq(WithdrawalStatus::Processing))
            .exec(&self.db)
            .await?;
        if business_handoff.rows_affected != 1 {
            return Err(anyhow!(
                "Withdrawal {} changed state before transaction handoff",
                wd_id
            ));
        }

        crate::services::metrics::inc_payout_broadcast(&wd.network, "success", "withdrawal");
        info!(
            withdrawal_id = %wd_id,
            tx_hash = %tx_hash,
            net_amount = wd.net_amount,
            to = %wd.to_address,
            network = %wd.network,
            "Withdrawal handed off to confirmation"
        );

        Ok(tx_hash)
    }

    /// Scan all Processing withdrawals with tx_hash and check on-chain status.
    ///
    /// Called every main loop tick (30s). Each record is checked once per cycle.
    async fn confirm_processing_withdrawals(&self) {
        use crate::entity::transactions::ChainTxState;

        // Repair a crash after the durable journal write but before the parent hash cache update.
        let _ = self
            .db
            .execute(sea_orm::Statement::from_string(
                sea_orm::DbBackend::Postgres,
                r#"
            UPDATE withdrawals w
            SET tx_hash = ot.tx_hash
            FROM outbound_transactions ot
            WHERE ot.withdrawal_id = w.id
              AND w.status = 'Processing'
              AND w.tx_hash IS NULL
              AND ot.tx_hash IS NOT NULL
              AND ot.purpose = 'token_transfer'
              AND ot.parent_transaction_id IS NULL
              AND ot.operation_type = 'withdrawal'
              AND ot.state IN ('Signed', 'BroadcastUnknown', 'Pending')
            "#,
            ))
            .await;

        let processing = match withdrawals::Entity::find()
            .filter(withdrawals::Column::Status.eq(WithdrawalStatus::Processing))
            .filter(withdrawals::Column::TxHash.is_not_null())
            .limit(CONFIRM_BATCH_SIZE)
            .all(&self.db)
            .await
        {
            Ok(records) => records,
            Err(e) => {
                warn!(error = %e, "Failed to query Processing withdrawals");
                return;
            }
        };

        for wd in &processing {
            let tx_hash = match &wd.tx_hash {
                Some(h) => h.to_string(),
                None => continue,
            };
            let wd_id = &wd.id;

            let net_enum = Network::from_str_lenient(&wd.network);
            let executor = net_enum.as_ref().and_then(|n| self.executors.get(n));

            let executor = match executor {
                Some(e) => e,
                None => {
                    warn!(withdrawal_id = %wd_id, network = %wd.network, "No executor for withdrawal confirmation");
                    continue;
                }
            };

            match executor.check_tx_status(&tx_hash, CONFIRM_BLOCKS).await {
                Ok(ChainTxState::Confirmed) => {
                    let outbound = match self
                        .outbound_store
                        .find_for_withdrawal_tx(wd_id, &tx_hash)
                        .await
                    {
                        Ok(Some(outbound)) => outbound,
                        Ok(None) => {
                            error!(withdrawal_id = %wd_id, tx_hash = %tx_hash, "Missing matching root outbound journal for confirmed withdrawal");
                            continue;
                        }
                        Err(error) => {
                            error!(withdrawal_id = %wd_id, error = %error, "Failed to load withdrawal outbound journal");
                            continue;
                        }
                    };
                    match self.complete_withdrawal(wd, &outbound.id).await {
                        Ok(true) => {}
                        Ok(false) => continue,
                        Err(error) => {
                            error!(withdrawal_id = %wd_id, error = %error, "Failed to atomically confirm withdrawal");
                            continue;
                        }
                    }
                    crate::services::metrics::inc_payout_confirmed(
                        &wd.network,
                        "confirmed",
                        "withdrawal",
                    );
                    info!(withdrawal_id = %wd_id, tx_hash = %tx_hash, "Withdrawal confirmed on-chain");

                    // Upsert trusted address (post-confirmation)
                    self.upsert_trusted_address(&wd.merchant_id, &wd.network, &wd.to_address)
                        .await;
                }
                Ok(ChainTxState::Failed) => {
                    let outbound = match self
                        .outbound_store
                        .find_for_withdrawal_tx(wd_id, &tx_hash)
                        .await
                    {
                        Ok(Some(outbound)) => outbound,
                        _ => continue,
                    };
                    crate::services::metrics::inc_payout_confirmed(
                        &wd.network,
                        "failed",
                        "withdrawal",
                    );
                    warn!(withdrawal_id = %wd_id, tx_hash = %tx_hash, "Withdrawal TX failed on-chain, refunding");
                    self.fail_and_refund(
                        wd,
                        "Transaction failed on-chain",
                        Some((outbound.id, outbound_transactions::OutboundState::Reverted)),
                    )
                    .await;
                }
                Ok(ChainTxState::Pending | ChainTxState::Unconfirmed) => {
                    if let Ok(Some(outbound)) = self
                        .outbound_store
                        .find_for_withdrawal_tx(wd_id, &tx_hash)
                        .await
                    {
                        let _ = self
                            .outbound_store
                            .mark_state(
                                &outbound.id,
                                outbound_transactions::OutboundState::Pending,
                                None,
                            )
                            .await;
                    }
                    debug!(withdrawal_id = %wd_id, "Withdrawal TX still pending/unconfirmed");
                }
                Ok(ChainTxState::NotFound) => {
                    self.recover_withdrawal_broadcast(wd, executor.as_ref())
                        .await;
                }
                Err(e) => {
                    warn!(withdrawal_id = %wd_id, error = %e, "Error checking withdrawal TX status");
                }
            }
        }

        // Orphan recovery: Processing + no tx_hash for > 5 min = crash between CAS and broadcast.
        // Safe to reset because active broadcasts complete in seconds, not minutes.
        let orphan_cutoff = chrono::Utc::now() - chrono::Duration::seconds(300);
        let orphan_result = self
            .db
            .execute(sea_orm::Statement::from_sql_and_values(
                sea_orm::DbBackend::Postgres,
                r#"
            WITH stale AS (
                SELECT w.id
                FROM withdrawals w
                WHERE w.status = 'Processing'
                  AND w.tx_hash IS NULL
                  AND w.updated_at < $1
                  AND NOT EXISTS (
                      SELECT 1 FROM outbound_transactions ot
                      WHERE ot.withdrawal_id = w.id
                        AND ot.purpose = 'token_transfer'
                        AND ot.parent_transaction_id IS NULL
                        AND ot.state IN ('Signed', 'BroadcastUnknown', 'Pending')
                  )
            ), failed_journals AS (
                UPDATE outbound_transactions ot
                SET state = 'Failed',
                    error_message = 'Auto-reset: interrupted before transaction signing',
                    updated_at = NOW()
                FROM stale
                WHERE ot.withdrawal_id = stale.id
                  AND ot.purpose = 'token_transfer'
                  AND ot.parent_transaction_id IS NULL
                  AND ot.state = 'Preparing'
                RETURNING ot.id
            )
            UPDATE withdrawals w
            SET status = 'Pending',
                error_reason = 'Auto-reset: interrupted before transaction signing',
                updated_at = NOW()
            FROM stale
            WHERE w.id = stale.id
            "#,
                [orphan_cutoff.into()],
            ))
            .await;

        if let Ok(result) = orphan_result {
            if result.rows_affected() > 0 {
                warn!(
                    count = result.rows_affected(),
                    "Reset orphaned Processing withdrawals (no tx_hash) back to Pending"
                );
            }
        }
    }

    async fn recover_withdrawal_broadcast(
        &self,
        wd: &withdrawals::Model,
        executor: &dyn PayoutExecutor,
    ) {
        let Some(tx_hash) = wd.tx_hash.as_deref() else {
            return;
        };
        let outbound = match self
            .outbound_store
            .find_for_withdrawal_tx(&wd.id, tx_hash)
            .await
        {
            Ok(Some(outbound)) => outbound,
            Ok(None) => {
                warn!(withdrawal_id = %wd.id, "Missing outbound journal; refusing timeout-based refund");
                return;
            }
            Err(error) => {
                warn!(withdrawal_id = %wd.id, error = %error, "Failed to load outbound journal");
                return;
            }
        };
        let payload = match self.outbound_store.decrypt_payload(&outbound) {
            Ok(payload) => payload,
            Err(error) => {
                error!(outbound_id = %outbound.id, error = %error, "Cannot decrypt signed withdrawal payload");
                return;
            }
        };

        match executor.recover_broadcast(&payload).await {
            Ok(crate::services::outbound::RecoveryDisposition::Pending) => {
                let _ = self
                    .outbound_store
                    .mark_broadcast(
                        &outbound.id,
                        crate::services::outbound::BroadcastDisposition::Accepted,
                        None,
                    )
                    .await;
            }
            Ok(crate::services::outbound::RecoveryDisposition::BroadcastUnknown(error)) => {
                let _ = self
                    .outbound_store
                    .mark_broadcast(
                        &outbound.id,
                        crate::services::outbound::BroadcastDisposition::Unknown,
                        Some(error),
                    )
                    .await;
            }
            Ok(
                disposition @ (crate::services::outbound::RecoveryDisposition::Expired
                | crate::services::outbound::RecoveryDisposition::Replaced),
            ) => {
                let state = match disposition {
                    crate::services::outbound::RecoveryDisposition::Expired => {
                        outbound_transactions::OutboundState::Expired
                    }
                    crate::services::outbound::RecoveryDisposition::Replaced => {
                        outbound_transactions::OutboundState::Replaced
                    }
                    _ => unreachable!(),
                };
                let reason = format!("Signed transaction proven unable to land: {state:?}");
                match self
                    .outbound_store
                    .stage_terminal_evidence(&outbound.id, state.clone(), &reason)
                    .await
                {
                    Ok(crate::services::outbound::TerminalEvidence::Ready) => {
                        self.fail_and_refund(wd, &reason, Some((outbound.id, state)))
                            .await;
                    }
                    Ok(crate::services::outbound::TerminalEvidence::Staged) => {
                        warn!(outbound_id = %outbound.id, "Staged terminal withdrawal evidence; awaiting recheck");
                    }
                    Ok(crate::services::outbound::TerminalEvidence::Conflict) => {}
                    Err(error) => {
                        warn!(outbound_id = %outbound.id, error = %error, "Failed to stage terminal withdrawal evidence");
                    }
                }
            }
            Err(error) => {
                warn!(outbound_id = %outbound.id, error = %error, "Withdrawal broadcast recovery failed");
            }
        }
    }

    // =========================================================================
    // Failure Handling
    // =========================================================================

    async fn complete_withdrawal(
        &self,
        wd: &withdrawals::Model,
        outbound_id: &str,
    ) -> Result<bool> {
        let txn = self.db.begin().await?;
        let parent = withdrawals::Entity::update_many()
            .col_expr(
                withdrawals::Column::Status,
                Expr::value(WithdrawalStatus::Completed),
            )
            .col_expr(
                withdrawals::Column::CompletedAt,
                Expr::value(Some(chrono::Utc::now())),
            )
            .col_expr(
                withdrawals::Column::UpdatedAt,
                Expr::value(chrono::Utc::now()),
            )
            .filter(withdrawals::Column::Id.eq(&wd.id))
            .filter(withdrawals::Column::Status.eq(WithdrawalStatus::Processing))
            .exec(&txn)
            .await?;
        if parent.rows_affected != 1 {
            txn.rollback().await?;
            return Ok(false);
        }

        if !self
            .outbound_store
            .mark_state_on(
                &txn,
                outbound_id,
                outbound_transactions::OutboundState::Confirmed,
                None,
            )
            .await?
        {
            txn.rollback().await?;
            return Ok(false);
        }

        txn.commit().await?;
        Ok(true)
    }

    /// Mark withdrawal as Failed and refund the full amount back to merchant balance.
    async fn fail_and_refund(
        &self,
        wd: &withdrawals::Model,
        reason: &str,
        terminal: Option<(String, outbound_transactions::OutboundState)>,
    ) {
        let env = wd.environment;

        let wd_id = wd.id.clone();
        let merchant_id = wd.merchant_id.clone();
        let gross_amount = wd.amount;
        let reason_owned = reason.to_string();
        let wd_network = wd.network.clone();

        // Parse network from withdrawal record for correct per-chain refund
        let network_enum = match crate::entity::Network::from_str_lenient(&wd_network) {
            Some(n) => n,
            None => {
                error!(
                    withdrawal_id = %wd_id,
                    network = %wd_network,
                    "Cannot parse network for refund; refusing to credit another chain balance"
                );
                self.alerting_service.send_alert(
                    "payout_refund_failed",
                    AlertLevel::Critical,
                    &format!(
                        "Withdrawal {} has invalid network {}; refund requires manual intervention.",
                        wd_id, wd_network
                    ),
                );
                return;
            }
        };

        let txn = match self.db.begin().await {
            Ok(txn) => txn,
            Err(error) => {
                error!(withdrawal_id = %wd.id, error = %error, "CRITICAL: Cannot begin withdrawal refund txn");
                return;
            }
        };

        let parent = withdrawals::Entity::update_many()
            .col_expr(
                withdrawals::Column::Status,
                Expr::value(WithdrawalStatus::Failed),
            )
            .col_expr(
                withdrawals::Column::ErrorReason,
                Expr::value(Some(reason_owned.clone())),
            )
            .col_expr(
                withdrawals::Column::UpdatedAt,
                Expr::value(chrono::Utc::now()),
            )
            .filter(withdrawals::Column::Id.eq(&wd_id))
            .filter(withdrawals::Column::Status.eq(WithdrawalStatus::Processing))
            .exec(&txn)
            .await;
        let parent_applied = matches!(parent, Ok(ref result) if result.rows_affected == 1);
        if !parent_applied {
            let _ = txn.rollback().await;
            debug!(withdrawal_id = %wd_id, "Withdrawal refund skipped after state conflict");
            return;
        }

        if let Some((outbound_id, state)) = terminal {
            match self
                .outbound_store
                .mark_state_on(&txn, &outbound_id, state, Some(reason_owned.clone()))
                .await
            {
                Ok(true) => {}
                Ok(false) => {
                    let _ = txn.rollback().await;
                    debug!(withdrawal_id = %wd_id, outbound_id = %outbound_id, "Withdrawal terminal transition lost CAS race");
                    return;
                }
                Err(error) => {
                    let _ = txn.rollback().await;
                    error!(withdrawal_id = %wd_id, error = %error, "Failed to update withdrawal outbound journal");
                    return;
                }
            }
        }

        if let Err(error) = self
            .billing_service
            .refund_cost(
                &txn,
                &merchant_id,
                None,
                gross_amount,
                Some(format!("wd_refund_{}", wd_id)),
                Some(format!("Withdrawal {} failed: {}", wd_id, reason_owned)),
                network_enum,
                env,
                &wd.currency,
            )
            .await
        {
            let _ = txn.rollback().await;
            error!(
                withdrawal_id = %wd.id,
                error = %error,
                "CRITICAL: Failed to refund withdrawal balance"
            );
            self.alerting_service.send_alert(
                "payout_refund_failed",
                AlertLevel::Critical,
                &format!(
                    "Withdrawal {} refund failed: {}. Manual intervention required.",
                    wd.id, error
                ),
            );
            return;
        }

        if let Err(error) = txn.commit().await {
            error!(withdrawal_id = %wd.id, error = %error, "CRITICAL: Failed to commit withdrawal refund");
            self.alerting_service.send_alert(
                "payout_refund_failed",
                AlertLevel::Critical,
                &format!("Withdrawal {} refund commit failed: {}", wd.id, error),
            );
        }
    }

    // =========================================================================
    // Payout API: create_payout (merchant → arbitrary address)
    // =========================================================================

    /// Create a payout from merchant balance to an arbitrary address.
    ///
    /// Unlike withdrawals (to merchant's own collection_address), payouts
    /// send to any address specified by the merchant via Public API.
    ///
    /// Flow:
    /// 1. Validate address + network
    /// 2. AML check on destination address
    /// 3. Amount limit checks (single + daily cumulative)
    /// 4. Atomic: lock balance → debit → billing log → insert payout record
    ///
    /// Idempotency enforced by UNIQUE(merchant_id, environment, idempotency_key).
    pub async fn create_payout(
        &self,
        merchant_id: &str,
        amount: i64,
        environment: crate::entity::Environment,
        network: crate::entity::Network,
        to_address: String,
        idempotency_key: String,
        description: Option<String>,
        metadata: Option<serde_json::Value>,
        currency: &str,
    ) -> Result<crate::entity::payouts::Model, PayoutError> {
        use crate::entity::payouts;

        if amount <= 0 {
            return Err(PayoutError::InvalidAmount(
                "Payout amount must be positive".into(),
            ));
        }

        // Validate destination address (network-aware)
        match network.chain_family() {
            crate::entity::ChainFamily::Tron => {
                crate::services::tron::address::validate_address(&to_address).map_err(|e| {
                    PayoutError::InvalidAddress {
                        message: format!("Invalid TRON address: {}", e),
                        param: "to_address".into(),
                    }
                })?;
            }
            crate::entity::ChainFamily::Evm => {
                if !to_address.starts_with("0x") || to_address.len() != 42 {
                    return Err(PayoutError::InvalidAddress {
                        message: "Invalid EVM address: must be 0x-prefixed 42-char hex".into(),
                        param: "to_address".into(),
                    });
                }
                // Validate hex characters (strip 0x prefix, decode remaining 40 hex chars)
                if hex::decode(&to_address[2..]).is_err() {
                    return Err(PayoutError::InvalidAddress {
                        message: "Invalid EVM address: contains non-hex characters".into(),
                        param: "to_address".into(),
                    });
                }
            }
            crate::entity::ChainFamily::Solana => {
                crate::entity::network::validate_solana_address(&to_address).map_err(|e| {
                    PayoutError::InvalidAddress {
                        message: format!("Invalid Solana address: {}", e),
                        param: "to_address".into(),
                    }
                })?;
            }
        }

        // Guard against self-transfer (treasury → treasury)
        let treasury_address = self
            .treasury_addresses
            .get(&network)
            .cloned()
            .unwrap_or_default();
        let is_self_transfer = match network.chain_family() {
            crate::entity::ChainFamily::Solana => to_address == treasury_address, // Base58 is case-sensitive
            _ => to_address.to_lowercase() == treasury_address.to_lowercase(),
        };
        if is_self_transfer {
            return Err(PayoutError::SelfTransfer {
                message: "Destination address cannot be the platform treasury address".into(),
                param: "to_address".into(),
            });
        }

        // AML check — mandatory for payouts (arbitrary destination address)
        let aml_result = self
            .aml_service
            .check_address(&to_address, network.as_str())
            .await
            .map_err(|e| PayoutError::Internal(e))?;
        if let RiskResult::Blocked { reason } = aml_result {
            return Err(PayoutError::InvalidAddress {
                message: format!(
                    "Destination address {} blocked by AML check: {}",
                    to_address, reason
                ),
                param: "to_address".into(),
            });
        }

        // Compute outbound fee (per-chain override → global fallback)
        let chain_fee = self.chain_outbound_fees.get(&network).copied();
        let payout_fee = self.fee_config.outbound_fee(amount, chain_fee);
        let net_amount = amount - payout_fee;

        if net_amount <= 0 {
            let amount_str = crate::api::dtos::checkout::from_micro(amount, currency);
            let fee_str = crate::api::dtos::checkout::from_micro(payout_fee, currency);
            return Err(PayoutError::InvalidAmount(format!(
                "Payout amount ({} {}) is too small to cover fee ({} {})",
                amount_str, currency, fee_str, currency
            )));
        }

        // Amount limit checks
        // Single payout max: 10,000 USDT = 10_000_000_000 microunits
        const MAX_SINGLE_PAYOUT: i64 = 10_000_000_000;
        if amount > MAX_SINGLE_PAYOUT {
            let max_str = crate::api::dtos::checkout::from_micro(MAX_SINGLE_PAYOUT, currency);
            return Err(PayoutError::InvalidAmount(format!(
                "Payout amount exceeds single payout limit ({} {})",
                max_str, currency
            )));
        }

        // Daily cumulative limit: 50,000 USDT = 50_000_000_000 microunits
        const MAX_DAILY_PAYOUT: i64 = 50_000_000_000;
        let today_start = chrono::Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        // PostgreSQL SUM(bigint) returns NUMERIC, must decode as Decimal then convert to i64
        let daily_total: i64 = {
            use sea_orm::prelude::Decimal;
            use sea_orm::{DeriveColumn, EnumIter, QuerySelect};

            #[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
            enum QueryAs {
                Total,
            }

            let total: Option<Decimal> = payouts::Entity::find()
                .filter(payouts::Column::MerchantId.eq(merchant_id))
                .filter(payouts::Column::Environment.eq(environment.clone()))
                .filter(payouts::Column::CreatedAt.gte(today_start))
                .filter(payouts::Column::Status.ne(payouts::PayoutStatus::Failed))
                .select_only()
                .column_as(payouts::Column::Amount.sum(), QueryAs::Total)
                .into_values::<Option<Decimal>, QueryAs>()
                .one(&self.db)
                .await?
                .flatten();
            total
                .and_then(|d| d.to_string().parse::<i64>().ok())
                .unwrap_or(0)
        };

        if daily_total + amount > MAX_DAILY_PAYOUT {
            let max_str = crate::api::dtos::checkout::from_micro(MAX_DAILY_PAYOUT, currency);
            let total_str = crate::api::dtos::checkout::from_micro(daily_total, currency);
            return Err(PayoutError::InvalidAmount(format!(
                "Payout would exceed daily limit ({} {}). Today's total: {} {}",
                max_str, currency, total_str, currency
            )));
        }

        let merchant_id_owned = merchant_id.to_string();
        let billing_service = self.billing_service.clone();
        let currency_owned = currency.to_string();

        // ── Risk Control: determine initial status for payout ──
        // NOTE (TOCTOU): Same as request_withdrawal — runs outside transaction.
        // Practical risk is negligible (see comment in request_withdrawal).
        let initial_status = self
            .determine_payout_status(merchant_id, &network, &to_address, amount)
            .await;
        let log_status = initial_status.clone();

        let payout_result = self
            .db
            .transaction::<_, payouts::Model, sea_orm::DbErr>(|txn| {
                let merchant_id = merchant_id_owned.clone();
                let billing_service = billing_service.clone();
                let to_address = to_address.clone();
                let env = environment.clone();
                let net = network.clone();
                let network_str = network.as_str().to_string();
                let idem_key = idempotency_key.clone();
                let currency = currency_owned.clone();
                let desc = description.clone();
                let meta = metadata.clone();

                Box::pin(async move {
                    // 1. Lock chain account row (SELECT ... FOR UPDATE)
                    let chain_account = billing_service
                        .get_chain_balance_lock(txn, &merchant_id, env.clone(), net.clone())
                        .await
                        .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

                    // 2. Balance check (dynamic: USDT or USDC)
                    let current_balance = if currency == "USDC" {
                        chain_account.usdc_balance
                    } else {
                        chain_account.usdt_balance
                    };
                    if current_balance < amount {
                        return Err(sea_orm::DbErr::Custom(format!(
                            "INSUFFICIENT_BALANCE:{}:{}",
                            current_balance, amount
                        )));
                    }

                    // 3. Debit balance
                    let previous_balance = current_balance;
                    let new_balance = current_balance - amount;
                    let mut account_active: crate::entity::merchant_chain_accounts::ActiveModel =
                        chain_account.into();
                    if currency == "USDC" {
                        account_active.usdc_balance = Set(new_balance);
                    } else {
                        account_active.usdt_balance = Set(new_balance);
                    }
                    account_active.updated_at = Set(chrono::Utc::now().into());
                    account_active.update(txn).await?;

                    // 4. Generate payout ID
                    let po_id = format!("po_{}", uuid::Uuid::new_v4().simple());

                    // 5. Create billing log (Payout — negative amount_change)
                    let log = billing_logs::ActiveModel {
                        id: Set(format!("bl_{}", uuid::Uuid::new_v4().simple())),
                        environment: Set(env.clone()),
                        network: Set(network_str.clone()),
                        merchant_id: Set(merchant_id.clone()),
                        session_id: Set(None),
                        external_ref_id: Set(Some(po_id.clone())),
                        billing_type: Set(billing_logs::BillingType::Payout),
                        previous_balance: Set(previous_balance),
                        amount_change: Set(-amount),
                        balance_after: Set(new_balance),
                        description: Set(Some(format!(
                            "Payout: gross={} {} fee={} {} net={} {} to={}",
                            amount as f64 / 1_000_000.0,
                            currency,
                            payout_fee as f64 / 1_000_000.0,
                            currency,
                            net_amount as f64 / 1_000_000.0,
                            currency,
                            to_address
                        ))),
                        token: Set(currency.clone()),
                        gross_amount: Set(None),
                        fee_amount: Set(None),
                        created_at: Set(chrono::Utc::now().into()),
                    };
                    log.insert(txn).await?;

                    // 6. Insert payout record
                    // UNIQUE(merchant_id, environment, idempotency_key) enforces idempotency
                    let payout = payouts::ActiveModel {
                        id: Set(po_id),
                        merchant_id: Set(merchant_id),
                        environment: Set(env),
                        network: Set(network_str),
                        to_address: Set(to_address),
                        amount: Set(amount),
                        fee: Set(payout_fee),
                        net_amount: Set(net_amount),
                        status: Set(initial_status),
                        tx_hash: Set(None),
                        error_reason: Set(None),
                        idempotency_key: Set(idem_key),
                        description: Set(desc),
                        metadata: Set(meta),
                        currency: Set(currency),
                        created_at: Set(chrono::Utc::now().into()),
                        updated_at: Set(chrono::Utc::now().into()),
                        completed_at: Set(None),
                        reviewed_by: Set(None),
                        reviewed_at: Set(None),
                    };

                    let saved = payout.insert(txn).await?;

                    info!(
                        payout_id = %saved.id,
                        merchant_id = %saved.merchant_id,
                        amount = saved.amount,
                        net_amount = saved.net_amount,
                        to = %saved.to_address,
                        status = ?log_status,
                        "Payout created"
                    );

                    Ok(saved)
                })
            })
            .await
            .map_err(|e| {
                let msg = e.to_string();
                // Parse structured balance error
                if let Some(rest) = msg.strip_prefix("Custom Error: INSUFFICIENT_BALANCE:") {
                    let parts: Vec<&str> = rest.splitn(3, ':').collect();
                    if parts.len() >= 2 {
                        let have = parts[0].parse::<i64>().unwrap_or(0);
                        let need = parts[1].parse::<i64>().unwrap_or(0);
                        let currency = parts.get(2).unwrap_or(&"USDT").to_string();
                        return PayoutError::InsufficientBalance {
                            have,
                            need,
                            currency,
                        };
                    }
                }
                // Check for idempotency key conflict (unique constraint violation)
                if msg.contains("idx_payouts_idempotency") {
                    return PayoutError::IdempotencyConflict;
                }
                match e {
                    sea_orm::TransactionError::Transaction(db_err) => PayoutError::Database(db_err),
                    sea_orm::TransactionError::Connection(db_err) => PayoutError::Database(db_err),
                }
            });

        // Idempotent replay: if conflict, return existing payout instead of 409
        let payout = match payout_result {
            Ok(p) => p,
            Err(PayoutError::IdempotencyConflict) => {
                let existing = payouts::Entity::find()
                    .filter(payouts::Column::MerchantId.eq(merchant_id))
                    .filter(payouts::Column::Environment.eq(environment.clone()))
                    .filter(payouts::Column::IdempotencyKey.eq(&idempotency_key))
                    .one(&self.db)
                    .await?;
                match existing {
                    Some(payout) => {
                        info!(
                            payout_id = %payout.id,
                            idempotency_key = %idempotency_key,
                            "Idempotent replay: returning existing payout"
                        );
                        payout
                    }
                    None => return Err(PayoutError::IdempotencyConflict),
                }
            }
            Err(e) => return Err(e),
        };

        Ok(payout)
    }

    // =========================================================================
    // Payout Query API
    // =========================================================================

    /// Get a single payout by ID (with merchant ownership check).
    pub async fn get_payout(
        &self,
        payout_id: &str,
        merchant_ids: &[String],
    ) -> Result<Option<crate::entity::payouts::Model>> {
        use crate::entity::payouts;
        let payout = payouts::Entity::find_by_id(payout_id)
            .filter(payouts::Column::MerchantId.is_in(merchant_ids))
            .one(&self.db)
            .await?;
        Ok(payout)
    }

    /// List payouts for a merchant with pagination.
    pub async fn list_payouts(
        &self,
        merchant_ids: &[String],
        environment: crate::entity::Environment,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<crate::entity::payouts::Model>, u64)> {
        use crate::entity::payouts;
        use sea_orm::{PaginatorTrait, QueryOrder};

        let paginator = payouts::Entity::find()
            .filter(payouts::Column::MerchantId.is_in(merchant_ids))
            .filter(payouts::Column::Environment.eq(environment))
            .order_by_desc(payouts::Column::CreatedAt)
            .paginate(&self.db, page_size);

        let total = paginator.num_items().await?;
        let items = paginator.fetch_page(page.saturating_sub(1)).await?;

        Ok((items, total))
    }

    // =========================================================================
    // Payout Worker
    // =========================================================================

    /// Process all pending payouts with semaphore-limited concurrency.
    ///
    /// Broadcast only — confirmation is handled by `confirm_processing_payouts()`
    /// in the main loop's next tick.
    async fn process_pending_payouts(&self) -> Result<()> {
        use crate::entity::payouts;

        let pending = payouts::Entity::find()
            .filter(payouts::Column::Status.eq(payouts::PayoutStatus::Pending))
            .all(&self.db)
            .await?;

        if pending.is_empty() {
            return Ok(());
        }

        info!(count = pending.len(), "Processing pending payouts");

        for po in pending {
            let permit = self
                .broadcast_semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| anyhow!("Semaphore closed: {}", e))?;

            let svc = self.clone();
            let po_id = po.id.clone();

            tokio::spawn(async move {
                let _permit = permit;
                if let Err(e) = svc.execute_payout_broadcast(po).await {
                    error!(payout_id = %po_id, error = %e, "Payout broadcast failed");
                }
                // Confirmation handled by confirm_processing_payouts() in next cycle
            });
        }

        Ok(())
    }

    /// Execute broadcast for a payout record. Mirrors execute_broadcast logic.
    async fn execute_payout_broadcast(&self, po: crate::entity::payouts::Model) -> Result<String> {
        use crate::entity::payouts;

        let po_id = po.id.clone();
        debug!(payout_id = %po_id, network = %po.network, "Starting payout broadcast");

        let net_enum = match Network::from_str_lenient(&po.network) {
            Some(n) => n,
            None => {
                error!(payout_id = %po_id, network = %po.network, "Invalid network on payout");
                return Err(anyhow!("Invalid network on payout: {}", po.network));
            }
        };
        // Look up executor for this network
        let executor = match self.executors.get(&net_enum) {
            Some(e) => e,
            None => {
                error!(payout_id = %po_id, network = %po.network, "No payout executor configured");
                return Err(anyhow!("No payout executor for network: {}", po.network));
            }
        };

        let treasury = match self.treasury_addresses.get(&net_enum) {
            Some(t) => t,
            None => {
                error!(payout_id = %po_id, network = %po.network, "No treasury address configured");
                return Err(anyhow!("No treasury for network: {}", po.network));
            }
        };

        // CAS: Pending → Processing
        let update_res = payouts::Entity::update_many()
            .col_expr(
                payouts::Column::Status,
                Expr::value(payouts::PayoutStatus::Processing),
            )
            .col_expr(payouts::Column::UpdatedAt, Expr::value(chrono::Utc::now()))
            .filter(payouts::Column::Id.eq(&po_id))
            .filter(payouts::Column::Status.eq(payouts::PayoutStatus::Pending))
            .exec(&self.db)
            .await?;

        if update_res.rows_affected == 0 {
            debug!(payout_id = %po_id, "Payout no longer pending, skipping");
            return Err(anyhow!("Payout no longer pending"));
        }

        let amount = match u64::try_from(po.net_amount) {
            Ok(a) => a,
            Err(_) => {
                let reason = format!("net_amount {} is negative", po.net_amount);
                error!(payout_id = %po_id, "Negative net_amount on payout");
                self.fail_and_refund_payout(&po, &reason, None).await;
                return Err(anyhow!("{}", reason));
            }
        };

        // Execute via chain-specific executor (with nonce lock)
        // Resolve token contract + decimals from ChainConfig (dynamic: USDT or USDC)
        let chain_config = net_enum.chain_config(&po.environment);
        let (token_contract, token_decimals) = if po.currency == "USDC" {
            let contract = chain_config.usdc_contract.clone().unwrap_or_default();
            let decimals = chain_config.usdc_decimals.unwrap_or(6);
            (contract, decimals)
        } else {
            (
                chain_config.usdt_contract.clone(),
                chain_config.usdt_decimals,
            )
        };
        let outbound_id = crate::services::outbound::new_id();
        let mut outbound = crate::services::outbound::preparing_model(
            outbound_id.clone(),
            po.merchant_id.clone(),
            po.environment,
            outbound_transactions::OutboundOperationType::Payout,
            po.network.clone(),
            treasury.clone(),
            po.to_address.clone(),
            po.net_amount,
            po.currency.clone(),
        );
        outbound.payout_id = Set(Some(po_id.clone()));
        if let Err(error) = crate::services::outbound::create_attempt(&self.db, outbound).await {
            self.fail_and_refund_payout(
                &po,
                &format!("Failed to create outbound journal: {error}"),
                None,
            )
            .await;
            return Err(error);
        }
        let payout_result = {
            let _lock = if let Some(lock) = self.broadcast_locks.get(&net_enum) {
                Some(lock.lock().await)
            } else {
                None
            };
            executor
                .execute_payout(
                    treasury,
                    &po.to_address,
                    amount,
                    TREASURY_ACCOUNT_INDEX,
                    TREASURY_PATH_INDEX,
                    &token_contract,
                    token_decimals,
                    &outbound_id,
                    &self.outbound_store,
                )
                .await
        };
        let payout_result = match payout_result {
            Ok(result) => result,
            Err(e) => {
                error!(payout_id = %po_id, error = %e, "Payout execution failed");
                let root_failed = self
                    .outbound_store
                    .mark_preparing_failed(&outbound_id, e.to_string())
                    .await;
                crate::services::metrics::inc_payout_broadcast(&po.network, "failed", "payout");
                match root_failed {
                    Ok(true) => {
                        self.fail_and_refund_payout(&po, &format!("Payout failed: {}", e), None)
                            .await;
                    }
                    Ok(false) => {
                        warn!(
                            payout_id = %po_id,
                            outbound_id = %outbound_id,
                            "Payout failure occurred after signing; retaining Processing state for recovery"
                        );
                    }
                    Err(error) => {
                        error!(
                            payout_id = %po_id,
                            outbound_id = %outbound_id,
                            error = %error,
                            "Cannot establish whether payout failed before signing; refusing refund"
                        );
                    }
                }
                return Err(anyhow!("Payout failed: {}", e));
            }
        };

        if !self
            .outbound_store
            .adopt_executor_result(
                &outbound_id,
                &payout_result.tx_hash,
                payout_result.broadcast_disposition.clone(),
            )
            .await?
        {
            return Err(anyhow!(
                "Payout outbound handoff conflicted for {}",
                outbound_id
            ));
        }

        let tx_hash = payout_result.tx_hash;

        if let Some(funding_tx_hash) = payout_result.funding_tx_hash.clone() {
            let _ = outbound_transactions::Entity::update_many()
                .col_expr(
                    outbound_transactions::Column::FundingTxHash,
                    Expr::value(Some(funding_tx_hash)),
                )
                .filter(outbound_transactions::Column::Id.eq(&outbound_id))
                .exec(&self.db)
                .await;
        }

        // Record tx_hash
        let business_handoff = payouts::Entity::update_many()
            .col_expr(payouts::Column::TxHash, Expr::value(Some(tx_hash.clone())))
            .col_expr(payouts::Column::UpdatedAt, Expr::value(chrono::Utc::now()))
            .filter(payouts::Column::Id.eq(&po_id))
            .filter(payouts::Column::Status.eq(payouts::PayoutStatus::Processing))
            .exec(&self.db)
            .await?;
        if business_handoff.rows_affected != 1 {
            return Err(anyhow!(
                "Payout {} changed state before transaction handoff",
                po_id
            ));
        }

        crate::services::metrics::inc_payout_broadcast(&po.network, "success", "payout");
        info!(
            payout_id = %po_id,
            tx_hash = %tx_hash,
            net_amount = po.net_amount,
            to = %po.to_address,
            network = %po.network,
            "Payout handed off to confirmation"
        );

        Ok(tx_hash)
    }

    /// Scan all Processing payouts with tx_hash and check on-chain status.
    ///
    /// Called every main loop tick (30s). Includes outbox webhook on confirmation.
    async fn confirm_processing_payouts(&self) {
        use crate::entity::payouts;
        use crate::entity::transactions::ChainTxState;

        let _ = self
            .db
            .execute(sea_orm::Statement::from_string(
                sea_orm::DbBackend::Postgres,
                r#"
            UPDATE payouts p
            SET tx_hash = ot.tx_hash, updated_at = NOW()
            FROM outbound_transactions ot
            WHERE ot.payout_id = p.id
              AND p.status = 'Processing'
              AND p.tx_hash IS NULL
              AND ot.tx_hash IS NOT NULL
              AND ot.purpose = 'token_transfer'
              AND ot.parent_transaction_id IS NULL
              AND ot.operation_type = 'payout'
              AND ot.state IN ('Signed', 'BroadcastUnknown', 'Pending')
            "#,
            ))
            .await;

        let processing = match payouts::Entity::find()
            .filter(payouts::Column::Status.eq(payouts::PayoutStatus::Processing))
            .filter(payouts::Column::TxHash.is_not_null())
            .limit(CONFIRM_BATCH_SIZE)
            .all(&self.db)
            .await
        {
            Ok(records) => records,
            Err(e) => {
                warn!(error = %e, "Failed to query Processing payouts");
                return;
            }
        };

        for po in &processing {
            let tx_hash = match &po.tx_hash {
                Some(h) => h.clone(),
                None => continue,
            };
            let po_id = &po.id;

            let net_enum = Network::from_str_lenient(&po.network);
            let executor = net_enum.as_ref().and_then(|n| self.executors.get(n));

            let executor = match executor {
                Some(e) => e,
                None => {
                    warn!(payout_id = %po_id, network = %po.network, "No executor for payout confirmation");
                    continue;
                }
            };

            match executor.check_tx_status(&tx_hash, CONFIRM_BLOCKS).await {
                Ok(ChainTxState::Confirmed) => {
                    let outbound = match self
                        .outbound_store
                        .find_for_payout_tx(po_id, &tx_hash)
                        .await
                    {
                        Ok(Some(outbound)) => outbound,
                        Ok(None) => {
                            error!(payout_id = %po_id, tx_hash = %tx_hash, "Missing matching root outbound journal for confirmed payout");
                            continue;
                        }
                        Err(error) => {
                            error!(payout_id = %po_id, error = %error, "Failed to load payout outbound journal");
                            continue;
                        }
                    };
                    let webhook_ids = match self.complete_payout(po, &outbound.id, &tx_hash).await {
                        Ok(Some(ids)) => ids,
                        Ok(None) => continue,
                        Err(error) => {
                            error!(payout_id = %po_id, error = %error, "Failed to atomically confirm payout");
                            continue;
                        }
                    };

                    if !webhook_ids.is_empty() {
                        self.webhook_service.trigger_delivery(&webhook_ids).await;
                    }

                    crate::services::metrics::inc_payout_confirmed(
                        &po.network,
                        "confirmed",
                        "payout",
                    );
                    info!(payout_id = %po_id, tx_hash = %tx_hash, "Payout confirmed on-chain");

                    // Upsert trusted address (post-confirmation)
                    self.upsert_trusted_address(&po.merchant_id, &po.network, &po.to_address)
                        .await;
                }
                Ok(ChainTxState::Failed) => {
                    let outbound = match self
                        .outbound_store
                        .find_for_payout_tx(po_id, &tx_hash)
                        .await
                    {
                        Ok(Some(outbound)) => outbound,
                        _ => continue,
                    };
                    crate::services::metrics::inc_payout_confirmed(&po.network, "failed", "payout");
                    warn!(payout_id = %po_id, tx_hash = %tx_hash, "Payout TX failed on-chain, refunding");
                    self.fail_and_refund_payout(
                        po,
                        "Transaction failed on-chain",
                        Some((outbound.id, outbound_transactions::OutboundState::Reverted)),
                    )
                    .await;
                }
                Ok(ChainTxState::Pending | ChainTxState::Unconfirmed) => {
                    if let Ok(Some(outbound)) = self
                        .outbound_store
                        .find_for_payout_tx(po_id, &tx_hash)
                        .await
                    {
                        let _ = self
                            .outbound_store
                            .mark_state(
                                &outbound.id,
                                outbound_transactions::OutboundState::Pending,
                                None,
                            )
                            .await;
                    }
                    debug!(payout_id = %po_id, "Payout TX still pending/unconfirmed");
                }
                Ok(ChainTxState::NotFound) => {
                    self.recover_payout_broadcast(po, executor.as_ref()).await;
                }
                Err(e) => {
                    warn!(payout_id = %po_id, error = %e, "Error checking payout TX status");
                }
            }
        }

        // Orphan recovery: Processing + no tx_hash for > 5 min = crash between CAS and broadcast.
        let orphan_cutoff = chrono::Utc::now() - chrono::Duration::seconds(300);
        let orphan_result = self
            .db
            .execute(sea_orm::Statement::from_sql_and_values(
                sea_orm::DbBackend::Postgres,
                r#"
            WITH stale AS (
                SELECT p.id
                FROM payouts p
                WHERE p.status = 'Processing'
                  AND p.tx_hash IS NULL
                  AND p.updated_at < $1
                  AND NOT EXISTS (
                      SELECT 1 FROM outbound_transactions ot
                      WHERE ot.payout_id = p.id
                        AND ot.purpose = 'token_transfer'
                        AND ot.parent_transaction_id IS NULL
                        AND ot.state IN ('Signed', 'BroadcastUnknown', 'Pending')
                  )
            ), failed_journals AS (
                UPDATE outbound_transactions ot
                SET state = 'Failed',
                    error_message = 'Auto-reset: interrupted before transaction signing',
                    updated_at = NOW()
                FROM stale
                WHERE ot.payout_id = stale.id
                  AND ot.purpose = 'token_transfer'
                  AND ot.parent_transaction_id IS NULL
                  AND ot.state = 'Preparing'
                RETURNING ot.id
            )
            UPDATE payouts p
            SET status = 'Pending',
                error_reason = 'Auto-reset: interrupted before transaction signing',
                updated_at = NOW()
            FROM stale
            WHERE p.id = stale.id
            "#,
                [orphan_cutoff.into()],
            ))
            .await;

        if let Ok(result) = orphan_result {
            if result.rows_affected() > 0 {
                warn!(
                    count = result.rows_affected(),
                    "Reset orphaned Processing payouts (no tx_hash) back to Pending"
                );
            }
        }
    }

    async fn recover_payout_broadcast(
        &self,
        po: &crate::entity::payouts::Model,
        executor: &dyn PayoutExecutor,
    ) {
        let Some(tx_hash) = po.tx_hash.as_deref() else {
            return;
        };
        let outbound = match self
            .outbound_store
            .find_for_payout_tx(&po.id, tx_hash)
            .await
        {
            Ok(Some(outbound)) => outbound,
            Ok(None) => {
                warn!(payout_id = %po.id, "Missing outbound journal; refusing timeout-based refund");
                return;
            }
            Err(error) => {
                warn!(payout_id = %po.id, error = %error, "Failed to load outbound journal");
                return;
            }
        };
        let payload = match self.outbound_store.decrypt_payload(&outbound) {
            Ok(payload) => payload,
            Err(error) => {
                error!(outbound_id = %outbound.id, error = %error, "Cannot decrypt signed payout payload");
                return;
            }
        };

        match executor.recover_broadcast(&payload).await {
            Ok(crate::services::outbound::RecoveryDisposition::Pending) => {
                let _ = self
                    .outbound_store
                    .mark_broadcast(
                        &outbound.id,
                        crate::services::outbound::BroadcastDisposition::Accepted,
                        None,
                    )
                    .await;
            }
            Ok(crate::services::outbound::RecoveryDisposition::BroadcastUnknown(error)) => {
                let _ = self
                    .outbound_store
                    .mark_broadcast(
                        &outbound.id,
                        crate::services::outbound::BroadcastDisposition::Unknown,
                        Some(error),
                    )
                    .await;
            }
            Ok(
                disposition @ (crate::services::outbound::RecoveryDisposition::Expired
                | crate::services::outbound::RecoveryDisposition::Replaced),
            ) => {
                let state = match disposition {
                    crate::services::outbound::RecoveryDisposition::Expired => {
                        outbound_transactions::OutboundState::Expired
                    }
                    crate::services::outbound::RecoveryDisposition::Replaced => {
                        outbound_transactions::OutboundState::Replaced
                    }
                    _ => unreachable!(),
                };
                let reason = format!("Signed transaction proven unable to land: {state:?}");
                match self
                    .outbound_store
                    .stage_terminal_evidence(&outbound.id, state.clone(), &reason)
                    .await
                {
                    Ok(crate::services::outbound::TerminalEvidence::Ready) => {
                        self.fail_and_refund_payout(po, &reason, Some((outbound.id, state)))
                            .await;
                    }
                    Ok(crate::services::outbound::TerminalEvidence::Staged) => {
                        warn!(outbound_id = %outbound.id, "Staged terminal payout evidence; awaiting recheck");
                    }
                    Ok(crate::services::outbound::TerminalEvidence::Conflict) => {}
                    Err(error) => {
                        warn!(outbound_id = %outbound.id, error = %error, "Failed to stage terminal payout evidence");
                    }
                }
            }
            Err(error) => {
                warn!(outbound_id = %outbound.id, error = %error, "Payout broadcast recovery failed");
            }
        }
    }

    async fn complete_payout(
        &self,
        po: &crate::entity::payouts::Model,
        outbound_id: &str,
        tx_hash: &str,
    ) -> Result<Option<Vec<String>>> {
        use crate::entity::payouts;

        let txn = self.db.begin().await?;
        let parent = payouts::Entity::update_many()
            .col_expr(
                payouts::Column::Status,
                Expr::value(payouts::PayoutStatus::Completed),
            )
            .col_expr(
                payouts::Column::CompletedAt,
                Expr::value(Some(chrono::Utc::now())),
            )
            .col_expr(payouts::Column::UpdatedAt, Expr::value(chrono::Utc::now()))
            .filter(payouts::Column::Id.eq(&po.id))
            .filter(payouts::Column::Status.eq(payouts::PayoutStatus::Processing))
            .exec(&txn)
            .await?;
        if parent.rows_affected != 1 {
            txn.rollback().await?;
            return Ok(None);
        }

        if !self
            .outbound_store
            .mark_state_on(
                &txn,
                outbound_id,
                outbound_transactions::OutboundState::Confirmed,
                None,
            )
            .await?
        {
            txn.rollback().await?;
            return Ok(None);
        }

        let network = Network::from_str_lenient(&po.network)
            .ok_or_else(|| anyhow!("Invalid payout network {}", po.network))?;
        let payload = Self::build_webhook_payload(
            po,
            "Completed",
            Some(chrono::Utc::now().timestamp()),
            None,
            Some(tx_hash.to_string()),
        );
        let webhook_ids = self
            .webhook_service
            .queue_event_with_txn(
                &txn,
                &po.id,
                &po.merchant_id,
                network,
                po.environment,
                Self::completed_event_type(&po.id),
                &payload,
            )
            .await?;

        txn.commit().await?;
        Ok(Some(webhook_ids))
    }

    /// Mark payout as Failed and refund the full amount back to merchant balance.
    async fn fail_and_refund_payout(
        &self,
        po: &crate::entity::payouts::Model,
        reason: &str,
        terminal: Option<(String, outbound_transactions::OutboundState)>,
    ) {
        use crate::entity::payouts;

        let po_id = po.id.clone();
        let merchant_id = po.merchant_id.clone();
        let gross_amount = po.amount;
        let reason_owned = reason.to_string();
        let po_network = po.network.clone();
        let env = po.environment;

        let network_enum = match crate::entity::Network::from_str_lenient(&po_network) {
            Some(n) => n,
            None => {
                error!(payout_id = %po_id, network = %po_network, "Cannot parse network for payout refund; refusing to credit another chain balance");
                self.alerting_service.send_alert(
                    "payout_refund_failed",
                    AlertLevel::Critical,
                    &format!(
                        "Payout {} has invalid network {}; refund requires manual intervention.",
                        po_id, po_network
                    ),
                );
                return;
            }
        };

        // Use explicit begin/commit for Outbox pattern (webhook queue inside same txn)
        let txn = match self.db.begin().await {
            Ok(t) => t,
            Err(e) => {
                error!(payout_id = %po_id, error = %e, "CRITICAL: Cannot begin refund txn");
                self.alerting_service.send_alert(
                    "payout_refund_failed",
                    AlertLevel::Critical,
                    &format!("Payout {} refund txn begin failed: {}", po_id, e),
                );
                return;
            }
        };

        // Claim the business order before any refund side effect.
        let parent = payouts::Entity::update_many()
            .col_expr(
                payouts::Column::Status,
                Expr::value(payouts::PayoutStatus::Failed),
            )
            .col_expr(
                payouts::Column::ErrorReason,
                Expr::value(Some(reason_owned.clone())),
            )
            .col_expr(payouts::Column::UpdatedAt, Expr::value(chrono::Utc::now()))
            .filter(payouts::Column::Id.eq(&po_id))
            .filter(payouts::Column::Status.eq(payouts::PayoutStatus::Processing))
            .exec(&txn)
            .await;
        if !matches!(parent, Ok(ref result) if result.rows_affected == 1) {
            let _ = txn.rollback().await;
            debug!(payout_id = %po_id, "Payout refund skipped after state conflict");
            return;
        }

        if let Some((outbound_id, state)) = terminal {
            match self
                .outbound_store
                .mark_state_on(&txn, &outbound_id, state, Some(reason_owned.clone()))
                .await
            {
                Ok(true) => {}
                Ok(false) => {
                    let _ = txn.rollback().await;
                    debug!(payout_id = %po_id, outbound_id = %outbound_id, "Payout terminal transition lost CAS race");
                    return;
                }
                Err(error) => {
                    let _ = txn.rollback().await;
                    error!(payout_id = %po_id, error = %error, "Failed to update payout outbound journal");
                    return;
                }
            }
        }

        // Refund full amount (gross) back to chain account balance.
        if let Err(e) = self
            .billing_service
            .refund_cost(
                &txn,
                &merchant_id,
                None,
                gross_amount,
                Some(format!("po_refund_{}", po_id)),
                Some(format!("Payout {} failed: {}", po_id, reason_owned)),
                network_enum.clone(),
                env.clone(),
                po.currency.as_str(),
            )
            .await
        {
            error!(payout_id = %po_id, error = %e, "CRITICAL: Failed to refund payout");
            self.alerting_service.send_alert(
                "payout_refund_failed",
                AlertLevel::Critical,
                &format!(
                    "Payout {} refund failed: {}. Manual intervention required.",
                    po_id, e
                ),
            );
            let _ = txn.rollback().await;
            return;
        }

        // Queue webhook in the same transaction.
        let payload = Self::build_webhook_payload(
            po,
            "Failed",
            None,
            Some(reason_owned.clone()),
            po.tx_hash.clone(),
        );
        let event_type = Self::failed_event_type(&po_id);
        let webhook_ids = match self
            .webhook_service
            .queue_event_with_txn(
                &txn,
                &po_id,
                &merchant_id,
                network_enum,
                env,
                event_type,
                &payload,
            )
            .await
        {
            Ok(ids) => ids,
            Err(error) => {
                let _ = txn.rollback().await;
                error!(payout_id = %po_id, error = %error, "Failed to queue payout failure outbox event; refund transaction rolled back");
                self.alerting_service.send_alert(
                    "payout_refund_failed",
                    AlertLevel::Critical,
                    &format!(
                        "Payout {} refund rolled back because its webhook outbox event could not be queued: {}",
                        po_id, error
                    ),
                );
                return;
            }
        };

        // Commit journal, order, balance and outbox together.
        if let Err(e) = txn.commit().await {
            error!(payout_id = %po_id, error = %e, "CRITICAL: Failed to commit payout refund txn");
            self.alerting_service.send_alert(
                "payout_refund_failed",
                AlertLevel::Critical,
                &format!(
                    "Payout {} refund commit failed: {}. Manual intervention required.",
                    po_id, e
                ),
            );
            return;
        }

        // 5. Post-commit: trigger webhook delivery
        if !webhook_ids.is_empty() {
            self.webhook_service.trigger_delivery(&webhook_ids).await;
        }
    }

    // =========================================================================
    // Risk Control: Approval Flow
    // =========================================================================

    /// Fetch payout settings for a merchant (lazy: returns defaults if no row exists).
    pub async fn get_payout_settings(
        &self,
        merchant_id: &str,
    ) -> crate::entity::payout_settings::Model {
        use crate::entity::payout_settings;

        match payout_settings::Entity::find()
            .filter(payout_settings::Column::MerchantId.eq(merchant_id))
            .one(&self.db)
            .await
        {
            Ok(Some(settings)) => settings,
            _ => {
                let mut defaults = payout_settings::Model::default();
                defaults.merchant_id = merchant_id.to_string();
                defaults
            }
        }
    }

    /// Update (upsert) payout settings for a merchant.
    pub async fn update_payout_settings(
        &self,
        merchant_id: &str,
        require_new_address_approval: Option<bool>,
        approval_threshold: Option<i64>,
        approver_roles: Option<serde_json::Value>,
        auto_withdraw_enabled: Option<bool>,
        auto_withdraw_threshold: Option<i64>,
        auto_withdraw_network: Option<String>,
        auto_withdraw_currency: Option<String>,
    ) -> Result<crate::entity::payout_settings::Model> {
        use crate::entity::payout_settings;

        let existing = payout_settings::Entity::find()
            .filter(payout_settings::Column::MerchantId.eq(merchant_id))
            .one(&self.db)
            .await?;

        let saved = if let Some(row) = existing {
            // Update existing
            let mut am: payout_settings::ActiveModel = row.into();
            if let Some(v) = require_new_address_approval {
                am.require_new_address_approval = Set(v);
            }
            if let Some(v) = approval_threshold {
                am.approval_threshold = Set(v);
            }
            if let Some(v) = approver_roles {
                am.approver_roles = Set(v);
            }
            if let Some(v) = auto_withdraw_enabled {
                am.auto_withdraw_enabled = Set(v);
            }
            if auto_withdraw_threshold.is_some() {
                am.auto_withdraw_threshold = Set(auto_withdraw_threshold);
            }
            if auto_withdraw_network.is_some() {
                am.auto_withdraw_network = Set(auto_withdraw_network);
            }
            if let Some(v) = auto_withdraw_currency {
                am.auto_withdraw_currency = Set(v);
            }
            am.updated_at = Set(chrono::Utc::now().into());
            am.update(&self.db).await?
        } else {
            // Insert new with defaults + overrides
            let am = payout_settings::ActiveModel {
                id: Set(format!("ps_{}", uuid::Uuid::new_v4().simple())),
                merchant_id: Set(merchant_id.to_string()),
                require_new_address_approval: Set(require_new_address_approval.unwrap_or(true)),
                approval_threshold: Set(approval_threshold.unwrap_or(5_000_000_000)),
                approver_roles: Set(
                    approver_roles.unwrap_or_else(|| serde_json::json!(["owner", "admin"]))
                ),
                auto_withdraw_enabled: Set(auto_withdraw_enabled.unwrap_or(false)),
                auto_withdraw_threshold: Set(auto_withdraw_threshold),
                auto_withdraw_network: Set(auto_withdraw_network),
                auto_withdraw_currency: Set(
                    auto_withdraw_currency.unwrap_or_else(|| "USDT".to_string())
                ),
                created_at: Set(chrono::Utc::now().into()),
                updated_at: Set(chrono::Utc::now().into()),
            };
            am.insert(&self.db).await?
        };

        info!(merchant_id = %merchant_id, "Payout settings updated");
        Ok(saved)
    }

    /// Determine whether a withdrawal should be PendingApproval or Pending.
    ///
    /// Rules:
    /// - If requested_by is Owner → Pending (Owner bypass)
    /// - If new address && require_new_address_approval → PendingApproval
    /// - If amount > approval_threshold (and threshold > 0) → PendingApproval
    async fn determine_withdrawal_status(
        &self,
        merchant_id: &str,
        network: &crate::entity::Network,
        to_address: &str,
        amount: i64,
        requested_by: Option<&str>,
    ) -> WithdrawalStatus {
        let settings = self.get_payout_settings(merchant_id).await;

        // Owner bypass: check if the requesting user is Owner
        if let Some(user_id) = requested_by {
            if self.is_owner(merchant_id, user_id).await {
                return WithdrawalStatus::Pending;
            }
        }

        if self
            .should_require_approval(&settings, merchant_id, network, to_address, amount)
            .await
        {
            WithdrawalStatus::PendingApproval
        } else {
            WithdrawalStatus::Pending
        }
    }

    /// Determine whether a payout (API) should be PendingApproval or Pending.
    ///
    /// No Owner bypass for API payouts (API Key has no user identity).
    async fn determine_payout_status(
        &self,
        merchant_id: &str,
        network: &crate::entity::Network,
        to_address: &str,
        amount: i64,
    ) -> crate::entity::payouts::PayoutStatus {
        use crate::entity::payouts::PayoutStatus;

        let settings = self.get_payout_settings(merchant_id).await;

        if self
            .should_require_approval(&settings, merchant_id, network, to_address, amount)
            .await
        {
            PayoutStatus::PendingApproval
        } else {
            PayoutStatus::Pending
        }
    }

    /// Core risk rule evaluation. Returns true if approval is required.
    async fn should_require_approval(
        &self,
        settings: &crate::entity::payout_settings::Model,
        merchant_id: &str,
        network: &crate::entity::Network,
        to_address: &str,
        amount: i64,
    ) -> bool {
        // Rule 1: New address check
        if settings.require_new_address_approval {
            let normalized = Self::normalize_address(network, to_address);
            let is_trusted = self
                .is_trusted_address(merchant_id, network.as_str(), &normalized)
                .await;
            if !is_trusted {
                info!(
                    merchant_id = %merchant_id,
                    address = %to_address,
                    "Risk control: new address requires approval"
                );
                return true;
            }
        }

        // Rule 2: Amount threshold (-1 = disabled, 0 = approve all, >0 = exceeding threshold)
        if settings.approval_threshold >= 0 && amount >= settings.approval_threshold {
            info!(
                merchant_id = %merchant_id,
                amount = amount,
                threshold = settings.approval_threshold,
                "Risk control: amount exceeds approval threshold"
            );
            return true;
        }

        false
    }

    /// Check if an address is in the trusted list for a merchant+network.
    async fn is_trusted_address(
        &self,
        merchant_id: &str,
        network: &str,
        normalized_address: &str,
    ) -> bool {
        use crate::entity::payout_trusted_addresses;

        payout_trusted_addresses::Entity::find()
            .filter(payout_trusted_addresses::Column::MerchantId.eq(merchant_id))
            .filter(payout_trusted_addresses::Column::Network.eq(network))
            .filter(payout_trusted_addresses::Column::Address.eq(normalized_address))
            .one(&self.db)
            .await
            .ok()
            .flatten()
            .is_some()
    }

    /// Normalize address for trusted-address storage.
    /// EVM: lowercase. TRON: as-is (Base58Check is case-sensitive).
    fn normalize_address(network: &crate::entity::Network, address: &str) -> String {
        match network.chain_family() {
            crate::entity::ChainFamily::Evm => address.to_lowercase(),
            crate::entity::ChainFamily::Tron => address.to_string(),
            // Solana uses Base58Check which is case-sensitive (like TRON)
            crate::entity::ChainFamily::Solana => address.to_string(),
        }
    }

    /// Check if a user is the Owner of a merchant/organization.
    async fn is_owner(&self, merchant_id: &str, user_id: &str) -> bool {
        use crate::entity::org_members;

        org_members::Entity::find()
            .filter(org_members::Column::OrgId.eq(merchant_id))
            .filter(org_members::Column::UserId.eq(Some(user_id.to_string())))
            .filter(org_members::Column::Role.eq(org_members::MemberRole::Owner))
            .filter(org_members::Column::Status.eq(org_members::MemberStatus::Active))
            .one(&self.db)
            .await
            .ok()
            .flatten()
            .is_some()
    }

    /// Upsert a trusted address after successful on-chain confirmation.
    pub async fn upsert_trusted_address(&self, merchant_id: &str, network: &str, address: &str) {
        let normalized = match crate::entity::Network::from_str_lenient(network) {
            Some(net) => Self::normalize_address(&net, address),
            None => address.to_string(),
        };

        let result = self
            .db
            .execute(sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Postgres,
                r#"
                INSERT INTO payout_trusted_addresses (id, merchant_id, network, address, first_used_at, last_used_at, total_payouts)
                VALUES ($1, $2, $3, $4, NOW(), NOW(), 1)
                ON CONFLICT (merchant_id, network, address)
                DO UPDATE SET last_used_at = NOW(), total_payouts = payout_trusted_addresses.total_payouts + 1
                "#,
                [
                    format!("pta_{}", uuid::Uuid::new_v4().simple()).into(),
                    merchant_id.to_string().into(),
                    network.to_string().into(),
                    normalized.into(),
                ],
            ))
            .await;

        if let Err(e) = result {
            warn!(
                merchant_id = %merchant_id,
                network = %network,
                address = %address,
                error = %e,
                "Failed to upsert trusted address (non-fatal)"
            );
        }
    }

    // =========================================================================
    // Approve / Reject endpoints
    // =========================================================================

    /// Approve a payout: PendingApproval → Pending.
    ///
    /// CAS ensures only one approval succeeds in case of concurrent requests.
    pub async fn approve_payout(
        &self,
        payout_id: &str,
        merchant_id: &str,
        approver_user_id: &str,
    ) -> Result<(), PayoutError> {
        use crate::entity::payouts;

        let result = payouts::Entity::update_many()
            .col_expr(
                payouts::Column::Status,
                Expr::value(payouts::PayoutStatus::Pending),
            )
            .col_expr(
                payouts::Column::ReviewedBy,
                Expr::value(Some(approver_user_id.to_string())),
            )
            .col_expr(
                payouts::Column::ReviewedAt,
                Expr::value(Some(chrono::Utc::now())),
            )
            .col_expr(payouts::Column::UpdatedAt, Expr::value(chrono::Utc::now()))
            .filter(payouts::Column::Id.eq(payout_id))
            .filter(payouts::Column::MerchantId.eq(merchant_id))
            .filter(payouts::Column::Status.eq(payouts::PayoutStatus::PendingApproval))
            .exec(&self.db)
            .await
            .map_err(PayoutError::Database)?;

        if result.rows_affected == 0 {
            return Err(PayoutError::InvalidAmount(
                "Payout not found or not in PendingApproval status".into(),
            ));
        }

        info!(
            payout_id = %payout_id,
            approved_by = %approver_user_id,
            "Payout approved, now Pending for broadcast"
        );
        Ok(())
    }

    /// Reject a payout: PendingApproval|Pending → Cancelled + refund.
    pub async fn reject_payout(
        &self,
        payout_id: &str,
        merchant_id: &str,
        rejector_user_id: &str,
        reason: Option<&str>,
    ) -> Result<(), PayoutError> {
        use crate::entity::payouts;

        // Fetch the payout first for refund data — allow reject from PendingApproval OR Pending
        let po = payouts::Entity::find_by_id(payout_id)
            .filter(payouts::Column::MerchantId.eq(merchant_id))
            .filter(payouts::Column::Status.is_in([
                payouts::PayoutStatus::PendingApproval,
                payouts::PayoutStatus::Pending,
            ]))
            .one(&self.db)
            .await
            .map_err(PayoutError::Database)?
            .ok_or_else(|| {
                PayoutError::InvalidAmount(
                    "Payout not found or not in a rejectable status (PendingApproval or Pending)"
                        .into(),
                )
            })?;

        // Atomic: reject + refund
        self.reject_and_refund_payout(&po, rejector_user_id, reason)
            .await
    }

    /// Approve a withdrawal: PendingApproval → Pending.
    pub async fn approve_withdrawal(
        &self,
        withdrawal_id: &str,
        merchant_id: &str,
        approver_user_id: &str,
    ) -> Result<(), PayoutError> {
        let result = withdrawals::Entity::update_many()
            .col_expr(
                withdrawals::Column::Status,
                Expr::value(WithdrawalStatus::Pending),
            )
            .col_expr(
                withdrawals::Column::ReviewedBy,
                Expr::value(Some(approver_user_id.to_string())),
            )
            .col_expr(
                withdrawals::Column::ReviewedAt,
                Expr::value(Some(chrono::Utc::now())),
            )
            .filter(withdrawals::Column::Id.eq(withdrawal_id))
            .filter(withdrawals::Column::MerchantId.eq(merchant_id))
            .filter(withdrawals::Column::Status.eq(WithdrawalStatus::PendingApproval))
            .exec(&self.db)
            .await
            .map_err(PayoutError::Database)?;

        if result.rows_affected == 0 {
            return Err(PayoutError::InvalidAmount(
                "Withdrawal not found or not in PendingApproval status".into(),
            ));
        }

        info!(
            withdrawal_id = %withdrawal_id,
            approved_by = %approver_user_id,
            "Withdrawal approved, now Pending for broadcast"
        );
        Ok(())
    }

    /// Reject a withdrawal: PendingApproval|Pending → Cancelled + refund.
    pub async fn reject_withdrawal(
        &self,
        withdrawal_id: &str,
        merchant_id: &str,
        rejector_user_id: &str,
        reason: Option<&str>,
    ) -> Result<(), PayoutError> {
        // Fetch the withdrawal for refund data — allow reject from PendingApproval OR Pending
        let wd = withdrawals::Entity::find_by_id(withdrawal_id)
            .filter(withdrawals::Column::MerchantId.eq(merchant_id))
            .filter(
                withdrawals::Column::Status.is_in([
                    WithdrawalStatus::PendingApproval,
                    WithdrawalStatus::Pending,
                ]),
            )
            .one(&self.db)
            .await
            .map_err(PayoutError::Database)?
            .ok_or_else(|| {
                PayoutError::InvalidAmount(
                    "Withdrawal not found or not in a rejectable status (PendingApproval or Pending)".into(),
                )
            })?;

        // Atomic: reject + refund
        self.reject_and_refund_withdrawal(&wd, rejector_user_id, reason)
            .await
    }

    /// Reject a withdrawal and refund balance atomically.
    async fn reject_and_refund_withdrawal(
        &self,
        wd: &withdrawals::Model,
        rejector_user_id: &str,
        reason: Option<&str>,
    ) -> Result<(), PayoutError> {
        let env = wd.environment;
        let wd_id = wd.id.clone();
        let merchant_id = wd.merchant_id.clone();
        let gross_amount = wd.amount;
        let wd_network = wd.network.clone();
        let rejector = rejector_user_id.to_string();
        let wd_currency = wd.currency.clone();
        let error_reason = reason
            .map(|r| format!("Rejected: {}", r))
            .unwrap_or_else(|| "Rejected by approver".to_string());

        let network_enum = crate::entity::Network::from_str_lenient(&wd_network)
            .unwrap_or(crate::entity::Network::Tron);

        let billing_service = self.billing_service.clone();

        self.db
            .transaction::<_, (), sea_orm::DbErr>(|txn| {
                let wd_id = wd_id.clone();
                let merchant_id = merchant_id.clone();
                let rejector = rejector.clone();
                let billing_service = billing_service.clone();
                let env = env;
                let network = network_enum.clone();
                let currency = wd_currency.clone();
                let error_reason = error_reason.clone();

                Box::pin(async move {
                    // 1. Refund balance
                    billing_service
                        .refund_cost(
                            txn,
                            &merchant_id,
                            None,
                            gross_amount,
                            Some(format!("wd_cancel_{}", wd_id)),
                            Some(format!("Withdrawal {} cancelled", wd_id)),
                            network,
                            env,
                            &currency,
                        )
                        .await
                        .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

                    // 2. CAS: PendingApproval|Pending → Cancelled
                    let res =
                        withdrawals::Entity::update_many()
                            .col_expr(
                                withdrawals::Column::Status,
                                Expr::value(WithdrawalStatus::Cancelled),
                            )
                            .col_expr(withdrawals::Column::ReviewedBy, Expr::value(Some(rejector)))
                            .col_expr(
                                withdrawals::Column::ReviewedAt,
                                Expr::value(Some(chrono::Utc::now())),
                            )
                            .col_expr(
                                withdrawals::Column::ErrorReason,
                                Expr::value(Some(error_reason)),
                            )
                            .filter(withdrawals::Column::Id.eq(&wd_id))
                            .filter(withdrawals::Column::Status.is_in([
                                WithdrawalStatus::PendingApproval,
                                WithdrawalStatus::Pending,
                            ]))
                            .exec(txn)
                            .await?;

                    if res.rows_affected == 0 {
                        return Err(sea_orm::DbErr::Custom("ALREADY_PROCESSED".to_string()));
                    }

                    Ok(())
                })
            })
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("ALREADY_PROCESSED") {
                    return PayoutError::InvalidAmount(
                        "Withdrawal already processed or not in a rejectable status".into(),
                    );
                }
                match e {
                    sea_orm::TransactionError::Transaction(db_err) => PayoutError::Database(db_err),
                    sea_orm::TransactionError::Connection(db_err) => PayoutError::Database(db_err),
                }
            })?;

        info!(
            withdrawal_id = %wd.id,
            rejected_by = %rejector_user_id,
            "Withdrawal rejected and refunded"
        );
        Ok(())
    }

    /// Reject a payout and refund balance atomically.
    async fn reject_and_refund_payout(
        &self,
        po: &crate::entity::payouts::Model,
        rejector_user_id: &str,
        reason: Option<&str>,
    ) -> Result<(), PayoutError> {
        use crate::entity::payouts;

        let env = po.environment.clone();
        let po_id = po.id.clone();
        let merchant_id = po.merchant_id.clone();
        let gross_amount = po.amount;
        let po_network = po.network.clone();
        let rejector = rejector_user_id.to_string();
        let po_currency = po.currency.clone();
        let error_reason = reason
            .map(|r| format!("Rejected: {}", r))
            .unwrap_or_else(|| "Rejected by approver".to_string());

        let network_enum = crate::entity::Network::from_str_lenient(&po_network)
            .unwrap_or(crate::entity::Network::Tron);

        let billing_service = self.billing_service.clone();

        self.db
            .transaction::<_, (), sea_orm::DbErr>(|txn| {
                let po_id = po_id.clone();
                let merchant_id = merchant_id.clone();
                let rejector = rejector.clone();
                let billing_service = billing_service.clone();
                let env = env.clone();
                let network = network_enum.clone();
                let currency = po_currency.clone();
                let error_reason = error_reason.clone();

                Box::pin(async move {
                    // 1. Refund balance
                    billing_service
                        .refund_cost(
                            txn,
                            &merchant_id,
                            None,
                            gross_amount,
                            Some(format!("po_cancel_{}", po_id)),
                            Some(format!("Payout {} cancelled", po_id)),
                            network,
                            env,
                            &currency,
                        )
                        .await
                        .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

                    // 2. CAS: PendingApproval|Pending → Cancelled
                    let res = payouts::Entity::update_many()
                        .col_expr(
                            payouts::Column::Status,
                            Expr::value(payouts::PayoutStatus::Cancelled),
                        )
                        .col_expr(payouts::Column::ReviewedBy, Expr::value(Some(rejector)))
                        .col_expr(
                            payouts::Column::ReviewedAt,
                            Expr::value(Some(chrono::Utc::now())),
                        )
                        .col_expr(
                            payouts::Column::ErrorReason,
                            Expr::value(Some(error_reason)),
                        )
                        .col_expr(payouts::Column::UpdatedAt, Expr::value(chrono::Utc::now()))
                        .filter(payouts::Column::Id.eq(&po_id))
                        .filter(payouts::Column::Status.is_in([
                            payouts::PayoutStatus::PendingApproval,
                            payouts::PayoutStatus::Pending,
                        ]))
                        .exec(txn)
                        .await?;

                    if res.rows_affected == 0 {
                        return Err(sea_orm::DbErr::Custom("ALREADY_PROCESSED".to_string()));
                    }

                    Ok(())
                })
            })
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("ALREADY_PROCESSED") {
                    return PayoutError::InvalidAmount(
                        "Payout already processed or not in a rejectable status".into(),
                    );
                }
                match e {
                    sea_orm::TransactionError::Transaction(db_err) => PayoutError::Database(db_err),
                    sea_orm::TransactionError::Connection(db_err) => PayoutError::Database(db_err),
                }
            })?;

        info!(
            payout_id = %po.id,
            rejected_by = %rejector_user_id,
            "Payout rejected and refunded"
        );
        Ok(())
    }

    // =========================================================================
    // Auto-expire stale PendingApproval records (24h timeout)
    // =========================================================================

    /// Scan PendingApproval withdrawals and payouts older than 24h,
    /// mark as ApprovalExpired and refund balance.
    async fn auto_expire_stale_approvals(&self) {
        use crate::entity::payouts;

        let cutoff = chrono::Utc::now() - chrono::Duration::hours(24);

        // 1. Expire stale withdrawals
        let stale_withdrawals = match withdrawals::Entity::find()
            .filter(withdrawals::Column::Status.eq(WithdrawalStatus::PendingApproval))
            .filter(withdrawals::Column::CreatedAt.lt(cutoff))
            .all(&self.db)
            .await
        {
            Ok(records) => records,
            Err(e) => {
                warn!(error = %e, "Failed to query stale PendingApproval withdrawals");
                vec![]
            }
        };

        for wd in &stale_withdrawals {
            self.expire_and_refund_withdrawal(wd).await;
        }

        // 2. Expire stale payouts
        let stale_payouts = match payouts::Entity::find()
            .filter(payouts::Column::Status.eq(payouts::PayoutStatus::PendingApproval))
            .filter(payouts::Column::CreatedAt.lt(cutoff))
            .all(&self.db)
            .await
        {
            Ok(records) => records,
            Err(e) => {
                warn!(error = %e, "Failed to query stale PendingApproval payouts");
                vec![]
            }
        };

        for po in &stale_payouts {
            self.expire_and_refund_payout(po).await;
        }
    }

    /// Mark a withdrawal as ApprovalExpired and refund balance.
    async fn expire_and_refund_withdrawal(&self, wd: &withdrawals::Model) {
        let env = wd.environment;
        let wd_id = wd.id.clone();
        let merchant_id = wd.merchant_id.clone();
        let gross_amount = wd.amount;
        let wd_network = wd.network.clone();
        let wd_currency = wd.currency.clone();

        let network_enum = crate::entity::Network::from_str_lenient(&wd_network)
            .unwrap_or(crate::entity::Network::Tron);

        let billing_service = self.billing_service.clone();

        let result = self
            .db
            .transaction::<_, (), sea_orm::DbErr>(|txn| {
                let wd_id = wd_id.clone();
                let merchant_id = merchant_id.clone();
                let billing_service = billing_service.clone();
                let env = env;
                let network = network_enum.clone();
                let currency = wd_currency.clone();

                Box::pin(async move {
                    billing_service
                        .refund_cost(
                            txn,
                            &merchant_id,
                            None,
                            gross_amount,
                            Some(format!("wd_expired_{}", wd_id)),
                            Some(format!(
                                "Withdrawal {} approval expired (24h timeout)",
                                wd_id
                            )),
                            network,
                            env,
                            &currency,
                        )
                        .await
                        .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

                    withdrawals::Entity::update_many()
                        .col_expr(
                            withdrawals::Column::Status,
                            Expr::value(WithdrawalStatus::ApprovalExpired),
                        )
                        .col_expr(
                            withdrawals::Column::ErrorReason,
                            Expr::value(Some("Approval expired after 24 hours")),
                        )
                        .filter(withdrawals::Column::Id.eq(&wd_id))
                        .filter(withdrawals::Column::Status.eq(WithdrawalStatus::PendingApproval))
                        .exec(txn)
                        .await?;

                    Ok(())
                })
            })
            .await;

        match result {
            Ok(()) => {
                warn!(
                    withdrawal_id = %wd.id,
                    "Withdrawal PendingApproval expired after 24h, refunded"
                );
            }
            Err(e) => {
                error!(
                    withdrawal_id = %wd.id,
                    error = %e,
                    "CRITICAL: Failed to expire/refund PendingApproval withdrawal"
                );
                self.alerting_service.send_alert(
                    "approval_expire_refund_failed",
                    AlertLevel::Critical,
                    &format!(
                        "Withdrawal {} expire+refund failed: {}. Manual intervention required.",
                        wd.id, e
                    ),
                );
            }
        }
    }

    /// Mark a payout as ApprovalExpired and refund balance.
    async fn expire_and_refund_payout(&self, po: &crate::entity::payouts::Model) {
        use crate::entity::payouts;

        let env = po.environment.clone();
        let po_id = po.id.clone();
        let merchant_id = po.merchant_id.clone();
        let gross_amount = po.amount;
        let po_network = po.network.clone();
        let po_currency = po.currency.clone();

        let network_enum = crate::entity::Network::from_str_lenient(&po_network)
            .unwrap_or(crate::entity::Network::Tron);

        let billing_service = self.billing_service.clone();

        let result = self
            .db
            .transaction::<_, (), sea_orm::DbErr>(|txn| {
                let po_id = po_id.clone();
                let merchant_id = merchant_id.clone();
                let billing_service = billing_service.clone();
                let env = env.clone();
                let network = network_enum.clone();
                let currency = po_currency.clone();

                Box::pin(async move {
                    billing_service
                        .refund_cost(
                            txn,
                            &merchant_id,
                            None,
                            gross_amount,
                            Some(format!("po_expired_{}", po_id)),
                            Some(format!("Payout {} approval expired (24h timeout)", po_id)),
                            network,
                            env,
                            &currency,
                        )
                        .await
                        .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

                    payouts::Entity::update_many()
                        .col_expr(
                            payouts::Column::Status,
                            Expr::value(payouts::PayoutStatus::ApprovalExpired),
                        )
                        .col_expr(
                            payouts::Column::ErrorReason,
                            Expr::value(Some("Approval expired after 24 hours")),
                        )
                        .col_expr(payouts::Column::UpdatedAt, Expr::value(chrono::Utc::now()))
                        .filter(payouts::Column::Id.eq(&po_id))
                        .filter(payouts::Column::Status.eq(payouts::PayoutStatus::PendingApproval))
                        .exec(txn)
                        .await?;

                    Ok(())
                })
            })
            .await;

        match result {
            Ok(()) => {
                warn!(
                    payout_id = %po.id,
                    "Payout PendingApproval expired after 24h, refunded"
                );
            }
            Err(e) => {
                error!(
                    payout_id = %po.id,
                    error = %e,
                    "CRITICAL: Failed to expire/refund PendingApproval payout"
                );
                self.alerting_service.send_alert(
                    "approval_expire_refund_failed",
                    AlertLevel::Critical,
                    &format!(
                        "Payout {} expire+refund failed: {}. Manual intervention required.",
                        po.id, e
                    ),
                );
            }
        }
    }
}
