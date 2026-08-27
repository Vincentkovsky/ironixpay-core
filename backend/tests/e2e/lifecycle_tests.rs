use ironix_pay::entity::{
    addresses, checkout_sessions, merchant_chain_accounts, merchants, outbound_transactions,
    webhook_endpoints, webhook_events, Addresses, CheckoutSessions, Environment, Network,
    WebhookEvents,
};
use ironix_pay::services::alerting::AlertingService;
use ironix_pay::services::sweeper::{SweeperConfig, SweeperService};

use ironix_pay::services::tron::interface::{
    AccountResource, BlockInfo, BroadcastResult, SignedTransaction, TransactionInfo,
    TronBroadcaster, UnsignedTransaction,
};
use ironix_pay::services::webhook::WebhookService;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use secrecy::Secret;
use std::sync::Arc;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

use crate::common;

// --- Mock Tron Broadcaster ---
struct TestTronClient;

#[async_trait::async_trait]
impl TronBroadcaster for TestTronClient {
    async fn get_usdt_balance(&self, _addr: &str) -> anyhow::Result<u64> {
        // Return sufficient balance for sweep (50 USDT)
        Ok(50_000_000)
    }

    async fn get_trc20_balance(&self, _addr: &str, _token_contract: &str) -> anyhow::Result<i64> {
        Ok(50_000_000)
    }

    async fn get_trx_balance(&self, _addr: &str) -> anyhow::Result<u64> {
        Ok(1_000_000_000) // 1000 TRX
    }

    async fn build_trc20_transfer(
        &self,
        from: &str,
        to: &str,
        amount: u64,
        _contract: &str,
    ) -> anyhow::Result<UnsignedTransaction> {
        Ok(UnsignedTransaction {
            raw_data: vec![],
            raw_data_hex: format!("tx_{}_{}_{}", from, to, amount),
            raw_data_json: None,
            expiration: None,
        })
    }

    async fn build_trx_transfer(
        &self,
        _from: &str,
        _to: &str,
        _amount: u64,
    ) -> anyhow::Result<UnsignedTransaction> {
        Ok(UnsignedTransaction {
            raw_data: vec![],
            raw_data_hex: "trx_transfer".to_string(),
            raw_data_json: None,
            expiration: None,
        })
    }

    fn sign_transaction(
        &self,
        unsigned_tx: &UnsignedTransaction,
        _private_key: &[u8],
    ) -> anyhow::Result<SignedTransaction> {
        Ok(SignedTransaction {
            tx_id: "tx_hash_123".to_string(),
            raw_data: unsigned_tx.raw_data.clone(),
            signature: vec![1, 2, 3],
            raw_data_json: None,
            expiration: unsigned_tx.expiration,
        })
    }

    async fn broadcast(&self, signed_tx: &SignedTransaction) -> anyhow::Result<BroadcastResult> {
        Ok(BroadcastResult {
            success: true,
            tx_hash: signed_tx.tx_id.clone(),
            message: Some("SUCCESS".to_string()),
        })
    }

    async fn get_current_block(&self) -> anyhow::Result<BlockInfo> {
        Ok(BlockInfo {
            number: 1000,
            timestamp: 1234567890,
        })
    }

    async fn get_transaction_info(&self, _id: &str) -> anyhow::Result<Option<TransactionInfo>> {
        Ok(None)
    }

    async fn estimate_energy(
        &self,
        _from: &str,
        _contract: &str,
        _func: &str,
        _params: &str,
    ) -> anyhow::Result<i64> {
        Ok(14000)
    }

    async fn get_account_resources(&self, _addr: &str) -> anyhow::Result<AccountResource> {
        Ok(AccountResource::default())
    }

    async fn get_transaction_by_id(&self, _id: &str) -> anyhow::Result<Option<SignedTransaction>> {
        Ok(None)
    }
}

