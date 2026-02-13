//! Migration to add ocr_text column to jobs table.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add ocr_text column (TEXT for potentially large content)
        manager
            .alter_table(
                Table::alter()
                    .table(Jobs::Table)
                    .add_column(ColumnDef::new(Jobs::OcrText).text().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Jobs::Table)
                    .drop_column(Jobs::OcrText)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Jobs {
    Table,
    OcrText,
}
