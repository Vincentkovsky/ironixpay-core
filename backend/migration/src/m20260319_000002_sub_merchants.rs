//! Sub-Merchant / PSP support.
//!
//! - Add `merchant_type` column to `merchants` table (direct vs sub_merchant)
//! - Create `sub_merchants` table for PSP → child org mapping

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // 1. Add merchant_type to merchants table
        db.execute_unprepared(
            r#"
            ALTER TABLE merchants
              ADD COLUMN merchant_type VARCHAR(20) NOT NULL DEFAULT 'direct'
            "#,
        )
        .await?;

        // 2. Create sub_merchants table
        db.execute_unprepared(
            r#"
            CREATE TABLE sub_merchants (
                id VARCHAR(255) PRIMARY KEY,
                parent_org_id VARCHAR(255) NOT NULL REFERENCES merchants(id),
                sub_merchant_code VARCHAR(100) NOT NULL,
                display_name VARCHAR(200) NOT NULL,
                child_org_id VARCHAR(255) NOT NULL REFERENCES merchants(id),
                status VARCHAR(20) NOT NULL DEFAULT 'active',
                created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                CONSTRAINT uq_sub_merchant_code UNIQUE(parent_org_id, sub_merchant_code)
            )
            "#,
        )
        .await?;

        // 3. Indexes
        db.execute_unprepared(
            "CREATE INDEX idx_sub_merchants_parent ON sub_merchants(parent_org_id)",
        )
        .await?;

        db.execute_unprepared(
            "CREATE INDEX idx_sub_merchants_child ON sub_merchants(child_org_id)",
        )
        .await?;

        // Ensure each child org maps to at most one sub-merchant record.
        // Without this, webhook routing could become ambiguous.
        db.execute_unprepared(
            "CREATE UNIQUE INDEX uq_sub_merchants_child_org ON sub_merchants(child_org_id)",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("DROP TABLE IF EXISTS sub_merchants").await?;
        db.execute_unprepared(
            "ALTER TABLE merchants DROP COLUMN IF EXISTS merchant_type",
        )
        .await?;

        Ok(())
    }
}
