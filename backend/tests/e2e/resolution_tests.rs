use ironix_pay::api::dtos::resolution::TransferRequest;
use ironix_pay::entity::payment_exceptions::{self, ExceptionStatus, ExceptionType, Resolution};
use ironix_pay::entity::Network;
use ironix_pay::services::alerting::AlertingService;
use ironix_pay::services::billing::BillingService;
use ironix_pay::services::energy::{EnergyManager, EnergyReceipt, EnergyRentalProvider};
use ironix_pay::services::merchant::MerchantService;
use ironix_pay::services::payout::executor::{PayoutExecutor, PayoutResult};
use ironix_pay::services::resolution::service::ResolutionService;
use ironix_pay::services::tron::interface::TronBroadcaster;
use ironix_pay::services::webhook::service::WebhookService;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use secrecy::Secret;
use std::sync::Arc;

use crate::common;

// --- Mocks ---
struct MockTronClient;

#[async_trait::async_trait]
impl TronBroadcaster for MockTronClient {
    async fn get_usdt_balance(&self, _addr: &str) -> anyhow::Result<u64> {
        Ok(100_000_000) // Sufficient balance
    }
    async fn get_trc20_balance(&self, _addr: &str, _token_contract: &str) -> anyhow::Result<i64> {
        Ok(100_000_000)
    }
    async fn get_trx_balance(&self, _addr: &str) -> anyhow::Result<u64> {
        Ok(100_000_000) // 100 TRX
    }
    async fn build_trc20_transfer(
        &self,
        _from: &str,
        _to: &str,
        _amount: u64,
        _contract: &str,
    ) -> anyhow::Result<ironix_pay::services::tron::interface::UnsignedTransaction> {
        Ok(ironix_pay::services::tron::interface::UnsignedTransaction {
            raw_data: vec![],
            raw_data_hex: "mock_tx".to_string(),
            raw_data_json: None,
            expiration: None,
        })
    }
    fn sign_transaction(
        &self,
        _tx: &ironix_pay::services::tron::interface::UnsignedTransaction,
        _pk: &[u8],
    ) -> anyhow::Result<ironix_pay::services::tron::interface::SignedTransaction> {
        unimplemented!()
    }
    async fn broadcast(
        &self,
        _tx: &ironix_pay::services::tron::interface::SignedTransaction,
    ) -> anyhow::Result<ironix_pay::services::tron::interface::BroadcastResult> {
        Ok(ironix_pay::services::tron::interface::BroadcastResult {
            success: true,
            tx_hash: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
            message: None,
        })
    }
    async fn build_trx_transfer(
        &self,
        _f: &str,
        _t: &str,
        _a: u64,
    ) -> anyhow::Result<ironix_pay::services::tron::interface::UnsignedTransaction> {
        unimplemented!()
    }
    async fn get_current_block(
        &self,
    ) -> anyhow::Result<ironix_pay::services::tron::interface::BlockInfo> {
        Ok(ironix_pay::services::tron::interface::BlockInfo {
            number: 200,
            timestamp: 0,
        })
    }
    async fn get_transaction_info(
        &self,
        tx: &str,
    ) -> anyhow::Result<Option<ironix_pay::services::tron::interface::TransactionInfo>> {
        if tx == "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" {
            Ok(Some(
                ironix_pay::services::tron::interface::TransactionInfo {
                    tx_hash: tx.to_string(),
                    block_number: 100,
                    success: true,
                    result: None,
                    fee_burned: 0,
                    revert_message: None,
                },
            ))
        } else {
            Ok(None)
        }
    }
    async fn get_account_resources(
        &self,
        _a: &str,
    ) -> anyhow::Result<ironix_pay::services::tron::interface::AccountResource> {
        Ok(ironix_pay::services::tron::interface::AccountResource {
            free_net_used: 0,
            free_net_limit: 5000,
            asset_net_used: vec![],
            net_limit: 10000,
            net_used: 0,
            energy_limit: 100000,
            energy_used: 0,
        })
    }
    async fn estimate_energy(&self, _o: &str, _c: &str, _f: &str, _p: &str) -> anyhow::Result<i64> {
        Ok(0)
    }
    async fn get_transaction_by_id(
        &self,
        _tx_hash: &str,
    ) -> anyhow::Result<Option<ironix_pay::services::tron::interface::SignedTransaction>> {
        Ok(None)
    }
}

