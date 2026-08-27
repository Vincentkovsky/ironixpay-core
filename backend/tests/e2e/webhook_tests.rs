//! Webhook Service Tests
//!
//! Tests for webhook event queueing and delivery.
//! Note: This test requires the new webhook_endpoints table to be set up.
//! Aligned with docs/system_design.md schema.

use ironix_pay::{
    crypto::encrypt_aes_gcm,
    entity::{
        addresses, checkout_sessions, merchant_chain_accounts, merchants, webhook_endpoints,
        webhook_events, Network, WebhookEvents,
    },
    services::alerting::AlertingService,
    services::webhook::WebhookService,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use secrecy::Secret;
use std::sync::Arc;
use wiremock::{
    matchers::{header, method, path},
    Mock, MockServer, ResponseTemplate,
};

use crate::common;

const TRON_NETWORK: &str = "TRON";

#[tokio::test]
async fn test_webhook_queue_and_delivery() -> anyhow::Result<()> {
    for key in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ] {
        std::env::remove_var(key);
    }
    std::env::set_var("NO_PROXY", "127.0.0.1,localhost");
    std::env::set_var("no_proxy", "127.0.0.1,localhost");
    common::init_logger();

    // 1. Setup Mock Server
    let mock_server = MockServer::start().await;
    let webhook_url = mock_server.uri() + "/webhook";

    Mock::given(method("POST"))
        .and(path("/webhook"))
        .and(header("Content-Type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_string("must not be stored"))
        .mount(&mock_server)
        .await;

    // 2. Setup Database & Service
    // 2. Setup Database & Service
    let test_db = common::setup_test_db().await;
    let db = &test_db.conn;
    // Test encryption key (32 bytes for AES-256)
    // Test encryption key (32 bytes for AES-256)
    let encryption_key_hex = hex::encode([1u8; 32]);
    let encryption_key = [1u8; 32]; // For encrypt_aes_gcm manually if needed
    let webhook_service = WebhookService::new_allowing_private_targets_for_tests(
        db.clone(),
        Secret::new(encryption_key_hex),
        5,
        3,
        Arc::new(AlertingService::new(
            None,
            ironix_pay::entity::Environment::Sandbox,
        )),
    );

    // 3. Create Merchant
    let merchant = merchants::ActiveModel {
        id: Set("mer_webhook_test".to_string()),
        name: Set("Test Merchant".to_string()),
        status: Set(merchants::MerchantStatus::Active),
        account_index: Set(Some(888)),
        ..Default::default()
    };
    merchant.insert(db).await?;

    // Create Chain Account (TronMainnet) with balance
    let chain_account = merchant_chain_accounts::ActiveModel {
        merchant_id: Set("mer_webhook_test".to_string()),
        environment: Set(ironix_pay::entity::Environment::Production),
        network: Set(Network::Tron),
        usdt_balance: Set(100_000_000),
        usdc_balance: Set(0),
        collection_address: Set(Some("TWebhookTestCollection".to_string())),
        xpub_encrypted: Set("e_xpub_mock".to_string()),
        ..Default::default()
    };
    chain_account.insert(db).await?;

    // 4. Create Webhook Endpoint (secret must be encrypted with same key)
    let plaintext_secret = "test_secret_123";
    let encrypted_secret =
        encrypt_aes_gcm(plaintext_secret, &encryption_key).expect("Failed to encrypt test secret");
    let endpoint = webhook_endpoints::ActiveModel {
        id: Set("ep_test_123".to_string()),
        merchant_id: Set("mer_webhook_test".to_string()),
        url: Set(webhook_url.clone()),
        secret_encrypted: Set(encrypted_secret),
        status: Set(webhook_endpoints::EndpointStatus::Enabled),
        environment: Set(ironix_pay::entity::Environment::Production),
        ..Default::default()
    };
    endpoint.insert(db).await?;

    // 5. Create Address (Required for FK)
    let address = addresses::ActiveModel {
        network: Set(TRON_NETWORK.to_string()),
        address: Set("TWebhookTestAddress".to_string()),
        merchant_id: Set("mer_webhook_test".to_string()),
        path_index: Set(0),
        status: Set(addresses::AddressStatus::Assigned),
        ..Default::default()
    };
    address.insert(db).await?;

    // 6. Create Session (Required for FK)
    let session = checkout_sessions::ActiveModel {
        id: Set("cs_webhook_test".to_string()),
        merchant_id: Set("mer_webhook_test".to_string()),
        network: Set(TRON_NETWORK.to_string()),
        pay_address: Set("TWebhookTestAddress".to_string()),
        currency: Set("USDT".to_string()),
        currency_contract: Set("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".to_string()),
        amount_expected: Set(100),
        amount_received: Set(0),
        pricing_currency: Set("USDT".to_string()),
        pricing_amount: Set(rust_decimal::Decimal::new(100, 6)),
        exchange_rate: Set(rust_decimal::Decimal::new(1, 0)),
        status: Set(checkout_sessions::SessionStatus::Pending),
        expires_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    };
    session.insert(db).await?;

    // 6. Queue Event (no webhook_url parameter - uses endpoint table)
    let payload = serde_json::json!({
        "event": "test_event",
        "data": "hello"
    });

    let event_ids = webhook_service
        .queue_event(
            "cs_webhook_test",
            "mer_webhook_test",
            Network::Tron,
            ironix_pay::entity::Environment::Production,
            "test.event",
            &payload,
        )
        .await?;

    // Trigger delivery for the queued events
    webhook_service.trigger_delivery(&event_ids).await;

    // 7. Wait for Background Delivery
    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

    // 8. Verify Event is Delivered
    if let Some(event_id) = event_ids.first() {
        let event = WebhookEvents::find_by_id(event_id).one(db).await?.unwrap();

        assert_eq!(
            event.status,
            webhook_events::WebhookEventStatus::Success,
            "Event should be marked as success"
        );
        assert!(event.attempt_count >= 1);
    }

    // Cleanup
    // Deleting events (loop delete to avoid iterator trait issues)
    let events = webhook_events::Entity::find()
        .filter(webhook_events::Column::SourceId.eq("cs_webhook_test"))
        .all(db)
        .await?;
    for event in events {
        webhook_events::Entity::delete_by_id(event.id)
            .exec(db)
            .await?;
    }

    // Deleting session
    checkout_sessions::Entity::delete_by_id("cs_webhook_test")
        .exec(db)
        .await?;

    // Deleting endpoint
    webhook_endpoints::Entity::delete_by_id("ep_test_123")
        .exec(db)
        .await?;

    // Deleting merchant
    merchants::Entity::delete_by_id("mer_webhook_test")
        .exec(db)
        .await?;

    Ok(())
}
