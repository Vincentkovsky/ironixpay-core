use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{ActiveModelTrait, ColumnTrait, Database, EntityTrait, QueryFilter, Set};
use std::sync::Arc;
use tokio::sync::Mutex;
// WalletConfig removed
use ironix_pay::entity::{
    addresses, checkout_sessions, merchant_chain_accounts, merchants,
    Environment as EntityEnvironment, Network,
};
use ironix_pay::services::alerting::AlertingService;
use ironix_pay::services::price::PriceOracle;
use ironix_pay::services::sweeper::executor::{SweepExecutor, SweepResult, SweepTxStatus};
use ironix_pay::services::sweeper::{SweeperConfig, SweeperService};
use uuid::Uuid;

/// Mock SweepExecutor for unit tests.
/// Configurable balance and broadcast behavior.
struct MockSweepExecutor {
    balances: Arc<Mutex<std::collections::HashMap<String, i64>>>,
}

impl MockSweepExecutor {
    fn new() -> Self {
        Self {
            balances: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    async fn set_balance(&self, address: &str, balance: i64) {
        self.balances
            .lock()
            .await
            .insert(address.to_string(), balance);
    }
}

#[async_trait]
impl SweepExecutor for MockSweepExecutor {
    async fn get_balance(&self, address: &str, _token_contract: &str) -> Result<i64> {
        let balances = self.balances.lock().await;
        Ok(*balances.get(address).unwrap_or(&0))
    }

    async fn execute_sweep(
        &self,
        from_address: &str,
        _to_address: &str,
        _account_index: i32,
        _path_index: u32,
        _token_contract: &str,
        _outbound_id: &str,
        _outbound_store: &ironix_pay::services::outbound::OutboundTransactionStore,
    ) -> Result<SweepResult> {
        let balances = self.balances.lock().await;
        let balance = *balances.get(from_address).unwrap_or(&0);
        Ok(SweepResult {
            tx_hash: format!("mock_tx_{}", uuid::Uuid::new_v4()),
            funding_tx_hash: None,
            amount_swept: balance,
            gas_cost_native: 350_000, // mock: 0.35 TRX
            broadcast_disposition: ironix_pay::services::outbound::BroadcastDisposition::Accepted,
        })
    }

    async fn check_tx_status(
        &self,
        _tx_hash: &str,
        _required_confirmations: i32,
    ) -> Result<SweepTxStatus> {
        Ok(SweepTxStatus::Confirmed)
    }

    async fn get_current_block(&self) -> Result<i64> {
        Ok(1000)
    }
}

struct MockOracle;
#[async_trait]
impl PriceOracle for MockOracle {
    async fn get_trx_usdt_price(&self) -> Result<Decimal> {
        Ok(Decimal::new(15, 2)) // 0.15
    }
    async fn get_native_usdt_price(&self, _network: Network) -> Result<Decimal> {
        Ok(Decimal::new(15, 2)) // 0.15
    }
}

use testcontainers::runners::AsyncRunner;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;

// ... imports ...

fn init_logger() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .with_test_writer()
        .try_init();
}

async fn create_test_service(
    config: SweeperConfig,
) -> (
    SweeperService,
    Arc<MockSweepExecutor>,
    sea_orm::DatabaseConnection,
    ContainerAsync<Postgres>, // Keep container alive
) {
    // Start Postgres container
    let container = Postgres::default().start().await.unwrap();
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let db_url = format!("postgres://postgres:postgres@{}:{}/postgres", host, port);

    // Connect to DB
    let db = Database::connect(&db_url).await.unwrap();

    // Run migrations to setup schema
    use sea_orm_migration::MigratorTrait;
    migration::Migrator::up(&db, None).await.unwrap();

    let mock_executor = Arc::new(MockSweepExecutor::new());
    let price_oracle: Arc<dyn PriceOracle> = Arc::new(MockOracle);

    let service = SweeperService::new(
        db.clone(),
        mock_executor.clone(),
        price_oracle,
        config,
        0.5,
        EntityEnvironment::Sandbox,
        Network::Tron,
        Arc::new(AlertingService::new(
            None,
            ironix_pay::entity::Environment::Sandbox,
        )),
        Arc::new(ironix_pay::services::outbound::OutboundTransactionStore::for_tests(db.clone())),
    );

    (service, mock_executor, db, container)
}

#[tokio::test]
async fn test_init_config() {
    // Just verify instantiation works
    let config = SweeperConfig::default();
    let (_service, _client, _db, _container) = create_test_service(config).await;
}

#[tokio::test]
async fn test_should_sweep_integration() {
    init_logger();
    let mut config = SweeperConfig::default();
    config.platform_treasury_address = Some("TTreasuryTestAddress".to_string());

    let (service, mock_executor, db, _container) = create_test_service(config).await;

    // 1. Setup Merchant
    let merchant_id = Uuid::new_v4();
    let merchant = merchants::ActiveModel {
        id: Set(merchant_id.to_string()),
        name: Set("Test Merchant".to_string()),
        // min_sweep_threshold removed
        ..Default::default()
    };
    merchant.insert(&db).await.unwrap();

    // Setup Chain Account (Shasta) with balance
    let chain_account = merchant_chain_accounts::ActiveModel {
        merchant_id: Set(merchant_id.to_string()),
        environment: Set(EntityEnvironment::Sandbox),
        network: Set(Network::Tron),
        usdt_balance: Set(100_000_000),
        usdc_balance: Set(0),
        collection_address: Set(Some("TCollectionAddr".to_string())),
        xpub_encrypted: Set("dummy_xpub_for_test".to_string()),
        ..Default::default()
    };
    chain_account.insert(&db).await.unwrap();

    // 2. Setup Address (Detected)
    let address_str = "TTestAddress1234567890".to_string();
    let address = addresses::ActiveModel {
        network: Set("TRON".to_string()),
        address: Set(address_str.clone()),
        status: Set(ironix_pay::entity::addresses::AddressStatus::Detected),
        merchant_id: Set(merchant_id.to_string()),
        path_index: Set(0),
        usdt_balance: Set(0), // Will be updated by sweeper logic from chain
        sweep_attempts: Set(0),
        updated_at: Set(Utc::now().into()),
        created_at: Set(Utc::now().into()),
        ..Default::default()
    };
    address.insert(&db).await.unwrap();

    // 3. Setup Session (Paid)
    let session = checkout_sessions::ActiveModel {
        id: Set("sess_001".to_string()),
        merchant_id: Set(merchant_id.to_string()),
        network: Set("TRON".to_string()),
        pay_address: Set(address_str.clone()),
        status: Set(ironix_pay::entity::checkout_sessions::SessionStatus::Paid),
        amount_expected: Set(100_000_000),
        currency: Set("USDT".to_string()),
        currency_contract: Set("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".to_string()),
        pricing_currency: Set("USDT".to_string()),
        pricing_amount: Set(rust_decimal::Decimal::new(100, 0)),
        exchange_rate: Set(rust_decimal::Decimal::ONE),
        created_at: Set(Utc::now().into()),
        expires_at: Set(Utc::now()
            .checked_add_signed(chrono::Duration::hours(1))
            .unwrap()
            .into()),
        ..Default::default()
    };
    session.insert(&db).await.unwrap();

    // 4. Set Mock Chain Balance (Above threshold)
    // 50 USDT
    mock_executor.set_balance(&address_str, 50_000_000).await;

    // 5. Run Broadcast Cycle
    Arc::new(service).broadcast_cycle().await.unwrap();

    // 6. Verify Status Changed to Sweeping
    let updated_addr = addresses::Entity::find()
        .filter(addresses::Column::Address.eq(&address_str))
        .one(&db)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        updated_addr.status,
        ironix_pay::entity::addresses::AddressStatus::Sweeping,
        "Address should be sweeping after successful check"
    );
}

