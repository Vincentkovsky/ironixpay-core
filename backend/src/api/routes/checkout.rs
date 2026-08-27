//! Checkout Session API Routes
//!
//! Aligned with docs/system_design.md
//!
//! Supports Idempotency-Key header per §7.4 for safe retries.

use axum::{
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    middleware,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use tracing::warn;
use validator::Validate;

use crate::api::dtos::checkout::{
    CreateSessionBody, SessionBuildContext, SessionFilterParams, SessionResponse,
    TransactionResponse,
};
use crate::api::dtos::pagination::{PaginatedResponse, PaginationRequest};
use crate::api::error::{ApiErrorResponse, AppError, AppJson, E_PARAMETER_INVALID};
use crate::api::middleware::auth::{auth_middleware, AuthenticatedMerchant};
use crate::api::middleware::idempotency::{
    cached_response, check_idempotency, conflict_response, processing_response,
    update_idempotency_response, IdempotencyResult, IDEMPOTENCY_KEY_HEADER,
};
use crate::entity::{checkout_sessions, transactions, Merchants, Network};
use crate::services::checkout::CreateSessionRequest;
use crate::AppState;
use sea_orm::EntityTrait;
use serde::Deserialize;

use crate::api::middleware::sub_merchant::{sub_merchant_scope, SubMerchantContext};

pub fn router() -> Router<AppState> {
    // Public routes (no auth required) - used by checkout frontend
    let public_routes = Router::new()
        // Public session view for checkout page (no IDOR check - session ID is secret)
        .route("/sessions/:id/view", get(get_session_public))
        // SSE endpoint for real-time session updates
        .route("/sessions/:id/events", get(super::sse::session_events))
        // Frontend-assisted payment detection (chain-validated, no auth)
        .route(
            "/sessions/:id/notify-payment",
            post(super::notify_payment::notify_payment),
        );

    // Authenticated routes (merchant API)
    let auth_routes = Router::new()
        .route("/sessions", post(create_session).get(list_sessions))
        .route("/sessions/:id", get(get_session))
        .route_layer(middleware::from_fn(sub_merchant_scope))
        .route_layer(middleware::from_fn(auth_middleware));

    // Merge public and authenticated routes
    public_routes.merge(auth_routes)
}

#[derive(Deserialize)]
pub(crate) struct ListSessionsParams {
    #[serde(flatten)]
    pagination: PaginationRequest,
    #[serde(flatten)]
    filter: SessionFilterParams,
    /// Optional network filter (e.g. "TRON", "BSC")
    network: Option<String>,
    /// Include sub-merchant sessions in results
    #[serde(default)]
    include_sub_merchants: bool,
    /// Filter to a specific sub-merchant code
    sub_merchant_code: Option<String>,
}

/// POST /v1/checkout/sessions
///
/// Creates a new payment session. Returns a `url` for redirecting the customer
/// to the hosted checkout page.
///
/// Supports `Idempotency-Key` header for safe retries.
/// If the same key is used with the same request body, returns cached response.
/// If the same key is used with different request body, returns 409 Conflict.
#[utoipa::path(
    post,
    path = "/v1/checkout/sessions",
    tag = "Checkout Sessions",
    security(("bearer_auth" = [])),
    params(
        ("Idempotency-Key" = Option<String>, Header, description = "Optional unique key for idempotent requests (recommended: UUID v4)"),
        ("X-Sub-Merchant-Code" = Option<String>, Header, description = "Sub-merchant code for PSP context switch. When provided, the session is created under the specified sub-merchant."),
    ),
    request_body = CreateSessionBody,
    responses(
        (status = 201, description = "Session created", body = SessionResponse),
        (status = 400, description = "Validation error", body = ApiErrorResponse,
            example = json!({"error": {"type": "invalid_request_error", "code": "parameter_invalid", "message": "Amount must be at least 1 USDT", "param": "amount", "doc_url": "https://ironixpay.com/guide/errors#parameter_invalid"}})),
        (status = 401, description = "Invalid API key", body = ApiErrorResponse,
            example = json!({"error": {"type": "invalid_request_error", "code": "authentication_failed", "message": "Invalid API key provided", "param": null, "doc_url": "https://ironixpay.com/guide/errors#authentication_failed"}})),
        (status = 409, description = "Idempotency conflict", body = ApiErrorResponse,
            example = json!({"error": {"type": "idempotency_error", "code": "idempotency_conflict", "message": "Idempotency key was used with a different request body", "param": null, "doc_url": "https://ironixpay.com/guide/errors#idempotency_conflict"}})),
    )
)]
pub async fn create_session(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    sub_merchant_ctx: Option<Extension<SubMerchantContext>>,
    headers: HeaderMap,
    AppJson(body): AppJson<CreateSessionBody>,
) -> axum::response::Response {
    let merchant_id = merchant.id.clone();

    // Validate request body (validator crate: url, currency, pricing_currency)
    if let Err(e) = body.validate() {
        return AppError::from(e).into_response();
    }

    // Validate cross-crypto consistency: pricing_currency crypto must equal currency
    if let Err(msg) = body.validate_currency_consistency() {
        return AppError::ValidationError {
            code: E_PARAMETER_INVALID,
            message: msg,
            param: Some("pricing_currency".to_string()),
        }
        .into_response();
    }

    // Validate and parse pricing_amount (positive, precision, range)
    let parsed_amount = match body.validate_amount() {
        Ok(d) => d,
        Err(msg) => {
            return AppError::ValidationError {
                code: E_PARAMETER_INVALID,
                message: msg,
                param: Some("pricing_amount".to_string()),
            }
            .into_response();
        }
    };

    // Settlement token is always body.currency (validated as crypto by validator)
    let settle_currency = body.currency.to_uppercase();

    // Validate USDC support on the selected network
    if settle_currency == "USDC" {
        let network_params = body.network.chain_config(&merchant.environment);
        if body.network == Network::Tron {
            return AppError::ValidationError {
                code: E_PARAMETER_INVALID,
                message: "USDC is not supported on TRON network.".to_string(),
                param: Some("currency".to_string()),
            }
            .into_response();
        }
        if merchant.environment == crate::entity::Environment::Sandbox {
            return AppError::ValidationError {
                code: E_PARAMETER_INVALID,
                message: "USDC is not supported in Sandbox environment.".to_string(),
                param: Some("currency".to_string()),
            }
            .into_response();
        }
        if network_params.usdc_contract.is_none() {
            return AppError::ValidationError {
                code: E_PARAMETER_INVALID,
                message: format!("USDC is not configured for network {:?}.", body.network),
                param: Some("currency".to_string()),
            }
            .into_response();
        }
    }

    // Check for Idempotency-Key header
    let idempotency_key = headers
        .get(IDEMPOTENCY_KEY_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // If idempotency key provided, check for cached response
    if let Some(ref key) = idempotency_key {
        let request_body = serde_json::to_vec(&body).unwrap_or_default();

        match check_idempotency(
            &state.db,
            &merchant_id,
            key,
            "/v1/checkout/sessions",
            &request_body,
        )
        .await
        {
            Ok(IdempotencyResult::Cached { status_code, body }) => {
                return cached_response(status_code, &body);
            }
            Ok(IdempotencyResult::Conflict) => {
                return conflict_response();
            }
            Ok(IdempotencyResult::Processing) => {
                return processing_response();
            }
            Ok(IdempotencyResult::Proceed { .. }) => {
                // Continue with request
            }
            Ok(IdempotencyResult::NoKey) => {
                // Continue with request (shouldn't happen if key is Some)
            }
            Err(e) => {
                warn!(error = %e, "Idempotency check failed, proceeding without cache");
                // Continue with request even if idempotency check fails
            }
        }
    }

    // Validate the requested network is enabled for this instance
    if !state.enabled_networks.contains(&body.network) {
        return AppError::ValidationError {
            code: E_PARAMETER_INVALID,
            message: format!(
                "The requested network '{}' is not supported. Supported networks: {:?}",
                body.network.as_str(),
                state
                    .enabled_networks
                    .iter()
                    .map(|n| n.as_str())
                    .collect::<Vec<_>>()
            ),
            param: Some("network".to_string()),
        }
        .into_response();
    }

    // Runtime health check — is the chain's indexer alive and processing?
    if !state.chain_health.is_healthy(&body.network) {
        return AppError::ServiceUnavailable(format!(
            "The {} network is temporarily unavailable. Please try again later.",
            body.network.as_str()
        ))
        .into_response();
    }

    // Build CreateSessionRequest — branch on crypto vs fiat pricing
    let is_fiat_mode = body.is_fiat_pricing();
    let pricing_currency_upper = body.pricing_currency.to_uppercase();

    let (amount_expected, pricing_amount, pricing_currency) = if is_fiat_mode {
        (
            0_i64, // placeholder — service will compute from exchange rate
            Some(parsed_amount),
            pricing_currency_upper,
        )
    } else {
        // Crypto mode: convert standard units to microunits directly
        let micro = crate::api::dtos::checkout::to_micro(parsed_amount, &settle_currency)
            .ok_or_else(|| "Amount overflow when converting to microunits.")
            .map_err(|msg| AppError::ValidationError {
                code: E_PARAMETER_INVALID,
                message: msg.to_string(),
                param: Some("pricing_amount".to_string()),
            });
        let micro = match micro {
            Ok(m) => m,
            Err(e) => return e.into_response(),
        };

        (micro, Some(parsed_amount), pricing_currency_upper)
    };

    let req = CreateSessionRequest {
        merchant_id: merchant_id.clone(),
        amount_expected,
        currency: settle_currency,
        environment: merchant.environment.clone(),
        network: body.network.clone(),
        client_reference_id: body.client_reference_id,
        success_url: body.success_url,
        cancel_url: body.cancel_url,
        pricing_amount,
        pricing_currency,
        // Use validated SubMerchantContext from middleware (not raw header)
        sub_merchant_code: sub_merchant_ctx.map(|Extension(ctx)| ctx.sub_merchant_code),
    };

    let result = state.checkout_service.create_session(req).await;

    // Build response
    let (status_code, response_body) = match result {
        Ok(session) => {
            // Async trigger address pool replenishment (fire-and-forget)
            // Uses recommended thresholds: 20 low watermark, 50 batch size
            // Derive network from the session's stored network string
            let session_network =
                Network::from_str_lenient(&session.network).unwrap_or(Network::Tron);
            state.address_manager.clone().trigger_replenish(
                session.merchant_id.clone(),
                session_network,
                merchant.environment.clone(),
                20, // threshold (low watermark)
                50, // batch_size
            );

            // Fetch merchant for display name + logo
            let merchant_model = Merchants::find_by_id(&session.merchant_id)
                .one(&state.db)
                .await
                .ok()
                .flatten();
            let merchant_name = merchant_model
                .as_ref()
                .map(|m| m.name.clone())
                .unwrap_or_else(|| "Unknown Merchant".to_string());
            let merchant_logo_url = merchant_model.as_ref().and_then(|m| m.logo_url.clone());

            let response = SessionResponse::from_model(
                &session,
                SessionBuildContext {
                    livemode: Network::is_livemode_env(
                        &state.config.environment.to_entity_environment(),
                    ),
                    merchant_name,
                    checkout_base_url: state.config.checkout_base_url.clone(),
                    currency_contract: None,
                    detection_rpc_url: None,
                    chain_family: None,
                    transactions: vec![],
                    merchant_logo_url,
                },
            );
            (
                StatusCode::CREATED,
                serde_json::to_value(&response).unwrap(),
            )
        }
        Err(e) => {
            // Alert on address pool exhaustion — this blocks all new payments
            if matches!(
                e,
                crate::services::checkout::CheckoutError::AddressPoolExhausted
            ) {
                use crate::services::alerting::AlertLevel;
                state.alerting_service.send_alert(
                    "address_pool_exhausted",
                    AlertLevel::Critical,
                    &format!(
                        "🚨 Address pool exhausted for merchant={} network={}. New sessions are blocked (503)!",
                        merchant_id, body.network.as_str()
                    ),
                );
            }
            AppError::from(e).to_status_and_body()
        }
    };

    // Cache response if idempotency key was provided
    if let Some(ref key) = idempotency_key {
        // [MODIFIED] Removed the logic that deletes idempotency keys on server error.
        // It is safer to cache the 500 error than to risk duplicate operations on retry.
        if let Err(e) =
            update_idempotency_response(&state.db, &merchant_id, key, status_code, &response_body)
                .await
        {
            warn!(error = %e, "Failed to cache idempotency response");
        }
    }

    // Build HTTP response
    axum::response::Response::builder()
        .status(status_code)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            serde_json::to_string(&response_body).unwrap_or_default(),
        ))
        .unwrap()
}

