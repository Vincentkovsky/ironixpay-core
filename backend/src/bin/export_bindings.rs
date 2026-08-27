use ironix_pay::api::dtos::auth::*;
use ironix_pay::api::dtos::checkout::*;
use ts_rs::TS;

fn main() {
    println!("Exporting TypeScript bindings... (v2)");

    // Create directory first
    // Note: ts-rs might create it, but good to be safe.
    let _ = std::fs::create_dir_all("../frontend/packages/api-client/src/bindings");

    // Auth & Merchant
    RegisterRequest::export().expect("Failed to export RegisterRequest");
    LoginRequest::export().expect("Failed to export LoginRequest");
    LoginResponse::export().expect("Failed to export LoginResponse");
    MerchantResponse::export().expect("Failed to export MerchantResponse");
    ApiKeyResponse::export().expect("Failed to export ApiKeyResponse");
    ApiKeyListResponse::export().expect("Failed to export ApiKeyListResponse");
    ApiError::export().expect("Failed to export ApiError");
    CreateApiKeyRequest::export().expect("Failed to export CreateApiKeyRequest");
    MerchantBalanceResponse::export().expect("Failed to export MerchantBalanceResponse");
    VerifyEmailRequest::export().expect("Failed to export VerifyEmailRequest");
    ResendVerificationRequest::export().expect("Failed to export ResendVerificationRequest");
    Verify2FARequest::export().expect("Failed to export Verify2FARequest");
    SuccessResponse::export().expect("Failed to export SuccessResponse");
    TotpSetupResponse::export().expect("Failed to export TotpSetupResponse");
    Enable2FARequest::export().expect("Failed to export Enable2FARequest");
    UpdateProfileRequest::export().expect("Failed to export UpdateProfileRequest");
    ChangePasswordRequest::export().expect("Failed to export ChangePasswordRequest");

    // Checkout
    CreateSessionBody::export().expect("Failed to export CreateSessionBody");
    SessionResponse::export().expect("Failed to export SessionResponse");
    SessionListResponse::export().expect("Failed to export SessionListResponse");
    // ApiError is shared, already exported above.

    println!("Done!");
}
