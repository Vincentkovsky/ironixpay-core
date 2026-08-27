use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();

        // Step 1: Backfill existing crypto-mode sessions with non-null values.
        // pricing_currency = currency (e.g., "USDT"), pricing_amount from microunits,
        // exchange_rate = 1.0 (self-referential).
        // Use ROUND + LEAST to safely fit into DECIMAL(18,8) — handles extreme test data.
        conn.execute_unprepared(
            "UPDATE checkout_sessions
             SET pricing_currency = currency,
                 pricing_amount = LEAST(ROUND(amount_expected::numeric / 1000000, 8), 9999999999.99999999),
                 exchange_rate = 1.00000000
             WHERE pricing_currency IS NULL",
        )
        .await?;

        // Step 2: Set NOT NULL constraints
        conn.execute_unprepared(
            "ALTER TABLE checkout_sessions
             ALTER COLUMN pricing_currency SET NOT NULL,
             ALTER COLUMN pricing_amount SET NOT NULL,
             ALTER COLUMN exchange_rate SET NOT NULL",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();

        // Reverse: make columns nullable again
        conn.execute_unprepared(
            "ALTER TABLE checkout_sessions
             ALTER COLUMN pricing_currency DROP NOT NULL,
             ALTER COLUMN pricing_amount DROP NOT NULL,
             ALTER COLUMN exchange_rate DROP NOT NULL",
        )
        .await?;

        // Optionally null out crypto-mode rows (pricing_currency = currency)
        conn.execute_unprepared(
            "UPDATE checkout_sessions
             SET pricing_currency = NULL, pricing_amount = NULL, exchange_rate = NULL
             WHERE pricing_currency = currency",
        )
        .await?;

        Ok(())
    }
}
