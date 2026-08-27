use super::ResolutionError;
use crate::api::dtos::resolution::{
    get_available_actions, ResolutionAction, ResolutionStatsResponse, DUST_THRESHOLD, USDT_DECIMALS,
};
use crate::entity::payment_exceptions;
use crate::entity::payment_exceptions::{ExceptionStatus, Resolution};
use crate::entity::{
    checkout_sessions, merchants, outbound_transactions, transactions, webhook_events, Network,
};
use crate::services::alerting::{AlertLevel, AlertingService};
use crate::services::checkout::{SessionEventPayload, TransactionInfo, WebhookPricingInfo};
use crate::services::payout::PayoutExecutor;
use crate::services::webhook::WebhookService;
use anyhow::{anyhow, Result};
use rust_decimal::Decimal;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use std::collections::HashMap;
use std::sync::Arc;

use tracing::{debug, error, info, warn};

/// Validates that an action is allowed for the given exception based on its type and status.
/// Uses the same logic as get_available_actions to ensure frontend/backend consistency.
fn validate_action(
    ex: &payment_exceptions::Model,
    action: ResolutionAction,
) -> Result<(), ResolutionError> {
    let allowed = get_available_actions(&ex.exception_type, &ex.status);
    if !allowed.contains(&action.as_str().to_string()) {
        return Err(ResolutionError::ActionNotAllowed {
            action: action.as_str().to_string(),
            reason: format!(
                "Not allowed for exception type {:?} in status {:?}. Allowed actions: {:?}",
                ex.exception_type, ex.status, allowed
            ),
        });
    }
    Ok(())
}
// Import SweeperService
use crate::services::billing::fee_config::FeeConfig;
use crate::services::billing::BillingService;

pub struct ResolutionService {
    db: DatabaseConnection,
    merchant_service: Arc<crate::services::merchant::MerchantService>,
    billing_service: Arc<BillingService>,
    fee_config: Arc<FeeConfig>,
    alerting_service: Arc<AlertingService>,
    webhook_service: Arc<WebhookService>,
    /// Process-level environment (from DB isolation)
    environment: crate::entity::Environment,
    /// Chain-specific payout executors for refund transfers
    payout_executors: HashMap<Network, Arc<dyn PayoutExecutor>>,
    /// Per-chain deposit fee floor overrides from chains.toml.
    chain_deposit_floors: HashMap<Network, i64>,
    /// Optional Xero service for enqueueing sync after exception resolution.
    xero_service: Option<Arc<crate::services::xero::XeroService>>,
    outbound_store: Arc<crate::services::outbound::OutboundTransactionStore>,
}

impl ResolutionService {
    pub fn new(
        db: DatabaseConnection,
        merchant_service: Arc<crate::services::merchant::MerchantService>,
        billing_service: Arc<BillingService>,
        fee_config: Arc<FeeConfig>,
        alerting_service: Arc<AlertingService>,
        webhook_service: Arc<WebhookService>,
        xero_service: Option<Arc<crate::services::xero::XeroService>>,
        environment: crate::entity::Environment,
        payout_executors: HashMap<Network, Arc<dyn PayoutExecutor>>,
        chain_deposit_floors: HashMap<Network, i64>,
        outbound_store: Arc<crate::services::outbound::OutboundTransactionStore>,
    ) -> Self {
        Self {
            db,
            merchant_service,
            billing_service,
            fee_config,
            alerting_service,
            webhook_service,
            xero_service,
            environment,
            payout_executors,
            chain_deposit_floors,
            outbound_store,
        }
    }

    /// Queue a `session.resolved` webhook within an existing transaction.
    /// Builds SessionEventPayload from the updated session + associated transactions.
    /// Returns event IDs for post-commit delivery.
    async fn queue_session_resolved_webhook<C: sea_orm::ConnectionTrait>(
        &self,
        txn: &C,
        session_id: &str,
        merchant_id: &str,
        network_str: &str,
    ) -> Result<Vec<String>> {
        let network_enum = Network::from_str_lenient(network_str)
            .ok_or_else(|| anyhow!("Invalid network '{}'", network_str))?;
        let env = self.environment.clone();

        // Fetch the latest session state (within txn to see uncommitted changes)
        let session = checkout_sessions::Entity::find_by_id(session_id)
            .one(txn)
            .await?
            .ok_or_else(|| anyhow!("Session not found for webhook"))?;

        // Fetch associated transactions directly via ORM
        let txs = transactions::Entity::find()
            .filter(transactions::Column::SessionId.eq(session_id))
            .order_by_asc(transactions::Column::BlockTimestamp)
            .all(txn)
            .await
            .unwrap_or_default();

        let tx_infos: Vec<TransactionInfo> = txs
            .iter()
            .map(|t| TransactionInfo {
                tx_hash: t.tx_hash.clone(),
                amount: crate::api::dtos::checkout::from_micro(t.amount, &session.currency),
                confirmations: t.confirmations_count,
                from_address: t.from_address.clone(),
                detected_at: t.block_timestamp.timestamp(),
            })
            .collect();

        let payload = SessionEventPayload {
            object: "checkout_session",
            id: session.id.clone(),
            merchant_id: session.merchant_id.clone(),
            amount: crate::api::dtos::checkout::from_micro(
                session.amount_expected,
                &session.currency,
            ),
            amount_received: crate::api::dtos::checkout::from_micro(
                session.amount_received,
                &session.currency,
            ),
            fee_amount: session
                .fee_amount
                .map(|v| crate::api::dtos::checkout::from_micro(v, &session.currency)),
            net_amount: session
                .net_amount
                .map(|v| crate::api::dtos::checkout::from_micro(v, &session.currency)),
            currency: session.currency.clone(),
            token_contract: session.currency_contract.clone(),
            network: session.network.clone(),
            livemode: Network::is_livemode_env(&self.environment),
            status: format!("{:?}", session.status),
            pay_address: session.pay_address.clone(),
            client_reference_id: session.client_reference_id.clone(),
            created_at: session.created_at.timestamp(),
            paid_at: None, // Resolution operations don't set paid_at
            tx_count: tx_infos.len() as i32,
            transactions: tx_infos,
            pricing: WebhookPricingInfo {
                currency: session.pricing_currency.clone(),
                amount: session.pricing_amount.normalize().to_string(),
                exchange_rate: session.exchange_rate.to_string(),
            },
        };

        self.webhook_service
            .queue_event_with_txn(
                txn,
                session_id,
                merchant_id,
                network_enum,
                env,
                webhook_events::EVENT_SESSION_RESOLVED,
                &payload,
            )
            .await
    }

