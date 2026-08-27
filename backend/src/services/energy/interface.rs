use anyhow::Result;
use async_trait::async_trait;

/// Energy Rental Provider Trait
///
/// Abstracts the external energy rental API for paying gas fees
/// on behalf of addresses that hold no TRX.
#[async_trait]
pub trait EnergyRentalProvider: Send + Sync {
    /// Delegate energy to a target address for sweeping.
    ///
    /// This is the core of the energy arbitrage:
    /// - Rent cheap energy from external pool
    /// - Delegate to the temp address before sweep broadcast
    ///
    /// # Arguments
    /// * `target_address` - The temp address executing the sweep
    /// * `energy_amount` - Amount of energy to delegate (65k or 131k depending on recipient balance)
    ///
    /// # Returns
    /// * `Ok(EnergyReceipt)` - Energy delegated successfully
    /// * `Err` - Delegation failed (caller should SKIP sweep to protect profit)
    async fn delegate_energy(
        &self,
        target_address: &str,
        energy_amount: u64,
    ) -> Result<EnergyReceipt>;
}

#[derive(Debug, Clone)]
pub struct EnergyReceipt {
    pub order_id: String,
    pub trx_hash: String,
    pub energy_amount: u64,
    pub cost_sun: i64, // Actual cost to platform (for billing)
    pub expires_at: i64,
}

#[derive(Debug, Default, Clone)]
pub struct DummyEnergyProvider;

#[async_trait]
impl EnergyRentalProvider for DummyEnergyProvider {
    async fn delegate_energy(
        &self,
        _target_address: &str,
        energy_amount: u64,
    ) -> Result<EnergyReceipt> {
        // Dummy implementation always returns a fake receipt
        Ok(EnergyReceipt {
            order_id: "dummy_order".to_string(),
            trx_hash: "dummy_hash".to_string(),
            energy_amount,
            cost_sun: 0,
            expires_at: 0,
        })
    }
}
