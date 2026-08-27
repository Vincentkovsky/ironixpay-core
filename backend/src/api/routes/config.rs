//! Config API Routes
//!
//! Exposes platform configuration to the dashboard frontend.
//! JWT auth is applied by the parent router (`/api/internal`).

use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use std::collections::BTreeMap;

use crate::api::middleware::auth::AuthenticatedMerchant;
use crate::services::billing::fee_config::FeeConfig;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/fees", get(get_fee_config))
}
use crate::entity::merchants::{FeeSource, FeeTier};

#[derive(Serialize)]
struct FeeConfigResponse {
    /// Global deposit fee percentage (e.g. "0.5" for 0.5%)
    deposit_fee_percentage: String,
    /// Merchant's custom fee percentage if set (e.g. "0.5" for 0.5%), null if using default
    custom_fee_percentage: Option<String>,
    /// Effective fee percentage for this merchant (e.g. "0.5" for 0.5%)
    effective_fee_percentage: String,
    /// Per-network outbound fee in USDT (e.g. "1.50")
    outbound_fees: BTreeMap<String, String>,
    /// Per-network deposit fee floor in USDT (e.g. "0.10")
    deposit_floors: BTreeMap<String, String>,
    /// Current pricing tier: "standard", "business", "enterprise"
    fee_tier: FeeTier,
    /// Who/what set the fee: "default", "auto_tier", "manual", "agent"
    fee_source: FeeSource,
    /// When the new-merchant promo ends (ISO 8601), null if already expired
    first_month_ends_at: Option<String>,
}

/// GET /api/internal/config/fees
///
/// Returns per-network outbound fees and deposit fee percentage.
/// Includes merchant-specific custom fee percentage if configured.
async fn get_fee_config(
    State(state): State<AppState>,
    merchant: AuthenticatedMerchant,
) -> Json<FeeConfigResponse> {
    let fee_config = FeeConfig::default();
    let default_fee = fee_config.flat_payout_fee;

    // Build per-network fee map from chains.toml
    let mut outbound_fees = BTreeMap::new();
    let mut deposit_floors = BTreeMap::new();
    for network in &state.enabled_networks {
        let chain_key = network.as_str().to_string();
        let fee_microunits = if *network == crate::entity::Network::Solana {
            state.config.solana.as_ref().and_then(|s| s.outbound_fee)
        } else {
            state
                .config
                .chains
                .get(network.as_str())
                .and_then(|c| c.outbound_fee)
        }
        .unwrap_or(default_fee);
        let fee_usdt = crate::api::dtos::checkout::from_micro(fee_microunits, "USDT");
        outbound_fees.insert(chain_key.clone(), fee_usdt);

        // Deposit floor: per-chain override or global default
        let floor_microunits = if *network == crate::entity::Network::Solana {
            state.config.solana.as_ref().and_then(|s| s.floor_deposit)
        } else {
            state
                .config
                .chains
                .get(network.as_str())
                .and_then(|c| c.floor_deposit)
        }
        .unwrap_or(fee_config.floor_deposit);
        let floor_usdt = crate::api::dtos::checkout::from_micro(floor_microunits, "USDT");
        deposit_floors.insert(chain_key, floor_usdt);
    }

    use rust_decimal::prelude::ToPrimitive;
    let global_pct = fee_config.fee_percentage.to_f64().unwrap_or(0.005) * 100.0;

    // Look up merchant's full record for fee fields
    use crate::entity::Merchants;
    use sea_orm::EntityTrait;
    let merchant_record = Merchants::find_by_id(&merchant.id)
        .one(&state.db)
        .await
        .ok()
        .flatten();

    let merchant_custom_pct = merchant_record
        .as_ref()
        .and_then(|m| m.custom_fee_percentage);

    let custom_display = merchant_custom_pct.map(|d| {
        let pct_f64 = d.to_f64().unwrap_or(0.0) * 100.0;
        format!("{:.1}", pct_f64)
    });

    let effective_pct = merchant_custom_pct
        .map(|d| d.to_f64().unwrap_or(0.005) * 100.0)
        .unwrap_or(global_pct);

    // Tier info — serde(rename_all = "snake_case") on the enums handles correct serialization
    let fee_tier = merchant_record
        .as_ref()
        .map(|m| m.fee_tier.clone())
        .unwrap_or(crate::entity::merchants::FeeTier::Business);
    let fee_source = merchant_record
        .as_ref()
        .map(|m| m.fee_source.clone())
        .unwrap_or(crate::entity::merchants::FeeSource::Default);
    let first_month_ends_at = merchant_record
        .as_ref()
        .and_then(|m| m.first_month_ends_at)
        .filter(|ends| chrono::Utc::now() < ends.with_timezone(&chrono::Utc))
        .map(|ends| ends.to_rfc3339());

    Json(FeeConfigResponse {
        deposit_fee_percentage: format!("{:.1}", global_pct),
        custom_fee_percentage: custom_display,
        effective_fee_percentage: format!("{:.1}", effective_pct),
        outbound_fees,
        deposit_floors,
        fee_tier,
        fee_source,
        first_month_ends_at,
    })
}
