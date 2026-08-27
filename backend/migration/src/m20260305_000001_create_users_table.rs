//! Migration: Create `users` table
//!
//! Extracts authentication/identity fields from `merchants` into a dedicated
//! `users` table. This enables multi-user organizations (Role & Organization feature).

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
                CREATE TABLE users (
                    id              VARCHAR PRIMARY KEY,
                    email           VARCHAR UNIQUE NOT NULL,
                    password_hash   VARCHAR NOT NULL,
                    name            VARCHAR NOT NULL,
                    totp_secret     VARCHAR,
                    is_totp_enabled BOOLEAN NOT NULL DEFAULT FALSE,
                    token_version   INT NOT NULL DEFAULT 0,
                    backup_codes    TEXT,
                    email_verified  BOOLEAN NOT NULL DEFAULT FALSE,
                    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                -- Index for login lookups
                CREATE INDEX idx_users_email ON users (email);
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS users;")
            .await?;

        Ok(())
    }
}
