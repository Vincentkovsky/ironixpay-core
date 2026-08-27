//! Xero Integration API Routes
//!
//! OAuth connection management, account configuration, and sync log viewing.
//! All routes are under `/api/internal/xero` with JWT auth.

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use uuid::Uuid;

use crate::api::dtos::pagination::PaginatedResponse;
use crate::api::dtos::xero::{
    XeroAccountDto, XeroCallbackRequest, XeroCallbackResponse, XeroCapabilityResponse,
    XeroConnectRequest, XeroConnectResponse, XeroConnectionResponse, XeroSelectTenantRequest,
    XeroSelectTenantResponse, XeroSyncLogResponse, XeroSyncLogsQuery, XeroTaxRateDto,
    XeroTenantDto, XeroUpdateConnectionRequest,
};
use crate::api::error::{AppError, E_PARAMETER_INVALID};
use crate::api::middleware::auth::{require_role, AuthenticatedMerchant};
use crate::entity::org_members::MemberRole;
use crate::services::xero::client::XeroError;
use crate::services::xero::XeroConfigError;
use crate::AppState;
use tracing::warn;

fn map_xero_rate_limit(err: &anyhow::Error) -> Option<AppError> {
    if let Some(XeroError::RateLimited { retry_after }) = err.downcast_ref::<XeroError>() {
        return Some(AppError::ServiceUnavailable(format!(
            "Xero API rate limited. Please retry in {} seconds.",
            retry_after
        )));
    }
    None
}

fn map_xero_callback_error(err: anyhow::Error, phase: &'static str) -> AppError {
    if let Some(app_err) = map_xero_rate_limit(&err) {
        return app_err;
    }

    let err_str = err.to_string();
    let err_lower = err_str.to_lowercase();

    if err_lower.contains("relation \"xero_connections\" does not exist")
        || err_lower.contains("relation \"xero_sync_logs\" does not exist")
        || err_lower.contains("column \"xero_tax_type\" does not exist")
    {
        return AppError::ServiceUnavailable(
            "Xero database schema is missing. Run `cd backend && cargo run -p migration -- up` and retry.".into(),
        );
    }

    if phase == "exchange_code" {
        if err_lower.contains("invalid_grant") {
            return AppError::ValidationError {
                code: E_PARAMETER_INVALID,
                message:
                    "Xero authorization code is invalid or expired. Please reconnect from the beginning.".into(),
                param: Some("code".to_string()),
            };
        }

        if err_lower.contains("invalid_client") {
            return AppError::ServiceUnavailable(
                "Xero OAuth client credentials are invalid. Check XERO_CLIENT_ID/XERO_CLIENT_SECRET."
                    .into(),
            );
        }

        if err_lower.contains("redirect_uri") {
            return AppError::ValidationError {
                code: E_PARAMETER_INVALID,
                message: "Xero redirect URI mismatch. Ensure XERO_REDIRECT_URI exactly matches the callback URL configured in Xero developer settings.".into(),
                param: Some("redirect_uri".to_string()),
            };
        }

        if err_lower.contains("failed to reach xero token endpoint")
            || err_lower.contains("failed to get xero connections")
        {
            return AppError::ServiceUnavailable(
                "Unable to reach Xero API. Please check network connectivity and retry.".into(),
            );
        }
    }

    if phase == "get_organisation" {
        if err_lower.contains("xero organisation api failed (401")
            || err_lower.contains("xero organisation api failed (403")
        {
            return AppError::ValidationError {
                code: E_PARAMETER_INVALID,
                message:
                    "Xero organization access denied. Please reconnect and ensure your Xero app has correct accounting scopes.".into(),
                param: Some("authorization".to_string()),
            };
        }
    }

    warn!(phase, error = %err, "Xero callback failed");
    if cfg!(debug_assertions) {
        return AppError::ServiceUnavailable(format!(
            "Xero callback failed at {}: {}",
            phase, err_str
        ));
    }
    AppError::InternalServerError(err)
}

