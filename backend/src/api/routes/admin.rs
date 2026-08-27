//! Admin Portal API Routes
//!
//! Platform-wide management endpoints for the admin operator.
//! Auth: ADMIN_TOKEN via `admin_auth` middleware (no merchant context).
//! All queries are cross-merchant, with optional `?environment=` filter.

use axum::{
    extract::{Extension, Path, Query},
    routing::{get, patch, post},
    Json, Router,
};
use chrono::Utc;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::Deserialize;
use std::collections::HashMap;

use validator::Validate;

use crate::api::dtos::admin::{
    ActiveQuery, AddressPoolStats, AdminAddressResponse, AdminBillingLogResponse,
    AdminDashboardStats, AdminMerchantDetail, AdminPaymentEventResponse, AdminPayoutResponse,
    AdminSweepResponse, AdminSystemHealth, AdminTransactionResponse, AdminWithdrawalResponse,
    ChainWallet, IndexerProgress, KillQueryResponse, MemberInfo, MerchantSummary,
    PlatformWalletsResponse, ProfileSummary, TreasuryOverview, TreasuryTransaction,
    UpdateMerchantFeeRequest,
};
use crate::api::dtos::pagination::{PaginatedResponse, PaginationRequest};

use crate::api::error::AppError;
use crate::entity::{
    addresses, billing_logs, checkout_sessions, merchant_chain_accounts, merchants, org_members,
    outbound_transactions, payment_events, payment_exceptions, payouts, transactions, users,
    withdrawals, Addresses, BillingLogs, CheckoutSessions, IndexerState, MerchantChainAccounts,
    Merchants, OutboundTransactions, PaymentEvents, PaymentExceptions, Payouts, Transactions,
    Withdrawals,
};
use crate::services::billing::fee_config::FeeConfig;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        // Dashboard
        .route("/stats", get(get_dashboard_stats))
        // Merchants
        .route("/merchants", get(list_merchants))
        .route("/merchants/:id", get(get_merchant_detail))
        .route("/merchants/:id/fee", patch(update_merchant_fee))
        .route("/merchants/:id/sessions", get(list_merchant_sessions))
        .route("/merchants/:id/addresses", get(list_merchant_addresses))
        .route("/merchants/:id/billing", get(list_merchant_billing))
        // Global lists
        .route("/sessions", get(list_all_sessions))
        .route("/sessions/:id", get(get_session_detail))
        .route("/addresses", get(list_all_addresses))
        .route("/exceptions", get(list_all_exceptions))
        .route("/exceptions/:id", get(get_exception_detail))
        .route("/sweeps", get(list_all_sweeps))
        .route("/sweeps/:id", get(get_sweep_detail))
        .route("/withdrawals", get(list_all_withdrawals))
        .route("/withdrawals/:id", get(get_withdrawal_detail))
        .route("/payouts", get(list_all_payouts))
        .route("/payouts/:id", get(get_payout_detail))
        .route("/transactions", get(list_all_transactions))
        .route(
            "/transactions/:network/:tx_hash/:log_index",
            get(get_transaction_detail),
        )
        .route("/payment-events", get(list_all_payment_events))
        .route("/payment-events/:id", get(get_payment_event_detail))
        // Billing Logs
        .route("/billing-logs", get(list_all_billing_logs))
        .route("/billing-logs/:id", get(get_billing_log_detail))
        // System
        .route("/system/health", get(get_system_health))
        .route("/system/queries", get(get_active_queries))
        .route("/system/queries/:pid/kill", post(kill_query))
        // AML
        .route("/aml/blocked", get(list_aml_blocked))
        // Treasury
        .route("/treasury", get(get_treasury_overview))
        // Platform Wallets (gas sponsor + treasury per chain)
        .route("/platform-wallets", get(get_platform_wallets))
        // Agents
        .route("/agents", get(list_agents).post(create_agent))
        .route("/agents/:id", get(get_agent).patch(update_agent))
        .route("/agents/:id/commission", get(get_agent_commission))
        // Sub-Merchants
        .route(
            "/sub-merchants",
            post(create_sub_merchant).get(list_sub_merchants),
        )
        .route(
            "/sub-merchants/:id",
            get(get_sub_merchant).patch(update_sub_merchant),
        )
}

