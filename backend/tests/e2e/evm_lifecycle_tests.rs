//! EVM Lifecycle E2E Tests
//!
//! BSC version of lifecycle_tests.rs. Tests the full flow:
//! Session(Pending) → Simulated Payment → Session(Paid) → Sweep → Address(Sweeping)
//!
//! Uses EVM-specific data: Network::Bsc, "BSC", 0x addresses.
//! All chain interactions are mocked — no RPC dependency.

use ironix_pay::entity::{
    addresses, checkout_sessions, merchant_chain_accounts, merchants, outbound_transactions,
    webhook_endpoints, webhook_events, Addresses, CheckoutSessions, Environment, Network,
    WebhookEvents,
};
use ironix_pay::services::alerting::AlertingService;
use ironix_pay::services::sweeper::{SweeperConfig, SweeperService};
use ironix_pay::services::webhook::WebhookService;

use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use secrecy::Secret;
use std::sync::Arc;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

use crate::common;

// --- Mock Price Oracle ---
struct TestPriceOracle;
#[async_trait::async_trait]
impl ironix_pay::services::price::PriceOracle for TestPriceOracle {
    async fn get_trx_usdt_price(&self) -> anyhow::Result<rust_decimal::Decimal> {
        Ok(rust_decimal::Decimal::new(10, 2)) // Not used for EVM
    }
    async fn get_native_usdt_price(
        &self,
        _network: ironix_pay::entity::Network,
    ) -> anyhow::Result<rust_decimal::Decimal> {
        Ok(rust_decimal::Decimal::new(600, 0)) // BNB ~600 USDT
    }
}

// ── BSC test constants ──────────────────────────────────────────────────

const BSC_PAY_ADDR: &str = "0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B";
const BSC_COLLECTION_ADDR: &str = "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97";
const BSC_USDT_CONTRACT: &str = "0x55d398326f99059fF775485246999027B3197955";

