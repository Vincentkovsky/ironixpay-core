//! Migration: Add environment/network columns for isolation
//!
//! Architecture:
//! - api_keys: NO field (environment derived from sk_test_/sk_live_ prefix)
//! - webhook_endpoints: `environment` (sandbox/production) - merchants have 2 URLs, not per-chain
//! - webhook_events: `network` - need to know which chain triggered event
//! - billing_logs: `network` - references chain-specific transactions

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // webhook_endpoints: Use `environment` (sandbox/production)
        // Merchants have dev.example.com and api.example.com, not per-chain URLs
        manager
            .alter_table(
                Table::alter()
                    .table(WebhookEndpoints::Table)
                    .add_column(
                        ColumnDef::new(WebhookEndpoints::Environment)
                            .string_len(32)
                            .not_null()
                            .default("production"),
                    )
                    .to_owned(),
            )
            .await?;

        // webhook_events: Use `network` - need to know which chain triggered event
        manager
            .alter_table(
                Table::alter()
                    .table(WebhookEvents::Table)
                    .add_column(
                        ColumnDef::new(WebhookEvents::Network)
                            .string_len(32)
                            .not_null()
                            .default("TRON_MAINNET"),
                    )
                    .to_owned(),
            )
            .await?;

        // billing_logs: Use `network` - references chain-specific transactions
        manager
            .alter_table(
                Table::alter()
                    .table(BillingLogs::Table)
                    .add_column(
                        ColumnDef::new(BillingLogs::Network)
                            .string_len(32)
                            .not_null()
                            .default("TRON_MAINNET"),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes
        manager
            .create_index(
                Index::create()
                    .name("idx_webhook_endpoints_merchant_env")
                    .table(WebhookEndpoints::Table)
                    .col(WebhookEndpoints::MerchantId)
                    .col(WebhookEndpoints::Environment)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_billing_logs_merchant_network")
                    .table(BillingLogs::Table)
                    .col(BillingLogs::MerchantId)
                    .col(BillingLogs::Network)
                    .to_owned(),
            )
            .await?;

        // [CRITICAL] Worker-optimized index for webhook retry queue
        // Ensures mainnet workers don't scan testnet garbage
        manager
            .create_index(
                Index::create()
                    .name("idx_webhook_events_worker")
                    .table(WebhookEvents::Table)
                    .col(WebhookEvents::Network)
                    .col(WebhookEvents::Status)
                    .col(WebhookEvents::NextRetryAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop indexes first
        manager
            .drop_index(Index::drop().name("idx_webhook_events_worker").to_owned())
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_billing_logs_merchant_network")
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_webhook_endpoints_merchant_env")
                    .to_owned(),
            )
            .await?;

        // Drop columns
        manager
            .alter_table(
                Table::alter()
                    .table(BillingLogs::Table)
                    .drop_column(BillingLogs::Network)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(WebhookEvents::Table)
                    .drop_column(WebhookEvents::Network)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(WebhookEndpoints::Table)
                    .drop_column(WebhookEndpoints::Environment)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum WebhookEndpoints {
    Table,
    MerchantId,
    Environment,
}

#[derive(Iden)]
enum WebhookEvents {
    Table,
    Network,
    Status,
    NextRetryAt,
}

#[derive(Iden)]
enum BillingLogs {
    Table,
    MerchantId,
    Network,
}
