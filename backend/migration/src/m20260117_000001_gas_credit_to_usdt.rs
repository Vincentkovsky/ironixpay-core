//! Reset gas_credit_balance and change semantics from TRX to USDT
//!
//! This migration:
//! 1. Resets all merchant gas_credit_balance to 0
//! 2. Adds a comment clarifying the new USDT semantics
//!
//! **Breaking Change**: Merchants need to top up their gas credit again in USDT.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Reset all gas credit balances to 0
        // The column semantics change from TRX (sun) to USDT (microunits)
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                UPDATE merchants SET gas_credit_balance = 0;

                COMMENT ON COLUMN merchants.gas_credit_balance IS
                    'Gas credit balance in USDT microunits (1 USDT = 1,000,000). Used to pay Energy rental costs during sweep operations.';
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Cannot restore previous TRX balances - just update comment
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                COMMENT ON COLUMN merchants.gas_credit_balance IS
                    'Gas credit balance in TRX (sun units).';
                "#,
            )
            .await?;

        Ok(())
    }
}
