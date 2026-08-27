//! Checkout Session Service
//!
//! Manages checkout session lifecycle: creation, payment matching, expiration.
//! Aligned with docs/system_design.md

use anyhow::Result;
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, Set, Statement, TransactionTrait,
};
use serde::Serialize;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

use crate::services::exchange_rate::ExchangeRateService;

use crate::entity::{checkout_sessions, transactions, CheckoutSessions, Environment, Network};

use super::CheckoutError;

// ============================================================
// Service Definition
// ============================================================

pub struct CheckoutService {
    db: DatabaseConnection,
    session_expiry_minutes: u64,
    /// Exchange rate service for fiat pricing (None = fiat pricing disabled)
    exchange_rate_service: Option<Arc<ExchangeRateService>>,
}

/// Request to create a new checkout session
///
/// All fields that affect financial isolation are **required** (not Optional).
/// This enforces explicit environment handling and prevents accidental
/// cross-network operations.
#[derive(Debug, Clone)]
pub struct CreateSessionRequest {
    pub merchant_id: String,
    pub amount_expected: i64,
    /// Currency code (e.g., "USDT") - REQUIRED
    pub currency: String,
    /// Environment context - REQUIRED
    pub environment: Environment,
    /// Network enum (e.g., Network::Tron) - REQUIRED
    pub network: Network,
    pub client_reference_id: Option<String>,
    /// URL to redirect after successful payment (optional for API-only integrations)
    pub success_url: Option<String>,
    /// URL to redirect after payment expires or is cancelled (optional)
    pub cancel_url: Option<String>,

    // ---- Pricing Fields (always populated) ----
    /// Pricing amount as Decimal (fiat: 10.50 for $10.50, crypto: 10.50 for 10.50 USDT)
    /// None only for fiat mode (service will compute from exchange rate)
    pub pricing_amount: Option<Decimal>,
    /// Pricing currency code ("USD", "CNY", "USDT", "USDC" — always set)
    pub pricing_currency: String,
    /// Sub-merchant code (set when session is created via PSP context switch)
    pub sub_merchant_code: Option<String>,
}

/// Individual transaction info for webhook payload
#[derive(Debug, Clone, Serialize)]
pub struct TransactionInfo {
    pub tx_hash: String,
    pub amount: String,
    pub confirmations: i32,
    /// Source address (for refunds and audit)
    pub from_address: String,
    /// Unix timestamp when transaction was detected on-chain
    pub detected_at: i64,
}

/// Fiat pricing info embedded in webhook payloads.
#[derive(Debug, Clone, Serialize)]
pub struct WebhookPricingInfo {
    pub currency: String,
    pub amount: String,
    pub exchange_rate: String,
}

/// Webhook event payload for session events
#[derive(Debug, Clone, Serialize)]
pub struct SessionEventPayload {
    /// Object type identifier (always "checkout_session")
    pub object: &'static str,
    pub id: String,
    pub merchant_id: String,
    /// Amount expected in standard units (e.g., "10.5" = 10.5 USDT)
    pub amount: String,
    /// Amount received in standard units
    pub amount_received: String,
    /// Platform fee deducted, in standard units. Only present for completed sessions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_amount: Option<String>,
    /// Net amount credited to merchant after fee, in standard units. Only present for completed sessions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub net_amount: Option<String>,
    pub currency: String,
    /// Token contract address for security verification (prevents fake token attacks)
    pub token_contract: String,
    pub network: String,
    /// `true` for production, `false` for sandbox
    pub livemode: bool,
    pub status: String,
    pub pay_address: String,
    pub client_reference_id: Option<String>,
    /// Unix timestamp when session was created
    pub created_at: i64,
    /// Unix timestamp when payment was completed (None for expired sessions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paid_at: Option<i64>,
    /// Total number of transactions credited to this session
    pub tx_count: i32,
    /// Complete transaction history, ordered by block_timestamp ASC
    pub transactions: Vec<TransactionInfo>,

    /// Pricing details. Always present. For crypto-only sessions, echoes the
    /// settlement currency and amount. For fiat sessions, includes exchange rate.
    pub pricing: WebhookPricingInfo,
}

