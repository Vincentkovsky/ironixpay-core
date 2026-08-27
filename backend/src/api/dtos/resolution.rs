use crate::api::dtos::checkout::from_micro;
use crate::entity::checkout_sessions;
use crate::entity::payment_exceptions::{ExceptionStatus, ExceptionType, Resolution};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use validator::Validate;

// Constants to avoid magic numbers
pub const USDT_DECIMALS: u32 = 6;
pub const DUST_THRESHOLD: i64 = 100_000; // 0.1 USDT
pub const MIN_TRX_FOR_TRANSFER: u64 = 30_000_000; // 30 TRX

#[derive(Serialize)]
pub struct ResolutionStatsResponse {
    pub unresolved_count: i64,
    pub unresolved_value: String,
    pub dust_count_24h: i64,
}

#[derive(Serialize)]
pub struct ExceptionResponse {
    pub id: String,
    pub exception_type: ExceptionType,
    pub amount: String, // Human readable: "100.50"
    pub currency: String,
    pub network: String,
    pub sender: String,
    pub tx_hash: String,
    pub session_id: Option<String>,
    pub client_ref_id: Option<String>,
    pub status: ExceptionStatus,
    pub resolution: Option<Resolution>,
    pub resolution_ref_id: Option<String>,
    /// On-chain tx hash of the resolution sweep/transfer (if resolved via ManualTransfer or AutoSweep)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution_tx_hash: Option<String>,
    /// Destination address of the resolution sweep/transfer
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution_to_address: Option<String>,
    /// Sub-merchant code (None if belongs to parent merchant directly)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_merchant_code: Option<String>,
    pub created_at: String,
    /// Available actions for this exception based on type and status
    pub available_actions: Vec<String>,
}

/// Available resolution action types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionAction {
    Accept,   // accept_expired_session
    Attach,   // attach_session
    Transfer, // manual_transfer (requires 2FA)
}

impl ResolutionAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResolutionAction::Accept => "accept",
            ResolutionAction::Attach => "attach",
            ResolutionAction::Transfer => "transfer",
        }
    }
}

/// Determine available actions based on exception type and status
pub fn get_available_actions(
    exception_type: &ExceptionType,
    status: &ExceptionStatus,
) -> Vec<String> {
    // Only Pending exceptions can have actions
    if *status != ExceptionStatus::Pending {
        return vec![];
    }

    match exception_type {
        ExceptionType::SessionExpired => {
            // Accept: credit to original session + auto-sweep to Treasury
            vec![
                ResolutionAction::Accept.as_str().to_string(),
                ResolutionAction::Transfer.as_str().to_string(),
            ]
        }
        ExceptionType::NoActiveSession | ExceptionType::SessionAlreadyCompleted => {
            // Attach: bind to another session + credit + auto-sweep
            vec![
                ResolutionAction::Attach.as_str().to_string(),
                ResolutionAction::Transfer.as_str().to_string(),
            ]
        }
        ExceptionType::UnderpaidExpired => {
            // Accept: credit expired partial payment, or Transfer: refund
            vec![
                ResolutionAction::Accept.as_str().to_string(),
                ResolutionAction::Transfer.as_str().to_string(),
            ]
        }
        ExceptionType::DustPayment => {
            // Dust is too small to do anything meaningful, auto-ignored
            vec![]
        }
        ExceptionType::RiskBlocked => {
            // AML risk detected - ONLY manual_transfer allowed (refund to user)
            // NO accept/attach - cannot credit risky funds to any session
            vec![ResolutionAction::Transfer.as_str().to_string()]
        }
        ExceptionType::WrongToken => {
            // Wrong token: payment currency doesn't match session
            // ONLY manual_transfer allowed — refund the wrong token back to sender
            vec![ResolutionAction::Transfer.as_str().to_string()]
        }
        ExceptionType::Unknown => {
            // Unknown type - transfer (refund) only
            vec![ResolutionAction::Transfer.as_str().to_string()]
        }
    }
}

#[derive(Deserialize, Validate)]
pub struct TransferRequest {
    /// Destination address (TRON Base58 or EVM 0x format).
    /// Full format validation is performed in the service layer based on the exception's network.
    #[validate(length(min = 26, max = 42, message = "Address must be 26-42 characters"))]
    pub to_address: String,

    /// Optional specific amount in human-readable decimal (e.g., "50.5").
    /// If None, the full amount of the exception will be transferred.
    #[validate(custom(function = "validate_decimal_amount"))]
    pub amount: Option<String>,

    pub notes: Option<String>,

    #[validate(length(min = 6, max = 6))]
    pub code: String, // 2FA Code
}

/// Custom validator to ensure the amount is a positive decimal string
fn validate_decimal_amount(amount: &str) -> Result<(), validator::ValidationError> {
    match amount.parse::<Decimal>() {
        Ok(val) => {
            if val <= Decimal::ZERO {
                return Err(validator::ValidationError::new("Amount must be positive"));
            }
            Ok(())
        }
        Err(_) => Err(validator::ValidationError::new("Invalid number format")),
    }
}

#[derive(Deserialize, Validate)]
pub struct AttachRequest {
    pub session_id: String,
}

impl ExceptionResponse {
    pub fn from_model(
        model: crate::entity::payment_exceptions::Model,
        session: Option<checkout_sessions::Model>,
        sweep_info: Option<(Option<String>, String)>, // (tx_hash, to_address)
        sub_merchant_code: Option<String>,
    ) -> Self {
        let available_actions = get_available_actions(&model.exception_type, &model.status);

        Self {
            id: model.id,
            exception_type: model.exception_type,
            // Convert microunits to human-readable string
            amount: from_micro(model.amount, &model.currency_symbol),
            currency: model.currency_symbol,
            network: model.network,
            sender: model.from_address,
            tx_hash: model.tx_hash,
            session_id: model.session_id,
            client_ref_id: session.and_then(|s| s.client_reference_id),
            status: model.status,
            resolution: model.resolution,
            resolution_ref_id: model.resolution_ref_id,
            resolution_tx_hash: sweep_info.as_ref().and_then(|(h, _)| h.clone()),
            resolution_to_address: sweep_info.map(|(_, a)| a),
            sub_merchant_code,
            created_at: model.created_at.to_rfc3339(),
            available_actions,
        }
    }
}
