use crate::config::Environment;
use crate::services::energy::interface::{EnergyReceipt, EnergyRentalProvider};
use crate::services::outbound::{
    BroadcastDisposition, OutboundTransactionStore, StoredSignedTransaction,
};
use crate::services::tron::interface::TronBroadcaster;
use anyhow::{anyhow, Result};
use sea_orm::{sea_query::Expr, ColumnTrait, EntityTrait, QueryFilter};
use std::sync::Arc;
use tokio::time::{sleep, Duration, Instant};
use tracing::{error, info, warn};

const MIN_ENERGY_RENTAL: u64 = 61_000;
const DEFAULT_USDT_ENERGY: u64 = 64_385; // Recipient has USDT
const DEFAULT_USDT_ENERGY_NO_BALANCE: u64 = 131_000; // Recipient has NO USDT

// TRON converts delegated SUN to energy with integer division. The observed
// EnergyLimit can therefore be one unit below the provider's requested amount.
const ENERGY_ROUNDING_TOLERANCE: i64 = 1;
const ENERGY_RENTAL_BUFFER: u64 = 100;
const BANDWIDTH_THRESHOLD: i64 = 345;
const TRX_BALANCE_THRESHOLD: u64 = 350_000; // 0.35 TRX (enough to burn 345 bandwidth)
const ACTIVATION_COST: u64 = 1_000; // 0.001 TRX (just activate the account)
const BANDWIDTH_FUND: u64 = 350_000; // 0.35 TRX (burn for 345 bandwidth when depleted)

fn available_bandwidth(resources: &crate::services::tron::interface::AccountResource) -> i64 {
    let free_bandwidth = resources
        .free_net_limit
        .saturating_sub(resources.free_net_used)
        .max(0);
    let staked_bandwidth = resources
        .net_limit
        .saturating_sub(resources.net_used)
        .max(0);
    free_bandwidth.saturating_add(staked_bandwidth)
}

fn available_energy(resources: &crate::services::tron::interface::AccountResource) -> i64 {
    resources
        .energy_limit
        .saturating_sub(resources.energy_used)
        .max(0)
}

fn has_required_energy(available: i64, required: i64) -> bool {
    available.saturating_add(ENERGY_ROUNDING_TOLERANCE) >= required
}

fn required_energy_rental(available: i64, required: i64) -> Option<u64> {
    if has_required_energy(available, required) {
        return None;
    }

    let needed = required.saturating_sub(available).max(0) as u64;
    Some(
        needed
            .saturating_add(ENERGY_RENTAL_BUFFER)
            .max(MIN_ENERGY_RENTAL),
    )
}

/// Result of ensure_resources: tracks what was spent.
#[derive(Debug)]
pub struct ResourceCost {
    /// TRX funding transaction hash (if bandwidth funding was needed)
    pub funding_tx_hash: Option<String>,
    /// Total TRX cost in SUN (bandwidth funding + energy rental)
    pub total_cost_sun: u64,
}

#[derive(Debug)]
struct ResourceStrategy {
    /// Need to transfer TRX amount (Sun)
    /// None means sufficient bandwidth/balance, no transfer needed
    fund_trx_amount: Option<u64>,

    /// Need to rent energy amount
    /// None means sufficient energy (or Sandbox burning TRX directly)
    rent_energy_amount: Option<u64>,

    /// Initial balance snapshot (for race-condition-free waiting)
    initial_trx_balance: u64,

    /// Energy that must remain available before the token transfer is signed.
    required_energy: i64,
}

pub struct EnergyManager {
    tron_client: Arc<dyn TronBroadcaster + Send + Sync>,
    energy_provider: Arc<dyn EnergyRentalProvider + Send + Sync>,
    environment: Environment,
    gas_sponsor_key: Option<Vec<u8>>,
    gas_sponsor_address: Option<String>,
    // Config: Bandwidth transfer amount (e.g. 0.35 TRX or 30 TRX)
    usdt_contract: String,
}

impl EnergyManager {
    pub fn new(
        tron_client: Arc<dyn TronBroadcaster + Send + Sync>,
        energy_provider: Arc<dyn EnergyRentalProvider + Send + Sync>,
        environment: Environment,
        gas_sponsor_key: Option<Vec<u8>>,
        gas_sponsor_address: Option<String>,
        usdt_contract: String,
    ) -> Self {
        Self {
            tron_client,
            energy_provider,
            environment,
            gas_sponsor_key,
            gas_sponsor_address,
            usdt_contract,
        }
    }

