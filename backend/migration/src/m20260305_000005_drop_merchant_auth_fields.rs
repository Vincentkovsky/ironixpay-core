//! Migration: Drop auth fields from merchants table
//!
//! After data migration to `users` table, these columns are no longer needed
//! on `merchants`. The merchants table now represents only the organization entity.

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
                ALTER TABLE merchants
                    DROP COLUMN email,
                    DROP COLUMN password_hash,
                    DROP COLUMN totp_secret,
                    DROP COLUMN is_totp_enabled,
                    DROP COLUMN token_version,
                    DROP COLUMN backup_codes,
                    DROP COLUMN email_verified;
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Restore columns with sensible defaults
        // Actual data restoration would require re-copying from users table
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE merchants
                    ADD COLUMN email VARCHAR,
                    ADD COLUMN password_hash VARCHAR,
                    ADD COLUMN totp_secret VARCHAR,
                    ADD COLUMN is_totp_enabled BOOLEAN NOT NULL DEFAULT FALSE,
                    ADD COLUMN token_version INT NOT NULL DEFAULT 0,
                    ADD COLUMN backup_codes TEXT,
                    ADD COLUMN email_verified BOOLEAN NOT NULL DEFAULT FALSE;

                -- Restore data from users table
                UPDATE merchants m
                SET email = u.email,
                    password_hash = u.password_hash,
                    totp_secret = u.totp_secret,
                    is_totp_enabled = u.is_totp_enabled,
                    token_version = u.token_version,
                    backup_codes = u.backup_codes,
                    email_verified = u.email_verified
                FROM users u
                WHERE m.id = u.id;

                -- Re-add NOT NULL + UNIQUE after data restoration
                ALTER TABLE merchants
                    ALTER COLUMN email SET NOT NULL,
                    ALTER COLUMN password_hash SET NOT NULL,
                    ADD CONSTRAINT merchants_email_key UNIQUE (email);
                "#,
            )
            .await?;

        Ok(())
    }
}
