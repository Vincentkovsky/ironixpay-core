use crate::entity::checkout_sessions::SessionStatus;
use crate::entity::Network;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use validator::Validate;

// ============================================================
// Amount conversion helpers
// ============================================================

/// Token on-chain precision — used for to_micro/from_micro numeric conversion.
/// All currently supported settlement tokens (USDT, USDC) use 6 decimals.
pub fn currency_decimals(currency: &str) -> u32 {
    match currency {
        "USDT" | "USDC" => 6,
        _ => 6, // Default fallback for any future tokens
    }
}

/// Maximum decimal places allowed in API input — used for DTO validation.
/// This is a business rule (payment UX), NOT the token precision.
pub fn max_input_decimals(currency: &str) -> u32 {
    match currency {
        "USDT" | "USDC" => 2, // 0.01 precision for payments
        "USD" | "EUR" | "GBP" | "CNY" | "SGD" | "HKD" | "TWD" | "RUB" => 2,
        "JPY" | "KRW" => 0, // No decimals
        _ => 2,
    }
}

/// Standard unit → microunits (e.g., "10.50" USDT → 10500000).
/// Returns None on overflow.
pub fn to_micro(amount: Decimal, currency: &str) -> Option<i64> {
    let decimals = currency_decimals(currency);
    let multiplier = Decimal::from(10_u64.pow(decimals));
    (amount * multiplier).round_dp(0).to_i64()
}

/// Microunits → standard unit string (e.g., 10500000 → "10.5").
pub fn from_micro(micro: i64, currency: &str) -> String {
    let decimals = currency_decimals(currency);
    let divisor = Decimal::from(10_u64.pow(decimals));
    (Decimal::from(micro) / divisor).normalize().to_string()
}

/// Check if a currency code is a supported crypto token.
pub fn is_crypto(currency: &str) -> bool {
    matches!(currency, "USDT" | "USDC")
}

/// Check if a currency code is a supported fiat currency.
pub fn is_fiat(currency: &str) -> bool {
    crate::services::exchange_rate::ExchangeRateService::is_supported_fiat(currency)
}

// ============================================================
// Request DTOs
// ============================================================

#[derive(Debug, Deserialize, Serialize, Clone, Validate, TS, utoipa::ToSchema)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct CreateSessionBody {
    /// The amount in `pricing_currency` units, as a decimal string.
    ///
    /// When `pricing_currency` is crypto (= `currency`): the exact token amount to collect.
    /// When `pricing_currency` is fiat: the fiat amount; the system converts
    /// using real-time exchange rates.
    ///
    /// Examples: `"10.50"`, `"100"`.
    /// Crypto: min 1.00, max 10,000,000.00, precision 0.01.
    /// Fiat: must be positive; minimum enforced after conversion (≥ 1 crypto equivalent).
    #[schema(example = "10.50")]
    pub pricing_amount: String,

    /// The pricing/denomination currency code.
    ///
    /// **Crypto** (`USDT`, `USDC`): direct token pricing — must equal `currency`.
    /// **Fiat** (`USD`, `CNY`, `EUR`, `GBP`, `JPY`, `KRW`, `SGD`, `HKD`, `TWD`, `RUB`):
    /// the system converts to the settlement token at the current exchange rate.
    #[validate(custom(function = "validate_pricing_currency"))]
    #[schema(example = "USD")]
    pub pricing_currency: String,

    /// The on-chain settlement token. This is the crypto token the customer actually pays
    /// and the merchant receives on-chain.
    ///
    /// Supported: `USDT`, `USDC`.
    /// USDC requires a compatible `network` (not TRON, not Sandbox).
    #[validate(custom(function = "validate_settle_currency"))]
    #[schema(example = "USDT")]
    pub currency: String,

    /// The blockchain network to use. All 7 networks are available in **production** (`sk_live_`).
    /// **Sandbox** (`sk_test_`) currently only supports `TRON` (mapped to TRON Nile testnet).
    /// Must match the environment of the API key used for authentication.
    pub network: Network,

    /// A unique string to reference this Checkout Session on your system, such as an order ID or cart ID.
    /// Can be used to reconcile the session with your internal records. Maximum 200 characters.
    #[schema(example = "order_20260211_001")]
    pub client_reference_id: Option<String>,

    /// The URL the customer is redirected to after a successful payment. Must be a valid, absolute URL.
    /// Optional — if omitted, the hosted checkout page will show a generic success message instead of redirecting.
    #[validate(url(message = "success_url must be a valid URL"))]
    #[schema(example = "https://example.com/success")]
    #[serde(default)]
    pub success_url: Option<String>,

    /// The URL the customer is redirected to if they cancel or the session expires. Must be a valid, absolute URL.
    /// Optional — if omitted, the hosted checkout page will show a generic expiry message instead of redirecting.
    #[validate(url(message = "cancel_url must be a valid URL"))]
    #[schema(example = "https://example.com/cancel")]
    #[serde(default)]
    pub cancel_url: Option<String>,
}

