//! Agent Dashboard Routes (Internal)
//!
//! Endpoints for merchants to check if they are agents and view commission data.
//! All routes are JWT-authed via the parent `/api/internal` router.

use axum::{
    extract::{Path, Query, State},
    routing::{get, patch},
    Json, Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::api::error::AppError;
use crate::api::middleware::auth::AuthenticatedMerchant;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/me", get(get_agent_me))
        .route("/overview", get(get_agent_overview))
        .route("/commission", get(get_agent_commission))
        .route("/merchants", get(list_agent_merchants))
        .route("/merchants/:merchant_id/rate", patch(update_merchant_rate))
}

/// Lightweight check: is the current merchant an agent?
/// Called on every login in production (2 SELECTs, fast).
#[derive(Serialize)]
struct AgentMeResponse {
    is_agent: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    referral_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    base_rate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_merchant_rate: Option<String>,
    referred_merchant_count: u64,
}

async fn get_agent_me(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
) -> Result<Json<AgentMeResponse>, AppError> {
    let agent = state
        .agent_service
        .find_by_merchant_id(&merchant.id)
        .await
        .map_err(AppError::InternalServerError)?;

    match agent {
        Some(a) => {
            let count = state
                .agent_service
                .count_referred_merchants(&a.id)
                .await
                .map_err(AppError::InternalServerError)?;

            Ok(Json(AgentMeResponse {
                is_agent: true,
                agent_id: Some(a.id),
                referral_code: Some(a.referral_code),
                base_rate: Some(format!(
                    "{}%",
                    (a.base_rate * rust_decimal::Decimal::from(100)).round_dp(2)
                )),
                default_merchant_rate: Some(format!(
                    "{}%",
                    (a.default_merchant_rate * rust_decimal::Decimal::from(100)).round_dp(2)
                )),
                referred_merchant_count: count,
            }))
        }
        None => Ok(Json(AgentMeResponse {
            is_agent: false,
            agent_id: None,
            referral_code: None,
            base_rate: None,
            default_merchant_rate: None,
            referred_merchant_count: 0,
        })),
    }
}

/// Full overview: agent info + all-time commission total.
/// Called only when Agent Dashboard page mounts.
async fn get_agent_overview(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
) -> Result<Json<crate::services::agent::AgentOverview>, AppError> {
    state
        .agent_service
        .get_overview(&merchant.id)
        .await
        .map(Json)
        .map_err(AppError::InternalServerError)
}

/// Date-range commission report.
/// Called when user clicks "Query" on Agent Dashboard.
#[derive(Deserialize)]
struct CommissionQuery {
    start_date: String,
    end_date: String,
}

async fn get_agent_commission(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Query(query): Query<CommissionQuery>,
) -> Result<Json<crate::services::agent::CommissionReport>, AppError> {
    // Find agent by merchant_id first
    let agent = state
        .agent_service
        .find_by_merchant_id(&merchant.id)
        .await
        .map_err(AppError::InternalServerError)?
        .ok_or_else(|| AppError::NotFound("Not an agent".to_string()))?;

    state
        .agent_service
        .get_commission_report(&agent.id, &query.start_date, &query.end_date)
        .await
        .map(Json)
        .map_err(AppError::InternalServerError)
}

// ─── Merchant Rate Management ────────────────────────────────────────────────

/// List merchants referred by this agent, with their current fee rates.
#[derive(Serialize)]
struct ReferredMerchantInfo {
    merchant_id: String,
    name: String,
    /// Current fee percentage as string, e.g. "0.4000%"
    current_rate: String,
    created_at: String,
}

async fn list_agent_merchants(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
) -> Result<Json<serde_json::Value>, AppError> {
    let agent = state
        .agent_service
        .find_by_merchant_id(&merchant.id)
        .await
        .map_err(AppError::InternalServerError)?
        .ok_or_else(|| AppError::NotFound("Not an agent".to_string()))?;

    let merchants = state
        .agent_service
        .list_referred_merchants(&agent.id)
        .await
        .map_err(AppError::InternalServerError)?;

    let default_rate = Decimal::new(1, 2); // 0.01 = 1% global default
    let items: Vec<ReferredMerchantInfo> = merchants
        .into_iter()
        .map(|m| {
            let rate = m.custom_fee_percentage.unwrap_or(default_rate);
            ReferredMerchantInfo {
                merchant_id: m.id,
                name: m.name,
                current_rate: format!("{}%", (rate * Decimal::from(100)).round_dp(2)),
                created_at: m.created_at.to_rfc3339(),
            }
        })
        .collect();

    Ok(Json(serde_json::json!({ "merchants": items })))
}

/// Update a referred merchant's fee rate.
/// Rate must be within [agent.base_rate, agent.max_markup].
#[derive(Deserialize)]
struct UpdateRateRequest {
    /// Fee rate as decimal fraction, e.g. 0.003 = 0.3%
    fee_rate: f64,
}

async fn update_merchant_rate(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Path(target_merchant_id): Path<String>,
    Json(req): Json<UpdateRateRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    use crate::entity::{merchants, Merchants};
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};

    // 1. Verify caller is an agent
    let agent = state
        .agent_service
        .find_by_merchant_id(&merchant.id)
        .await
        .map_err(AppError::InternalServerError)?
        .ok_or_else(|| AppError::NotFound("Not an agent".to_string()))?;

    // 2. Verify target merchant was referred by this agent
    let target = Merchants::find_by_id(&target_merchant_id)
        .one(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?
        .ok_or_else(|| AppError::NotFound("Merchant not found".to_string()))?;

    if target.referred_by_agent_id.as_deref() != Some(&agent.id) {
        return Err(AppError::PermissionDenied(
            "You can only update rates for merchants you referred".to_string(),
        ));
    }

    // 3. Validate fee_rate is within [base_rate, max_markup]
    let fee_rate = Decimal::try_from(req.fee_rate).map_err(|_| AppError::ValidationError {
        code: crate::api::error::E_PARAMETER_INVALID,
        message: "Invalid fee_rate value".into(),
        param: Some("fee_rate".into()),
    })?;

    if fee_rate < agent.base_rate || fee_rate > agent.max_markup {
        return Err(AppError::ValidationError {
            code: crate::api::error::E_PARAMETER_INVALID,
            message: format!(
                "fee_rate must be between {} and {} ({}% to {}%)",
                agent.base_rate,
                agent.max_markup,
                agent.base_rate * Decimal::from(100),
                agent.max_markup * Decimal::from(100),
            ),
            param: Some("fee_rate".into()),
        });
    }

    // 4. Update merchant's custom_fee_percentage + fee_source
    let mut active: merchants::ActiveModel = target.into();
    active.custom_fee_percentage = Set(Some(fee_rate));
    active.fee_source = Set(merchants::FeeSource::Agent);
    active.updated_at = Set(chrono::Utc::now().into());
    let updated = active
        .update(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?;

    tracing::info!(
        agent_id = %agent.id,
        merchant_id = %target_merchant_id,
        new_rate = %fee_rate,
        "Agent updated merchant fee rate"
    );

    Ok(Json(serde_json::json!({
        "merchant_id": updated.id,
        "custom_fee_percentage": format!("{}%", (fee_rate * Decimal::from(100)).round_dp(2)),
    })))
}
