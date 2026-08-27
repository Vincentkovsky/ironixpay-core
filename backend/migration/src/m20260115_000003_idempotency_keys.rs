//! Migration: Add idempotency_keys table
//!
//! Per system_design.md §7.4: Implements Idempotency-Key header support
//! for write operations to prevent duplicate requests from network retries.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create idempotency_keys table
        manager
            .create_table(
                Table::create()
                    .table(IdempotencyKeys::Table)
                    .if_not_exists()
                    // Composite primary key: (merchant_id, idempotency_key)
                    .col(
                        ColumnDef::new(IdempotencyKeys::MerchantId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IdempotencyKeys::IdempotencyKey)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IdempotencyKeys::RequestPath)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IdempotencyKeys::RequestHash)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IdempotencyKeys::ResponseCode)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(IdempotencyKeys::ResponseBody)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(IdempotencyKeys::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(IdempotencyKeys::MerchantId)
                            .col(IdempotencyKeys::IdempotencyKey),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_idempotency_keys_merchant")
                            .from(IdempotencyKeys::Table, IdempotencyKeys::MerchantId)
                            .to(Merchants::Table, Merchants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for automatic cleanup (24h expiry)
        manager
            .create_index(
                Index::create()
                    .name("idx_idempotency_keys_expire")
                    .table(IdempotencyKeys::Table)
                    .col(IdempotencyKeys::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(IdempotencyKeys::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum IdempotencyKeys {
    Table,
    MerchantId,
    IdempotencyKey,
    RequestPath,
    RequestHash,
    ResponseCode,
    ResponseBody,
    CreatedAt,
}

#[derive(Iden)]
enum Merchants {
    Table,
    Id,
}
