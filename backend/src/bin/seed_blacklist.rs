//! AML Blacklist Seeder
//!
//! Seeds the `aml_blacklist` table with OFAC SDN-sanctioned TRON addresses.
//!
//! Usage: cargo run --bin seed_blacklist
//!
//! Data sources:
//! - OFAC SDN List (via https://github.com/0xB10C/ofac-sanctioned-digital-currency-addresses)
//!
//! This is idempotent - running multiple times will not create duplicates.

use anyhow::Result;
use ironix_pay::services::aml::entity;
use sea_orm::{ActiveModelTrait, ConnectOptions, Database, EntityTrait, Set};
use std::env;
use tracing::{info, warn};

/// OFAC SDN-sanctioned TRON addresses
/// Source: https://github.com/0xB10C/ofac-sanctioned-digital-currency-addresses
/// Last updated: 2026-02-06
const OFAC_TRON_ADDRESSES: &[&str] = &[
    "TASWbk6X1wiTku5TMmMQYqYFvshVEtfJy8",
    "TAYhjpL8pPs8T84FSM329nffQpc6jD8GBM",
    "TAoLw5yD5XUoHWeBZRSZ1ExK9HMv2CiPvP",
    "TBHTJqAy4DhHhmT3dNceJYNRz4SdLofLre",
    "TBcLqqqyZjNj1ptuXFgj5H768NhNU5nDyn",
    "TC8axQvzJEVR3NKN6mZnJtGy7537GEmh38",
    "TCA9vmjsYw9MtPKEwRBtGhKFRfr4CLxJAv",
    "TCFD8N3vM5b4Gr5f1kkajQsodRVNyyAq1d",
    "TCu5onCzXuqxjvVzdB2tR4FLuF66d4yRqf",
    "TCzq6m2zxnQkrZrf8cqYcK6bbXQYAfWYKC",
    "TDFtJtyLPgN3oWUoHh23oJox3T5V5nR11K",
    "TEcuHDQthTmULe8fFLUccBPpjfXaTmJuuD",
    "TFdTr9C3BqQrzKBXqSxJfAZFTh8UwBAfSg",
    "TFwjPScaJRCbSWVAywE1S1WgaUgSnyYUbD",
    "TGckaiamj5NzaYx6Qp6Zu7kahuHArzUo99",
    "TGsNFrgWfbGN2gX25Wcf8oTejtxtQkvmEx",
    "TJkBr9TZ1xBeJoF7RNWqyEMbYqVJ6fXXHR",
    "TL1k1U6SHohxBqb68kCodxHc9y2LXoDSep",
    "TLvuvpfBKdxddxSsJefeiGCe9eVY8HUroE",
    "TMStbg5fgb4uTV7fK1gEYF9hKAzP3siPsG",
    "TMuCgBejD5RsNANZdjGtaM3YyKGNgDoy7N",
    "TNDjh6WGLYyWmkh8vfu42bXVHUqFNQ3rDq",
    "TNZxGWCwvsHr6JxQxzoeDXV597Yf7Zb7nV",
    "TNmRfnSUXZoWWzxcDDbf95eGQYXt1mJDt8",
    "TNuA5CQ6LB4jTHoNrjEeQZJmcmhQuHMbQ7",
    "TPJ1JNX98MJpHueBJeF5SVSg85z8mYg1P1",
    "TQ5H49Wz3K57zNHmuXVp6uLzFwitxviABs",
    "TRBACioxdrdsYEZHvJWiUDZcMdBPpEe5Ub",
    "TRakpsE1mZjCUMNPyozR4BW2ZtJsF7ZWFN",
    "TSxAAo67VTDgKT537EVXxdogkJtk9c6ojz",
    "TTKnV2S1295UrPr7N67Tp9ykNL7xX2Z4Uj",
    "TTS9o5KkpGgH8cK9LofLmMAPYb5zfQvSNa",
    "TTUDyVhhpCC1xJoPmWzdjLAzeoPwbSABdr",
    "TU4tDFRvcKhAZ1jdihojmBWZqvJhQCnJ4F",
    "TVNyvx2astt2AB1Us67ENjfMZeEXZeiuu6",
    "TYDUutYN4YLKUPeT7TG27Yyqw6kNVLq9QZ",
];

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    info!("🚀 AML Blacklist Seeder starting...");

    // Load database URL from environment
    dotenvy::dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    // Connect to database
    let mut opt = ConnectOptions::new(database_url);
    opt.sqlx_logging(false);
    let db = Database::connect(opt).await?;
    info!("✅ Connected to database");

    // Load existing addresses to check for duplicates
    let existing = entity::Entity::find().all(&db).await?;
    let existing_set: std::collections::HashSet<_> =
        existing.iter().map(|e| e.address.clone()).collect();
    info!("📊 Found {} existing blacklist entries", existing.len());

    // Seed OFAC addresses
    let mut inserted = 0;
    let mut skipped = 0;

    for address in OFAC_TRON_ADDRESSES {
        if existing_set.contains(*address) {
            skipped += 1;
            continue;
        }

        let record = entity::ActiveModel {
            address: Set(address.to_string()),
            source: Set("OFAC_SDN".to_string()),
            risk_level: Set(Some("SANCTIONS".to_string())),
            note: Set(Some("OFAC SDN List - sanctioned address".to_string())),
            created_at: Set(chrono::Utc::now().into()),
        };

        match record.insert(&db).await {
            Ok(_) => {
                inserted += 1;
                info!("  ✅ Added: {}", address);
            }
            Err(e) => {
                warn!("  ⚠️  Failed to insert {}: {}", address, e);
            }
        }
    }

    info!("");
    info!("========================================");
    info!("📈 Seeding complete!");
    info!("   Inserted: {}", inserted);
    info!("   Skipped (existing): {}", skipped);
    info!("   Total OFAC addresses: {}", OFAC_TRON_ADDRESSES.len());
    info!("========================================");

    Ok(())
}