/// Information about an expired session for webhook notifications
#[derive(Debug, Clone)]
pub struct ExpiredSessionInfo {
    pub session_id: String,
    pub merchant_id: String,
    pub network: String,
    pub pay_address: String,
    pub currency: String,
    /// Token contract address for security verification
    pub currency_contract: String,
    pub amount_expected: i64,
    pub amount_received: i64,
    pub client_reference_id: Option<String>,
    /// Session creation timestamp (for webhook payload)
    pub created_at: i64,
    // ---- Pricing Snapshot (always populated) ----
    pub pricing_currency: String,
    pub pricing_amount: Decimal,
    pub exchange_rate: Decimal,
}

impl CheckoutService {
    pub fn new(
        db: DatabaseConnection,
        session_expiry_minutes: u64,
        exchange_rate_service: Option<Arc<ExchangeRateService>>,
    ) -> Self {
        Self {
            db,
            session_expiry_minutes,
            exchange_rate_service,
        }
    }

    /// Create a new checkout session
    ///
    /// **Atomicity**: Address allocation and session creation are performed
    /// within a single database transaction. If session creation fails,
    /// the address allocation is automatically rolled back.
    ///
    /// **Idempotency**: If `client_reference_id` is provided, the system checks
    /// for an existing active session with the same merchant_id + client_reference_id.
    /// If found, returns the existing session instead of creating a duplicate.
    /// This protects against network retries creating multiple sessions.
    ///
    /// # Errors
    /// - `CheckoutError::AddressPoolExhausted` - No available addresses (HTTP 503)
    /// - `CheckoutError::InvalidRequest` - Unsupported network or currency
    pub async fn create_session(
        &self,
        req: CreateSessionRequest,
    ) -> Result<checkout_sessions::Model, CheckoutError> {
        // Validate network and get network-specific params
        let network_params = req.network.chain_config(&req.environment);

        // Validate currency
        match req.currency.as_str() {
            "USDT" => {}
            "USDC" => {
                // USDC not supported on TRON (no USDC contract)
                if req.network == Network::Tron {
                    return Err(CheckoutError::InvalidRequest(
                        "USDC is not supported on TRON network.".to_string(),
                    ));
                }
                // USDC not supported in Sandbox (no test contracts configured)
                if req.environment == Environment::Sandbox {
                    return Err(CheckoutError::InvalidRequest(
                        "USDC is not supported in Sandbox environment.".to_string(),
                    ));
                }
                // Verify chain actually has a USDC contract configured
                if network_params.usdc_contract.is_none() {
                    return Err(CheckoutError::InvalidRequest(format!(
                        "USDC is not configured for network {:?}.",
                        req.network
                    )));
                }
            }
            _ => {
                return Err(CheckoutError::InvalidRequest(format!(
                    "Unsupported currency: {}. Supported: USDT, USDC.",
                    req.currency
                )));
            }
        }

        // ============================================================
        // PRICING: Determine amount_expected and pricing snapshot
        // ============================================================
        let pricing_currency = req.pricing_currency.to_uppercase();
        let is_fiat = !crate::api::dtos::checkout::is_crypto(&pricing_currency);

        let (amount_expected, pricing_amount, exchange_rate) = if is_fiat {
            let pricing_amt = req.pricing_amount.ok_or_else(|| {
                CheckoutError::InvalidRequest("pricing_amount required for fiat mode.".to_string())
            })?;

            // Fiat pricing mode: lookup exchange rate
            let ers = self.exchange_rate_service.as_ref().ok_or_else(|| {
                CheckoutError::InvalidRequest(
                    "Fiat pricing is not available (exchange rate service not configured)."
                        .to_string(),
                )
            })?;

            let crypto = req.currency.to_uppercase();
            let fetched_rate = ers.get_rate(&crypto, &pricing_currency).map_err(|e| {
                CheckoutError::InvalidRequest(format!(
                    "Cannot get exchange rate for {}/{}: {}",
                    crypto, pricing_currency, e
                ))
            })?;

            if fetched_rate <= Decimal::ZERO {
                return Err(CheckoutError::InvalidRequest(
                    "Exchange rate is zero or negative.".to_string(),
                ));
            }
            let usdt_amount = pricing_amt / fetched_rate;

            let micro =
                crate::api::dtos::checkout::to_micro(usdt_amount, &crypto).ok_or_else(|| {
                    CheckoutError::InvalidRequest(
                        "Amount overflow after fiat conversion.".to_string(),
                    )
                })?;

            if micro < 1_000_000 {
                return Err(CheckoutError::InvalidRequest(format!(
                    "Fiat-priced amount converts to less than 1 {} minimum.",
                    crypto
                )));
            }

            info!(
                pricing_currency = %pricing_currency,
                pricing_amount = %pricing_amt,
                exchange_rate = %fetched_rate,
                usdt_amount = %usdt_amount,
                amount_expected = micro,
                "Fiat pricing conversion completed"
            );

            (micro, pricing_amt, fetched_rate)
        } else {
            // Crypto pricing mode: amount already converted to microunits by route handler
            // pricing_amount = the original decimal amount, exchange_rate = 1.0
            let pricing_amt = req.pricing_amount.ok_or_else(|| {
                CheckoutError::InvalidRequest(
                    "pricing_amount required for crypto mode.".to_string(),
                )
            })?;
            (req.amount_expected, pricing_amt, Decimal::ONE)
        };

        // Resolve currency contract dynamically from ChainConfig
        let currency_contract = match req.currency.as_str() {
            "USDC" => network_params.usdc_contract.clone().unwrap_or_default(),
            _ => network_params.usdt_contract.clone(),
        };

        // ============================================================
        // IDEMPOTENCY CHECK: Return existing session if client_reference_id matches
        // ============================================================
        if let Some(ref client_ref_id) = req.client_reference_id {
            let existing = CheckoutSessions::find()
                .filter(checkout_sessions::Column::MerchantId.eq(&req.merchant_id))
                .filter(checkout_sessions::Column::Network.eq(req.network.as_str())) // Network isolation
                .filter(checkout_sessions::Column::Currency.eq(&req.currency)) // Currency isolation
                .filter(checkout_sessions::Column::ClientReferenceId.eq(client_ref_id))
                // Removed status filter: check ALL statuses to prevent "zombie" retries
                .one(&self.db)
                .await?;

            if let Some(session) = existing {
                // If the session is already Expired, return 410 Gone.
                // This forces the merchant to generate a new order ID (client_ref_id)
                // instead of silently creating a *new* session for an expired order.
                if session.status == checkout_sessions::SessionStatus::Expired {
                    info!(
                        session_id = %session.id,
                        client_reference_id = %client_ref_id,
                        "Client retried an expired session - returning 410 Gone"
                    );
                    return Err(CheckoutError::SessionExpired(session.id));
                }

                info!(
                    session_id = %session.id,
                    client_reference_id = %client_ref_id,
                    "Returning existing session (idempotency)"
                );
                return Ok(session);
            }
        }

        // ============================================================
        // Create new session atomically
        // ============================================================
        let session_id = format!("cs_{}", Uuid::new_v4().simple());
        let expires_at = Utc::now() + Duration::minutes(self.session_expiry_minutes as i64);

        // Begin atomic transaction for address allocation + session creation
        let txn = self.db.begin().await?;

        // Atomic address allocation using UPDATE ... RETURNING (within transaction)
        let pay_address = self
            .allocate_address_atomic_with_txn(
                &txn,
                &req.merchant_id,
                req.network.clone(),
                req.environment.clone(),
                &req.currency,
            )
            .await?
            .ok_or(CheckoutError::AddressPoolExhausted)?;

        // Create session within the same transaction
        let session = checkout_sessions::ActiveModel {
            id: Set(session_id.clone()),
            merchant_id: Set(req.merchant_id),
            network: Set(req.network.as_str().to_string()),
            pay_address: Set(pay_address.clone()),
            client_reference_id: Set(req.client_reference_id),
            currency: Set(req.currency),
            currency_contract: Set(currency_contract),
            amount_expected: Set(amount_expected),
            amount_received: Set(0),
            status: Set(checkout_sessions::SessionStatus::Pending),
            success_url: Set(req.success_url),
            cancel_url: Set(req.cancel_url),
            expires_at: Set(expires_at.into()),
            // Pricing snapshot fields (always populated)
            pricing_currency: Set(pricing_currency),
            pricing_amount: Set(pricing_amount),
            exchange_rate: Set(exchange_rate),
            sub_merchant_code: Set(req.sub_merchant_code),
            ..Default::default()
        };

        let model = session.insert(&txn).await?;

        // Commit the atomic transaction
        txn.commit().await?;

        crate::services::metrics::inc_session("created", req.network.as_str());
        info!(session_id = %session_id, pay_address = %pay_address, "Checkout session created atomically");

        Ok(model)
    }