/// Validate that the settlement currency is a supported crypto token.
fn validate_settle_currency(currency: &str) -> Result<(), validator::ValidationError> {
    let upper = currency.to_uppercase();
    if !is_crypto(&upper) {
        let mut err = validator::ValidationError::new("unsupported_currency");
        err.message = Some(
            format!(
                "Unsupported settlement currency '{}'. Must be 'USDT' or 'USDC'.",
                currency,
            )
            .into(),
        );
        return Err(err);
    }
    Ok(())
}

/// Validate that the pricing currency is a supported crypto or fiat currency code.
fn validate_pricing_currency(currency: &str) -> Result<(), validator::ValidationError> {
    let upper = currency.to_uppercase();
    if !is_crypto(&upper) && !is_fiat(&upper) {
        let mut err = validator::ValidationError::new("unsupported_pricing_currency");
        err.message = Some(
            format!(
                "Unsupported pricing_currency '{}'. Supported crypto: USDT, USDC. Supported fiat: {}.",
                currency,
                crate::services::exchange_rate::ExchangeRateService::supported_fiats().join(", ")
            )
            .into(),
        );
        return Err(err);
    }
    Ok(())
}

impl CreateSessionBody {
    /// Validate the `pricing_amount` field: parse as Decimal, check positivity, range, and precision.
    pub fn validate_amount(&self) -> Result<Decimal, String> {
        let pricing_cur = self.pricing_currency.to_uppercase();

        // Parse pricing_amount string to Decimal
        let amount: Decimal = self.pricing_amount.parse().map_err(|_| {
            format!(
                "Invalid pricing_amount '{}'. Must be a valid decimal number (e.g., \"10.50\").",
                self.pricing_amount
            )
        })?;

        // Must be positive
        if amount <= Decimal::ZERO {
            return Err("pricing_amount must be positive.".to_string());
        }

        // Check decimal precision against the pricing currency
        let max_dp = max_input_decimals(&pricing_cur);
        let actual_dp = amount.scale();
        if actual_dp > max_dp {
            return Err(format!(
                "{} amounts support up to {} decimal place(s). Got '{}' with {} decimal places.",
                pricing_cur, max_dp, self.pricing_amount, actual_dp
            ));
        }

        // Crypto pricing: enforce min/max range at input
        if is_crypto(&pricing_cur) {
            if amount < Decimal::from(1) {
                return Err(format!("Minimum pricing_amount is 1.00 {}.", pricing_cur));
            }
            if amount > Decimal::from(10_000_000) {
                return Err(format!(
                    "Maximum pricing_amount is 10000000.00 {}.",
                    pricing_cur
                ));
            }
        }
        // Fiat: only positivity check here; post-conversion minimum (≥ 1 crypto) enforced in service

        Ok(amount)
    }

    /// Whether this request uses fiat pricing (pricing_currency is a fiat code).
    pub fn is_fiat_pricing(&self) -> bool {
        !is_crypto(&self.pricing_currency.to_uppercase())
    }

    /// Validate cross-crypto consistency: if pricing_currency is crypto, it must equal currency.
    pub fn validate_currency_consistency(&self) -> Result<(), String> {
        let pricing_cur = self.pricing_currency.to_uppercase();
        let settle_cur = self.currency.to_uppercase();
        if is_crypto(&pricing_cur) && pricing_cur != settle_cur {
            return Err(format!(
                "When pricing_currency is a crypto token ('{}'), it must match currency ('{}').",
                self.pricing_currency, self.currency
            ));
        }
        Ok(())
    }
}

// ============================================================
// Response DTOs
// ============================================================

