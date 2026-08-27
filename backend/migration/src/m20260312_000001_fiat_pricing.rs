use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();

        // 1. checkout_sessions: ADD 5 fiat pricing snapshot fields
        conn.execute_unprepared(
            "ALTER TABLE checkout_sessions ADD COLUMN pricing_currency VARCHAR(8)",
        )
        .await?;
        conn.execute_unprepared(
            "ALTER TABLE checkout_sessions ADD COLUMN pricing_amount DECIMAL(18,8)",
        )
        .await?;
        conn.execute_unprepared(
            "ALTER TABLE checkout_sessions ADD COLUMN exchange_rate DECIMAL(18,8)",
        )
        .await?;

        // 2. exchange_rates: Cache table for latest rates (UPSERT pattern)
        conn.execute_unprepared(
            "CREATE TABLE exchange_rates (
                id SERIAL PRIMARY KEY,
                crypto VARCHAR(8) NOT NULL,
                fiat VARCHAR(8) NOT NULL,
                rate DECIMAL(18,8) NOT NULL,
                source VARCHAR(32) NOT NULL DEFAULT 'coingecko',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                UNIQUE (crypto, fiat)
            )",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();

        // Reverse in opposite order
        conn.execute_unprepared("DROP TABLE IF EXISTS exchange_rates")
            .await?;
        conn.execute_unprepared("ALTER TABLE checkout_sessions DROP COLUMN exchange_rate")
            .await?;
        conn.execute_unprepared("ALTER TABLE checkout_sessions DROP COLUMN pricing_amount")
            .await?;
        conn.execute_unprepared("ALTER TABLE checkout_sessions DROP COLUMN pricing_currency")
            .await?;

        Ok(())
    }
}
