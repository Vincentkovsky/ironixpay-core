//! Ledger mode migration
//!
//! - Creates `withdrawals` table for merchant payout tracking
//! - Adds `fee_amount` and `net_amount` to `checkout_sessions`

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Create withdrawals table
        manager
            .create_table(
                Table::create()
                    .table(Withdrawals::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Withdrawals::Id)
                            .string_len(64)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Withdrawals::MerchantId)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Withdrawals::Environment)
                            .string_len(20)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Withdrawals::Amount).big_integer().not_null())
                    .col(
                        ColumnDef::new(Withdrawals::NetworkFee)
                            .big_integer()
                            .not_null()
                            .default(1_000_000i64),
                    )
                    .col(
                        ColumnDef::new(Withdrawals::NetAmount)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Withdrawals::ToAddress)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Withdrawals::Status)
                            .string_len(20)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Withdrawals::TxHash).string_len(100))
                    .col(ColumnDef::new(Withdrawals::ErrorReason).text())
                    .col(
                        ColumnDef::new(Withdrawals::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Withdrawals::CompletedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_withdrawals_merchant")
                            .from(Withdrawals::Table, Withdrawals::MerchantId)
                            .to(Merchants::Table, Merchants::Id),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for querying by merchant + status
        manager
            .create_index(
                Index::create()
                    .name("idx_withdrawals_merchant")
                    .table(Withdrawals::Table)
                    .col(Withdrawals::MerchantId)
                    .col(Withdrawals::Status)
                    .to_owned(),
            )
            .await?;

        // 2. Add fee_amount and net_amount to checkout_sessions
        manager
            .alter_table(
                Table::alter()
                    .table(CheckoutSessions::Table)
                    .add_column(ColumnDef::new(CheckoutSessions::FeeAmount).big_integer())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CheckoutSessions::Table)
                    .add_column(ColumnDef::new(CheckoutSessions::NetAmount).big_integer())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CheckoutSessions::Table)
                    .drop_column(CheckoutSessions::NetAmount)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(CheckoutSessions::Table)
                    .drop_column(CheckoutSessions::FeeAmount)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Withdrawals::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Withdrawals {
    Table,
    Id,
    MerchantId,
    Environment,
    Amount,
    NetworkFee,
    NetAmount,
    ToAddress,
    Status,
    TxHash,
    ErrorReason,
    CreatedAt,
    CompletedAt,
}

#[derive(DeriveIden)]
enum CheckoutSessions {
    Table,
    FeeAmount,
    NetAmount,
}

#[derive(DeriveIden)]
enum Merchants {
    Table,
    Id,
}
