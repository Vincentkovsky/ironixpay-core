//! Concurrency Tests for Address Manager
//!
//! Tests concurrent address allocation to ensure no race conditions.
//! Aligned with docs/system_design.md schema.

use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set};
use secrecy::Secret;
use std::collections::HashSet;
use std::sync::Arc;
use ironix_pay::entity::{addresses, merchants};
use ironix_pay::services::{
    address::key_provider::MockMasterKeyProvider, address::AddressManager, webhook::WebhookService,
};
use uuid::Uuid;

const TRON_NETWORK: &str = "TRON";
const TEST_PREFIX: &str = "TConcurrentTest";

#[tokio::test]
async fn test_concurrent_address_allocation() {
    // 1. Setup Database
    // 1. Setup Database
    use crate::common;
    let test_db = common::setup_test_db().await;
    let db = &test_db.conn;

    // Cleanup not needed as it's a fresh container, but keeping logic is harmless
    // Just ensuring we use the new db instance

    // 2. Create Merchant
    let merchant_id = format!("mer_{}", Uuid::new_v4().to_string().replace("-", ""));
    let merchant = merchants::ActiveModel {
        id: Set(merchant_id.clone()),
        name: Set("ConcurrentTestMerchant".to_string()),
        status: Set(merchants::MerchantStatus::Active),
        account_index: Set(999), // Test account
        last_path_index: Set(0),
        collection_address: Set(Some("TTestCollectionAddress".to_string())),
        flat_fee: Set(0),
        min_sweep_threshold: Set(1_000_000),
        gas_credit_balance: Set(100_000_000),
        ..Default::default()
    };
    merchant.insert(db).await.unwrap();

    // 3. Seed Pool with 5 Idle Addresses
    for i in 1..=5 {
        let addr = addresses::ActiveModel {
            network: Set(TRON_NETWORK.to_string()),
            address: Set(format!("{}Addr{}", TEST_PREFIX, i)),
            merchant_id: Set(merchant_id.clone()),
            status: Set(addresses::AddressStatus::Idle),
            path_index: Set(i),
            usdt_balance: Set(0),
            sweep_attempts: Set(0),
            error_reason: Set(None),
            ..Default::default()
        };
        addr.insert(db).await.unwrap();
    }

    let hex_key = hex::encode([1u8; 32]);
    let manager = Arc::new(AddressManager::new(
        db.clone(),
        Secret::new(hex_key),
        Box::new(MockMasterKeyProvider::new(None)),
    ));
    let merchant_id_arc = Arc::new(merchant_id.clone());

    // 4. Concurrent requests
    // We have 5 idle addresses.
    // We launch 10 concurrent requests.
    // Expected: 5 should reuse existing addresses, 5 should fail (pool exhausted)
    // Unless generate_addresses is called to create more.

    let mut handles = vec![];
    for _i in 0..10 {
        let manager = manager.clone();
        let merchant_id = merchant_id_arc.clone();

        handles.push(tokio::spawn(async move {
            manager.allocate_address(&merchant_id, TRON_NETWORK).await
        }));
    }

    let mut allocated_addresses = Vec::new();
    let mut failures = 0;
    for handle in handles {
        match handle.await.unwrap() {
            Ok((_, addr)) => allocated_addresses.push(addr),
            Err(_) => failures += 1,
        }
    }

    // 5. Verify Results
    println!("Allocated: {:?}", allocated_addresses);
    println!("Failures (pool exhausted): {}", failures);

    // All allocated addresses must be unique (no double allocation)
    let unique: HashSet<&String> = allocated_addresses.iter().collect();
    assert_eq!(
        unique.len(),
        allocated_addresses.len(),
        "All allocated addresses must be unique"
    );

    // We should have allocated exactly 5 (all idle addresses)
    assert_eq!(
        allocated_addresses.len(),
        5,
        "Should allocate exactly 5 addresses from pool"
    );
    assert_eq!(failures, 5, "5 requests should fail due to pool exhausted");

    // Verify DB state - no idle addresses left
    let idle_count = addresses::Entity::find()
        .filter(addresses::Column::MerchantId.eq(&merchant_id))
        .filter(addresses::Column::Status.eq(addresses::AddressStatus::Idle))
        .count(db)
        .await
        .unwrap();

    assert_eq!(idle_count, 0, "All idle addresses should be used");

    // Cleanup
    addresses::Entity::delete_many()
        .filter(addresses::Column::Address.contains(TEST_PREFIX))
        .exec(db)
        .await
        .unwrap();
    merchants::Entity::delete_by_id(&merchant_id)
        .exec(db)
        .await
        .unwrap();
}
