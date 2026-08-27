use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Rename sweep_tx_id -> external_ref_id
        manager
            .alter_table(
                Table::alter()
                    .table(BillingLogs::Table)
                    .rename_column(BillingLogs::SweepTxId, BillingLogs::ExternalRefId)
                    .to_owned(),
            )
            .await?;

        // Create index for external_ref_id
        manager
            .create_index(
                Index::create()
                    .table(BillingLogs::Table)
                    .name("idx_billing_external_ref")
                    .col(BillingLogs::ExternalRefId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .table(BillingLogs::Table)
                    .name("idx_billing_external_ref")
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BillingLogs::Table)
                    .rename_column(BillingLogs::ExternalRefId, BillingLogs::SweepTxId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum BillingLogs {
    Table,
    SweepTxId,
    ExternalRefId,
}
