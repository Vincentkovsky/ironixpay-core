//! EVM Sweeper Logic Tests
//!
//! Mirror of sweeper_logic_tests.rs but using BSC-specific data:
//! - Network::Bsc + "BSC" network string
//! - 0x-prefixed EVM addresses
//! - MockSweepExecutor for chain-agnostic sweep logic
//!
//! Validates that broadcast_cycle correctly handles EVM addresses.

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{ActiveModelTrait, ColumnTrait, Database, EntityTrait, QueryFilter, Set};
use std::sync::Arc;
use tokio::sync::Mutex;

use ironix_pay::entity::{
    addresses, checkout_sessions, merchant_chain_accounts, merchants,
    Environment as EntityEnvironment, Network,
};
use ironix_pay::services::alerting::AlertingService;
use ironix_pay::services::price::PriceOracle;
use ironix_pay::services::sweeper::executor::{SweepExecutor, SweepResult, SweepTxStatus};
use ironix_pay::services::sweeper::{SweeperConfig, SweeperService};
use uuid::Uuid;

/// Mock SweepExecutor (identical to sweeper_logic_tests but reusable for EVM scenarios)
struct MockEvmSweepExecutor {
    balances: Arc<Mutex<std::collections::HashMap<String, i64>>>,
}

impl MockEvmSweepExecutor {
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
impl SweepExecutor for MockEvmSweepExecutor {
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
            tx_hash: format!("0xmock_{}", uuid::Uuid::new_v4()),
            funding_tx_hash: Some(format!("0xfund_{}", uuid::Uuid::new_v4())), // EVM has gas funding
            amount_swept: balance,
            gas_cost_native: 50_000_000_000_000u64, // mock: ~0.00005 BNB in wei
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
        Ok(50_000_000) // BSC block height
    }
}

struct MockEvmOracle;
#[async_trait]
impl PriceOracle for MockEvmOracle {
    async fn get_trx_usdt_price(&self) -> Result<Decimal> {
        Ok(Decimal::new(15, 2)) // Not used for EVM
    }
    async fn get_native_usdt_price(&self, _network: Network) -> Result<Decimal> {
        // BNB price ~600 USDT
        Ok(Decimal::new(600, 0))
    }
}

use testcontainers::runners::AsyncRunner;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;

fn init_logger() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .with_test_writer()
        .try_init();
}

async fn create_bsc_test_service(
    config: SweeperConfig,
) -> (
    SweeperService,
    Arc<MockEvmSweepExecutor>,
    sea_orm::DatabaseConnection,
    ContainerAsync<Postgres>,
) {
    let container = Postgres::default().start().await.unwrap();
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let db_url = format!("postgres://postgres:postgres@{}:{}/postgres", host, port);

    let db = Database::connect(&db_url).await.unwrap();

    use sea_orm_migration::MigratorTrait;
    migration::Migrator::up(&db, None).await.unwrap();

    let mock_executor = Arc::new(MockEvmSweepExecutor::new());
    let price_oracle: Arc<dyn PriceOracle> = Arc::new(MockEvmOracle);

    let service = SweeperService::new(
        db.clone(),
        mock_executor.clone(),
        price_oracle,
        config,
        0.5,
        EntityEnvironment::Sandbox,
        Network::Bsc,
        Arc::new(AlertingService::new(
            None,
            ironix_pay::entity::Environment::Sandbox,
        )),
        Arc::new(ironix_pay::services::outbound::OutboundTransactionStore::for_tests(db.clone())),
    );

    (service, mock_executor, db, container)
}

// ── BSC-specific test data helpers ──────────────────────────────────────

/// EVM-format address (0x-prefixed, EIP-55 checksummed)
const BSC_TEST_ADDR: &str = "0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B";
/// BSC USDT contract on mainnet
const BSC_USDT_CONTRACT: &str = "0x55d398326f99059fF775485246999027B3197955";
/// Platform treasury (EVM format)
const BSC_TREASURY: &str = "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97";

