//! Migration: Create `org_members` table
//!
//! Links users to organizations (merchants) with role-based access.
//! Supports pending invitations via NULLABLE user_id + invited_email.

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
                CREATE TABLE org_members (
                    id              VARCHAR PRIMARY KEY,
                    org_id          VARCHAR NOT NULL REFERENCES merchants(id),
                    user_id         VARCHAR REFERENCES users(id),
                    invited_email   VARCHAR,
                    role            VARCHAR NOT NULL,
                    invited_by      VARCHAR REFERENCES users(id),
                    invited_at      TIMESTAMPTZ DEFAULT NOW(),
                    accepted_at     TIMESTAMPTZ,
                    status          VARCHAR NOT NULL DEFAULT 'active',
                    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    UNIQUE(org_id, user_id),
                    UNIQUE(org_id, invited_email)
                );

                -- Index for user membership lookups (login flow)
                CREATE INDEX idx_org_members_user_id ON org_members (user_id) WHERE user_id IS NOT NULL;
                -- Index for org member listing
                CREATE INDEX idx_org_members_org_id ON org_members (org_id);
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS org_members;")
            .await?;

        Ok(())
    }
}
