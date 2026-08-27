//! Notify Payment Route
//!
//! Lightweight, unauthenticated endpoint for frontend-assisted payment detection.
//! The frontend polls the blockchain for USDT transfers and submits tx_hash here.
//! Backend validates the transaction is a real USDT transfer to the session's
//! pay_address, then broadcasts a `PaymentDetected` SSE event.
//!
//! This is a "radar" — it only triggers UI feedback. The Indexer remains the
//! sole authority for confirming payments and updating session state.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use tracing::{debug, warn};

use crate::entity::Network;
use crate::services::sse::SseEvent;
use crate::AppState;

#[derive(Deserialize)]
pub struct NotifyPaymentRequest {
    pub tx_hash: String,
}

/// POST /v1/checkout/sessions/:id/notify-payment
///
/// Chain-agnostic handler: dispatches validation to the correct ChainClient
/// implementation via the `validate_payment_tx` trait method.
pub async fn notify_payment(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(body): Json<NotifyPaymentRequest>,
) -> impl IntoResponse {
    // 1. Fetch session
    use crate::entity::checkout_sessions;
    use sea_orm::EntityTrait;

    let session = match checkout_sessions::Entity::find_by_id(&session_id)
        .one(&state.db)
        .await
    {
        Ok(Some(s)) => s,
        Ok(None) => {
            return StatusCode::NOT_FOUND;
        }
        Err(e) => {
            warn!(error = %e, session_id, "notify-payment: DB error");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    // 2. Skip terminal sessions
    if session.status.is_terminal() {
        return StatusCode::OK;
    }

    // 3. Resolve network and get chain client
    let network = match Network::from_str_lenient(&session.network) {
        Some(n) => n,
        None => {
            warn!(network = session.network, "notify-payment: unknown network");
            return StatusCode::BAD_REQUEST;
        }
    };

    let chain_client = match state.chain_clients.get(&network) {
        Some(client) => client.clone(),
        None => {
            warn!(?network, "notify-payment: no chain client");
            return StatusCode::BAD_REQUEST;
        }
    };

    // 4. Get USDT contract address from ChainConfig
    let env = state.config.environment.to_entity_environment();
    let chain_config = network.chain_config(&env);

    // 5. Validate: is this a real USDT transfer to the session's pay_address?
    let valid = chain_client
        .validate_payment_tx(
            &body.tx_hash,
            &session.pay_address,
            &chain_config.usdt_contract,
        )
        .await;

    if !valid {
        debug!(
            tx_hash = body.tx_hash,
            session_id, "notify-payment: validation failed"
        );
        return StatusCode::UNPROCESSABLE_ENTITY;
    }

    // 6. Broadcast PaymentDetected SSE event
    state.sse_broadcaster.broadcast(
        &session_id,
        SseEvent::PaymentDetected {
            tx_hash: body.tx_hash.clone(),
            amount: crate::api::dtos::checkout::from_micro(0, &session.currency), // Detection only — Indexer will provide the actual amount
        },
    );

    debug!(
        tx_hash = body.tx_hash,
        session_id, "notify-payment: validated, SSE broadcast sent"
    );

    StatusCode::OK
}
