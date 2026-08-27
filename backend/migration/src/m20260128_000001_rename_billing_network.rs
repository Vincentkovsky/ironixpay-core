use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Rename network -> environment
        manager
            .alter_table(
                Table::alter()
                    .table(BillingLogs::Table)
                    .rename_column(BillingLogs::Network, BillingLogs::Environment)
                    .to_owned(),
            )
            .await?;

        // 2. Data Migration: TRON_MAINNET -> Production, TRON_NILE -> Sandbox
        // Note: This assumes only these two values exist.
        // We use raw SQL for data updates as it's the most reliable way in migrations.
        let sql_prod =
            "UPDATE billing_logs SET environment = 'Production' WHERE environment = 'TRON_MAINNET'";
        let sql_sandbox =
            "UPDATE billing_logs SET environment = 'Sandbox' WHERE environment = 'TRON_NILE'";

        manager
            .get_connection()
            .execute_unprepared(sql_prod)
            .await?;
        manager
            .get_connection()
            .execute_unprepared(sql_sandbox)
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Reverse Data Migration
        let sql_mainnet =
            "UPDATE billing_logs SET environment = 'TRON_MAINNET' WHERE environment = 'Production'";
        let sql_nile =
            "UPDATE billing_logs SET environment = 'TRON_NILE' WHERE environment = 'Sandbox'";

        manager
            .get_connection()
            .execute_unprepared(sql_mainnet)
            .await?;
        manager
            .get_connection()
            .execute_unprepared(sql_nile)
            .await?;

        // Rename environment -> network
        manager
            .alter_table(
                Table::alter()
                    .table(BillingLogs::Table)
                    .rename_column(BillingLogs::Environment, BillingLogs::Network)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum BillingLogs {
    Table,
    Network,
    Environment,
}