// --- Mock Price Oracle ---
struct TestPriceOracle;
#[async_trait::async_trait]
impl ironix_pay::services::price::PriceOracle for TestPriceOracle {
    async fn get_trx_usdt_price(&self) -> anyhow::Result<rust_decimal::Decimal> {
        Ok(rust_decimal::Decimal::new(10, 2)) // 0.10
    }
    async fn get_native_usdt_price(
        &self,
        _network: ironix_pay::entity::Network,
    ) -> anyhow::Result<rust_decimal::Decimal> {
        Ok(rust_decimal::Decimal::new(10, 2)) // 0.10
    }
}

#[tokio::test]
async fn test_full_lifecycle_happy_path() -> anyhow::Result<()> {
    // Set NO_PROXY to allow wiremock
    std::env::set_var("NO_PROXY", "127.0.0.1,localhost");
    common::init_logger();

    // 1. Setup Environment
    let mock_server = MockServer::start().await;
    let webhook_url = mock_server.uri() + "/webhook";

    // Expect 1 webhook call
    Mock::given(method("POST"))
        .and(path("/webhook"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let test_db = common::setup_test_db().await;
    let db = &test_db.conn;

    // 2. Setup Services
    let encryption_key = [0u8; 32];

    // Webhook Service
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

    let _tron_client: Arc<dyn TronBroadcaster + Send + Sync> = Arc::new(TestTronClient);

    // Initialize Mock Sweep Executor
    struct TestSweepExecutor;
    #[async_trait::async_trait]
    impl ironix_pay::services::sweeper::executor::SweepExecutor for TestSweepExecutor {
        async fn get_balance(&self, _address: &str, _token_contract: &str) -> anyhow::Result<i64> {
            Ok(50_000_000) // 50 USDT
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
                tx_hash: "tx_hash_123".to_string(),
                funding_tx_hash: None,
                amount_swept: 50_000_000,
                gas_cost_native: 350_000, // mock: 0.35 TRX
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
            Ok(1000)
        }
    }

    // Initialize Sweeper
    let mut sweeper_config = SweeperConfig::default();
    sweeper_config.platform_treasury_address = Some("TTreasuryAddr".to_string());
    let sweeper_service = SweeperService::new(
        db.clone(),
        Arc::new(TestSweepExecutor),
        Arc::new(TestPriceOracle),
        sweeper_config,
        0.5,
        Environment::Production,
        Network::Tron,
        alerting_service,
        Arc::new(ironix_pay::services::outbound::OutboundTransactionStore::for_tests(db.clone())),
    );

    // 3. Create Merchant & Config
    let merchant = merchants::ActiveModel {
        id: Set("mer_lifecycle".to_string()),
        name: Set("Lifecycle Merchant".to_string()),
        status: Set(merchants::MerchantStatus::Active),

        account_index: Set(Some(1)),
        ..Default::default()
    };
    merchant.insert(db).await?;

    // Create Chain Account (TronMainnet) with balance
    let chain_account = merchant_chain_accounts::ActiveModel {
        merchant_id: Set("mer_lifecycle".to_string()),
        environment: Set(Environment::Production),
        network: Set(Network::Tron),
        usdt_balance: Set(100_000_000),
        usdc_balance: Set(0),
        collection_address: Set(Some("TColAddr".to_string())),
        xpub_encrypted: Set("e_xpub_mock".to_string()),
        ..Default::default()
    };
    chain_account.insert(db).await?;

    // 4. Create Webhook Endpoint
    let endpoint = webhook_endpoints::ActiveModel {
        id: Set("ep_life".to_string()),
        merchant_id: Set("mer_lifecycle".to_string()),
        url: Set(webhook_url.clone()),
        status: Set(webhook_endpoints::EndpointStatus::Enabled),
        secret_encrypted: Set(
            ironix_pay::crypto::encrypt_aes_gcm("test_secret", &[0u8; 32]).unwrap(),
        ),
        environment: Set(Environment::Production),
        ..Default::default()
    };
    endpoint.insert(db).await?;

    // 5. Create Address (Assigned)
    let address = addresses::ActiveModel {
        network: Set("TRON".to_string()),
        address: Set("TAllocatedAddr".to_string()),
        merchant_id: Set("mer_lifecycle".to_string()),
        path_index: Set(0),
        status: Set(addresses::AddressStatus::Assigned),
        usdt_balance: Set(0),
        ..Default::default()
    };
    address.insert(db).await?;

    // 6. Create Session (Pending)
    let session = checkout_sessions::ActiveModel {
        id: Set("cs_life".to_string()),
        merchant_id: Set("mer_lifecycle".to_string()),
        network: Set("TRON".to_string()),
        pay_address: Set("TAllocatedAddr".to_string()),
        amount_expected: Set(50_000_000), // 50 USDT
        amount_received: Set(0),
        pricing_currency: Set("USDT".to_string()),
        pricing_amount: Set(rust_decimal::Decimal::new(50, 0)),
        exchange_rate: Set(rust_decimal::Decimal::new(1, 0)),
        status: Set(checkout_sessions::SessionStatus::Pending),
        currency: Set("USDT".to_string()),
        currency_contract: Set("TR7...".to_string()),
        expires_at: Set((chrono::Utc::now() + chrono::Duration::hours(1)).into()),
        ..Default::default()
    };
    session.insert(db).await?;

    // 7. Simulate Payment (Atomic Update)
    // Normally Indexer does this: Update Address Balance & Session Status + Queue Webhook

    // A. Update Address Balance
    let mut addr_model: addresses::ActiveModel =
        Addresses::find_by_id(("TRON".to_string(), "TAllocatedAddr".to_string()))
            .one(db)
            .await?
            .unwrap()
            .into();
    addr_model.usdt_balance = Set(50_000_000); // 50 USDT
    addr_model.status = Set(addresses::AddressStatus::Detected);
    addr_model.update(db).await?;

    // B. Update Session Status
    let mut sess_model: checkout_sessions::ActiveModel =
        CheckoutSessions::find_by_id("cs_life".to_string())
            .one(db)
            .await?
            .unwrap()
            .into();
    sess_model.status = Set(checkout_sessions::SessionStatus::Paid);
    sess_model.amount_received = Set(50_000_000);
    sess_model.update(db).await?;

    // C. Queue Webhook
    let payload = serde_json::json!({"event": "payment.success"});
    let event_ids = webhook_service
        .queue_event(
            "cs_life",
            "mer_lifecycle",
            Network::Tron,
            Environment::Production,
            "payment.success",
            &payload,
        )
        .await?;

    // 8. Trigger Webhook Delivery

    // Wait for webhook (MockServer expectation verifies this implicitly if we wait enough)
    // But we manually check DB status too
    tokio::time::sleep(tokio::time::Duration::from_millis(3000)).await;

    let event = WebhookEvents::find_by_id(&event_ids[0])
        .one(db)
        .await?
        .unwrap();
    assert_eq!(event.status, webhook_events::WebhookEventStatus::Success);

    // 9. Trigger Sweeper
    Arc::new(sweeper_service).broadcast_cycle().await?;

    // 10. Verify Sweep
    // A. Check Address Status -> Sweeping (simulates tx sent)
    let addr_final = Addresses::find_by_id(("TRON".to_string(), "TAllocatedAddr".to_string()))
        .one(db)
        .await?
        .unwrap();
    assert_eq!(
        addr_final.status,
        addresses::AddressStatus::Sweeping,
        "Address should be sweeping"
    );

    // B. Check Sweep Transaction Record
    let sweep_tx = outbound_transactions::Entity::find()
        .filter(outbound_transactions::Column::FromAddress.eq("TAllocatedAddr"))
        .one(db)
        .await?
        .expect("Sweep transaction should exist");

    assert_eq!(
        sweep_tx.state,
        ironix_pay::entity::outbound_transactions::OutboundState::Pending
    );
    assert_eq!(sweep_tx.amount, 50_000_000);

    Ok(())
}