    /// Atomic address allocation using single UPDATE ... RETURNING statement
    /// within an existing transaction.
    ///
    /// **Currency affinity**: Prefers addresses where the *other* token's balance is 0,
    /// reducing co-located funds and potential WrongToken exceptions.
    async fn allocate_address_atomic_with_txn<C>(
        &self,
        txn: &C,
        merchant_id: &str,
        network: Network,
        _environment: Environment,
        currency: &str,
    ) -> Result<Option<String>, CheckoutError>
    where
        C: ConnectionTrait,
    {
        // Address affinity: prefer addresses clean of the *other* token.
        // USDT session → prefer usdc_balance=0 addresses, sort by usdt_balance DESC (reuse swepted)
        // USDC session → prefer usdt_balance=0 addresses, sort by usdc_balance DESC
        let sql = if currency == "USDC" {
            r#"
            UPDATE addresses
            SET status = 'Assigned',
                updated_at = NOW()
            WHERE (network, address) = (
                SELECT a.network, a.address
                FROM addresses a
                WHERE a.status = 'Idle' AND a.merchant_id = $1 AND a.network = $2
                  AND NOT EXISTS (
                      SELECT 1 FROM addresses a2
                      WHERE a2.address = a.address
                        AND a2.status != 'Idle'
                        AND a2.network != a.network
                  )
                ORDER BY
                    (CASE WHEN a.usdt_balance = 0 THEN 0 ELSE 1 END),
                    a.usdc_balance DESC
                LIMIT 1
                FOR UPDATE SKIP LOCKED
            )
            RETURNING address
            "#
        } else {
            r#"
            UPDATE addresses
            SET status = 'Assigned',
                updated_at = NOW()
            WHERE (network, address) = (
                SELECT a.network, a.address
                FROM addresses a
                WHERE a.status = 'Idle' AND a.merchant_id = $1 AND a.network = $2
                  AND NOT EXISTS (
                      SELECT 1 FROM addresses a2
                      WHERE a2.address = a.address
                        AND a2.status != 'Idle'
                        AND a2.network != a.network
                  )
                ORDER BY
                    (CASE WHEN a.usdc_balance = 0 THEN 0 ELSE 1 END),
                    a.usdt_balance DESC
                LIMIT 1
                FOR UPDATE SKIP LOCKED
            )
            RETURNING address
            "#
        };

        let result = txn
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                [merchant_id.into(), network.as_str().into()],
            ))
            .await?;

        match result {
            Some(row) => {
                let address: String = row
                    .try_get("", "address")
                    .map_err(CheckoutError::Database)?;
                Ok(Some(address))
            }
            None => Ok(None),
        }
    }

    /// Get session by ID with network isolation
    ///
    /// Requires both `session_id` and `network` to enforce environment isolation
    /// at the database query level (defense-in-depth).
    pub async fn get_session(
        &self,
        session_id: &str,
        network: Network,
        _environment: Environment,
    ) -> Result<Option<checkout_sessions::Model>, CheckoutError> {
        let session = CheckoutSessions::find()
            .filter(checkout_sessions::Column::Id.eq(session_id))
            .filter(checkout_sessions::Column::Network.eq(network.as_str()))
            .one(&self.db)
            .await?;
        Ok(session)
    }

    /// List sessions for a merchant with advanced filtering
    ///
    /// # Arguments
    /// * `merchant_id` - The merchant to list sessions for
    /// * `network` - Optional network filter. None = all networks for the environment
    /// * `environment` - Environment filter (required for financial isolation)
    /// * `pagination` - Pagination parameters
    /// * `filter` - Filtering parameters (status, date, search)
    ///
    /// # Financial Isolation
    /// Environment is always required - sessions are never listed across environments.
    /// Network is optional - when None, returns sessions across all chains for that environment.
    pub async fn list_sessions(
        &self,
        merchant_ids: &[String],
        network: Option<Network>,
        _environment: Environment,
        pagination: &crate::api::dtos::pagination::PaginationRequest,
        filter: &crate::api::dtos::checkout::SessionFilterParams,
        search_text: Option<&str>, // Passed explicitly to avoid serde::flatten conflict
    ) -> Result<(Vec<checkout_sessions::Model>, u64), CheckoutError> {
        let mut query = CheckoutSessions::find()
            .filter(checkout_sessions::Column::MerchantId.is_in(merchant_ids));

        // Network filter: if specific network requested, filter by it; otherwise show all
        if let Some(ref net) = network {
            query = query.filter(checkout_sessions::Column::Network.eq(net.as_str()));
        }

        // Filter by Status
        if let Some(ref statuses) = filter.status {
            if !statuses.is_empty() {
                query = query.filter(checkout_sessions::Column::Status.is_in(statuses.clone()));
            }
        }
        // Filter by Date Range
        if let Some(ref after) = filter.created_after {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(after) {
                query = query.filter(checkout_sessions::Column::CreatedAt.gte(dt));
            }
        }
        if let Some(ref before) = filter.created_before {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(before) {
                query = query.filter(checkout_sessions::Column::CreatedAt.lte(dt));
            }
        }

        // Filter by Search Text (ID, Client Ref, or Tx Hash)
        // NOTE: search_text is passed explicitly (not from filter) to avoid serde::flatten conflict
        if let Some(search) = search_text.filter(|s| !s.is_empty()) {
            let mut extra_session_ids = Vec::new();

            // Step 1: Check if search text looks like a Tx Hash (64+ chars for Tron/EVM)
            // If so, query transactions table first to find associated sessions.
            // We MUST enforce merchant_id isolation here too.
            if search.len() >= 64 {
                let mut tx_query = transactions::Entity::find()
                    .filter(transactions::Column::TxHash.eq(search))
                    .filter(transactions::Column::MerchantId.is_in(merchant_ids));
                if let Some(ref net) = network {
                    tx_query = tx_query.filter(transactions::Column::Network.eq(net.as_str()));
                }
                let txs = tx_query
                    .all(&self.db)
                    .await
                    .map_err(CheckoutError::Database)?;

                for tx in txs {
                    extra_session_ids.push(tx.session_id);
                }
            }

            // Step 2: Construct the main search condition
            let mut search_condition = sea_orm::Condition::any()
                .add(checkout_sessions::Column::Id.eq(search))
                .add(checkout_sessions::Column::ClientReferenceId.eq(search));

            // Add found session IDs from Tx search
            if !extra_session_ids.is_empty() {
                search_condition =
                    search_condition.add(checkout_sessions::Column::Id.is_in(extra_session_ids));
            }

            query = query.filter(search_condition);
        }

        // Order by CreatedAt DESC
        query = query.order_by_desc(checkout_sessions::Column::CreatedAt);

        let paginator = query.paginate(&self.db, pagination.page_size);

        let total = paginator
            .num_items()
            .await
            .map_err(CheckoutError::Database)?;
        let data = paginator
            .fetch_page(pagination.page - 1)
            .await
            .map_err(CheckoutError::Database)?;

        Ok((data, total))
    }

    /// Get session by payment address
    /// Returns sessions that are actively waiting for payment (Pending or Underpaid)
    pub async fn get_session_by_address(
        &self,
        network: Network,
        _environment: Environment,
        address: &str,
    ) -> Result<Option<checkout_sessions::Model>, CheckoutError> {
        let session = CheckoutSessions::find()
            .filter(checkout_sessions::Column::Network.eq(network.as_str()))
            .filter(checkout_sessions::Column::PayAddress.eq(address))
            .filter(checkout_sessions::Column::Status.is_in([
                checkout_sessions::SessionStatus::Pending,
                checkout_sessions::SessionStatus::Underpaid,
            ]))
            .one(&self.db)
            .await?;
        Ok(session)
    }

    /// Apply a confirmed payment to a session
    ///
    /// This is the **centralized business logic** for payment status updates.
    /// It handles:
    /// - Amount accumulation
    /// - Status determination (using `SessionStatus::determine_by_amount`)
    /// - Rolling expiration extension for underpaid sessions
    ///
    /// # Arguments
    /// * `session_id` - The session to update
    /// * `amount_received` - The new payment amount to add (not total)
    /// * `underpayment_threshold` - Tolerance for minor underpayments
    ///
    /// # Returns
    /// The updated session model, which can be used to:
    /// - Check if `status.is_successful()` to trigger Sweeper
    /// - Check status changes to trigger Webhook notifications
    ///
    /// # Errors
    /// - `CheckoutError::SessionNotFound` - Session ID not found
    ///
    /// # Note
    /// This method is `pub(crate)` - intended for internal use by `PaymentEventProcessor`.
    /// External callers should not update payment status directly.
    /// Prefer using `apply_payment_with_txn` for atomic operations.
    #[allow(dead_code)]
    pub(crate) async fn apply_payment(
        &self,
        session_id: &str,
        amount_received: i64,
        underpayment_threshold: i64,
    ) -> Result<checkout_sessions::Model, CheckoutError> {
        self.apply_payment_with_txn(
            &self.db,
            session_id,
            amount_received,
            underpayment_threshold,
        )
        .await
    }

    /// Apply a confirmed payment to a session within an existing transaction
    ///
    /// This variant accepts a transaction/connection reference, enabling atomic
    /// operations across multiple tables (e.g., session update + event status update).
    ///
    /// # Arguments
    /// * `txn` - Database connection or transaction to use
    /// * `session_id` - The session to update
    /// * `amount_received` - The new payment amount to add (not total)
    /// * `underpayment_threshold` - Tolerance for minor underpayments
    ///
    /// # Example
    /// ```ignore
    /// let txn = db.begin().await?;
    /// checkout_service.apply_payment_with_txn(&txn, session_id, amount, threshold).await?;
    /// mark_event_processed_with_txn(&txn, event_id).await?;
    /// txn.commit().await?;
    /// ```
    pub(crate) async fn apply_payment_with_txn<C>(
        &self,
        txn: &C,
        session_id: &str,
        amount_received: i64,
        underpayment_threshold: i64,
    ) -> Result<checkout_sessions::Model, CheckoutError>
    where
        C: sea_orm::ConnectionTrait,
    {
        let session = CheckoutSessions::find_by_id(session_id)
            .one(txn)
            .await?
            .ok_or_else(|| CheckoutError::SessionNotFound(session_id.to_string()))?;

        // Skip if session is already in terminal state (idempotency)
        if session.status.is_terminal() {
            info!(
                session_id = %session_id,
                status = ?session.status,
                "Session already terminal, skipping payment application"
            );
            return Ok(session);
        }

        // Calculate new status using centralized logic
        let total_received = session.amount_received + amount_received;
        let new_status = checkout_sessions::SessionStatus::determine_by_amount(
            session.amount_expected,
            total_received,
            underpayment_threshold,
        );

        let mut active: checkout_sessions::ActiveModel = session.into();
        active.amount_received = Set(total_received);
        active.status = Set(new_status.clone());
        active.updated_at = Set(Utc::now().into());

        // Rolling extension for Underpaid
        if new_status == checkout_sessions::SessionStatus::Underpaid {
            let new_expires = Utc::now() + Duration::hours(24);
            active.expires_at = Set(new_expires.into());
            info!(session_id = %session_id, "Applied rolling extension (+24h)");
        }

        let updated = active.update(txn).await?;

        info!(
            session_id = %session_id,
            amount_received = total_received,
            status = ?new_status,
            "Payment applied to session"
        );

        Ok(updated)
    }

    // ============================================================
    // Session Expiry (Split into Query + Transactional Update)
    // ============================================================

    /// Query sessions that are candidates for expiration (read-only, no mutation)
    ///
    /// Returns sessions that have passed their TTL and are still in Pending/Underpaid status.
    /// This is a pure read operation - no database state is changed.
    ///
    /// Used by SessionExpiryWorker to get candidates before processing each one
    /// in an individual transaction with `mark_session_expired_with_txn`.
    pub async fn get_expiry_candidates(&self) -> Result<Vec<ExpiredSessionInfo>, CheckoutError> {
        let now = Utc::now();

        // Read-only query: find sessions past expiry that are still awaiting payment
        let sql = r#"
            SELECT id, merchant_id, network, pay_address, currency, currency_contract,
                   amount_expected, amount_received, client_reference_id,
                   pricing_currency, pricing_amount, exchange_rate,
                   EXTRACT(EPOCH FROM created_at)::bigint as created_at_epoch
            FROM checkout_sessions
            WHERE status IN ('Pending', 'Underpaid')
              AND expires_at < $1
        "#;

        let stmt = Statement::from_sql_and_values(DbBackend::Postgres, sql, [now.into()]);
        let rows = self.db.query_all(stmt).await?;

        let mut candidates = Vec::with_capacity(rows.len());
        for row in rows {
            candidates.push(ExpiredSessionInfo {
                session_id: row.try_get("", "id").map_err(CheckoutError::Database)?,
                merchant_id: row
                    .try_get("", "merchant_id")
                    .map_err(CheckoutError::Database)?,
                network: row
                    .try_get("", "network")
                    .map_err(CheckoutError::Database)?,
                pay_address: row
                    .try_get("", "pay_address")
                    .map_err(CheckoutError::Database)?,
                currency: row
                    .try_get("", "currency")
                    .map_err(CheckoutError::Database)?,
                currency_contract: row
                    .try_get("", "currency_contract")
                    .map_err(CheckoutError::Database)?,
                amount_expected: row
                    .try_get("", "amount_expected")
                    .map_err(CheckoutError::Database)?,
                amount_received: row
                    .try_get("", "amount_received")
                    .map_err(CheckoutError::Database)?,
                client_reference_id: row.try_get("", "client_reference_id").ok(),
                created_at: row
                    .try_get::<i64>("", "created_at_epoch")
                    .map_err(CheckoutError::Database)?,
                pricing_currency: row
                    .try_get("", "pricing_currency")
                    .map_err(CheckoutError::Database)?,
                pricing_amount: row
                    .try_get("", "pricing_amount")
                    .map_err(CheckoutError::Database)?,
                exchange_rate: row
                    .try_get("", "exchange_rate")
                    .map_err(CheckoutError::Database)?,
            });
        }

        Ok(candidates)
    }

    /// Mark a single session as expired within provided transaction (with CAS check)
    ///
    /// Uses Compare-And-Swap (CAS) logic: only updates if status is still Pending/Underpaid.
    /// This prevents race conditions where a payment arrives between `get_expiry_candidates`
    /// and this update.
    ///
    /// Also transitions the associated address to Cooling state.
    ///
    /// # Returns
    /// - `Ok(true)` - Session was successfully marked as Expired
    /// - `Ok(false)` - Session status had already changed (e.g., paid), no update performed
    ///
    /// # Atomicity
    /// Caller must commit the transaction after this AND webhook insertion to ensure
    /// both operations succeed or fail together (Outbox pattern).
    pub async fn mark_session_expired_with_txn<C>(
        &self,
        txn: &C,
        session: &ExpiredSessionInfo,
    ) -> Result<bool, CheckoutError>
    where
        C: ConnectionTrait,
    {
        // CAS Update: Only expire if still in Pending/Underpaid status
        // This prevents overwriting a Paid status if payment arrived after get_expiry_candidates
        //
        // ADDRESS STATUS LOGIC:
        // - Pending → Expired (amount_received = 0, no exception balance): Address → Cooling
        // - Pending → Expired (amount_received = 0, HAS exception balance): Address stays Detected
        //   (WrongToken funds need Sweeper to collect before address can be recycled)
        // - Underpaid → Expired (amount_received > 0): Address stays Detected (Sweeper collects residual)
        //
        // STATUS GUARD: Only transition from Assigned/Detected to Cooling.
        // Prevents overwriting Sweeping (active sweep in progress) or other states.
        let sql = r#"
            WITH expired AS (
                UPDATE checkout_sessions
                SET status = 'Expired',
                    updated_at = NOW()
                WHERE id = $1
                  AND status IN ('Pending', 'Underpaid')
                RETURNING id, pay_address, network, amount_received
            ),
            cooling AS (
                UPDATE addresses
                SET status = 'Cooling',
                    updated_at = NOW()
                WHERE (address, network) IN (
                    SELECT pay_address, network FROM expired WHERE amount_received = 0
                )
                  AND usdt_balance = 0
                  AND usdc_balance = 0
                  AND status IN ('Assigned', 'Detected')
            )
            SELECT COUNT(*) as affected FROM expired
        "#;

        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            [session.session_id.clone().into()],
        );

        let result = txn.query_one(stmt).await?;

        let affected: i64 = result
            .map(|row| row.try_get("", "affected").unwrap_or(0))
            .unwrap_or(0);

        if affected > 0 {
            crate::services::metrics::inc_session("expired", &session.network);
            info!(
                session_id = %session.session_id,
                "Session marked as Expired (CAS success)"
            );
            Ok(true)
        } else {
            info!(
                session_id = %session.session_id,
                "Session expiry skipped: status already changed (CAS miss)"
            );
            Ok(false)
        }
    }

    /// Legacy method - DEPRECATED
    ///
    /// This method has a dual-write bug: it updates sessions in auto-commit mode
    /// before webhooks can be inserted in the same transaction.
    ///
    /// Use `get_expiry_candidates()` + `mark_session_expired_with_txn()` instead.
    #[deprecated(
        since = "0.2.0",
        note = "Use get_expiry_candidates + mark_session_expired_with_txn for atomic operations"
    )]
    pub async fn expire_sessions(&self) -> Result<Vec<ExpiredSessionInfo>, CheckoutError> {
        // For backward compatibility, just call the new read-only method
        // Note: This does NOT actually expire sessions anymore!
        // Callers must migrate to the new pattern.
        self.get_expiry_candidates().await
    }

    /// Get all credited transactions for a session
    ///
    /// Used to build webhook payload with complete transaction history.
    /// Returns transactions ordered by block_timestamp ASC (oldest first).
    pub async fn get_session_transactions(
        &self,
        session_id: &str,
        currency: &str,
    ) -> Result<Vec<TransactionInfo>, CheckoutError> {
        self.get_session_transactions_with_conn(&self.db, session_id, currency)
            .await
    }

    /// Get all credited transactions for a session (within provided transaction)
    ///
    /// This variant accepts a transaction/connection reference, enabling visibility
    /// of uncommitted changes within the same transaction (e.g., newly credited transactions).
    ///
    /// # Arguments
    /// * `conn` - Database connection or transaction to use
    /// * `session_id` - The session to fetch transactions for
    pub async fn get_session_transactions_with_conn<C>(
        &self,
        conn: &C,
        session_id: &str,
        currency: &str,
    ) -> Result<Vec<TransactionInfo>, CheckoutError>
    where
        C: sea_orm::ConnectionTrait,
    {
        use crate::entity::transactions::{ChainTxState, Entity as Transactions};

        let txs = Transactions::find()
            .filter(transactions::Column::SessionId.eq(session_id))
            .filter(transactions::Column::IsCredited.eq(true))
            .filter(transactions::Column::Status.eq(ChainTxState::Confirmed))
            .order_by_asc(transactions::Column::BlockTimestamp)
            .all(conn)
            .await
            .map_err(CheckoutError::Database)?;

        Ok(txs
            .into_iter()
            .map(|tx| TransactionInfo {
                tx_hash: tx.tx_hash,
                amount: crate::api::dtos::checkout::from_micro(tx.amount, currency),
                confirmations: tx.confirmations_count,
                from_address: tx.from_address,
                detected_at: tx.block_timestamp.timestamp(),
            })
            .collect())
    }
}
