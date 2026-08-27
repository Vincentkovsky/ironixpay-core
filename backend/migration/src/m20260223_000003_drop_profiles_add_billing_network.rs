//! Migration: Drop merchant_profiles table + Add network to billing_logs
//!
//! merchant_profiles is now obsolete — balance lives on merchant_chain_accounts.
//! billing_logs needs a network column to track which chain the credit/debit belongs to.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Add network column to billing_logs (default TRON for existing rows)
        manager
            .alter_table(
                Table::alter()
                    .table(BillingLogs::Table)
                    .add_column(
                        ColumnDef::new(BillingLogs::Network)
                            .string_len(32)
                            .not_null()
                            .default("TRON"),
                    )
                    .to_owned(),
            )
            .await?;

        // 2. Drop merchant_profiles table (balance now on merchant_chain_accounts)
        manager
            .drop_table(Table::drop().table(MerchantProfiles::Table).to_owned())
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Recreate merchant_profiles
        manager
            .create_table(
                Table::create()
                    .table(MerchantProfiles::Table)
                    .col(
                        ColumnDef::new(MerchantProfiles::MerchantId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MerchantProfiles::Environment)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MerchantProfiles::Balance)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(MerchantProfiles::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MerchantProfiles::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(MerchantProfiles::MerchantId)
                            .col(MerchantProfiles::Environment),
                    )
                    .to_owned(),
            )
            .await?;

        // Drop network column from billing_logs
        manager
            .alter_table(
                Table::alter()
                    .table(BillingLogs::Table)
                    .drop_column(BillingLogs::Network)
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
}

#[derive(Iden)]
enum MerchantProfiles {
    Table,
    MerchantId,
    Environment,
    Balance,
    CreatedAt,
    UpdatedAt,
}
