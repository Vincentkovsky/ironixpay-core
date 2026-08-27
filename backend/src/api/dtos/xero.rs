//! Xero Integration DTOs

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

// ─── Request DTOs ───

#[derive(Debug, Deserialize, Serialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct XeroConnectRequest {
    /// Optional: pre-selected environment override (defaults to merchant's current env)
    pub environment: Option<String>,
    /// Force re-auth even when an active connection already exists.
    pub force_reauth: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct XeroCallbackRequest {
    pub code: String,
    pub state: String,
}

#[derive(Debug, Deserialize, Serialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct XeroSelectTenantRequest {
    /// Tenant ID selected by the user.
    pub tenant_id: String,
}

#[derive(Debug, Deserialize, Serialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct XeroUpdateConnectionRequest {
    /// None => no change, Some(None) => clear, Some(Some(v)) => set
    pub xero_account_code: Option<Option<String>>,
    /// None => no change, Some(None) => clear, Some(Some(v)) => set
    pub xero_fee_account_code: Option<Option<String>>,
    /// None => no change, Some(None) => clear, Some(Some(v)) => set
    pub xero_payment_account_code: Option<Option<String>>,
    /// None => no change, Some(None) => reset to NONE, Some(Some(v)) => set
    pub xero_tax_type: Option<Option<String>>,
    pub auto_sync_enabled: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct XeroSyncLogsQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub session_id: Option<String>,
}

// ─── Response DTOs ───

#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct XeroConnectResponse {
    pub authorize_url: String,
}

#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct XeroCapabilityResponse {
    pub enabled: bool,
}

#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct XeroCallbackResponse {
    /// If multiple tenants found, client must call select-tenant
    pub tenants: Vec<XeroTenantDto>,
    /// Set if only one tenant (auto-selected)
    pub connection_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct XeroTenantDto {
    pub tenant_id: String,
    pub tenant_name: String,
    pub tenant_type: String,
}

#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct XeroConnectionResponse {
    pub id: Uuid,
    pub environment: String,
    pub xero_tenant_id: String,
    pub xero_tenant_name: Option<String>,
    pub xero_account_code: Option<String>,
    pub xero_fee_account_code: Option<String>,
    pub xero_payment_account_code: Option<String>,
    pub xero_tax_type: String,
    pub default_currency: String,
    pub auto_sync_enabled: bool,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct XeroSelectTenantResponse {
    pub connection_id: Uuid,
}

#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct XeroAccountDto {
    pub account_id: String,
    pub code: String,
    pub name: String,
    pub r#type: String,
    pub class: String,
    pub enable_payments: bool,
}

#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct XeroTaxRateDto {
    pub tax_type: String,
    pub name: String,
    pub display_tax_rate: f64,
    pub can_apply_to_revenue: bool,
    pub status: String,
}

#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct XeroSyncLogResponse {
    pub id: Uuid,
    pub session_id: String,
    pub xero_invoice_id: Option<String>,
    pub xero_payment_id: Option<String>,
    pub status: String,
    pub attempt_count: i32,
    pub last_error: Option<String>,
    pub next_retry_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<crate::entity::xero_connections::Model> for XeroConnectionResponse {
    fn from(m: crate::entity::xero_connections::Model) -> Self {
        Self {
            id: m.id,
            environment: m.environment.to_string(),
            xero_tenant_id: m.xero_tenant_id,
            xero_tenant_name: m.xero_tenant_name,
            xero_account_code: m.xero_account_code,
            xero_fee_account_code: m.xero_fee_account_code,
            xero_payment_account_code: m.xero_payment_account_code,
            xero_tax_type: m.xero_tax_type,
            default_currency: m.default_currency,
            auto_sync_enabled: m.auto_sync_enabled,
            status: m.status.to_string(),
            created_at: m.created_at.to_rfc3339(),
            updated_at: m.updated_at.to_rfc3339(),
        }
    }
}

impl From<crate::entity::xero_sync_logs::Model> for XeroSyncLogResponse {
    fn from(m: crate::entity::xero_sync_logs::Model) -> Self {
        Self {
            id: m.id,
            session_id: m.session_id,
            xero_invoice_id: m.xero_invoice_id,
            xero_payment_id: m.xero_payment_id,
            status: m.status.to_string(),
            attempt_count: m.attempt_count,
            last_error: m.last_error,
            next_retry_at: m.next_retry_at.map(|t| t.to_rfc3339()),
            created_at: m.created_at.to_rfc3339(),
            updated_at: m.updated_at.to_rfc3339(),
        }
    }
}
