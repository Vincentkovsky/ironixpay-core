//! Migration: Add `balance` column to `merchant_chain_accounts`.
//!
//! Migrates balance data from `merchant_profiles` (unified) to per-chain rows.
//! Existing balances are assigned to the TRON chain account for each merchant/env.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Add balance column (default 0)
        manager
            .alter_table(
                Table::alter()
                    .table(MerchantChainAccounts::Table)
                    .add_column(
                        ColumnDef::new(MerchantChainAccounts::Balance)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        // 2. Data migration: copy existing merchant_profiles.balance into TRON chain account rows
        let db = manager.get_connection();
        db.execute_unprepared(
            r#"
            UPDATE merchant_chain_accounts mca
            SET balance = mp.balance
            FROM merchant_profiles mp
            WHERE mca.merchant_id = mp.merchant_id
              AND mca.environment = mp.environment
              AND mca.network = 'TRON'
              AND mp.balance > 0
            "#,
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(MerchantChainAccounts::Table)
                    .drop_column(MerchantChainAccounts::Balance)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum MerchantChainAccounts {
    Table,
    Balance,
}
