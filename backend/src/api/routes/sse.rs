//! SSE Routes for real-time session updates
//!
//! Provides Server-Sent Events endpoint for checkout pages to receive
//! real-time payment status updates instead of polling.

use axum::{
    extract::{Path, State},
    response::sse::{Event, Sse},
};
use std::convert::Infallible;
use tokio_stream::Stream;

use crate::services::{sse::create_sse_stream, AppState};

/// SSE endpoint for session events
///
/// GET /v1/checkout/sessions/{id}/events
///
/// Streams real-time updates for a checkout session including:
/// - `session_updated`: When payment status changes
/// - Heartbeat comments every 30s to keep connection alive
///
/// The frontend should connect via EventSource and fall back to polling on error.
/// The stream will terminate gracefully when the server shuts down.
pub async fn session_events(
    Path(session_id): Path<String>,
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = create_sse_stream(
        state.sse_broadcaster.clone(),
        session_id,
        state.cancel_token.clone(),
    );
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keep-alive"),
    )
}
