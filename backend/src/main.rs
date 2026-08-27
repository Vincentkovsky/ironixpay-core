//! Tron Checkout Backend
//!
//! A financial-grade USDT payment engine for the Tron blockchain.
//! Aligned with docs/system_design.md

use std::collections::HashMap;
use std::process;
use std::sync::Arc;

use axum::Router;

use axum_prometheus::metrics_exporter_prometheus::{Matcher, PrometheusBuilder};
use axum_prometheus::{utils::SECONDS_DURATION_BUCKETS, PrometheusMetricLayerBuilder};
use sea_orm::Database;
use sea_orm_migration::prelude::*;
use secrecy::ExposeSecret;
use tokio::net::TcpListener;
use tokio::signal;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Use types from library crate
use ironix_pay::api::{self, middleware::rate_limit::RateLimitKeyExtractor};
use ironix_pay::config::Config;
use ironix_pay::entity::Network;
use ironix_pay::migration::Migrator;
use ironix_pay::services::solana::{
    indexer::SolanaIndexer, noop_scanner::SolanaBridgeScanner, sweep_executor::SolanaSweepExecutor,
    SolanaClient,
};
use ironix_pay::services::{
    self, address::AddressManager, alerting::AlertingService, aml::AmlService,
    chain::traits::ChainClient, chain_health::ChainHealthRegistry, checkout::CheckoutService,
    indexer::TransactionIndexer, merchant::MerchantService,
    payment_processor::PaymentEventProcessor, supervisor::supervisor_loop, tron::TronClient,
    webhook::WebhookService,
};
use ironix_pay::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Fail-Fast: Install global panic hook
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_panic(info);
        error!(
            "CRITICAL: Thread panicked! Initiating fail-fast shutdown. Info: {:?}",
            info
        );
        process::exit(1);
    }));

    // 2. Production Hardening: Conditional Dotenv
    // We must check the ENVIRONMENT variable *before* loading Config,
    // because Config::from_env() relies on variables that might be in .env.
    //
    // Policy:
    // - Production/Sandbox (Docker/K8s): Env vars injected via YAML. Do NOT load .env (avoid I/O & overrides).
    // - Development/Local: Load .env.
    // - Default (missing var): Load .env (assume local).
    let env_str = std::env::var("ENVIRONMENT")
        .unwrap_or_default()
        .to_lowercase();
    let is_deployed = matches!(env_str.as_str(), "production" | "prod" | "sandbox");

    if !is_deployed {
        dotenvy::dotenv().ok();
    }

    // Load config (now that .env is potentially loaded)
    let config = Config::from_env();

    // 3. Sentry Error Tracking (must init BEFORE tracing subscriber)
    //    SENTRY_DSN env var controls activation — empty/missing = disabled (noop)
    let _sentry_guard = sentry::init(sentry::ClientOptions {
        dsn: std::env::var("SENTRY_DSN")
            .ok()
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().expect("Invalid SENTRY_DSN")),
        environment: Some(config.environment.to_string().into()),
        release: sentry::release_name!(),
        traces_sample_rate: if is_deployed { 0.1 } else { 1.0 },
        attach_stacktrace: true,
        ..Default::default()
    });

    // 4. Production Hardening: Non-blocking logging + Sentry layer
    let (non_blocking, _log_guard) = tracing_appender::non_blocking(std::io::stdout());
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,ironix_pay=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer().with_writer(non_blocking))
        .with(sentry::integrations::tracing::layer())
        .init();

    info!("Starting Tron Checkout on {}:{}", config.host, config.port);

    // Create cancellation token
    let cancel_token = CancellationToken::new();

    // Fail-Fast: Unified JoinSet
    let mut background_tasks = JoinSet::new();

    // Spawn signal handler task
    let signal_token = cancel_token.clone();
    background_tasks.spawn(async move {
        let sc = async {
            signal::ctrl_c()
                .await
                .expect("failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("failed to install signal handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = sc => info!("Received Ctrl+C, initiating graceful shutdown..."),
            _ = terminate => info!("Received SIGTERM, initiating graceful shutdown..."),
            _ = signal_token.cancelled() => info!("Internal shutdown requested, stopping signal handler"),
        }

        signal_token.cancel();
        Ok::<(), anyhow::Error>(())
    });

    // Connect to database
    let mut db_opt = sea_orm::ConnectOptions::new(config.database_url.expose_secret());
    db_opt.max_connections(config.database_max_connections);
    db_opt.connect_timeout(std::time::Duration::from_secs(10));
    db_opt.acquire_timeout(std::time::Duration::from_secs(10));

    // Disable SQL logging in deployed environments (Prod/Sandbox) to prevent noise/leaks
    let enable_sql_log = !is_deployed || std::env::var("SQL_LOG").unwrap_or_default() == "true";

    db_opt.sqlx_logging(enable_sql_log);
    db_opt.sqlx_logging_level(tracing::log::LevelFilter::Debug);

    info!(
        max_connections = config.database_max_connections,
        sql_log_enabled = enable_sql_log,
        "Connecting to database..."
    );

    let db = Database::connect(db_opt).await?;
    info!("Connected to database");

    info!("Running database migrations...");
    Migrator::up(&db, None).await?;
    info!("Migrations completed successfully");

    // Reset stuck Processing exceptions from previous run (server crash recovery)
    reset_stuck_processing_exceptions(&db).await?;

    // Derive USDT contract from the canonical Network::chain_config().
    // This is the SINGLE SOURCE OF TRUTH for contract addresses per environment.
    let network_entity =
        Network::from_str_lenient(&config.network).expect("Invalid network configured");
    let entity_env = config.environment.to_entity_environment();
    let chain_config = network_entity.chain_config(&entity_env);
    let usdt_contract = chain_config.usdt_contract;
    info!(usdt_contract = %usdt_contract, "USDT contract derived from chain config");

    // Initialize services
    let tron_rpc_urls = config
        .chains
        .get("TRON")
        .map(|chain| chain.rpc_urls.clone())
        .filter(|urls| !urls.is_empty())
        .unwrap_or_else(|| vec![config.tron_full_node.clone()]);
    let tron_client = Arc::new(TronClient::new_with_endpoints(
        tron_rpc_urls,
        usdt_contract.clone(),
        config.trongrid_api_key.clone(),
    ));

    // ─── Key Provider Selection ────────────────────────────────────────────
    // If AWS_KMS_KEY_ID is set: use KmsEnvelopeProvider (per-request decrypt)
    // Otherwise: use LocalMnemonicProvider (sandbox, mnemonic from .env)

    use services::address::key_provider::{
        KmsEnvelopeProvider, LocalMnemonicProvider, MasterKeyProvider, MasterKeyProviderBox,
    };

    let use_kms = config.aws_kms_key_id.is_some() && config.encrypted_seed.is_some();

    let (
        master_key_provider,
        transaction_signer,
        gas_sponsor_private_key,
        treasury_addr,
        gas_sponsor_addr,
    ) = if use_kms {
        // ── KMS Mode (Production) ──────────────────────────────────
        let kms_key_id = config
            .aws_kms_key_id
            .clone()
            .expect("AWS_KMS_KEY_ID is required when ENCRYPTED_SEED is set");
        let encrypted_seed_b64 = config
            .encrypted_seed
            .as_ref()
            .expect("ENCRYPTED_SEED required for KMS mode")
            .expose_secret()
            .clone();

        info!("🔐 Initializing KMS Envelope Provider (per-request decrypt)");
        let provider = KmsEnvelopeProvider::new(kms_key_id.clone(), &encrypted_seed_b64)
            .await
            .expect("Failed to initialize KMS Envelope Provider");

        // Derive treasury address via KMS provider
        let treasury_xpub = provider
            .get_account_xpub(0)
            .await
            .expect("Failed to derive treasury xpub via KMS");
        let treasury = services::address::hd_wallet::derive_tron_address(&treasury_xpub, 0)
            .expect("Failed to derive treasury address from xpub");
        info!(treasury_address = %treasury, "Derived treasury address via KMS");

        // Derive gas sponsor address: same xpub, path_index=1
        let gs_addr = services::address::hd_wallet::derive_tron_address(&treasury_xpub, 1)
            .expect("Failed to derive gas sponsor address from xpub");
        info!(gas_sponsor_address = %gs_addr, "Derived gas sponsor address via KMS");

        // Gas Sponsor Key: HD-derived at m/44'/195'/0'/0/1 (platform reserved, path_index=1)
        let gas_key: Option<Vec<u8>> = {
            let key = provider
                .derive_raw_private_key(0, 1, 195) // account=0, path=1, TRON
                .await
                .expect("Failed to derive gas sponsor key via KMS");
            info!("🔐 Gas sponsor key derived via KMS (m/44'/195'/0'/0/1)");
            Some(key)
        };

        // Clone provider for Box (MasterKeyProvider) and Arc (TransactionSigner)
        let mkp: MasterKeyProviderBox = Box::new(provider.clone());
        let ts: Arc<dyn services::address::key_provider::TransactionSigner + Send + Sync> =
            Arc::new(provider);

        (mkp, ts, gas_key, treasury, gs_addr)
    } else {
        // ── Local Mode (Sandbox) ────────────────────────────────────
        info!("🔑 Using Local Mnemonic Provider (Sandbox)");
        let mnemonic = config
            .master_mnemonic
            .clone()
            .expect("MASTER_MNEMONIC required in non-KMS mode");

        let mkp: MasterKeyProviderBox = Box::new(LocalMnemonicProvider::new(mnemonic.clone()));
        let ts: Arc<dyn services::address::key_provider::TransactionSigner + Send + Sync> =
            Arc::new(LocalMnemonicProvider::new(mnemonic.clone()));

        // Gas Sponsor Key: HD-derived at m/44'/195'/0'/0/1 (platform reserved, path_index=1)
        let gas_key: Option<Vec<u8>> = {
            let key = services::address::hd_wallet::derive_private_key_from_mnemonic(
                mnemonic.expose_secret(),
                0, // account_index = 0 (platform)
                1, // path_index = 1 (gas sponsor)
            )
            .expect("Failed to derive gas sponsor key from mnemonic");
            info!("🔑 Gas sponsor key derived from mnemonic (m/44'/195'/0'/0/1)");
            Some(key.to_vec())
        };

        // Treasury derivation: direct mnemonic access
        let treasury_xpub = services::address::hd_wallet::derive_account_xpub_from_mnemonic(
            mnemonic.expose_secret(),
            0,
        )
        .expect("Failed to derive treasury xpub");
        let treasury = services::address::hd_wallet::derive_tron_address(&treasury_xpub, 0)
            .expect("Failed to derive treasury address");
        info!(treasury_address = %treasury, "Derived treasury address from mnemonic");

        // Gas sponsor address: same xpub, path_index=1
        let gs_addr = services::address::hd_wallet::derive_tron_address(&treasury_xpub, 1)
            .expect("Failed to derive gas sponsor address");
        info!(gas_sponsor_address = %gs_addr, "Derived gas sponsor address from mnemonic");

        (mkp, ts, gas_key, treasury, gs_addr)
    };

    // ── EVM Platform Xpub (derived before AddressManager takes ownership of provider) ──
    // Needed for BSC treasury/gas_sponsor HD derivation in both KMS and Local modes.
    let evm_platform_xpub: Option<String> = if config.chains.contains_key("BSC") {
        let xpub = master_key_provider
            .get_account_xpub_for_coin(0, 60)
            .await
            .expect("Failed to derive EVM platform xpub (account=0, coin_type=60)");
        info!("Derived EVM platform xpub (account=0, coin_type=60)");
        Some(xpub)
    } else {
        None
    };

    // ── Solana Treasury Address (HD-derived, hardcoded account=0, path=0) ──
    // Matches TRON/EVM convention: treasury is always at the first key of the platform account.
    let solana_treasury_address: Option<String> = if config.solana.is_some() {
        let derived = master_key_provider
            .batch_derive_addresses(
                0,   // account_index = 0 (platform)
                501, // Solana coin_type
                0,   // path_index = 0 (treasury)
                1,
            )
            .await
            .expect("Failed to derive Solana treasury address");
        let addr = derived[0].1.clone();
        info!(solana_treasury = %addr, "Derived Solana treasury address (m/44'/501'/0'/0')");
        Some(addr)
    } else {
        None
    };

    let address_manager = Arc::new(AddressManager::new(
        db.clone(),
        config.encryption_key.clone(),
        master_key_provider,
    ));

    // ...

    // ── Exchange Rate Service (early init for CheckoutService injection) ──
    // Sandbox delays initial CoinGecko sync by 5s to stagger with prod and avoid 429
    let rate_startup_delay = if matches!(env_str.as_str(), "sandbox") {
        5
    } else {
        0
    };
    let exchange_rate_service = Arc::new(services::exchange_rate::ExchangeRateService::new(
        db.clone(),
        config.coingecko_api_key.clone(),
        config.rate_sync_interval,
        rate_startup_delay,
    ));

    let checkout_service = Arc::new(CheckoutService::new(
        db.clone(),
        config.session_expiry_minutes,
        Some(exchange_rate_service.clone()),
    ));

    // ── Production DB Connection (Sandbox only, for JIT TOTP sync) ──
    let prod_db: Option<sea_orm::DatabaseConnection> =
        if matches!(config.environment, ironix_pay::config::Environment::Sandbox) {
            if let Some(ref prod_url) = config.production_database_url {
                match Database::connect(prod_url.expose_secret()).await {
                    Ok(conn) => {
                        info!("Connected to production database for TOTP sync");
                        Some(conn)
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "Failed to connect to production database — TOTP sync disabled"
                        );
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

    let merchant_service = MerchantService::new(
        db.clone(),
        config.jwt_secret.clone(),
        config.jwt_expiry_hours,
        config.environment.clone(),
    );

    let email_sender: Arc<dyn services::email::EmailSender> =
        if let Ok(api_key) = std::env::var("RESEND_API_KEY") {
            info!("Initializing Resend Email Sender");
            let from =
                std::env::var("EMAIL_FROM").unwrap_or_else(|_| "onboarding@resend.dev".to_string());
            let base_url = std::env::var("BASE_URL")
                .unwrap_or_else(|_| format!("http://{}:{}", config.host, config.port));
            Arc::new(
                services::email::resend::ResendEmailService::try_new(api_key, from, base_url)
                    .expect("Failed to initialize Resend Email Service"),
            )
        } else {
            info!("RESEND_API_KEY not found, using Dummy Email Sender");
            Arc::new(services::email::dummy::DummyEmailService::default())
        };

    // Build enabled networks list early (needed by MerchantService for JIT shadow accounts)
    let enabled_networks = {
        let mut nets = vec![Network::Tron]; // TRON always enabled
        for key in config.chains.keys() {
            if let Some(net) = Network::from_str_lenient(key) {
                if net != Network::Tron {
                    nets.push(net);
                }
            }
        }
        // Solana has its own config section (not in chains HashMap)
        if config.solana.is_some() {
            nets.push(Network::Solana);
        }
        nets
    };

    let mut merchant_svc_builder = merchant_service
        .with_email_service(email_sender.clone())
        .with_address_manager(address_manager.clone())
        .with_enabled_networks(enabled_networks.clone())
        .with_blocked_email_domains(config.blocked_email_domains.clone());
    if let Some(pdb) = prod_db {
        merchant_svc_builder = merchant_svc_builder.with_production_db(pdb);
    }
    let merchant_service = Arc::new(merchant_svc_builder);
    let lead_notification_email = std::env::var("LEAD_NOTIFICATION_EMAIL")
        .unwrap_or_else(|_| "support@ironixpay.com".to_string());
    let lead_service = Arc::new(services::lead::LeadService::new(
        db.clone(),
        email_sender,
        lead_notification_email,
    ));
    let turnstile_service = config.turnstile_secret_key.clone().map(|secret_key| {
        info!(
            expected_hostname = %config.turnstile_expected_hostname,
            "Cloudflare Turnstile registration verification enabled"
        );
        Arc::new(services::turnstile::TurnstileService::new(
            secret_key,
            config.turnstile_expected_hostname.clone(),
        ))
    });
    if turnstile_service.is_none() {
        warn!("Cloudflare Turnstile registration verification is disabled");
    }

    let energy_provider: Arc<dyn services::energy::EnergyRentalProvider> =
        if let Some(key) = &config.netts_api_key {
            info!("Initializing Netts Energy Provider");
            Arc::new(
                services::energy::NettsEnergyProvider::try_new(
                    key.expose_secret().clone(),
                    "https://netts.io/apiv2".to_string(),
                )
                .expect("Failed to initialize Netts Energy Provider"),
            ) as Arc<dyn services::energy::EnergyRentalProvider>
        } else {
            info!("No Netts API key found, using dummy provider");
            Arc::new(services::energy::DummyEnergyProvider::default())
                as Arc<dyn services::energy::EnergyRentalProvider>
        };

    let energy_manager = Arc::new(services::energy::EnergyManager::new(
        tron_client.clone(),
        energy_provider.clone(),
        config.environment.clone(),
        gas_sponsor_private_key.clone(),
        Some(gas_sponsor_addr.clone()),
        usdt_contract.clone(),
    ));

    let transaction_monitor = Arc::new(
        services::transaction_monitor::service::TransactionMonitor::new(tron_client.clone()),
    );

    // Treasury address was derived above in the provider selection block
    info!(treasury_address = %treasury_addr, "Platform TRON treasury address ready");

    // Safety assertion: if chains.toml declares TRON addresses, verify they match HD-derived.
    // Catches misconfigured mnemonics at startup instead of losing funds at runtime.
    if let Some(tron_cfg) = config.chains.get("TRON") {
        if let Some(ref declared) = tron_cfg.treasury_address {
            if declared != &treasury_addr {
                panic!(
                    "CRITICAL: TRON treasury address mismatch!\n\
                     HD-derived:  {}\n\
                     chains.toml: {}\n\
                     This likely means MASTER_MNEMONIC / ENCRYPTED_SEED does not match the declared treasury. \
                     Fix chains.toml or check your seed configuration.",
                    treasury_addr, declared
                );
            }
            info!("✅ TRON treasury address matches chains.toml declaration");
        }

        if let Some(ref gs) = tron_cfg.gas_sponsor {
            if let Some(ref declared_addr) = gs.address {
                if declared_addr != &gas_sponsor_addr {
                    panic!(
                        "CRITICAL: TRON gas sponsor address mismatch!\n\
                         HD-derived (m/44'/195'/0'/0/1):  {}\n\
                         chains.toml gas_sponsor.address:  {}\n\
                         Fix chains.toml or check your seed configuration.",
                        gas_sponsor_addr, declared_addr
                    );
                }
                info!("✅ TRON gas sponsor address matches chains.toml declaration");
            }
        }
    }

    let sweeper_config = services::sweeper::SweeperConfig {
        confirmation_blocks: config.sweep_confirmation_blocks,
        energy_estimate: config.sweep_energy_estimate,
        platform_treasury_address: Some(treasury_addr.clone()),
        ..Default::default()
    };

    let price_oracle: Arc<dyn services::price::PriceOracle> = Arc::new(
        services::price::BinancePriceOracle::try_new(Some(60))
            .expect("Failed to initialize Binance Price Oracle"),
    );

    let billing_service = Arc::new(services::billing::BillingService::new());
    let outbound_store = Arc::new(
        services::outbound::OutboundTransactionStore::try_new(
            db.clone(),
            config.encryption_key.clone(),
        )
        .expect("Failed to initialize outbound transaction encryption"),
    );

    // Construct chain-specific sweep executor
    let broadcaster: Arc<dyn services::tron::interface::TronBroadcaster + Send + Sync> =
        tron_client.clone();
    let tron_sweep_executor: Arc<dyn services::sweeper::executor::SweepExecutor> =
        Arc::new(services::sweeper::executor::TronSweepExecutor::new(
            tron_client.clone(),
            broadcaster,
            energy_manager.clone(),
            transaction_monitor.clone(),
            transaction_signer.clone(),
        ));

    // MVP Alerting: Initialize AlertingService (must be before services that depend on it)
    let alerting_webhook = std::env::var("ALERT_WEBHOOK_URL").ok();
    if alerting_webhook.is_some() {
        info!("AlertingService configured with Slack/DingTalk webhook");
    } else {
        info!("AlertingService running in log-only mode (no ALERT_WEBHOOK_URL)");
    }
    let alerting_service = Arc::new(AlertingService::new(
        alerting_webhook,
        config.environment.to_entity_environment(),
    ));

    // ── Service Health Registry ──────────────────────────────────────────
    // Heartbeat-based monitoring for background services.
    // Used by /ready (informational, fail-open) and admin /system/health.
    // Created early so services can attach via .with_health().
    let service_health = services::ServiceHealthRegistry::new(&[
        "payment_processor",
        "tron_sweeper",
        "webhook_recovery",
        "payout_worker",
    ]);
    let sweeper_service = Arc::new(
        services::sweeper::SweeperService::new(
            db.clone(),
            tron_sweep_executor,
            price_oracle,
            sweeper_config.clone(),
            config.sweep_threshold_ratio,
            config.environment.to_entity_environment(),
            ironix_pay::entity::Network::Tron,
            alerting_service.clone(),
            outbound_store.clone(),
        )
        .with_health(service_health.clone(), "tron_sweeper".to_string()),
    );

    let fee_config = Arc::new(services::billing::fee_config::FeeConfig::default());

    let webhook_service = Arc::new(
        WebhookService::new(
            db.clone(),
            config.encryption_key.clone(),
            10,
            7,
            alerting_service.clone(),
        )
        .with_health(service_health.clone(), "webhook_recovery".to_string()),
    );

    // ── EVM Chain Address Resolution (HD-derived) ─────────────────────────────
    // All EVM chains share the same HD-derived addresses (coin_type=60).
    // The TRON assertion above validates seed correctness; EVM addresses are
    // purely derived without additional TOML assertions.
    let has_any_evm_chain = config.chains.keys().any(|k| {
        Network::from_str_lenient(k)
            .map(|n| n.chain_family() == ironix_pay::entity::ChainFamily::Evm)
            .unwrap_or(false)
    });
    let (evm_gas_sponsor_address, evm_treasury_address) = if has_any_evm_chain {
        let evm_xpub = evm_platform_xpub
            .as_ref()
            .expect("EVM platform xpub must be derived when any EVM chain is configured");

        // Treasury: m/44'/60'/0'/0/0
        let treasury = services::address::hd_wallet::derive_evm_address(evm_xpub, 0)
            .expect("Failed to derive EVM treasury address");

        // Gas Sponsor: m/44'/60'/0'/0/1
        let gas_sponsor = services::address::hd_wallet::derive_evm_address(evm_xpub, 1)
            .expect("Failed to derive EVM gas sponsor address");

        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        info!("  EVM Treasury:     {}", treasury);
        info!("  EVM Gas Sponsor:  {}", gas_sponsor);
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        (Some(gas_sponsor), Some(treasury))
    } else {
        (None, None)
    };

    // ── Xero Accounting Integration (optional) ──
    let xero_service = if let (Some(client_id), Some(client_secret), Some(redirect_uri)) = (
        config.xero_client_id.clone(),
        config.xero_client_secret.clone(),
        config.xero_redirect_uri.clone(),
    ) {
        info!("Xero accounting integration enabled");
        Some(Arc::new(services::xero::XeroService::new(
            db.clone(),
            config.encryption_key.clone(),
            client_id,
            client_secret,
            redirect_uri,
            Some(exchange_rate_service.clone()),
        )))
    } else {
        None
    };

    // ResolutionService is constructed below, together with PayoutService

    // AML Service for compliance checking (created before PayoutService which needs it)
    let aml_service = Arc::new(AmlService::new(
        db.clone(),
        ironix_pay::services::aml::AmlConfig::default(),
    ));

    // Load AML blacklist into memory cache at startup
    if let Err(e) = aml_service.load_blacklist_from_db().await {
        error!("Failed to load AML blacklist: {}", e);
        // Continue running - L2 API check will still work
    }

    // Save payout gas funders per network so sweepers can share the same Mutex
    let mut solana_payout_client: Option<Arc<SolanaClient>> = None;
    let mut evm_payout_gas_funders: HashMap<Network, Arc<services::evm::gas_funder::EvmGasFunder>> =
        HashMap::new();
    let (payout_service, resolution_service, chain_deposit_floors) = {
        use std::collections::HashMap;
        use tokio::sync::Mutex;

        let mut executors: HashMap<Network, Arc<dyn services::payout::PayoutExecutor>> =
            HashMap::new();
        let mut treasury_addresses: HashMap<Network, String> = HashMap::new();
        // Per-network broadcast lock for treasury nonce serialization.
        // TODO: If scaling backend instances > 1, replace with Postgres advisory lock.
        let mut broadcast_locks: HashMap<Network, Arc<Mutex<()>>> = HashMap::new();

        // TRON executor (always registered)
        executors.insert(
            Network::Tron,
            Arc::new(services::payout::TronPayoutExecutor::new(
                tron_client.clone(),
                transaction_signer.clone(),
                energy_manager.clone(),
            )),
        );
        treasury_addresses.insert(Network::Tron, treasury_addr.clone());
        broadcast_locks.insert(Network::Tron, Arc::new(Mutex::new(())));

        // EVM payout executors: register one per configured EVM chain
        if let Some(ref evm_treasury) = evm_treasury_address {
            let entity_env = config.environment.to_entity_environment();
            for (key, chain_cfg) in &config.chains {
                let network = match Network::from_str_lenient(key) {
                    Some(n) if n.chain_family() == ironix_pay::entity::ChainFamily::Evm => n,
                    _ => continue,
                };
                let chain_config = network.chain_config(&entity_env);
                let chain_id = chain_config.chain_id.unwrap_or(1);
                let evm_client = Arc::new(services::evm::EvmClient::new(
                    chain_cfg.rpc_urls.clone(),
                    chain_id,
                ));
                let gs = chain_cfg.gas_sponsor.as_ref();
                let gas_funder = Arc::new(services::evm::gas_funder::EvmGasFunder::new(
                    evm_client.clone(),
                    transaction_signer.clone(),
                    evm_gas_sponsor_address
                        .clone()
                        .expect("EVM gas sponsor address should be derived"),
                    gs.map(|g| g.account_index).unwrap_or(0),
                    gs.map(|g| g.path_index).unwrap_or(1),
                ));
                // Save for reuse by sweeper (same gas sponsor = same Mutex per chain)
                evm_payout_gas_funders.insert(network, gas_funder.clone());
                executors.insert(
                    network,
                    Arc::new(services::payout::EvmPayoutExecutor::new(
                        evm_client,
                        transaction_signer.clone(),
                        gas_funder,
                    )),
                );
                treasury_addresses.insert(network, evm_treasury.clone());
                broadcast_locks.insert(network, Arc::new(Mutex::new(())));
                info!(network = %key, "Payout executor registered (treasury={})", evm_treasury);
            }
        }

        // Solana payout executor (conditional — only if Solana is configured)
        if let Some(ref solana_cfg) = config.solana {
            let client = Arc::new(SolanaClient::new(
                solana_cfg.rpc_urls.clone(),
                Network::Solana,
            ));
            executors.insert(
                Network::Solana,
                Arc::new(services::payout::SolanaPayoutExecutor::new(
                    client.clone(),
                    transaction_signer.clone(),
                )),
            );
            let sol_treasury = solana_treasury_address
                .clone()
                .expect("Solana treasury address must be derived when Solana is configured");
            treasury_addresses.insert(Network::Solana, sol_treasury.clone());
            broadcast_locks.insert(Network::Solana, Arc::new(Mutex::new(())));
            info!(
                "Payout executor registered for Solana (treasury={})",
                sol_treasury
            );
            solana_payout_client = Some(client);
        }

        // Build per-network outbound fee map from chains.toml
        // Keyed by Network (not ChainFamily) so ETH and L2s can have different fees.
        let mut chain_outbound_fees: HashMap<Network, i64> = HashMap::new();
        let mut chain_deposit_floors: HashMap<Network, i64> = HashMap::new();
        for (key, chain_cfg) in &config.chains {
            if let Some(net) = Network::from_str_lenient(key) {
                if let Some(fee) = chain_cfg.outbound_fee {
                    chain_outbound_fees.insert(net.clone(), fee);
                    info!(
                        chain = key,
                        outbound_fee = fee,
                        "Per-network outbound fee configured"
                    );
                }
                if let Some(floor) = chain_cfg.floor_deposit {
                    chain_deposit_floors.insert(net.clone(), floor);
                    info!(
                        chain = key,
                        floor_deposit = floor,
                        "Per-network deposit floor configured"
                    );
                }
            }
        }
        // Solana fees (separate config section)
        if let Some(ref sol_cfg) = config.solana {
            if let Some(fee) = sol_cfg.outbound_fee {
                chain_outbound_fees.insert(Network::Solana, fee);
                info!(
                    chain = "Solana",
                    outbound_fee = fee,
                    "Per-network outbound fee configured"
                );
            }
            if let Some(floor) = sol_cfg.floor_deposit {
                chain_deposit_floors.insert(Network::Solana, floor);
                info!(
                    chain = "Solana",
                    floor_deposit = floor,
                    "Per-network deposit floor configured"
                );
            }
        }

        // Validate payout infrastructure: warn if any enabled network is missing an executor.
        for net in &enabled_networks {
            if !executors.contains_key(net) {
                warn!(
                    network = ?net,
                    "No PayoutExecutor registered for enabled network. \
                     Payouts will fail for this network until implemented."
                );
            }
            if !treasury_addresses.contains_key(net) {
                warn!(
                    network = ?net,
                    "No treasury address for enabled network. \
                     Payouts disabled for this network."
                );
            }
            if !broadcast_locks.contains_key(net) {
                // Auto-create broadcast lock so the rest of the system works
                broadcast_locks.insert(net.clone(), Arc::new(Mutex::new(())));
            }
        }

        let payout_svc = Arc::new(
            services::payout::PayoutService::new(
                db.clone(),
                billing_service.clone(),
                fee_config.clone(),
                executors.clone(),
                treasury_addresses.clone(),
                broadcast_locks.clone(),
                alerting_service.clone(),
                chain_outbound_fees,
                aml_service.clone(),
                webhook_service.clone(),
                outbound_store.clone(),
            )
            .with_health(service_health.clone(), "payout_worker".to_string()),
        );

        let resolution_svc = Arc::new(services::resolution::service::ResolutionService::new(
            db.clone(),
            merchant_service.clone(),
            billing_service.clone(),
            fee_config.clone(),
            alerting_service.clone(),
            webhook_service.clone(),
            xero_service.clone(),
            config.environment.to_entity_environment(),
            executors,
            chain_deposit_floors.clone(),
            outbound_store.clone(),
        ));

        (payout_svc, resolution_svc, chain_deposit_floors)
    };

    // SSE Broadcaster for real-time checkout updates (created before PaymentProcessor to inject)
    let sse_broadcaster = Arc::new(services::sse::SseBroadcaster::new());

    let payment_processor = Arc::new(
        PaymentEventProcessor::with_sse(
            db.clone(),
            checkout_service.clone(),
            webhook_service.clone(),
            aml_service.clone(),
            billing_service.clone(),
            fee_config.clone(),
            Some(sse_broadcaster.clone()),
            config.environment.to_entity_environment(),
            alerting_service.clone(),
            chain_deposit_floors,
        )
        .with_health(service_health.clone(), "payment_processor".to_string())
        .with_xero(xero_service.clone()),
    );

    // Build chain client registry (for balance queries, etc.)
    let mut chain_clients: HashMap<Network, Arc<dyn ChainClient>> = HashMap::new();
    chain_clients.insert(Network::Tron, tron_client.clone() as Arc<dyn ChainClient>);

    // Register EVM chain clients for all configured EVM chains
    for (key, chain_toml) in &config.chains {
        if let Some(net) = Network::from_str_lenient(key) {
            if net.chain_family() == ironix_pay::entity::ChainFamily::Evm {
                let entity_env = config.environment.to_entity_environment();
                let chain_id = net.chain_config(&entity_env).chain_id.unwrap_or(1);
                let evm_client = Arc::new(services::evm::EvmClient::new(
                    chain_toml.rpc_urls.clone(),
                    chain_id,
                ));
                chain_clients.insert(net, evm_client as Arc<dyn ChainClient>);
                info!(chain = key, rpc = %chain_toml.rpc_urls[0], endpoints = chain_toml.rpc_urls.len(), "EVM chain client registered");
            }
        }
    }

    // enabled_networks was already built above (for MerchantService)
    info!(networks = ?enabled_networks, "Enabled networks for this instance");

    // ── Chain Health Registry ────────────────────────────────────────────
    // Tracks runtime health of each chain's indexer.
    // Must be created BEFORE AppState so it can be shared with both
    // API handlers (circuit breaker) and supervisor tasks.
    let chain_health = ChainHealthRegistry::new(&enabled_networks);

    let sub_merchant_service = Arc::new(services::sub_merchant::SubMerchantService::new(
        db.clone(),
        address_manager.clone(),
        enabled_networks.clone(),
        Arc::new(config.clone()),
    ));

    // ── Helius Webhook State (created early for AppState injection) ─────
    // The event channel and shared ATA cache are created here so that the
    // webhook handler (via AppState → Router) can receive events from Helius.
    // The ATA cache is populated later during Solana service setup.
    let mut solana_event_channel: Option<(
        tokio::sync::mpsc::Sender<ironix_pay::services::indexer::scanner::IndexerTransferEvent>,
        tokio::sync::mpsc::Receiver<ironix_pay::services::indexer::scanner::IndexerTransferEvent>,
    )>;
    let helius_webhook_state: Option<ironix_pay::api::routes::helius_webhook::HeliusWebhookState>;

    if config.solana.is_some() && config.helius_webhook_secret.is_some() {
        let (tx, rx) = tokio::sync::mpsc::channel(1000);
        let solana_chain_cfg =
            Network::Solana.chain_config(&config.environment.to_entity_environment());
        let watchlist: std::collections::HashMap<String, String> = solana_chain_cfg
            .supported_tokens()
            .into_iter()
            .map(|(mint, symbol)| (mint.to_string(), symbol.to_string()))
            .collect();

        helius_webhook_state = Some(
            ironix_pay::api::routes::helius_webhook::HeliusWebhookState {
                event_tx: tx.clone(),
                ata_cache: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
                watchlist,
                webhook_secret: config.helius_webhook_secret.clone().unwrap(),
            },
        );
        solana_event_channel = Some((tx, rx));
        info!("Helius webhook state initialized (ATA cache will be populated after hydration)");
    } else {
        helius_webhook_state = None;
        solana_event_channel = None;
    }

    // ── R2 Object Storage (optional — for merchant branding) ────────────
    let r2_storage = services::storage::R2StorageService::try_new(&config)
        .await
        .map(std::sync::Arc::new);
    if r2_storage.is_some() {
        info!("R2 storage service enabled for merchant branding");
    }

    let state = AppState {
        config: Arc::new(config.clone()),
        db: db.clone(),
        tron_client: tron_client.clone(),
        chain_clients,
        checkout_service: checkout_service.clone(),
        address_manager,
        merchant_service,
        payment_processor: payment_processor.clone(),
        webhook_service: webhook_service.clone(),
        billing_service: billing_service.clone(),
        agent_service: Arc::new(services::agent::AgentService::new(db.clone())),
        lead_service,
        turnstile_service,
        tron_sweeper_service: sweeper_service.clone(),
        resolution_service: resolution_service.clone(),
        tron_energy_manager: energy_manager.clone(),
        tron_transaction_monitor: transaction_monitor.clone(),
        alerting_service: alerting_service.clone(),
        payout_service: payout_service.clone(),
        sub_merchant_service,
        sse_broadcaster: sse_broadcaster.clone(),
        exchange_rate_service: exchange_rate_service.clone(),
        cancel_token: cancel_token.clone(),
        treasury_address: treasury_addr.clone(),
        gas_sponsor_address: gas_sponsor_addr.clone(),
        evm_treasury_address: evm_treasury_address.clone(),
        evm_gas_sponsor_address: evm_gas_sponsor_address.clone(),
        enabled_networks,
        chain_health: chain_health.clone(),
        service_health: service_health.clone(),
        helius_webhook_state,
        solana_treasury_address: solana_treasury_address.clone(),
        solana_client: solana_payout_client.as_ref().cloned(),
        r2_storage,
        xero_service: xero_service.clone(),
    };

    // ── Multi-chain Backfill ─────────────────────────────────────────────
    // Ensure all existing merchants have chain accounts for every enabled network.
    // This handles the case where a new chain (e.g. BSC) is added after merchants
    // were already registered — they only have TRON chain accounts.
    if state.enabled_networks.len() > 1 {
        let am = state.address_manager.clone();
        let db_clone = db.clone();
        let networks = state.enabled_networks.clone();
        let entity_env = config.environment.to_entity_environment();
        tokio::spawn(async move {
            use ironix_pay::entity::merchants;
            use sea_orm::EntityTrait;
            use tracing::{info, warn};

            let merchants = match merchants::Entity::find().all(&db_clone).await {
                Ok(m) => m,
                Err(e) => {
                    warn!("Chain account backfill: failed to query merchants: {}", e);
                    return;
                }
            };

            let mut total_created = 0u32;
            for merchant in &merchants {
                for network in &networks {
                    match am
                        .initialize_merchant_addresses(
                            &merchant.id,
                            network.clone(),
                            entity_env.clone(),
                        )
                        .await
                    {
                        Ok(result) if !result.already_initialized => {
                            total_created += 1;
                            info!(
                                merchant_id = %merchant.id,
                                network = ?network,
                                addresses = result.addresses_created,
                                "Backfill: created chain account + address pool"
                            );
                        }
                        Ok(_) => {} // Already had this chain account
                        Err(e) => {
                            warn!(
                                merchant_id = %merchant.id,
                                network = ?network,
                                error = %e,
                                "Backfill: failed to initialize chain account"
                            );
                        }
                    }
                }
            }

            if total_created > 0 {
                info!(
                    merchants = merchants.len(),
                    new_chain_accounts = total_created,
                    "Multi-chain backfill completed"
                );
            } else {
                info!("Multi-chain backfill: all merchants up-to-date");
            }
        });
    }

    // IMPORTANT: tower-governor's per_second(N) means "1 token every N seconds" (interval),
    // NOT "N tokens per second" (rate). This is different from governor::Quota::per_second().
    // Formula: 60,000ms / rate_per_minute = milliseconds per token
    // Example: 6000 req/min → 60000/6000 = 10ms per token = 100 req/s
    let replenish_interval_ms = 60_000 / config.rate_limit_per_minute.max(1);
    let burst = std::cmp::max(20, (config.rate_limit_per_minute / 6) as u32); // 10s capacity

    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(RateLimitKeyExtractor)
            .per_millisecond(replenish_interval_ms)
            .burst_size(burst)
            .use_headers() // Return x-ratelimit-limit, x-ratelimit-remaining, retry-after
            .finish()
            .unwrap(),
    );

    // RAM Protection: Manual GC for Rate Limiter
    // (Compatible with tower-governor 0.4/0.5 via direct limiter access)
    // Cleanup every 60 seconds to remove stale keys (e.g. from random key attacks)
    let governor_limiter = governor_conf.limiter().clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
        governor_limiter.retain_recent();
    });

    // 4. Production Hardening: Safe CORS parsing
    // Build list of allowed origins: CORS_ALLOWED_ORIGINS takes priority, fallback to CHECKOUT_BASE_URL
    let allowed_origins: Vec<axum::http::HeaderValue> = if config.cors_allowed_origins.is_empty() {
        // Fallback: just use CHECKOUT_BASE_URL
        vec![config
            .checkout_base_url
            .parse::<axum::http::HeaderValue>()
            .expect("CRITICAL: Invalid CHECKOUT_BASE_URL")]
    } else {
        config
            .cors_allowed_origins
            .iter()
            .map(|o| {
                o.parse::<axum::http::HeaderValue>()
                    .unwrap_or_else(|_| panic!("CRITICAL: Invalid CORS origin: {}", o))
            })
            .collect()
    };

    // Treat both Production and Sandbox (deployed) as "Strict" environments for CORS
    let cors_layer = if is_deployed {
        CorsLayer::new()
            .allow_origin(allowed_origins)
            .allow_methods(Any)
            .allow_headers([
                axum::http::header::AUTHORIZATION,
                axum::http::header::CONTENT_TYPE,
                "X-Environment".parse().unwrap(),
                "Idempotency-Key".parse().unwrap(),
            ])
    } else {
        // Local Development: Allow common frontend ports
        CorsLayer::new()
            .allow_origin([
                "http://localhost:3000"
                    .parse::<axum::http::HeaderValue>()
                    .unwrap(),
                "http://localhost:3001"
                    .parse::<axum::http::HeaderValue>()
                    .unwrap(),
                "http://localhost:5173"
                    .parse::<axum::http::HeaderValue>()
                    .unwrap(),
                "http://127.0.0.1:3000"
                    .parse::<axum::http::HeaderValue>()
                    .unwrap(),
                "http://127.0.0.1:3001"
                    .parse::<axum::http::HeaderValue>()
                    .unwrap(),
                "http://127.0.0.1:5173"
                    .parse::<axum::http::HeaderValue>()
                    .unwrap(),
                "http://localhost:3002" // Demo app
                    .parse::<axum::http::HeaderValue>()
                    .unwrap(),
                "http://127.0.0.1:3002"
                    .parse::<axum::http::HeaderValue>()
                    .unwrap(),
                "http://localhost:3004" // Website (VitePress)
                    .parse::<axum::http::HeaderValue>()
                    .unwrap(),
                "http://127.0.0.1:3004"
                    .parse::<axum::http::HeaderValue>()
                    .unwrap(),
                "http://127.0.0.1:8080"
                    .parse::<axum::http::HeaderValue>()
                    .unwrap(),
                "http://localhost:4173" // Vite preview
                    .parse::<axum::http::HeaderValue>()
                    .unwrap(),
                "http://127.0.0.1:4173"
                    .parse::<axum::http::HeaderValue>()
                    .unwrap(),
            ])
            .allow_methods(Any)
            .allow_headers(Any)
    };

    // ── Prometheus Metrics Layer ──────────────────────────────────────────
    // Must be initialized before the router. Sets up the global metrics recorder.
    // HTTP metrics (request count, duration, pending) are auto-instrumented.
    // Uses install_recorder() instead of build() to avoid binding a separate
    // HTTP listener on port 9000 (conflicts with Docker Desktop).
    let (prometheus_layer, metric_handle) = PrometheusMetricLayerBuilder::new()
        .with_prefix("ironixpay")
        .with_ignore_patterns(&["/metrics", "/health", "/ready"])
        .with_metrics_from_fn(|| {
            PrometheusBuilder::new()
                .set_buckets_for_metric(
                    Matcher::Full("ironixpay_http_requests_duration_seconds".to_string()),
                    SECONDS_DURATION_BUCKETS,
                )
                .unwrap()
                .install_recorder()
                .unwrap()
        })
        .build_pair();

    // /metrics route lives OUTSIDE GovernorLayer to avoid Prometheus scrapes
    // being rate-limited (15s interval would exhaust token bucket quickly).
    let metrics_router = Router::new()
        .route(
            "/metrics",
            axum::routing::get(move || async move { metric_handle.render() }),
        )
        .with_state(state.clone());

    let app = Router::new()
        .merge(api::create_router())
        .layer(GovernorLayer {
            config: governor_conf,
        })
        .merge(metrics_router) // After GovernorLayer → not rate-limited
        .layer(prometheus_layer) // Below all routes → instruments all HTTP requests
        .layer(cors_layer)
        .layer(axum::Extension(state.clone())) // Support middleware using Extension extraction
        .layer(TraceLayer::new_for_http())
        .layer(sentry::integrations::tower::SentryHttpLayer::new().enable_transaction())
        .layer(sentry::integrations::tower::NewSentryLayer::<
            axum::extract::Request,
        >::new_from_top())
        .with_state(state.clone());

    // Build token watchlist from chain config
    let tron_watchlist: Vec<(String, String)> = vec![(usdt_contract.clone(), "USDT".to_string())];
    let tron_indexer_watchlist: std::collections::HashMap<String, String> = tron_watchlist
        .iter()
        .map(|(c, s)| (c.clone(), s.clone()))
        .collect();

    let tron_scanner: Arc<dyn services::indexer::BlockScanner> =
        Arc::new(services::indexer::scanner::TronBlockScanner::new(
            state.tron_client.clone(),
            tron_watchlist,
        ));
    let indexer = TransactionIndexer::new(
        state.db.clone(),
        config.database_url.clone(),
        tron_scanner,
        tron_indexer_watchlist,
        Network::from_str_lenient(&config.network).expect("Invalid network configured"),
        config.environment.to_entity_environment(),
        alerting_service.clone(),
        Some(chain_health.clone()),
    );

    // ── Startup Recovery: Reset stale Processing exceptions ──────────────
    // If previous instance crashed during manual_transfer spawn, exceptions
    // may be stuck in Processing with no tx_hash. Reset → Pending for retry.
    resolution_service.recover_stale_processing().await;

    // ── Spawn Background Services ──────────────────────────────────────
    //
    // Two execution groups:
    //   1. critical_tasks (JoinSet, fail-fast): HTTP Server, Payment Processor, Session Expiry
    //      Any failure → cancel_token.cancel() → full process shutdown
    //   2. Isolated (tokio::spawn + supervisor): Indexers, Sweepers, Webhook, Payout, SSE
    //      Failures → supervisor restarts with exponential backoff, alerts, no process death

    // ── 0. Exchange Rate Sync (isolated) ──
    {
        let svc = exchange_rate_service.clone();
        let token = cancel_token.clone();
        tokio::spawn(async move {
            if let Err(e) = svc.start(token).await {
                tracing::error!(error = %e, "ExchangeRateService stopped unexpectedly");
            }
        });
    }

    // ── 1. TRON Indexer (isolated + supervised) ──
    // Track handles so we can await graceful shutdown later
    let mut isolated_handles: Vec<tokio::task::JoinHandle<anyhow::Result<()>>> = Vec::new();
    {
        let indexer_arc = Arc::new(indexer);
        let token = cancel_token.clone();
        let health = chain_health.clone();
        let alerting = alerting_service.clone();
        let tron_network =
            Network::from_str_lenient(&config.network).expect("Invalid network configured");
        isolated_handles.push(tokio::spawn(supervisor_loop(
            "TRON Indexer",
            Some((health, tron_network)),
            token.clone(),
            alerting,
            move || {
                let indexer = indexer_arc.clone();
                let token = token.clone();
                async move { indexer.start(token).await }
            },
        )));
    }

    // ── 2. Session Expiry Worker (critical — DB dependent) ──
    {
        let worker = services::checkout::SessionExpiryWorker::new(
            db.clone(),
            checkout_service.clone(),
            webhook_service.clone(),
            config.environment.to_entity_environment(),
        );
        let token = cancel_token.clone();
        background_tasks.spawn(async move { worker.run(token).await });
    }

    // ── 3. TRON Sweeper (isolated + supervised) ──
    {
        let sweeper_clone = sweeper_service.clone();
        let token = cancel_token.clone();
        let alerting = alerting_service.clone();
        isolated_handles.push(tokio::spawn(supervisor_loop(
            "TRON Sweeper",
            None, // Sweeper health doesn't affect checkout acceptance
            token.clone(),
            alerting,
            move || {
                let sweeper = sweeper_clone.clone();
                let token = token.clone();
                async move {
                    sweeper
                        .start(token)
                        .await
                        .map_err(|e| anyhow::anyhow!("TRON Sweeper failure: {}", e))
                }
            },
        )));
    }
    // ── 3b. EVM Chain Indexer + Sweeper (conditional, isolated + supervised) ──
    // Generic helper that wires up indexer + sweeper for any EVM chain.
    #[allow(clippy::too_many_arguments)]
    fn spawn_evm_chain(
        chain_key: &str,
        network: Network,
        chain_cfg: &ironix_pay::config::ChainTomlConfig,
        poll_interval: std::time::Duration,
        gas_sponsor_address: String,
        treasury_address: Option<String>,
        db: sea_orm::DatabaseConnection,
        database_url: secrecy::Secret<String>,
        entity_env: ironix_pay::entity::network::Environment,
        transaction_signer: Arc<
            dyn ironix_pay::services::address::key_provider::TransactionSigner + Send + Sync,
        >,
        chain_health: services::chain_health::ChainHealthRegistry,
        alerting_service: Arc<services::alerting::AlertingService>,
        cancel_token: CancellationToken,
        sweeper_config: services::sweeper::SweeperConfig,
        sweep_threshold_ratio: f64,
        // If provided, reuse this gas funder (shares Mutex with payout executor).
        // If None, create a new one for this chain.
        existing_gas_funder: Option<Arc<services::evm::gas_funder::EvmGasFunder>>,
        service_health: services::ServiceHealthRegistry,
        outbound_store: Arc<services::outbound::OutboundTransactionStore>,
    ) -> Vec<tokio::task::JoinHandle<anyhow::Result<()>>> {
        let mut handles = Vec::new();
        let chain_config = network.chain_config(&entity_env);
        let evm_client = Arc::new(services::evm::EvmClient::new(
            chain_cfg.rpc_urls.clone(),
            chain_config.chain_id.unwrap_or(1),
        ));
        let usdt_contract = chain_cfg
            .usdt_contract
            .clone()
            .unwrap_or(chain_config.usdt_contract);

        // Build token watchlist for this EVM chain
        let mut evm_watchlist: Vec<(String, String)> =
            vec![(usdt_contract.clone(), "USDT".to_string())];
        // Add USDC if this chain supports it
        if let Some(ref usdc_addr) = chain_config.usdc_contract {
            evm_watchlist.push((usdc_addr.clone(), "USDC".to_string()));
        }
        let evm_indexer_watchlist: std::collections::HashMap<String, String> = evm_watchlist
            .iter()
            .map(|(c, s)| (c.clone(), s.clone()))
            .collect();

        // Indexer
        let scanner: Arc<dyn services::indexer::BlockScanner> =
            Arc::new(services::indexer::scanner::EvmBlockScanner::new(
                evm_client.clone(),
                evm_watchlist,
                chain_config.usdt_decimals,
                chain_config.confirmation_blocks as i32,
                poll_interval,
            ));
        let indexer = TransactionIndexer::new(
            db.clone(),
            database_url,
            scanner,
            evm_indexer_watchlist,
            network,
            entity_env.clone(),
            alerting_service.clone(),
            Some(chain_health.clone()),
        );

        {
            let indexer_arc = Arc::new(indexer);
            let token = cancel_token.clone();
            let health = chain_health.clone();
            let alerting = alerting_service.clone();
            let task_name = network.indexer_task_name();
            handles.push(tokio::spawn(supervisor_loop(
                task_name,
                Some((health, network)),
                token.clone(),
                alerting,
                move || {
                    let indexer = indexer_arc.clone();
                    let token = token.clone();
                    async move { indexer.start(token).await }
                },
            )));
        }

        // Sweeper — reuse shared gas funder if available, otherwise create new
        let gas_funder = existing_gas_funder.unwrap_or_else(|| {
            let gs = chain_cfg.gas_sponsor.as_ref();
            Arc::new(services::evm::gas_funder::EvmGasFunder::new(
                evm_client.clone(),
                transaction_signer.clone(),
                gas_sponsor_address,
                gs.map(|g| g.account_index).unwrap_or(0),
                gs.map(|g| g.path_index).unwrap_or(1),
            ))
        });
        let executor: Arc<dyn services::sweeper::executor::SweepExecutor> =
            Arc::new(services::sweeper::executor::EvmSweepExecutor::new(
                evm_client,
                transaction_signer,
                gas_funder,
                chain_config.usdt_decimals,
            ));
        let price_oracle: Arc<dyn services::price::PriceOracle> = Arc::new(
            services::price::BinancePriceOracle::try_new(Some(60))
                .expect("Failed to initialize Price Oracle"),
        );
        let chain_sweeper_config = services::sweeper::SweeperConfig {
            platform_treasury_address: treasury_address,
            ..sweeper_config
        };
        let sweeper = Arc::new(
            services::sweeper::SweeperService::new(
                db,
                executor,
                price_oracle,
                chain_sweeper_config,
                sweep_threshold_ratio,
                entity_env,
                network,
                alerting_service.clone(),
                outbound_store,
            )
            .with_health(
                service_health,
                format!("sweeper_{}", chain_key.to_lowercase()),
            ),
        );

        {
            let token = cancel_token.clone();
            let alerting = alerting_service.clone();
            let task_name = network.sweeper_task_name();
            handles.push(tokio::spawn(supervisor_loop(
                task_name,
                None, // Sweeper health doesn't affect checkout acceptance
                token.clone(),
                alerting,
                move || {
                    let sweeper = sweeper.clone();
                    let token = token.clone();
                    async move {
                        sweeper
                            .start(token)
                            .await
                            .map_err(|e| anyhow::anyhow!("{} failure: {}", task_name, e))
                    }
                },
            )));
        }

        handles
    }

    // ── 3c. Spawn all configured EVM chains (data-driven) ──
    // Iterates over chains.toml entries, skipping TRON (handled above).
    // Poll interval is sourced from ChainConfig::poll_interval_secs.
    for (key, cfg) in &config.chains {
        let network = match Network::from_str_lenient(key) {
            Some(n) if n.chain_family() == ironix_pay::entity::ChainFamily::Evm => n,
            _ => continue, // Skip TRON (handled above) and unknown keys
        };
        let entity_env = config.environment.to_entity_environment();
        let chain_config = network.chain_config(&entity_env);
        let poll_interval = std::time::Duration::from_secs(chain_config.poll_interval_secs);
        // Dynamically register EVM sweeper name in the service health registry
        let evm_sweeper_name = format!("sweeper_{}", key.to_lowercase());
        service_health.register_service(&evm_sweeper_name);
        let handles = spawn_evm_chain(
            key,
            network,
            cfg,
            poll_interval,
            evm_gas_sponsor_address
                .clone()
                .expect("EVM gas sponsor address should be derived"),
            evm_treasury_address.clone(),
            db.clone(),
            config.database_url.clone(),
            entity_env,
            transaction_signer.clone(),
            chain_health.clone(),
            alerting_service.clone(),
            cancel_token.clone(),
            sweeper_config.clone(),
            config.sweep_threshold_ratio,
            evm_payout_gas_funders.get(&network).cloned(),
            service_health.clone(),
            outbound_store.clone(),
        );
        isolated_handles.extend(handles);
        info!(network = %key, "EVM indexer + sweeper spawned (fault-isolated)");
    }

    // ── 3d. Solana Indexer + Sweeper (conditional, isolated + supervised) ──
    // Solana uses a fundamentally different scanning model (signature-cursor vs block-range),
    // so it gets its own SolanaIndexer. Events are bridged to a dedicated TransactionIndexer
    // instance via an MPSC channel for payment detection.
    if let Some(ref solana_cfg) = config.solana {
        info!("Solana integration enabled — spawning indexer + sweeper");
        let entity_env = config.environment.to_entity_environment();

        // 1. SolanaClient (RPC with failover) — reuse from payout registration if available
        let solana_client = solana_payout_client.take().unwrap_or_else(|| {
            Arc::new(SolanaClient::new(
                solana_cfg.rpc_urls.clone(),
                Network::Solana,
            ))
        });

        // 2. Build watchlist from hardcoded ChainConfig (mint → symbol)
        let solana_chain_config = Network::Solana.chain_config(&entity_env);
        let solana_watchlist: HashMap<String, String> = solana_chain_config
            .supported_tokens()
            .into_iter()
            .map(|(mint, symbol)| (mint.to_string(), symbol.to_string()))
            .collect();

        // 3. Solana TransactionIndexer (uses SolanaBridgeScanner — real scanning done by SolanaIndexer)
        //    This instance provides: address DashMap, process_event pipeline, AddressSyncManager
        //    The bridge scanner delegates get_current_block + verify_transaction to the real
        //    Solana RPC so that check_confirmations works correctly.
        let bridge_scanner: Arc<dyn services::indexer::BlockScanner> =
            Arc::new(SolanaBridgeScanner::new(solana_client.clone()));
        let solana_tx_indexer = Arc::new(TransactionIndexer::new(
            db.clone(),
            config.database_url.clone(),
            bridge_scanner,
            solana_watchlist,
            Network::Solana,
            entity_env.clone(),
            alerting_service.clone(),
            Some(chain_health.clone()),
        ));

        // 4. Start TransactionIndexer (hydrates address cache, starts AddressSyncManager)
        //    Notify signal ensures SolanaIndexer waits for hydration before scanning
        let hydration_ready = Arc::new(tokio::sync::Notify::new());
        {
            let bridge = solana_tx_indexer.clone();
            let token = cancel_token.clone();
            let alerting = alerting_service.clone();
            let ready = hydration_ready.clone();
            isolated_handles.push(tokio::spawn(supervisor_loop(
                "Solana Bridge",
                Some((chain_health.clone(), Network::Solana)),
                token.clone(),
                alerting,
                move || {
                    let b = bridge.clone();
                    let t = token.clone();
                    let r = ready.clone();
                    async move {
                        // Notify after start begins (hydration happens at start of start())
                        r.notify_one();
                        b.start(t).await
                    }
                },
            )));
        }

        // 5. SolanaIndexer (signature-cursor scanner) + event channel
        // If Helius webhook is configured, reuse the pre-created channel;
        // otherwise create a fresh one for polling-only mode.
        let (event_tx, mut event_rx) = if let Some((tx, rx)) = solana_event_channel.take() {
            info!("Solana: reusing Helius webhook event channel");
            (tx, rx)
        } else {
            tokio::sync::mpsc::channel::<ironix_pay::services::indexer::scanner::IndexerTransferEvent>(
                1000,
            )
        };

        // Capture shared ATA cache reference for populating after hydration
        let helius_ata_cache = state
            .helius_webhook_state
            .as_ref()
            .map(|ws| ws.ata_cache.clone());

        {
            let shared_addrs = solana_tx_indexer.shared_address_cache();
            let indexer_watchlist: Vec<(String, String)> = solana_chain_config
                .supported_tokens()
                .into_iter()
                .map(|(mint, symbol)| (mint.to_string(), symbol.to_string()))
                .collect();
            // Pass helius_ata_cache to SolanaIndexer so sync_ata_cache() automatically
            // keeps the webhook handler's ATA lookup in sync with new addresses.
            let solana_scanner = Arc::new(tokio::sync::Mutex::new(SolanaIndexer::new(
                solana_client.clone(),
                indexer_watchlist.clone(),
                None,
                shared_addrs.clone(),
                helius_ata_cache,
            )));

            let token = cancel_token.clone();
            let alerting = alerting_service.clone();
            let ready = hydration_ready.clone();
            let event_tx_for_manager = event_tx.clone(); // Clone before supervisor_loop moves event_tx
            let scanner_for_manager = solana_scanner.clone(); // Clone before supervisor_loop moves scanner

            // Determine if Helius webhook is configured
            let helius_configured = config.helius_api_key.is_some()
                && config.helius_webhook_url.is_some()
                && config.helius_webhook_secret.is_some();

            if helius_configured {
                // Webhook mode: skip SolanaIndexer polling loop (saves ~80K credits/day).
                // HeliusWebhookManager handles real-time events via webhook + reconciliation fallback.
                //
                // IMPORTANT: We still need to hydrate the ATA cache once so the webhook handler
                // and reconciliation can match incoming Helius events to monitored addresses.
                let scanner_for_hydration = solana_scanner.clone();
                let ready_for_hydration = ready.clone();
                tokio::spawn(async move {
                    // Wait for TransactionIndexer to populate shared_addresses via LISTEN/NOTIFY
                    ready_for_hydration.notified().await;
                    let mut guard = scanner_for_hydration.lock().await;
                    guard.sync_ata_cache();
                    info!(
                        ata_count = guard.ata_cache().len(),
                        "Solana ATA cache hydrated for Helius webhook (polling disabled)"
                    );
                });
                info!(
                    "Solana indexer polling DISABLED (Helius webhook active — saves RPC credits)"
                );
            } else {
                // No webhook: use traditional polling as fallback
                isolated_handles.push(tokio::spawn(supervisor_loop(
                    "Solana Indexer",
                    None, // Health reported by the Bridge task above
                    token.clone(),
                    alerting,
                    move || {
                        let scanner = solana_scanner.clone();
                        let tx = event_tx.clone();
                        let t = token.clone();
                        let r = ready.clone();
                        async move {
                            // Wait for TransactionIndexer hydration before scanning
                            r.notified().await;
                            let mut guard = scanner.lock().await;
                            guard.run(tx, t).await;
                            Ok(())
                        }
                    },
                )));
            }

            // 5b. HeliusWebhookManager (conditional: requires HELIUS_API_KEY)
            if let (Some(api_key), Some(webhook_url), Some(webhook_secret)) = (
                config.helius_api_key.clone(),
                config.helius_webhook_url.clone(),
                config.helius_webhook_secret.clone(),
            ) {
                use ironix_pay::services::solana::helius_manager::HeliusWebhookManager;
                let solana_chain_cfg =
                    Network::Solana.chain_config(&config.environment.to_entity_environment());
                let token_mints: Vec<String> = solana_chain_cfg
                    .supported_tokens()
                    .into_iter()
                    .map(|(mint, _symbol)| mint.to_string())
                    .collect();
                let manager = HeliusWebhookManager::new(
                    api_key,
                    webhook_url,
                    webhook_secret,
                    db.clone(),
                    Network::Solana,
                    token_mints,
                    solana_client.clone(),
                    scanner_for_manager,
                    event_tx_for_manager,
                    &solana_cfg.rpc_urls[0],
                );
                let token = cancel_token.clone();
                isolated_handles.push(tokio::spawn(async move {
                    let mut mgr = manager;
                    mgr.run(token).await;
                    Ok(())
                }));
                info!("HeliusWebhookManager spawned");
            }
        }

        // 6. Event receiver: bridges SolanaIndexer events → TransactionIndexer.process_event
        {
            let receiver_indexer = solana_tx_indexer.clone();
            let token = cancel_token.clone();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = token.cancelled() => {
                            info!("Solana event receiver: shutdown");
                            break;
                        }
                        Some(event) = event_rx.recv() => {
                            let slot = event.block_number;
                            if let Err(e) = receiver_indexer.ingest_external_event(&event, slot).await {
                                tracing::error!(
                                    tx = %event.tx_hash,
                                    error = %e,
                                    "Solana event processing failed"
                                );
                            }
                        }
                        else => break, // Channel closed
                    }
                }
            });
        }

        // 7. Solana Sweeper
        {
            let token_decimals: HashMap<String, u8> = solana_chain_config
                .supported_tokens()
                .into_iter()
                .filter_map(|(mint, symbol)| {
                    solana_chain_config
                        .token_decimals(symbol)
                        .map(|d| (mint.to_string(), d))
                })
                .collect();
            let sol_treasury = solana_treasury_address
                .clone()
                .expect("Solana treasury address must be derived when Solana is configured");
            let executor: Arc<dyn services::sweeper::executor::SweepExecutor> =
                Arc::new(SolanaSweepExecutor::new(
                    solana_client,
                    transaction_signer.clone(),
                    sol_treasury.clone(),
                    token_decimals,
                ));
            let price_oracle: Arc<dyn services::price::PriceOracle> = Arc::new(
                services::price::BinancePriceOracle::try_new(Some(60))
                    .expect("Failed to initialize Solana Price Oracle"),
            );
            let solana_sweeper_config = services::sweeper::SweeperConfig {
                platform_treasury_address: Some(sol_treasury),
                ..sweeper_config.clone()
            };
            service_health.register_service("sweeper_solana");
            let sweeper = Arc::new(
                services::sweeper::SweeperService::new(
                    db.clone(),
                    executor,
                    price_oracle,
                    solana_sweeper_config,
                    config.sweep_threshold_ratio,
                    entity_env,
                    Network::Solana,
                    alerting_service.clone(),
                    outbound_store.clone(),
                )
                .with_health(service_health.clone(), "sweeper_solana".to_string()),
            );
            let token = cancel_token.clone();
            let alerting = alerting_service.clone();
            isolated_handles.push(tokio::spawn(supervisor_loop(
                "Solana Sweeper",
                None,
                token.clone(),
                alerting,
                move || {
                    let s = sweeper.clone();
                    let t = token.clone();
                    async move {
                        s.start(t)
                            .await
                            .map_err(|e| anyhow::anyhow!("Solana Sweeper failure: {}", e))
                    }
                },
            )));
        }

        info!("Solana indexer + sweeper spawned (fault-isolated)");
    }

    // ── 4. Payment Event Processor (critical — DB dependent) ──
    {
        let processor_clone = payment_processor.clone();
        let token = cancel_token.clone();
        background_tasks.spawn(async move {
            info!("Starting payment event processor...");
            if let Err(e) = processor_clone.start(token).await {
                error!("CRITICAL: Payment event processor failed: {}", e);
                return Err(anyhow::anyhow!("Processor failure: {}", e));
            }
            Ok(())
        });
    }

    // ── 5. Webhook Recovery Loop (isolated + supervised) ──
    {
        let webhook_clone = webhook_service.clone();
        let token = cancel_token.clone();
        let alerting = alerting_service.clone();
        isolated_handles.push(tokio::spawn(supervisor_loop(
            "Webhook Recovery",
            None, // Not chain-specific
            token.clone(),
            alerting,
            move || {
                let webhook = webhook_clone.clone();
                let token = token.clone();
                async move {
                    webhook
                        .start_recovery_loop(token)
                        .await
                        .map_err(|e| anyhow::anyhow!("Webhook recovery failure: {}", e))
                }
            },
        )));
    }

    // ── 6. SSE Broadcaster Cleanup Loop (isolated + supervised) ──
    {
        let broadcaster_clone = sse_broadcaster.clone();
        let token = cancel_token.clone();
        let alerting = alerting_service.clone();
        isolated_handles.push(tokio::spawn(supervisor_loop(
            "SSE Cleanup",
            None,
            token.clone(),
            alerting,
            move || {
                let broadcaster = broadcaster_clone.clone();
                let token = token.clone();
                async move {
                    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
                    loop {
                        tokio::select! {
                            _ = token.cancelled() => {
                                info!("SSE broadcaster cleanup loop shutdown");
                                break;
                            }
                            _ = interval.tick() => {
                                broadcaster.cleanup_idle_channels();
                            }
                        }
                    }
                    Ok(())
                }
            },
        )));
    }

    // ── 7. Payout Worker (isolated + supervised) ──
    {
        let payout_clone = payout_service.clone();
        let token = cancel_token.clone();
        let alerting = alerting_service.clone();
        isolated_handles.push(tokio::spawn(supervisor_loop(
            "Payout Worker",
            None, // Multi-chain, not tied to single network
            token.clone(),
            alerting,
            move || {
                let payout = payout_clone.clone();
                let token = token.clone();
                async move {
                    payout
                        .start(token)
                        .await
                        .map_err(|e| anyhow::anyhow!("Payout worker failure: {}", e))
                }
            },
        )));
    }

    // ── 8. Auto-Withdraw Worker (isolated + supervised) ──
    {
        let db_clone = db.clone();
        let payout_clone = payout_service.clone();
        let entity_env = config.environment.to_entity_environment();
        let token = cancel_token.clone();
        let alerting = alerting_service.clone();
        isolated_handles.push(tokio::spawn(supervisor_loop(
            "Auto-Withdraw",
            None,
            token.clone(),
            alerting,
            move || {
                let db = db_clone.clone();
                let payout = payout_clone.clone();
                let env = entity_env.clone();
                let token = token.clone();
                async move { services::payout::auto_withdraw::run(db, payout, env, token).await }
            },
        )));
    }

    // ── 9. Tier Calculator (isolated, production only) ──
    if is_deployed {
        let tier_svc = Arc::new(services::tier_calculator::TierCalculatorService::new(
            db.clone(),
            services::tier_calculator::TierConfig::default(),
        ));
        let token = cancel_token.clone();
        let alerting = alerting_service.clone();
        isolated_handles.push(tokio::spawn(supervisor_loop(
            "Tier Calculator",
            None,
            token.clone(),
            alerting,
            move || {
                let svc = tier_svc.clone();
                let token = token.clone();
                async move {
                    svc.start(token)
                        .await
                        .map_err(|e| anyhow::anyhow!("Tier Calculator failure: {}", e))
                }
            },
        )));
    }
    // ── 10. Xero Sync Worker (isolated + supervised, optional) ──
    if let Some(ref xero_svc) = xero_service {
        let xero_clone = xero_svc.clone();
        let token = cancel_token.clone();
        let alerting = alerting_service.clone();
        isolated_handles.push(tokio::spawn(supervisor_loop(
            "Xero Sync Worker",
            None,
            token.clone(),
            alerting,
            move || {
                let worker =
                    services::xero::worker::XeroSyncWorker::new(xero_clone.clone(), token.clone());
                async move { worker.run().await }
            },
        )));
    }

    // --- Start HTTP Server ---

    let addr = format!("{}:{}", config.host, config.port);
    let listener = TcpListener::bind(&addr).await?;
    info!("Listening on {}", addr);

    let server_shutdown_token = cancel_token.clone();
    let shutdown_signal = async move {
        server_shutdown_token.cancelled().await;
        info!("Http server shutting down...");
    };

    background_tasks.spawn(async move {
        info!("Starting Axum server...");
        // SSE streams now respond to cancel_token, so graceful shutdown works correctly
        match axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown_signal)
        .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("CRITICAL: Axum server failed: {}", e);
                Err(anyhow::anyhow!("Server failure: {}", e))
            }
        }
    });

    // --- Main Monitor Loop (critical tasks only) ---
    // Isolated tasks (indexers, sweepers, etc.) are managed by their own
    // supervisor loops via tokio::spawn and do NOT affect this JoinSet.
    let mut is_error_shutdown = false;

    while let Some(task_result) = background_tasks.join_next().await {
        match task_result {
            Ok(service_result) => match service_result {
                Ok(_) => {
                    if !cancel_token.is_cancelled() {
                        info!("A background task stopped unexpectedly. Shutting down...");
                        cancel_token.cancel();
                    }
                }
                Err(e) => {
                    error!(
                        "CRITICAL: Background service failed: {}. Shutting down all services.",
                        e
                    );
                    is_error_shutdown = true;
                    cancel_token.cancel();
                }
            },
            Err(join_err) => {
                if join_err.is_panic() {
                    error!("CRITICAL: A background task PANICKED! Force exiting.");
                    process::exit(1);
                } else if join_err.is_cancelled() {
                    // Normal
                } else {
                    error!("CRITICAL: Background task execution failed: {}", join_err);
                    is_error_shutdown = true;
                    cancel_token.cancel();
                }
            }
        }

        if background_tasks.is_empty() {
            break;
        }
    }

    info!("All tasks terminated.");

    if is_error_shutdown {
        error!("Process exiting with ERROR due to background task failure.");
    } else {
        info!("Process exiting successfully.");
    }

    // ── Wait for isolated tasks to complete graceful shutdown ──────────
    // Give supervised tasks time to finish cleanup (e.g., indexer saving
    // last_processed_block, sweeper completing mid-cycle DB commits).
    // 10s timeout prevents indefinite hangs.
    info!(
        "Waiting for {} isolated tasks to shut down...",
        isolated_handles.len()
    );
    let total_isolated = isolated_handles.len();
    let shutdown_deadline = tokio::time::sleep(std::time::Duration::from_secs(10));
    tokio::pin!(shutdown_deadline);

    for (i, handle) in isolated_handles.into_iter().enumerate() {
        tokio::select! {
            result = handle => {
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => info!("Isolated task {} exited with: {}", i, e),
                    Err(e) => info!("Isolated task {} join error: {}", i, e),
                }
            }
            _ = &mut shutdown_deadline => {
                info!("Shutdown deadline reached, {} tasks still running — exiting", total_isolated.saturating_sub(i + 1));
                break;
            }
        }
    }
    info!("Isolated tasks shutdown complete.");

    if is_error_shutdown {
        anyhow::bail!("System shutdown due to critical component failure");
    } else {
        Ok(())
    }
}