/// GET /v1/checkout/sessions/:id
///
/// Retrieves the details of an existing session, including associated
/// on-chain transactions.
#[utoipa::path(
    get,
    path = "/v1/checkout/sessions/{id}",
    tag = "Checkout Sessions",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Session ID (e.g. `cs_abc123def456`)"),
    ),
    responses(
        (status = 200, description = "Session found", body = SessionResponse),
        (status = 401, description = "Invalid API key", body = ApiErrorResponse,
            example = json!({"error": {"type": "invalid_request_error", "code": "authentication_failed", "message": "Invalid API key provided", "param": null, "doc_url": "https://ironixpay.com/guide/errors#authentication_failed"}})),
        (status = 404, description = "Session not found", body = ApiErrorResponse,
            example = json!({"error": {"type": "invalid_request_error", "code": "resource_missing", "message": "No such session: 'cs_abc123'", "param": null, "doc_url": "https://ironixpay.com/guide/errors#resource_missing"}})),
    )
)]
pub async fn get_session(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Path(session_id): Path<String>,
) -> Result<Json<SessionResponse>, AppError> {
    // [OPTIMIZED] Use find_with_related to fetch session and transactions efficiently
    // Note: find_with_related returns (Model, Vec<RelatedModel>)
    let session_data = checkout_sessions::Entity::find_by_id(&session_id)
        .find_with_related(transactions::Entity)
        .all(&state.db)
        .await?;

    // Since we're finding by ID, we expect 0 or 1 result from the base query
    let (session, mut txs) = match session_data.into_iter().next() {
        Some(data) => data,
        None => return Err(AppError::NotFound("Session not found".to_string())),
    };

    // IDOR Protection: Ensure session belongs to the merchant or their sub-merchants
    let allowed_ids = state
        .sub_merchant_service
        .get_all_child_org_ids(&merchant.id)
        .await
        .unwrap_or_default();
    let is_own = session.merchant_id == merchant.id;
    let is_child = allowed_ids.contains(&session.merchant_id);
    if !is_own && !is_child {
        tracing::warn!(
            session_id = %session_id,
            request_merchant = %merchant.id,
            session_merchant = %session.merchant_id,
            "Session access denied: merchant mismatch"
        );
        return Err(AppError::NotFound("Session not found".to_string()));
    }

    // Environment isolation is guaranteed by:
    // 1. Per-environment DB isolation (ironixpay_prod / ironixpay_sandbox)
    // 2. merchant_id ownership check above (L285)

    // Sort transactions by created_at (ascending) to show payment history timeline
    txs.sort_by_key(|t| t.created_at);

    // Map internal transactions to API response
    let transactions: Vec<TransactionResponse> = txs
        .into_iter()
        .map(|t| TransactionResponse {
            network: t.network,
            tx_hash: t.tx_hash,
            amount: crate::api::dtos::checkout::from_micro(t.amount, &session.currency),
            status: format!("{:?}", t.status),
            created_at: t.created_at.to_rfc3339(),
        })
        .collect();

    // Fetch merchant for display name + logo
    let merchant_model = Merchants::find_by_id(&session.merchant_id)
        .one(&state.db)
        .await?;
    let merchant_name = merchant_model
        .as_ref()
        .map(|m| m.name.clone())
        .unwrap_or_else(|| "Unknown Merchant".to_string());
    let merchant_logo_url = merchant_model.as_ref().and_then(|m| m.logo_url.clone());

    let response = SessionResponse::from_model(
        &session,
        SessionBuildContext {
            livemode: Network::is_livemode_env(&state.config.environment.to_entity_environment()),
            merchant_name,
            checkout_base_url: state.config.checkout_base_url.clone(),
            currency_contract: None,
            detection_rpc_url: None,
            chain_family: None,
            transactions,
            merchant_logo_url,
        },
    );
    Ok(Json(response))
}