#[tokio::test]
async fn test_should_not_sweep_insufficient_gas() {
    let config = SweeperConfig::default();

    let (service, mock_executor, db, _container) = create_test_service(config).await;

    // 1. Setup Merchant with LOW gas
    let merchant_id = Uuid::new_v4();
    let merchant = merchants::ActiveModel {
        id: Set(merchant_id.to_string()),
        name: Set("NoGasMerchant".to_string()),
        // min_sweep_threshold removed
        ..Default::default()
    };
    merchant.insert(&db).await.unwrap();

    // Setup Chain Account (Shasta) - NO balance
    let chain_account = merchant_chain_accounts::ActiveModel {
        merchant_id: Set(merchant_id.to_string()),
        environment: Set(EntityEnvironment::Sandbox),
        network: Set(Network::Tron),
        usdt_balance: Set(0),
        usdc_balance: Set(0),
        collection_address: Set(Some("TCollectionAddr".to_string())),
        xpub_encrypted: Set("dummy_xpub_for_test".to_string()),
        ..Default::default()
    };
    chain_account.insert(&db).await.unwrap();

    // 2. Setup Address
    let address_str = "TNoGasAddress".to_string();
    let address = addresses::ActiveModel {
        network: Set("TRON".to_string()),
        address: Set(address_str.clone()),
        status: Set(ironix_pay::entity::addresses::AddressStatus::Detected),
        merchant_id: Set(merchant_id.to_string()),
        path_index: Set(1),
        usdt_balance: Set(0),
        sweep_attempts: Set(0),
        updated_at: Set(Utc::now().into()),
        created_at: Set(Utc::now().into()),
        ..Default::default()
    };
    address.insert(&db).await.unwrap();

    // 3. Setup Session (Paid)
    let session = checkout_sessions::ActiveModel {
        id: Set("sess_002".to_string()),
        merchant_id: Set(merchant_id.to_string()),
        network: Set("TRON".to_string()),
        pay_address: Set(address_str.clone()),
        status: Set(ironix_pay::entity::checkout_sessions::SessionStatus::Paid),
        amount_expected: Set(100_000_000),
        currency: Set("USDT".to_string()),
        currency_contract: Set("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".to_string()),
        pricing_currency: Set("USDT".to_string()),
        pricing_amount: Set(rust_decimal::Decimal::new(100, 0)),
        exchange_rate: Set(rust_decimal::Decimal::ONE),
        created_at: Set(Utc::now().into()),
        expires_at: Set(Utc::now()
            .checked_add_signed(chrono::Duration::hours(1))
            .unwrap()
            .into()),
        ..Default::default()
    };
    session.insert(&db).await.unwrap();

    // 4. Set Balance
    mock_executor.set_balance(&address_str, 50_000_000).await;

    // 5. Run Broadcast Cycle
    Arc::new(service).broadcast_cycle().await.unwrap();

    // 6. Verify Status REMAINS Detected
    let updated_addr = addresses::Entity::find()
        .filter(addresses::Column::Address.eq(&address_str))
        .one(&db)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        updated_addr.status,
        ironix_pay::entity::addresses::AddressStatus::Detected,
        "Address should NOT sweep if gas credit is insufficient"
    );
}

