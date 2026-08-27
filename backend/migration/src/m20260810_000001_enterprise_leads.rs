//! Migration: Enterprise lead intake
//!
//! Persists public website enterprise inquiries before email notification.

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
                CREATE TABLE enterprise_leads (
                    id                  VARCHAR(64) PRIMARY KEY,
                    company_name        VARCHAR(120) NOT NULL,
                    company_website     VARCHAR(300),
                    contact_email       VARCHAR(254) NOT NULL,
                    telegram            VARCHAR(100),
                    business_type       VARCHAR(40) NOT NULL,
                    monthly_volume      VARCHAR(40) NOT NULL,
                    networks            JSONB NOT NULL DEFAULT '[]'::jsonb,
                    integration_needs   JSONB NOT NULL DEFAULT '[]'::jsonb,
                    message             TEXT,
                    locale              VARCHAR(5) NOT NULL DEFAULT 'en',
                    source              VARCHAR(40) NOT NULL DEFAULT 'website_enterprise',
                    status              VARCHAR(20) NOT NULL DEFAULT 'new',
                    notification_status VARCHAR(20) NOT NULL DEFAULT 'pending',
                    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    CONSTRAINT chk_enterprise_leads_locale
                        CHECK (locale IN ('en', 'zh')),
                    CONSTRAINT chk_enterprise_leads_status
                        CHECK (status IN ('new', 'contacted', 'qualified', 'closed')),
                    CONSTRAINT chk_enterprise_leads_notification_status
                        CHECK (notification_status IN ('pending', 'sent', 'failed'))
                );

                CREATE INDEX idx_enterprise_leads_status_created
                    ON enterprise_leads(status, created_at DESC);
                CREATE INDEX idx_enterprise_leads_contact_email
                    ON enterprise_leads(contact_email);
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS enterprise_leads;")
            .await?;
        Ok(())
    }
}
