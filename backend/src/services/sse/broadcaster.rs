//! SSE Broadcaster Implementation
//!
//! Uses tokio::sync::broadcast for multi-consumer message distribution.
//! Each session_id has its own broadcast channel.

use axum::response::sse::Event;
use dashmap::DashMap;
use serde::Serialize;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_stream::{wrappers::BroadcastStream, Stream, StreamExt};

/// Channel capacity for each session's broadcast channel
const CHANNEL_CAPACITY: usize = 100;

/// Heartbeat interval in seconds
const HEARTBEAT_INTERVAL_SECS: u64 = 30;

/// SSE Event types that can be broadcast to clients
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseEvent {
    /// Payment has been detected (0-confirmation, awaiting blockchain confirmation)
    PaymentDetected { tx_hash: String, amount: String },
    /// Session status has been updated (payment confirmed)
    SessionUpdated {
        status: String,
        amount_received: String,
        /// ISO 8601 expiry timestamp (updated on rolling extension for underpaid)
        #[serde(skip_serializing_if = "Option::is_none")]
        expires_at: Option<String>,
    },
    // Heartbeat is generated directly in stream merge, not as enum variant
}

/// SSE Broadcaster for distributing events to connected clients
///
/// Thread-safe broadcaster using DashMap for concurrent access.
/// Each session_id gets its own broadcast channel.
pub struct SseBroadcaster {
    /// Map of session_id -> broadcast sender
    channels: DashMap<String, broadcast::Sender<SseEvent>>,
}

impl Default for SseBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

impl SseBroadcaster {
    /// Create a new broadcaster
    pub fn new() -> Self {
        Self {
            channels: DashMap::new(),
        }
    }

    /// Subscribe to events for a specific session
    ///
    /// Creates a new channel if one doesn't exist for this session.
    /// Returns a broadcast receiver.
    pub fn subscribe(&self, session_id: &str) -> broadcast::Receiver<SseEvent> {
        let tx = self
            .channels
            .entry(session_id.to_string())
            .or_insert_with(|| {
                let (tx, _rx) = broadcast::channel(CHANNEL_CAPACITY);
                tx
            })
            .value()
            .clone();

        tx.subscribe()
    }

    /// Broadcast an event to all subscribers of a session
    ///
    /// If no subscribers exist, the event is silently dropped.
    /// NOTE: Cleanup is handled by cleanup_idle_channels, NOT here,
    /// to avoid race conditions with concurrent subscriptions.
    pub fn broadcast(&self, session_id: &str, event: SseEvent) {
        if let Some(tx) = self.channels.get(session_id) {
            // Ignore send errors (no subscribers is acceptable)
            let _ = tx.send(event);
            // ⚠️ No cleanup here - rely on cleanup_idle_channels to avoid race condition
        }
    }

    /// Get the number of active channels (for monitoring)
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Clean up idle channels with no subscribers
    ///
    /// Should be called periodically to prevent memory leaks.
    /// DashMap's retain is safe - it locks shards atomically.
    pub fn cleanup_idle_channels(&self) {
        self.channels.retain(|_session_id, tx| {
            // Keep channels that still have receivers
            tx.receiver_count() > 0
        });
    }

    /// Get heartbeat interval duration
    pub fn heartbeat_interval() -> Duration {
        Duration::from_secs(HEARTBEAT_INTERVAL_SECS)
    }
}

/// Create an SSE event stream with heartbeat and graceful shutdown support
///
/// Uses BroadcastStream + merge pattern to avoid manual Stream implementation
/// that could cause CPU busy loops. This is the safe, idiomatic approach.
///
/// The stream will terminate when the cancellation token is triggered,
/// allowing graceful HTTP server shutdown.
pub fn create_sse_stream(
    broadcaster: Arc<SseBroadcaster>,
    session_id: String,
    cancel_token: tokio_util::sync::CancellationToken,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let rx = broadcaster.subscribe(&session_id);

    // 1. Business event stream - converts SseEvent to Axum SSE Event
    let event_stream = BroadcastStream::new(rx)
        .filter_map(|result| {
            // Filter out Lagged errors (message backlog), only process valid messages
            result.ok().map(|sse_event| {
                Event::default()
                    .event("session_updated")
                    .json_data(&sse_event)
                    .unwrap_or_else(|_| Event::default().comment("serialization error"))
            })
        })
        .map(Ok); // Wrap in Result

    // 2. Heartbeat stream - sends keep-alive comments at fixed interval
    let interval = tokio::time::interval(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));
    let heartbeat_stream = tokio_stream::wrappers::IntervalStream::new(interval)
        .map(|_| Ok(Event::default().comment("keep-alive")));

    // 3. Merge streams - events take priority, heartbeats fill gaps
    let merged = event_stream.merge(heartbeat_stream);

    // 4. Wrap with cancellation - stream ends when token is cancelled
    async_stream::stream! {
        tokio::pin!(merged);
        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    // Send a final comment and terminate
                    yield Ok(Event::default().comment("server shutdown"));
                    break;
                }
                item = merged.next() => {
                    match item {
                        Some(event) => yield event,
                        None => break, // Stream ended naturally
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_broadcaster_subscribe_and_broadcast() {
        let broadcaster = SseBroadcaster::new();
        let session_id = "test_session";

        // Subscribe to events
        let mut rx = broadcaster.subscribe(session_id);

        // Broadcast an event
        broadcaster.broadcast(
            session_id,
            SseEvent::SessionUpdated {
                status: "Paid".to_string(),
                amount_received: "1".to_string(),
                expires_at: None,
            },
        );

        // Should receive the event
        let event = rx.recv().await;
        assert!(event.is_ok());

        match event.unwrap() {
            SseEvent::SessionUpdated {
                status,
                amount_received,
                expires_at,
            } => {
                assert_eq!(status, "Paid");
                assert_eq!(amount_received, "1");
                assert!(expires_at.is_none());
            }
            _ => panic!("Unexpected event type"),
        }
    }

    #[test]
    fn test_broadcaster_cleanup() {
        let broadcaster = SseBroadcaster::new();

        // Create a channel but don't keep subscriber
        {
            let _rx = broadcaster.subscribe("session1");
            assert_eq!(broadcaster.channel_count(), 1);
        }
        // Receiver is dropped here

        // Cleanup should remove the idle channel
        broadcaster.cleanup_idle_channels();
        assert_eq!(broadcaster.channel_count(), 0);
    }
}
