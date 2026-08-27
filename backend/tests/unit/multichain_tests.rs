//! Multi-Chain Unit Tests
//!
//! Tests for EVM-specific logic that doesn't exist in TRON-only tests:
//! - Decimal normalization (BSC 18-dec, ETH 6-dec)
//! - EVM log parsing (parse_transfer_logs full flow)
//! - Address format validation (EIP-55 checksum)
//!
//! All tests are pure logic — no RPC, no DB, no Docker.

// ─── Decimal Normalization Tests ───────────────────────────────────────────
//
// Tests the normalization logic used in both:
// - EvmSweepExecutor::normalize_to_6_decimals (sweeper)
// - TransactionIndexer::normalize_amount (indexer)
//
// Since both are private methods, we replicate the exact same algorithm here.
// Any changes to the production code must be reflected in these tests.

/// Replicate EvmSweepExecutor::normalize_to_6_decimals logic for testing.
/// Converts from chain-native decimals to 6-decimal i64.
fn normalize_to_6_decimals(balance: alloy_primitives::U256, usdt_decimals: u8) -> i64 {
    if usdt_decimals <= 6 {
        balance.to_string().parse::<i64>().unwrap_or(0)
    } else {
        let shift = (usdt_decimals - 6) as u32;
        let divisor = alloy_primitives::U256::from(10u64).pow(alloy_primitives::U256::from(shift));
        let normalized = balance / divisor;
        normalized.to_string().parse::<i64>().unwrap_or(0)
    }
}

/// Replicate TransactionIndexer::normalize_amount logic for testing.
/// Converts amount string from chain precision to 6-decimal i64.
fn normalize_amount(amount_str: &str, usdt_decimals: u8) -> i64 {
    if usdt_decimals <= 6 {
        amount_str.parse::<i64>().unwrap_or(0)
    } else {
        let divisor_exp = (usdt_decimals - 6) as u32;
        let value = alloy_primitives::U256::from_str_radix(amount_str, 10)
            .unwrap_or(alloy_primitives::U256::ZERO);
        let divisor =
            alloy_primitives::U256::from(10u64).pow(alloy_primitives::U256::from(divisor_exp));
        let normalized = value / divisor;
        normalized.to_string().parse::<i64>().unwrap_or(0)
    }
}

#[cfg(test)]
mod decimal_normalization_tests {
    use super::*;
    use alloy_primitives::U256;

    // ── Sweeper: normalize_to_6_decimals ────────────────────────────────

    #[test]
    fn test_bsc_18dec_1_usdt() {
        // BSC: 1 USDT = 10^18
        let balance = U256::from(1_000_000_000_000_000_000u64);
        assert_eq!(normalize_to_6_decimals(balance, 18), 1_000_000);
    }

    #[test]
    fn test_bsc_18dec_50_usdt() {
        // BSC: 50 USDT = 50 * 10^18
        let balance = U256::from(50u64) * U256::from(10u64).pow(U256::from(18u64));
        assert_eq!(normalize_to_6_decimals(balance, 18), 50_000_000);
    }

    #[test]
    fn test_bsc_18dec_fractional_usdt() {
        // BSC: 0.5 USDT = 5 * 10^17
        let balance = U256::from(500_000_000_000_000_000u64);
        assert_eq!(normalize_to_6_decimals(balance, 18), 500_000);
    }

    #[test]
    fn test_bsc_18dec_sub_micro_usdt() {
        // BSC: 0.0000005 USDT = 5 * 10^11 (below 6-decimal precision)
        // Integer division: 500_000_000_000 / 10^12 = 0
        let balance = U256::from(500_000_000_000u64);
        assert_eq!(normalize_to_6_decimals(balance, 18), 0);
    }

    #[test]
    fn test_eth_6dec_1_usdt() {
        // ETH: 1 USDT = 10^6 (no normalization needed)
        let balance = U256::from(1_000_000u64);
        assert_eq!(normalize_to_6_decimals(balance, 6), 1_000_000);
    }

