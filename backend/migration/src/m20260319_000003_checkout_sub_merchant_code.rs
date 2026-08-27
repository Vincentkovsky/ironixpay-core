use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add sub_merchant_code to checkout_sessions for efficient DTO responses.
        // Populated at session creation time when X-Sub-Merchant-Code header is present.
        // NULL for direct merchant sessions.
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("checkout_sessions"))
                    .add_column(
                        ColumnDef::new(Alias::new("sub_merchant_code"))
                            .string_len(100)
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("checkout_sessions"))
                    .drop_column(Alias::new("sub_merchant_code"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
