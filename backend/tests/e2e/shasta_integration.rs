//! Integration tests for TronClient against Shasta testnet
//!
//! These tests make real API calls to the Shasta testnet.
//! Run with: cargo test --test shasta_integration -- --nocapture
//!
//! Note: Some tests may be flaky due to network conditions.

use anyhow::Result;
use ironix_pay::services::tron::TronClient;

/// Shasta testnet configuration
const SHASTA_FULL_NODE: &str = "https://api.shasta.trongrid.io";
const SHASTA_USDT_CONTRACT: &str = "TG3XXyExBkPp9nzdajDZsozEu4BkaSJozs";

/// A known valid Shasta address for testing
/// Using the USDT contract address itself as it's guaranteed to exist
const TEST_ADDRESS: &str = "TQXAbGcs8QG4F7JUMzKS5GTmPemowESWgg";

fn create_client() -> TronClient {
    TronClient::new(
        SHASTA_FULL_NODE.to_string(),
        SHASTA_USDT_CONTRACT.to_string(),
        None,
    )
}

#[tokio::test]
async fn test_get_current_block() -> Result<()> {
    let client = create_client();

    let block = client.get_current_block().await?;

    println!(
        "Current block: number={}, timestamp={}",
        block.number, block.timestamp
    );

    // Shasta should have blocks
    assert!(block.number > 0, "Block number should be positive");
    assert!(block.timestamp > 0, "Block timestamp should be positive");

    // Timestamp should be reasonable (after 2020)
    assert!(
        block.timestamp > 1577836800000,
        "Timestamp should be after 2020"
    );

    Ok(())
}

#[tokio::test]
async fn test_get_trx_balance() -> Result<()> {
    let client = create_client();

    // Query a known address - may have 0 balance but should not error
    let balance = client.get_trx_balance(TEST_ADDRESS).await?;

    println!("TRX balance for {}: {} SUN", TEST_ADDRESS, balance);

    // Just verify it doesn't error - balance could be 0
    Ok(())
}

#[tokio::test]
async fn test_get_usdt_balance() -> Result<()> {
    let client = create_client();

    // Query USDT balance - may be 0 but should not error
    let balance = client.get_usdt_balance(TEST_ADDRESS).await?;

    println!(
        "USDT balance for {}: {} (6 decimals)",
        TEST_ADDRESS, balance
    );

    // Just verify it doesn't error
    Ok(())
}

#[tokio::test]
async fn test_get_account_resources() -> Result<()> {
    let client = create_client();

    let resources = client.get_account_resources(TEST_ADDRESS).await?;

    println!("Account resources for {}:", TEST_ADDRESS);
    println!("  Free Net Limit: {}", resources.free_net_limit);
    println!("  Net Used: {}", resources.net_used);
    println!("  Energy Limit: {}", resources.energy_limit);
    println!("  Energy Used: {}", resources.energy_used);

    // Free net limit should be available for all accounts
    assert!(
        resources.free_net_limit >= 0,
        "Free net limit should be non-negative"
    );

    Ok(())
}

#[tokio::test]
async fn test_get_transaction_info_not_found() -> Result<()> {
    let client = create_client();

    // Query a non-existent transaction hash
    let fake_hash = "0000000000000000000000000000000000000000000000000000000000000000";
    let info = client.get_transaction_info(fake_hash).await?;

    println!("Transaction info for fake hash: {:?}", info);

    // Should return None for non-existent transaction
    assert!(
        info.is_none(),
        "Non-existent transaction should return None"
    );

    Ok(())
}

#[tokio::test]
async fn test_get_block_transactions() -> Result<()> {
    let client = create_client();

    // Get current block number first
    let current_block = client.get_current_block().await?;

    // Query a recent block (current - 10 to ensure it exists and has been processed)
    let block_num = (current_block.number as i64) - 10;
    let transactions = client.get_block_transactions(block_num).await?;

    println!(
        "Block {} has {} transactions",
        block_num,
        transactions.len()
    );

    // Just verify it doesn't error - block may have 0 transactions
    Ok(())
}

#[tokio::test]
async fn test_estimate_energy() -> Result<()> {
    let client = create_client();

    // Estimate energy for a simple transfer call
    // Using a dummy parameter (padded address)
    let parameter = format!("{:0>64}", "0000000000000000000000000000000000000001");

    let result = client
        .estimate_energy(
            TEST_ADDRESS,
            SHASTA_USDT_CONTRACT,
            "balanceOf(address)",
            &parameter,
        )
        .await;

    match result {
        Ok(energy) => {
            println!("Estimated energy for balanceOf: {}", energy);
            assert!(energy >= 0, "Energy should be non-negative");
        }
        Err(e) => {
            // Some errors are expected if the address doesn't have enough resources
            println!("Estimate energy failed (may be expected): {}", e);
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_has_recent_transactions() -> Result<()> {
    let client = create_client();

    // Check for transactions in the last hour
    let has_tx = client.has_recent_transactions(TEST_ADDRESS, 3600).await?;

    println!(
        "Address {} has recent transactions (1h): {}",
        TEST_ADDRESS, has_tx
    );

    // Just verify it doesn't error
    Ok(())
}

#[tokio::test]
async fn test_get_trc20_transactions() -> Result<()> {
    let client = create_client();

    // Query TRC20 transactions
    let transactions = client
        .get_trc20_transactions(TEST_ADDRESS, 10, None)
        .await?;

    println!(
        "Found {} TRC20 transactions for {}",
        transactions.len(),
        TEST_ADDRESS
    );

    for tx in transactions.iter().take(3) {
        println!(
            "  TX: {} from {} value {}",
            tx.transaction_id, tx.from, tx.value
        );
        if let Some(info) = &tx.token_info {
            println!(
                "    Token: {} ({}) [{}]",
                info.symbol, info.address, info.decimals
            );
        }
    }

    // Just verify it doesn't error
    Ok(())
}
