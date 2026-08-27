//! Migration: Add address_type column to addresses table
//!
//! Distinguishes between checkout (session) addresses and merchant deposit addresses.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Add the column with default value 'checkout'
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE addresses
                ADD COLUMN address_type VARCHAR(20) NOT NULL DEFAULT 'checkout';
                "#,
            )
            .await?;

        // 2. Mark existing deposit addresses (path_index = 2147483647) as merchant_deposit
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                UPDATE addresses
                SET address_type = 'merchant_deposit'
                WHERE path_index = 2147483647;
                "#,
            )
            .await?;

        // 3. Add index for efficient filtering by type
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE INDEX idx_addresses_type ON addresses (address_type);
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                DROP INDEX IF EXISTS idx_addresses_type;
                ALTER TABLE addresses DROP COLUMN IF EXISTS address_type;
                "#,
            )
            .await?;

        Ok(())
    }
}
