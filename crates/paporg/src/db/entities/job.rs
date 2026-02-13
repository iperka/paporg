//! Job entity for persistent storage.

use sea_orm::entity::prelude::*;

/// Job entity model.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "jobs")]
pub struct Model {
    /// Unique job identifier (UUID).
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    /// Original filename being processed.
    pub filename: String,
    /// Original source path (input file).
    pub source_path: String,
    /// Path to archived file (for re-runs).
    pub archive_path: Option<String>,
    /// Final output path.
    pub output_path: Option<String>,
    /// Detected category.
    #[sea_orm(default_value = "unsorted")]
    pub category: String,
    /// Name of the import source that discovered this job.
    pub source_name: Option<String>,
    /// Job status: pending, processing, completed, failed, superseded.
    #[sea_orm(default_value = "pending")]
    pub status: String,
    /// Error message if failed.
    pub error: Option<String>,
    /// When the job was created.
    pub created_at: DateTimeUtc,
    /// When the job was last updated.
    pub updated_at: DateTimeUtc,
    /// When the job completed.
    pub completed_at: Option<DateTimeUtc>,
    /// JSON array of symlink paths.
    pub symlinks: Option<String>,
    /// Current processing phase.
    pub current_phase: Option<String>,
    /// Human-readable status message.
    pub message: Option<String>,
    /// MIME type of the source file (e.g., "application/pdf", "image/png").
    pub mime_type: Option<String>,
    /// Extracted text content (from OCR or embedded text).
    pub ocr_text: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