struct MockEnergyProvider;
#[async_trait::async_trait]
impl EnergyRentalProvider for MockEnergyProvider {
    async fn delegate_energy(&self, _target: &str, _amount: u64) -> anyhow::Result<EnergyReceipt> {
        Ok(EnergyReceipt {
            order_id: "mock".to_string(),
            trx_hash: "mock".to_string(),
            energy_amount: 0,
            cost_sun: 0,
            expires_at: 0,
        })
    }
}

// --- Mock Payout Executor ---
struct MockPayoutExecutor;

#[async_trait::async_trait]
impl PayoutExecutor for MockPayoutExecutor {
    async fn execute_payout(
        &self,
        _from_address: &str,
        _to_address: &str,
        _amount: u64,
        _account_index: i32,
        _path_index: u32,
        _token_contract: &str,
        _token_decimals: u8,
        _outbound_id: &str,
        _outbound_store: &ironix_pay::services::outbound::OutboundTransactionStore,
    ) -> anyhow::Result<PayoutResult> {
        Ok(PayoutResult {
            tx_hash: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
            funding_tx_hash: None,
            broadcast_disposition: ironix_pay::services::outbound::BroadcastDisposition::Accepted,
        })
    }

    async fn check_tx_status(
        &self,
        _tx_hash: &str,
        _min_confirmations: u64,
    ) -> anyhow::Result<ironix_pay::entity::transactions::ChainTxState> {
        Ok(ironix_pay::entity::transactions::ChainTxState::Confirmed)
    }
}

