//! Sweeper Service Implementation
//!
//! Handles USDT collection from customer addresses to merchant wallets.
//! Chain-agnostic: all chain-specific operations delegated to SweepExecutor.
//! Aligned with docs/system_design.md

use crate::entity::checkout_sessions::SettlementStatus;
use crate::entity::outbound_transactions::OutboundState;
use crate::entity::Environment;
use crate::entity::Network;
use crate::entity::{
    addresses, checkout_sessions, merchants, outbound_transactions, payment_exceptions, Addresses,
    CheckoutSessions, OutboundTransactions,
};
use crate::services::alerting::{AlertLevel, AlertingService};
use crate::services::price::PriceOracle;
use std::collections::{HashMap, HashSet};

use super::executor::{SweepExecutor, SweepTxStatus};
use anyhow::{anyhow, Result};
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection,
    EntityTrait, FromQueryResult, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Configuration options for sweeper behavior
#[derive(Debug, Clone)]
pub struct SweeperConfig {
    /// Blocks to wait for confirmation (default: 19)
    pub confirmation_blocks: u64,
    /// Energy estimate for TRC20 transfer (default: 65_000)
    pub energy_estimate: u64,
    /// Maximum sweep retry attempts before giving up (default: 3)
    pub max_sweep_attempts: i32,
    /// Sweep stuck timeout in seconds - tx not on chain after this time is marked stuck (default: 300 = 5 min)
    pub stuck_timeout_seconds: u64,
    /// Cooling period in seconds before address can be recycled (default: 3600 = 60 min)
    pub cooling_period_seconds: u64,
    /// Force sweep stagnant addresses older than this in hours (default: 24)
    pub stagnant_address_hours: i64,
    /// TRX amount to transfer for bandwidth (default: 350_000 sun = 0.35 TRX)
    pub bandwidth_trx_amount: u64,
    /// Maximum concurrent sweep tasks in broadcast_cycle (default: 20)
    pub max_concurrent_sweeps: usize,
    /// Platform treasury address for receiving swept funds (Ledger mode)
    pub platform_treasury_address: Option<String>,
}

impl Default for SweeperConfig {
    fn default() -> Self {
        Self {
            confirmation_blocks: 19,
            energy_estimate: 65_000,
            max_sweep_attempts: 3,
            stuck_timeout_seconds: 300,
            cooling_period_seconds: 3600, // 1 hour
            stagnant_address_hours: 24,
            bandwidth_trx_amount: 350_000,   // 0.35 TRX in sun
            max_concurrent_sweeps: 20,       // Max parallel sweep tasks
            platform_treasury_address: None, // Must be configured for sweeps
        }
    }
}

pub struct SweeperService {
    db: DatabaseConnection,
    executor: Arc<dyn SweepExecutor>,
    price_oracle: Arc<dyn PriceOracle>,
    sweep_config: SweeperConfig,
    environment: Environment,
    /// Network this sweeper is responsible for.
    /// Each sweeper instance only processes addresses on its own network.
    network: Network,
    #[allow(dead_code)] // Reserved for future use: dynamic threshold
    sweep_threshold_ratio: f64,
    alerting_service: Arc<AlertingService>,
    outbound_store: Arc<crate::services::outbound::OutboundTransactionStore>,
    /// Optional heartbeat reporter for /ready and admin health monitoring.
    /// Set via `with_health()`. When None, no heartbeat is sent (test-safe).
    service_health: Option<(
        crate::services::service_health::ServiceHealthRegistry,
        String,
    )>,
}

impl SweeperService {
    pub fn new(
        db: DatabaseConnection,
        executor: Arc<dyn SweepExecutor>,
        price_oracle: Arc<dyn PriceOracle>,
        sweep_config: SweeperConfig,
        sweep_threshold_ratio: f64,
        environment: Environment,
        network: Network,
        alerting_service: Arc<AlertingService>,
        outbound_store: Arc<crate::services::outbound::OutboundTransactionStore>,
    ) -> Self {
        Self {
            db,
            executor,
            price_oracle,
            sweep_config,
            sweep_threshold_ratio,
            environment,
            network,
            alerting_service,
            outbound_store,
            service_health: None,
        }
    }

    /// Attach heartbeat reporting for service health monitoring.
    /// `service_name` is the key registered in `ServiceHealthRegistry`.
    pub fn with_health(
        mut self,
        registry: crate::services::service_health::ServiceHealthRegistry,
        service_name: String,
    ) -> Self {
        self.service_health = Some((registry, service_name));
        self
    }

    // Removed is_sandbox() helper as logic moved to EnergyManager