fn map_xero_update_error(err: anyhow::Error) -> AppError {
    if let Some(app_err) = map_xero_rate_limit(&err) {
        return app_err;
    }

    if let Some(cfg) = err.downcast_ref::<XeroConfigError>() {
        return match cfg {
            XeroConfigError::MissingField { field } => AppError::ValidationError {
                code: E_PARAMETER_INVALID,
                message: cfg.to_string(),
                param: Some((*field).to_string()),
            },
            XeroConfigError::InvalidField { field, .. } => AppError::ValidationError {
                code: E_PARAMETER_INVALID,
                message: cfg.to_string(),
                param: Some((*field).to_string()),
            },
        };
    }
    AppError::InternalServerError(err)
}

fn map_xero_api_error(err: anyhow::Error, phase: &'static str) -> AppError {
    if let Some(app_err) = map_xero_rate_limit(&err) {
        return app_err;
    }

    warn!(phase, error = %err, "Xero API call failed");
    if cfg!(debug_assertions) {
        return AppError::ServiceUnavailable(format!("Xero {} failed: {}", phase, err));
    }
    AppError::InternalServerError(err)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/capability", get(get_capability))
        .route("/connect", post(connect))
        .route("/callback", post(callback))
        .route("/select-tenant", post(select_tenant))
        .route(
            "/connection",
            get(get_connection)
                .put(update_connection)
                .delete(disconnect),
        )
        .route("/accounts", get(get_accounts))
        .route("/tax-rates", get(get_tax_rates))
        .route("/sync-logs", get(list_sync_logs))
        .route("/sync-logs/:id/retry", post(retry_sync_log))
}

/// GET /api/internal/xero/capability
///
/// Returns whether Xero integration is configured on this backend environment.
async fn get_capability(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
) -> Result<Json<XeroCapabilityResponse>, AppError> {
    require_role(&merchant, &[MemberRole::Owner, MemberRole::Admin])?;
    Ok(Json(XeroCapabilityResponse {
        enabled: state.xero_service.is_some(),
    }))
}

/// POST /api/internal/xero/connect
///
/// Initiate OAuth flow. Returns Xero authorization URL.
async fn connect(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Json(body): Json<XeroConnectRequest>,
) -> Result<Json<XeroConnectResponse>, AppError> {
    require_role(&merchant, &[MemberRole::Owner, MemberRole::Admin])?;

    let xero_service = state
        .xero_service
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("Xero integration is not configured".into()))?;

    // Check existing connection status.
    if let Some(existing) = xero_service
        .get_connection(&merchant.id, merchant.environment)
        .await
        .map_err(AppError::InternalServerError)?
    {
        match existing.status {
            crate::entity::xero_connections::XeroConnectionStatus::Active => {
                if !body.force_reauth.unwrap_or(false) {
                    return Err(AppError::Conflict(
                        "Xero is already connected for this environment. Use reconnect to re-authorize.".into(),
                    ));
                }
            }
            _ => {}
        }
    }

    let encrypted_state = xero_service
        .issue_oauth_state(&merchant.id, merchant.environment)
        .await
        .map_err(AppError::InternalServerError)?;

    let authorize_url = xero_service.authorize_url(&encrypted_state);

    Ok(Json(XeroConnectResponse { authorize_url }))
}