    /// Ensure that the target address has enough resources (Energy & Bandwidth)
    /// to perform a TRC20 transfer.
    ///
    /// # Logic
    /// Uses a "Read-Evaluate-Write" strategy:
    /// 1. Snapshot: Fetch account resources and balance in parallel.
    /// 2. Plan: Determine if TRX funding or Energy rental is needed.
    /// 3. Execute: Perform necessary actions (transfer TRX, rent energy).
    ///
    /// # Returns
    /// - `Ok(ResourceCost)` with funding tx hash and total TRX cost in SUN.
    /// - `Err` if something failed.
    pub async fn ensure_resources(
        &self,
        from_address: &str,
        balance_usdt: i64, // Used to decide energy delegation amount
        to_address: &str,  // Used to estimate energy
        parent_outbound_id: &str,
        outbound_store: &OutboundTransactionStore,
    ) -> Result<ResourceCost> {
        // Step 1: Snapshot & Plan (Fetch data in parallel, formulate strategy)
        let strategy = self
            .assess_requirements(from_address, balance_usdt, to_address)
            .await?;

        info!(
            "Resource strategy formulated for {}: {:?}",
            from_address, strategy
        );

        let mut funding_tx_hash = None;
        let mut total_cost_sun: u64 = 0;

        // Step 2: Execute TRX Funding (if needed)
        if let Some(amount) = strategy.fund_trx_amount {
            let tx_hash = self
                .execute_fund_bandwidth(
                    from_address,
                    amount,
                    strategy.initial_trx_balance, // Pass initial balance
                    parent_outbound_id,
                    outbound_store,
                )
                .await?;
            funding_tx_hash = Some(tx_hash);
            total_cost_sun += amount; // TRX sent from gas sponsor
        }

        // Step 3: Execute Energy Rental (if needed)
        // Note: If account was inactive, Step 2's wait logic ensures it's now active for rental
        if let Some(amount) = strategy.rent_energy_amount {
            info!("Delegating {} energy to {}", amount, from_address);
            let energy_outbound = outbound_store
                .create_child_attempt(
                    parent_outbound_id,
                    crate::entity::outbound_transactions::OutboundPurpose::EnergyFunding,
                    "external_energy_provider".to_string(),
                    from_address.to_string(),
                    i64::try_from(amount).map_err(|_| anyhow!("Energy amount exceeds i64"))?,
                    "ENERGY".to_string(),
                )
                .await?;
            let receipt: EnergyReceipt = match self
                .energy_provider
                .delegate_energy(from_address, amount)
                .await
            {
                Ok(receipt) => receipt,
                Err(error) => {
                    let _ = outbound_store
                        .mark_preparing_failed(&energy_outbound.id, error.to_string())
                        .await;
                    return Err(error);
                }
            };
            crate::entity::outbound_transactions::Entity::update_many()
                .col_expr(
                    crate::entity::outbound_transactions::Column::ProviderReference,
                    Expr::value(Some(receipt.order_id.clone())),
                )
                .col_expr(
                    crate::entity::outbound_transactions::Column::EnergyDelegateTxHash,
                    Expr::value(Some(receipt.trx_hash.clone())),
                )
                .filter(crate::entity::outbound_transactions::Column::Id.eq(&energy_outbound.id))
                .exec(outbound_store.db())
                .await?;
            if !outbound_store
                .adopt_executor_result(
                    &energy_outbound.id,
                    &receipt.trx_hash,
                    BroadcastDisposition::Accepted,
                )
                .await?
            {
                return Err(anyhow!(
                    "Energy funding journal state changed before handoff"
                ));
            }
            total_cost_sun += receipt.cost_sun.unsigned_abs();

            // Wait for on-chain resource limit sync
            self.wait_for_energy(from_address, strategy.required_energy)
                .await?;
            let confirmed = outbound_store
                .mark_state(
                    &energy_outbound.id,
                    crate::entity::outbound_transactions::OutboundState::Confirmed,
                    None,
                )
                .await?;
            if !confirmed {
                let current =
                    crate::entity::outbound_transactions::Entity::find_by_id(&energy_outbound.id)
                        .one(outbound_store.db())
                        .await?;
                if !matches!(
                    current.map(|row| row.state),
                    Some(crate::entity::outbound_transactions::OutboundState::Confirmed)
                ) {
                    return Err(anyhow!(
                        "Energy funding journal changed before confirmation"
                    ));
                }
            }
        }

        Ok(ResourceCost {
            funding_tx_hash,
            total_cost_sun,
        })
    }

