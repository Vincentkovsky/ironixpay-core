use ironix_pay::entity::webhook_endpoints::EndpointStatus;
use ironix_pay::entity::webhook_events::WebhookEventStatus;
use ironix_pay::entity::{
    addresses, checkout_sessions, merchants, webhook_endpoints, webhook_events, WebhookEvents,
};
use ironix_pay::services::alerting::AlertingService;
use ironix_pay::services::webhook::WebhookService;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use secrecy::Secret;
use std::sync::Arc;
use tokio::time::Duration;
use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};

use crate::common;

#[tokio::test]
async fn test_webhook_retry_logic() -> anyhow::Result<()> {
    std::env::set_var("NO_PROXY", "127.0.0.1,localhost");
    common::init_logger();

    // 1. Setup
    let mock_server = MockServer::start().await;
    let db = common::setup_test_db().await;

    // Config: 3 retries max
    let webhook_service = WebhookService::new_allowing_private_targets_for_tests(
        db.conn.clone(),
        Secret::new(common::TEST_ENCRYPTION_KEY_HEX.to_string()),
        5, // timeout
        3, // max_retries
        Arc::new(AlertingService::new(
            None,
            ironix_pay::entity::Environment::Sandbox,
        )),
    );

    // 1.5 Create Merchant
    let merchant = merchants::ActiveModel {
        id: Set("mer_retry_test".to_string()),
        name: Set("Retry Test Merchant".to_string()),
        status: Set(merchants::MerchantStatus::Active),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    };
    merchant.insert(&db.conn).await?;

    // 1.6 Create Address
    let address = addresses::ActiveModel {
        network: Set("TRON".to_string()),
        address: Set("TFakeAddress123".to_string()),
        merchant_id: Set("mer_retry_test".to_string()),
        path_index: Set(0),
        native_balance: Set(0),
        usdt_balance: Set(0),
        status: Set(ironix_pay::entity::addresses::AddressStatus::Assigned),
        sweep_attempts: Set(0),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    };
    address.insert(&db.conn).await?;

    // 1.7 Create Session
    let session = checkout_sessions::ActiveModel {
        id: Set("sess_fake".to_string()),
        merchant_id: Set("mer_retry_test".to_string()),
        network: Set("TRON".to_string()),
        pay_address: Set("TFakeAddress123".to_string()),
        currency: Set("USDT".to_string()),
        currency_contract: Set("TR7NH...".to_string()),
        amount_expected: Set(100),
        amount_received: Set(0),
        pricing_currency: Set("USDT".to_string()),
        pricing_amount: Set(rust_decimal::Decimal::new(100, 6)),
        exchange_rate: Set(rust_decimal::Decimal::new(1, 0)),
        status: Set(ironix_pay::entity::checkout_sessions::SessionStatus::Pending),
        expires_at: Set((chrono::Utc::now() + chrono::Duration::hours(1)).into()),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    };
    session.insert(&db.conn).await?;

    // 2. Create Endpoint
    let endpoint_id = uuid::Uuid::new_v4().to_string();
    let endpoint = webhook_endpoints::ActiveModel {
        id: Set(endpoint_id.clone()),
        merchant_id: Set("mer_retry_test".to_string()),
        url: Set(mock_server.uri()),
        secret_encrypted: Set(ironix_pay::crypto::encrypt_aes_gcm(
            "secret_123",
            &common::TEST_ENCRYPTION_KEY,
        )
        .unwrap()),
        status: Set(EndpointStatus::Enabled),
        description: Set(Some("Retry Test Endpoint".to_string())),
        environment: Set(ironix_pay::entity::Environment::Production),
        created_at: Set(chrono::Utc::now().into()),
    };
    endpoint.insert(&db.conn).await?;

    // 3. Create Event (Pending)
    let event_id = format!("evt_{}", uuid::Uuid::new_v4().simple());
    let event = webhook_events::ActiveModel {
        id: Set(event_id.clone()),
        endpoint_id: Set(endpoint_id.clone()),
        source_id: Set("sess_fake".to_string()),
        merchant_id: Set("mer_retry_test".to_string()),
        event_type: Set("payment.detected".to_string()),
        payload: Set(serde_json::json!({"foo": "bar"})),
        status: Set(WebhookEventStatus::Pending),
        http_status_code: Set(None),
        created_at: Set(chrono::Utc::now().into()),
        next_retry_at: Set(None), // Ready immediately for first try
        last_attempt_at: Set(None),
        attempt_count: Set(0),
        network: Set("TRON".to_string()),
        target_url: Set(mock_server.uri()),
    };
    event.insert(&db.conn).await?;

    // 4. Mock Failures (500 Internal Server Error)
    // First attempt fails
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)
        .mount(&mock_server)
        .await;

    // Run processing (Attempt 1)
    // We use recover_pending_events which picks up Pending items
    webhook_service.recover_pending_events().await?;

    // Wait slightly for async background task to complete (since recover spawns tasks)
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify: Status should be 'Failed' with retry scheduled
    let stored_event = WebhookEvents::find_by_id(&event_id)
        .one(&db.conn)
        .await?
        .unwrap();
    assert_eq!(stored_event.attempt_count, 1);
    assert_eq!(stored_event.status, WebhookEventStatus::Failed);
    assert!(stored_event.next_retry_at.is_some());

    // Reset Mock for Attempt 2
    mock_server.reset().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)
        .mount(&mock_server)
        .await;

    // Force next_retry_at to be now (skip backoff wait) to allow recovery to pick it up
    let mut active: webhook_events::ActiveModel = stored_event.into();
    active.next_retry_at = Set(Some(chrono::Utc::now().into()));
    active.update(&db.conn).await?;

    // Run processing (Attempt 2)
    webhook_service.recover_pending_events().await?;
    tokio::time::sleep(Duration::from_millis(100)).await;

    let stored_event = WebhookEvents::find_by_id(&event_id)
        .one(&db.conn)
        .await?
        .unwrap();
    assert_eq!(stored_event.attempt_count, 2);
    assert_eq!(stored_event.status, WebhookEventStatus::Failed);

    // Reset Mock for Attempt 3 (Success)
    mock_server.reset().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;

    // Force next_retry_at to be now
    let mut active: webhook_events::ActiveModel = stored_event.into();
    active.next_retry_at = Set(Some(chrono::Utc::now().into()));
    active.update(&db.conn).await?;

    // Run processing (Attempt 3)
    webhook_service.recover_pending_events().await?;
    tokio::time::sleep(Duration::from_millis(100)).await;

    let stored_event = WebhookEvents::find_by_id(&event_id)
        .one(&db.conn)
        .await?
        .unwrap();
    assert_eq!(stored_event.attempt_count, 3);
    assert_eq!(stored_event.status, WebhookEventStatus::Success);

    Ok(())
}