    #[test]
    fn test_eth_6dec_no_op() {
        // ETH: 5 USDT = 5_000_000 (identity pass-through)
        let balance = U256::from(5_000_000u64);
        assert_eq!(normalize_to_6_decimals(balance, 6), 5_000_000);
    }

    #[test]
    fn test_zero_balance() {
        assert_eq!(normalize_to_6_decimals(U256::ZERO, 18), 0);
        assert_eq!(normalize_to_6_decimals(U256::ZERO, 6), 0);
    }

    #[test]
    fn test_bsc_18dec_large_amount() {
        // 10,000 USDT on BSC = 10^4 * 10^18 = 10^22
        let balance = U256::from(10_000u64) * U256::from(10u64).pow(U256::from(18u64));
        assert_eq!(normalize_to_6_decimals(balance, 18), 10_000_000_000i64);
    }

    // ── Indexer: normalize_amount ───────────────────────────────────────

    #[test]
    fn test_indexer_tron_6dec() {
        // TRON: "1000000" = 1 USDT → direct parse
        assert_eq!(normalize_amount("1000000", 6), 1_000_000);
    }

    #[test]
    fn test_indexer_bsc_18dec() {
        // BSC: "1000000000000000000" = 1 USDT → divide by 10^12
        assert_eq!(normalize_amount("1000000000000000000", 18), 1_000_000);
    }

    #[test]
    fn test_indexer_bsc_50_usdt() {
        // BSC: 50 USDT = "50000000000000000000"
        assert_eq!(normalize_amount("50000000000000000000", 18), 50_000_000);
    }

    #[test]
    fn test_indexer_eth_5_usdt() {
        // ETH: 5 USDT = "5000000"
        assert_eq!(normalize_amount("5000000", 6), 5_000_000);
    }

    #[test]
    fn test_indexer_empty_string() {
        assert_eq!(normalize_amount("", 6), 0);
        assert_eq!(normalize_amount("", 18), 0);
    }

    #[test]
    fn test_indexer_invalid_string() {
        assert_eq!(normalize_amount("not_a_number", 6), 0);
        assert_eq!(normalize_amount("not_a_number", 18), 0);
    }

    #[test]
    fn test_indexer_zero() {
        assert_eq!(normalize_amount("0", 6), 0);
        assert_eq!(normalize_amount("0", 18), 0);
    }

    // ── Cross-validation: both paths produce identical results ──────────

    #[test]
    fn test_sweeper_indexer_consistency_bsc() {
        // Both normalization paths should produce the same result
        let bsc_raw = "25000000000000000000"; // 25 USDT on BSC
        let bsc_u256 = U256::from(25u64) * U256::from(10u64).pow(U256::from(18u64));

        let from_indexer = normalize_amount(bsc_raw, 18);
        let from_sweeper = normalize_to_6_decimals(bsc_u256, 18);

        assert_eq!(from_indexer, from_sweeper, "Indexer and Sweeper must agree");
        assert_eq!(from_indexer, 25_000_000); // 25 USDT in 6-decimal
    }

    #[test]
    fn test_sweeper_indexer_consistency_eth() {
        let eth_raw = "5000000"; // 5 USDT on ETH
        let eth_u256 = U256::from(5_000_000u64);

        let from_indexer = normalize_amount(eth_raw, 6);
        let from_sweeper = normalize_to_6_decimals(eth_u256, 6);

        assert_eq!(from_indexer, from_sweeper, "Indexer and Sweeper must agree");
        assert_eq!(from_indexer, 5_000_000); // 5 USDT in 6-decimal
    }
}

// ─── EVM Log Parsing Tests ─────────────────────────────────────────────────
//
// Tests for EvmBlockScanner::parse_transfer_logs (the full pipeline).
// Exercises the complete flow: raw EvmLog → IndexerTransferEvent.

