//! Initial migration to create jobs table.

use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Jobs::Table)
                    .if_not_exists()
                    .col(string(Jobs::Id).primary_key())
                    .col(string(Jobs::Filename).not_null())
                    .col(string(Jobs::SourcePath).not_null())
                    .col(string_null(Jobs::ArchivePath))
                    .col(string_null(Jobs::OutputPath))
                    .col(string(Jobs::Category).not_null().default("unsorted"))
                    .col(string_null(Jobs::SourceName))
                    .col(string(Jobs::Status).not_null().default("pending"))
                    .col(text_null(Jobs::Error))
                    .col(timestamp_with_time_zone(Jobs::CreatedAt).not_null())
                    .col(timestamp_with_time_zone(Jobs::UpdatedAt).not_null())
                    .col(timestamp_with_time_zone_null(Jobs::CompletedAt))
                    .col(text_null(Jobs::Symlinks))
                    .col(string_null(Jobs::CurrentPhase))
                    .col(text_null(Jobs::Message))
                    .to_owned(),
            )
            .await?;

        // Create indexes for common queries
        manager
            .create_index(
                Index::create()
                    .name("idx_jobs_status")
                    .table(Jobs::Table)
                    .col(Jobs::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_jobs_category")
                    .table(Jobs::Table)
                    .col(Jobs::Category)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_jobs_created_at")
                    .table(Jobs::Table)
                    .col(Jobs::CreatedAt)
                    .to_owned(),
            )
            .await?;

        // Index on nullable source_name column.
        // This is valid in SQLite/PostgreSQL/MySQL and supports queries filtering by source.
        // NULL values are included in the index, allowing efficient IS NULL checks.
        manager
            .create_index(
                Index::create()
                    .name("idx_jobs_source_name")
                    .table(Jobs::Table)
                    .col(Jobs::SourceName)
                    .to_owned(),
            )
            .await?;

        // Composite index for common query pattern: filter by status, order by created_at
        manager
            .create_index(
                Index::create()
                    .name("idx_jobs_status_created_at")
                    .table(Jobs::Table)
                    .col(Jobs::Status)
                    .col(Jobs::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Jobs::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Jobs {
    Table,
    Id,
    Filename,
    SourcePath,
    ArchivePath,
    OutputPath,
    Category,
    SourceName,
    Status,
    Error,
    CreatedAt,
    UpdatedAt,
    CompletedAt,
    Symlinks,
    CurrentPhase,
    Message,
}