    /// Best-effort Xero sync enqueue after exception is resolved into a session.
    async fn enqueue_xero_sync_if_enabled(&self, merchant_id: &str, session_id: &str) {
        let Some(xero_svc) = self.xero_service.as_ref() else {
            return;
        };

        if let Err(e) = xero_svc
            .enqueue_sync_if_enabled(merchant_id, self.environment.clone(), session_id)
            .await
        {
            warn!(
                merchant_id = %merchant_id,
                session_id = %session_id,
                error = %e,
                "Failed to enqueue Xero sync after exception resolution"
            );
        }
    }

    pub async fn get_stats(
        &self,
        merchant_ids: &[String],
        network: Option<&str>,
    ) -> Result<ResolutionStatsResponse> {
        let now = chrono::Utc::now();
        let last_24h = now - chrono::Duration::hours(24);

        // Optimized with SQL SUM and COUNT to avoid OOM
        // Note: PostgreSQL SUM(bigint) returns NUMERIC, so we use Decimal
        let mut stats_query = payment_exceptions::Entity::find()
            .filter(payment_exceptions::Column::MerchantId.is_in(merchant_ids));
        if let Some(net) = network {
            stats_query = stats_query.filter(payment_exceptions::Column::Network.eq(net));
        }
        let stats = stats_query
            .filter(
                payment_exceptions::Column::Status.eq(payment_exceptions::ExceptionStatus::Pending),
            )
            .select_only()
            .column_as(payment_exceptions::Column::Amount.sum(), "unresolved_value")
            .column_as(payment_exceptions::Column::Id.count(), "unresolved_count")
            .into_tuple::<(Option<Decimal>, i64)>()
            .one(&self.db)
            .await?
            .unwrap_or((None, 0));

        let unresolved_count = stats.1;
        let unresolved_value =
            stats.0.unwrap_or(Decimal::ZERO) / Decimal::from(10_u32.pow(USDT_DECIMALS));

        let mut dust_query = payment_exceptions::Entity::find()
            .filter(payment_exceptions::Column::MerchantId.is_in(merchant_ids));
        if let Some(net) = network {
            dust_query = dust_query.filter(payment_exceptions::Column::Network.eq(net));
        }
        let dust_count_24h = dust_query
            .filter(
                payment_exceptions::Column::ExceptionType
                    .eq(payment_exceptions::ExceptionType::DustPayment),
            )
            .filter(payment_exceptions::Column::CreatedAt.gte(last_24h))
            .count(&self.db)
            .await?;

        Ok(ResolutionStatsResponse {
            unresolved_count,
            unresolved_value: unresolved_value.to_string(),
            dust_count_24h: dust_count_24h as i64,
        })
    }

    pub async fn list_exceptions(
        &self,
        merchant_ids: &[String],
        network: Option<&str>,
        status_filter: Option<String>,
        exception_type_filter: Option<String>,
        search_text: Option<String>,
        page: u64,
        page_size: u64,
    ) -> Result<(
        Vec<(payment_exceptions::Model, Option<checkout_sessions::Model>)>,
        u64,
    )> {
        let mut query = payment_exceptions::Entity::find()
            .find_also_related(checkout_sessions::Entity)
            .filter(payment_exceptions::Column::MerchantId.is_in(merchant_ids))
            // Dust payments are auto-resolved and too noisy for the list.
            // They remain in DB for audit; dust_count_24h stat is unaffected.
            .filter(
                payment_exceptions::Column::ExceptionType
                    .ne(payment_exceptions::ExceptionType::DustPayment),
            );
        if let Some(net) = network {
            query = query.filter(payment_exceptions::Column::Network.eq(net));
        }

        if let Some(search) = search_text.filter(|s| !s.is_empty()) {
            query = query.filter(
                Condition::any()
                    .add(payment_exceptions::Column::TxHash.contains(&search))
                    .add(payment_exceptions::Column::FromAddress.contains(&search))
                    .add(payment_exceptions::Column::ToAddress.contains(search)),
            );
        }

        // Exception Type filter
        if let Some(ref type_str) = exception_type_filter {
            if !type_str.is_empty() {
                use payment_exceptions::ExceptionType;
                let exception_type = match type_str.as_str() {
                    "SessionExpired" => Some(ExceptionType::SessionExpired),
                    "NoActiveSession" => Some(ExceptionType::NoActiveSession),
                    "SessionAlreadyCompleted" => Some(ExceptionType::SessionAlreadyCompleted),
                    "RiskBlocked" => Some(ExceptionType::RiskBlocked),
                    "UnderpaidExpired" => Some(ExceptionType::UnderpaidExpired),
                    _ => None,
                };
                if let Some(et) = exception_type {
                    query = query.filter(payment_exceptions::Column::ExceptionType.eq(et));
                }
            }
        }

        // Status filter
        if let Some(status) = status_filter {
            match status.as_str() {
                "pending" => {
                    query = query.filter(
                        payment_exceptions::Column::Status
                            .eq(payment_exceptions::ExceptionStatus::Pending),
                    );
                }
                "processing" => {
                    query = query.filter(
                        payment_exceptions::Column::Status
                            .eq(payment_exceptions::ExceptionStatus::Processing),
                    );
                }
                "resolved" => {
                    query = query.filter(
                        payment_exceptions::Column::Status
                            .eq(payment_exceptions::ExceptionStatus::Resolved),
                    );
                }
                "failed" => {
                    query = query.filter(
                        payment_exceptions::Column::Status
                            .eq(payment_exceptions::ExceptionStatus::Failed),
                    );
                }
                _ => {}
            }
        }

        let paginator = query
            .order_by_desc(payment_exceptions::Column::CreatedAt)
            .paginate(&self.db, page_size);

        let total = paginator.num_items().await?;
        let data = paginator.fetch_page(page.saturating_sub(1)).await?;

        Ok((data, total))
    }