/// Reset exceptions stuck in Processing state from previous server run.
/// This prevents "zombie" exceptions where a tokio::spawn task was killed mid-execution.
/// IMPORTANT: Only reset exceptions that have NOT been broadcasted (resolution_ref_id IS NULL).
/// If an exception has a tx_hash, it means the broadcast succeeded and we should wait for confirmation.
async fn reset_stuck_processing_exceptions(db: &sea_orm::DatabaseConnection) -> anyhow::Result<()> {
    use ironix_pay::entity::payment_exceptions::{self, ExceptionStatus};
    use sea_orm::{sea_query::Expr, ColumnTrait, EntityTrait, QueryFilter};

    let result = payment_exceptions::Entity::update_many()
        .col_expr(
            payment_exceptions::Column::Status,
            Expr::value(ExceptionStatus::Pending),
        )
        .col_expr(
            payment_exceptions::Column::Notes,
            Expr::value(Some(
                "Auto-reset from Processing: server restarted during execution",
            )),
        )
        .col_expr(
            payment_exceptions::Column::UpdatedAt,
            Expr::value(chrono::Utc::now()),
        )
        .filter(payment_exceptions::Column::Status.eq(ExceptionStatus::Processing))
        // CRITICAL: Only reset exceptions that have NOT been broadcasted yet
        // If resolution_ref_id has a tx_hash, the broadcast succeeded and sweeper will confirm it
        .filter(payment_exceptions::Column::ResolutionRefId.is_null())
        .exec(db)
        .await?;

    if result.rows_affected > 0 {
        tracing::warn!(
            count = result.rows_affected,
            "Reset stuck Processing exceptions (no tx_hash) back to Pending"
        );
    }
    Ok(())
}
