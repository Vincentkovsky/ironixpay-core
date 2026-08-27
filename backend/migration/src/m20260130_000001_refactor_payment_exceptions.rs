use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Statement;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. 先添加新列 resolution_ref_id
        manager
            .alter_table(
                Table::alter()
                    .table(PaymentExceptions::Table)
                    .add_column(
                        ColumnDef::new(PaymentExceptions::ResolutionRefId)
                            .string()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // 2. 【关键】执行数据清洗 (Data Migration)
        // 将旧的 status 映射到新的 status + resolution
        // 注意：这里假设你的数据库是 Postgres，使用 SQL 语法
        let sql = r#"
            UPDATE payment_exceptions
            SET
                -- 1. 映射 Resolution 字段
                resolution = CASE
                    WHEN status = 'refunded' THEN 'Transferred'
                    WHEN status = 'credited' THEN 'Accepted'
                    WHEN status = 'swept'    THEN 'Ignored'
                    WHEN status = 'ignored'  THEN 'Ignored'
                    ELSE resolution -- 如果本来就有值，保持不变
                END,

                -- 2. 映射 Status 字段 (统一归为 Resolved)
                status = CASE
                    WHEN status IN ('refunded', 'credited', 'swept', 'ignored', 'resolved') THEN 'Resolved'
                    ELSE 'Pending' -- 其他情况默认为 Pending
                END
            WHERE status NOT IN ('Pending', 'Resolved');
        "#;

        // 执行原生 SQL
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                sql.to_owned(),
            ))
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 回滚操作：删除列
        // 注意：数据清洗很难完美回滚，因为信息在合并过程中丢失了（比如 'refunded' 和 'swept' 都变成了 'Resolved'）
        // 这里通常只做结构的 rollback
        manager
            .alter_table(
                Table::alter()
                    .table(PaymentExceptions::Table)
                    .drop_column(PaymentExceptions::ResolutionRefId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum PaymentExceptions {
    Table,
    ResolutionRefId,
}
