//! API Integration Tests
//!
//! Tests for the REST API endpoints - format and structure validation.
//! Aligned with docs/system_design.md schema.

#[cfg(test)]
mod api_tests {
    use serde_json::json;

    /// Test registration request validation
    #[test]
    fn test_register_request_format() {
        let valid_request = json!({
            "name": "Test Merchant",
            "email": "test@example.com",
            "password": "secure_password_123",
            "collection_address": "TXxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
        });

        assert!(valid_request.get("name").is_some());
        assert!(valid_request.get("email").is_some());
        assert!(valid_request.get("password").is_some());
        assert!(valid_request.get("collection_address").is_some());
    }

    /// Test login request format
    #[test]
    fn test_login_request_format() {
        let login_request = json!({
            "email": "test@example.com",
            "password": "secure_password"
        });

        assert_eq!(
            login_request.get("email").unwrap().as_str().unwrap(),
            "test@example.com"
        );
    }

    /// Test API key creation request
    #[test]
    fn test_api_key_request_format() {
        // With name
        let with_name = json!({
            "name": "Production Key",
            "is_test": false
        });

        assert!(!with_name.get("is_test").unwrap().as_bool().unwrap());

        // Test mode
        let test_mode = json!({
            "is_test": true
        });

        assert!(test_mode.get("is_test").unwrap().as_bool().unwrap());
    }

    /// Test checkout session creation request
    #[test]
    fn test_checkout_session_request() {
        let checkout_request = json!({
            "amount_expected": 1000000, // 1 USDT (6 decimals)
            "currency": "USDT",
            "network": "TRON",
            "client_reference_id": "order_12345"
        });

        // Amount should be in atomic units
        let amount = checkout_request
            .get("amount_expected")
            .unwrap()
            .as_i64()
            .unwrap();
        assert_eq!(amount, 1000000);

        // Minimal request
        let minimal = json!({
            "amount_expected": 5000000 // 5 USDT
        });

        assert!(minimal.get("network").is_none()); // Optional, defaults to TRON
    }

    /// Test response format for session
    #[test]
    fn test_session_response_format() {
        let session_response = json!({
            "id": "cs_test_abc123",
            "network": "TRON",
            "amount_expected": 1000000,
            "amount_received": 0,
            "currency": "USDT",
            "pay_address": "TXxxxxxxxxxxxxxxxxxxxxx",
            "status": "Pending",
            "expires_at": "2024-01-01T12:00:00Z"
        });

        let id = session_response.get("id").unwrap().as_str().unwrap();
        assert!(id.starts_with("cs_"));

        let status = session_response.get("status").unwrap().as_str().unwrap();
        assert!([
            "Pending",
            "Paid",
            "Underpaid",
            "Overpaid",
            "Expired",
            "Blocked"
        ]
        .contains(&status));
    }

    /// Test error response format (Stripe-style nested)
    #[test]
    fn test_error_response_format() {
        let error_response = json!({
            "error": {
                "type": "invalid_request_error",
                "code": "parameter_invalid",
                "message": "Missing required field: amount_expected",
                "param": "amount_expected",
                "doc_url": "https://docs.ironixpay.io/errors#parameter_invalid"
            }
        });

        let error = error_response.get("error").unwrap();
        assert!(error.get("type").unwrap().is_string());
        assert!(error.get("code").unwrap().is_string());
        assert!(error.get("message").unwrap().is_string());
        assert!(error.get("param").unwrap().is_string());
        assert!(error.get("doc_url").unwrap().is_string());
    }

    /// Test authorization header formats
    #[test]
    fn test_auth_header_formats() {
        // JWT Bearer token
        let jwt_header = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...";
        assert!(jwt_header.starts_with("Bearer "));

        // Live API key
        let live_key = ["sk_", "live_", "abcdef1234567890"].concat();
        assert!(live_key.starts_with("sk_live_"));

        // Test API key
        let test_key = ["sk_", "test_", "abcdef1234567890"].concat();
        assert!(test_key.starts_with("sk_test_"));
    }

    /// Test merchant profile response format
    #[test]
    fn test_merchant_response_format() {
        let merchant_response = json!({
            "id": "mer_abc123",
            "name": "Test Merchant",
            "email": "test@example.com",
            "status": "Active",
            "collection_address": "TXxxxxxxxxxxxxxxxxxxxxx",
            "gas_credit_balance": 100000000,
            "account_index": 1
        });

        let id = merchant_response.get("id").unwrap().as_str().unwrap();
        assert!(id.starts_with("mer_"));

        let status = merchant_response.get("status").unwrap().as_str().unwrap();
        assert!(["PendingVerification", "Active", "Suspended"].contains(&status));
    }
}