/// GET /v1/checkout/sessions/:id/view
///
/// Public endpoint for checkout frontend to display session details.
/// No authentication required - the session ID itself acts as a secret token.
/// Does NOT include sensitive merchant information.
async fn get_session_public(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionResponse>, AppError> {
    // Fetch session with transactions
    let session_data = checkout_sessions::Entity::find_by_id(&session_id)
        .find_with_related(transactions::Entity)
        .all(&state.db)
        .await?;

    let (session, mut txs) = match session_data.into_iter().next() {
        Some(data) => data,
        None => return Err(AppError::NotFound("Session not found".to_string())),
    };

    // Sort transactions by created_at (ascending)
    txs.sort_by_key(|t| t.created_at);

    let transactions: Vec<TransactionResponse> = txs
        .into_iter()
        .map(|t| TransactionResponse {
            network: t.network,
            tx_hash: t.tx_hash,
            amount: crate::api::dtos::checkout::from_micro(t.amount, &session.currency),
            status: format!("{:?}", t.status),
            created_at: t.created_at.to_rfc3339(),
        })
        .collect();

    // Fetch merchant for display name + logo
    let merchant_model = Merchants::find_by_id(&session.merchant_id)
        .one(&state.db)
        .await?;
    let merchant_name = merchant_model
        .as_ref()
        .map(|m| m.name.clone())
        .unwrap_or_else(|| "Merchant".to_string());
    let merchant_logo_url = merchant_model.as_ref().and_then(|m| m.logo_url.clone());

    // Resolve detection fields from ChainConfig
    let (currency_contract, detection_rpc_url, chain_family) =
        if let Some(network) = Network::from_str_lenient(&session.network) {
            let env = state.config.environment.to_entity_environment();
            let cc = network.chain_config(&env);
            let family = match network.chain_family() {
                crate::entity::network::ChainFamily::Tron => "tron",
                crate::entity::network::ChainFamily::Evm => "evm",
                crate::entity::network::ChainFamily::Solana => "solana",
            };
            let contract = match session.currency.as_str() {
                "USDC" => cc.usdc_contract.unwrap_or_default(),
                _ => cc.usdt_contract,
            };
            (
                Some(contract),
                Some(cc.detection_rpc_url.to_string()),
                Some(family.to_string()),
            )
        } else {
            (None, None, None)
        };

    let response = SessionResponse::from_model(
        &session,
        SessionBuildContext {
            livemode: Network::is_livemode_env(&state.config.environment.to_entity_environment()),
            merchant_name,
            checkout_base_url: state.config.checkout_base_url.clone(),
            currency_contract,
            detection_rpc_url,
            chain_family,
            transactions,
            merchant_logo_url,
        },
    );
    Ok(Json(response))
}

/// GET /v1/checkout/sessions
///
/// Lists sessions for the authenticated merchant, with optional filtering.
#[utoipa::path(
    get,
    path = "/v1/checkout/sessions",
    tag = "Checkout Sessions",
    security(("bearer_auth" = [])),
    params(
        ("page" = Option<u64>, Query, description = "Page number (default: 1)"),
        ("page_size" = Option<u64>, Query, description = "Items per page (default: 20, max: 100)"),
        ("search_text" = Option<String>, Query, description = "Search by session ID, tx hash, or pay address"),
        ("status" = Option<String>, Query, description = "Filter by status (e.g. `Pending`, `Paid`, `Expired`). Supports multiple values via repeated parameter."),
        ("network" = Option<String>, Query, description = "Filter by network (e.g. `TRON`, `BSC`, `ETHEREUM`)"),
        ("created_after" = Option<String>, Query, description = "Filter sessions created after this ISO 8601 datetime"),
        ("created_before" = Option<String>, Query, description = "Filter sessions created before this ISO 8601 datetime"),
        ("include_sub_merchants" = Option<bool>, Query, description = "Include sub-merchant sessions in results"),
        ("sub_merchant_code" = Option<String>, Query, description = "Filter to a specific sub-merchant code"),
    ),
    responses(
        (status = 200, description = "Paginated sessions", body = Vec<SessionResponse>),
        (status = 401, description = "Invalid API key", body = ApiErrorResponse,
            example = json!({"error": {"type": "invalid_request_error", "code": "authentication_failed", "message": "Invalid API key provided", "param": null, "doc_url": "https://ironixpay.com/guide/errors#authentication_failed"}})),
    )
)]
pub(crate) async fn list_sessions(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
    Query(params): Query<ListSessionsParams>,
) -> Result<Json<PaginatedResponse<SessionResponse>>, AppError> {
    params.pagination.validate()?;

    // Resolve merchant_ids based on sub-merchant filter (opt-in)
    let (merchant_ids, _code_map) = state
        .sub_merchant_service
        .resolve_merchant_ids(
            &merchant.id,
            params.include_sub_merchants,
            params.sub_merchant_code.as_deref(),
        )
        .await?;

    // Fetch merchant model for name + logo
    let merchant_model = Merchants::find_by_id(&merchant.id).one(&state.db).await?;
    let merchant_name = merchant_model
        .as_ref()
        .map(|m| m.name.clone())
        .unwrap_or_else(|| "Unknown Merchant".to_string());
    let merchant_logo_url = merchant_model.as_ref().and_then(|m| m.logo_url.clone());

    // Call service with pagination and filters
    // NOTE: search_text is passed from pagination struct to avoid serde::flatten conflict
    // Parse optional network filter from query params
    let network_filter = params.network.as_deref().and_then(|n| {
        if n.is_empty() || n == "all" {
            None
        } else {
            Network::from_str_lenient(n)
        }
    });
    let (sessions, total) = state
        .checkout_service
        .list_sessions(
            &merchant_ids,
            network_filter,
            merchant.environment,
            &params.pagination,
            &params.filter,
            params.pagination.search_text.as_deref(), // Pass search_text explicitly
        )
        .await?;

    let livemode = Network::is_livemode_env(&state.config.environment.to_entity_environment());
    let session_responses: Vec<SessionResponse> = sessions
        .iter()
        .map(|s| {
            SessionResponse::from_model(
                s,
                SessionBuildContext {
                    livemode,
                    merchant_name: merchant_name.clone(),
                    checkout_base_url: state.config.checkout_base_url.clone(),
                    currency_contract: None,
                    detection_rpc_url: None,
                    chain_family: None,
                    transactions: vec![],
                    merchant_logo_url: merchant_logo_url.clone(),
                },
            )
        })
        .collect();

    Ok(Json(PaginatedResponse::new(
        session_responses,
        total,
        params.pagination.page,
        params.pagination.page_size,
    )))
}
