use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Make success_url and cancel_url nullable for API-only integrations
        // that don't need redirect URLs
        let db = manager.get_connection();

        db.execute_unprepared(
            "ALTER TABLE checkout_sessions ALTER COLUMN success_url DROP NOT NULL"
        ).await?;

        db.execute_unprepared(
            "ALTER TABLE checkout_sessions ALTER COLUMN cancel_url DROP NOT NULL"
        ).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Set existing NULLs to empty string before restoring NOT NULL
        db.execute_unprepared(
            "UPDATE checkout_sessions SET success_url = '' WHERE success_url IS NULL"
        ).await?;
        db.execute_unprepared(
            "UPDATE checkout_sessions SET cancel_url = '' WHERE cancel_url IS NULL"
        ).await?;

        db.execute_unprepared(
            "ALTER TABLE checkout_sessions ALTER COLUMN success_url SET NOT NULL"
        ).await?;
        db.execute_unprepared(
            "ALTER TABLE checkout_sessions ALTER COLUMN cancel_url SET NOT NULL"
        ).await?;

        Ok(())
    }
}
