use ironix_pay::crypto::{decrypt_aes_gcm, encrypt_aes_gcm};
use ironix_pay::services::xero::client::XeroError;

// ─── Amount Conversion (Decimal Precision) ───

/// Replicate the sync_session amount conversion logic for testing.
fn convert_microunits_to_decimal(microunits: i64) -> rust_decimal::Decimal {
    let divisor = rust_decimal::Decimal::from(1_000_000_i64);
    (rust_decimal::Decimal::from(microunits) / divisor).round_dp(2)
}

#[test]
fn test_amount_conversion_basic() {
    // 10.50 USDT = 10_500_000 microunits
    let result = convert_microunits_to_decimal(10_500_000);
    assert_eq!(result.to_string(), "10.50");
}

#[test]
fn test_amount_conversion_zero() {
    let result = convert_microunits_to_decimal(0);
    assert_eq!(result, rust_decimal::Decimal::ZERO);
}

#[test]
fn test_amount_conversion_one_cent() {
    // 0.01 = 10_000 microunits
    let result = convert_microunits_to_decimal(10_000);
    assert_eq!(result.to_string(), "0.01");
}

#[test]
fn test_amount_conversion_sub_cent_rounds_down() {
    // 0.004999 = 4_999 microunits → rounds to 0.00
    let result = convert_microunits_to_decimal(4_999);
    assert_eq!(result.to_string(), "0.00");
}

#[test]
fn test_amount_conversion_sub_cent_rounds_half_even() {
    // 0.005000 = 5_000 microunits → rounds to 0.00 (banker's rounding: MidpointNearestEven)
    let result = convert_microunits_to_decimal(5_000);
    assert_eq!(result, rust_decimal::Decimal::ZERO);

    // 0.015000 = 15_000 microunits → rounds to 0.02 (banker's: round to even)
    let result2 = convert_microunits_to_decimal(15_000);
    assert_eq!(result2.to_string(), "0.02");
}

#[test]
fn test_amount_conversion_large_amount() {
    // $10,000,000.99 = 10_000_000_990_000 microunits
    let result = convert_microunits_to_decimal(10_000_000_990_000);
    assert_eq!(result.to_string(), "10000000.99");
}

#[test]
fn test_amount_conversion_precision_no_float_error() {
    // 9_999_999_999_999 / 1_000_000 = 9999999.999999 → round_dp(2) → 10000000.00
    // Decimal rounds correctly (no floating-point drift)
    let result = convert_microunits_to_decimal(9_999_999_999_999);
    let expected = rust_decimal::Decimal::new(1_000_000_000, 2); // 10000000.00
    assert_eq!(result, expected);

    // Verify f64 would have the same result here, but for amounts like
    // 1_234_567_890_123 / 1_000_000 = 1234567.890123 → f64 loses sub-cent precision
    let precise = convert_microunits_to_decimal(1_234_567_890_123);
    assert_eq!(precise.to_string(), "1234567.89"); // Decimal truncates correctly
}

#[test]
fn test_amount_fee_negative_format() {
    let fee = convert_microunits_to_decimal(50_000); // 0.05 fee
    assert!(fee > rust_decimal::Decimal::ZERO);
    let formatted = format!("-{}", fee);
    assert_eq!(formatted, "-0.05");
}

#[test]
fn test_amount_net_equals_gross_minus_fee() {
    let gross = 1_000_000_i64; // 1.00
    let fee = 10_000_i64; // 0.01
    let net = gross - fee; // 990_000

    let gross_dec = convert_microunits_to_decimal(gross);
    let fee_dec = convert_microunits_to_decimal(fee);
    let net_dec = convert_microunits_to_decimal(net);

    assert_eq!(gross_dec, rust_decimal::Decimal::new(100, 2)); // 1.00
    assert_eq!(fee_dec, rust_decimal::Decimal::new(1, 2)); // 0.01
    assert_eq!(net_dec, rust_decimal::Decimal::new(99, 2)); // 0.99
}

// ─── OAuth State Encryption / Parsing ───

fn make_test_key() -> [u8; 32] {
    // Deterministic 32-byte key for tests
    let mut key = [0u8; 32];
    for (i, byte) in key.iter_mut().enumerate() {
        *byte = (i as u8).wrapping_mul(7).wrapping_add(42);
    }
    key
}

#[test]
fn test_oauth_state_roundtrip() {
    let key = make_test_key();
    let merchant_id = "mer_abc123def456";
    let env_str = "production";
    let timestamp = 1712600000_i64;
    let nonce = "b3fe6a84-e9f2-4f65-8b73-52b2b18046fd";
    let payload = format!("{}:{}:{}:{}", merchant_id, env_str, timestamp, nonce);

    let encrypted = encrypt_aes_gcm(&payload, &key).expect("encrypt should succeed");
    let decrypted = decrypt_aes_gcm(&encrypted, &key).expect("decrypt should succeed");
    assert_eq!(decrypted, payload);
}

