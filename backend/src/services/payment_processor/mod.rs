//! Payment Event Processor Service
//!
//! Implements the consumer side of the Transactional Outbox pattern.
//! Polls payment_events table and updates checkout_sessions status.
//! This ensures CheckoutService is the sole owner of session state updates.

pub mod service;

pub use service::PaymentEventProcessor;
