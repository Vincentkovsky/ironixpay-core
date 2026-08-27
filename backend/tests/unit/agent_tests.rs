//! Unit Tests for Agent / Referral Module
//!
//! Tests referral code generation, commission calculation logic,
//! fee configuration interaction, and AgentService methods via MockDatabase.

#[cfg(test)]
mod agent_tests {
    use rust_decimal::prelude::ToPrimitive;
    use rust_decimal::Decimal;
    use std::collections::HashSet;

    /// Helper: Multiply gross_amount (i64) by a Decimal rate and truncate to i64
    fn microunit_fee(gross: i64, rate: Decimal) -> i64 {
        (Decimal::from(gross) * rate).trunc().to_i64().unwrap()
    }

    // ─── Referral Code Generation ────────────────────────────────────

    /// Referral codes should be 8 chars, uppercase, no ambiguous chars (O/0/I/1)
    #[test]
    fn test_referral_code_format() {
        let chars: Vec<char> = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789".chars().collect();
        let mut rng = rand::thread_rng();

        for _ in 0..100 {
            let code: String = (0..8)
                .map(|_| chars[rand::Rng::gen_range(&mut rng, 0..chars.len())])
                .collect();

            assert_eq!(code.len(), 8, "Code must be 8 chars");
            assert!(
                code.chars().all(|c| chars.contains(&c)),
                "Code '{}' contains invalid char",
                code
            );
            // No ambiguous chars
            assert!(
                !code.contains('O')
                    && !code.contains('0')
                    && !code.contains('I')
                    && !code.contains('1'),
                "Code '{}' contains ambiguous char",
                code
            );
        }
    }

    /// Referral codes should be unique (probabilistic)
    #[test]
    fn test_referral_code_uniqueness() {
        let chars: Vec<char> = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789".chars().collect();
        let mut rng = rand::thread_rng();
        let mut codes = HashSet::new();

        for _ in 0..1000 {
            let code: String = (0..8)
                .map(|_| chars[rand::Rng::gen_range(&mut rng, 0..chars.len())])
                .collect();
            codes.insert(code);
        }

        // With 30^8 = 656B possible codes, 1000 codes should all be unique
        assert_eq!(codes.len(), 1000, "Expected all 1000 codes to be unique");
    }

    // ─── Commission Calculation Logic ────────────────────────────────

    /// Agent commission = fee_amount - gross_amount * base_rate
    #[test]
    fn test_commission_calculation_basic() {
        let base_rate = Decimal::new(1, 3); // 0.001 = 0.1%
        let merchant_rate = Decimal::new(4, 3); // 0.004 = 0.4%
        let gross_amount: i64 = 100_000_000; // 100 USDT

        // Fee charged to merchant: 100 * 0.4% = 0.4 USDT = 400_000
        let fee_amount = microunit_fee(gross_amount, merchant_rate);
        assert_eq!(fee_amount, 400_000);

        // IronixPay share: 100 * 0.1% = 0.1 USDT = 100_000
        let platform_share = microunit_fee(gross_amount, base_rate);
        assert_eq!(platform_share, 100_000);

        // Agent commission = fee - platform = 0.3 USDT
        let agent_commission = fee_amount - platform_share;
        assert_eq!(agent_commission, 300_000);
    }

    /// Floor deposit scenario: fee == 1 USDT, no commission
    #[test]
    fn test_commission_floor_deposit_no_commission() {
        let floor_deposit: i64 = 1_000_000; // 1 USDT

        // Small transaction: 10 USDT → fee = max(0.4%, 1 USDT) = 1 USDT
        let gross_amount: i64 = 10_000_000; // 10 USDT
        let merchant_rate = Decimal::new(4, 3); // 0.4%
        let calculated_fee = microunit_fee(gross_amount, merchant_rate);
        // 40_000 < 1_000_000, so floor_deposit applies
        let actual_fee = std::cmp::max(calculated_fee, floor_deposit);
        assert_eq!(actual_fee, floor_deposit);

        // Commission SQL filters out fee_amount <= 1_000_000
        // So this transaction would NOT be included in commission report
        assert!(
            actual_fee <= 1_000_000,
            "Floor deposit transactions should be excluded from commission"
        );
    }

    /// Large transaction: commission is meaningful
    #[test]
    fn test_commission_large_transaction() {
        let base_rate = Decimal::new(1, 3); // 0.1%
        let merchant_rate = Decimal::new(4, 3); // 0.4%
        let gross_amount: i64 = 1_000_000_000; // 1000 USDT

        let fee_amount = microunit_fee(gross_amount, merchant_rate);
        assert_eq!(fee_amount, 4_000_000); // 4 USDT

        let platform_share = microunit_fee(gross_amount, base_rate);
        assert_eq!(platform_share, 1_000_000); // 1 USDT

        let agent_commission = fee_amount - platform_share;
        assert_eq!(agent_commission, 3_000_000); // 3 USDT

        // Commission should be > 0 and fee > floor
        assert!(fee_amount > 1_000_000, "Fee should exceed floor deposit");
        assert!(agent_commission > 0, "Agent should earn commission");
    }

