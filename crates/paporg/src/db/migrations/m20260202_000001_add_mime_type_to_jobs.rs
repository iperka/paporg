//! Migration to add mime_type column to jobs table.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add column
        manager
            .alter_table(
                Table::alter()
                    .table(Jobs::Table)
                    // MIME types are typically short (e.g., "application/pdf", max ~100 chars)
                    .add_column(ColumnDef::new(Jobs::MimeType).string_len(255).null())
                    .to_owned(),
            )
            .await?;

        // Add index on MimeType for filtering jobs by type
        manager
            .create_index(
                Index::create()
                    .name("idx_jobs_mime_type")
                    .table(Jobs::Table)
                    .col(Jobs::MimeType)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop index first
        manager
            .drop_index(
                Index::drop()
                    .name("idx_jobs_mime_type")
                    .table(Jobs::Table)
                    .to_owned(),
            )
            .await?;

        // Drop column
        manager
            .alter_table(
                Table::alter()
                    .table(Jobs::Table)
                    .drop_column(Jobs::MimeType)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Jobs {
    Table,
    MimeType,
}
