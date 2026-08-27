//! Migration: Migrate auth data from merchants → users + org_members
//!
//! For each merchant:
//! 1. Creates a user row (reusing the merchant's ID for backward JWT compat)
//! 2. Creates an org_member row (role=owner, status=active)
//!
//! ⚠️ PRE-REQUISITE: Run email dedup check before this migration:
//!   SELECT email, COUNT(*) FROM merchants GROUP BY email HAVING COUNT(*) > 1;
//!   If duplicates exist, resolve them manually first.

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
                -- Ensure gen_random_uuid() is available (needed for testcontainers/fresh DBs)
                CREATE EXTENSION IF NOT EXISTS pgcrypto;

                -- 1. Create a user for each merchant (reuse merchant ID for JWT backward compat)
                INSERT INTO users (id, email, password_hash, name, totp_secret,
                                   is_totp_enabled, token_version, backup_codes,
                                   email_verified, created_at, updated_at)
                SELECT
                    id,
                    email, password_hash, name, totp_secret,
                    is_totp_enabled, token_version, backup_codes,
                    email_verified, created_at, updated_at
                FROM merchants;

                -- 2. Create owner membership for each merchant
                INSERT INTO org_members (id, org_id, user_id, role, status, accepted_at, created_at, updated_at)
                SELECT
                    'om_' || gen_random_uuid(),
                    id,
                    id,
                    'owner',
                    'active',
                    NOW(),
                    NOW(),
                    NOW()
                FROM merchants;
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Reverse: delete migrated data (org_members first due to FK)
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                DELETE FROM org_members WHERE role = 'owner'
                    AND user_id = org_id;
                DELETE FROM users WHERE id IN (SELECT id FROM merchants);
                "#,
            )
            .await?;

        Ok(())
    }
}
