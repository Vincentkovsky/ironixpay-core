//! Migration: Add configurable Xero tax type on connection config
//!
//! Default is NONE (No Tax) to keep invoice amount aligned with net payment.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE xero_connections ADD COLUMN xero_tax_type VARCHAR(50) NOT NULL DEFAULT 'NONE'",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE xero_connections DROP COLUMN IF EXISTS xero_tax_type")
            .await?;
        Ok(())
    }
}
