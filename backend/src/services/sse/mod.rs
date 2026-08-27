//! SSE (Server-Sent Events) Broadcaster Service
//!
//! Provides real-time event broadcasting to connected SSE clients.
//! Uses tokio::sync::broadcast for efficient multi-consumer messaging.

mod broadcaster;

pub use broadcaster::create_sse_stream;
pub use broadcaster::SseBroadcaster;
pub use broadcaster::SseEvent;