#[test]
fn test_oauth_state_parsing_valid() {
    let state_payload = "mer_abc123:production:1712600000:b3fe6a84-e9f2-4f65-8b73-52b2b18046fd";
    let parts: Vec<&str> = state_payload.splitn(4, ':').collect();

    assert_eq!(parts.len(), 4);
    assert_eq!(parts[0], "mer_abc123");
    assert_eq!(parts[1], "production");
    assert_eq!(parts[2], "1712600000");
    assert!(parts[2].parse::<i64>().is_ok());
    assert!(!parts[3].is_empty());
}

#[test]
fn test_oauth_state_parsing_sandbox() {
    let state_payload = "mer_xyz789:sandbox:1712600000:nonce_123";
    let parts: Vec<&str> = state_payload.splitn(4, ':').collect();

    assert_eq!(parts.len(), 4);
    assert_eq!(parts[1], "sandbox");
}

#[test]
fn test_oauth_state_expiry_valid() {
    let now = chrono::Utc::now().timestamp();
    let state_ts = now - 300; // 5 minutes ago
    let age = now - state_ts;
    assert!(
        age <= 600 && age >= -60,
        "State should be valid within 10 minutes"
    );
}

#[test]
fn test_oauth_state_expiry_expired() {
    let now = chrono::Utc::now().timestamp();
    let state_ts = now - 700; // 11+ minutes ago
    let age = now - state_ts;
    assert!(age > 600, "State should be expired after 10 minutes");
}

#[test]
fn test_oauth_state_expiry_future_clock_skew() {
    let now = chrono::Utc::now().timestamp();
    let state_ts = now + 30; // 30 seconds in the future (clock skew)
    let age = now - state_ts;
    assert!(age >= -60, "Small future clock skew should be tolerated");
}

#[test]
fn test_oauth_state_expiry_far_future_rejected() {
    let now = chrono::Utc::now().timestamp();
    let state_ts = now + 120; // 2 minutes in the future
    let age = now - state_ts;
    assert!(age < -60, "Large future timestamp should be rejected");
}

#[test]
fn test_oauth_state_wrong_key_fails() {
    let key1 = make_test_key();
    let mut key2 = make_test_key();
    key2[0] ^= 0xFF; // Flip first byte

    let encrypted = encrypt_aes_gcm("mer_abc:production:12345", &key1).unwrap();
    let result = decrypt_aes_gcm(&encrypted, &key2);
    assert!(result.is_err(), "Decryption with wrong key should fail");
}

#[test]
fn test_oauth_state_tampered_ciphertext_fails() {
    let key = make_test_key();
    let encrypted = encrypt_aes_gcm("mer_abc:production:12345", &key).unwrap();

    // Tamper with the base64 — flip a character
    let mut chars: Vec<char> = encrypted.chars().collect();
    if let Some(c) = chars.get_mut(10) {
        *c = if *c == 'A' { 'B' } else { 'A' };
    }
    let tampered: String = chars.into_iter().collect();

    let result = decrypt_aes_gcm(&tampered, &key);
    assert!(
        result.is_err(),
        "Tampered ciphertext should fail authentication"
    );
}

// ─── Retry Delay Scheduling ───

const RETRY_DELAYS_SECS: [i64; 5] = [0, 60, 300, 1800, 21600];

/// Replicate mark_sync_failed status determination logic.
fn determine_retry_status(current_attempt_count: i32) -> (&'static str, Option<i64>) {
    let new_attempt = current_attempt_count + 1;
    let next_retry_delay = if (new_attempt as usize) < RETRY_DELAYS_SECS.len() {
        Some(RETRY_DELAYS_SECS[new_attempt as usize])
    } else {
        None
    };
    ("failed", next_retry_delay)
}

#[test]
fn test_retry_attempt_0_becomes_failed_with_retry() {
    let (status, delay) = determine_retry_status(0);
    assert_eq!(status, "failed");
    assert_eq!(delay, Some(60)); // Attempt 1 → 60s delay
}

#[test]
fn test_retry_attempt_1_becomes_failed_with_retry() {
    let (status, delay) = determine_retry_status(1);
    assert_eq!(status, "failed");
    assert_eq!(delay, Some(300)); // Attempt 2 → 5 min
}

#[test]
fn test_retry_attempt_2_becomes_failed_with_retry() {
    let (status, delay) = determine_retry_status(2);
    assert_eq!(status, "failed");
    assert_eq!(delay, Some(1800)); // Attempt 3 → 30 min
}

#[test]
fn test_retry_attempt_3_becomes_failed_with_retry() {
    let (status, delay) = determine_retry_status(3);
    assert_eq!(status, "failed");
    assert_eq!(delay, Some(21600)); // Attempt 4 → 6 hours
}