    /// Shared helper: compute fee, credit merchant balance, and update session amounts/status.
    ///
    /// Used by both `accept_expired_session` and `attach_session` to avoid code duplication.
    async fn credit_and_update_session(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        exception: &payment_exceptions::Model,
        merchant_id: &str,
        session_id: &str,
        new_status: checkout_sessions::SessionStatus,
        description: String,
        custom_pct: Option<rust_decimal::Decimal>,
    ) -> Result<()> {
        let network_enum = crate::entity::Network::from_str_lenient(&exception.network)
            .ok_or_else(|| anyhow!("Invalid network '{}'", exception.network))?;
        let chain_floor = self.chain_deposit_floors.get(&network_enum).copied();
        let (actual_fee, net) =
            self.fee_config
                .net_after_fee_for_chain(exception.amount, chain_floor, custom_pct);
        let env = self.environment.clone();
        self.billing_service
            .process_deposit(
                txn,
                merchant_id,
                net,
                Some(format!("exception_{}", exception.id)),
                Some(description),
                network_enum,
                env,
                &exception.currency_symbol,
                Some(exception.amount),
                Some(actual_fee),
            )
            .await?;

        // Update session: amount + status (always), fee/net (always — even when net = 0)
        let session_update = checkout_sessions::Entity::update_many()
            .col_expr(
                checkout_sessions::Column::AmountReceived,
                Expr::col(checkout_sessions::Column::AmountReceived).add(exception.amount),
            )
            .col_expr(checkout_sessions::Column::Status, Expr::value(new_status))
            .col_expr(
                checkout_sessions::Column::UpdatedAt,
                Expr::value(chrono::Utc::now()),
            )
            .col_expr(
                checkout_sessions::Column::FeeAmount,
                Expr::cust_with_values::<_, sea_orm::Value, _>(
                    "COALESCE(fee_amount, 0) + $1",
                    [actual_fee.into()],
                ),
            )
            .col_expr(
                checkout_sessions::Column::NetAmount,
                Expr::cust_with_values::<_, sea_orm::Value, _>(
                    "COALESCE(net_amount, 0) + $1",
                    [net.into()],
                ),
            )
            .filter(checkout_sessions::Column::Id.eq(session_id));

        session_update.exec(txn).await?;
        Ok(())
    }

