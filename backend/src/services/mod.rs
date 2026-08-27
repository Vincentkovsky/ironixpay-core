//! Services module
//!
//! Contains all business logic services.
//! Aligned with docs/system_design.md

use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::sync::Arc;

use crate::config::Config;
use crate::entity::Network;
use crate::services::{
    address::AddressManager, billing::BillingService, checkout::CheckoutService,
    merchant::MerchantService, payment_processor::PaymentEventProcessor, tron::TronClient,
    webhook::WebhookService,
};
use chain::traits::ChainClient;

pub mod address;
pub mod agent;
pub mod alerting;
pub mod aml;
pub mod billing;
pub mod chain;
pub mod chain_health;
pub mod checkout;
pub mod email;
pub mod energy;
pub mod evm;
pub mod exchange_rate;
pub mod indexer;
pub mod lead;
pub mod merchant;
pub mod metrics;
pub mod outbound;
pub mod payment_processor;
pub mod payout;
pub mod price;
pub mod resolution;
pub mod service_health;
pub mod solana;
pub mod sse;
pub mod storage;
pub mod sub_merchant;
pub mod supervisor;
pub mod sweeper;
pub mod tier_calculator;
pub mod transaction_monitor;
pub mod tron;
pub mod turnstile;
pub mod webhook;
pub mod xero;

// Re-export commonly used types
pub use address::AddressAllocationError;
pub use alerting::AlertingService;
pub use chain_health::ChainHealthRegistry;
pub use service_health::ServiceHealthRegistry;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: DatabaseConnection,
    pub tron_client: Arc<TronClient>,
    /// Chain-agnostic client registry. Keyed by Network enum.
    /// Phase 2: populated with TronClient; future phases add EvmClient etc.
    pub chain_clients: HashMap<Network, Arc<dyn ChainClient>>,
    pub checkout_service: Arc<CheckoutService>,
    pub address_manager: Arc<AddressManager>,
    pub merchant_service: Arc<MerchantService>,
    pub payment_processor: Arc<PaymentEventProcessor>,
    pub webhook_service: Arc<WebhookService>,
    pub billing_service: Arc<BillingService>,
    /// Agent referral service
    pub agent_service: Arc<agent::AgentService>,
    /// Public website enterprise lead intake and notification service.
    pub lead_service: Arc<lead::LeadService>,
    /// Cloudflare Turnstile verifier for public registration abuse protection.
    /// None only when explicitly disabled for local development.
    pub turnstile_service: Option<Arc<turnstile::TurnstileService>>,
    /// TRON sweeper instance. EVM sweepers run as independent supervised tasks
    /// and are not stored in AppState (they share gas funders via spawn_evm_chain).
    pub tron_sweeper_service: Arc<sweeper::SweeperService>,
    pub resolution_service: Arc<resolution::service::ResolutionService>,
    /// TRON energy manager. EVM chains use EvmGasFunder managed per-chain in spawned tasks.
    pub tron_energy_manager: Arc<energy::EnergyManager>,
    /// TRON transaction monitor (lifecycle tracking for TRON-specific broadcast patterns).
    pub tron_transaction_monitor: Arc<transaction_monitor::service::TransactionMonitor>,
    pub alerting_service: Arc<AlertingService>,
    pub payout_service: Arc<payout::PayoutService>,
    pub sub_merchant_service: Arc<sub_merchant::SubMerchantService>,
    pub sse_broadcaster: Arc<sse::SseBroadcaster>,
    /// Exchange rate service for fiat pricing (CoinGecko sync + cache)
    pub exchange_rate_service: Arc<exchange_rate::ExchangeRateService>,
    /// Cancellation token for graceful shutdown (used by SSE streams)
    pub cancel_token: tokio_util::sync::CancellationToken,
    /// HD-derived platform treasury address (account_index=0, path_index=0)
    pub treasury_address: String,
    /// HD-derived TRON gas sponsor address (account_index=0, path_index=1)
    pub gas_sponsor_address: String,
    /// HD-derived EVM treasury address (None if no EVM chains enabled)
    pub evm_treasury_address: Option<String>,
    /// HD-derived EVM gas sponsor address (None if no EVM chains enabled)
    pub evm_gas_sponsor_address: Option<String>,
    /// Networks enabled for this instance (determined by chains.toml)
    pub enabled_networks: Vec<Network>,
    /// Runtime chain health registry (updated by indexer tasks + supervisor)
    pub chain_health: ChainHealthRegistry,
    /// Background service heartbeat registry (sweeper, processor, webhook, payout)
    pub service_health: ServiceHealthRegistry,
    /// Helius webhook shared state (Solana event-driven indexing).
    /// None when Solana is not configured or Helius webhook is disabled.
    pub helius_webhook_state: Option<crate::api::routes::helius_webhook::HeliusWebhookState>,
    /// HD-derived Solana treasury address (None if Solana not enabled)
    pub solana_treasury_address: Option<String>,
    /// Solana RPC client (None if Solana not enabled)
    pub solana_client: Option<Arc<solana::SolanaClient>>,
    /// R2 object storage for merchant branding assets (None if R2 not configured)
    pub r2_storage: storage::OptionalR2,
    /// Xero accounting integration (None if XERO_CLIENT_ID not configured)
    pub xero_service: Option<Arc<xero::XeroService>>,
}
