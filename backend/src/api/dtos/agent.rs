//! Agent DTOs — Request/Response types for agent management APIs.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Admin: Create a new agent from an existing merchant
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateAgentRequest {
    /// Merchant ID to promote to agent
    pub merchant_id: String,
    /// Override base rate (e.g. "0.001" for 0.1%), default: 0.1%
    pub base_rate: Option<String>,
    /// Override max markup (e.g. "0.01" for 1.0%), default: 1.0%
    pub max_markup: Option<String>,
    /// Override default merchant rate (e.g. "0.008" for 0.8%), default: 0.8%
    pub default_merchant_rate: Option<String>,
}

/// Admin: Update agent configuration
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateAgentRequest {
    pub base_rate: Option<String>,
    pub max_markup: Option<String>,
    pub default_merchant_rate: Option<String>,
    /// "active" or "suspended"
    pub status: Option<String>,
}

/// Agent profile response
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentResponse {
    pub id: String,
    pub merchant_id: String,
    pub merchant_name: String,
    pub referral_code: String,
    pub base_rate: String,
    pub max_markup: String,
    pub default_merchant_rate: String,
    pub status: String,
    pub referred_merchant_count: u64,
    pub created_at: String,
}

/// Commission report query parameters
#[derive(Debug, Deserialize, ToSchema)]
pub struct CommissionQuery {
    /// ISO date: YYYY-MM-DD (defaults to 30 days ago)
    pub start_date: Option<String>,
    /// ISO date: YYYY-MM-DD (defaults to today)
    pub end_date: Option<String>,
}
