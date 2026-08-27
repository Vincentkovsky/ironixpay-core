use crate::entity::{webhook_endpoints, webhook_events};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Serialize)]
pub struct WebhookConfigResponse {
    pub url: String,
    /// Masked secret (e.g., "whsec_***...abcd")
    pub secret: String,
    pub status: String,
    pub description: Option<String>,
    pub created_at: String,
}

#[derive(Deserialize, Validate)]
pub struct UpdateWebhookConfigRequest {
    // Optional: when None, keeps existing URL (for status-only updates).
    // When Some(""), validator rejects as invalid URL format.
    #[validate(url)]
    pub url: Option<String>,
    pub status: Option<String>, // "enabled" or "disabled"
}

/// Response for rotate-secret endpoint, returning the new plaintext secret once.
#[derive(Serialize)]
pub struct RotateSecretResponse {
    pub secret: String,
}

#[derive(Deserialize)]
pub struct WebhookLogFilter {
    #[serde(flatten)]
    pub pagination: crate::api::dtos::pagination::PaginationRequest,
    pub source_id: Option<String>,
}

#[derive(Serialize)]
pub struct WebhookLogResponse {
    pub id: String,
    pub event_type: String,
    pub created_at: String,
    pub target_url: String,
    pub request_payload: serde_json::Value,
    pub status: webhook_events::WebhookEventStatus,
    pub http_status: Option<i32>,
    pub next_retry_at: Option<String>,
}

impl From<webhook_endpoints::Model> for WebhookConfigResponse {
    fn from(model: webhook_endpoints::Model) -> Self {
        Self {
            url: model.url,
            secret: "whsec_****************".to_string(),
            status: match model.status {
                webhook_endpoints::EndpointStatus::Enabled => "enabled".to_string(),
                webhook_endpoints::EndpointStatus::Disabled => "disabled".to_string(),
            },
            description: model.description,
            created_at: model.created_at.to_rfc3339(),
        }
    }
}

impl From<webhook_events::Model> for WebhookLogResponse {
    fn from(model: webhook_events::Model) -> Self {
        Self {
            id: model.id,
            event_type: model.event_type,
            created_at: model.created_at.to_rfc3339(),
            target_url: model.target_url,
            request_payload: model.payload,
            status: model.status,
            http_status: model.http_status_code,
            next_retry_at: model.next_retry_at.map(|dt| dt.to_rfc3339()),
        }
    }
}