#[derive(Debug, Serialize, Deserialize, Clone, TS, utoipa::ToSchema)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct TransactionResponse {
    /// The blockchain network this transaction occurred on (e.g., `"TRON"`, `"BSC"`, `"ETHEREUM"`).
    /// May differ from the session's network in cross-chain exception resolution cases.
    #[schema(example = "TRON")]
    pub network: String,
    /// The on-chain transaction hash. Can be used to look up the transaction on a block explorer (e.g. Tronscan).
    #[schema(example = "8d490254caa24a5ff0b6522976a52d90d0e0fc4187dd78659ea2b78236fc8afc")]
    pub tx_hash: String,
    /// The transfer amount in standard units (e.g., `"10.5"` = 10.5 USDT).
    #[schema(example = "10.5")]
    pub amount: String,
    /// The on-chain confirmation status: `Confirmed` or `Pending`.
    #[schema(example = "Confirmed")]
    pub status: String,
    /// The time at which the transaction was detected, in UTC (ISO 8601 format, e.g. `2026-02-11T03:56:00Z`).
    #[schema(example = "2026-02-11T03:56:00Z")]
    pub created_at: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, Validate, TS)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct SessionFilterParams {
    #[serde(
        default,
        deserialize_with = "crate::api::dtos::pagination::deserialize_option_vec_or_single"
    )]
    pub status: Option<Vec<SessionStatus>>,
    pub created_after: Option<String>,
    pub created_before: Option<String>,
    // NOTE: search_text is intentionally NOT here - it's in PaginationRequest.
    // Having it in both structs causes serde::flatten conflicts where the value
    // would be deserialized into PaginationRequest.search_text but SessionFilterParams.search_text stays None.
}

/// Pricing snapshot — always present. For crypto-only sessions, pricing_currency
/// equals the settlement currency (e.g., "USDT") and exchange_rate is "1".
#[derive(Debug, Serialize, Deserialize, Clone, TS, utoipa::ToSchema)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct PricingInfo {
    /// The pricing/denomination currency code (e.g., "USD", "CNY", or "USDT").
    #[schema(example = "USD")]
    pub currency: String,
    /// The original amount in the pricing currency (e.g., "10.50").
    #[schema(example = "10.50")]
    pub amount: String,
    /// The exchange rate at session creation (1 crypto = N fiat, or "1" for crypto-only).
    #[schema(example = "1.00000000")]
    pub exchange_rate: String,
}

// TODO: Refactor to separate DTO Enums when internal states are introduced.
#[derive(Debug, Serialize, Deserialize, Clone, TS, utoipa::ToSchema)]
#[ts(export, export_to = "../frontend/packages/api-client/src/bindings/")]
pub struct SessionResponse {
    /// Unique identifier for the Checkout Session, prefixed with `cs_`.
    #[schema(example = "cs_abc123def456")]
    pub id: String,
    /// `true` for production (mainnet), `false` for sandbox (testnet).
    #[schema(example = false)]
    pub livemode: bool,
    /// The display name of the merchant who created this session.
    #[schema(example = "Acme Store")]
    pub merchant_name: String,
    /// The blockchain network identifier (e.g. `TRON`, `BSC`).
    #[schema(example = "TRON")]
    pub network: String,
    /// The settlement token code (always a crypto token: `USDT` or `USDC`).
    #[schema(example = "USDT")]
    pub currency: String,
    /// The total amount expected for this session, in standard units (e.g., `"10.5"` = 10.5 USDT).
    #[schema(example = "10.5")]
    pub amount: String,
    /// The total amount received so far, in standard units. Updated as on-chain payments are detected.
    #[schema(example = "10.5")]
    pub amount_received: String,
    /// The payment address generated for this session. The customer sends tokens to this address.
    #[schema(example = "TQFEyGNzHZAJmebJUvsoZvJghHm2yNhXAD")]
    pub pay_address: String,
    /// The current status of the session. See the session lifecycle documentation for details.
    pub status: SessionStatus,
    /// The merchant's custom reference ID, if provided at session creation.
    #[schema(example = "order_20260211_001")]
    pub client_reference_id: Option<String>,
    /// The URL of the hosted checkout page. Redirect the customer to this URL to complete payment.
    #[schema(example = "https://checkout.ironixpay.com/cs_abc123def456")]
    pub url: String,
    /// The URL the customer is redirected to after successful payment.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "https://example.com/success")]
    pub success_url: Option<String>,
    /// The URL the customer is redirected to if payment is cancelled or the session expires.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "https://example.com/cancel")]
    pub cancel_url: Option<String>,
    /// The time at which the session expires and no longer accepts payments, in UTC (ISO 8601).
    #[schema(example = "2026-02-11T04:26:00Z")]
    pub expires_at: String,
    /// The time at which the session was created, in UTC (ISO 8601).
    #[schema(example = "2026-02-11T03:56:00Z")]
    pub created_at: String,
    /// The platform fee deducted, in standard units (e.g., `"1"` = 1 USDT).
    /// Only populated for completed sessions (Paid/Overpaid).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "1")]
    pub fee_amount: Option<String>,
    /// The net amount credited to the merchant after fee deduction, in standard units.
    /// Only populated for completed sessions (Paid/Overpaid).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "9.5")]
    pub net_amount: Option<String>,
    /// USDT/USDC contract address for this network/environment.
    /// Used by frontend for wallet deeplinks and payment detection polling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency_contract: Option<String>,
    /// Public RPC URL for frontend payment detection polling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection_rpc_url: Option<String>,
    /// Chain family for frontend polling dispatch ("tron" or "evm").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_family: Option<String>,