    pub async fn accept_expired_session(
        &self,
        exception_id: &str,
        _caller_merchant_id: &str,
        allowed_merchant_ids: &[String],
    ) -> Result<(), ResolutionError> {
        use sea_orm::{QuerySelect, TransactionTrait};
        let txn = self.db.begin().await?;

        let ex = payment_exceptions::Entity::find_by_id(exception_id)
            .one(&txn)
            .await?
            .ok_or_else(|| {
                ResolutionError::NotFound(format!("Exception '{}' not found", exception_id))
            })?;

        // IDOR: exception must belong to caller or one of their sub-merchants
        let ex_merchant_id = ex.merchant_id.as_deref().unwrap_or("");
        if !allowed_merchant_ids.iter().any(|id| id == ex_merchant_id) {
            return Err(ResolutionError::Unauthorized);
        }
        let merchant_id = ex_merchant_id.to_string();

        // Unified action validation
        validate_action(&ex, ResolutionAction::Accept)?;

        // CAS guard: atomically claim this exception (Pending → Processing)
        // Prevents double-credit if two requests arrive concurrently.
        let cas = payment_exceptions::Entity::update_many()
            .col_expr(
                payment_exceptions::Column::Status,
                Expr::value(ExceptionStatus::Processing),
            )
            .col_expr(
                payment_exceptions::Column::UpdatedAt,
                Expr::value(chrono::Utc::now()),
            )
            .filter(payment_exceptions::Column::Id.eq(exception_id))
            .filter(payment_exceptions::Column::Status.eq(ExceptionStatus::Pending))
            .exec(&txn)
            .await?;

        if cas.rows_affected == 0 {
            return Err(ResolutionError::InvalidState);
        }

        let session_id = ex.session_id.clone().ok_or_else(|| {
            ResolutionError::ValidationError("Exception has no session_id".into())
        })?;

        // 1. Lock and fetch current state (Pessimistic Locking)
        let session = checkout_sessions::Entity::find_by_id(&session_id)
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or_else(|| {
                ResolutionError::SessionNotFound(format!("Session '{}' not found", session_id))
            })?;

        // UnderpaidExpired vs SessionExpired have different semantics:
        //
        // UnderpaidExpired: Payment was received and amount_received is already set
        // by the payment processor. But balance was NOT credited (lazy credit model).
        // Accept means "I accept this partial amount as sufficient" →
        // credit balance based on existing amount_received, set status = Paid.
        //
        // SessionExpired: Payment arrived AFTER the session expired. amount_received
        // does NOT include this payment yet. Must add ex.amount to session first,
        // then credit balance.
        let is_underpaid_expired =
            ex.exception_type == payment_exceptions::ExceptionType::UnderpaidExpired;

        // Step 2: Determine credit amount and update amount_received if needed
        let credit_amount = if is_underpaid_expired {
            // Amount already on session — don't add again
            session.amount_received
        } else {
            // SessionExpired: add exception amount to session
            let new_amount = session.amount_received + ex.amount;
            checkout_sessions::Entity::update_many()
                .col_expr(
                    checkout_sessions::Column::AmountReceived,
                    Expr::col(checkout_sessions::Column::AmountReceived).add(ex.amount),
                )
                .col_expr(
                    checkout_sessions::Column::UpdatedAt,
                    Expr::value(chrono::Utc::now()),
                )
                .filter(checkout_sessions::Column::Id.eq(&session_id))
                .exec(&txn)
                .await?;
            new_amount
        };

        // Step 3: Calculate fee on total amount and credit merchant balance
        // Look up merchant's custom fee percentage
        let merchant_custom_pct = merchants::Entity::find_by_id(&merchant_id)
            .one(&txn)
            .await?
            .and_then(|m| m.custom_fee_percentage);

        let (actual_fee, net) = {
            let network_enum =
                crate::entity::Network::from_str_lenient(&ex.network).ok_or_else(|| {
                    ResolutionError::ValidationError(format!("Invalid network '{}'", ex.network))
                })?;
            let chain_floor = self.chain_deposit_floors.get(&network_enum).copied();
            self.fee_config
                .net_after_fee_for_chain(credit_amount, chain_floor, merchant_custom_pct)
        };

        // Always record billing — even when net = 0 (fee consumed entire amount).
        let network_enum =
            crate::entity::Network::from_str_lenient(&ex.network).ok_or_else(|| {
                ResolutionError::ValidationError(format!("Invalid network '{}'", ex.network))
            })?;
        let env = self.environment.clone();
        self.billing_service
            .process_deposit(
                &txn,
                &merchant_id,
                net,
                Some(format!("exception_{}", ex.id)),
                Some(format!(
                    "Accept exception {}: {} {} received, {} {} fee, {} {} net",
                    exception_id,
                    credit_amount as f64 / 1_000_000.0,
                    ex.currency_symbol,
                    actual_fee as f64 / 1_000_000.0,
                    ex.currency_symbol,
                    net as f64 / 1_000_000.0,
                    ex.currency_symbol,
                )),
                network_enum,
                env,
                &ex.currency_symbol,
                Some(credit_amount),
                Some(actual_fee),
            )
            .await?;

        // Step 4: Update session fee/net and optionally status
        //
        // UnderpaidExpired: Session is already Expired (terminal). Accept means
        // "I accept this partial amount" — credit balance but keep status Expired.
        // Do NOT change status: Expired → Underpaid would be a semantic regression
        // (terminal → intermediate) and could trigger Expiry Worker re-entry.
        //
        // SessionExpired: Late payment arrived. Determine new status based on
        // updated total (may become Paid/Overpaid).
        {
            let mut session_update = checkout_sessions::Entity::update_many()
                .col_expr(
                    checkout_sessions::Column::UpdatedAt,
                    Expr::value(chrono::Utc::now()),
                )
                .col_expr(
                    checkout_sessions::Column::FeeAmount,
                    Expr::value(actual_fee),
                )
                .col_expr(checkout_sessions::Column::NetAmount, Expr::value(net))
                .filter(checkout_sessions::Column::Id.eq(&session_id));

            // Only change status for SessionExpired (not UnderpaidExpired)
            if !is_underpaid_expired {
                let new_status = checkout_sessions::SessionStatus::determine_by_amount(
                    session.amount_expected,
                    credit_amount,
                    DUST_THRESHOLD,
                );
                session_update = session_update
                    .col_expr(checkout_sessions::Column::Status, Expr::value(new_status));
            }

            session_update.exec(&txn).await?;
        }

        info!(
            exception_id = %exception_id,
            session_id = %session_id,
            credit_amount = credit_amount,
            fee = actual_fee,
            net = net,
            is_underpaid_expired = is_underpaid_expired,
            "Exception accepted: session finalized, merchant balance credited"
        );

        // 5. Update Exception Status
        let ex_network = ex.network.clone();
        let mut ex_active: payment_exceptions::ActiveModel = ex.into();
        ex_active.status = Set(ExceptionStatus::Resolved);
        ex_active.resolution = Set(Some(Resolution::Accepted));
        ex_active.resolved_at = Set(Some(chrono::Utc::now().into()));
        ex_active.update(&txn).await?;

        // 6. Queue webhook (Transactional Outbox)
        let webhook_ids = self
            .queue_session_resolved_webhook(&txn, &session_id, &merchant_id, &ex_network)
            .await
            .unwrap_or_else(|e| {
                error!(exception_id = %exception_id, error = %e, "Failed to queue session.resolved webhook — merchant will NOT be notified");
                vec![]
            });

        txn.commit().await?;

        // Post-commit: best-effort enqueue Xero sync for the resolved session.
        self.enqueue_xero_sync_if_enabled(&merchant_id, &session_id)
            .await;

        // Post-commit: trigger webhook delivery (fire-and-forget)
        if !webhook_ids.is_empty() {
            info!(
                exception_id = %exception_id,
                event_count = webhook_ids.len(),
                "Triggering session.resolved webhook delivery"
            );
            self.webhook_service.trigger_delivery(&webhook_ids).await;
        }

        crate::services::metrics::inc_resolution("accept_expired", "success");
        Ok(())
    }