    /// Core Logic: Assessment Phase
    async fn assess_requirements(
        &self,
        from_address: &str,
        balance_usdt: i64,
        to_address: &str,
    ) -> Result<ResourceStrategy> {
        // 1. Fetch Account Resources and Balance in parallel (Reduce IO wait)
        let (resources_result, balance_result) = tokio::join!(
            Box::pin(self.tron_client.get_account_resources(from_address)),
            Box::pin(self.tron_client.get_trx_balance(from_address))
        );

        // Fail-fast on network errors to prevent misdiagnosis of "inactive"
        let resources =
            resources_result.map_err(|e| anyhow!("Failed to fetch resources: {}", e))?;
        let trx_balance = balance_result.map_err(|e| anyhow!("Failed to fetch balance: {}", e))?;

        // Determine if inactive (0 balance AND no resources AND 0 free bandwidth)
        // CRITICAL: free_net_limit == 0 is the strongest indicator of a non-existent account.
        let is_inactive = trx_balance == 0
            && resources.free_net_limit == 0
            && resources.net_limit == 0
            && resources.energy_limit == 0;

        // --- Strategy A: Bandwidth/Activation ---
        let bandwidth_available = available_bandwidth(&resources);

        // Threshold for checking balance after funding (relaxed slightly for gas/fees)
        let _funding_threshold =
            |amount: u64, initial: u64| initial + (amount as f64 * 0.95) as u64;

        let fund_trx_amount = if self.environment == Environment::Sandbox {
            // Sandbox Logic: Burn TRX for everything
            let threshold = 20_000_000; // 20 TRX
            let target = 30_000_000; // 30 TRX
            if trx_balance < threshold {
                Some(target)
            } else {
                None
            }
        } else {
            // Production Logic
            if is_inactive {
                Some(ACTIVATION_COST)
            } else if bandwidth_available < BANDWIDTH_THRESHOLD {
                // 带宽不够，必须烧 TRX。检查余额够不够烧 345 bandwidth = 0.345 TRX？
                if trx_balance < TRX_BALANCE_THRESHOLD {
                    // 不够烧，补 0.35 TRX
                    Some(BANDWIDTH_FUND)
                } else {
                    None
                }
            } else {
                // 带宽充足，不需要充值
                None
            }
        };

        // --- Strategy B: Energy ---
        let mut rent_energy_amount = None;
        let required_energy;

        if self.environment != Environment::Sandbox {
            // Calculate remaining available energy
            let energy_remaining = available_energy(&resources);

            // Estimate required energy
            // If inactive, skip estimate_energy (it will fail), use default based on account state
            // If we know it's inactive (no resources/balance), it likely has no USDT either (Empty Wallet)
            let nominal_energy = if is_inactive {
                DEFAULT_USDT_ENERGY_NO_BALANCE
            } else {
                match self
                    .estimate_energy_usage(from_address, to_address, balance_usdt)
                    .await
                {
                    Ok(e) => e,
                    Err(e) => {
                        warn!(
                            "Estimate energy failed, checking recipient balance. Err: {}",
                            e
                        );
                        // Fallback: Check if recipient already has USDT
                        match self.tron_client.get_usdt_balance(to_address).await {
                            Ok(bal) if bal > 0 => {
                                info!(address=%to_address, "Recipient has USDT, using 65k energy fallback");
                                DEFAULT_USDT_ENERGY
                            }
                            _ => {
                                info!(address=%to_address, "Recipient has no USDT (or check failed), using 131k energy fallback");
                                DEFAULT_USDT_ENERGY_NO_BALANCE
                            }
                        }
                    }
                }
            };

            let energy_target = nominal_energy;
            required_energy =
                i64::try_from(energy_target).map_err(|_| anyhow!("Energy target exceeds i64"))?;

            if let Some(amount_to_rent) = required_energy_rental(energy_remaining, required_energy)
            {
                rent_energy_amount = Some(amount_to_rent);
            }
        } else {
            required_energy = 0;
        }

        Ok(ResourceStrategy {
            fund_trx_amount,
            rent_energy_amount,
            initial_trx_balance: trx_balance, // Pass snapshot
            required_energy,
        })
    }

