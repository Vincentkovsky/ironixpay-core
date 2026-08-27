//! Migration: Remove address_type column from addresses table
//!
//! The column only ever had one value ('checkout') and is now redundant.
//! All addresses are checkout addresses.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE addresses
                ADD COLUMN address_type VARCHAR(20) NOT NULL DEFAULT 'checkout';

                CREATE INDEX idx_addresses_type ON addresses (address_type);
                "#,
            )
            .await?;

        Ok(())
    }
}
