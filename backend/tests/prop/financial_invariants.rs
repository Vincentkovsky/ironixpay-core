use ironix_pay::services::billing::fee_config::FeeConfig;
use proptest::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

proptest! {
    #[test]
    fn test_fee_always_at_least_floor(
        floor in 0..1_000_000_000i64,
        amount in 0..10_000_000_000i64,
        pct in 0.0..1.0f64
    ) {
        let pct_dec = Decimal::try_from(pct).unwrap_or_default();
        let config = FeeConfig {
            fee_percentage: pct_dec,
            floor_deposit: floor,
            ..Default::default()
        };

        let fee = config.estimate_fee(amount, floor, None);
        prop_assert!(fee >= floor);
    }

    #[test]
    fn test_fee_never_exceeds_amount_if_amount_greater_than_floor(
        floor in 0..1_000_000i64,
        amount in 0..100_000_000_000i64,
        pct in 0.0..0.5f64 // Realistic percentage < 50%
    ) {
        // Condition: amount > floor (otherwise fee = floor > amount is valid behavior for minimums)
        if amount > floor {
             let pct_dec = Decimal::try_from(pct).unwrap_or_default();
             let config = FeeConfig {
                fee_percentage: pct_dec,
                floor_deposit: floor,
                ..Default::default()
            };
            let fee = config.estimate_fee(amount, floor, None);

            // Calculate expected using Decimal (same logic as FeeConfig)
            let pct_fee = (Decimal::from(amount) * pct_dec).trunc().to_i64().unwrap_or(0);
            let expected = std::cmp::max(floor, pct_fee);
            prop_assert_eq!(fee, expected);
        }
    }

    #[test]
    fn test_fee_is_deterministic(
        floor in 0..1_000_000i64,
        amount in 0..100_000_000_000i64,
        pct in 0.0..1.0f64
    ) {
        let pct_dec = Decimal::try_from(pct).unwrap_or_default();
        let config = FeeConfig {
            fee_percentage: pct_dec,
            floor_deposit: floor,
            ..Default::default()
        };
        prop_assert_eq!(config.estimate_fee(amount, floor, None), config.estimate_fee(amount, floor, None));
    }

    #[test]
    fn test_net_after_fee_never_negative(
        floor in 0..1_000_000_000i64,
        amount in 0..10_000_000_000i64,
        pct in 0.0..1.0f64
    ) {
        let pct_dec = Decimal::try_from(pct).unwrap_or_default();
        let config = FeeConfig {
            fee_percentage: pct_dec,
            floor_deposit: floor,
            ..Default::default()
        };

        let (actual_fee, net) = config.net_after_fee(amount, floor, None);
        prop_assert!(net >= 0, "net should never be negative: amount={}, fee={}, net={}", amount, actual_fee, net);
        prop_assert!(actual_fee <= amount, "fee should not exceed amount: fee={}, amount={}", actual_fee, amount);
        prop_assert_eq!(net, amount - actual_fee);
    }
}