    pub async fn start(self: Arc<Self>, token: tokio_util::sync::CancellationToken) -> Result<()> {
        info!(network=%self.network.display_name(&self.environment), "Sweeper service started");
        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    info!("Sweeper received shutdown signal");
                    break;
                }
                _ = async {
                    // 1. Broadcast pending sweeps (concurrently)
                    if let Err(e) = self.clone().broadcast_cycle().await {
                        error!("Broadcast cycle error: {}", e);
                    }
                    // 2. Check confirmation status
                    if let Err(e) = self.confirmation_cycle().await {
                        error!("Confirmation cycle error: {}", e);
                    }
                    // 3. Recycle cooling addresses
                    if let Err(e) = self.recycle_cycle().await {
                        error!("Recycle cycle error: {}", e);
                    }
                    // 4. Recycle expired Assigned addresses (P0 Fix: prevent address pool leak)
                    if let Err(e) = self.recycle_expired_assigned_cycle().await {
                        error!("Recycle expired assigned error: {}", e);
                    }
                } => {}
            }

            // Heartbeat after successful cycle (proves we're alive and making progress)
            if let Some((ref registry, ref name)) = self.service_health {
                registry.heartbeat(name);
            }

            // Wait with cancellation support
            tokio::select! {
                _ = token.cancelled() => {
                    info!("Sweeper received shutdown signal during wait");
                    break;
                }
                _ = sleep(Duration::from_secs(60)) => {}
            }
        }
        info!("Sweeper service shutdown complete");
        Ok(())
    }

    /// Sweep a specific address (called by indexer)
    ///
    /// Sweeps ALL token balances (USDT + USDC) on the address, not just one token.
    pub async fn sweep_address(&self, network: &str, address: &str) -> Result<String> {
        use crate::entity::{checkout_sessions, CheckoutSessions};

        info!(network, address, "Manual sweep triggered");

        // Find the address
        let addr = Addresses::find()
            .filter(addresses::Column::Network.eq(network))
            .filter(addresses::Column::Address.eq(address))
            .filter(addresses::Column::Status.eq(addresses::AddressStatus::Detected))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Address not found or not in Detected state"))?;

        // Get merchant
        let merchant = merchants::Entity::find_by_id(&addr.merchant_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Merchant not found"))?;

        // Get latest session for this address (for audit linkage)
        // DB failure shouldn't prevent sweep - just skip session_id
        let session_id = match CheckoutSessions::find()
            .filter(checkout_sessions::Column::Network.eq(network))
            .filter(checkout_sessions::Column::PayAddress.eq(address))
            .order_by_desc(checkout_sessions::Column::CreatedAt)
            .one(&self.db)
            .await
        {
            Ok(Some(s)) => Some(s.id),
            Ok(None) => None,
            Err(e) => {
                warn!(network=%network, address=%address, error=%e, "Failed to query session for audit, proceeding without session_id");
                None
            }
        };

        // Sweep all tokens with balance (USDT + USDC)
        // NOTE: execute_sweep_logic atomically transitions address Detected → Sweeping.
        // After the first token sweeps, the CAS guard will reject the second token's sweep
        // (address is no longer Detected). This is expected — the second token will be
        // swept in the next broadcast_cycle after the first sweep confirms and the address
        // returns to Detected (if balance remains).
        let chain_config = self.network.chain_config(&self.environment);
        let mut last_tx_hash = String::new();

        // USDT sweep
        if addr.usdt_balance > 0 {
            match self
                .execute_sweep_logic(
                    &addr,
                    &merchant,
                    session_id.clone(),
                    None,
                    None,
                    outbound_transactions::OutboundOperationType::AutoSweep,
                    "USDT",
                    &chain_config.usdt_contract,
                )
                .await
            {
                Ok(hash) => last_tx_hash = hash,
                Err(e) => warn!(address=%address, token="USDT", error=%e, "USDT sweep failed"),
            }
        }

        // USDC sweep (if supported on this chain)
        // May fail with CAS contention if USDT sweep already changed status — that's OK.
        if addr.usdc_balance > 0 {
            if let Some(ref usdc_contract) = chain_config.usdc_contract {
                match self
                    .execute_sweep_logic(
                        &addr,
                        &merchant,
                        session_id,
                        None,
                        None,
                        outbound_transactions::OutboundOperationType::AutoSweep,
                        "USDC",
                        usdc_contract,
                    )
                    .await
                {
                    Ok(hash) => last_tx_hash = hash,
                    Err(e) => {
                        warn!(address=%address, token="USDC", error=%e, "USDC sweep failed (may be CAS contention from prior sweep)")
                    }
                }
            } else {
                warn!(address=%address, network=%network, usdc_balance=addr.usdc_balance,
                    "Address has USDC balance but chain has no USDC contract, skipping USDC sweep");
            }
        }

        if last_tx_hash.is_empty() {
            return Err(anyhow::anyhow!("No token balances to sweep"));
        }

        Ok(last_tx_hash)
    }

    /// Broadcast cycle: Find eligible addresses and initiate sweeps **concurrently**
    ///
    /// Per system_design.md, sweep eligibility is determined by:
    /// - **Criteria A (Normal)**: Session status is `Paid` or `Overpaid`
    /// - **Criteria B (Risk)**: Balance exceeds large amount threshold (force sweep) - currently disabled for non-Paid sessions.
    ///
    /// Note: Expired and Orphaned sessions are NO LONGER auto-swept. They must be resolved via Resolution Center.
    ///
    /// **Concurrency**: Uses `Semaphore` to limit max concurrent sweeps to avoid RPC overload.
    pub async fn broadcast_cycle(self: Arc<Self>) -> Result<()> {
        use crate::entity::{checkout_sessions, CheckoutSessions, Network};

        // Custom struct for initial candidate fetch
        #[derive(Debug, FromQueryResult)]
        struct SweepCandidate {
            network: String,
            address: String,
            #[allow(dead_code)]
            path_index: i32,
            merchant_id: String,
            sweep_attempts: i32,
            usdt_balance: i64,
            usdc_balance: i64,
        }

        // Find candidates: Checkout addresses with Status = Detected (has received payment)
        let candidates = Addresses::find()
            .select_only()
            .column(addresses::Column::Network)
            .column(addresses::Column::Address)
            .column(addresses::Column::PathIndex)
            .column(addresses::Column::MerchantId)
            .column(addresses::Column::SweepAttempts)
            .column(addresses::Column::UsdtBalance)
            .column(addresses::Column::UsdcBalance)
            .filter(
                // Checkout addresses with Detected status — scoped to this sweeper's network
                sea_orm::Condition::all()
                    .add(addresses::Column::Status.eq(addresses::AddressStatus::Detected))
                    .add(addresses::Column::Network.eq(self.network.as_str())),
            )
            .into_model::<SweepCandidate>()
            .all(&self.db)
            .await?;

        // 2. Batch Fetch Sessions
        // ============================================================
        let cand_addresses: Vec<String> = candidates.iter().map(|c| c.address.clone()).collect();
        let sessions = if !cand_addresses.is_empty() {
            CheckoutSessions::find()
                .filter(checkout_sessions::Column::PayAddress.is_in(cand_addresses.clone()))
                .all(&self.db)
                .await?
        } else {
            Vec::new()
        };

        // Build Session Map: Pay Address -> Most Recent Session
        let mut session_map: HashMap<String, checkout_sessions::Model> = HashMap::new();
        let mut sorted_sessions = sessions;
        sorted_sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        for s in sorted_sessions {
            session_map.entry(s.pay_address.clone()).or_insert(s);
        }

        // ============================================================
        // 2b. MEDIUM-5 FIX: Batch Pre-Fetch Merchants
        // ============================================================
        // Collect unique merchant IDs from candidates
        let merchant_ids: std::collections::HashSet<String> =
            candidates.iter().map(|c| c.merchant_id.clone()).collect();

        // Batch fetch all merchants in one query
        let merchants_list = if !merchant_ids.is_empty() {
            merchants::Entity::find()
                .filter(merchants::Column::Id.is_in(merchant_ids.clone()))
                .all(&self.db)
                .await?
        } else {
            Vec::new()
        };

        // Build merchant lookup map
        let merchant_map: HashMap<String, merchants::Model> = merchants_list
            .into_iter()
            .map(|m| (m.id.clone(), m))
            .collect();

        // ============================================================
        // 2c. Batch Pre-Fetch Unresolved Exceptions
        // ============================================================
        // Addresses with Pending/Processing exceptions must NOT be auto-swept.
        // ManualTransfer refunds originate from the payment address, so funds must stay.
        let exception_blocked_addresses: HashSet<String> = if !cand_addresses.is_empty() {
            payment_exceptions::Entity::find()
                .filter(payment_exceptions::Column::ToAddress.is_in(cand_addresses.clone()))
                .filter(payment_exceptions::Column::Network.eq(self.network.as_str()))
                .filter(payment_exceptions::Column::Status.is_in([
                    payment_exceptions::ExceptionStatus::Pending,
                    payment_exceptions::ExceptionStatus::Processing,
                ]))
                .select_only()
                .column(payment_exceptions::Column::ToAddress)
                .into_tuple::<String>()
                .all(&self.db)
                .await?
                .into_iter()
                .collect()
        } else {
            HashSet::new()
        };
        if !exception_blocked_addresses.is_empty() {
            info!(
                count = exception_blocked_addresses.len(),
                "Sweep cycle: skipping addresses with unresolved exceptions"
            );
        }

        // ============================================================
        // 3. Concurrency Setup
        // ============================================================
        let semaphore = Arc::new(Semaphore::new(self.sweep_config.max_concurrent_sweeps));
        let mut join_set: JoinSet<()> = JoinSet::new();

        // 4. Process Candidates - Collect sweep tasks
        // ============================================================
        for cand in candidates {
            if cand.sweep_attempts >= self.sweep_config.max_sweep_attempts {
                warn!(address=%cand.address, attempts=cand.sweep_attempts, max=self.sweep_config.max_sweep_attempts, "Max sweep retries reached. Skipping.");
                continue;
            }

            // Resolve Network Enum (validate network string)
            if Network::from_str_lenient(cand.network.as_str()).is_none() {
                error!(address=%cand.address, network=%cand.network, "Unsupported network for sweeper");
                continue;
            }

            // Retrieve Session from Map
            let session = session_map.get(&cand.address);

            // ============================================================
            // Exception Guard: skip addresses with unresolved exceptions.
            // ManualTransfer refunds originate from the payment address, so funds
            // must remain in place until the merchant resolves the exception.
            // ============================================================
            if exception_blocked_addresses.contains(&cand.address) {
                debug!(address=%cand.address, "Skipping sweep: address has unresolved exceptions");
                continue;
            }

            // ============================================================
            // Core Optimization: Skip RPC if not eligible
            // Sweep criteria (aligned with system_design.md §3.3):
            //   A: Session Paid/Overpaid → always sweep (address must be recycled)
            //   B: Session Expired + amount >= threshold → residual recovery
            //   C: Orphan (no session) → sweep to treasury
            //   D: Detected/Pending → wait (avoid premature sweep wasting gas
            //      on partial-pay + top-up scenarios)
            //
            // Minimum amount for residual sweeps (Expired/Orphan):
            // Below this, gas cost may exceed recovered amount. 1 USDT is close
            // to break-even for TRON energy delegation (~0.7-1.5 USDT), but
            // sweeping is still worthwhile for address pool recovery.
            // Paid/Overpaid bypasses this because address recycling is mandatory.
            // ============================================================
            const SWEEP_MIN_AMOUNT: i64 = 1_000_000; // 1 token (6 decimals)

            // Get chain config to resolve token contracts
            let chain_config = self.network.chain_config(&self.environment);

            // Total balance across both tokens (for sweep eligibility decision)
            let total_balance = cand.usdt_balance + cand.usdc_balance;

            let sweep_eligible = {
                match session {
                    Some(s) => {
                        use crate::entity::checkout_sessions::SessionStatus;
                        match s.status {
                            SessionStatus::Paid | SessionStatus::Overpaid => true,
                            SessionStatus::Expired => {
                                // Sweep if EITHER:
                                // A) Session received payment (normal expired residual), OR
                                // B) Address has balance from exception payments (e.g., WrongToken
                                //    which updates address balance but not session.amount_received)
                                if s.amount_received >= SWEEP_MIN_AMOUNT
                                    || total_balance >= SWEEP_MIN_AMOUNT
                                {
                                    true
                                } else {
                                    debug!(address=%cand.address, amount_received=s.amount_received,
                                        address_balance=total_balance, threshold=SWEEP_MIN_AMOUNT,
                                        "Skipping expired sweep (below min amount)");
                                    false
                                }
                            }
                            _ => {
                                debug!(address=%cand.address, status=?s.status,
                                    "Skipping sweep (session not in terminal state)");
                                false
                            }
                        }
                    }
                    None => {
                        // Orphan address: no session → sweep to treasury
                        // Must check min amount to avoid wasting gas on dust
                        if total_balance >= SWEEP_MIN_AMOUNT {
                            debug!(address=%cand.address, balance=total_balance, "Sweeping orphan address (no session)");
                            true
                        } else {
                            debug!(address=%cand.address, balance=total_balance, threshold=SWEEP_MIN_AMOUNT,
                                "Skipping orphan sweep (below min amount)");
                            false
                        }
                    }
                }
            };

            if !sweep_eligible {
                // Fast-track to Cooling for addresses that don't need sweeping:
                // - Expired: Sub-threshold exception balance (WrongToken dust, not worth gas to sweep)
                if let Some(s) = session {
                    use crate::entity::checkout_sessions::SessionStatus;
                    if s.status == SessionStatus::Expired {
                        if let Err(e) = Addresses::update_many()
                            .col_expr(
                                addresses::Column::Status,
                                Expr::value(addresses::AddressStatus::Cooling),
                            )
                            .col_expr(addresses::Column::UpdatedAt, Expr::cust("NOW()"))
                            .filter(addresses::Column::Network.eq(&cand.network))
                            .filter(addresses::Column::Address.eq(&cand.address))
                            .filter(
                                addresses::Column::Status.eq(addresses::AddressStatus::Detected),
                            )
                            .exec(&self.db)
                            .await
                        {
                            warn!(address=%cand.address, error=%e,
                                "Failed to fast-track address to Cooling");
                        } else {
                            debug!(address=%cand.address, status=?s.status,
                                "Fast-tracked expired dust address to Cooling");
                        }
                    }
                }
                continue;
            }

            // ============================================================
            // Fetch Merchant & Config (from pre-fetched map)
            // ============================================================
            // 1. Merchant (from pre-fetched map - MEDIUM-5 FIX)
            let merchant = match merchant_map.get(&cand.merchant_id) {
                Some(m) => m.clone(),
                None => continue,
            };

            // 2. Determine sweep destination: ALL sweeps go to Platform Treasury (Ledger mode)
            let collection_address = match &self.sweep_config.platform_treasury_address {
                Some(treasury) => treasury.clone(),
                None => {
                    warn!(
                        address=%cand.address,
                        "Skipping sweep: platform_treasury_address not configured"
                    );
                    continue;
                }
            };

            // 4. Get Address Entity
            let addr = match Addresses::find()
                .filter(addresses::Column::Network.eq(&cand.network))
                .filter(addresses::Column::Address.eq(&cand.address))
                .one(&self.db)
                .await
            {
                Ok(Some(a)) => a,
                _ => continue,
            };

            // 5. Prepare data for spawned task (all owned types)
            let session_id = session.map(|s| s.id.clone());
            let address_str = cand.address.clone();

            // ============================================================
            // 5b. Generate per-token SweepTasks
            // An address may have both USDT and USDC; each gets a separate sweep tx.
            // ============================================================
            struct SweepTask {
                token: String,
                token_contract: String,
                #[allow(dead_code)]
                balance: i64,
            }
            let mut sweep_tasks: Vec<SweepTask> = Vec::new();

            if cand.usdt_balance > 0 {
                sweep_tasks.push(SweepTask {
                    token: "USDT".to_string(),
                    token_contract: chain_config.usdt_contract.clone(),
                    balance: cand.usdt_balance,
                });
            }
            if cand.usdc_balance > 0 {
                if let Some(ref usdc_contract) = chain_config.usdc_contract {
                    sweep_tasks.push(SweepTask {
                        token: "USDC".to_string(),
                        token_contract: usdc_contract.clone(),
                        balance: cand.usdc_balance,
                    });
                } else {
                    warn!(address=%cand.address, network=%cand.network, usdc_balance=cand.usdc_balance,
                        "Address has USDC balance but chain has no USDC contract configured, skipping USDC sweep");
                }
            }

            if sweep_tasks.is_empty() {
                // No token balances to sweep (shouldn't happen as Detected implies funds)
                continue;
            }

            // ============================================================
            // 6. Spawn Sweep Task with Semaphore Rate Limiting
            // ============================================================
            for task in sweep_tasks {
                let permit = semaphore
                    .clone()
                    .acquire_owned()
                    .await
                    .map_err(|e| anyhow!("Semaphore acquire failed: {}", e))?;
                let sweeper = self.clone();
                let addr = addr.clone();
                let merchant = merchant.clone();
                let session_id = session_id.clone();
                let collection_address = collection_address.clone();
                let address_str = address_str.clone();

                join_set.spawn(async move {
                    // Permit is held until this task completes
                    let _permit = permit;
                    let sweep_start = std::time::Instant::now();
                    let net_str = addr.network.clone();

                    match sweeper.execute_sweep_logic(
                        &addr, &merchant, session_id, Some(collection_address), None,
                        outbound_transactions::OutboundOperationType::AutoSweep,
                        &task.token, &task.token_contract,
                    ).await {
                        Ok(_) => {
                            crate::services::metrics::inc_sweep(&net_str, "success", &task.token);
                            crate::services::metrics::record_sweep_duration(&net_str, sweep_start.elapsed().as_secs_f64());
                            info!(address=%address_str, token=%task.token, "Sweep executed successfully");
                        }
                        Err(e) => {
                            crate::services::metrics::inc_sweep(&net_str, "failed", &task.token);
                            crate::services::metrics::record_sweep_duration(&net_str, sweep_start.elapsed().as_secs_f64());
                            let msg = e.to_string();
                            if msg.contains("collection_address") || msg.contains("Merchant not found") {
                                error!(address=%address_str, token=%task.token, error=%msg, "Sweep failed (Permanent Config Error)");
                            } else {
                                error!(address=%address_str, token=%task.token, error=%msg, "Sweep failed (Transient/Network Error)");
                            }
                        }
                    }
                });
            }
        }

        // ============================================================
        // 7. Await All Spawned Tasks
        // ============================================================
        let spawned_count = join_set.len();
        if spawned_count > 0 {
            info!(count=%spawned_count, max_concurrent=%self.sweep_config.max_concurrent_sweeps, "Awaiting concurrent sweep tasks");
        }

        while let Some(result) = join_set.join_next().await {
            if let Err(e) = result {
                error!(error=%e, "Sweep task panicked");
            }
        }

        Ok(())
    }

    /// Estimate the fee that will be charged to the merchant for a sweep.
    /// Deprecated in Ledger mode — fees are calculated at payment confirmation, not sweep.
    #[doc(hidden)]
    pub fn estimate_sweep_fee(&self, _sweep_amount: i64) -> i64 {
        // In Ledger mode, sweep itself has no fee (billing happens on payment confirmed)
        0
    }

    /// Execute sweep logic for a single address.
    ///
    /// This method is public to allow `ResolutionService` to trigger manual sweeps.
    /// It handles the entire sweep lifecycle:
    /// 1. Validation (Collection Address, Balance via executor)
    /// 2. Database State Updates (Atomic lock: address → Sweeping)
    /// 3. Chain execution via `SweepExecutor` (build + sign + resources + broadcast)
    /// 4. Rollback on failure
    ///
    /// # Arguments
    /// * `collection_address_override` - If provided, skips DB lookup for collection address.
    ///   Used by `broadcast_cycle` to avoid redundant queries.
    /// * `exception_id` - For manual sweeps/transfers, links to the payment_exception.
    /// * `sweep_type` - Type of operation (AutoSweep, ManualSweep, ManualTransfer).
    pub async fn execute_sweep_logic(
        &self,
        addr: &addresses::Model,
        merchant: &merchants::Model,
        session_id: Option<String>,
        collection_address_override: Option<String>,
        exception_id: Option<String>,
        sweep_type: outbound_transactions::OutboundOperationType,
        token: &str,
        token_contract: &str,
    ) -> Result<String> {
        use crate::entity::{merchant_chain_accounts, Network};

        // Resolve Network + Environment from address record
        let net_enum = Network::from_str_lenient(addr.network.as_str())
            .ok_or_else(|| anyhow!("Invalid network: {}", addr.network))?;
        let env = self.environment.clone();

        let collection_address = match collection_address_override {
            Some(addr) => addr,
            None => {
                // Lookup from DB if not provided
                let chain_account = merchant_chain_accounts::Entity::find_by_id((
                    merchant.id.clone(),
                    env.clone(),
                    net_enum,
                ))
                .one(&self.db)
                .await?
                .ok_or_else(|| anyhow!("Merchant chain account not found"))?;

                match chain_account.collection_address {
                    Some(coll_addr) => coll_addr,
                    None => {
                        warn!(
                            merchant_id = %merchant.id,
                            address = %addr.address,
                            "Sweep skipped: merchant has no collection_address configured for this chain"
                        );
                        return Err(anyhow!(
                            "Sweep skipped: merchant has no collection_address configured"
                        ));
                    }
                }
            }
        };

        // Step 1: Get balance via executor (chain-agnostic)
        let balance = self
            .executor
            .get_balance(&addr.address, token_contract)
            .await?;
        if balance == 0 {
            return Err(anyhow!("No funds to sweep"));
        }

        // Create sweep transaction record
        let sweep_id = format!("swp_{}", Uuid::new_v4().to_string().replace("-", ""));

        // Step 2: Atomic DB update — lock address → Sweeping, insert sweep_tx
        // tx_hash is initially None; will be updated after executor returns.
        let network = addr.network.clone();
        let address = addr.address.clone();
        let merchant_id = merchant.id.clone();
        let collection_address_clone = collection_address.clone();
        let session_id_for_rollback = session_id.clone();
        let token_owned = token.to_string();
        let environment = self.environment;

        self.db
            .transaction::<_, (), sea_orm::DbErr>(|txn| {
                let network = network.clone();
                let address = address.clone();
                let sweep_id = sweep_id.clone();
                let merchant_id = merchant_id.clone();
                let collection_address = collection_address_clone.clone();
                let token_owned = token_owned.clone();
                let environment = environment.clone();

                Box::pin(async move {
                    // Update address status
                    let update_res = Addresses::update_many()
                        .col_expr(
                            addresses::Column::Status,
                            Expr::value(addresses::AddressStatus::Sweeping),
                        )
                        .col_expr(
                            addresses::Column::SweepAttempts,
                            Expr::cust("sweep_attempts + 1"),
                        )
                        .col_expr(addresses::Column::UpdatedAt, Expr::cust("NOW()"))
                        .filter(addresses::Column::Network.eq(network.clone()))
                        .filter(addresses::Column::Address.eq(address.clone()))
                        .filter(addresses::Column::Status.eq(addresses::AddressStatus::Detected))
                        .exec(txn)
                        .await?;

                    if update_res.rows_affected == 0 {
                        return Err(sea_orm::DbErr::Custom(
                            "Address already locked or status changed".to_owned(),
                        ));
                    }

                    let sweep_tx = outbound_transactions::ActiveModel {
                        id: Set(sweep_id),
                        merchant_id: Set(merchant_id.clone()),
                        session_id: Set(session_id.clone()),
                        exception_id: Set(exception_id.clone()),
                        operation_type: Set(sweep_type.clone()),
                        network: Set(network.clone()),
                        from_address: Set(address.clone()),
                        to_address: Set(collection_address),
                        energy_delegate_tx_hash: Set(None),
                        funding_tx_hash: Set(None),
                        tx_hash: Set(None),
                        amount: Set(balance),
                        state: Set(OutboundState::Preparing),
                        environment: Set(environment),
                        token: Set(token_owned),
                        ..Default::default()
                    };
                    sweep_tx.insert(txn).await?;

                    Ok(())
                })
            })
            .await?;

        // Step 3: Execute chain-side sweep via executor
        // All chain-specific logic (build tx, sign, resources, broadcast) is encapsulated here.
        // If this fails, we rollback the DB state.
        let result = match self
            .executor
            .execute_sweep(
                &addr.address,
                &collection_address,
                merchant.account_index.unwrap_or(0),
                addr.path_index as u32,
                token_contract,
                &sweep_id,
                &self.outbound_store,
            )
            .await
        {
            Ok(result) => result,
            Err(e) => {
                let err_msg = e.to_string();
                let is_permanent = Self::is_permanent_sweep_error(&err_msg);

                if is_permanent {
                    error!(
                        address = %addr.address,
                        sweep_id = %sweep_id,
                        error = %err_msg,
                        "Sweep failed with PERMANENT error — stopping retries"
                    );
                    self.alerting_service.send_alert(
                        "sweeper_permanent_failure",
                        AlertLevel::Critical,
                        &format!(
                            "🚨 Sweep PERMANENT failure (retries stopped): address={} network={} error={}",
                            addr.address, addr.network, err_msg
                        ),
                    );
                    self.mark_sweep_permanent_failure(
                        &addr.network,
                        &addr.address,
                        &sweep_id,
                        &format!("Permanent failure: {}", err_msg),
                        session_id_for_rollback.as_deref(),
                    )
                    .await;
                } else {
                    error!(
                        address = %addr.address,
                        sweep_id = %sweep_id,
                        error = %err_msg,
                        "Sweep failed with transient error, rolling back for retry"
                    );
                    self.alerting_service.send_alert(
                        "sweeper_broadcast_failed",
                        AlertLevel::Warning,
                        &format!(
                            "Sweep broadcast failed (transient, will retry): address={} network={} error={}",
                            addr.address, addr.network, err_msg
                        ),
                    );
                    self.rollback_sweep(
                        &addr.network,
                        &addr.address,
                        &sweep_id,
                        &format!("Sweep execution failed: {}", err_msg),
                        session_id_for_rollback.as_deref(),
                    )
                    .await;
                }
                return Err(e);
            }
        };

        if !self
            .outbound_store
            .adopt_executor_result(
                &sweep_id,
                &result.tx_hash,
                result.broadcast_disposition.clone(),
            )
            .await?
        {
            return Err(anyhow!(
                "Sweep outbound handoff conflicted for {}",
                sweep_id
            ));
        }

        let tx_hash = result.tx_hash.clone();

        // Step 4: Calculate gas cost in USDT via PriceOracle (chain-aware decimals)
        let chain_family = net_enum.chain_family();
        let cost_in_usdt: Option<i64> = {
            use rust_decimal::prelude::*;
            // Native token precision: TRON SUN = 6 decimals, EVM Wei = 18 decimals
            let native_decimals: u32 = match chain_family {
                crate::entity::ChainFamily::Tron => 6,
                crate::entity::ChainFamily::Evm => 18,
                crate::entity::ChainFamily::Solana => 9,
            };
            match self.price_oracle.get_native_usdt_price(self.network).await {
                Ok(price) => {
                    let divisor = Decimal::from(10u64.pow(native_decimals));
                    let cost_native = Decimal::from(result.gas_cost_native) / divisor;
                    let cost_usdt = cost_native * price;
                    // Convert to 6-decimal i64 for DB
                    let usdt_6dec = cost_usdt * Decimal::from(1_000_000i64);
                    usdt_6dec.to_i64()
                }
                Err(e) => {
                    warn!(
                        sweep_id = %sweep_id,
                        error = %e,
                        "Failed to get native/USDT price, cost_in_usdt will be NULL"
                    );
                    None
                }
            }
        };

        // Uneconomical sweep alert (Sentry-visible via warn!)
        if let Some(cost) = cost_in_usdt {
            if cost > result.amount_swept as i64 {
                warn!(
                    sweep_id = %sweep_id,
                    amount_swept_usdt = result.amount_swept,
                    cost_usdt = cost,
                    chain = ?chain_family,
                    "Uneconomical sweep: gas cost exceeds swept amount"
                );
            }
        }

        // Step 5: Update sweep_tx with actual tx_hash, funding info, and cost
        let _ = OutboundTransactions::update_many()
            .col_expr(
                outbound_transactions::Column::TxHash,
                Expr::value(Some(result.tx_hash.clone())),
            )
            .col_expr(
                outbound_transactions::Column::FundingTxHash,
                Expr::value(result.funding_tx_hash),
            )
            .col_expr(
                outbound_transactions::Column::Amount,
                Expr::value(result.amount_swept),
            )
            .col_expr(
                outbound_transactions::Column::CostInUsdt,
                Expr::value(cost_in_usdt),
            )
            .filter(outbound_transactions::Column::Id.eq(sweep_id.clone()))
            .exec(&self.db)
            .await;

        // Update settlement status on the checkout session
        if let Some(sid) = session_id_for_rollback.clone() {
            let _ = CheckoutSessions::update_many()
                .col_expr(
                    checkout_sessions::Column::SettlementStatus,
                    Expr::value(SettlementStatus::Settling),
                )
                .col_expr(
                    checkout_sessions::Column::SettlementTxHash,
                    Expr::value(Some(tx_hash.clone())),
                )
                .col_expr(checkout_sessions::Column::UpdatedAt, Expr::cust("NOW()"))
                .filter(checkout_sessions::Column::Id.eq(sid))
                .exec(&self.db)
                .await;
        }

        info!(
            sweep_id = %sweep_id,
            tx_hash = %tx_hash,
            amount = result.amount_swept,
            "Sweep handed off to confirmation"
        );

        Ok(tx_hash)
    }

    /// Rollback a failed sweep attempt (pre-broadcast failure).
    ///
    /// Resets the address to Detected state, decrements sweep_attempts (undoing the
    /// increment from execute_sweep_logic), and marks the sweep transaction as Failed.
    /// Also reverts checkout_sessions.settlement_status from Settling → Unsettled.
    ///
    /// Pre-broadcast failures (resource prep, signing, broadcast network error) are
    /// transient and should not count against max_sweep_attempts. The address has real
    /// USDT that must eventually be swept.
    async fn rollback_sweep(
        &self,
        network: &str,
        address: &str,
        sweep_id: &str,
        error_reason: &str,
        session_id: Option<&str>,
    ) {
        error!(
            network = %network,
            address = %address,
            sweep_id = %sweep_id,
            error = %error_reason,
            "Rolling back sweep due to failure"
        );

        let result = self
            .db
            .transaction::<_, (), sea_orm::DbErr>(|txn| {
                let network = network.to_string();
                let address = address.to_string();
                let sweep_id = sweep_id.to_string();
                let error_reason = error_reason.to_string();
                let session_id = session_id.map(|s| s.to_string());

                Box::pin(async move {
                    // Claim the pre-signing journal row before releasing business state.
                    let transition = OutboundTransactions::update_many()
                        .col_expr(
                            outbound_transactions::Column::State,
                            sea_orm::sea_query::Expr::value(OutboundState::Failed),
                        )
                        .col_expr(
                            outbound_transactions::Column::ErrorMessage,
                            sea_orm::sea_query::Expr::value(error_reason.clone()),
                        )
                        .filter(outbound_transactions::Column::Id.eq(sweep_id))
                        .filter(outbound_transactions::Column::State.eq(OutboundState::Preparing))
                        .exec(txn)
                        .await?;
                    if transition.rows_affected != 1 {
                        return Ok(());
                    }

                    // Reset address to Detected and undo the sweep_attempts increment.
                    Addresses::update_many()
                        .col_expr(
                            addresses::Column::Status,
                            sea_orm::sea_query::Expr::value(addresses::AddressStatus::Detected),
                        )
                        .col_expr(
                            addresses::Column::SweepAttempts,
                            sea_orm::sea_query::Expr::cust("GREATEST(sweep_attempts - 1, 0)"),
                        )
                        .col_expr(
                            addresses::Column::UpdatedAt,
                            sea_orm::sea_query::Expr::cust("NOW()"),
                        )
                        .filter(addresses::Column::Network.eq(network))
                        .filter(addresses::Column::Address.eq(address))
                        .exec(txn)
                        .await?;

                    // Revert settlement_status from Settling → Unsettled
                    if let Some(sid) = session_id {
                        CheckoutSessions::update_many()
                            .col_expr(
                                checkout_sessions::Column::SettlementStatus,
                                sea_orm::sea_query::Expr::value(SettlementStatus::Unsettled),
                            )
                            .col_expr(
                                checkout_sessions::Column::SettlementTxHash,
                                sea_orm::sea_query::Expr::value(None::<String>),
                            )
                            .col_expr(
                                checkout_sessions::Column::UpdatedAt,
                                sea_orm::sea_query::Expr::cust("NOW()"),
                            )
                            .filter(checkout_sessions::Column::Id.eq(sid))
                            .filter(
                                checkout_sessions::Column::SettlementStatus
                                    .eq(SettlementStatus::Settling),
                            )
                            .exec(txn)
                            .await?;
                    }

                    Ok(())
                })
            })
            .await;

        if let Err(e) = result {
            error!(
                network = %network,
                address = %address,
                sweep_id = %sweep_id,
                rollback_error = %e,
                "CRITICAL: Rollback failed! Address may be stuck in Sweeping state. Manual intervention required."
            );
            self.alerting_service.send_alert(
                "sweeper_rollback_failed",
                AlertLevel::Critical,
                &format!(
                    "🚨 Sweep rollback FAILED! Address {} ({}) stuck in Sweeping. Manual intervention required: {}",
                    address, network, e
                ),
            );
        }
    }

    /// Classify sweep errors as permanent (will never succeed) vs transient (may succeed on retry).
    ///
    /// Permanent errors indicate fundamental issues that no amount of retrying will fix:
    /// - Account doesn't exist on-chain (never activated)
    /// - Key mismatch (wrong mnemonic → wrong signature)
    /// - Zero balance (nothing to sweep)
    ///
    /// Transient errors are network/infra issues that may resolve on retry:
    /// - Timeouts, connection failures, rate limits, RPC node errors
    fn is_permanent_sweep_error(error_msg: &str) -> bool {
        let permanent_patterns = [
            // TRON: address not activated (no TRX ever received)
            "no OwnerAccount",
            "OwnerAccount not found",
            // TRON/EVM: signature/key mismatch (wrong mnemonic → wrong private key)
            "validate signature error",
            "Invalid signature",
            "invalid sender",
            // EVM: contract-level revert (logic error, not gas)
            "execution reverted",
        ];

        let lower = error_msg.to_lowercase();
        permanent_patterns
            .iter()
            .any(|p| lower.contains(&p.to_lowercase()))
    }

    /// Handle a permanent sweep failure:
    /// - Reset address to Detected (don't leave stuck in Sweeping)
    /// - Set sweep_attempts = max_sweep_attempts (immediately stops retries)
    /// - Mark sweep_transaction as Failed with error message
    /// - Revert session settlement_status if applicable
    ///
    /// Unlike `rollback_sweep` (which decrements sweep_attempts for retry),
    /// this method caps sweep_attempts to ensure no further retry cycles.
    async fn mark_sweep_permanent_failure(
        &self,
        network: &str,
        address: &str,
        sweep_id: &str,
        error_reason: &str,
        session_id: Option<&str>,
    ) {
        let max_attempts = self.sweep_config.max_sweep_attempts;

        error!(
            network = %network,
            address = %address,
            sweep_id = %sweep_id,
            error = %error_reason,
            max_attempts = max_attempts,
            "Marking sweep as permanent failure (attempts capped to max)"
        );

        let result = self
            .db
            .transaction::<_, (), sea_orm::DbErr>(|txn| {
                let network = network.to_string();
                let address = address.to_string();
                let sweep_id = sweep_id.to_string();
                let error_reason = error_reason.to_string();
                let session_id = session_id.map(|s| s.to_string());

                Box::pin(async move {
                    // Permanent classification is only valid before signing.
                    let transition = OutboundTransactions::update_many()
                        .col_expr(
                            outbound_transactions::Column::State,
                            sea_orm::sea_query::Expr::value(OutboundState::Failed),
                        )
                        .col_expr(
                            outbound_transactions::Column::ErrorMessage,
                            sea_orm::sea_query::Expr::value(error_reason.clone()),
                        )
                        .filter(outbound_transactions::Column::Id.eq(sweep_id))
                        .filter(outbound_transactions::Column::State.eq(OutboundState::Preparing))
                        .exec(txn)
                        .await?;
                    if transition.rows_affected != 1 {
                        return Ok(());
                    }

                    // Reset address to Detected but cap sweep_attempts at max
                    // (broadcast_cycle skips addresses with sweep_attempts >= max)
                    Addresses::update_many()
                        .col_expr(
                            addresses::Column::Status,
                            sea_orm::sea_query::Expr::value(addresses::AddressStatus::Detected),
                        )
                        .col_expr(
                            addresses::Column::SweepAttempts,
                            sea_orm::sea_query::Expr::value(max_attempts),
                        )
                        .col_expr(
                            addresses::Column::UpdatedAt,
                            sea_orm::sea_query::Expr::cust("NOW()"),
                        )
                        .filter(addresses::Column::Network.eq(network))
                        .filter(addresses::Column::Address.eq(address))
                        .exec(txn)
                        .await?;

                    // Revert settlement_status from Settling → Unsettled
                    if let Some(sid) = session_id {
                        CheckoutSessions::update_many()
                            .col_expr(
                                checkout_sessions::Column::SettlementStatus,
                                sea_orm::sea_query::Expr::value(SettlementStatus::Unsettled),
                            )
                            .col_expr(
                                checkout_sessions::Column::SettlementTxHash,
                                sea_orm::sea_query::Expr::value(None::<String>),
                            )
                            .col_expr(
                                checkout_sessions::Column::UpdatedAt,
                                sea_orm::sea_query::Expr::cust("NOW()"),
                            )
                            .filter(checkout_sessions::Column::Id.eq(sid))
                            .filter(
                                checkout_sessions::Column::SettlementStatus
                                    .eq(SettlementStatus::Settling),
                            )
                            .exec(txn)
                            .await?;
                    }

                    Ok(())
                })
            })
            .await;

        if let Err(e) = result {
            error!(
                network = %network,
                address = %address,
                sweep_id = %sweep_id,
                error = %e,
                "CRITICAL: Failed to mark sweep as permanent failure! Address may be stuck."
            );
        }
    }

    pub async fn confirmation_cycle(&self) -> Result<()> {
        let pending_txs = OutboundTransactions::find()
            .filter(
                outbound_transactions::Column::Purpose
                    .eq(outbound_transactions::OutboundPurpose::TokenTransfer),
            )
            .filter(outbound_transactions::Column::ParentTransactionId.is_null())
            .filter(outbound_transactions::Column::OperationType.is_in([
                outbound_transactions::OutboundOperationType::AutoSweep,
                outbound_transactions::OutboundOperationType::ManualSweep,
                outbound_transactions::OutboundOperationType::ManualTransfer,
            ]))
            .filter(outbound_transactions::Column::State.is_in([
                OutboundState::Signed,
                OutboundState::BroadcastUnknown,
                OutboundState::Pending,
            ]))
            .filter(outbound_transactions::Column::Network.eq(self.network.as_str()))
            .all(&self.db)
            .await?;

        if pending_txs.is_empty() {
            return Ok(());
        }

        // Optimization: Fetch latest block via executor (used for stuck-tx age detection)
        let _latest_block = self.executor.get_current_block().await.ok();

        for tx_rec in pending_txs {
            let tx_hash = match &tx_rec.tx_hash {
                Some(h) => h,
                None => continue,
            };

            match self
                .executor
                .check_tx_status(tx_hash, self.sweep_config.confirmation_blocks as i32)
                .await
            {
                Ok(SweepTxStatus::Confirmed) => {
                    info!(tx_hash=%tx_hash, "Sweep confirmed on chain");
                    if let Err(e) = self.finalize_sweep_success(&tx_rec).await {
                        error!(tx_hash=%tx_hash, error=%e, "Failed to finalize sweep success, will retry");
                    }
                }
                Ok(SweepTxStatus::Failed) => {
                    error!(tx_hash=%tx_hash, "Sweep REVERTED on chain — funds may still be at source address");
                    if let Err(e) = self
                        .handle_sweep_failure(
                            &tx_rec,
                            OutboundState::Reverted,
                            "Transaction reverted on-chain",
                        )
                        .await
                    {
                        error!(tx_hash=%tx_hash, error=%e, "Failed to handle sweep failure, will retry");
                    }
                }
                Ok(SweepTxStatus::Pending) => {
                    let _ = self
                        .outbound_store
                        .mark_state(&tx_rec.id, OutboundState::Pending, None)
                        .await;
                    let age = chrono::Utc::now() - tx_rec.created_at.with_timezone(&chrono::Utc);
                    let stuck_threshold =
                        chrono::Duration::seconds(self.sweep_config.stuck_timeout_seconds as i64);
                    if age > stuck_threshold {
                        error!(
                            tx_hash = %tx_hash,
                            age_seconds = age.num_seconds(),
                            threshold_seconds = self.sweep_config.stuck_timeout_seconds,
                            status = "Pending",
                            "Sweep transaction remains pending beyond the alert threshold"
                        );
                    }
                }
                Ok(SweepTxStatus::NotFound) => {
                    let payload = match self.outbound_store.decrypt_payload(&tx_rec) {
                        Ok(payload) => payload,
                        Err(error) => {
                            error!(outbound_id = %tx_rec.id, error = %error, "Cannot recover signed sweep payload");
                            continue;
                        }
                    };
                    match self.executor.recover_broadcast(&payload).await {
                        Ok(crate::services::outbound::RecoveryDisposition::Pending) => {
                            let _ = self
                                .outbound_store
                                .mark_broadcast(
                                    &tx_rec.id,
                                    crate::services::outbound::BroadcastDisposition::Accepted,
                                    None,
                                )
                                .await;
                        }
                        Ok(crate::services::outbound::RecoveryDisposition::BroadcastUnknown(
                            error,
                        )) => {
                            let _ = self
                                .outbound_store
                                .mark_broadcast(
                                    &tx_rec.id,
                                    crate::services::outbound::BroadcastDisposition::Unknown,
                                    Some(error),
                                )
                                .await;
                        }
                        Ok(
                            disposition @ (crate::services::outbound::RecoveryDisposition::Expired
                            | crate::services::outbound::RecoveryDisposition::Replaced),
                        ) => {
                            let state = match disposition {
                                crate::services::outbound::RecoveryDisposition::Expired => {
                                    OutboundState::Expired
                                }
                                crate::services::outbound::RecoveryDisposition::Replaced => {
                                    OutboundState::Replaced
                                }
                                _ => unreachable!(),
                            };
                            let reason = "Signed transaction was proven unable to land";
                            let evidence = self
                                .outbound_store
                                .stage_terminal_evidence(&tx_rec.id, state.clone(), reason)
                                .await;
                            match evidence {
                                Ok(crate::services::outbound::TerminalEvidence::Ready) => {
                                    if let Err(error) =
                                        self.handle_sweep_failure(&tx_rec, state, reason).await
                                    {
                                        error!(outbound_id = %tx_rec.id, error = %error, "Failed to release terminal sweep");
                                    }
                                }
                                Ok(crate::services::outbound::TerminalEvidence::Staged) => {
                                    warn!(outbound_id = %tx_rec.id, "Staged terminal sweep evidence; awaiting independent recheck");
                                }
                                Ok(crate::services::outbound::TerminalEvidence::Conflict) => {}
                                Err(error) => {
                                    error!(outbound_id = %tx_rec.id, error = %error, "Failed to stage terminal sweep evidence");
                                }
                            }
                        }
                        Err(error) => {
                            error!(outbound_id = %tx_rec.id, error = %error, "Sweep recovery check failed");
                        }
                    }
                }
                Err(e) => {
                    error!(tx_hash=%tx_hash, error=%e, "Failed to check tx status");
                }
            }
        }

        // ================================================================
        // Orphan recovery: Sweeping + no tx_hash for > 5 min
        // If app crashes between CAS (address → Sweeping) and broadcast
        // completion (tx_hash written), the address is permanently stuck.
        // Active broadcasts complete in seconds, so 5 min is safe.
        // ================================================================
        let orphan_cutoff = chrono::Utc::now() - chrono::Duration::seconds(300);

        // 1. Preparing + no signed hash is safe to reset; signed rows are never reset by age.
        let orphan_sweeps = OutboundTransactions::update_many()
            .col_expr(
                outbound_transactions::Column::State,
                Expr::value(OutboundState::Failed),
            )
            .col_expr(
                outbound_transactions::Column::ErrorMessage,
                Expr::value(Some(
                    "Auto-reset: broadcast interrupted (no tx_hash after 5 min)",
                )),
            )
            .filter(outbound_transactions::Column::State.eq(OutboundState::Preparing))
            .filter(
                outbound_transactions::Column::Purpose
                    .eq(outbound_transactions::OutboundPurpose::TokenTransfer),
            )
            .filter(outbound_transactions::Column::ParentTransactionId.is_null())
            .filter(outbound_transactions::Column::OperationType.is_in([
                outbound_transactions::OutboundOperationType::AutoSweep,
                outbound_transactions::OutboundOperationType::ManualSweep,
                outbound_transactions::OutboundOperationType::ManualTransfer,
            ]))
            .filter(outbound_transactions::Column::TxHash.is_null())
            .filter(outbound_transactions::Column::Network.eq(self.network.as_str()))
            .filter(outbound_transactions::Column::CreatedAt.lt(orphan_cutoff))
            .exec(&self.db)
            .await;

        if let Ok(result) = &orphan_sweeps {
            if result.rows_affected > 0 {
                warn!(
                    count = result.rows_affected,
                    network = %self.network,
                    "Reset orphaned Pending sweep records (no tx_hash) → Failed"
                );
            }
        }

        // 2. Reset orphaned addresses: Sweeping → Detected (so broadcast_cycle re-picks them)
        //
        // CRITICAL: Only reset if there's NO Pending sweep with a tx_hash for this address.
        // Ghost broadcast scenario: broadcast may have succeeded + tx_hash written, but app died
        // before finalize_sweep_success. Address is Sweeping, sweep has tx_hash — the
        // confirmation_cycle will handle it. Resetting the address would cause double-sweep.
        let orphan_addr_result: Result<sea_orm::ExecResult, sea_orm::DbErr> = self
            .db
            .execute(sea_orm::Statement::from_sql_and_values(
                sea_orm::DbBackend::Postgres,
                r#"
                UPDATE addresses
                SET status = 'Detected',
                    sweep_attempts = GREATEST(sweep_attempts - 1, 0),
                    updated_at = NOW()
                WHERE status = 'Sweeping'
                  AND network = $1
                  AND updated_at < $2
                  AND NOT EXISTS (
                      SELECT 1 FROM outbound_transactions
                      WHERE outbound_transactions.from_address = addresses.address
                        AND outbound_transactions.network = addresses.network
                        AND outbound_transactions.purpose = 'token_transfer'
                        AND outbound_transactions.parent_transaction_id IS NULL
                        AND outbound_transactions.operation_type IN ('auto_sweep', 'manual_sweep', 'manual_transfer')
                        AND outbound_transactions.state IN ('Signed', 'BroadcastUnknown', 'Pending')
                        AND outbound_transactions.tx_hash IS NOT NULL
                  )
                "#,
                [self.network.as_str().into(), orphan_cutoff.into()],
            ))
            .await;

        if let Ok(result) = &orphan_addr_result {
            if result.rows_affected() > 0 {
                warn!(
                    count = result.rows_affected(),
                    network = %self.network,
                    "Reset orphaned Sweeping addresses (no active sweep with tx_hash) → Detected"
                );
            }
        }

        Ok(())
    }

    async fn finalize_sweep_success(&self, tx_rec: &outbound_transactions::Model) -> Result<bool> {
        let network = tx_rec.network.clone();
        let from_address = tx_rec.from_address.clone();
        let _merchant_id = tx_rec.merchant_id.clone();
        let sweep_id = tx_rec.id.clone();
        let sweep_amount = tx_rec.amount;
        let session_id = tx_rec.session_id.clone();
        let swept_token = tx_rec.token.clone();

        // ============================================================
        // LEDGER MODE: No fee is charged at sweep time.
        // Fees were already deducted when the payment was credited to the merchant.
        // cost_in_usdt was calculated at sweep broadcast time using PriceOracle.
        // ============================================================
        let cost_usdt = tx_rec.cost_in_usdt.unwrap_or(0);
        let cost_usdt_f64 = cost_usdt as f64 / 1_000_000.0;

        debug!(
            sweep_id = %sweep_id,
            sweep_amount = sweep_amount,
            cost_in_usdt = cost_usdt,
            cost_usdt_display = cost_usdt_f64,
            "Sweep confirmed (Ledger mode: cost_in_usdt={:.6} USDT)",
            cost_usdt_f64
        );

        // Legacy safeguard removed: BillingService handles network isolation now.

        let result = self
            .db
            .transaction::<_, bool, sea_orm::DbErr>(|txn| {
                let network = network.clone();
                let from_address = from_address.clone();
                let sweep_id = sweep_id.clone();
                let session_id = session_id.clone();
                let exception_id = tx_rec.exception_id.clone();
                let sweep_type = tx_rec.operation_type.clone();
                let swept_token = swept_token.clone();

                Box::pin(async move {
                    // Update sweep transaction
                    let transition = OutboundTransactions::update_many()
                        .col_expr(
                            outbound_transactions::Column::State,
                            Expr::value(OutboundState::Confirmed),
                        )
                        .col_expr(
                            outbound_transactions::Column::ConfirmedAt,
                            Expr::cust("NOW()"),
                        )
                        .filter(outbound_transactions::Column::Id.eq(sweep_id.clone()))
                        .filter(outbound_transactions::Column::State.is_in([
                            OutboundState::Signed,
                            OutboundState::BroadcastUnknown,
                            OutboundState::Pending,
                        ]))
                        .exec(txn)
                        .await?;
                    if transition.rows_affected != 1 {
                        return Ok(false);
                    }

                    // Update address balance after sweep confirmation.
                    // Normal sweep: set status to Cooling and clear balance (full sweep).
                    // ManualTransfer: keep status as Detected (sweeper will later sweep
                    // remaining fee portion) but subtract the transferred amount.
                    if sweep_type != outbound_transactions::OutboundOperationType::ManualTransfer {
                        let mut addr_update = Addresses::update_many()
                            .col_expr(
                                addresses::Column::Status,
                                Expr::value(addresses::AddressStatus::Cooling),
                            )
                            .col_expr(addresses::Column::UpdatedAt, Expr::cust("NOW()"));

                        // Clear the correct balance column based on which token was swept
                        if swept_token == "USDC" {
                            addr_update = addr_update
                                .col_expr(addresses::Column::UsdcBalance, Expr::value(0i64));
                        } else {
                            addr_update = addr_update
                                .col_expr(addresses::Column::UsdtBalance, Expr::value(0i64));
                        }

                        addr_update
                            .filter(addresses::Column::Network.eq(network.clone()))
                            .filter(addresses::Column::Address.eq(from_address.clone()))
                            .exec(txn)
                            .await?;
                    } else {
                        // ManualTransfer: subtract transferred amount, keep address Detected.
                        // Fee portion remains on-chain; sweeper will collect it after
                        // exception is resolved and the exception guard lifts.
                        let balance_col = if swept_token == "USDC" {
                            addresses::Column::UsdcBalance
                        } else {
                            addresses::Column::UsdtBalance
                        };
                        Addresses::update_many()
                            .col_expr(
                                balance_col,
                                Expr::col(balance_col).sub(Expr::value(sweep_amount)),
                            )
                            .col_expr(addresses::Column::UpdatedAt, Expr::cust("NOW()"))
                            .filter(addresses::Column::Network.eq(network.clone()))
                            .filter(addresses::Column::Address.eq(from_address.clone()))
                            .exec(txn)
                            .await?;
                    }

                    // Update checkout_sessions.settlement_status to 'Settled' (AutoSweep only)
                    if let Some(sid) = session_id.clone() {
                        CheckoutSessions::update_many()
                            .col_expr(
                                checkout_sessions::Column::SettlementStatus,
                                Expr::value(SettlementStatus::Settled),
                            )
                            .col_expr(checkout_sessions::Column::UpdatedAt, Expr::cust("NOW()"))
                            .filter(checkout_sessions::Column::Id.eq(sid))
                            .exec(txn)
                            .await?;
                    }

                    // A sweep drains the full address balance, so settle every successful
                    // session associated with that address, including legacy records.
                    CheckoutSessions::update_many()
                        .col_expr(
                            checkout_sessions::Column::SettlementStatus,
                            Expr::value(SettlementStatus::Settled),
                        )
                        .col_expr(checkout_sessions::Column::UpdatedAt, Expr::cust("NOW()"))
                        .filter(checkout_sessions::Column::PayAddress.eq(from_address.clone()))
                        .filter(checkout_sessions::Column::Network.eq(network.clone()))
                        .filter(
                            checkout_sessions::Column::SettlementStatus
                                .eq(SettlementStatus::Unsettled),
                        )
                        .filter(checkout_sessions::Column::Status.is_in([
                            checkout_sessions::SessionStatus::Paid,
                            checkout_sessions::SessionStatus::Overpaid,
                        ]))
                        .exec(txn)
                        .await?;

                    // NEW: For ManualSweep/ManualTransfer, resolve the linked exception
                    if let Some(ex_id) = exception_id {
                        let resolution = match sweep_type {
                            outbound_transactions::OutboundOperationType::ManualSweep => {
                                payment_exceptions::Resolution::Swept
                            }
                            outbound_transactions::OutboundOperationType::ManualTransfer => {
                                payment_exceptions::Resolution::Transferred
                            }
                            _ => return Ok(true), // AutoSweep has no exception
                        };

                        payment_exceptions::Entity::update_many()
                            .col_expr(
                                payment_exceptions::Column::Status,
                                Expr::value(payment_exceptions::ExceptionStatus::Resolved),
                            )
                            .col_expr(
                                payment_exceptions::Column::Resolution,
                                Expr::value(Some(resolution)),
                            )
                            .col_expr(payment_exceptions::Column::ResolvedAt, Expr::cust("NOW()"))
                            .col_expr(payment_exceptions::Column::UpdatedAt, Expr::cust("NOW()"))
                            .filter(payment_exceptions::Column::Id.eq(ex_id))
                            .exec(txn)
                            .await?;
                    }

                    Ok(true)
                })
            })
            .await?;

        Ok(result)
    }

    async fn handle_sweep_failure(
        &self,
        tx_rec: &outbound_transactions::Model,
        terminal_state: OutboundState,
        reason: &str,
    ) -> Result<bool> {
        let network = tx_rec.network.clone();
        let from_address = tx_rec.from_address.clone();
        let sweep_id = tx_rec.id.clone();

        let result = self
            .db
            .transaction::<_, bool, sea_orm::DbErr>(|txn| {
                let network = network.clone();
                let from_address = from_address.clone();
                let sweep_id = sweep_id.clone();
                let session_id = tx_rec.session_id.clone();
                let exception_id = tx_rec.exception_id.clone();
                let sweep_type = tx_rec.operation_type.clone();
                let terminal_state = terminal_state.clone();
                let reason = reason.to_string();

                Box::pin(async move {
                    let transition = OutboundTransactions::update_many()
                        .col_expr(
                            outbound_transactions::Column::State,
                            Expr::value(terminal_state),
                        )
                        .col_expr(
                            outbound_transactions::Column::ErrorMessage,
                            Expr::value(Some(reason)),
                        )
                        .col_expr(
                            outbound_transactions::Column::ObservedAt,
                            Expr::cust("NOW()"),
                        )
                        .col_expr(
                            outbound_transactions::Column::UpdatedAt,
                            Expr::cust("NOW()"),
                        )
                        .filter(outbound_transactions::Column::Id.eq(&sweep_id))
                        .filter(outbound_transactions::Column::State.is_in([
                            OutboundState::Signed,
                            OutboundState::BroadcastUnknown,
                            OutboundState::Pending,
                        ]))
                        .exec(txn)
                        .await?;
                    if transition.rows_affected != 1 {
                        return Ok(false);
                    }

                    // Reset address to Detected for retry.
                    // SKIP for ManualTransfer: address status was never changed to Sweeping,
                    // so resetting would incorrectly overwrite the real status.
                    // Do NOT increment sweep_attempts here — it was already incremented
                    // in execute_sweep_logic when entering Sweeping state.
                    if sweep_type != outbound_transactions::OutboundOperationType::ManualTransfer {
                        Addresses::update_many()
                            .col_expr(
                                addresses::Column::Status,
                                Expr::value(addresses::AddressStatus::Detected),
                            )
                            .col_expr(addresses::Column::UpdatedAt, Expr::cust("NOW()"))
                            .filter(addresses::Column::Network.eq(network.clone()))
                            .filter(addresses::Column::Address.eq(from_address.clone()))
                            .exec(txn)
                            .await?;
                    }

                    // NEW: Update checkout_sessions.settlement_status to 'Failed'
                    // Guard: only revert if still in Settling (don't overwrite Settled from another path)
                    if let Some(sid) = session_id {
                        CheckoutSessions::update_many()
                            .col_expr(
                                checkout_sessions::Column::SettlementStatus,
                                Expr::value(SettlementStatus::Failed),
                            )
                            .col_expr(checkout_sessions::Column::UpdatedAt, Expr::cust("NOW()"))
                            .filter(checkout_sessions::Column::Id.eq(sid))
                            .filter(
                                checkout_sessions::Column::SettlementStatus
                                    .eq(SettlementStatus::Settling),
                            )
                            .exec(txn)
                            .await?;
                    }

                    // Rollback exception to Pending on ManualTransfer/ManualSweep failure.
                    // Without this, exception stays stuck in Processing forever.
                    if let Some(ex_id) = exception_id {
                        if sweep_type == outbound_transactions::OutboundOperationType::ManualTransfer
                            || sweep_type == outbound_transactions::OutboundOperationType::ManualSweep
                        {
                            payment_exceptions::Entity::update_many()
                                .col_expr(
                                    payment_exceptions::Column::Status,
                                    Expr::value(
                                        payment_exceptions::ExceptionStatus::Pending,
                                    ),
                                )
                                .col_expr(
                                    payment_exceptions::Column::Notes,
                                    Expr::value(Some(format!(
                                        "On-chain transaction failed (sweep {}). Rolled back to Pending for retry.",
                                        sweep_id
                                    ))),
                                )
                                .col_expr(
                                    payment_exceptions::Column::UpdatedAt,
                                    Expr::cust("NOW()"),
                                )
                                .filter(payment_exceptions::Column::Id.eq(ex_id))
                                .filter(
                                    payment_exceptions::Column::Status.eq(
                                        payment_exceptions::ExceptionStatus::Processing,
                                    ),
                                )
                                .exec(txn)
                                .await?;
                        }
                    }
                    Ok(true)
                })
            })
            .await?;

        Ok(result)
    }

    /// Recycle cooling addresses back to Idle pool once all token balances are zero.
    async fn recycle_cycle(&self) -> Result<()> {
        let cooling_period =
            chrono::Duration::seconds(self.sweep_config.cooling_period_seconds as i64);
        let threshold = chrono::Utc::now() - cooling_period;

        let query = addresses::Entity::update_many()
            .col_expr(
                addresses::Column::Status,
                sea_orm::sea_query::Expr::value(addresses::AddressStatus::Idle),
            )
            .filter(addresses::Column::Status.eq(addresses::AddressStatus::Cooling))
            .filter(addresses::Column::Network.eq(self.network.as_str()))
            .filter(addresses::Column::UpdatedAt.lt(threshold))
            .filter(addresses::Column::UsdtBalance.eq(0i64))
            .filter(addresses::Column::UsdcBalance.eq(0i64));

        let res = query.exec(&self.db).await?;

        if res.rows_affected > 0 {
            info!(
                count = res.rows_affected,
                "Recycled cooling addresses to Idle"
            );
        }

        Ok(())
    }

    /// Recycle expired Assigned addresses back to Idle pool.
    ///
    /// **P0 Fix: Address Pool Leak Prevention**
    ///
    /// Problem: When a session expires without payment, the address stays in `Assigned`
    /// state forever, causing the address pool to gradually deplete.
    ///
    /// Solution: Find addresses that:
    /// - Are in `Assigned` state
    /// - Have an expired session (24+ hours ago)
    /// - Have zero USDT balance (no funds to sweep)
    ///
    /// These addresses are safe to return to the `Idle` pool for reuse.
    async fn recycle_expired_assigned_cycle(&self) -> Result<()> {
        use crate::entity::{checkout_sessions, CheckoutSessions};

        // Only recycle sessions expired > 24 hours ago (grace period for late payments)
        let expiry_threshold = chrono::Utc::now() - chrono::Duration::hours(24);

        // Find expired sessions with their pay addresses (scoped to this sweeper's network)
        let expired_sessions = CheckoutSessions::find()
            .filter(checkout_sessions::Column::Status.eq(checkout_sessions::SessionStatus::Expired))
            .filter(checkout_sessions::Column::Network.eq(self.network.as_str()))
            .filter(checkout_sessions::Column::ExpiresAt.lt(expiry_threshold))
            .all(&self.db)
            .await?;

        if expired_sessions.is_empty() {
            return Ok(());
        }

        let mut recycled_count = 0u64;

        for session in expired_sessions {
            // Get the pay address for this session
            let pay_address = &session.pay_address;

            // Only recycle if:
            // 1. Address is still in Assigned state (hasn't received any payment)
            // 2. Both balances are 0 (no funds stuck — check USDT and USDC)
            let result = addresses::Entity::update_many()
                .col_expr(
                    addresses::Column::Status,
                    sea_orm::sea_query::Expr::value(addresses::AddressStatus::Idle),
                )
                .filter(addresses::Column::Address.eq(pay_address))
                .filter(addresses::Column::Status.eq(addresses::AddressStatus::Assigned))
                .filter(addresses::Column::UsdtBalance.eq(0i64))
                .filter(addresses::Column::UsdcBalance.eq(0i64))
                .exec(&self.db)
                .await?;

            recycled_count += result.rows_affected;
        }

        if recycled_count > 0 {
            info!(
                count = recycled_count,
                "Recycled expired Assigned addresses to Idle pool"
            );
        }

        Ok(())
    }

    // define_private_key method removed

    // transfer_bandwidth_trx and estimate_sweep_energy removed (moved to EnergyManager)
}
