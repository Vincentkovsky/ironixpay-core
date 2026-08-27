//! Migration: Convert account_index to use PostgreSQL SEQUENCE
//!
//! This removes the need for application-level locking and ensures
//! atomic, gap-free account_index allocation.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // 1. Create a sequence starting from MAX(account_index) + 1
        //    If table is empty, start from 1
        db.execute_unprepared(
            r#"
            DO $$
            DECLARE
                max_idx INTEGER;
            BEGIN
                SELECT COALESCE(MAX(account_index), 0) + 1 INTO max_idx FROM merchants;
                EXECUTE format('CREATE SEQUENCE merchant_account_index_seq START WITH %s', max_idx);
            END $$;
            "#,
        )
        .await?;

        // 2. Set the sequence as the default for account_index
        db.execute_unprepared(
            r#"
            ALTER TABLE merchants
            ALTER COLUMN account_index SET DEFAULT nextval('merchant_account_index_seq');
            "#,
        )
        .await?;

        // 3. Make the sequence owned by the column (for cleanup on drop)
        db.execute_unprepared(
            r#"
            ALTER SEQUENCE merchant_account_index_seq OWNED BY merchants.account_index;
            "#,
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Remove default and drop sequence
        db.execute_unprepared(
            r#"
            ALTER TABLE merchants ALTER COLUMN account_index DROP DEFAULT;
            DROP SEQUENCE IF EXISTS merchant_account_index_seq;
            "#,
        )
        .await?;

        Ok(())
    }
}