async fn insert_test_merchant(db: &DatabaseConnection, id: &str) -> anyhow::Result<()> {
    use ironix_pay::entity::merchants::{self, MerchantStatus};
    use ironix_pay::entity::{org_members, users};

    let now = chrono::Utc::now().fixed_offset();

    // 1. Create merchant (org)
    let merchant = merchants::ActiveModel {
        id: Set(id.to_string()),
        name: Set("Test Merchant".to_string()),
        status: Set(MerchantStatus::Active),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    merchants::Entity::insert(merchant).exec(db).await?;

    // 2. Create user (auth identity) — reuse merchant ID for simplicity
    let user = users::ActiveModel {
        id: Set(id.to_string()),
        email: Set(format!("{}@test.com", id)),
        password_hash: Set("dummy_hash".to_string()),
        name: Set("Test User".to_string()),
        is_totp_enabled: Set(false),
        token_version: Set(0),
        email_verified: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    users::Entity::insert(user).exec(db).await?;

    // 3. Create org_member (owner)
    let member = org_members::ActiveModel {
        id: Set(format!("om_{}", id)),
        org_id: Set(id.to_string()),
        user_id: Set(Some(id.to_string())),
        role: Set(org_members::MemberRole::Owner),
        status: Set(org_members::MemberStatus::Active),
        accepted_at: Set(Some(now)),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    org_members::Entity::insert(member).exec(db).await?;

    Ok(())
}

async fn insert_test_address(
    db: &DatabaseConnection,
    merchant_id: &str,
    address: &str,
) -> anyhow::Result<()> {
    use ironix_pay::entity::addresses;
    let addr = addresses::ActiveModel {
        address: Set(address.to_string()),
        network: Set("TRON".to_string()),
        merchant_id: Set(merchant_id.to_string()),
        path_index: Set(0),
        ..Default::default()
    };
    addresses::Entity::insert(addr).exec(db).await?;
    Ok(())
}

#[tokio::test]
async fn test_resolution_accept() -> anyhow::Result<()> {
    common::init_logger();
    let test_db = common::setup_test_db().await;
    let db = &test_db.conn;

    insert_test_merchant(db, "mer_1").await?;
    insert_test_address(db, "mer_1", "TReceiver").await?;

    // Chain account needed for balance updates during resolution
    use ironix_pay::entity::{merchant_chain_accounts, Environment as EntityEnv};
    let chain_account = merchant_chain_accounts::ActiveModel {
        merchant_id: Set("mer_1".to_string()),
        environment: Set(EntityEnv::Sandbox),
        network: Set(Network::Tron),
        usdt_balance: Set(0),
        usdc_balance: Set(0),
        collection_address: Set(Some("TCollectionAddr".to_string())),
        xpub_encrypted: Set("dummy_xpub".to_string()),
        ..Default::default()
    };
    chain_account.insert(db).await?;

    let merchant_service = Arc::new(MerchantService::new(
        db.clone(),
        secrecy::Secret::new("test_secret".to_string()),
        24,
        ironix_pay::config::Environment::Sandbox,
    ));

    let tron_client: Arc<dyn TronBroadcaster + Send + Sync> = Arc::new(MockTronClient);
    let _energy_manager = Arc::new(EnergyManager::new(
        tron_client.clone(),
        Arc::new(MockEnergyProvider),
        ironix_pay::config::Environment::Sandbox,
        None,
        None,
        "TR7NH...".to_string(),
    ));

    let billing_service = Arc::new(BillingService::new());
    let fee_config = Arc::new(ironix_pay::services::billing::fee_config::FeeConfig::default());
    let webhook_service = Arc::new(WebhookService::new(
        db.clone(),
        Secret::new("test_webhook_key".to_string()),
        5,
        3,
        Arc::new(AlertingService::new(
            None,
            ironix_pay::entity::Environment::Sandbox,
        )),
    ));
    let service = ResolutionService::new(
        db.clone(),
        merchant_service.clone(),
        billing_service,
        fee_config,
        Arc::new(AlertingService::new(
            None,
            ironix_pay::entity::Environment::Sandbox,
        )),
        webhook_service,
        None,
        ironix_pay::entity::Environment::Sandbox,
        std::collections::HashMap::new(), // payout_executors
        std::collections::HashMap::new(), // chain_deposit_floors
        Arc::new(ironix_pay::services::outbound::OutboundTransactionStore::for_tests(db.clone())),
    );

    use ironix_pay::entity::checkout_sessions;
    let session = checkout_sessions::ActiveModel {
        id: Set("sess_accept".to_string()),
        merchant_id: Set("mer_1".to_string()),
        status: Set(checkout_sessions::SessionStatus::Expired),
        pay_address: Set("TReceiver".to_string()),
        amount_expected: Set(100),
        amount_received: Set(0),
        pricing_currency: Set("USDT".to_string()),
        pricing_amount: Set(rust_decimal::Decimal::new(100, 6)),
        exchange_rate: Set(rust_decimal::Decimal::new(1, 0)),
        currency: Set("USDT".to_string()),
        currency_contract: Set("TR7NH...".to_string()),
        network: Set("TRON".to_string()),
        expires_at: Set(chrono::Utc::now().into()),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    };
    session.insert(db).await?;

    let ex = payment_exceptions::ActiveModel {
        id: Set("ex_accept".to_string()),
        merchant_id: Set(Some("mer_1".to_string())),
        status: Set(ExceptionStatus::Pending),
        network: Set("TRON".to_string()),
        tx_hash: Set("tx_accept_123".to_string()),
        to_address: Set("TReceiver".to_string()),
        from_address: Set("TSender".to_string()),
        amount: Set(100),
        currency_symbol: Set("USDT".to_string()),
        exception_type: Set(ExceptionType::SessionExpired),
        session_id: Set(Some("sess_accept".to_string())),
        log_index: Set(0),
        block_number: Set(100),
        block_timestamp: Set(chrono::Utc::now().into()),
        ..Default::default()
    };
    ex.insert(db).await?;

    service
        .accept_expired_session("ex_accept", "mer_1", &["mer_1".to_string()])
        .await?;

    let ex = payment_exceptions::Entity::find_by_id("ex_accept")
        .one(db)
        .await?
        .unwrap();
    assert_eq!(ex.status, ExceptionStatus::Resolved);
    assert_eq!(ex.resolution, Some(Resolution::Accepted));
    assert!(ex.resolved_at.is_some());

    // Verify Session is also updated
    let session = checkout_sessions::Entity::find_by_id("sess_accept")
        .one(db)
        .await?
        .unwrap();
    assert_eq!(session.status, checkout_sessions::SessionStatus::Paid);

    Ok(())
}

#[tokio::test]
async fn test_resolution_transfer() -> anyhow::Result<()> {
    let test_db = common::setup_test_db().await;
    let db = &test_db.conn;

    insert_test_merchant(db, "mer_1").await?;

    // Create chain account with sufficient balance for billing
    use ironix_pay::entity::{merchant_chain_accounts, Environment, Network};
    let chain_account = merchant_chain_accounts::ActiveModel {
        merchant_id: Set("mer_1".to_string()),
        environment: Set(Environment::Sandbox),
        network: Set(Network::Tron),
        usdt_balance: Set(100_000_000),
        usdc_balance: Set(0),
        collection_address: Set(Some("TCollectionAddr".to_string())),
        xpub_encrypted: Set("dummy_xpub_for_test".to_string()),
        ..Default::default()
    };
    chain_account.insert(db).await?;

    // Need an address in DB for the signer to find the path_index
    use ironix_pay::entity::addresses;
    let addr = addresses::ActiveModel {
        address: Set("TReceiver".to_string()),
        network: Set("TRON".to_string()),
        merchant_id: Set("mer_1".to_string()),
        path_index: Set(5),
        usdt_balance: Set(10_000_000),
        ..Default::default()
    };
    addr.insert(db).await?;

    let merchant_service = Arc::new(MerchantService::new(
        db.clone(),
        secrecy::Secret::new("test_secret".to_string()),
        24,
        ironix_pay::config::Environment::Sandbox,
    ));

    let tron_client: Arc<dyn TronBroadcaster + Send + Sync> = Arc::new(MockTronClient);
    let _energy_manager = Arc::new(EnergyManager::new(
        tron_client.clone(),
        Arc::new(MockEnergyProvider),
        ironix_pay::config::Environment::Sandbox,
        None,
        None,
        "TR7NH...".to_string(),
    ));

    let billing_service = Arc::new(BillingService::new());
    let fee_config = Arc::new(ironix_pay::services::billing::fee_config::FeeConfig::default());
    let webhook_service = Arc::new(WebhookService::new(
        db.clone(),
        Secret::new("test_webhook_key".to_string()),
        5,
        3,
        Arc::new(AlertingService::new(
            None,
            ironix_pay::entity::Environment::Sandbox,
        )),
    ));
    // Wire up payout_executors for transfer test
    let mut payout_executors: std::collections::HashMap<Network, Arc<dyn PayoutExecutor>> =
        std::collections::HashMap::new();
    payout_executors.insert(Network::Tron, Arc::new(MockPayoutExecutor));

    let service = ResolutionService::new(
        db.clone(),
        merchant_service.clone(),
        billing_service,
        fee_config,
        Arc::new(AlertingService::new(
            None,
            ironix_pay::entity::Environment::Sandbox,
        )),
        webhook_service,
        None,
        ironix_pay::entity::Environment::Sandbox,
        payout_executors,
        std::collections::HashMap::new(), // chain_deposit_floors
        Arc::new(ironix_pay::services::outbound::OutboundTransactionStore::for_tests(db.clone())),
    );

    // A stale pre-signing claim must fail its active root before the exception is retried.
    payment_exceptions::ActiveModel {
        id: Set("ex_stale_preparing".to_string()),
        merchant_id: Set(Some("mer_1".to_string())),
        status: Set(ExceptionStatus::Processing),
        network: Set("TRON".to_string()),
        tx_hash: Set("tx_stale_preparing".to_string()),
        to_address: Set("TReceiver".to_string()),
        from_address: Set("TSender".to_string()),
        amount: Set(10_000_000),
        currency_symbol: Set("USDT".to_string()),
        exception_type: Set(ExceptionType::Unknown),
        log_index: Set(0),
        block_number: Set(99),
        block_timestamp: Set(chrono::Utc::now().into()),
        updated_at: Set((chrono::Utc::now() - chrono::Duration::minutes(2)).into()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    let stale_outbound_id = ironix_pay::services::outbound::new_id();
    let mut stale_outbound = ironix_pay::services::outbound::preparing_model(
        stale_outbound_id.clone(),
        "mer_1".to_string(),
        ironix_pay::entity::Environment::Sandbox,
        ironix_pay::entity::outbound_transactions::OutboundOperationType::ManualTransfer,
        "TRON".to_string(),
        "TReceiver".to_string(),
        "TMuA6YqfCeX8EhbfYEg5y7S4DqzSJireY9".to_string(),
        8_500_000,
        "USDT".to_string(),
    );
    stale_outbound.exception_id = Set(Some("ex_stale_preparing".to_string()));
    ironix_pay::services::outbound::create_attempt(db, stale_outbound).await?;

    service.recover_stale_processing().await;
    assert_eq!(
        payment_exceptions::Entity::find_by_id("ex_stale_preparing")
            .one(db)
            .await?
            .unwrap()
            .status,
        ExceptionStatus::Pending
    );
    assert_eq!(
        ironix_pay::entity::outbound_transactions::Entity::find_by_id(&stale_outbound_id)
            .one(db)
            .await?
            .unwrap()
            .state,
        ironix_pay::entity::outbound_transactions::OutboundState::Failed
    );

    let ex = payment_exceptions::ActiveModel {
        id: Set("ex_transfer".to_string()),
        merchant_id: Set(Some("mer_1".to_string())),
        status: Set(ExceptionStatus::Pending),
        network: Set("TRON".to_string()),
        tx_hash: Set("tx_orig_123".to_string()),
        to_address: Set("TReceiver".to_string()),
        from_address: Set("TSender".to_string()),
        amount: Set(10_000_000), // 10 USDT
        currency_symbol: Set("USDT".to_string()),
        exception_type: Set(ExceptionType::Unknown),
        log_index: Set(0),
        block_number: Set(100),
        block_timestamp: Set(chrono::Utc::now().into()),
        ..Default::default()
    };
    ex.insert(db).await?;

    let result = service
        .manual_transfer(
            "ex_transfer",
            "mer_1",
            &["mer_1".to_string()],
            TransferRequest {
                to_address: "TMuA6YqfCeX8EhbfYEg5y7S4DqzSJireY9".to_string(),
                amount: Some("0.5".to_string()), // Test decimal input (0.5 USDT = 500,000 Sun)
                notes: Some("Refund test".to_string()),
                code: "000000".to_string(),
            },
        )
        .await?;
    // Async payout flow: manual_transfer spawns background task and returns "submitted"
    assert_eq!(result, "submitted");

    // Wait briefly for async payout to process
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let ex = payment_exceptions::Entity::find_by_id("ex_transfer")
        .one(db)
        .await?
        .unwrap();
    // Async flow: status transitions to Processing (sync part), then Resolved (async part)
    // After the background task completes, it should be Resolved
    assert!(
        ex.status == ExceptionStatus::Processing || ex.status == ExceptionStatus::Resolved,
        "Expected Processing or Resolved, got {:?}",
        ex.status,
    );

    Ok(())
}

#[tokio::test]
async fn test_resolution_attach() -> anyhow::Result<()> {
    let test_db = common::setup_test_db().await;
    let db = &test_db.conn;

    insert_test_merchant(db, "mer_1").await?;
    insert_test_address(db, "mer_1", "TAddr").await?;

    // Chain account needed for balance updates during resolution
    use ironix_pay::entity::{merchant_chain_accounts, Environment as EntityEnv};
    let chain_account = merchant_chain_accounts::ActiveModel {
        merchant_id: Set("mer_1".to_string()),
        environment: Set(EntityEnv::Sandbox),
        network: Set(Network::Tron),
        usdt_balance: Set(0),
        usdc_balance: Set(0),
        collection_address: Set(Some("TCollectionAddr".to_string())),
        xpub_encrypted: Set("dummy_xpub".to_string()),
        ..Default::default()
    };
    chain_account.insert(db).await?;

    // Need a target session
    use ironix_pay::entity::checkout_sessions;
    let session = checkout_sessions::ActiveModel {
        id: Set("sess_target".to_string()),
        merchant_id: Set("mer_1".to_string()),
        status: Set(checkout_sessions::SessionStatus::Expired),
        pay_address: Set("TAddr".to_string()),
        amount_expected: Set(1000000),
        amount_received: Set(0),
        pricing_currency: Set("USDT".to_string()),
        pricing_amount: Set(rust_decimal::Decimal::new(1, 0)),
        exchange_rate: Set(rust_decimal::Decimal::new(1, 0)),
        currency: Set("USDT".to_string()),
        currency_contract: Set("TR7NH...".to_string()),
        network: Set("TRON".to_string()),
        expires_at: Set(chrono::Utc::now().into()),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    };
    session.insert(db).await?;

    let merchant_service = Arc::new(MerchantService::new(
        db.clone(),
        secrecy::Secret::new("test_secret".to_string()),
        24,
        ironix_pay::config::Environment::Sandbox,
    ));

    let tron_client: Arc<dyn TronBroadcaster + Send + Sync> = Arc::new(MockTronClient);
    let _energy_manager = Arc::new(EnergyManager::new(
        tron_client.clone(),
        Arc::new(MockEnergyProvider),
        ironix_pay::config::Environment::Sandbox,
        None,
        None,
        "TR7NH...".to_string(),
    ));

    let billing_service = Arc::new(BillingService::new());
    let fee_config = Arc::new(ironix_pay::services::billing::fee_config::FeeConfig::default());
    let webhook_service = Arc::new(WebhookService::new(
        db.clone(),
        Secret::new("test_webhook_key".to_string()),
        5,
        3,
        Arc::new(AlertingService::new(
            None,
            ironix_pay::entity::Environment::Sandbox,
        )),
    ));
    let service = ResolutionService::new(
        db.clone(),
        merchant_service.clone(),
        billing_service,
        fee_config,
        Arc::new(AlertingService::new(
            None,
            ironix_pay::entity::Environment::Sandbox,
        )),
        webhook_service,
        None,
        ironix_pay::entity::Environment::Sandbox,
        std::collections::HashMap::new(), // payout_executors
        std::collections::HashMap::new(), // chain_deposit_floors
        Arc::new(ironix_pay::services::outbound::OutboundTransactionStore::for_tests(db.clone())),
    );

    let ex = payment_exceptions::ActiveModel {
        id: Set("ex_attach".to_string()),
        merchant_id: Set(Some("mer_1".to_string())),
        status: Set(ExceptionStatus::Pending),
        network: Set("TRON".to_string()),
        exception_type: Set(ExceptionType::NoActiveSession),
        amount: Set(1000000),
        currency_symbol: Set("USDT".to_string()),
        to_address: Set("TAddr".to_string()),
        from_address: Set("TSender".to_string()),
        tx_hash: Set("tx_attach_123".to_string()),
        log_index: Set(0),
        block_number: Set(100),
        block_timestamp: Set(chrono::Utc::now().into()),
        ..Default::default()
    };
    ex.insert(db).await?;

    service
        .attach_session("ex_attach", "mer_1", &["mer_1".to_string()], "sess_target")
        .await?;

    let ex = payment_exceptions::Entity::find_by_id("ex_attach")
        .one(db)
        .await?
        .unwrap();
    assert_eq!(ex.status, ExceptionStatus::Resolved);
    assert_eq!(ex.resolution, Some(Resolution::Attached));
    assert_eq!(ex.resolution_ref_id, Some("sess_target".to_string()));
    assert_eq!(ex.session_id, Some("sess_target".to_string()));

    // Verify Session is updated
    let session = checkout_sessions::Entity::find_by_id("sess_target")
        .one(db)
        .await?
        .unwrap();
    assert_eq!(session.amount_received, 1000000);
    assert_eq!(session.status, checkout_sessions::SessionStatus::Paid);

    Ok(())
}