/// POST /api/internal/xero/callback
///
/// Complete OAuth flow. Exchange code for tokens, return tenant list.
async fn callback(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Json(body): Json<XeroCallbackRequest>,
) -> Result<Json<XeroCallbackResponse>, AppError> {
    require_role(&merchant, &[MemberRole::Owner, MemberRole::Admin])?;

    let xero_service = state
        .xero_service
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("Xero integration is not configured".into()))?;

    xero_service
        .verify_and_consume_oauth_state(&body.state, &merchant.id, merchant.environment)
        .await
        .map_err(|_| AppError::AuthError("Invalid or expired OAuth state token".into()))?;

    // Exchange code
    let (tokens, tenants) = xero_service
        .exchange_code(&body.code)
        .await
        .map_err(|e| map_xero_callback_error(e, "exchange_code"))?;

    let tenant_dtos: Vec<XeroTenantDto> = tenants
        .iter()
        .map(|t| XeroTenantDto {
            tenant_id: t.tenant_id.clone(),
            tenant_name: t.tenant_name.clone(),
            tenant_type: t.tenant_type.clone(),
        })
        .collect();

    if tenants.len() == 1 {
        // Auto-select single tenant
        let tenant = &tenants[0];

        // Fetch org info for default currency
        let org = xero_service
            .client
            .get_organisation(&tokens.access_token, &tenant.tenant_id)
            .await
            .map_err(|e| map_xero_callback_error(e, "get_organisation"))?;
        let default_currency = org["BaseCurrency"].as_str().unwrap_or("USD").to_string();

        let connection = xero_service
            .save_connection(
                &merchant.id,
                merchant.environment,
                &tokens,
                tenant,
                &default_currency,
            )
            .await
            .map_err(|e| map_xero_callback_error(e, "save_connection"))?;

        Ok(Json(XeroCallbackResponse {
            tenants: tenant_dtos,
            connection_id: Some(connection.id),
        }))
    } else {
        // Multiple tenants — store tokens temporarily for select-tenant
        // Save a "pending_selection" connection with first tenant's data.
        // Frontend must call select-tenant to finalize.
        // Worker skips connections with PendingSelection status.
        if let Some(first_tenant) = tenants.first() {
            xero_service
                .save_pending_selection_connection(
                    &merchant.id,
                    merchant.environment,
                    &tokens,
                    first_tenant,
                )
                .await
                .map_err(|e| map_xero_callback_error(e, "save_pending_connection"))?;
        }

        Ok(Json(XeroCallbackResponse {
            tenants: tenant_dtos,
            connection_id: None,
        }))
    }
}

/// POST /api/internal/xero/select-tenant
///
/// Select tenant from multi-tenant list (after callback returned multiple).
async fn select_tenant(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Json(body): Json<XeroSelectTenantRequest>,
) -> Result<Json<XeroSelectTenantResponse>, AppError> {
    require_role(&merchant, &[MemberRole::Owner, MemberRole::Admin])?;

    let xero_service = state
        .xero_service
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("Xero integration is not configured".into()))?;

    let connection = xero_service
        .get_connection(&merchant.id, merchant.environment)
        .await
        .map_err(AppError::InternalServerError)?
        .ok_or_else(|| {
            AppError::NotFound("No pending Xero connection. Please restart OAuth flow.".into())
        })?;

    if connection.status != crate::entity::xero_connections::XeroConnectionStatus::PendingSelection
    {
        return Err(AppError::Conflict(
            "Xero tenant selection is only allowed while connection is pending selection.".into(),
        ));
    }

    // Re-fetch tenants and verify selected tenant ID.
    let access_token = xero_service
        .get_access_token(&connection)
        .await
        .map_err(|e| map_xero_api_error(e, "get_access_token"))?;

    let tenants = xero_service
        .client
        .get_connections(&access_token)
        .await
        .map_err(|e| map_xero_api_error(e, "get_connections"))?;

    let selected = tenants
        .iter()
        .find(|t| t.tenant_id == body.tenant_id)
        .ok_or_else(|| AppError::ValidationError {
            code: E_PARAMETER_INVALID,
            message: "Selected tenant is no longer available. Please restart OAuth flow.".into(),
            param: Some("tenant_id".into()),
        })?;

    // Fetch org info for currency
    let org = xero_service
        .client
        .get_organisation(&access_token, &selected.tenant_id)
        .await
        .map_err(|e| map_xero_api_error(e, "get_organisation"))?;
    let default_currency = org["BaseCurrency"].as_str().unwrap_or("USD").to_string();

    let updated = xero_service
        .activate_selected_tenant(connection, selected, &default_currency)
        .await
        .map_err(|e| map_xero_api_error(e, "activate_selected_tenant"))?;

    Ok(Json(XeroSelectTenantResponse {
        connection_id: updated.id,
    }))
}

