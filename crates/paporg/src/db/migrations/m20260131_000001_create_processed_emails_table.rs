//! Migration to create the processed_emails table for tracking email UIDs.

use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProcessedEmails::Table)
                    .if_not_exists()
                    .col(string(ProcessedEmails::Id).primary_key())
                    .col(string(ProcessedEmails::SourceName).not_null())
                    .col(unsigned(ProcessedEmails::Uidvalidity).not_null())
                    .col(unsigned(ProcessedEmails::Uid).not_null())
                    .col(string_null(ProcessedEmails::MessageId))
                    .col(timestamp_with_time_zone(ProcessedEmails::ProcessedAt).not_null())
                    .to_owned(),
            )
            .await?;

        // Create index for efficient lookup by source and uidvalidity
        manager
            .create_index(
                Index::create()
                    .name("idx_processed_emails_source_validity")
                    .table(ProcessedEmails::Table)
                    .col(ProcessedEmails::SourceName)
                    .col(ProcessedEmails::Uidvalidity)
                    .to_owned(),
            )
            .await?;

        // Create unique constraint on source + uidvalidity + uid
        manager
            .create_index(
                Index::create()
                    .name("idx_processed_emails_unique")
                    .table(ProcessedEmails::Table)
                    .col(ProcessedEmails::SourceName)
                    .col(ProcessedEmails::Uidvalidity)
                    .col(ProcessedEmails::Uid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ProcessedEmails::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ProcessedEmails {
    Table,
    Id,
    SourceName,
    Uidvalidity,
    Uid,
    MessageId,
    ProcessedAt,
}
