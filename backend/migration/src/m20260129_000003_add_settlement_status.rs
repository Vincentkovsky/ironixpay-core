use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Add settlement_status and settlement_tx_hash columns to checkout_sessions
        manager
            .alter_table(
                Table::alter()
                    .table(CheckoutSessions::Table)
                    .add_column(
                        ColumnDef::new(CheckoutSessions::SettlementStatus)
                            .string()
                            .not_null()
                            .default("Unsettled"),
                    )
                    .add_column(
                        ColumnDef::new(CheckoutSessions::SettlementTxHash)
                            .string()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // 2. Add composite index for performance
        // (merchant_id, status, settlement_status)
        manager
            .create_index(
                Index::create()
                    .name("idx_sessions_payment_settlement")
                    .table(CheckoutSessions::Table)
                    .col(CheckoutSessions::MerchantId)
                    .col(CheckoutSessions::Status)
                    .col(CheckoutSessions::SettlementStatus)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Remove index
        manager
            .drop_index(
                Index::drop()
                    .name("idx_sessions_payment_settlement")
                    .table(CheckoutSessions::Table)
                    .to_owned(),
            )
            .await?;

        // 2. Remove columns
        manager
            .alter_table(
                Table::alter()
                    .table(CheckoutSessions::Table)
                    .drop_column(CheckoutSessions::SettlementStatus)
                    .drop_column(CheckoutSessions::SettlementTxHash)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

/// Learn from m20260113_000001_init_schema.rs
#[derive(Iden)]
enum CheckoutSessions {
    Table,
    MerchantId,
    Status,
    SettlementStatus,
    SettlementTxHash,
}