#[cfg(test)]
mod evm_log_parsing_tests {
    use ironix_pay::services::evm::EvmLog;
    use ironix_pay::services::indexer::scanner::EvmBlockScanner;

    /// ERC-20 Transfer(address,address,uint256) event signature topic
    const TRANSFER_TOPIC: &str =
        "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

    /// Helper: Build a valid Transfer EVM log.
    fn make_transfer_log(
        from_topic: &str,
        to_topic: &str,
        amount_data: &str,
        block_number: &str,
        tx_hash: &str,
        log_index: &str,
    ) -> EvmLog {
        EvmLog {
            address: "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string(), // USDT
            topics: vec![
                TRANSFER_TOPIC.to_string(),
                from_topic.to_string(),
                to_topic.to_string(),
            ],
            data: amount_data.to_string(),
            block_number: Some(block_number.to_string()),
            transaction_hash: Some(tx_hash.to_string()),
            log_index: Some(log_index.to_string()),
            removed: None,
        }
    }

    #[test]
    fn test_parse_single_valid_transfer() {
        let log = make_transfer_log(
            "0x000000000000000000000000ab5801a7d398351b8be11c439e05c5b3259aec9b",
            "0x0000000000000000000000004838b106fce9647bdf1e7877bf73ce8b0bad5f97",
            "0x00000000000000000000000000000000000000000000000000000000000f4240", // 1_000_000
            "0x10",                                                               // block 16
            "0xabc123",
            "0x0",
        );

        let events = EvmBlockScanner::parse_transfer_logs(vec![log]).unwrap();

        assert_eq!(events.len(), 1);
        let e = &events[0];
        assert_eq!(e.tx_hash, "0xabc123");
        assert_eq!(e.amount, "1000000");
        assert_eq!(e.block_number, 16);
        assert_eq!(e.event_index, 0);
        // Addresses should be EIP-55 checksummed
        assert!(e.from.starts_with("0x"));
        assert!(e.to.starts_with("0x"));
        assert_eq!(e.from.len(), 42);
        assert_eq!(e.to.len(), 42);
    }

