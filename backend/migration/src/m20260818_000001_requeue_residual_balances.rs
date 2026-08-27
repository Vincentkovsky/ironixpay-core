//! Requeue successful sessions whose funds remain in reusable address states.

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
                UPDATE addresses AS a
                SET status = 'Detected', updated_at = NOW()
                WHERE a.status IN ('Idle', 'Cooling')
                  AND (a.usdt_balance > 0 OR a.usdc_balance > 0)
                  AND EXISTS (
                      SELECT 1
                      FROM checkout_sessions AS cs
                      WHERE cs.network = a.network
                        AND LOWER(cs.pay_address) = LOWER(a.address)
                        AND cs.status IN ('Paid', 'Overpaid')
                        AND cs.settlement_status = 'Unsettled'
                  )
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
