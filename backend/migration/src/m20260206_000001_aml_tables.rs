//! AML tables migration
//!
//! Creates tables for Anti-Money Laundering compliance:
//! - aml_blacklist: OFAC/sanctions blacklist addresses
//! - aml_api_cache: Cached GoPlus API results (24h TTL)

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create aml_blacklist table
        manager
            .create_table(
                Table::create()
                    .table(AmlBlacklist::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AmlBlacklist::Address)
                            .string_len(64)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AmlBlacklist::Source)
                            .string_len(50)
                            .not_null(),
                    )
                    .col(ColumnDef::new(AmlBlacklist::RiskLevel).string_len(20))
                    .col(ColumnDef::new(AmlBlacklist::Note).text())
                    .col(
                        ColumnDef::new(AmlBlacklist::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Create aml_api_cache table
        manager
            .create_table(
                Table::create()
                    .table(AmlApiCache::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AmlApiCache::Address)
                            .string_len(64)
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(AmlApiCache::IsRisky).boolean().not_null())
                    .col(ColumnDef::new(AmlApiCache::RiskReason).text())
                    .col(
                        ColumnDef::new(AmlApiCache::CheckedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for cache cleanup (DELETE WHERE checked_at < ...)
        manager
            .create_index(
                Index::create()
                    .name("idx_aml_cache_checked_at")
                    .table(AmlApiCache::Table)
                    .col(AmlApiCache::CheckedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AmlApiCache::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(AmlBlacklist::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum AmlBlacklist {
    Table,
    Address,
    Source,
    RiskLevel,
    Note,
    CreatedAt,
}

#[derive(DeriveIden)]
enum AmlApiCache {
    Table,
    Address,
    IsRisky,
    RiskReason,
    CheckedAt,
}
