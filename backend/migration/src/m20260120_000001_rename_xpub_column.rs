use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Rename hd_account_xpub to xpub_encrypted
        manager
            .alter_table(
                Table::alter()
                    .table(Merchants::Table)
                    .rename_column(Alias::new("hd_account_xpub"), Merchants::XpubEncrypted)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Merchants::Table)
                    .rename_column(Merchants::XpubEncrypted, Alias::new("hd_account_xpub"))
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Merchants {
    Table,
    XpubEncrypted,
}