    pub async fn attach_session(
        &self,
        exception_id: &str,
        _caller_merchant_id: &str,
        allowed_merchant_ids: &[String],
        session_id: &str,
    ) -> Result<(), ResolutionError> {
        use sea_orm::{QuerySelect, TransactionTrait};
        let txn = self.db.begin().await?;

        let ex = payment_exceptions::Entity::find_by_id(exception_id)
            .one(&txn)
            .await?
            .ok_or_else(|| {
                ResolutionError::NotFound(format!("Exception '{}' not found", exception_id))
            })?;

        // IDOR: exception must belong to caller or one of their sub-merchants
        let ex_merchant_id = ex.merchant_id.as_deref().unwrap_or("");
        if !allowed_merchant_ids.iter().any(|id| id == ex_merchant_id) {
            return Err(ResolutionError::Unauthorized);
        }
        let merchant_id = ex_merchant_id.to_string();

        // Unified action validation (checks status and exception_type)
        validate_action(&ex, ResolutionAction::Attach)?;

        // CAS guard: atomically claim this exception (Pending → Processing)
        // Prevents double-credit if two requests arrive concurrently.
        let cas = payment_exceptions::Entity::update_many()
            .col_expr(
                payment_exceptions::Column::Status,
                Expr::value(ExceptionStatus::Processing),
            )
            .col_expr(
                payment_exceptions::Column::UpdatedAt,
                Expr::value(chrono::Utc::now()),
            )
            .filter(payment_exceptions::Column::Id.eq(exception_id))
            .filter(payment_exceptions::Column::Status.eq(ExceptionStatus::Pending))
            .exec(&txn)
            .await?;

        if cas.rows_affected == 0 {
            return Err(ResolutionError::InvalidState);
        }

        // 1. Lock and Update Session (Pessimistic Locking)
        let session = checkout_sessions::Entity::find_by_id(session_id)
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or_else(|| {
                ResolutionError::SessionNotFound(format!("Session '{}' not found", session_id))
            })?;

        // Cross-tenant guard: target session must belong to the merchant family
        if !allowed_merchant_ids.contains(&session.merchant_id) {
            return Err(ResolutionError::Unauthorized);
        }

        // 2. Atomic Update using Expression
        let new_amount_received = session.amount_received + ex.amount;
        let new_status = checkout_sessions::SessionStatus::determine_by_amount(
            session.amount_expected,
            new_amount_received,
            DUST_THRESHOLD,
        );

        // 3-4. Credit merchant + update session (shared logic)
        // Look up merchant's custom fee percentage
        let merchant_custom_pct = merchants::Entity::find_by_id(&merchant_id)
            .one(&txn)
            .await?
            .and_then(|m| m.custom_fee_percentage);

        let (actual_fee, net) = {
            let network_enum =
                crate::entity::Network::from_str_lenient(&ex.network).ok_or_else(|| {
                    ResolutionError::ValidationError(format!("Invalid network '{}'", ex.network))
                })?;
            let chain_floor = self.chain_deposit_floors.get(&network_enum).copied();
            self.fee_config
                .net_after_fee_for_chain(ex.amount, chain_floor, merchant_custom_pct)
        };
        self.credit_and_update_session(
            &txn,
            &ex,
            &merchant_id,
            session_id,
            new_status,
            format!(
                "Attach exception {} to session {}: {:.2} {} received, {:.2} {} fee, {:.2} {} net",
                exception_id,
                session_id,
                ex.amount as f64 / 1_000_000.0,
                ex.currency_symbol,
                actual_fee as f64 / 1_000_000.0,
                ex.currency_symbol,
                net as f64 / 1_000_000.0,
                ex.currency_symbol,
            ),
            merchant_custom_pct,
        )
        .await?;

        // 5. Create Transaction Record for Transaction History display
        // ON CONFLICT DO NOTHING: idempotent — safe to retry
        use crate::entity::transactions::Column as TxCol;
        let tx_model = transactions::ActiveModel {
            network: Set(ex.network.clone()),
            tx_hash: Set(ex.tx_hash.clone()),
            log_index: Set(ex.log_index),
            session_id: Set(Some(session_id.to_string())),
            merchant_id: Set(merchant_id.to_string()),
            currency_symbol: Set(ex.currency_symbol.clone()),
            currency_contract: Set({
                // Resolve correct contract for the exception's token
                let ex_network = crate::entity::Network::from_str_lenient(&ex.network)
                    .unwrap_or(crate::entity::Network::Tron);
                let chain_cfg = ex_network.chain_config(&self.environment);
                chain_cfg
                    .token_contract(&ex.currency_symbol)
                    .unwrap_or(&chain_cfg.usdt_contract)
                    .to_string()
            }),
            from_address: Set(ex.from_address.clone()),
            to_address: Set(ex.to_address.clone()),
            amount: Set(ex.amount),
            status: Set(transactions::ChainTxState::Confirmed),
            confirmations_count: Set(19), // Already confirmed on-chain
            block_number: Set(ex.block_number),
            block_timestamp: Set(ex.block_timestamp),
            is_credited: Set(true), // Already credited via attach
            created_at: Set(chrono::Utc::now().into()),
            updated_at: Set(chrono::Utc::now().into()),
        };
        transactions::Entity::insert(tx_model)
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns([
                    TxCol::Network,
                    TxCol::TxHash,
                    TxCol::LogIndex,
                ])
                .do_nothing()
                .to_owned(),
            )
            .do_nothing()
            .exec(&txn)
            .await?;

        // 6. Update Exception Status
        let ex_network = ex.network.clone();
        let mut ex_active: payment_exceptions::ActiveModel = ex.into();
        ex_active.session_id = Set(Some(session_id.to_string()));
        ex_active.status = Set(ExceptionStatus::Resolved);
        ex_active.resolution = Set(Some(Resolution::Attached));
        ex_active.resolution_ref_id = Set(Some(session_id.to_string()));
        ex_active.resolved_at = Set(Some(chrono::Utc::now().into()));
        ex_active.update(&txn).await?;

        // 7. Queue webhook (Transactional Outbox)
        let webhook_ids = self
            .queue_session_resolved_webhook(&txn, session_id, &merchant_id, &ex_network)
            .await
            .unwrap_or_else(|e| {
                error!(exception_id = %exception_id, error = %e, "Failed to queue session.resolved webhook — merchant will NOT be notified");
                vec![]
            });

        txn.commit().await?;

        // Post-commit: best-effort enqueue Xero sync for the attached session.
        self.enqueue_xero_sync_if_enabled(&merchant_id, session_id)
            .await;