#[tokio::test]
async fn test_should_not_sweep_insufficient_balance() {
    let mut config = SweeperConfig::default();

    config.stagnant_address_hours = 24 * 7;

    let (service, mock_executor, db, _container) = create_test_service(config).await;

    // 1. Setup Merchant
    let merchant_id = Uuid::new_v4();
    let merchant = merchants::ActiveModel {
        id: Set(merchant_id.to_string()),
        name: Set("LowBalMerchant".to_string()),
        // min_sweep_threshold removed
        ..Default::default()
    };
    merchant.insert(&db).await.unwrap();

    // Setup Chain Account (Shasta) with balance
    let chain_account = merchant_chain_accounts::ActiveModel {
        merchant_id: Set(merchant_id.to_string()),
        environment: Set(EntityEnvironment::Sandbox),
        network: Set(Network::Tron),
        usdt_balance: Set(100_000_000),
        usdc_balance: Set(0),
        collection_address: Set(Some("TCollectionAddr".to_string())),
        xpub_encrypted: Set("dummy_xpub_for_test".to_string()),
        ..Default::default()
    };
    chain_account.insert(&db).await.unwrap();

    // 2. Setup Address
    let address_str = "TLowBalAddress".to_string();
    let address = addresses::ActiveModel {
        network: Set("TRON".to_string()),
        address: Set(address_str.clone()),
        status: Set(ironix_pay::entity::addresses::AddressStatus::Detected),
        merchant_id: Set(merchant_id.to_string()),
        path_index: Set(2),
        updated_at: Set(Utc::now().into()), // Recent update, not stagnant
        created_at: Set(Utc::now().into()),
        ..Default::default()
    };
    address.insert(&db).await.unwrap();

    // 3. Setup Session (Paid)
    let session = checkout_sessions::ActiveModel {
        id: Set("sess_003".to_string()),
        merchant_id: Set(merchant_id.to_string()),
        network: Set("TRON".to_string()),
        pay_address: Set(address_str.clone()),
        status: Set(ironix_pay::entity::checkout_sessions::SessionStatus::Paid),
        amount_expected: Set(100_000_000),
        currency: Set("USDT".to_string()),
        currency_contract: Set("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".to_string()),
        pricing_currency: Set("USDT".to_string()),
        pricing_amount: Set(rust_decimal::Decimal::new(100, 0)),
        exchange_rate: Set(rust_decimal::Decimal::ONE),
        created_at: Set(Utc::now().into()),
        expires_at: Set(Utc::now()
            .checked_add_signed(chrono::Duration::hours(1))
            .unwrap()
            .into()),
        ..Default::default()
    };
    session.insert(&db).await.unwrap();

    // 4. Set Balance (Below threshold)
    // 5 USDT
    mock_executor.set_balance(&address_str, 5_000_000).await;

    // 5. Run Broadcast Cycle
    Arc::new(service).broadcast_cycle().await.unwrap();

    // 6. Verify Status REMAINS Detected
    let updated_addr = addresses::Entity::find()
        .filter(addresses::Column::Address.eq(&address_str))
        .one(&db)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        updated_addr.status,
        ironix_pay::entity::addresses::AddressStatus::Detected,
        "Address should NOT sweep if balance is too low"
    );
}
