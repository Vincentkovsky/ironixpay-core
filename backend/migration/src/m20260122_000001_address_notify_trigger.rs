//! Migration: Add PostgreSQL LISTEN/NOTIFY trigger for address creation
//!
//! This enables real-time cache synchronization in TransactionIndexer.
//! New addresses are pushed to listeners via pg_notify within milliseconds.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create the notification function
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE OR REPLACE FUNCTION notify_address_created()
                RETURNS TRIGGER AS $$
                BEGIN
                    PERFORM pg_notify(
                        'address_created',
                        json_build_object(
                            'network', NEW.network,
                            'address', NEW.address
                        )::text
                    );
                    RETURN NEW;
                END;
                $$ LANGUAGE plpgsql;
                "#,
            )
            .await?;

        // Create the trigger
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TRIGGER address_created_trigger
                AFTER INSERT ON addresses
                FOR EACH ROW
                EXECUTE FUNCTION notify_address_created();
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop trigger first
        manager
            .get_connection()
            .execute_unprepared("DROP TRIGGER IF EXISTS address_created_trigger ON addresses;")
            .await?;

        // Then drop function
        manager
            .get_connection()
            .execute_unprepared("DROP FUNCTION IF EXISTS notify_address_created();")
            .await?;

        Ok(())
    }
}