/// GET /api/internal/xero/connection
async fn get_connection(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
) -> Result<Json<Option<XeroConnectionResponse>>, AppError> {
    let xero_service = state
        .xero_service
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("Xero integration is not configured".into()))?;

    let connection = xero_service
        .get_connection(&merchant.id, merchant.environment)
        .await
        .map_err(AppError::InternalServerError)?;

    Ok(Json(connection.map(|c| c.into())))
}

/// PUT /api/internal/xero/connection
async fn update_connection(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Json(body): Json<XeroUpdateConnectionRequest>,
) -> Result<Json<XeroConnectionResponse>, AppError> {
    require_role(&merchant, &[MemberRole::Owner, MemberRole::Admin])?;

    let xero_service = state
        .xero_service
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("Xero integration is not configured".into()))?;

    let connection = xero_service
        .get_connection(&merchant.id, merchant.environment)
        .await
        .map_err(AppError::InternalServerError)?
        .ok_or_else(|| AppError::NotFound("No Xero connection found".into()))?;

    let updated = xero_service
        .update_connection_config(
            connection.id,
            body.xero_account_code,
            body.xero_fee_account_code,
            body.xero_payment_account_code,
            body.xero_tax_type,
            body.auto_sync_enabled,
        )
        .await
        .map_err(map_xero_update_error)?;

    Ok(Json(updated.into()))
}

/// DELETE /api/internal/xero/connection
async fn disconnect(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
) -> Result<Json<serde_json::Value>, AppError> {
    require_role(&merchant, &[MemberRole::Owner, MemberRole::Admin])?;

    let xero_service = state
        .xero_service
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("Xero integration is not configured".into()))?;

    let connection = xero_service
        .get_connection(&merchant.id, merchant.environment)
        .await
        .map_err(AppError::InternalServerError)?
        .ok_or_else(|| AppError::NotFound("No Xero connection found".into()))?;

    xero_service
        .disconnect(connection.id)
        .await
        .map_err(AppError::InternalServerError)?;

    Ok(Json(serde_json::json!({ "status": "disconnected" })))
}

/// GET /api/internal/xero/accounts
///
/// Fetch Xero chart of accounts for configuration (revenue + expense + payment accounts).
async fn get_accounts(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
) -> Result<Json<Vec<XeroAccountDto>>, AppError> {
    let xero_service = state
        .xero_service
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("Xero integration is not configured".into()))?;

    let connection = xero_service
        .get_connection(&merchant.id, merchant.environment)
        .await
        .map_err(AppError::InternalServerError)?
        .ok_or_else(|| AppError::NotFound("No Xero connection found".into()))?;

    let access_token = xero_service
        .get_access_token(&connection)
        .await
        .map_err(|e| map_xero_api_error(e, "get_access_token"))?;

    let accounts = xero_service
        .client
        .get_accounts(&access_token, &connection.xero_tenant_id)
        .await
        .map_err(|e| map_xero_api_error(e, "get_accounts"))?;

    let dtos: Vec<XeroAccountDto> = accounts
        .iter()
        .filter_map(|a| {
            Some(XeroAccountDto {
                account_id: a["AccountID"].as_str()?.to_string(),
                code: a["Code"].as_str().unwrap_or("").to_string(),
                name: a["Name"].as_str()?.to_string(),
                r#type: a["Type"].as_str().unwrap_or("").to_string(),
                class: a["Class"].as_str().unwrap_or("").to_string(),
                enable_payments: a["EnablePaymentsToAccount"].as_bool().unwrap_or(false),
            })
        })
        .collect();

    Ok(Json(dtos))
}

