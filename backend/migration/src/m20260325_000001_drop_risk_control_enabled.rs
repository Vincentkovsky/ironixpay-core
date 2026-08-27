//! Migration: Drop risk_control_enabled from payout_settings
//!
//! The master switch caused a disconnect between the UI and actual risk enforcement.
//! Individual rules (require_new_address_approval, approval_threshold) now take effect
//! directly without a gating switch.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE payout_settings DROP COLUMN IF EXISTS risk_control_enabled",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE payout_settings ADD COLUMN risk_control_enabled BOOLEAN NOT NULL DEFAULT false",
            )
            .await?;
        Ok(())
    }
}