    #[test]
    fn test_parse_multiple_transfers() {
        let log1 = make_transfer_log(
            "0x000000000000000000000000ab5801a7d398351b8be11c439e05c5b3259aec9b",
            "0x0000000000000000000000004838b106fce9647bdf1e7877bf73ce8b0bad5f97",
            "0x00000000000000000000000000000000000000000000000000000000000f4240",
            "0x64", // block 100
            "0xtx1",
            "0x0",
        );
        let log2 = make_transfer_log(
            "0x0000000000000000000000004838b106fce9647bdf1e7877bf73ce8b0bad5f97",
            "0x000000000000000000000000ab5801a7d398351b8be11c439e05c5b3259aec9b",
            "0x0000000000000000000000000000000000000000000000000000000001e84800", // 32_000_000
            "0x64",
            "0xtx2",
            "0x1",
        );

        let events = EvmBlockScanner::parse_transfer_logs(vec![log1, log2]).unwrap();

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].amount, "1000000");
        assert_eq!(events[1].amount, "32000000");
        assert_eq!(events[0].tx_hash, "0xtx1");
        assert_eq!(events[1].tx_hash, "0xtx2");
    }

    #[test]
    fn test_skip_removed_log_reorg() {
        let mut log = make_transfer_log(
            "0x000000000000000000000000ab5801a7d398351b8be11c439e05c5b3259aec9b",
            "0x0000000000000000000000004838b106fce9647bdf1e7877bf73ce8b0bad5f97",
            "0x00000000000000000000000000000000000000000000000000000000000f4240",
            "0x10",
            "0xreorged",
            "0x0",
        );
        log.removed = Some(true); // This log was reorged

        let events = EvmBlockScanner::parse_transfer_logs(vec![log]).unwrap();
        assert_eq!(events.len(), 0, "Removed (reorged) logs must be skipped");
    }

    #[test]
    fn test_skip_insufficient_topics() {
        // Only 2 topics (missing 'to' address)
        let log = EvmLog {
            address: "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string(),
            topics: vec![
                TRANSFER_TOPIC.to_string(),
                "0x000000000000000000000000ab5801a7d398351b8be11c439e05c5b3259aec9b".to_string(),
            ],
            data: "0x00000000000000000000000000000000000000000000000000000000000f4240".to_string(),
            block_number: Some("0x10".to_string()),
            transaction_hash: Some("0xbadtopics".to_string()),
            log_index: Some("0x0".to_string()),
            removed: None,
        };

        let events = EvmBlockScanner::parse_transfer_logs(vec![log]).unwrap();
        assert_eq!(events.len(), 0, "Logs with < 3 topics must be skipped");
    }

    #[test]
    fn test_mixed_valid_and_invalid_logs() {
        let valid = make_transfer_log(
            "0x000000000000000000000000ab5801a7d398351b8be11c439e05c5b3259aec9b",
            "0x0000000000000000000000004838b106fce9647bdf1e7877bf73ce8b0bad5f97",
            "0x00000000000000000000000000000000000000000000000000000000000f4240",
            "0x10",
            "0xvalid",
            "0x0",
        );
        let mut removed = make_transfer_log(
            "0x000000000000000000000000ab5801a7d398351b8be11c439e05c5b3259aec9b",
            "0x0000000000000000000000004838b106fce9647bdf1e7877bf73ce8b0bad5f97",
            "0x00000000000000000000000000000000000000000000000000000000000f4240",
            "0x10",
            "0xremoved",
            "0x1",
        );
        removed.removed = Some(true);

        let events = EvmBlockScanner::parse_transfer_logs(vec![valid, removed]).unwrap();

        assert_eq!(events.len(), 1, "Only valid logs should be returned");
        assert_eq!(events[0].tx_hash, "0xvalid");
    }

    #[test]
    fn test_bsc_large_amount_18_decimals() {
        // 1000 USDT in BSC 18-decimal: "1000000000000000000000" = 10^21
        // This tests that the parser handles large hex values correctly
        let data = "0x00000000000000000000000000000000000000000000003635c9adc5dea00000"; // 10^21
        let log = make_transfer_log(
            "0x000000000000000000000000ab5801a7d398351b8be11c439e05c5b3259aec9b",
            "0x0000000000000000000000004838b106fce9647bdf1e7877bf73ce8b0bad5f97",
            data,
            "0xff",
            "0xbsc_big",
            "0x5",
        );

        let events = EvmBlockScanner::parse_transfer_logs(vec![log]).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].amount, "1000000000000000000000");
    }

    #[test]
    fn test_eip55_address_consistency() {
        // Verify that parse_transfer_logs produces EIP-55 checksummed addresses
        // that would match addresses stored in DB by derive_evm_address
        // (both use alloy_primitives::Address::to_checksum(None))
        let log = make_transfer_log(
            "0x000000000000000000000000ab5801a7d398351b8be11c439e05c5b3259aec9b",
            "0x0000000000000000000000004838b106fce9647bdf1e7877bf73ce8b0bad5f97",
            "0x00000000000000000000000000000000000000000000000000000000000f4240",
            "0x10",
            "0xtx",
            "0x0",
        );

        let events = EvmBlockScanner::parse_transfer_logs(vec![log]).unwrap();
        let e = &events[0];

        // EIP-55 encodes the same address deterministically
        // Running it again should produce the exact same result
        let from_bytes = hex::decode(&e.from[2..]).unwrap();
        let from_addr = alloy_primitives::Address::from_slice(&from_bytes);
        assert_eq!(
            e.from,
            from_addr.to_checksum(None),
            "Address must be stable EIP-55"
        );
    }

    #[test]
    fn test_block_timestamp_is_zero() {
        // EVM logs don't include block timestamp; scanner sets to 0.
        // Indexer resolves this later.
        let log = make_transfer_log(
            "0x000000000000000000000000ab5801a7d398351b8be11c439e05c5b3259aec9b",
            "0x0000000000000000000000004838b106fce9647bdf1e7877bf73ce8b0bad5f97",
            "0x00000000000000000000000000000000000000000000000000000000000f4240",
            "0x10",
            "0xtx",
            "0x0",
        );

        let events = EvmBlockScanner::parse_transfer_logs(vec![log]).unwrap();
        assert_eq!(
            events[0].block_timestamp, 0,
            "EVM logs must set block_timestamp to 0"
        );
    }
}