/// GET /api/internal/xero/tax-rates
///
/// Fetch Xero tax rates for revenue invoice mapping.
async fn get_tax_rates(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
) -> Result<Json<Vec<XeroTaxRateDto>>, AppError> {
    let xero_service = state
        .xero_service
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("Xero integration is not configured".into()))?;

    let connection = xero_service
        .get_connection(&merchant.id, merchant.environment)
        .await
        .map_err(AppError::InternalServerError)?
        .ok_or_else(|| AppError::NotFound("No Xero connection found".into()))?;

    let access_token = xero_service
        .get_access_token(&connection)
        .await
        .map_err(|e| map_xero_api_error(e, "get_access_token"))?;

    let tax_rates = xero_service
        .client
        .get_tax_rates(&access_token, &connection.xero_tenant_id)
        .await
        .map_err(|e| map_xero_api_error(e, "get_tax_rates"))?;

    let mut dtos: Vec<XeroTaxRateDto> = tax_rates
        .iter()
        .filter_map(|t| {
            Some(XeroTaxRateDto {
                tax_type: t["TaxType"].as_str()?.to_string(),
                name: t["Name"].as_str().unwrap_or("").to_string(),
                display_tax_rate: t["DisplayTaxRate"].as_f64().unwrap_or(0.0),
                can_apply_to_revenue: t["CanApplyToRevenue"].as_bool().unwrap_or(false),
                status: t["Status"].as_str().unwrap_or("").to_string(),
            })
        })
        .collect();

    // Keep only active rates for revenue mapping.
    dtos.retain(|t| t.status.eq_ignore_ascii_case("ACTIVE") && t.can_apply_to_revenue);

    // Ensure default "No Tax" is always selectable.
    if !dtos.iter().any(|t| t.tax_type.eq_ignore_ascii_case("NONE")) {
        dtos.insert(
            0,
            XeroTaxRateDto {
                tax_type: "NONE".to_string(),
                name: "No Tax".to_string(),
                display_tax_rate: 0.0,
                can_apply_to_revenue: true,
                status: "ACTIVE".to_string(),
            },
        );
    }

    Ok(Json(dtos))
}

/// GET /api/internal/xero/sync-logs
async fn list_sync_logs(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Query(query): Query<XeroSyncLogsQuery>,
) -> Result<Json<PaginatedResponse<XeroSyncLogResponse>>, AppError> {
    let xero_service = state
        .xero_service
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("Xero integration is not configured".into()))?;

    let connection = xero_service
        .get_connection(&merchant.id, merchant.environment)
        .await
        .map_err(AppError::InternalServerError)?
        .ok_or_else(|| AppError::NotFound("No Xero connection found".into()))?;

    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20).min(100);

    let (logs, total) = xero_service
        .list_sync_logs(connection.id, page, per_page, query.session_id.as_deref())
        .await
        .map_err(AppError::InternalServerError)?;

    Ok(Json(PaginatedResponse::new(
        logs.into_iter().map(|l| l.into()).collect(),
        total,
        page,
        per_page,
    )))
}

/// POST /api/internal/xero/sync-logs/:id/retry
async fn retry_sync_log(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_role(&merchant, &[MemberRole::Owner, MemberRole::Admin])?;

    let xero_service = state
        .xero_service
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("Xero integration is not configured".into()))?;

    // Verify the sync log belongs to this merchant's connection
    let connection = xero_service
        .get_connection(&merchant.id, merchant.environment)
        .await
        .map_err(AppError::InternalServerError)?
        .ok_or_else(|| AppError::NotFound("No Xero connection found".into()))?;

    xero_service
        .retry_sync_log(id, connection.id)
        .await
        .map_err(AppError::InternalServerError)?;

    Ok(Json(serde_json::json!({ "status": "queued" })))
}