// ─── Query Params ───────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
pub struct EnvironmentFilter {
    #[serde(default)]
    pub environment: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct MerchantFilter {
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct SessionFilter {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub merchant_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ExceptionFilter {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub exception_type: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct SweepFilter {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub sweep_type: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct WithdrawalFilter {
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct PayoutFilter {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub merchant_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct TransactionFilter {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub network: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct PaymentEventFilter {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub event_type: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct AddressFilter {
    #[serde(default)]
    pub network: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub merchant_id: Option<String>,
}

// ─── Helper ─────────────────────────────────────────────────

/// Format USDT microunits (i64) into human-readable string (e.g. "123.456789")
fn format_usdt(microunits: i64) -> String {
    let whole = microunits / 1_000_000;
    let frac = (microunits % 1_000_000).unsigned_abs();
    format!("{}.{:06}", whole, frac)
}

/// Build address pool stats from a SeaORM query result
async fn count_address_pool(
    db: &DatabaseConnection,
    merchant_id: Option<&str>,
) -> Result<AddressPoolStats, AppError> {
    let mut base = Addresses::find();
    if let Some(mid) = merchant_id {
        base = base.filter(addresses::Column::MerchantId.eq(mid));
    }

    let all = base.clone().count(db).await? as u64;
    let idle = base
        .clone()
        .filter(addresses::Column::Status.eq("Idle"))
        .count(db)
        .await? as u64;
    let assigned = base
        .clone()
        .filter(addresses::Column::Status.eq("Assigned"))
        .count(db)
        .await? as u64;
    let detected = base
        .clone()
        .filter(addresses::Column::Status.eq("Detected"))
        .count(db)
        .await? as u64;
    let sweeping = base
        .clone()
        .filter(addresses::Column::Status.eq("Sweeping"))
        .count(db)
        .await? as u64;
    let cooling = base
        .clone()
        .filter(addresses::Column::Status.eq("Cooling"))
        .count(db)
        .await? as u64;
    let locked = base
        .clone()
        .filter(addresses::Column::Status.eq("Locked"))
        .count(db)
        .await? as u64;
    let error = base
        .clone()
        .filter(addresses::Column::Status.eq("Error"))
        .count(db)
        .await? as u64;

    Ok(AddressPoolStats {
        total: all,
        idle,
        assigned,
        detected,
        sweeping,
        cooling,
        locked,
        error,
    })
}

// ─── Dashboard ──────────────────────────────────────────────

/// GET /api/admin/stats
async fn get_dashboard_stats(
    Extension(state): Extension<AppState>,
) -> Result<Json<AdminDashboardStats>, AppError> {
    let db = &state.db;

    // Exclude sub-merchant backing orgs from merchant counts
    let total_merchants = Merchants::find()
        .filter(merchants::Column::MerchantType.ne(merchants::MerchantType::SubMerchant))
        .count(db)
        .await? as u64;
    let active_merchants = Merchants::find()
        .filter(merchants::Column::Status.eq("active"))
        .filter(merchants::Column::MerchantType.ne(merchants::MerchantType::SubMerchant))
        .count(db)
        .await? as u64;

    let active_sessions = CheckoutSessions::find()
        .filter(checkout_sessions::Column::Status.is_in(vec!["Pending", "Underpaid"]))
        .count(db)
        .await? as u64;

    // 24h volume: sum of amount_received for sessions created in last 24h
    let twenty_four_h_ago = Utc::now() - chrono::Duration::hours(24);
    let volume_24h: i64 = CheckoutSessions::find()
        .filter(checkout_sessions::Column::CreatedAt.gte(twenty_four_h_ago))
        .filter(checkout_sessions::Column::Status.is_in(vec!["Paid", "Overpaid"]))
        .select_only()
        .column_as(
            sea_orm::sea_query::Expr::col(checkout_sessions::Column::AmountReceived).sum(),
            "total",
        )
        .into_tuple::<Option<Decimal>>()
        .one(db)
        .await?
        .flatten()
        .and_then(|d| d.to_i64())
        .unwrap_or(0);

    // Global liability: sum of all chain account balances
    let global_liability: i64 = MerchantChainAccounts::find()
        .select_only()
        .column_as(
            sea_orm::sea_query::Expr::col(merchant_chain_accounts::Column::UsdtBalance).sum(),
            "total",
        )
        .into_tuple::<Option<Decimal>>()
        .one(db)
        .await?
        .flatten()
        .and_then(|d| d.to_i64())
        .unwrap_or(0);

    let pending_exceptions = PaymentExceptions::find()
        .filter(payment_exceptions::Column::Status.eq("Pending"))
        .count(db)
        .await? as u64;

    let pending_withdrawals = Withdrawals::find()
        .filter(withdrawals::Column::Status.eq("Pending"))
        .count(db)
        .await? as u64;

    let pending_payouts = Payouts::find()
        .filter(payouts::Column::Status.is_in(vec!["Pending", "Processing"]))
        .count(db)
        .await? as u64;

    // Treasury balance: fetch on-chain USDT balance (best-effort, non-blocking)
    // Read from AppState.treasury_address (HD-derived at startup), not config (env var)
    let treasury_address = state.treasury_address.clone();
    let (treasury_balance, treasury_addr_out) =
        match state.tron_client.get_usdt_balance(&treasury_address).await {
            Ok(balance) => (Some(format_usdt(balance)), Some(treasury_address)),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to fetch treasury balance");
                (None, Some(treasury_address))
            }
        };

    Ok(Json(AdminDashboardStats {
        total_merchants,
        active_merchants,
        active_sessions,
        total_volume_24h: format_usdt(volume_24h),
        global_liability: format_usdt(global_liability),
        pending_exceptions,
        pending_withdrawals,
        pending_payouts,
        treasury_balance,
        treasury_address: treasury_addr_out,
    }))
}

// ─── Merchants ──────────────────────────────────────────────

/// GET /api/admin/merchants
async fn list_merchants(
    Extension(state): Extension<AppState>,
    Query(pagination): Query<PaginationRequest>,
    Query(filter): Query<MerchantFilter>,
) -> Result<Json<PaginatedResponse<MerchantSummary>>, AppError> {
    pagination.validate()?;
    let db = &state.db;

    // Exclude sub-merchant backing orgs from admin merchant list
    let mut query = Merchants::find()
        .filter(merchants::Column::MerchantType.ne(merchants::MerchantType::SubMerchant));

    if let Some(ref status) = filter.status {
        query = query.filter(merchants::Column::Status.eq(status.as_str()));
    }

    if let Some(ref search) = pagination.search_text {
        query = query.filter(
            Condition::any()
                .add(merchants::Column::Name.contains(search))
                .add(merchants::Column::Id.contains(search)),
        );
    }

    let total = query.clone().count(db).await?;
    let offset = (pagination.page - 1) * pagination.page_size;
    let items = query
        .order_by_desc(merchants::Column::CreatedAt)
        .offset(offset)
        .limit(pagination.page_size)
        .all(db)
        .await?;

    // Collect merchant IDs for batch-loading owner users
    let merchant_ids: Vec<String> = items.iter().map(|m| m.id.clone()).collect();

    // Batch load owner memberships with user_ids
    let owner_memberships = org_members::Entity::find()
        .filter(org_members::Column::OrgId.is_in(merchant_ids.clone()))
        .filter(org_members::Column::Role.eq("owner"))
        .filter(org_members::Column::Status.eq("active"))
        .all(db)
        .await?;

    // Collect user_ids from memberships
    let user_ids: Vec<String> = owner_memberships
        .iter()
        .filter_map(|m| m.user_id.clone())
        .collect();

    // Batch load users
    let owner_users: Vec<users::Model> = if !user_ids.is_empty() {
        users::Entity::find()
            .filter(users::Column::Id.is_in(user_ids))
            .all(db)
            .await?
    } else {
        vec![]
    };

    // Batch count active members per org (only for current page)
    let member_counts: Vec<(String, i64)> = if !merchant_ids.is_empty() {
        org_members::Entity::find()
            .filter(org_members::Column::OrgId.is_in(merchant_ids.clone()))
            .filter(org_members::Column::Status.eq("active"))
            .select_only()
            .column(org_members::Column::OrgId)
            .column_as(org_members::Column::Id.count(), "count")
            .group_by(org_members::Column::OrgId)
            .into_tuple::<(String, i64)>()
            .all(db)
            .await?
    } else {
        vec![]
    };
    let member_count_map: HashMap<String, u64> = member_counts
        .into_iter()
        .map(|(org_id, count)| (org_id, count as u64))
        .collect();

    // Build lookup maps: org_id -> user_id -> user
    let org_to_user_id: std::collections::HashMap<String, String> = owner_memberships
        .into_iter()
        .filter_map(|m| m.user_id.map(|uid| (m.org_id, uid)))
        .collect();
    let user_map: std::collections::HashMap<String, &users::Model> =
        owner_users.iter().map(|u| (u.id.clone(), u)).collect();

    let data: Vec<MerchantSummary> = items
        .into_iter()
        .map(|m| {
            let owner_user = org_to_user_id
                .get(&m.id)
                .and_then(|uid| user_map.get(uid).copied());
            let member_count = member_count_map.get(&m.id).copied().unwrap_or(0);
            MerchantSummary {
                id: m.id.clone(),
                name: m.name,
                email: owner_user.map(|u| u.email.clone()).unwrap_or_default(),
                owner_name: owner_user.map(|u| u.name.clone()),
                status: format!("{:?}", m.status),
                is_totp_enabled: owner_user.map(|u| u.is_totp_enabled).unwrap_or(false),
                email_verified: owner_user.map(|u| u.email_verified).unwrap_or(false),
                member_count,
                created_at: m.created_at.to_rfc3339(),
            }
        })
        .collect();

    Ok(Json(PaginatedResponse::new(
        data,
        total,
        pagination.page,
        pagination.page_size,
    )))
}

/// GET /api/admin/merchants/:id
async fn get_merchant_detail(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AdminMerchantDetail>, AppError> {
    let db = &state.db;

    let merchant = Merchants::find_by_id(&id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Merchant '{}' not found", id)))?;

    let chain_accounts = MerchantChainAccounts::find()
        .filter(merchant_chain_accounts::Column::MerchantId.eq(&id))
        .all(db)
        .await?;

    let api_key_count = crate::entity::ApiKeys::find()
        .filter(crate::entity::api_keys::Column::MerchantId.eq(&id))
        .count(db)
        .await? as u64;

    let address_stats = count_address_pool(db, Some(&id)).await?;

    let total_sessions = CheckoutSessions::find()
        .filter(checkout_sessions::Column::MerchantId.eq(&id))
        .count(db)
        .await? as u64;

    let active_sessions = CheckoutSessions::find()
        .filter(checkout_sessions::Column::MerchantId.eq(&id))
        .filter(checkout_sessions::Column::Status.is_in(vec!["Pending", "Underpaid"]))
        .count(db)
        .await? as u64;

    // Load ALL org members for this merchant
    let all_members = org_members::Entity::find()
        .filter(org_members::Column::OrgId.eq(&id))
        .all(db)
        .await?;

    // Collect user IDs from members who have accepted (user_id is set)
    let member_user_ids: Vec<String> = all_members
        .iter()
        .filter_map(|m| m.user_id.clone())
        .collect();

    // Batch load member users
    let member_users: Vec<users::Model> = if !member_user_ids.is_empty() {
        users::Entity::find()
            .filter(users::Column::Id.is_in(member_user_ids))
            .all(db)
            .await?
    } else {
        vec![]
    };
    let member_user_map: HashMap<String, &users::Model> =
        member_users.iter().map(|u| (u.id.clone(), u)).collect();

    // Find owner user model from loaded users
    let owner_membership = all_members
        .iter()
        .find(|m| matches!(m.role, org_members::MemberRole::Owner));
    let owner_user_model = owner_membership
        .and_then(|m| m.user_id.as_ref())
        .and_then(|uid| member_user_map.get(uid).copied());

    // Active member count
    let active_member_count = all_members
        .iter()
        .filter(|m| matches!(m.status, org_members::MemberStatus::Active))
        .count() as u64;

    // Build members list
    let members: Vec<MemberInfo> = all_members
        .iter()
        .map(|m| {
            let user = m
                .user_id
                .as_ref()
                .and_then(|uid| member_user_map.get(uid).copied());
            MemberInfo {
                id: m.id.clone(),
                user_id: m.user_id.clone(),
                email: user
                    .map(|u| u.email.clone())
                    .or_else(|| m.invited_email.clone())
                    .unwrap_or_default(),
                name: user.map(|u| u.name.clone()),
                role: format!("{:?}", m.role),
                status: format!("{:?}", m.status),
                joined_at: m.accepted_at.map(|t| t.to_rfc3339()),
            }
        })
        .collect();

    // Resolve effective fee percentage for display
    let fee_config = FeeConfig::default();
    let effective_pct = merchant
        .custom_fee_percentage
        .map(|d| d.to_f64().unwrap_or(0.005) * 100.0)
        .unwrap_or_else(|| fee_config.fee_percentage.to_f64().unwrap_or(0.005) * 100.0);

    Ok(Json(AdminMerchantDetail {
        merchant: MerchantSummary {
            id: merchant.id,
            name: merchant.name,
            email: owner_user_model
                .map(|u| u.email.clone())
                .unwrap_or_default(),
            owner_name: owner_user_model.map(|u| u.name.clone()),
            status: format!("{:?}", merchant.status),
            is_totp_enabled: owner_user_model.map(|u| u.is_totp_enabled).unwrap_or(false),
            email_verified: owner_user_model.map(|u| u.email_verified).unwrap_or(false),
            member_count: active_member_count,
            created_at: merchant.created_at.to_rfc3339(),
        },
        custom_fee_percentage: merchant.custom_fee_percentage.map(|d| d.to_string()),
        effective_fee_percentage: format!("{:.2}", effective_pct),
        profiles: chain_accounts
            .into_iter()
            .map(|ca| ProfileSummary {
                environment: format!("{:?}", ca.environment),
                network: ca.network.as_str().to_string(),
                balance: format_usdt(ca.usdt_balance),
            })
            .collect(),
        api_key_count,
        address_stats,
        total_sessions,
        active_sessions,
        members,
    }))
}

/// PATCH /api/admin/merchants/:id/fee
///
/// Update a merchant's custom fee percentage.
/// Body: `{ "custom_fee_percentage": 0.005 }` (0.005 = 0.5%)
/// Set to `null` to revert to global default.
async fn update_merchant_fee(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateMerchantFeeRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let db = &state.db;
    // Validate range if set
    let custom_pct = match req.custom_fee_percentage {
        Some(pct) => {
            if !(0.0..=0.10).contains(&pct) {
                return Err(AppError::ValidationError {
                    code: crate::api::error::E_PARAMETER_INVALID,
                    message: "custom_fee_percentage must be between 0 and 0.10 (0% to 10%)".into(),
                    param: Some("custom_fee_percentage".into()),
                });
            }
            Some(
                Decimal::try_from(pct).map_err(|_| AppError::ValidationError {
                    code: crate::api::error::E_PARAMETER_INVALID,
                    message: "Invalid decimal value".into(),
                    param: Some("custom_fee_percentage".into()),
                })?,
            )
        }
        None => None,
    };

    // Find merchant
    let merchant = Merchants::find_by_id(&id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Merchant '{}' not found", id)))?;

    // Update
    let mut active: merchants::ActiveModel = merchant.into();
    active.custom_fee_percentage = sea_orm::Set(custom_pct);
    // Set fee_source: manual if custom, default if clearing
    active.fee_source = sea_orm::Set(if custom_pct.is_some() {
        merchants::FeeSource::Manual
    } else {
        merchants::FeeSource::Default
    });
    active.updated_at = sea_orm::Set(Utc::now().into());
    ActiveModelTrait::update(active, db).await?;

    let global_default_display = {
        let fee_config = FeeConfig::default();
        format!(
            "{:.1}%",
            fee_config.fee_percentage.to_f64().unwrap_or(0.005) * 100.0
        )
    };
    let display_pct = custom_pct
        .map(|d| format!("{}%", d * Decimal::from(100)))
        .unwrap_or_else(|| format!("default ({})", global_default_display));

    tracing::info!(
        merchant_id = %id,
        custom_fee_percentage = ?custom_pct,
        "Merchant fee updated to {}",
        display_pct
    );

    Ok(Json(serde_json::json!({
        "merchant_id": id,
        "custom_fee_percentage": custom_pct.map(|d: rust_decimal::Decimal| d.to_string()),
        "display": display_pct,
    })))
}
/// GET /api/admin/merchants/:id/sessions
async fn list_merchant_sessions(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
    Query(pagination): Query<PaginationRequest>,
    Query(filter): Query<SessionFilter>,
) -> Result<Json<PaginatedResponse<serde_json::Value>>, AppError> {
    pagination.validate()?;
    let db = &state.db;

    let mut query = CheckoutSessions::find().filter(checkout_sessions::Column::MerchantId.eq(&id));

    if let Some(ref status) = filter.status {
        query = query.filter(checkout_sessions::Column::Status.eq(status.as_str()));
    }

    let total = query.clone().count(db).await?;
    let offset = (pagination.page - 1) * pagination.page_size;
    let items = query
        .order_by_desc(checkout_sessions::Column::CreatedAt)
        .offset(offset)
        .limit(pagination.page_size)
        .all(db)
        .await?;

    let data: Vec<serde_json::Value> = items
        .into_iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id,
                "merchant_id": s.merchant_id,
                "network": s.network,
                "pay_address": s.pay_address,
                "currency": s.currency,
                "amount_expected": format_usdt(s.amount_expected),
                "amount_received": format_usdt(s.amount_received),
                "status": format!("{:?}", s.status),
                "settlement_status": format!("{:?}", s.settlement_status),
                "expires_at": s.expires_at.to_rfc3339(),
                "created_at": s.created_at.to_rfc3339(),
            })
        })
        .collect();

    Ok(Json(PaginatedResponse::new(
        data,
        total,
        pagination.page,
        pagination.page_size,
    )))
}

/// GET /api/admin/merchants/:id/addresses
async fn list_merchant_addresses(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AddressPoolStats>, AppError> {
    let stats = count_address_pool(&state.db, Some(&id)).await?;
    Ok(Json(stats))
}

/// GET /api/admin/merchants/:id/billing
async fn list_merchant_billing(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
    Query(pagination): Query<PaginationRequest>,
) -> Result<Json<PaginatedResponse<AdminBillingLogResponse>>, AppError> {
    pagination.validate()?;
    let db = &state.db;

    let query = BillingLogs::find().filter(billing_logs::Column::MerchantId.eq(&id));

    let total = query.clone().count(db).await?;
    let offset = (pagination.page - 1) * pagination.page_size;
    let items = query
        .order_by_desc(billing_logs::Column::CreatedAt)
        .offset(offset)
        .limit(pagination.page_size)
        .all(db)
        .await?;

    let data: Vec<AdminBillingLogResponse> = items
        .into_iter()
        .map(|b| AdminBillingLogResponse {
            id: b.id,
            environment: format!("{:?}", b.environment),
            network: b.network,
            merchant_id: b.merchant_id,
            session_id: b.session_id,
            external_ref_id: b.external_ref_id,
            billing_type: format!("{:?}", b.billing_type),
            previous_balance: format_usdt(b.previous_balance),
            amount_change: format_usdt(b.amount_change),
            balance_after: format_usdt(b.balance_after),
            description: b.description,
            created_at: b.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(PaginatedResponse::new(
        data,
        total,
        pagination.page,
        pagination.page_size,
    )))
}

// ─── Global Lists ───────────────────────────────────────────

/// GET /api/admin/sessions
async fn list_all_sessions(
    Extension(state): Extension<AppState>,
    Query(pagination): Query<PaginationRequest>,
    Query(filter): Query<SessionFilter>,
) -> Result<Json<PaginatedResponse<serde_json::Value>>, AppError> {
    pagination.validate()?;
    let db = &state.db;

    let mut query = CheckoutSessions::find();

    if let Some(ref status) = filter.status {
        query = query.filter(checkout_sessions::Column::Status.eq(status.as_str()));
    }
    if let Some(ref merchant_id) = filter.merchant_id {
        query = query.filter(checkout_sessions::Column::MerchantId.eq(merchant_id.as_str()));
    }
    if let Some(ref search) = pagination.search_text {
        query = query.filter(
            Condition::any()
                .add(checkout_sessions::Column::Id.contains(search))
                .add(checkout_sessions::Column::PayAddress.contains(search))
                .add(checkout_sessions::Column::MerchantId.contains(search)),
        );
    }

    let total = query.clone().count(db).await?;
    let offset = (pagination.page - 1) * pagination.page_size;
    let items = query
        .order_by_desc(checkout_sessions::Column::CreatedAt)
        .offset(offset)
        .limit(pagination.page_size)
        .all(db)
        .await?;

    let data: Vec<serde_json::Value> = items
        .into_iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id,
                "merchant_id": s.merchant_id,
                "network": s.network,
                "pay_address": s.pay_address,
                "currency": s.currency,
                "amount_expected": format_usdt(s.amount_expected),
                "amount_received": format_usdt(s.amount_received),
                "status": format!("{:?}", s.status),
                "settlement_status": format!("{:?}", s.settlement_status),
                "expires_at": s.expires_at.to_rfc3339(),
                "created_at": s.created_at.to_rfc3339(),
            })
        })
        .collect();

    Ok(Json(PaginatedResponse::new(
        data,
        total,
        pagination.page,
        pagination.page_size,
    )))
}

/// GET /api/admin/exceptions
async fn list_all_exceptions(
    Extension(state): Extension<AppState>,
    Query(pagination): Query<PaginationRequest>,
    Query(filter): Query<ExceptionFilter>,
) -> Result<Json<PaginatedResponse<serde_json::Value>>, AppError> {
    pagination.validate()?;
    let db = &state.db;

    let mut query = PaymentExceptions::find();

    if let Some(ref status) = filter.status {
        query = query.filter(payment_exceptions::Column::Status.eq(status.as_str()));
    }
    if let Some(ref et) = filter.exception_type {
        query = query.filter(payment_exceptions::Column::ExceptionType.eq(et.as_str()));
    }
    if let Some(ref search) = pagination.search_text {
        query = query.filter(
            Condition::any()
                .add(payment_exceptions::Column::Id.contains(search))
                .add(payment_exceptions::Column::TxHash.contains(search))
                .add(payment_exceptions::Column::ToAddress.contains(search)),
        );
    }

    let total = query.clone().count(db).await?;
    let offset = (pagination.page - 1) * pagination.page_size;
    let items = query
        .order_by_desc(payment_exceptions::Column::CreatedAt)
        .offset(offset)
        .limit(pagination.page_size)
        .all(db)
        .await?;

    let data: Vec<serde_json::Value> = items
        .into_iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "network": e.network,
                "tx_hash": e.tx_hash,
                "exception_type": format!("{:?}", e.exception_type),
                "to_address": e.to_address,
                "from_address": e.from_address,
                "amount": format_usdt(e.amount),
                "merchant_id": e.merchant_id,
                "session_id": e.session_id,
                "status": format!("{:?}", e.status),
                "resolution": e.resolution.map(|r| format!("{:?}", r)),
                "created_at": e.created_at.to_rfc3339(),
            })
        })
        .collect();

    Ok(Json(PaginatedResponse::new(
        data,
        total,
        pagination.page,
        pagination.page_size,
    )))
}

/// GET /api/admin/sweeps
async fn list_all_sweeps(
    Extension(state): Extension<AppState>,
    Query(pagination): Query<PaginationRequest>,
    Query(filter): Query<SweepFilter>,
) -> Result<Json<PaginatedResponse<AdminSweepResponse>>, AppError> {
    pagination.validate()?;
    let db = &state.db;

    let mut query = OutboundTransactions::find()
        .filter(
            outbound_transactions::Column::Purpose
                .eq(outbound_transactions::OutboundPurpose::TokenTransfer),
        )
        .filter(outbound_transactions::Column::OperationType.is_in([
            outbound_transactions::OutboundOperationType::AutoSweep,
            outbound_transactions::OutboundOperationType::ManualSweep,
            outbound_transactions::OutboundOperationType::ManualTransfer,
        ]));

    if let Some(ref status) = filter.status {
        query = query.filter(outbound_transactions::Column::State.eq(status.as_str()));
    }
    if let Some(ref st) = filter.sweep_type {
        query = query.filter(outbound_transactions::Column::OperationType.eq(st.as_str()));
    }

    let total = query.clone().count(db).await?;
    let offset = (pagination.page - 1) * pagination.page_size;
    let items = query
        .order_by_desc(outbound_transactions::Column::CreatedAt)
        .offset(offset)
        .limit(pagination.page_size)
        .all(db)
        .await?;

    let data: Vec<AdminSweepResponse> = items
        .into_iter()
        .map(|s| AdminSweepResponse {
            id: s.id,
            merchant_id: s.merchant_id,
            session_id: s.session_id,
            sweep_type: format!("{:?}", s.operation_type),
            network: s.network,
            from_address: s.from_address,
            to_address: s.to_address,
            tx_hash: s.tx_hash,
            amount: format_usdt(s.amount),
            cost_in_usdt: s.cost_in_usdt.map(format_usdt),
            status: format!("{:?}", s.state),
            error_message: s.error_message,
            created_at: s.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(PaginatedResponse::new(
        data,
        total,
        pagination.page,
        pagination.page_size,
    )))
}

/// GET /api/admin/withdrawals
async fn list_all_withdrawals(
    Extension(state): Extension<AppState>,
    Query(pagination): Query<PaginationRequest>,
    Query(filter): Query<WithdrawalFilter>,
) -> Result<Json<PaginatedResponse<AdminWithdrawalResponse>>, AppError> {
    pagination.validate()?;
    let db = &state.db;

    let mut query = Withdrawals::find();

    if let Some(ref status) = filter.status {
        query = query.filter(withdrawals::Column::Status.eq(status.as_str()));
    }

    let total = query.clone().count(db).await?;
    let offset = (pagination.page - 1) * pagination.page_size;
    let items = query
        .order_by_desc(withdrawals::Column::CreatedAt)
        .offset(offset)
        .limit(pagination.page_size)
        .all(db)
        .await?;

    let data: Vec<AdminWithdrawalResponse> = items
        .into_iter()
        .map(|w| AdminWithdrawalResponse {
            id: w.id,
            merchant_id: w.merchant_id,
            environment: format!("{:?}", w.environment),
            network: w.network,
            amount: format_usdt(w.amount),
            network_fee: format_usdt(w.network_fee),
            net_amount: format_usdt(w.net_amount),
            to_address: w.to_address,
            status: format!("{:?}", w.status),
            tx_hash: w.tx_hash,
            error_reason: w.error_reason,
            currency: w.currency,
            created_at: w.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(PaginatedResponse::new(
        data,
        total,
        pagination.page,
        pagination.page_size,
    )))
}

// ─── Payouts ────────────────────────────────────────────────

/// GET /api/admin/payouts
async fn list_all_payouts(
    Extension(state): Extension<AppState>,
    Query(pagination): Query<PaginationRequest>,
    Query(filter): Query<PayoutFilter>,
) -> Result<Json<PaginatedResponse<AdminPayoutResponse>>, AppError> {
    pagination.validate()?;
    let db = &state.db;

    let mut query = Payouts::find();

    if let Some(ref status) = filter.status {
        query = query.filter(payouts::Column::Status.eq(status.as_str()));
    }
    if let Some(ref merchant_id) = filter.merchant_id {
        query = query.filter(payouts::Column::MerchantId.eq(merchant_id.as_str()));
    }
    if let Some(ref search) = pagination.search_text {
        query = query.filter(
            sea_orm::Condition::any()
                .add(payouts::Column::Id.contains(search))
                .add(payouts::Column::ToAddress.contains(search))
                .add(payouts::Column::TxHash.contains(search))
                .add(payouts::Column::MerchantId.contains(search)),
        );
    }

    let total = query.clone().count(db).await?;
    let offset = (pagination.page - 1) * pagination.page_size;
    let items = query
        .order_by_desc(payouts::Column::CreatedAt)
        .offset(offset)
        .limit(pagination.page_size)
        .all(db)
        .await?;

    let data: Vec<AdminPayoutResponse> = items
        .into_iter()
        .map(|p| AdminPayoutResponse {
            id: p.id,
            merchant_id: p.merchant_id,
            environment: format!("{:?}", p.environment),
            network: p.network,
            amount: format_usdt(p.amount),
            fee: format_usdt(p.fee),
            net_amount: format_usdt(p.net_amount),
            to_address: p.to_address,
            status: format!("{:?}", p.status),
            tx_hash: p.tx_hash,
            description: p.description,
            error_reason: p.error_reason,
            currency: p.currency,
            created_at: p.created_at.to_rfc3339(),
            completed_at: p.completed_at.map(|t| t.to_rfc3339()),
        })
        .collect();

    Ok(Json(PaginatedResponse::new(
        data,
        total,
        pagination.page,
        pagination.page_size,
    )))
}

/// GET /api/admin/payouts/:id
async fn get_payout_detail(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AdminPayoutResponse>, AppError> {
    let p = Payouts::find_by_id(&id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Payout '{}' not found", id)))?;

    Ok(Json(AdminPayoutResponse {
        id: p.id,
        merchant_id: p.merchant_id,
        environment: format!("{:?}", p.environment),
        network: p.network,
        amount: format_usdt(p.amount),
        fee: format_usdt(p.fee),
        net_amount: format_usdt(p.net_amount),
        to_address: p.to_address,
        status: format!("{:?}", p.status),
        tx_hash: p.tx_hash,
        description: p.description,
        error_reason: p.error_reason,
        currency: p.currency,
        created_at: p.created_at.to_rfc3339(),
        completed_at: p.completed_at.map(|t| t.to_rfc3339()),
    }))
}

// ─── System Health ──────────────────────────────────────────

/// GET /api/admin/system/health
async fn get_system_health(
    Extension(state): Extension<AppState>,
) -> Result<Json<AdminSystemHealth>, AppError> {
    let db = &state.db;

    // Database health
    let db_ok = db.ping().await.is_ok();

    // Per-chain RPC health — ping all chains concurrently (avoid N×timeout)
    let rpc_futures: Vec<_> = state
        .chain_clients
        .iter()
        .map(|(network, client)| {
            let name = network.as_str().to_string();
            let client = client.clone();
            async move { (name, client.get_current_block().await.is_ok()) }
        })
        .collect();
    let chain_rpc: HashMap<String, bool> = futures::future::join_all(rpc_futures)
        .await
        .into_iter()
        .collect();

    // Indexer states — chain_head_block is persisted by indexer, no extra RPC needed
    let indexer_states = IndexerState::find().all(db).await?;
    let indexer: Vec<IndexerProgress> = indexer_states
        .into_iter()
        .map(|s| {
            let behind = s
                .chain_head_block
                .map(|head| (head - s.last_processed_block).max(0));

            // Look up RPC status from chain_clients (EVM only)
            let rpc_status = crate::entity::Network::from_str_lenient(&s.network)
                .and_then(|net| state.chain_clients.get(&net))
                .and_then(
                    |client: &std::sync::Arc<dyn crate::services::chain::traits::ChainClient>| {
                        client.rpc_status()
                    },
                );

            IndexerProgress {
                network: s.network,
                last_processed_block: s.last_processed_block,
                chain_head_block: s.chain_head_block,
                blocks_behind: behind,
                updated_at: s.updated_at.to_rfc3339(),
                active_rpc: rpc_status.as_ref().map(|r| r.provider.clone()),
                is_fallback: rpc_status.as_ref().map(|r| r.is_fallback),
                active_endpoint: rpc_status.as_ref().map(|r| r.active_endpoint.clone()),
            }
        })
        .collect();

    // Address pool split by network
    let all_addresses = Addresses::find().all(db).await?;
    let mut pool_map: HashMap<String, [u64; 8]> = HashMap::new();
    for a in &all_addresses {
        let entry = pool_map.entry(a.network.clone()).or_insert([0; 8]);
        entry[0] += 1; // total
        match format!("{:?}", a.status).as_str() {
            "Idle" => entry[1] += 1,
            "Assigned" => entry[2] += 1,
            "Detected" => entry[3] += 1,
            "Sweeping" => entry[4] += 1,
            "Cooling" => entry[5] += 1,
            "Locked" => entry[6] += 1,
            "Error" => entry[7] += 1,
            _ => {}
        }
    }
    let address_pool: HashMap<String, AddressPoolStats> = pool_map
        .into_iter()
        .map(|(network, c)| {
            (
                network,
                AddressPoolStats {
                    total: c[0],
                    idle: c[1],
                    assigned: c[2],
                    detected: c[3],
                    sweeping: c[4],
                    cooling: c[5],
                    locked: c[6],
                    error: c[7],
                },
            )
        })
        .collect();

    // Service heartbeat statuses
    let services: HashMap<String, String> = state
        .service_health
        .all_statuses()
        .into_iter()
        .map(|(name, status)| (name, status.to_string()))
        .collect();

    Ok(Json(AdminSystemHealth {
        database: db_ok,
        chain_rpc,
        indexer,
        address_pool,
        services,
    }))
}

// ─── Addresses ──────────────────────────────────────────────

/// GET /api/admin/addresses
async fn list_all_addresses(
    Extension(state): Extension<AppState>,
    Query(pagination): Query<PaginationRequest>,
    Query(filter): Query<AddressFilter>,
) -> Result<Json<PaginatedResponse<AdminAddressResponse>>, AppError> {
    pagination.validate()?;
    let db = &state.db;

    let mut query = Addresses::find();

    if let Some(ref network) = filter.network {
        query = query.filter(addresses::Column::Network.eq(network.as_str()));
    }
    if let Some(ref status) = filter.status {
        query = query.filter(addresses::Column::Status.eq(status.as_str()));
    }
    if let Some(ref merchant_id) = filter.merchant_id {
        query = query.filter(addresses::Column::MerchantId.eq(merchant_id.as_str()));
    }
    if let Some(ref search) = pagination.search_text {
        query = query.filter(
            Condition::any()
                .add(addresses::Column::Address.contains(search))
                .add(addresses::Column::MerchantId.contains(search)),
        );
    }

    let total = query.clone().count(db).await?;
    let offset = (pagination.page - 1) * pagination.page_size;
    let items = query
        .order_by_desc(addresses::Column::UpdatedAt)
        .offset(offset)
        .limit(pagination.page_size)
        .all(db)
        .await?;

    let data: Vec<AdminAddressResponse> = items
        .into_iter()
        .map(|a| AdminAddressResponse {
            network: a.network,
            address: a.address,
            merchant_id: a.merchant_id,
            status: format!("{:?}", a.status),
            usdt_balance: format_usdt(a.usdt_balance),
            usdc_balance: format_usdt(a.usdc_balance),
            native_balance: format_usdt(a.native_balance),
            sweep_attempts: a.sweep_attempts,
            error_reason: a.error_reason,
            created_at: a.created_at.to_rfc3339(),
            updated_at: a.updated_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(PaginatedResponse::new(
        data,
        total,
        pagination.page,
        pagination.page_size,
    )))
}

// ─── AML ────────────────────────────────────────────────────

/// GET /api/admin/aml/blocked
async fn list_aml_blocked(
    Extension(state): Extension<AppState>,
    Query(pagination): Query<PaginationRequest>,
) -> Result<Json<PaginatedResponse<serde_json::Value>>, AppError> {
    pagination.validate()?;
    let db = &state.db;

    let query = PaymentExceptions::find()
        .filter(payment_exceptions::Column::ExceptionType.eq("risk_blocked"));

    let total = query.clone().count(db).await?;
    let offset = (pagination.page - 1) * pagination.page_size;
    let items = query
        .order_by_desc(payment_exceptions::Column::CreatedAt)
        .offset(offset)
        .limit(pagination.page_size)
        .all(db)
        .await?;

    let data: Vec<serde_json::Value> = items
        .into_iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "tx_hash": e.tx_hash,
                "from_address": e.from_address,
                "to_address": e.to_address,
                "amount": format_usdt(e.amount),
                "merchant_id": e.merchant_id,
                "status": format!("{:?}", e.status),
                "created_at": e.created_at.to_rfc3339(),
            })
        })
        .collect();

    Ok(Json(PaginatedResponse::new(
        data,
        total,
        pagination.page,
        pagination.page_size,
    )))
}

// ─── Active Queries Monitor ─────────────────────────────────

/// GET /api/admin/system/queries
///
/// Lists active database queries from pg_stat_activity.
/// Excludes idle connections and the monitoring query itself.
async fn get_active_queries(
    Extension(state): Extension<AppState>,
) -> Result<Json<Vec<ActiveQuery>>, AppError> {
    let db = &state.db;

    let sql = r#"
        SELECT
            pid,
            EXTRACT(EPOCH FROM (now() - query_start))::float8 AS duration_secs,
            state,
            query,
            client_addr::text,
            application_name,
            wait_event_type
        FROM pg_stat_activity
        WHERE state != 'idle'
          AND pid != pg_backend_pid()
          AND query NOT ILIKE '%pg_stat_activity%'
        ORDER BY duration_secs DESC NULLS LAST
        LIMIT 50
    "#;

    let stmt = sea_orm::Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);
    let rows = db.query_all(stmt).await.map_err(|e| {
        AppError::InternalServerError(anyhow::anyhow!("Failed to query pg_stat_activity: {}", e))
    })?;

    let mut queries = Vec::new();
    for row in rows {
        let duration_secs: f64 = row.try_get_by_index::<f64>(1).unwrap_or(0.0);

        // Format duration as human-readable
        let duration_display = if duration_secs < 1.0 {
            format!("{:.0}ms", duration_secs * 1000.0)
        } else if duration_secs < 60.0 {
            format!("{:.1}s", duration_secs)
        } else if duration_secs < 3600.0 {
            format!("{:.0}m {:.0}s", duration_secs / 60.0, duration_secs % 60.0)
        } else {
            format!(
                "{:.0}h {:.0}m",
                duration_secs / 3600.0,
                (duration_secs % 3600.0) / 60.0
            )
        };

        queries.push(ActiveQuery {
            pid: row.try_get_by_index::<i32>(0).unwrap_or(0),
            duration_seconds: duration_secs,
            duration_display,
            state: row.try_get_by_index::<String>(2).unwrap_or_default(),
            query: row.try_get_by_index::<String>(3).unwrap_or_default(),
            client_addr: row.try_get_by_index::<Option<String>>(4).unwrap_or(None),
            application_name: row.try_get_by_index::<String>(5).unwrap_or_default(),
            wait_event_type: row.try_get_by_index::<Option<String>>(6).unwrap_or(None),
        });
    }

    Ok(Json(queries))
}

/// POST /api/admin/system/queries/:pid/kill
///
/// Terminates a database backend process by PID.
async fn kill_query(
    Extension(state): Extension<AppState>,
    Path(pid): Path<i32>,
) -> Result<Json<KillQueryResponse>, AppError> {
    let db = &state.db;

    let sql = format!("SELECT pg_terminate_backend({})", pid);
    let stmt = sea_orm::Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);

