//! Database migrations.

use sea_orm_migration::prelude::*;

mod m20240130_000001_create_jobs_table;
mod m20260131_000001_create_processed_emails_table;
mod m20260201_000001_create_oauth_tokens_table;
mod m20260202_000001_add_mime_type_to_jobs;
mod m20260204_000001_add_ocr_text_to_jobs;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240130_000001_create_jobs_table::Migration),
            Box::new(m20260131_000001_create_processed_emails_table::Migration),
            Box::new(m20260201_000001_create_oauth_tokens_table::Migration),
            Box::new(m20260202_000001_add_mime_type_to_jobs::Migration),
            Box::new(m20260204_000001_add_ocr_text_to_jobs::Migration),
        ]
    }
}
