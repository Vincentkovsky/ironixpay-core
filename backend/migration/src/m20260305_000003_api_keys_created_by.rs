//! Migration: Add `created_by_user_id` to `api_keys`
//!
//! Nullable audit field to track which user created each API key.
//! Existing keys will have NULL (created before multi-user support).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE api_keys
                ADD COLUMN created_by_user_id VARCHAR REFERENCES users(id);
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE api_keys DROP COLUMN IF EXISTS created_by_user_id;
                "#,
            )
            .await?;

        Ok(())
    }
}
