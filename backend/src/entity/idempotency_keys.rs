//! Idempotency Keys entity
//!
//! Per system_design.md §7.4: Stores request fingerprints and cached responses
//! for idempotent write operations. Records expire after 24 hours.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "idempotency_keys")]
pub struct Model {
    /// Merchant ID (part of composite primary key)
    #[sea_orm(primary_key, auto_increment = false)]
    pub merchant_id: String,

    /// Idempotency key provided by client (part of composite primary key)
    #[sea_orm(primary_key, auto_increment = false)]
    pub idempotency_key: String,

    /// Request path (e.g., /v1/checkout/sessions)
    pub request_path: String,

    /// SHA256 hash of request body for conflict detection
    pub request_hash: String,

    /// Cached HTTP response status code (0 = processing)
    pub response_code: i32,

    /// Cached HTTP response body (JSON)
    #[sea_orm(column_type = "JsonBinary")]
    pub response_body: serde_json::Value,

    /// Creation timestamp (used for 24h expiry)
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::merchants::Entity",
        from = "Column::MerchantId",
        to = "super::merchants::Column::Id"
    )]
    Merchant,
}

impl Related<super::merchants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Merchant.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Result of idempotency check
#[derive(Debug, Clone)]
pub enum IdempotencyCheckResult {
    /// No existing record, proceed with request
    Proceed,
    /// Request is currently being processed by another worker
    Processing,
    /// Found cached response, return it
    CachedResponse {
        status_code: i32,
        body: serde_json::Value,
    },
    /// Same key but different request body - conflict
    Conflict,
}