    /// Execution Logic: Fund TRX and Wait
    ///
    /// `initial_balance` is passed from Snapshot phase to avoid race conditions.
    async fn execute_fund_bandwidth(
        &self,
        from_address: &str,
        amount: u64,
        initial_balance: u64, // Pass initial balance from snapshot
        parent_outbound_id: &str,
        outbound_store: &OutboundTransactionStore,
    ) -> Result<String> {
        info!(address=%from_address, amount=%amount, "Executing TRX funding...");

        if let (Some(key), Some(sponsor)) = (&self.gas_sponsor_key, &self.gas_sponsor_address) {
            let funding_outbound = outbound_store
                .create_child_attempt(
                    parent_outbound_id,
                    crate::entity::outbound_transactions::OutboundPurpose::GasFunding,
                    sponsor.clone(),
                    from_address.to_string(),
                    i64::try_from(amount).map_err(|_| anyhow!("TRX funding exceeds i64"))?,
                    "TRX".to_string(),
                )
                .await?;
            // Build
            let unsigned = match self
                .tron_client
                .build_trx_transfer(sponsor, from_address, amount)
                .await
            {
                Ok(unsigned) => unsigned,
                Err(error) => {
                    let _ = outbound_store
                        .mark_preparing_failed(&funding_outbound.id, error.to_string())
                        .await;
                    return Err(error);
                }
            };

            // Sign
            let signed_tx = match self.tron_client.sign_transaction(&unsigned, key) {
                Ok(signed) => signed,
                Err(error) => {
                    let _ = outbound_store
                        .mark_preparing_failed(&funding_outbound.id, error.to_string())
                        .await;
                    return Err(error);
                }
            };
            let local_tx_hash = signed_tx.tx_id.clone();
            outbound_store
                .record_signed(
                    &funding_outbound.id,
                    &StoredSignedTransaction::Tron {
                        tx_hash: local_tx_hash.clone(),
                        raw_data_hex: hex::encode(&signed_tx.raw_data),
                        signature_hex: hex::encode(&signed_tx.signature),
                        raw_data_json: signed_tx.raw_data_json.clone(),
                        expiration_ms: signed_tx.expiration,
                    },
                )
                .await?;

            // Broadcast
            let broadcast = self.tron_client.broadcast(&signed_tx).await;
            let (disposition, error) = match broadcast {
                Ok(result)
                    if result.success
                        && (result.tx_hash.is_empty() || result.tx_hash == local_tx_hash) =>
                {
                    (BroadcastDisposition::Accepted, None)
                }
                Ok(result) => (
                    BroadcastDisposition::Unknown,
                    Some(
                        result
                            .message
                            .unwrap_or_else(|| "TRX funding acknowledgement mismatch".into()),
                    ),
                ),
                Err(error) => (
                    BroadcastDisposition::Unknown,
                    Some(format!("TRX funding broadcast was ambiguous: {error}")),
                ),
            };
            let _ = outbound_store
                .mark_broadcast(&funding_outbound.id, disposition, error)
                .await?;

            let tx_hash = local_tx_hash;
            info!(tx_hash=%tx_hash, "Funding broadcasted, waiting for arrival...");

            // Wait Logic
            let start = Instant::now();

            // Use passed initial_balance to avoid race condition where funds arrive before we query
            // If inactive, initial_balance should be 0 (verified in snapshot)
            let threshold_balance = initial_balance + (amount as f64 * 0.95) as u64;

            loop {
                if start.elapsed().as_secs() > 60 {
                    error!(address=%from_address, "Timeout waiting for TRX funding confirmation (60s exceeded)");
                    return Err(anyhow!("Timeout waiting for funding confirmation"));
                }
                sleep(Duration::from_secs(3)).await;

                let current: u64 = self
                    .tron_client
                    .get_trx_balance(from_address)
                    .await
                    .unwrap_or(0);

                // Success conditions:
                // 1. Balance reached threshold (initial + ~amount)
                if current >= threshold_balance {
                    let confirmed = outbound_store
                        .mark_state(
                            &funding_outbound.id,
                            crate::entity::outbound_transactions::OutboundState::Confirmed,
                            None,
                        )
                        .await?;
                    if !confirmed {
                        let current = crate::entity::outbound_transactions::Entity::find_by_id(
                            &funding_outbound.id,
                        )
                        .one(outbound_store.db())
                        .await?;
                        if !matches!(
                            current.map(|row| row.state),
                            Some(crate::entity::outbound_transactions::OutboundState::Confirmed)
                        ) {
                            return Err(anyhow!("TRX funding journal changed before confirmation"));
                        }
                    }
                    info!(address=%from_address, old=%initial_balance, new=%current, "Funding confirmed on-chain");
                    break;
                }
            }

            Ok(tx_hash)
        } else {
            Err(anyhow!("No sponsor wallet configured"))
        }
    }