#[tokio::test]
async fn test_bsc_sweep_happy_path() {
    init_logger();
    let mut config = SweeperConfig::default();
    config.platform_treasury_address = Some(BSC_TREASURY.to_string());

    let (service, mock_executor, db, _container) = create_bsc_test_service(config).await;

    // 1. Setup Merchant
    let merchant_id = Uuid::new_v4();
    let merchant = merchants::ActiveModel {
        id: Set(merchant_id.to_string()),
        name: Set("BSC Test Merchant".to_string()),
        ..Default::default()
    };
    merchant.insert(&db).await.unwrap();

    // 2. Setup Chain Account for BSC
    let chain_account = merchant_chain_accounts::ActiveModel {
        merchant_id: Set(merchant_id.to_string()),
        environment: Set(EntityEnvironment::Sandbox),
        network: Set(Network::Bsc),
        usdt_balance: Set(100_000_000),
        usdc_balance: Set(0),
        collection_address: Set(Some(BSC_TREASURY.to_string())),
        xpub_encrypted: Set("dummy_evm_xpub_for_test".to_string()),
        ..Default::default()
    };
    chain_account.insert(&db).await.unwrap();

    // 3. Setup Address (Detected, EVM format)
    let address = addresses::ActiveModel {
        network: Set("BSC".to_string()),
        address: Set(BSC_TEST_ADDR.to_string()),
        status: Set(addresses::AddressStatus::Detected),
        merchant_id: Set(merchant_id.to_string()),
        path_index: Set(0),
        usdt_balance: Set(0),
        sweep_attempts: Set(0),
        updated_at: Set(Utc::now().into()),
        created_at: Set(Utc::now().into()),
        ..Default::default()
    };
    address.insert(&db).await.unwrap();

    // 4. Setup Session (Paid, BSC network)
    let session = checkout_sessions::ActiveModel {
        id: Set("sess_bsc_001".to_string()),
        merchant_id: Set(merchant_id.to_string()),
        network: Set("BSC".to_string()),
        pay_address: Set(BSC_TEST_ADDR.to_string()),
        status: Set(checkout_sessions::SessionStatus::Paid),
        amount_expected: Set(50_000_000), // 50 USDT (6-decimal)
        currency: Set("USDT".to_string()),
        currency_contract: Set(BSC_USDT_CONTRACT.to_string()),
        pricing_currency: Set("USDT".to_string()),
        pricing_amount: Set(rust_decimal::Decimal::new(50, 0)),
        exchange_rate: Set(rust_decimal::Decimal::ONE),
        created_at: Set(Utc::now().into()),
        expires_at: Set(Utc::now()
            .checked_add_signed(chrono::Duration::hours(1))
            .unwrap()
            .into()),
        ..Default::default()
    };
    session.insert(&db).await.unwrap();

    // 5. Set Mock Balance (50 USDT in 6-decimal)
    mock_executor.set_balance(BSC_TEST_ADDR, 50_000_000).await;

    // 6. Run Broadcast Cycle
    Arc::new(service).broadcast_cycle().await.unwrap();

    // 7. Verify Address Status → Sweeping
    let updated_addr = addresses::Entity::find()
        .filter(addresses::Column::Address.eq(BSC_TEST_ADDR))
        .one(&db)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        updated_addr.status,
        addresses::AddressStatus::Sweeping,
        "BSC address should transition to Sweeping after successful broadcast_cycle"
    );
}

#[tokio::test]
async fn test_bsc_sweep_no_treasury() {
    init_logger();
    let config = SweeperConfig::default(); // No treasury configured

    let (service, mock_executor, db, _container) = create_bsc_test_service(config).await;

    // Setup minimal BSC merchant + address
    let merchant_id = Uuid::new_v4();
    let merchant = merchants::ActiveModel {
        id: Set(merchant_id.to_string()),
        name: Set("No Treasury Merchant".to_string()),
        ..Default::default()
    };
    merchant.insert(&db).await.unwrap();

    let chain_account = merchant_chain_accounts::ActiveModel {
        merchant_id: Set(merchant_id.to_string()),
        environment: Set(EntityEnvironment::Sandbox),
        network: Set(Network::Bsc),
        usdt_balance: Set(100_000_000),
        usdc_balance: Set(0),
        collection_address: Set(Some(BSC_TREASURY.to_string())),
        xpub_encrypted: Set("dummy_xpub".to_string()),
        ..Default::default()
    };
    chain_account.insert(&db).await.unwrap();

    let addr_str = "0x1234567890AbCdEf1234567890aBcDeF12345678";
    let address = addresses::ActiveModel {
        network: Set("BSC".to_string()),
        address: Set(addr_str.to_string()),
        status: Set(addresses::AddressStatus::Detected),
        merchant_id: Set(merchant_id.to_string()),
        path_index: Set(1),
        usdt_balance: Set(0),
        sweep_attempts: Set(0),
        updated_at: Set(Utc::now().into()),
        created_at: Set(Utc::now().into()),
        ..Default::default()
    };
    address.insert(&db).await.unwrap();

    let session = checkout_sessions::ActiveModel {
        id: Set("sess_bsc_no_treasury".to_string()),
        merchant_id: Set(merchant_id.to_string()),
        network: Set("BSC".to_string()),
        pay_address: Set(addr_str.to_string()),
        status: Set(checkout_sessions::SessionStatus::Paid),
        amount_expected: Set(10_000_000),
        currency: Set("USDT".to_string()),
        currency_contract: Set(BSC_USDT_CONTRACT.to_string()),
        pricing_currency: Set("USDT".to_string()),
        pricing_amount: Set(rust_decimal::Decimal::new(50, 0)),
        exchange_rate: Set(rust_decimal::Decimal::ONE),
        created_at: Set(Utc::now().into()),
        expires_at: Set(Utc::now()
            .checked_add_signed(chrono::Duration::hours(1))
            .unwrap()
            .into()),
        ..Default::default()
    };
    session.insert(&db).await.unwrap();

    mock_executor.set_balance(addr_str, 10_000_000).await;

    // Broadcast cycle should skip (no treasury configured)
    Arc::new(service).broadcast_cycle().await.unwrap();

    let updated = addresses::Entity::find()
        .filter(addresses::Column::Address.eq(addr_str))
        .one(&db)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        updated.status,
        addresses::AddressStatus::Detected,
        "Address should remain Detected when treasury is not configured"
    );
}
