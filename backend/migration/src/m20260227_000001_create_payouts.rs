//! Create payouts table for merchant-to-end-user payouts.
//!
//! Separate from withdrawals (merchant self-withdrawal) for clean separation
//! of concerns: different auth model, AML requirements, and rate limiting.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create payouts table
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("payouts"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("merchant_id"))
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("environment"))
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("network")).string().not_null())
                    .col(ColumnDef::new(Alias::new("to_address")).string().not_null())
                    .col(
                        ColumnDef::new(Alias::new("amount"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("fee")).big_integer().not_null())
                    .col(
                        ColumnDef::new(Alias::new("net_amount"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("status"))
                            .string()
                            .not_null()
                            .default("Pending"),
                    )
                    .col(ColumnDef::new(Alias::new("tx_hash")).string().null())
                    .col(ColumnDef::new(Alias::new("error_reason")).string().null())
                    .col(
                        ColumnDef::new(Alias::new("idempotency_key"))
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("description")).string().null())
                    .col(ColumnDef::new(Alias::new("metadata")).json_binary().null())
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Alias::new("updated_at"))
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Alias::new("completed_at"))
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Alias::new("payouts"), Alias::new("merchant_id"))
                            .to(Alias::new("merchants"), Alias::new("id")),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint: idempotency per merchant per environment
        manager
            .create_index(
                Index::create()
                    .name("idx_payouts_idempotency")
                    .table(Alias::new("payouts"))
                    .col(Alias::new("merchant_id"))
                    .col(Alias::new("environment"))
                    .col(Alias::new("idempotency_key"))
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Status index for worker scanning
        manager
            .create_index(
                Index::create()
                    .name("idx_payouts_status")
                    .table(Alias::new("payouts"))
                    .col(Alias::new("status"))
                    .col(Alias::new("environment"))
                    .to_owned(),
            )
            .await?;

        // Merchant listing index
        manager
            .create_index(
                Index::create()
                    .name("idx_payouts_merchant")
                    .table(Alias::new("payouts"))
                    .col(Alias::new("merchant_id"))
                    .col(Alias::new("environment"))
                    .col(Alias::new("created_at"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("payouts")).to_owned())
            .await
    }
}