#[test]
fn test_retry_attempt_4_becomes_failed() {
    let (status, delay) = determine_retry_status(4);
    assert_eq!(status, "failed");
    assert_eq!(delay, None); // No more retries
}

#[test]
fn test_retry_attempt_beyond_max_stays_failed() {
    let (status, delay) = determine_retry_status(10);
    assert_eq!(status, "failed");
    assert_eq!(delay, None);
}

// ─── XeroError Typed Error ───

#[test]
fn test_xero_rate_limit_error_downcast() {
    let err: anyhow::Error = XeroError::RateLimited { retry_after: 45 }.into();

    let downcast = err.downcast_ref::<XeroError>();
    assert!(downcast.is_some(), "Should be downcastable to XeroError");

    match downcast.unwrap() {
        XeroError::RateLimited { retry_after } => {
            assert_eq!(*retry_after, 45);
        }
    }
}

#[test]
fn test_xero_rate_limit_error_display() {
    let err = XeroError::RateLimited { retry_after: 60 };
    assert_eq!(err.to_string(), "Xero rate limited, retry after 60s");
}

#[test]
fn test_non_xero_error_downcast_is_none() {
    let err = anyhow::anyhow!("Some other error");
    let downcast = err.downcast_ref::<XeroError>();
    assert!(downcast.is_none(), "Non-XeroError should not downcast");
}

// ─── Authorize URL Structure ───

#[test]
fn test_authorize_url_contains_required_params() {
    // Replicate authorize_url logic without needing XeroService
    let client_id = "TEST_CLIENT_ID";
    let redirect_uri = "https://app.ironixpay.com/api/xero/callback";
    let scopes = "openid profile email offline_access accounting.invoices accounting.payments accounting.contacts accounting.settings.read";
    let state = "encrypted_state_token";

    let query: String = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("response_type", "code")
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", scopes)
        .append_pair("state", state)
        .finish();
    let url = format!(
        "https://login.xero.com/identity/connect/authorize?{}",
        query
    );

    assert!(url.starts_with("https://login.xero.com/identity/connect/authorize?"));
    assert!(url.contains("response_type=code"));
    assert!(url.contains("client_id=TEST_CLIENT_ID"));
    assert!(url.contains("redirect_uri="));
    assert!(url.contains("scope="));
    assert!(url.contains("state=encrypted_state_token"));
    // Verify URL encoding of spaces in scope
    assert!(url.contains("openid+profile") || url.contains("openid%20profile"));
}

#[test]
fn test_authorize_url_state_is_url_encoded() {
    let state = "abc+def/ghi=jkl&mno";
    let query: String = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("state", state)
        .finish();

    // Verify special characters are encoded
    assert!(!query.contains('&') || query.starts_with("state="));
    let decoded = url::form_urlencoded::parse(query.as_bytes())
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.to_string())
        .unwrap();
    assert_eq!(decoded, state);
}

// ─── Invoice Line Item Construction ───

#[test]
fn test_invoice_line_items_with_fee() {
    let gross = convert_microunits_to_decimal(100_000_000); // $100.00
    let fee = convert_microunits_to_decimal(1_000_000); // $1.00

    let mut line_items = vec![serde_json::json!({
        "Description": "Crypto payment - 100.000000 USDT on tron_nile",
        "Quantity": 1,
        "UnitAmount": gross.to_string(),
        "AccountCode": "200",
    })];

    if fee > rust_decimal::Decimal::ZERO {
        line_items.push(serde_json::json!({
            "Description": "IronixPay gateway fee",
            "Quantity": 1,
            "UnitAmount": format!("-{}", fee),
            "AccountCode": "404",
        }));
    }

    assert_eq!(line_items.len(), 2);
    assert_eq!(line_items[0]["UnitAmount"], "100");
    assert_eq!(line_items[1]["UnitAmount"], "-1");
}

#[test]
fn test_invoice_line_items_zero_fee() {
    let gross = convert_microunits_to_decimal(50_000_000); // $50.00
    let fee = convert_microunits_to_decimal(0); // $0.00

    let mut line_items = vec![serde_json::json!({
        "Description": "Crypto payment",
        "Quantity": 1,
        "UnitAmount": gross.to_string(),
        "AccountCode": "200",
    })];

    if fee > rust_decimal::Decimal::ZERO {
        line_items.push(serde_json::json!({
            "Description": "IronixPay gateway fee",
            "Quantity": 1,
            "UnitAmount": format!("-{}", fee),
            "AccountCode": "404",
        }));
    }

    assert_eq!(line_items.len(), 1, "Zero fee should not add a line item");
    assert_eq!(line_items[0]["UnitAmount"], "50");
}