#[tokio::test]
async fn test_webhook_signature() -> anyhow::Result<()> {
    std::env::set_var("NO_PROXY", "127.0.0.1,localhost");
    common::init_logger();

    let mock_server = MockServer::start().await;
    let db = common::setup_test_db().await;
    let webhook_service = WebhookService::new_allowing_private_targets_for_tests(
        db.conn.clone(),
        Secret::new(common::TEST_ENCRYPTION_KEY_HEX.to_string()),
        5,
        3,
        Arc::new(AlertingService::new(
            None,
            ironix_pay::entity::Environment::Sandbox,
        )),
    );

    let secret = "my_super_secret_key";

    // 1.5 Create Merchant
    let merchant = merchants::ActiveModel {
        id: Set("mer_sig_test".to_string()),
        name: Set("Sig Test Merchant".to_string()),
        status: Set(merchants::MerchantStatus::Active),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    };
    merchant.insert(&db.conn).await?;

    // 1.6 Create Address
    let address = addresses::ActiveModel {
        network: Set("TRON".to_string()),
        address: Set("TFakeAddressSig".to_string()),
        merchant_id: Set("mer_sig_test".to_string()),
        path_index: Set(0),
        native_balance: Set(0),
        usdt_balance: Set(0),
        status: Set(ironix_pay::entity::addresses::AddressStatus::Assigned),
        sweep_attempts: Set(0),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    };
    address.insert(&db.conn).await?;

    // 1.7 Create Session
    let session = checkout_sessions::ActiveModel {
        id: Set("sess_sig_test".to_string()),
        merchant_id: Set("mer_sig_test".to_string()),
        network: Set("TRON".to_string()),
        pay_address: Set("TFakeAddressSig".to_string()),
        currency: Set("USDT".to_string()),
        currency_contract: Set("TR7NH...".to_string()),
        amount_expected: Set(100),
        amount_received: Set(0),
        pricing_currency: Set("USDT".to_string()),
        pricing_amount: Set(rust_decimal::Decimal::new(100, 6)),
        exchange_rate: Set(rust_decimal::Decimal::new(1, 0)),
        status: Set(ironix_pay::entity::checkout_sessions::SessionStatus::Pending),
        expires_at: Set((chrono::Utc::now() + chrono::Duration::hours(1)).into()),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    };
    session.insert(&db.conn).await?;

    // 1. Create Endpoint
    let endpoint_id = uuid::Uuid::new_v4().to_string();
    let endpoint = webhook_endpoints::ActiveModel {
        id: Set(endpoint_id.clone()),
        merchant_id: Set("mer_sig_test".to_string()),
        url: Set(mock_server.uri()),
        secret_encrypted: Set(ironix_pay::crypto::encrypt_aes_gcm(
            secret,
            &common::TEST_ENCRYPTION_KEY,
        )
        .unwrap()),
        status: Set(EndpointStatus::Enabled),
        description: Set(Some("Sig Test".to_string())),
        environment: Set(ironix_pay::entity::Environment::Production),
        created_at: Set(chrono::Utc::now().into()),
    };
    endpoint.insert(&db.conn).await?;

    // 2. Mock expectation with signature validation
    Mock::given(method("POST"))
        .and(move |req: &wiremock::Request| {
            // Check for headers
            let headers = &req.headers;
            let sig_header = headers.get("X-Signature");
            let ts_header = headers.get("X-Timestamp");

            if sig_header.is_none() || ts_header.is_none() {
                return false;
            }
            let signature = sig_header.unwrap().to_str().unwrap();
            let timestamp = ts_header.unwrap().to_str().unwrap();

            // Validate signature: HMAC-256(secret, timestamp + "." + payload)

            // Reconstruct payload string from bytes
            let payload_str = std::str::from_utf8(&req.body).unwrap();

            // Re-implement generation logic for verification
            use hmac::{Hmac, Mac};
            use sha2::Sha256;
            type HmacSha256 = Hmac<Sha256>;

            let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
            let message = format!("{}.{}", timestamp, payload_str);
            mac.update(message.as_bytes());
            let expected_sig = hex::encode(mac.finalize().into_bytes());

            signature == expected_sig
        })
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;

    // 3. Create and Process Event
    let event_id = format!("evt_{}", uuid::Uuid::new_v4().simple());
    let event = webhook_events::ActiveModel {
        id: Set(event_id.clone()),
        endpoint_id: Set(endpoint_id),
        source_id: Set("sess_sig_test".to_string()),
        merchant_id: Set("mer_sig_test".to_string()),
        event_type: Set("payment.detected".to_string()),
        payload: Set(serde_json::json!({"amount": 100, "currency": "USDT"})),
        status: Set(WebhookEventStatus::Pending),
        http_status_code: Set(None),
        created_at: Set(chrono::Utc::now().into()),
        next_retry_at: Set(None),
        last_attempt_at: Set(None),
        attempt_count: Set(0),
        network: Set("TRON".to_string()),
        target_url: Set(mock_server.uri()),
    };
    event.insert(&db.conn).await?;

    webhook_service.recover_pending_events().await?;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify it succeeded
    let stored_event = WebhookEvents::find_by_id(&event_id)
        .one(&db.conn)
        .await?
        .unwrap();
    assert_eq!(stored_event.status, WebhookEventStatus::Success);

    Ok(())
}
