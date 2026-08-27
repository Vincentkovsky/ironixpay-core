//! Fee Configuration
//!
//! Shared fee configuration used by SweeperService, PayoutService, and ResolutionService.
//! Extracted from SweeperConfig to avoid cross-service dependency.
//!
//! Uses `rust_decimal::Decimal` for percentage to avoid f64 precision loss
//! on large amounts (> 2^53 microunits ≈ 9M USDT).

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

/// Platform fee configuration (No-Loss Fee Schedule)
///
/// All fees use the formula: `fee = max(floor, amount * percentage)`
/// Different floors ensure the platform never loses money on any transaction type.
///
/// ## Precision
/// `fee_percentage` is stored as `Decimal` to avoid f64 precision loss.
/// All arithmetic happens in `Decimal` space; the result is truncated (floor)
/// back to `i64` microunits.
#[derive(Clone, Debug)]
pub struct FeeConfig {
    /// Fee as a decimal fraction (default: 0.005 = 0.5%).
    /// Uses `Decimal` for lossless representation — no floating-point rounding.
    pub fee_percentage: Decimal,
    /// Minimum fee for deposits/sweep (USDT microunits, default: 1_000_000 = 1 USDT)
    pub floor_deposit: i64,
    /// Default outbound fee for withdrawals/payouts (USDT microunits, default: 1_500_000 = 1.5 USDT).
    /// Per-chain overrides in chains.toml take precedence via `outbound_fee()` method.
    pub flat_payout_fee: i64,
    /// Minimum fee for refunds (USDT microunits, default: 1_500_000 = 1.5 USDT)
    /// Rationale: worst-case TRC20 transfer ≈ 1.2 USDT gas + 25% safety margin.
    /// Adjust if energy market changes significantly.
    pub floor_refund: i64,
}

impl Default for FeeConfig {
    fn default() -> Self {
        Self {
            // Decimal::new(5, 3) = 0.005 exactly — no floating-point error
            fee_percentage: Decimal::new(5, 3), // 0.5%
            floor_deposit: 1_000_000,           // 1.0 USDT
            flat_payout_fee: 1_500_000,         // 1.5 USDT flat
            floor_refund: 1_500_000,            // 1.5 USDT
        }
    }
}

impl FeeConfig {
    /// Calculate fee: `max(floor, truncate(amount * percentage))`
    ///
    /// If `custom_pct` is provided, it overrides the global `fee_percentage`.
    /// Multiplication is done in `Decimal` to preserve precision.
    /// Result is truncated (rounded toward zero) to i64 microunits.
    pub fn estimate_fee(&self, amount: i64, floor: i64, custom_pct: Option<Decimal>) -> i64 {
        let pct = custom_pct.unwrap_or(self.fee_percentage);
        let amount_dec = Decimal::from(amount);
        let percentage_fee_dec = amount_dec * pct;
        // Truncate toward zero (floor for positive values).
        // `to_i64()` returns None only if the value overflows i64, which
        // cannot happen here: amount fits in i64 and percentage < 1.
        let percentage_fee = percentage_fee_dec.trunc().to_i64().unwrap_or(0);
        std::cmp::max(floor, percentage_fee)
    }

    /// Calculate deposit fee (uses floor_deposit, global percentage)
    pub fn deposit_fee(&self, amount: i64) -> i64 {
        self.estimate_fee(amount, self.floor_deposit, None)
    }

    /// Calculate deposit fee with optional per-chain floor override.
    /// Falls back to global `floor_deposit` when `chain_floor` is `None`.
    pub fn deposit_fee_for_chain(&self, amount: i64, chain_floor: Option<i64>) -> i64 {
        let floor = chain_floor.unwrap_or(self.floor_deposit);
        self.estimate_fee(amount, floor, None)
    }

    /// Like `net_after_fee` but with optional per-chain floor override.
    pub fn net_after_fee_for_chain(
        &self,
        amount: i64,
        chain_floor: Option<i64>,
        custom_pct: Option<Decimal>,
    ) -> (i64, i64) {
        let floor = chain_floor.unwrap_or(self.floor_deposit);
        self.net_after_fee(amount, floor, custom_pct)
    }