    /// Pricing details. Always present. For crypto-only sessions, echoes the settlement
    /// currency and amount. For fiat sessions, includes exchange rate details.
    pub pricing: PricingInfo,

    /// List of on-chain transactions associated with this session.
    /// Only populated when retrieving a single session by ID.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub transactions: Vec<TransactionResponse>,

    /// Sub-merchant code if this session was created via PSP context switch.
    /// Only present for sub-merchant sessions.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "shop_tokyo")]
    pub sub_merchant_code: Option<String>,

    /// Merchant logo URL for white-label checkout branding.
    /// Only present when the merchant has uploaded a logo.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_logo_url: Option<String>,
}

/// Context needed to build a SessionResponse from a checkout_sessions::Model.
/// Provides the non-model data that varies by endpoint.
pub struct SessionBuildContext {
    pub livemode: bool,
    pub merchant_name: String,
    pub checkout_base_url: String,
    pub currency_contract: Option<String>,
    pub detection_rpc_url: Option<String>,
    pub chain_family: Option<String>,
    pub transactions: Vec<TransactionResponse>,
    pub merchant_logo_url: Option<String>,
}

impl SessionResponse {
    /// Build a SessionResponse from a checkout_sessions::Model + context.
    ///
    /// Centralizes all amount → standard unit conversion and fiat pricing assembly.
    pub fn from_model(
        session: &crate::entity::checkout_sessions::Model,
        ctx: SessionBuildContext,
    ) -> Self {
        let currency = &session.currency;

        // Build PricingInfo — always present, always from non-null DB fields.
        let pricing = PricingInfo {
            currency: session.pricing_currency.clone(),
            amount: session.pricing_amount.normalize().to_string(),
            exchange_rate: session.exchange_rate.to_string(),
        };

        Self {
            id: session.id.clone(),
            livemode: ctx.livemode,
            merchant_name: ctx.merchant_name,
            network: session.network.clone(),
            currency: currency.clone(),
            // Convert all i64 microunits to standard unit strings
            amount: from_micro(session.amount_expected, currency),
            amount_received: from_micro(session.amount_received, currency),
            pay_address: session.pay_address.clone(),
            status: session.status.clone(),
            client_reference_id: session.client_reference_id.clone(),
            url: format!("{}/checkout/{}", ctx.checkout_base_url, session.id),
            success_url: session.success_url.clone(),
            cancel_url: session.cancel_url.clone(),
            expires_at: session.expires_at.to_rfc3339(),
            created_at: session.created_at.to_rfc3339(),
            fee_amount: session.fee_amount.map(|v| from_micro(v, currency)),
            net_amount: session.net_amount.map(|v| from_micro(v, currency)),
            currency_contract: ctx.currency_contract,
            detection_rpc_url: ctx.detection_rpc_url,
            chain_family: ctx.chain_family,
            pricing,
            transactions: ctx.transactions,
            sub_merchant_code: session.sub_merchant_code.clone(),
            merchant_logo_url: ctx.merchant_logo_url,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(
    export,
    export_to = "../frontend/packages/api-client/src/bindings/SessionListResponse.ts"
)]
pub struct SessionListResponse {
    pub sessions: Vec<SessionResponse>,
}
