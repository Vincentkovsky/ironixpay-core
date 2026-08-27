use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. addresses: ADD usdc_balance
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE addresses ADD COLUMN usdc_balance BIGINT NOT NULL DEFAULT 0",
            )
            .await?;

        // 2. merchant_chain_accounts: RENAME balance → usdt_balance, ADD usdc_balance
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE merchant_chain_accounts RENAME COLUMN balance TO usdt_balance",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE merchant_chain_accounts ADD COLUMN usdc_balance BIGINT NOT NULL DEFAULT 0",
            )
            .await?;

        // 3. billing_logs: ADD token
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE billing_logs ADD COLUMN token VARCHAR(10) NOT NULL DEFAULT 'USDT'",
            )
            .await?;

        // 4. sweep_transactions: ADD token (which token was swept)
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE sweep_transactions ADD COLUMN token VARCHAR(10) NOT NULL DEFAULT 'USDT'",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Reverse: drop new columns, rename back
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE sweep_transactions DROP COLUMN token")
            .await?;

        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE billing_logs DROP COLUMN token")
            .await?;

        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE merchant_chain_accounts DROP COLUMN usdc_balance")
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE merchant_chain_accounts RENAME COLUMN usdt_balance TO balance",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE addresses DROP COLUMN usdc_balance")
            .await?;

        Ok(())
    }
}