    /// Calculate outbound fee for withdrawal/payout (flat fee, not percentage-based).
    ///
    /// `chain_outbound_fee`: per-chain override from chains.toml.
    /// Falls back to `flat_payout_fee` if not set.
    pub fn outbound_fee(&self, _amount: i64, chain_outbound_fee: Option<i64>) -> i64 {
        chain_outbound_fee.unwrap_or(self.flat_payout_fee)
    }

    /// Calculate refund fee (uses floor_refund, global percentage — refund fees are cost-based)
    pub fn refund_fee(&self, amount: i64) -> i64 {
        self.estimate_fee(amount, self.floor_refund, None)
    }

    /// Calculate net amount after fee, capped so fee never exceeds amount.
    /// Returns (actual_fee, net_amount). net_amount may be 0 for dust deposits.
    ///
    /// If `custom_pct` is provided, it overrides the global `fee_percentage`.
    pub fn net_after_fee(
        &self,
        amount: i64,
        floor: i64,
        custom_pct: Option<Decimal>,
    ) -> (i64, i64) {
        let fee = self.estimate_fee(amount, floor, custom_pct);
        let actual_fee = std::cmp::min(fee, amount);
        let net = amount - actual_fee;
        (actual_fee, net)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_pct_overrides_global() {
        let config = FeeConfig::default(); // 0.5%
        let amount = 100_000_000; // 100 USDT
        let floor = 1_000_000; // 1 USDT

        // Custom 0.1%: 0.1% of 100 USDT = 100_000 microunits, but floor is 1_000_000
        let custom = Some(Decimal::new(1, 3)); // 0.001
        let fee = config.estimate_fee(amount, floor, custom);
        // 100_000 < 1_000_000 so floor wins
        assert_eq!(fee, 1_000_000);
    }

    #[test]
    fn test_custom_pct_overrides_global_large_amount() {
        let config = FeeConfig::default(); // 0.5%
        let amount = 500_000_000; // 500 USDT
        let floor = 1_000_000; // 1 USDT

        // Global: 0.5% of 500 = 2.5 USDT
        let global_fee = config.estimate_fee(amount, floor, None);
        assert_eq!(global_fee, 2_500_000);

        // Custom 1%: 1% of 500 = 5 USDT
        let custom_fee = config.estimate_fee(amount, floor, Some(Decimal::new(1, 2)));
        assert_eq!(custom_fee, 5_000_000);

        // Custom 1% > global 0.5% for this amount
        assert!(custom_fee > global_fee);
    }

    #[test]
    fn test_none_uses_global_default() {
        let config = FeeConfig::default(); // 0.5%
        let amount = 200_000_000; // 200 USDT
        let floor = 1_000_000;

        let fee_none = config.estimate_fee(amount, floor, None);
        let fee_explicit = config.estimate_fee(amount, floor, Some(Decimal::new(5, 3)));
        assert_eq!(fee_none, fee_explicit); // Both should be 0.5% = 1 USDT
        assert_eq!(fee_none, 1_000_000);
    }

    #[test]
    fn test_zero_percent_uses_floor() {
        let config = FeeConfig::default();
        let amount = 500_000_000; // 500 USDT
        let floor = 1_000_000; // 1 USDT

        // 0% fee → floor kicks in
        let fee = config.estimate_fee(amount, floor, Some(Decimal::ZERO));
        assert_eq!(fee, 1_000_000); // floor = 1 USDT
    }

    #[test]
    fn test_net_after_fee_with_custom_pct() {
        let config = FeeConfig::default();
        let amount = 500_000_000; // 500 USDT
        let floor = 1_000_000;

        // Global 0.5%: fee = 2.5 USDT, net = 497.5 USDT
        let (fee_g, net_g) = config.net_after_fee(amount, floor, None);
        assert_eq!(fee_g, 2_500_000);
        assert_eq!(net_g, 497_500_000);

        // Custom 1%: fee = 5 USDT, net = 495 USDT
        let (fee_c, net_c) = config.net_after_fee(amount, floor, Some(Decimal::new(1, 2)));
        assert_eq!(fee_c, 5_000_000);
        assert_eq!(net_c, 495_000_000);
    }
}
