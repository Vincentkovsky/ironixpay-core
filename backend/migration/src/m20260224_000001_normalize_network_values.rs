//! Migration: Normalize network values to pure chain names
//!
//! Replaces environment-specific network names (TRON_NILE, TRON_MAINNET, etc.)
//! with pure chain identifiers (TRON, BSC, ETHEREUM).
//!
//! DEPLOYMENT NOTE: This migration must be run via psql (not via SeaORM migrator)
//! because it needs to DROP and RE-ADD FK constraints, which requires careful
//! transaction handling. See the SQL block below.
//!
//! The SeaORM migrator will see this migration as already applied (inserted into
//! seaql_migrations as part of the psql transaction).
//!
//! psql $DATABASE_URL -f migration/sql/normalize_network_values.sql

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Drop FK constraints referencing addresses(network, address) PK
        db.execute_unprepared(
            "ALTER TABLE checkout_sessions DROP CONSTRAINT IF EXISTS fk_checkout_sessions_address",
        )
        .await?;
        db.execute_unprepared(
            "ALTER TABLE sweep_transactions DROP CONSTRAINT IF EXISTS fk_sweep_transactions_address"
        ).await?;

        // Delete stale indexer_state rows that would collide with normalized values
        db.execute_unprepared(
            "DELETE FROM indexer_state WHERE network IN ('TRON', 'BSC', 'ETHEREUM')
             AND network NOT IN (SELECT DISTINCT network FROM addresses)",
        )
        .await?;

        // -- TRON normalization --
        db.execute_unprepared(
            "UPDATE addresses SET network = 'TRON' WHERE network IN ('TRON_MAINNET', 'TRON_NILE', 'TRON_SHASTA')"
        ).await?;
        db.execute_unprepared(
            "UPDATE checkout_sessions SET network = 'TRON' WHERE network IN ('TRON_MAINNET', 'TRON_NILE', 'TRON_SHASTA')"
        ).await?;
        db.execute_unprepared(
            "UPDATE transactions SET network = 'TRON' WHERE network IN ('TRON_MAINNET', 'TRON_NILE', 'TRON_SHASTA')"
        ).await?;
        db.execute_unprepared(
            "UPDATE sweep_transactions SET network = 'TRON' WHERE network IN ('TRON_MAINNET', 'TRON_NILE', 'TRON_SHASTA')"
        ).await?;
        db.execute_unprepared(
            "UPDATE payment_exceptions SET network = 'TRON' WHERE network IN ('TRON_MAINNET', 'TRON_NILE', 'TRON_SHASTA')"
        ).await?;
        db.execute_unprepared(
            "UPDATE indexer_state SET network = 'TRON' WHERE network IN ('TRON_MAINNET', 'TRON_NILE', 'TRON_SHASTA')"
        ).await?;

        // -- BSC normalization --
        db.execute_unprepared(
            "UPDATE addresses SET network = 'BSC' WHERE network IN ('BSC_MAINNET', 'BSC_TESTNET')",
        )
        .await?;
        db.execute_unprepared(
            "UPDATE checkout_sessions SET network = 'BSC' WHERE network IN ('BSC_MAINNET', 'BSC_TESTNET')"
        ).await?;
        db.execute_unprepared(
            "UPDATE transactions SET network = 'BSC' WHERE network IN ('BSC_MAINNET', 'BSC_TESTNET')"
        ).await?;
        db.execute_unprepared(
            "UPDATE sweep_transactions SET network = 'BSC' WHERE network IN ('BSC_MAINNET', 'BSC_TESTNET')"
        ).await?;
        db.execute_unprepared(
            "UPDATE payment_exceptions SET network = 'BSC' WHERE network IN ('BSC_MAINNET', 'BSC_TESTNET')"
        ).await?;
        db.execute_unprepared(
            "UPDATE indexer_state SET network = 'BSC' WHERE network IN ('BSC_MAINNET', 'BSC_TESTNET')"
        ).await?;

        // Re-add FK constraints
        db.execute_unprepared(
            "ALTER TABLE checkout_sessions ADD CONSTRAINT fk_checkout_sessions_address
             FOREIGN KEY (network, pay_address) REFERENCES addresses(network, address)",
        )
        .await?;
        db.execute_unprepared(
            "ALTER TABLE sweep_transactions ADD CONSTRAINT fk_sweep_transactions_address
             FOREIGN KEY (network, from_address) REFERENCES addresses(network, address) ON DELETE RESTRICT"
        ).await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Irreversible: old format embedded redundant environment info
        Ok(())
    }
}
