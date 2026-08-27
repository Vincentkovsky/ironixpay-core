//! Migration: Enforce single owner per organization
//!
//! Creates a partial unique index on org_members to prevent
//! multiple active owners within the same organization.

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
                CREATE UNIQUE INDEX idx_org_members_one_owner_per_org
                ON org_members (org_id)
                WHERE role = 'owner' AND status = 'active';
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX IF EXISTS idx_org_members_one_owner_per_org;")
            .await?;

        Ok(())
    }
}