/// Full lifecycle test for BSC chain:
/// 1. Create merchant + BSC chain account
/// 2. Create address (Assigned) → Simulate payment → (Detected + Paid)
/// 3. Queue webhook → Verify delivery
/// 4. Run sweeper → Verify Address(Sweeping) + sweep_transaction record
#[tokio::test]
async fn test_bsc_lifecycle_happy_path() -> anyhow::Result<()> {
    std::env::set_var("NO_PROXY", "127.0.0.1,localhost");
    common::init_logger();

    // 1. Setup Environment
    let mock_server = MockServer::start().await;
    let webhook_url = mock_server.uri() + "/webhook";

    Mock::given(method("POST"))
        .and(path("/webhook"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let test_db = common::setup_test_db().await;
    let db = &test_db.conn;

    // 2. Setup Services
    let encryption_key = [0u8; 32];
    let hex_key = hex::encode(encryption_key);
    let alerting_service = Arc::new(AlertingService::new(
        None,
        ironix_pay::entity::Environment::Sandbox,
    ));
    let webhook_service = WebhookService::new_allowing_private_targets_for_tests(
        db.clone(),
        Secret::new(hex_key),
        3,
        3,
        alerting_service.clone(),
    );

    // Mock SweepExecutor for EVM (chain-agnostic trait)
    struct BscSweepExecutor;
    #[async_trait::async_trait]
    impl ironix_pay::services::sweeper::executor::SweepExecutor for BscSweepExecutor {
        async fn get_balance(&self, _address: &str, _token_contract: &str) -> anyhow::Result<i64> {
            Ok(50_000_000) // 50 USDT (already in 6-decimal)
        }
        async fn execute_sweep(
            &self,
            _from: &str,
            _to: &str,
            _account_index: i32,
            _path_index: u32,
            _token_contract: &str,
            _outbound_id: &str,
            _outbound_store: &ironix_pay::services::outbound::OutboundTransactionStore,
        ) -> anyhow::Result<ironix_pay::services::sweeper::executor::SweepResult> {
            Ok(ironix_pay::services::sweeper::executor::SweepResult {
                tx_hash: "0xbsc_sweep_tx_hash_123".to_string(),
                funding_tx_hash: Some("0xbsc_fund_tx_hash_456".to_string()),
                amount_swept: 50_000_000,
                gas_cost_native: 50_000_000_000_000u64, // ~0.00005 BNB in wei
                broadcast_disposition:
                    ironix_pay::services::outbound::BroadcastDisposition::Accepted,
            })
        }
        async fn check_tx_status(
            &self,
            _tx_hash: &str,
            _required_confirmations: i32,
        ) -> anyhow::Result<ironix_pay::services::sweeper::executor::SweepTxStatus> {
            Ok(ironix_pay::services::sweeper::executor::SweepTxStatus::Confirmed)
        }
        async fn get_current_block(&self) -> anyhow::Result<i64> {
            Ok(50_000_000) // BSC block height
        }
    }

    // Initialize Sweeper with BSC treasury
    let mut sweeper_config = SweeperConfig::default();
    sweeper_config.platform_treasury_address = Some(BSC_COLLECTION_ADDR.to_string());
    let sweeper_service = SweeperService::new(
        db.clone(),
        Arc::new(BscSweepExecutor),
        Arc::new(TestPriceOracle),
        sweeper_config,
        0.5,
        Environment::Production,
        Network::Bsc,
        alerting_service,
        Arc::new(ironix_pay::services::outbound::OutboundTransactionStore::for_tests(db.clone())),
    );

    // 3. Create Merchant & BSC Chain Account
    let merchant = merchants::ActiveModel {
        id: Set("mer_bsc_life".to_string()),
        name: Set("BSC Lifecycle Merchant".to_string()),
        status: Set(merchants::MerchantStatus::Active),
        account_index: Set(Some(1)),
        ..Default::default()
    };
    merchant.insert(db).await?;

    let chain_account = merchant_chain_accounts::ActiveModel {
        merchant_id: Set("mer_bsc_life".to_string()),
        environment: Set(Environment::Production),
        network: Set(Network::Bsc),
        usdt_balance: Set(100_000_000),
        usdc_balance: Set(0),
        collection_address: Set(Some(BSC_COLLECTION_ADDR.to_string())),
        xpub_encrypted: Set("e_xpub_bsc_mock".to_string()),
        ..Default::default()
    };
    chain_account.insert(db).await?;

    // 4. Create Webhook Endpoint
    let endpoint = webhook_endpoints::ActiveModel {
        id: Set("ep_bsc_life".to_string()),
        merchant_id: Set("mer_bsc_life".to_string()),
        url: Set(webhook_url.clone()),
        status: Set(webhook_endpoints::EndpointStatus::Enabled),
        secret_encrypted: Set(
            ironix_pay::crypto::encrypt_aes_gcm("test_secret", &[0u8; 32]).unwrap(),
        ),
        environment: Set(Environment::Production),
        ..Default::default()
    };
    endpoint.insert(db).await?;

    // 5. Create BSC Address (Assigned)
    let address = addresses::ActiveModel {
        network: Set("BSC".to_string()),
        address: Set(BSC_PAY_ADDR.to_string()),
        merchant_id: Set("mer_bsc_life".to_string()),
        path_index: Set(0),
        status: Set(addresses::AddressStatus::Assigned),
        usdt_balance: Set(0),
        ..Default::default()
    };
    address.insert(db).await?;

    // 6. Create Session (Pending, BSC)
    let session = checkout_sessions::ActiveModel {
        id: Set("cs_bsc_life".to_string()),
        merchant_id: Set("mer_bsc_life".to_string()),
        network: Set("BSC".to_string()),
        pay_address: Set(BSC_PAY_ADDR.to_string()),
        amount_expected: Set(50_000_000),
        amount_received: Set(0),
        pricing_currency: Set("USDT".to_string()),
        pricing_amount: Set(rust_decimal::Decimal::new(50, 0)),
        exchange_rate: Set(rust_decimal::Decimal::new(1, 0)),
        status: Set(checkout_sessions::SessionStatus::Pending),
        currency: Set("USDT".to_string()),
        currency_contract: Set(BSC_USDT_CONTRACT.to_string()),
        expires_at: Set((chrono::Utc::now() + chrono::Duration::hours(1)).into()),
        ..Default::default()
    };
    session.insert(db).await?;

    // 7. Simulate Payment (what indexer would do)
    // A. Update Address Balance + Status → Detected
    let mut addr_model: addresses::ActiveModel =
        Addresses::find_by_id(("BSC".to_string(), BSC_PAY_ADDR.to_string()))
            .one(db)
            .await?
            .unwrap()
            .into();
    addr_model.usdt_balance = Set(50_000_000);
    addr_model.status = Set(addresses::AddressStatus::Detected);
    addr_model.update(db).await?;

    // B. Update Session Status → Paid
    let mut sess_model: checkout_sessions::ActiveModel =
        CheckoutSessions::find_by_id("cs_bsc_life".to_string())
            .one(db)
            .await?
            .unwrap()
            .into();
    sess_model.status = Set(checkout_sessions::SessionStatus::Paid);
    sess_model.amount_received = Set(50_000_000);
    sess_model.update(db).await?;

    // C. Queue Webhook
    let payload = serde_json::json!({"event": "payment.success", "chain": "BSC"});
    let event_ids = webhook_service
        .queue_event(
            "cs_bsc_life",
            "mer_bsc_life",
            Network::Bsc,
            Environment::Production,
            "payment.success",
            &payload,
        )
        .await?;

    // 8. Verify Webhook Delivery
    tokio::time::sleep(tokio::time::Duration::from_millis(3000)).await;

    let event = WebhookEvents::find_by_id(&event_ids[0])
        .one(db)
        .await?
        .unwrap();
    assert_eq!(
        event.status,
        webhook_events::WebhookEventStatus::Success,
        "BSC webhook should be delivered successfully"
    );

    // 9. Trigger Sweeper
    Arc::new(sweeper_service).broadcast_cycle().await?;

    // 10. Verify Sweep Results
    // A. Address → Sweeping
    let addr_final = Addresses::find_by_id(("BSC".to_string(), BSC_PAY_ADDR.to_string()))
        .one(db)
        .await?
        .unwrap();
    assert_eq!(
        addr_final.status,
        addresses::AddressStatus::Sweeping,
        "BSC address should transition to Sweeping"
    );

    // B. Sweep Transaction Record
    let sweep_tx = outbound_transactions::Entity::find()
        .filter(outbound_transactions::Column::FromAddress.eq(BSC_PAY_ADDR))
        .one(db)
        .await?
        .expect("BSC sweep transaction should exist");

    assert_eq!(
        sweep_tx.state,
        ironix_pay::entity::outbound_transactions::OutboundState::Pending
    );
    assert_eq!(
        sweep_tx.amount, 50_000_000,
        "Sweep amount should be 50 USDT"
    );
    assert_eq!(sweep_tx.network, "BSC", "Sweep should be on BSC");

    // C. Verify tx_hash has 0x prefix (EVM format)
    assert!(
        sweep_tx
            .tx_hash
            .as_ref()
            .map_or(false, |h| h.starts_with("0x")),
        "BSC sweep tx_hash should have 0x prefix"
    );

    // D. Verify funding_tx_hash exists (EVM gas funding)
    assert!(
        sweep_tx.funding_tx_hash.is_some(),
        "BSC sweep should have funding_tx_hash (gas funding step)"
    );

    Ok(())
}