// ─── Chain Config Validation Tests ─────────────────────────────────────────
//
// Parameterized data-correctness test for all supported networks.
// Guards against typos in chain_id, decimals, or contract addresses
// when editing network.rs.

#[cfg(test)]
mod chain_config_tests {
    use ironix_pay::entity::{ChainFamily, Environment, Network};

    /// (Network, expected_chain_id, expected_decimals, expected_symbol, expected_chain_family)
    const PRODUCTION_CHAINS: &[(Network, Option<u64>, u8, &str, ChainFamily)] = &[
        (Network::Tron, None, 6, "TRX", ChainFamily::Tron),
        (Network::Bsc, Some(56), 18, "BNB", ChainFamily::Evm),
        (Network::Ethereum, Some(1), 6, "ETH", ChainFamily::Evm),
        (Network::Polygon, Some(137), 6, "POL", ChainFamily::Evm),
        (Network::Arbitrum, Some(42161), 6, "ETH", ChainFamily::Evm),
        (Network::Base, Some(8453), 6, "ETH", ChainFamily::Evm),
        (Network::Optimism, Some(10), 6, "ETH", ChainFamily::Evm),
    ];

    #[test]
    fn test_all_production_chain_configs() {
        for (network, expected_chain_id, expected_decimals, expected_symbol, expected_family) in
            PRODUCTION_CHAINS
        {
            let config = network.chain_config(&Environment::Production);

            assert_eq!(
                config.chain_id, *expected_chain_id,
                "{:?} chain_id mismatch",
                network
            );
            assert_eq!(
                config.usdt_decimals, *expected_decimals,
                "{:?} usdt_decimals mismatch",
                network
            );
            assert_eq!(
                config.native_symbol, *expected_symbol,
                "{:?} native_symbol mismatch",
                network
            );
            assert_eq!(
                network.chain_family(),
                *expected_family,
                "{:?} chain_family mismatch",
                network
            );

            // All chains must have non-zero confirmations
            assert!(
                config.confirmation_blocks > 0,
                "{:?} must require at least 1 confirmation",
                network
            );

            // All chains must have non-empty USDT contract
            assert!(
                !config.usdt_contract.is_empty(),
                "{:?} must have a USDT contract address",
                network
            );

            // EVM chains must have 0x-prefixed contracts
            if network.chain_family() == ChainFamily::Evm {
                assert!(
                    config.usdt_contract.starts_with("0x"),
                    "{:?} USDT contract must start with 0x",
                    network
                );
            }
        }
    }

    #[test]
    fn test_bsc_is_the_only_18_decimal_chain() {
        // This is a critical invariant: BSC is the only chain with 18-decimal USDT.
        // If another chain is added with 18 decimals, this test will remind the developer
        // to verify that normalization logic handles it correctly.
        for network in [
            Network::Tron,
            Network::Ethereum,
            Network::Polygon,
            Network::Arbitrum,
            Network::Base,
            Network::Optimism,
        ] {
            let config = network.chain_config(&Environment::Production);
            assert_eq!(
                config.usdt_decimals, 6,
                "{:?} should use 6-decimal USDT (only BSC uses 18)",
                network
            );
        }

        let bsc_config = Network::Bsc.chain_config(&Environment::Production);
        assert_eq!(bsc_config.usdt_decimals, 18, "BSC must use 18-decimal USDT");
    }
}