    /*
        /// Execution Logic: Rent Energy
        async fn execute_rent_energy(&self, address: &str, amount: u64) -> Result<()> {
            info!(address=%address, amount=%amount, "Executing energy rental...");
            self.energy_provider.delegate_energy(address, amount).await?;
            // Simple wait for propagation
            sleep(Duration::from_secs(2)).await;
            Ok(())
        }
    */

    /// Helper: Wait for energy delegation to sync on-chain
    async fn wait_for_energy(&self, from_address: &str, required_energy: i64) -> Result<()> {
        let start = Instant::now();
        loop {
            if start.elapsed().as_secs() > 45 {
                error!(address=%from_address, required=%required_energy, "Timeout waiting for energy delegation sync (45s exceeded)");
                return Err(anyhow!(
                    "Timeout waiting for energy delegation confirmation"
                ));
            }

            let resources = self
                .tron_client
                .get_account_resources(from_address)
                .await
                .map_err(|e| anyhow!("Check resources failed: {}", e))?;

            let energy_available = available_energy(&resources);
            if has_required_energy(energy_available, required_energy) {
                info!(address=%from_address, available=%energy_available, required=%required_energy, "Energy delegation synced on-chain");
                break;
            }

            sleep(Duration::from_secs(2)).await;
        }
        Ok(())
    }

    // Extracted from SweeperService logic
    async fn estimate_energy_usage(
        &self,
        from_address: &str,
        to_address: &str,
        amount: i64,
    ) -> Result<u64> {
        use alloy_primitives::{Address as EvmAddress, U256};
        use alloy_sol_types::{sol, SolCall};

        // Construct TRC20 transfer calldata called 'transfer'
        sol! {
            function transfer(address to, uint256 amount) external returns (bool);
        }

        // Convert Tron address to EVM address for ABI encoding
        let to_evm_bytes = crate::services::tron::address::tron_to_evm(to_address)?;
        let to_evm = EvmAddress::from_slice(&to_evm_bytes);

        // Build ABI-encoded parameter (without selector)
        let call = transferCall {
            to: to_evm,
            amount: U256::from(amount),
        };
        let encoded = call.abi_encode();
        // Skip selector (first 4 bytes)
        let param_hex = hex::encode(&encoded[4..]);

        let energy = self
            .tron_client
            .estimate_energy(
                from_address,
                &self.usdt_contract,
                "transfer(address,uint256)",
                &param_hex,
            )
            .await?;

        Ok(energy as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        available_bandwidth, available_energy, has_required_energy, required_energy_rental,
        ENERGY_RENTAL_BUFFER, MIN_ENERGY_RENTAL,
    };
    use crate::services::tron::interface::AccountResource;

    #[test]
    fn available_bandwidth_subtracts_free_and_staked_usage_separately() {
        let resources = AccountResource {
            free_net_used: 400,
            free_net_limit: 600,
            net_used: 100,
            net_limit: 300,
            ..Default::default()
        };

        assert_eq!(available_bandwidth(&resources), 400);
    }

    #[test]
    fn available_bandwidth_never_goes_negative() {
        let resources = AccountResource {
            free_net_used: 700,
            free_net_limit: 600,
            net_used: 400,
            net_limit: 300,
            ..Default::default()
        };

        assert_eq!(available_bandwidth(&resources), 0);
    }

    #[test]
    fn available_energy_subtracts_already_consumed_energy() {
        let resources = AccountResource {
            energy_limit: 64_384,
            energy_used: 384,
            ..Default::default()
        };

        assert_eq!(available_energy(&resources), 64_000);
    }

    #[test]
    fn one_unit_tron_rounding_gap_is_accepted() {
        assert!(has_required_energy(64_384, 64_385));
        assert_eq!(required_energy_rental(64_384, 64_385), None);
    }

    #[test]
    fn larger_energy_shortfall_still_requires_rental() {
        assert!(!has_required_energy(64_383, 64_385));
        assert_eq!(
            required_energy_rental(64_383, 64_385),
            Some(MIN_ENERGY_RENTAL)
        );
    }

    #[test]
    fn new_rentals_include_rounding_buffer() {
        assert_eq!(
            required_energy_rental(0, 64_385),
            Some(64_385 + ENERGY_RENTAL_BUFFER)
        );
    }
}