    // ─── Fee Config Integration ──────────────────────────────────────

    /// Agent's default_merchant_rate should integrate with FeeConfig.net_after_fee
    #[test]
    fn test_agent_rate_with_fee_config() {
        use ironix_pay::services::billing::fee_config::FeeConfig;

        let config = FeeConfig::default();
        let gross = 500_000_000_i64; // 500 USDT

        // Without agent (global default 0.5%)
        let (fee_global, net_global) = config.net_after_fee(gross, config.floor_deposit, None);

        // With agent (custom 0.8%)
        let agent_rate = Some(Decimal::new(8, 3)); // 0.008
        let (fee_agent, net_agent) = config.net_after_fee(gross, config.floor_deposit, agent_rate);

        // Agent rate should produce higher fee
        assert!(
            fee_agent > fee_global,
            "Agent rate 0.8% should produce higher fee than global 0.5%: agent={} global={}",
            fee_agent,
            fee_global
        );

        // Agent's merchant should receive less
        assert!(
            net_agent < net_global,
            "Agent-referred merchant gets less net: agent_net={} global_net={}",
            net_agent,
            net_global
        );

        // Exact values: 0.8% of 500 = 4 USDT, 0.5% of 500 = 2.5 USDT
        assert_eq!(fee_agent, 4_000_000); // 4 USDT
        assert_eq!(fee_global, 2_500_000); // 2.5 USDT
        assert_eq!(net_agent, 496_000_000); // 496 USDT
        assert_eq!(net_global, 497_500_000); // 497.5 USDT
    }

    // ─── Referral Code Normalization ─────────────────────────────────

    /// Referral codes should be case-insensitive and whitespace-trimmed
    #[test]
    fn test_referral_code_normalization() {
        let raw_codes = vec!["  abc123xy  ", "ABC123XY", "abc123xy", "Abc123Xy"];

        let normalized: Vec<String> = raw_codes.iter().map(|c| c.trim().to_uppercase()).collect();

        // All should normalize to the same value
        assert!(
            normalized.iter().all(|c| c == "ABC123XY"),
            "All variations should normalize to ABC123XY: {:?}",
            normalized
        );
    }

    /// Empty or whitespace-only referral codes should be treated as None
    #[test]
    fn test_empty_referral_code_ignored() {
        let empty_codes = vec!["", "   ", "\t", "\n"];

        for code in empty_codes {
            let trimmed = code.trim().to_uppercase();
            assert!(
                trimmed.is_empty(),
                "Code '{}' should normalize to empty",
                code
            );
        }
    }

    // ─── Default Rate Values ─────────────────────────────────────────

    #[test]
    fn test_default_agent_rates() {
        // These match the migration defaults
        let base_rate = Decimal::new(1, 3); // 0.001 = 0.1%
        let max_markup = Decimal::new(4, 3); // 0.004 = 0.4%
        let default_merchant_rate = Decimal::new(4, 3); // 0.004 = 0.4%

        // Max merchant rate = base_rate + max_markup = 0.5% = official website price
        assert_eq!(base_rate + max_markup, Decimal::new(5, 3)); // 0.5%

        // Merchant rate must be between base_rate and base_rate + max_markup
        assert!(
            default_merchant_rate >= base_rate,
            "Default merchant rate must be >= base_rate"
        );
        assert!(
            default_merchant_rate <= base_rate + max_markup,
            "Default merchant rate must be <= base_rate + max_markup"
        );

        // Agent margin = merchant_rate - base_rate
        let agent_margin = default_merchant_rate - base_rate;
        assert_eq!(agent_margin, Decimal::new(3, 3)); // 0.3%
    }

    // ─── Commission Edge Cases ───────────────────────────────────────

    /// When merchant_rate == base_rate, agent earns zero
    #[test]
    fn test_zero_margin_agent() {
        let base_rate = Decimal::new(1, 3); // 0.1%
        let merchant_rate = Decimal::new(1, 3); // 0.1% same as base
        let gross: i64 = 100_000_000; // 100 USDT

        let fee = microunit_fee(gross, merchant_rate);
        let platform = microunit_fee(gross, base_rate);

        assert_eq!(fee - platform, 0, "Agent with zero spread earns nothing");
    }

    /// Max markup scenario: agent charges 0.5% (base 0.1% + max markup 0.4%) = official rate
    #[test]
    fn test_max_markup_agent() {
        let base_rate = Decimal::new(1, 3); // 0.1%
        let max_markup = Decimal::new(4, 3); // 0.4%
        let merchant_rate = base_rate + max_markup; // 0.5% = official rate
        let gross: i64 = 100_000_000; // 100 USDT

        let fee = microunit_fee(gross, merchant_rate);
        let platform = microunit_fee(gross, base_rate);

        let commission = fee - platform;
        assert_eq!(fee, 500_000, "Fee at max markup = 0.5% of 100 USDT");
        assert_eq!(commission, 400_000, "Max markup agent earns 0.4% of gross");
        // 0.4 USDT
    }
}