    let result: Option<sea_orm::QueryResult> = db.query_one(stmt).await.map_err(|e| {
        AppError::InternalServerError(anyhow::anyhow!(
            "Failed to terminate backend {}: {}",
            pid,
            e
        ))
    })?;

    let terminated = match result {
        Some(row) => row.try_get_by_index::<bool>(0).unwrap_or(false),
        None => false,
    };

    Ok(Json(KillQueryResponse { pid, terminated }))
}

// ─── Detail Endpoints ───────────────────────────────────────

/// GET /api/admin/sessions/:id
async fn get_session_detail(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let db = &state.db;
    let session = CheckoutSessions::find_by_id(&id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Session '{}' not found", id)))?;

    // Look up the associated billing_log for balance context
    let billing_log = BillingLogs::find()
        .filter(billing_logs::Column::SessionId.eq(&id))
        .one(db)
        .await?;

    let mut json = serde_json::json!({
        "id": session.id,
        "merchant_id": session.merchant_id,
        "network": session.network,
        "pay_address": session.pay_address,
        "client_reference_id": session.client_reference_id,
        "currency": session.currency,
        "currency_contract": session.currency_contract,
        "amount_expected": format_usdt(session.amount_expected),
        "amount_received": format_usdt(session.amount_received),
        "status": format!("{:?}", session.status),
        "settlement_status": format!("{:?}", session.settlement_status),
        "settlement_tx_hash": session.settlement_tx_hash,
        "fee_amount": session.fee_amount.map(format_usdt),
        "net_amount": session.net_amount.map(format_usdt),
        "success_url": session.success_url,
        "cancel_url": session.cancel_url,
        "expires_at": session.expires_at.to_rfc3339(),
        "created_at": session.created_at.to_rfc3339(),
        "updated_at": session.updated_at.to_rfc3339(),
    });

    // Append billing balance context if a billing_log exists
    if let Some(bl) = billing_log {
        let obj = json.as_object_mut().unwrap();
        obj.insert("billing_log_id".into(), serde_json::Value::String(bl.id));
        obj.insert(
            "balance_before".into(),
            serde_json::Value::String(format_usdt(bl.previous_balance)),
        );
        obj.insert(
            "balance_after".into(),
            serde_json::Value::String(format_usdt(bl.balance_after)),
        );
    }

    Ok(Json(json))
}

/// GET /api/admin/exceptions/:id
async fn get_exception_detail(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let db = &state.db;
    let e = PaymentExceptions::find_by_id(&id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Exception '{}' not found", id)))?;

    Ok(Json(serde_json::json!({
        "id": e.id,
        "network": e.network,
        "tx_hash": e.tx_hash,
        "log_index": e.log_index,
        "exception_type": format!("{:?}", e.exception_type),
        "to_address": e.to_address,
        "from_address": e.from_address,
        "amount": format_usdt(e.amount),
        "currency_symbol": e.currency_symbol,
        "merchant_id": e.merchant_id,
        "session_id": e.session_id,
        "block_number": e.block_number,
        "block_timestamp": e.block_timestamp.to_rfc3339(),
        "status": format!("{:?}", e.status),
        "resolution": e.resolution.map(|r| format!("{:?}", r)),
        "resolution_ref_id": e.resolution_ref_id,
        "resolved_at": e.resolved_at.map(|d| d.to_rfc3339()),
        "resolved_by": e.resolved_by,
        "notes": e.notes,
        "created_at": e.created_at.to_rfc3339(),
        "updated_at": e.updated_at.to_rfc3339(),
    })))
}

/// GET /api/admin/sweeps/:id
async fn get_sweep_detail(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AdminSweepResponse>, AppError> {
    let db = &state.db;
    let s = OutboundTransactions::find_by_id(&id)
        .filter(
            outbound_transactions::Column::Purpose
                .eq(outbound_transactions::OutboundPurpose::TokenTransfer),
        )
        .filter(outbound_transactions::Column::OperationType.is_in([
            outbound_transactions::OutboundOperationType::AutoSweep,
            outbound_transactions::OutboundOperationType::ManualSweep,
            outbound_transactions::OutboundOperationType::ManualTransfer,
        ]))
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Sweep '{}' not found", id)))?;

    Ok(Json(AdminSweepResponse {
        id: s.id,
        merchant_id: s.merchant_id,
        session_id: s.session_id,
        sweep_type: format!("{:?}", s.operation_type),
        network: s.network,
        from_address: s.from_address,
        to_address: s.to_address,
        tx_hash: s.tx_hash,
        amount: format_usdt(s.amount),
        cost_in_usdt: s.cost_in_usdt.map(format_usdt),
        status: format!("{:?}", s.state),
        error_message: s.error_message,
        created_at: s.created_at.to_rfc3339(),
    }))
}

/// GET /api/admin/withdrawals/:id
async fn get_withdrawal_detail(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AdminWithdrawalResponse>, AppError> {
    let db = &state.db;
    let w = Withdrawals::find_by_id(&id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Withdrawal '{}' not found", id)))?;

    Ok(Json(AdminWithdrawalResponse {
        id: w.id,
        merchant_id: w.merchant_id,
        environment: format!("{:?}", w.environment),
        network: w.network,
        amount: format_usdt(w.amount),
        network_fee: format_usdt(w.network_fee),
        net_amount: format_usdt(w.net_amount),
        to_address: w.to_address,
        status: format!("{:?}", w.status),
        tx_hash: w.tx_hash,
        error_reason: w.error_reason,
        currency: w.currency,
        created_at: w.created_at.to_rfc3339(),
    }))
}

// ─── Transactions ───────────────────────────────────────────

/// GET /api/admin/transactions
async fn list_all_transactions(
    Extension(state): Extension<AppState>,
    Query(pagination): Query<PaginationRequest>,
    Query(filter): Query<TransactionFilter>,
) -> Result<Json<PaginatedResponse<AdminTransactionResponse>>, AppError> {
    pagination.validate()?;
    let db = &state.db;

    let mut query = Transactions::find();

    if let Some(ref status) = filter.status {
        query = query.filter(transactions::Column::Status.eq(status.as_str()));
    }
    if let Some(ref network) = filter.network {
        query = query.filter(transactions::Column::Network.eq(network.as_str()));
    }
    if let Some(ref session_id) = filter.session_id {
        query = query.filter(transactions::Column::SessionId.eq(session_id.as_str()));
    }
    if let Some(ref search) = pagination.search_text {
        query = query.filter(
            Condition::any()
                .add(transactions::Column::TxHash.contains(search))
                .add(transactions::Column::FromAddress.contains(search))
                .add(transactions::Column::ToAddress.contains(search))
                .add(transactions::Column::MerchantId.contains(search)),
        );
    }

    let total = query.clone().count(db).await?;
    let offset = (pagination.page - 1) * pagination.page_size;
    let items = query
        .order_by_desc(transactions::Column::CreatedAt)
        .offset(offset)
        .limit(pagination.page_size)
        .all(db)
        .await?;

    let data: Vec<AdminTransactionResponse> = items
        .into_iter()
        .map(|t| AdminTransactionResponse {
            network: t.network,
            tx_hash: t.tx_hash,
            log_index: t.log_index,
            session_id: t.session_id,
            merchant_id: t.merchant_id,
            currency_symbol: t.currency_symbol,
            from_address: t.from_address,
            to_address: t.to_address,
            amount: format_usdt(t.amount),
            status: format!("{:?}", t.status),
            confirmations_count: t.confirmations_count,
            block_number: t.block_number,
            block_timestamp: t.block_timestamp.to_rfc3339(),
            is_credited: t.is_credited,
            created_at: t.created_at.to_rfc3339(),
            updated_at: t.updated_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(PaginatedResponse::new(
        data,
        total,
        pagination.page,
        pagination.page_size,
    )))
}

/// GET /api/admin/transactions/:network/:tx_hash/:log_index
async fn get_transaction_detail(
    Extension(state): Extension<AppState>,
    Path((network, tx_hash, log_index)): Path<(String, String, i32)>,
) -> Result<Json<AdminTransactionResponse>, AppError> {
    let db = &state.db;
    let t = Transactions::find()
        .filter(transactions::Column::Network.eq(&network))
        .filter(transactions::Column::TxHash.eq(&tx_hash))
        .filter(transactions::Column::LogIndex.eq(log_index))
        .one(db)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Transaction '{}/{}/{}' not found",
                network, tx_hash, log_index
            ))
        })?;

    Ok(Json(AdminTransactionResponse {
        network: t.network,
        tx_hash: t.tx_hash,
        log_index: t.log_index,
        session_id: t.session_id,
        merchant_id: t.merchant_id,
        currency_symbol: t.currency_symbol,
        from_address: t.from_address,
        to_address: t.to_address,
        amount: format_usdt(t.amount),
        status: format!("{:?}", t.status),
        confirmations_count: t.confirmations_count,
        block_number: t.block_number,
        block_timestamp: t.block_timestamp.to_rfc3339(),
        is_credited: t.is_credited,
        created_at: t.created_at.to_rfc3339(),
        updated_at: t.updated_at.to_rfc3339(),
    }))
}

// ─── Payment Events ─────────────────────────────────────────

/// GET /api/admin/payment-events
async fn list_all_payment_events(
    Extension(state): Extension<AppState>,
    Query(pagination): Query<PaginationRequest>,
    Query(filter): Query<PaymentEventFilter>,
) -> Result<Json<PaginatedResponse<AdminPaymentEventResponse>>, AppError> {
    pagination.validate()?;
    let db = &state.db;

    let mut query = PaymentEvents::find();

    if let Some(ref status) = filter.status {
        query = query.filter(payment_events::Column::Status.eq(status.as_str()));
    }
    if let Some(ref et) = filter.event_type {
        query = query.filter(payment_events::Column::EventType.eq(et.as_str()));
    }
    if let Some(ref search) = pagination.search_text {
        query = query.filter(
            Condition::any()
                .add(payment_events::Column::Id.contains(search))
                .add(payment_events::Column::TxHash.contains(search))
                .add(payment_events::Column::SessionId.contains(search)),
        );
    }

    let total = query.clone().count(db).await?;
    let offset = (pagination.page - 1) * pagination.page_size;
    let items = query
        .order_by_desc(payment_events::Column::CreatedAt)
        .offset(offset)
        .limit(pagination.page_size)
        .all(db)
        .await?;

    let data: Vec<AdminPaymentEventResponse> = items
        .into_iter()
        .map(|e| AdminPaymentEventResponse {
            id: e.id,
            event_type: format!("{:?}", e.event_type),
            session_id: e.session_id,
            tx_network: e.tx_network,
            tx_hash: e.tx_hash,
            tx_log_index: e.tx_log_index,
            amount: format_usdt(e.amount),
            status: format!("{:?}", e.status),
            attempt_count: e.attempt_count,
            error_message: e.error_message,
            created_at: e.created_at.to_rfc3339(),
            updated_at: e.updated_at.to_rfc3339(),
            processed_at: e.processed_at.map(|d| d.to_rfc3339()),
        })
        .collect();

    Ok(Json(PaginatedResponse::new(
        data,
        total,
        pagination.page,
        pagination.page_size,
    )))
}

/// GET /api/admin/payment-events/:id
async fn get_payment_event_detail(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AdminPaymentEventResponse>, AppError> {
    let db = &state.db;
    let e = PaymentEvents::find_by_id(&id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Payment event '{}' not found", id)))?;

    Ok(Json(AdminPaymentEventResponse {
        id: e.id,
        event_type: format!("{:?}", e.event_type),
        session_id: e.session_id,
        tx_network: e.tx_network,
        tx_hash: e.tx_hash,
        tx_log_index: e.tx_log_index,
        amount: format_usdt(e.amount),
        status: format!("{:?}", e.status),
        attempt_count: e.attempt_count,
        error_message: e.error_message,
        created_at: e.created_at.to_rfc3339(),
        updated_at: e.updated_at.to_rfc3339(),
        processed_at: e.processed_at.map(|d| d.to_rfc3339()),
    }))
}

// ─── Billing Logs (Global) ──────────────────────────────────────

/// GET /api/admin/billing-logs
async fn list_all_billing_logs(
    Extension(state): Extension<AppState>,
    Query(pagination): Query<PaginationRequest>,
    Query(env_filter): Query<EnvironmentFilter>,
) -> Result<Json<PaginatedResponse<AdminBillingLogResponse>>, AppError> {
    pagination.validate()?;
    let db = &state.db;

    let mut condition = Condition::all();
    if let Some(ref env) = env_filter.environment {
        condition = condition.add(billing_logs::Column::Environment.eq(env.as_str()));
    }

    let query = BillingLogs::find().filter(condition);
    let total = query.clone().count(db).await?;
    let offset = (pagination.page - 1) * pagination.page_size;
    let items = query
        .order_by_desc(billing_logs::Column::CreatedAt)
        .offset(offset)
        .limit(pagination.page_size)
        .all(db)
        .await?;

    let data: Vec<AdminBillingLogResponse> = items
        .into_iter()
        .map(|b| AdminBillingLogResponse {
            id: b.id,
            environment: format!("{:?}", b.environment),
            network: b.network,
            merchant_id: b.merchant_id,
            session_id: b.session_id,
            external_ref_id: b.external_ref_id,
            billing_type: format!("{:?}", b.billing_type),
            previous_balance: format_usdt(b.previous_balance),
            amount_change: format_usdt(b.amount_change),
            balance_after: format_usdt(b.balance_after),
            description: b.description,
            created_at: b.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(PaginatedResponse::new(
        data,
        total,
        pagination.page,
        pagination.page_size,
    )))
}

/// GET /api/admin/billing-logs/:id
async fn get_billing_log_detail(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AdminBillingLogResponse>, AppError> {
    let db = &state.db;
    let b = BillingLogs::find_by_id(&id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Billing log '{}' not found", id)))?;

    Ok(Json(AdminBillingLogResponse {
        id: b.id,
        environment: format!("{:?}", b.environment),
        network: b.network,
        merchant_id: b.merchant_id,
        session_id: b.session_id,
        external_ref_id: b.external_ref_id,
        billing_type: format!("{:?}", b.billing_type),
        previous_balance: format_usdt(b.previous_balance),
        amount_change: format_usdt(b.amount_change),
        balance_after: format_usdt(b.balance_after),
        description: b.description,
        created_at: b.created_at.to_rfc3339(),
    }))
}

// ─── Treasury Overview ──────────────────────────────────────────

/// GET /api/admin/treasury
/// Returns full treasury overview: balance, reconciliation, alerting, and history.
async fn get_treasury_overview(
    Extension(state): Extension<AppState>,
) -> Result<Json<TreasuryOverview>, AppError> {
    let db = &state.db;
    let treasury_address = state.treasury_address.clone();
    let threshold = state.config.treasury_low_balance_threshold;

    // 1. On-chain USDT balance (best-effort)
    let balance_raw: Option<i64> = match state.tron_client.get_usdt_balance(&treasury_address).await
    {
        Ok(b) => Some(b),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to fetch treasury balance");
            None
        }
    };

    // 2. Total swept in: sum of confirmed sweep amounts going TO treasury
    let total_swept_in: i64 = OutboundTransactions::find()
        .filter(outbound_transactions::Column::State.eq("Confirmed"))
        .filter(
            outbound_transactions::Column::Purpose
                .eq(outbound_transactions::OutboundPurpose::TokenTransfer),
        )
        .filter(outbound_transactions::Column::OperationType.is_in([
            outbound_transactions::OutboundOperationType::AutoSweep,
            outbound_transactions::OutboundOperationType::ManualSweep,
            outbound_transactions::OutboundOperationType::ManualTransfer,
        ]))
        .filter(outbound_transactions::Column::ToAddress.eq(&treasury_address))
        .select_only()
        .column_as(
            sea_orm::sea_query::Expr::col(outbound_transactions::Column::Amount).sum(),
            "total",
        )
        .into_tuple::<Option<Decimal>>()
        .one(db)
        .await?
        .flatten()
        .and_then(|d| d.to_i64())
        .unwrap_or(0);

    // 3. Total paid out from this TRON treasury: withdrawals + payout API orders.
    let total_withdrawals: i64 = Withdrawals::find()
        .filter(withdrawals::Column::Status.eq("Completed"))
        .filter(withdrawals::Column::Network.eq("TRON"))
        .select_only()
        .column_as(
            sea_orm::sea_query::Expr::col(withdrawals::Column::NetAmount).sum(),
            "total",
        )
        .into_tuple::<Option<Decimal>>()
        .one(db)
        .await?
        .flatten()
        .and_then(|d| d.to_i64())
        .unwrap_or(0);
    let total_payouts: i64 = Payouts::find()
        .filter(payouts::Column::Status.eq("Completed"))
        .filter(payouts::Column::Network.eq("TRON"))
        .select_only()
        .column_as(
            sea_orm::sea_query::Expr::col(payouts::Column::NetAmount).sum(),
            "total",
        )
        .into_tuple::<Option<Decimal>>()
        .one(db)
        .await?
        .flatten()
        .and_then(|d| d.to_i64())
        .unwrap_or(0);
    let total_paid_out = total_withdrawals.saturating_add(total_payouts);

    // 4. Reconciliation
    let expected_balance = total_swept_in - total_paid_out;
    let discrepancy = match balance_raw {
        Some(actual) => format_usdt(expected_balance - actual),
        None => "N/A".to_string(),
    };

    // 5. Low-balance alert
    let low_balance_alert = match balance_raw {
        Some(b) => b < threshold,
        None => false,
    };

    // 6. Recent transactions — last 20 sweeps + withdrawals merged
    let recent_sweeps: Vec<TreasuryTransaction> = OutboundTransactions::find()
        .filter(outbound_transactions::Column::State.eq("Confirmed"))
        .filter(
            outbound_transactions::Column::Purpose
                .eq(outbound_transactions::OutboundPurpose::TokenTransfer),
        )
        .filter(outbound_transactions::Column::OperationType.is_in([
            outbound_transactions::OutboundOperationType::AutoSweep,
            outbound_transactions::OutboundOperationType::ManualSweep,
            outbound_transactions::OutboundOperationType::ManualTransfer,
        ]))
        .filter(outbound_transactions::Column::ToAddress.eq(&treasury_address))
        .order_by_desc(outbound_transactions::Column::CreatedAt)
        .limit(20)
        .all(db)
        .await?
        .into_iter()
        .map(|s| TreasuryTransaction {
            direction: "in".to_string(),
            tx_type: serde_json::to_value(&s.operation_type)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| format!("{:?}", s.operation_type)),
            amount: format_usdt(s.amount),
            merchant_id: s.merchant_id,
            tx_hash: s.tx_hash,
            created_at: s.created_at.to_rfc3339(),
        })
        .collect();

    let recent_withdrawals: Vec<TreasuryTransaction> = Withdrawals::find()
        .filter(withdrawals::Column::Status.eq("Completed"))
        .filter(withdrawals::Column::Network.eq("TRON"))
        .order_by_desc(withdrawals::Column::CreatedAt)
        .limit(20)
        .all(db)
        .await?
        .into_iter()
        .map(|w| TreasuryTransaction {
            direction: "out".to_string(),
            tx_type: "withdrawal".to_string(),
            amount: format_usdt(w.net_amount),
            merchant_id: w.merchant_id,
            tx_hash: w.tx_hash,
            created_at: w.created_at.to_rfc3339(),
        })
        .collect();

    let recent_payouts: Vec<TreasuryTransaction> = Payouts::find()
        .filter(payouts::Column::Status.eq("Completed"))
        .filter(payouts::Column::Network.eq("TRON"))
        .order_by_desc(payouts::Column::CreatedAt)
        .limit(20)
        .all(db)
        .await?
        .into_iter()
        .map(|p| TreasuryTransaction {
            direction: "out".to_string(),
            tx_type: "payout".to_string(),
            amount: format_usdt(p.net_amount),
            merchant_id: p.merchant_id,
            tx_hash: p.tx_hash,
            created_at: p.created_at.to_rfc3339(),
        })
        .collect();

    // Merge and sort by created_at descending, take 20
    let mut recent_transactions = recent_sweeps;
    recent_transactions.extend(recent_withdrawals);
    recent_transactions.extend(recent_payouts);
    recent_transactions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    recent_transactions.truncate(20);

    Ok(Json(TreasuryOverview {
        balance: balance_raw.map(format_usdt),
        address: treasury_address,
        low_balance_alert,
        threshold: format_usdt(threshold),
        total_swept_in: format_usdt(total_swept_in),
        total_paid_out: format_usdt(total_paid_out),
        expected_balance: format_usdt(expected_balance),
        discrepancy,
        recent_transactions,
    }))
}

// ─── Platform Wallets ───────────────────────────────────────

/// GET /api/admin/platform-wallets
/// Returns per-chain treasury and gas sponsor addresses + balances.
/// Low balance thresholds computed server-side.
async fn get_platform_wallets(
    Extension(state): Extension<AppState>,
) -> Result<Json<PlatformWalletsResponse>, AppError> {
    let mut wallets = Vec::new();

    // ── TRON ──
    let tron_treasury_addr = state.treasury_address.clone();
    let tron_gs_addr = state.gas_sponsor_address.clone();
    let tron_client = state.tron_client.clone();

    // Query TRON balances concurrently
    let (tron_usdt_result, tron_trx_result) = tokio::join!(
        tron_client.get_usdt_balance(&tron_treasury_addr),
        tron_client.get_trx_balance(&tron_gs_addr),
    );

    let tron_usdt_balance = tron_usdt_result.ok().map(|b| format_usdt(b));
    let tron_trx_balance = tron_trx_result.ok();
    // TRX threshold: 100 TRX = 100_000_000 sun
    let tron_gs_low = tron_trx_balance.map(|b| b < 100_000_000).unwrap_or(true);
    let tron_trx_formatted = tron_trx_balance.map(|b| {
        let whole = b / 1_000_000;
        let frac = b % 1_000_000;
        format!("{}.{:06}", whole, frac)
    });

    wallets.push(ChainWallet {
        chain: "TRON".to_string(),
        treasury_address: tron_treasury_addr,
        treasury_usdt_balance: tron_usdt_balance,
        treasury_usdc_balance: None, // USDC not supported on TRON
        gas_sponsor_address: tron_gs_addr,
        gas_sponsor_native_balance: tron_trx_formatted,
        native_symbol: "TRX".to_string(),
        gas_sponsor_low_balance: tron_gs_low,
    });

    // ── EVM Chains (all share the same HD-derived treasury/gas-sponsor addresses) ──
    if let (Some(evm_treasury), Some(evm_gs)) =
        (&state.evm_treasury_address, &state.evm_gas_sponsor_address)
    {
        let entity_env = state.config.environment.to_entity_environment();

        // Format U256 with 18 decimals (native token) into human-readable string
        let format_18dec = |val: alloy_primitives::U256| -> String {
            let divisor = alloy_primitives::U256::from(1_000_000_000_000_000_000u64);
            let whole = val / divisor;
            let frac = val % divisor;
            let frac_scaled = frac / alloy_primitives::U256::from(1_000_000_000_000u64);
            format!("{}.{:06}", whole, frac_scaled)
        };

        // Format U256 with variable decimals (USDT: 6 on most chains, 18 on BSC)
        let format_token = |val: alloy_primitives::U256, decimals: u8| -> String {
            if decimals == 18 {
                format_18dec(val)
            } else {
                // 6-decimal tokens (standard USDT)
                let divisor = alloy_primitives::U256::from(1_000_000u64);
                let whole = val / divisor;
                let frac = val % divisor;
                format!("{}.{:06}", whole, frac)
            }
        };

        for (network, client) in state.chain_clients.iter() {
            // Skip TRON — handled separately above with T-prefix addresses
            if network.chain_family() == crate::entity::ChainFamily::Tron {
                continue;
            }
            let chain_cfg = network.chain_config(&entity_env);
            let usdt_contract = chain_cfg.usdt_contract;
            let usdt_decimals = chain_cfg.usdt_decimals;
            let native_symbol = chain_cfg.native_symbol.to_string();

            // Query balances concurrently (USDT + USDC + native)
            let usdc_contract = chain_cfg.usdc_contract.clone();
            let (usdt_result, usdc_result, native_result) = tokio::join!(
                client.get_token_balance(evm_treasury, &usdt_contract),
                async {
                    if let Some(ref contract) = usdc_contract {
                        client.get_token_balance(evm_treasury, contract).await.ok()
                    } else {
                        None
                    }
                },
                client.get_native_balance(evm_gs),
            );

            let usdt_balance = usdt_result.ok().map(|b| format_token(b, usdt_decimals));
            let usdc_decimals = chain_cfg.usdc_decimals.unwrap_or(6);
            let usdc_balance = usdc_result.map(|b| format_token(b, usdc_decimals));

            // Low balance threshold: 0.05 native token = 50_000_000_000_000_000 wei
            let low_threshold = alloy_primitives::U256::from(50_000_000_000_000_000u64);
            let gs_low = native_result
                .as_ref()
                .ok()
                .map(|b| *b < low_threshold)
                .unwrap_or(true);
            let native_formatted = native_result.ok().map(|b| format_18dec(b));

            wallets.push(ChainWallet {
                chain: network.as_str().to_string(),
                treasury_address: evm_treasury.clone(),
                treasury_usdt_balance: usdt_balance,
                treasury_usdc_balance: usdc_balance,
                gas_sponsor_address: evm_gs.clone(),
                gas_sponsor_native_balance: native_formatted,
                native_symbol,
                gas_sponsor_low_balance: gs_low,
            });
        }
    }

    // ── Solana ──
    if let (Some(sol_treasury), Some(sol_client)) =
        (&state.solana_treasury_address, &state.solana_client)
    {
        use crate::entity::Network;
        let entity_env = state.config.environment.to_entity_environment();
        let chain_cfg = Network::Solana.chain_config(&entity_env);

        // Query USDT + USDC + native SOL balances concurrently
        let usdt_mint = chain_cfg.usdt_contract.to_string();
        let usdc_mint: Option<String> = chain_cfg.usdc_contract.map(|s| s.to_string());

        let (usdt_result, usdc_result, sol_result) = tokio::join!(
            sol_client.get_spl_token_balance(sol_treasury, &usdt_mint),
            async {
                if let Some(ref mint) = usdc_mint {
                    sol_client
                        .get_spl_token_balance(sol_treasury, mint)
                        .await
                        .ok()
                } else {
                    None
                }
            },
            sol_client.get_sol_balance(sol_treasury),
        );

        let usdt_balance = usdt_result.ok().map(|b| format_usdt(b));
        let usdc_balance = usdc_result.map(|b| format_usdt(b));

        // Low threshold: 0.05 SOL = 50_000_000 lamports
        let gs_low = sol_result
            .as_ref()
            .ok()
            .map(|b| *b < 50_000_000)
            .unwrap_or(true);

        // SOL: lamports to SOL (9 decimals)
        let sol_balance = sol_result.ok().map(|lamports| {
            let whole = lamports / 1_000_000_000;
            let frac = lamports % 1_000_000_000;
            format!("{}.{:09}", whole, frac)
        });

        wallets.push(ChainWallet {
            chain: "SOLANA".to_string(),
            treasury_address: sol_treasury.clone(),
            treasury_usdt_balance: usdt_balance,
            treasury_usdc_balance: usdc_balance,
            gas_sponsor_address: sol_treasury.clone(), // Solana: fee payer = treasury
            gas_sponsor_native_balance: sol_balance,
            native_symbol: "SOL".to_string(),
            gas_sponsor_low_balance: gs_low,
        });
    }

    Ok(Json(PlatformWalletsResponse { wallets }))
}

// ─── Agent Management ──────────────────────────────────────────

async fn create_agent(
    Extension(state): Extension<AppState>,
    Json(body): Json<crate::api::dtos::agent::CreateAgentRequest>,
) -> Result<Json<serde_json::Value>, crate::api::error::AppError> {
    use std::str::FromStr;
    let base_rate = body
        .base_rate
        .as_deref()
        .map(Decimal::from_str)
        .transpose()
        .map_err(|e| anyhow::anyhow!("Invalid base_rate: {}", e))?;
    let max_markup = body
        .max_markup
        .as_deref()
        .map(Decimal::from_str)
        .transpose()
        .map_err(|e| anyhow::anyhow!("Invalid max_markup: {}", e))?;
    let default_merchant_rate = body
        .default_merchant_rate
        .as_deref()
        .map(Decimal::from_str)
        .transpose()
        .map_err(|e| anyhow::anyhow!("Invalid default_merchant_rate: {}", e))?;
    let agent = state
        .agent_service
        .create_agent(
            &body.merchant_id,
            base_rate,
            max_markup,
            default_merchant_rate,
        )
        .await?;
    Ok(Json(serde_json::json!({
        "id": agent.id,
        "merchant_id": agent.merchant_id,
        "referral_code": agent.referral_code,
        "base_rate": agent.base_rate.to_string(),
        "max_markup": agent.max_markup.to_string(),
        "default_merchant_rate": agent.default_merchant_rate.to_string(),
        "status": agent.status,
        "created_at": agent.created_at.to_rfc3339(),
    })))
}

async fn list_agents(
    Extension(state): Extension<AppState>,
) -> Result<Json<serde_json::Value>, crate::api::error::AppError> {
    let (agents, total) = state.agent_service.list_agents(1, 100).await?;
    let mut items = Vec::new();
    for a in agents {
        let count = state
            .agent_service
            .count_referred_merchants(&a.id)
            .await
            .unwrap_or(0);
        items.push(serde_json::json!({
            "id": a.id,
            "merchant_id": a.merchant_id,
            "referral_code": a.referral_code,
            "base_rate": a.base_rate.to_string(),
            "max_markup": a.max_markup.to_string(),
            "default_merchant_rate": a.default_merchant_rate.to_string(),
            "status": a.status,
            "referred_merchant_count": count,
            "created_at": a.created_at.to_rfc3339(),
        }));
    }
    Ok(Json(serde_json::json!({ "agents": items, "total": total })))
}

async fn get_agent(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, crate::api::error::AppError> {
    let agent = state
        .agent_service
        .get_agent(&id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Agent not found"))?;
    let count = state
        .agent_service
        .count_referred_merchants(&agent.id)
        .await
        .unwrap_or(0);
    let referred = state
        .agent_service
        .list_referred_merchants(&agent.id)
        .await
        .unwrap_or_default();
    Ok(Json(serde_json::json!({
        "id": agent.id,
        "merchant_id": agent.merchant_id,
        "referral_code": agent.referral_code,
        "base_rate": agent.base_rate.to_string(),
        "max_markup": agent.max_markup.to_string(),
        "default_merchant_rate": agent.default_merchant_rate.to_string(),
        "status": agent.status,
        "referred_merchant_count": count,
        "referred_merchants": referred.iter().map(|m| serde_json::json!({
            "id": m.id,
            "name": m.name,
            "custom_fee_percentage": m.custom_fee_percentage.map(|d| d.to_string()),
            "created_at": m.created_at.to_rfc3339(),
        })).collect::<Vec<_>>(),
        "created_at": agent.created_at.to_rfc3339(),
    })))
}

async fn update_agent(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
    Json(body): Json<crate::api::dtos::agent::UpdateAgentRequest>,
) -> Result<Json<serde_json::Value>, crate::api::error::AppError> {
    use std::str::FromStr;
    let base_rate = body
        .base_rate
        .as_deref()
        .map(Decimal::from_str)
        .transpose()
        .map_err(|e| anyhow::anyhow!("Invalid base_rate: {}", e))?;
    let max_markup = body
        .max_markup
        .as_deref()
        .map(Decimal::from_str)
        .transpose()
        .map_err(|e| anyhow::anyhow!("Invalid max_markup: {}", e))?;
    let default_merchant_rate = body
        .default_merchant_rate
        .as_deref()
        .map(Decimal::from_str)
        .transpose()
        .map_err(|e| anyhow::anyhow!("Invalid default_merchant_rate: {}", e))?;
    let updated = state
        .agent_service
        .update_agent(
            &id,
            base_rate,
            max_markup,
            default_merchant_rate,
            body.status,
        )
        .await?;
    Ok(Json(serde_json::json!({
        "id": updated.id,
        "base_rate": updated.base_rate.to_string(),
        "max_markup": updated.max_markup.to_string(),
        "default_merchant_rate": updated.default_merchant_rate.to_string(),
        "status": updated.status,
    })))
}

async fn get_agent_commission(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
    Query(query): Query<crate::api::dtos::agent::CommissionQuery>,
) -> Result<Json<serde_json::Value>, crate::api::error::AppError> {
    let start = query
        .start_date
        .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| (chrono::Utc::now() - chrono::Duration::days(30)).date_naive());
    let end = query
        .end_date
        .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| chrono::Utc::now().date_naive());

    let start_str = start.format("%Y-%m-%dT00:00:00Z").to_string();
    let end_str = end.format("%Y-%m-%dT23:59:59Z").to_string();

    let report = state
        .agent_service
        .get_commission_report(&id, &start_str, &end_str)
        .await?;
    Ok(Json(serde_json::json!(report)))
}

// ─── Sub-Merchants ──────────────────────────────────────────

#[derive(Debug, Deserialize, Validate)]
struct CreateSubMerchantBody {
    /// PSP's org ID
    #[validate(length(min = 1))]
    parent_org_id: String,
    /// PSP-defined sub-merchant code
    #[validate(length(min = 1, max = 100))]
    sub_merchant_code: String,
    /// Display name for the sub-merchant
    #[validate(length(min = 1, max = 200))]
    display_name: String,
}

#[derive(Debug, Deserialize, Default)]
struct SubMerchantFilter {
    #[serde(default)]
    parent_org_id: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
struct UpdateSubMerchantBody {
    #[validate(length(min = 1, max = 200))]
    display_name: Option<String>,
    status: Option<crate::entity::sub_merchants::SubMerchantStatus>,
}

/// POST /api/admin/sub-merchants
async fn create_sub_merchant(
    Extension(state): Extension<AppState>,
    Json(body): Json<CreateSubMerchantBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    body.validate()?;

    let result = state
        .sub_merchant_service
        .create(crate::services::sub_merchant::CreateSubMerchantInput {
            parent_org_id: body.parent_org_id,
            sub_merchant_code: body.sub_merchant_code,
            display_name: body.display_name,
        })
        .await?;

    Ok(Json(serde_json::json!(result)))
}

/// GET /api/admin/sub-merchants
async fn list_sub_merchants(
    Extension(state): Extension<AppState>,
    Query(pagination): Query<PaginationRequest>,
    Query(filter): Query<SubMerchantFilter>,
) -> Result<Json<PaginatedResponse<serde_json::Value>>, AppError> {
    pagination.validate()?;

    let result = state
        .sub_merchant_service
        .list(
            filter.parent_org_id.as_deref(),
            crate::services::sub_merchant::Pagination {
                page: pagination.page,
                page_size: pagination.page_size,
            },
        )
        .await?;

    let data: Vec<serde_json::Value> = result
        .items
        .into_iter()
        .map(|sm| serde_json::json!(sm))
        .collect();

    Ok(Json(PaginatedResponse::new(
        data,
        result.total,
        result.page,
        result.page_size,
    )))
}

/// GET /api/admin/sub-merchants/:id
async fn get_sub_merchant(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let result = state.sub_merchant_service.get_by_id(&id).await?;

    Ok(Json(serde_json::json!(result)))
}

/// PATCH /api/admin/sub-merchants/:id
async fn update_sub_merchant(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateSubMerchantBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    body.validate()?;

    let result = state
        .sub_merchant_service
        .update_by_id(
            &id,
            crate::services::sub_merchant::UpdateSubMerchantInput {
                display_name: body.display_name,
                status: body.status,
            },
        )
        .await?;

    Ok(Json(serde_json::json!(result)))
}
