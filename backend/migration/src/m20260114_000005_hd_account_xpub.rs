use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Merchants::Table)
                    .add_column(ColumnDef::new(Alias::new("hd_account_xpub")).text().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Merchants::Table)
                    .drop_column(Alias::new("hd_account_xpub"))
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Merchants {
    Table,
}