        // Post-commit: trigger webhook delivery (fire-and-forget)
        if !webhook_ids.is_empty() {
            info!(
                exception_id = %exception_id,
                event_count = webhook_ids.len(),
                "Triggering session.resolved webhook delivery"
            );
            self.webhook_service.trigger_delivery(&webhook_ids).await;
        }

        crate::services::metrics::inc_resolution("attach_session", "success");
        Ok(())
    }

    pub async fn manual_transfer(
        &self,
        exception_id: &str,
        caller_merchant_id: &str,
        allowed_merchant_ids: &[String],
        req: crate::api::dtos::resolution::TransferRequest,
    ) -> Result<String, ResolutionError> {
        // ============== SYNCHRONOUS PART (must complete before returning) ==============

        // 1. Verify 2FA (always against the caller/parent merchant)
        self.merchant_service
            .verify_totp_action(caller_merchant_id, &req.code)
            .await
            .map_err(|e| ResolutionError::TwoFactorFailed(e.to_string()))?;

        let ex = payment_exceptions::Entity::find_by_id(exception_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                ResolutionError::NotFound(format!("Exception '{}' not found", exception_id))
            })?;

        // IDOR: exception must belong to caller or one of their sub-merchants
        let ex_merchant_id = ex.merchant_id.as_deref().unwrap_or("");
        if !allowed_merchant_ids.iter().any(|id| id == ex_merchant_id) {
            return Err(ResolutionError::Unauthorized);
        }
        let merchant_id = ex_merchant_id.to_string();

        // Unified action validation (checks status and exception_type)
        validate_action(&ex, ResolutionAction::Transfer)?;

        // Funds are at `ex.to_address` (payment address).
        // Sweeper skips addresses with unresolved exceptions, so funds are always here.
        let source_address = ex.to_address.clone();

        // Destination: provided in request
        let destination = req.to_address.clone();

        // 0. Network-aware Address Validation
        let exception_network = Network::from_str_lenient(&ex.network).ok_or_else(|| {
            ResolutionError::ValidationError(format!(
                "Invalid network '{}' on exception",
                ex.network
            ))
        })?;
        // Verify payout executor is available for this network
        if !self.payout_executors.contains_key(&exception_network) {
            return Err(ResolutionError::ValidationError(format!(
                "Payout executor not configured for network {}. Cannot process exception.",
                ex.network
            )));
        }

        exception_network
            .validate_collection_address(&destination)
            .map_err(|e| {
                ResolutionError::ValidationError(format!("Invalid destination address: {}", e))
            })?;

        // AML RESTRICTION: RiskBlocked exceptions cannot be transferred to merchant's collection_address
        // This prevents AML-risky funds from entering the merchant's account.
        if ex.exception_type == payment_exceptions::ExceptionType::RiskBlocked {
            use crate::entity::merchant_chain_accounts;

            // Fetch all collection addresses for this merchant
            let chain_accounts = merchant_chain_accounts::Entity::find()
                .filter(merchant_chain_accounts::Column::MerchantId.eq(&merchant_id))
                .all(&self.db)
                .await?;

            for account in chain_accounts {
                if let Some(ref collection_addr) = account.collection_address {
                    if collection_addr.eq_ignore_ascii_case(&destination) {
                        return Err(ResolutionError::AmlBlocked(
                            "AML Compliance: RiskBlocked exceptions cannot be transferred to merchant collection addresses. \
                            Please transfer to an external address such as the original sender for refund.".into()
                        ));
                    }
                }
            }
        }

        let is_aml = ex.exception_type == payment_exceptions::ExceptionType::RiskBlocked;

        // Merchant needed for AML path (HD account_index for signing from payment address).
        let merchant = merchants::Entity::find_by_id(&merchant_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                ResolutionError::NotFound(format!("Merchant '{}' not found", merchant_id))
            })?;

        // AML: full refund from payment address, no fee (platform doesn't profit from dirty funds).
        // Others: refund from payment address with platform fee deducted.
        let (refund_fee, refund_amount) = if is_aml {
            (0i64, ex.amount)
        } else {
            let (fee, net) =
                self.fee_config
                    .net_after_fee(ex.amount, self.fee_config.floor_refund, None);
            (fee, net)
        };

        if refund_amount <= 0 {
            return Err(ResolutionError::AmountTooSmall {
                amount: ex.amount,
                fee: refund_fee,
            });
        }

        // Unified refund design: always send from payment address with merchant HD key.
        // Sweeper skips addresses with unresolved exceptions, so funds are always here.
        // AML (RiskBlocked): address is Locked, additional compliance restrictions apply.
        use crate::entity::addresses;
        let addr = addresses::Entity::find()
            .filter(addresses::Column::Address.eq(&source_address))
            .filter(addresses::Column::Network.eq(&ex.network))
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                ResolutionError::NotFound(format!("Address '{}' not found", source_address))
            })?;
        let (payout_from, payout_account_index, payout_path_index) = (
            source_address.clone(),
            merchant.account_index.unwrap_or(0),
            addr.path_index as u32,
        );

        // Pre-validate token contract and decimals BEFORE CAS.
        // If this fails, the exception stays Pending — merchant can retry.
        // CRITICAL: Must be before CAS to avoid stuck Processing state.
        let chain_config = exception_network.chain_config(&self.environment);
        let resolved_token_contract = chain_config
            .token_contract(&ex.currency_symbol)
            .ok_or_else(|| {
                ResolutionError::ValidationError(format!(
                    "Token '{}' not supported on network {}",
                    ex.currency_symbol, ex.network
                ))
            })?
            .to_string();
        let resolved_token_decimals = chain_config
            .token_decimals(&ex.currency_symbol)
            .ok_or_else(|| {
                ResolutionError::ValidationError(format!(
                    "Token decimals not configured for '{}' on network {}",
                    ex.currency_symbol, ex.network
                ))
            })?;

        // Pre-check: verify payment address has sufficient balance (DB-tracked).
        // This catches historical exceptions where funds were already swept to treasury
        // before the sweeper exception guard was deployed.
        let available_balance = if ex.currency_symbol == "USDC" {
            addr.usdc_balance
        } else {
            addr.usdt_balance
        };
        if available_balance < refund_amount {
            return Err(ResolutionError::InsufficientBalance {
                available: available_balance,
                required: refund_amount,
            });
        }

        info!(
            exception_id = exception_id,
            gross = ex.amount,
            fee = refund_fee,
            net = refund_amount,
            "Refund fee calculated"
        );

        // 2. Atomic Check and Mark as Processing (Optimistic Locking) - BEFORE spawn
        let update_res = payment_exceptions::Entity::update_many()
            .col_expr(
                payment_exceptions::Column::Status,
                Expr::value(ExceptionStatus::Processing),
            )
            .col_expr(
                payment_exceptions::Column::UpdatedAt,
                Expr::value(chrono::Utc::now()),
            )
            .col_expr(
                payment_exceptions::Column::Notes,
                Expr::value(Some(format!(
                    "Processing: refund gross={} fee={} net={}",
                    ex.amount, refund_fee, refund_amount
                ))),
            )
            .filter(payment_exceptions::Column::Id.eq(exception_id))
            .filter(payment_exceptions::Column::Status.eq(ExceptionStatus::Pending))
            .exec(&self.db)
            .await?;

        if update_res.rows_affected == 0 {
            return Err(ResolutionError::InvalidState);
        }

        // Create the durable attempt before the spawned task can sign or broadcast.
        let outbound_id = crate::services::outbound::new_id();
        let mut outbound = crate::services::outbound::preparing_model(
            outbound_id.clone(),
            merchant_id.to_string(),
            self.environment,
            outbound_transactions::OutboundOperationType::ManualTransfer,
            ex.network.clone(),
            payout_from.clone(),
            destination.clone(),
            refund_amount,
            ex.currency_symbol.clone(),
        );
        outbound.exception_id = Set(Some(exception_id.to_string()));
        if let Err(error) = crate::services::outbound::create_attempt(&self.db, outbound).await {
            let _ = payment_exceptions::Entity::update_many()
                .col_expr(
                    payment_exceptions::Column::Status,
                    Expr::value(ExceptionStatus::Pending),
                )
                .col_expr(
                    payment_exceptions::Column::Notes,
                    Expr::value(Some(format!("Failed to create outbound journal: {error}"))),
                )
                .filter(payment_exceptions::Column::Id.eq(exception_id))
                .exec(&self.db)
                .await;
            return Err(ResolutionError::Internal(error));
        }

        let amount = refund_amount;
        let user_notes = req.notes.clone();

        // ============== ASYNC PART (spawned to background) ==============
        // All refunds sent from payment address with merchant HD key.
        // No broadcast lock needed — each payment address has independent nonces.

        let db = self.db.clone();
        let alerting_service = self.alerting_service.clone();
        let ex_id = exception_id.to_string();
        let network = ex.network.clone();
        // Token contract + decimals already validated above (before CAS).
        let token_contract = resolved_token_contract;
        let token_decimals = resolved_token_decimals;
        let executor = self
            .payout_executors
            .get(&exception_network)
            .cloned()
            .expect("verified above");
        let outbound_store = self.outbound_store.clone();

        tokio::spawn(async move {
            debug!(exception_id = %ex_id, network = %network, is_aml = is_aml, "Starting async manual transfer");

            // Helper: rollback exception to Pending on failure
            let rollback = |db: &DatabaseConnection,
                            ex_id: &str,
                            stage: &str,
                            err: &dyn std::fmt::Display| {
                let db = db.clone();
                let ex_id = ex_id.to_string();
                let msg = format!("Transfer failed ({}): {}", stage, err);
                async move {
                    let _ = payment_exceptions::Entity::update_many()
                        .col_expr(
                            payment_exceptions::Column::Status,
                            Expr::value(ExceptionStatus::Pending),
                        )
                        .col_expr(payment_exceptions::Column::Notes, Expr::value(Some(msg)))
                        .col_expr(
                            payment_exceptions::Column::UpdatedAt,
                            Expr::value(chrono::Utc::now()),
                        )
                        .filter(payment_exceptions::Column::Id.eq(&ex_id))
                        .filter(payment_exceptions::Column::Status.eq(ExceptionStatus::Processing))
                        .exec(&db)
                        .await;
                }
            };

            // ─── Step 1: Execute payout via PayoutExecutor ───
            // Always from payment address with merchant HD indices.
            let transfer_result: Result<
                (
                    String,
                    Option<String>,
                    crate::services::outbound::BroadcastDisposition,
                ),
                anyhow::Error,
            > = {
                match executor
                    .execute_payout(
                        &payout_from,
                        &destination,
                        u64::try_from(amount).expect("refund_amount validated > 0 above"),
                        payout_account_index,
                        payout_path_index,
                        &token_contract,
                        token_decimals,
                        &outbound_id,
                        &outbound_store,
                    )
                    .await
                {
                    Ok(result) => Ok((
                        result.tx_hash,
                        result.funding_tx_hash,
                        result.broadcast_disposition,
                    )),
                    Err(e) => Err(e),
                }
            };

            // ─── Step 2: Handle result (shared for both chains) ───

            let (tx_hash, funding_tx_hash, broadcast_disposition) = match transfer_result {
                Ok((tx_hash, funding_tx_hash, broadcast_disposition)) => {
                    (tx_hash, funding_tx_hash, broadcast_disposition)
                }
                Err(e) => {
                    warn!(exception_id = %ex_id, error = %e, "Transfer failed");
                    let root_failed = outbound_store
                        .mark_preparing_failed(&outbound_id, e.to_string())
                        .await;
                    alerting_service.send_alert(
                        "resolution_manual_failed",
                        AlertLevel::Warning,
                        &format!("Manual transfer failed for exception {}: {}", ex_id, e),
                    );
                    match root_failed {
                        Ok(true) => rollback(&db, &ex_id, "pre-signing", &e).await,
                        Ok(false) => {
                            warn!(
                                exception_id = %ex_id,
                                outbound_id = %outbound_id,
                                "Manual transfer failure occurred after signing; retaining Processing state for recovery"
                            );
                        }
                        Err(error) => {
                            error!(
                                exception_id = %ex_id,
                                outbound_id = %outbound_id,
                                error = %error,
                                "Cannot establish whether manual transfer failed before signing; refusing rollback"
                            );
                        }
                    }
                    return;
                }
            };

            match outbound_store
                .adopt_executor_result(&outbound_id, &tx_hash, broadcast_disposition)
                .await
            {
                Ok(true) => {}
                Ok(false) => {
                    error!(outbound_id = %outbound_id, "Manual transfer outbound handoff conflicted; retaining Processing state for recovery");
                    return;
                }
                Err(error) => {
                    error!(outbound_id = %outbound_id, error = %error, "Failed to adopt manual transfer result; retaining Processing state for recovery");
                    return;
                }
            }

            if let Some(funding_tx_hash) = funding_tx_hash {
                let _ = outbound_transactions::Entity::update_many()
                    .col_expr(
                        outbound_transactions::Column::FundingTxHash,
                        Expr::value(Some(funding_tx_hash)),
                    )
                    .filter(outbound_transactions::Column::Id.eq(&outbound_id))
                    .exec(&db)
                    .await;
            }

            // ─── Step 3: Update exception with tx_hash ───

            let notes_content = if let Some(notes) = user_notes {
                format!("Broadcasted: {} | {}", tx_hash, notes)
            } else {
                format!("Broadcasted: {}", tx_hash)
            };

            if let Err(e) = payment_exceptions::Entity::update_many()
                .col_expr(
                    payment_exceptions::Column::ResolutionRefId,
                    Expr::value(Some(tx_hash.clone())),
                )
                .col_expr(
                    payment_exceptions::Column::Notes,
                    Expr::value(Some(notes_content)),
                )
                .col_expr(
                    payment_exceptions::Column::UpdatedAt,
                    Expr::value(chrono::Utc::now()),
                )
                .filter(payment_exceptions::Column::Id.eq(&ex_id))
                .filter(payment_exceptions::Column::Status.eq(ExceptionStatus::Processing))
                .exec(&db)
                .await
            {
                error!(exception_id = %ex_id, error = %e, "Failed to update exception after broadcast success");
            }

            debug!(exception_id = %ex_id, tx_hash = %tx_hash, "Async manual transfer broadcasted");
        });

        crate::services::metrics::inc_resolution("manual_transfer", "success");
        // Return immediately
        Ok("submitted".to_string())
    }

    /// Recover exceptions stuck in Processing state from interrupted spawned tasks.
    ///
    /// `manual_transfer` does CAS (Pending → Processing) synchronously, then spawns
    /// a background task for broadcast. If the app crashes mid-spawn, the exception
    /// stays Processing with no `resolution_ref_id` (tx_hash). This method resets
    /// them back to Pending so the merchant can retry.
    ///
    /// CRITICAL: Must NOT reset exceptions that have a sweep record with a tx_hash.
    /// Ghost broadcast scenario: broadcast succeeded + sweep inserted + but
    /// resolution_ref_id not yet written. The confirmation_cycle will handle
    /// finalizing these. Resetting would allow a double-transfer.
    ///
    /// 60s threshold: Docker ensures sequential deploy (old container dead before
    /// new starts), and broadcasts complete in seconds. 60s is very conservative.
    /// Called once at startup in main.rs.
    pub async fn recover_stale_processing(&self) {
        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(60);

        let result = self
            .db
            .execute(sea_orm::Statement::from_sql_and_values(
                sea_orm::DbBackend::Postgres,
                r#"
                WITH stale AS (
                    SELECT payment_exceptions.id
                    FROM payment_exceptions
                    WHERE status = 'Processing'
                      AND resolution_ref_id IS NULL
                      AND updated_at < $1
                      AND NOT EXISTS (
                          SELECT 1 FROM outbound_transactions
                          WHERE outbound_transactions.exception_id = payment_exceptions.id
                            AND outbound_transactions.purpose = 'token_transfer'
                            AND outbound_transactions.parent_transaction_id IS NULL
                            AND outbound_transactions.operation_type IN ('manual_sweep', 'manual_transfer')
                            AND outbound_transactions.state IN ('Signed', 'BroadcastUnknown', 'Pending')
                            AND outbound_transactions.tx_hash IS NOT NULL
                      )
                ), failed_journals AS (
                    UPDATE outbound_transactions
                    SET state = 'Failed',
                        error_message = 'Auto-reset: transfer interrupted before transaction signing',
                        updated_at = NOW()
                    FROM stale
                    WHERE outbound_transactions.exception_id = stale.id
                      AND outbound_transactions.purpose = 'token_transfer'
                      AND outbound_transactions.parent_transaction_id IS NULL
                      AND outbound_transactions.operation_type IN ('manual_sweep', 'manual_transfer')
                      AND outbound_transactions.state = 'Preparing'
                    RETURNING outbound_transactions.id
                )
                UPDATE payment_exceptions
                SET status = 'Pending',
                    notes = 'Auto-reset: transfer interrupted (Processing with no tx_hash)',
                    updated_at = NOW()
                FROM stale
                WHERE payment_exceptions.id = stale.id
                "#,
                [cutoff.into()],
            ))
            .await;

        match result {
            Ok(r) if r.rows_affected() > 0 => {
                warn!(
                    count = r.rows_affected(),
                    "Reset stale Processing exceptions (no resolution_ref_id, no sweep with tx_hash) → Pending"
                );
            }
            Err(e) => {
                error!(error = %e, "Failed to recover stale Processing exceptions");
            }
            _ => {}
        }
    }
}
