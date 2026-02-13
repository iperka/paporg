//! Processed email entity for tracking which emails have been processed.

use sea_orm::entity::prelude::*;

/// Processed email entity model.
///
/// Tracks which email UIDs have been processed to avoid re-processing.
/// The combination of source_name + uidvalidity + uid uniquely identifies
/// an email message.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "processed_emails")]
pub struct Model {
    /// Unique identifier: "{source}:{uidvalidity}:{uid}".
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// Name of the import source that processed this email.
    pub source_name: String,

    /// UIDVALIDITY of the folder when the email was processed.
    /// If this changes, the folder was recreated and UIDs are invalid.
    pub uidvalidity: u32,

    /// UID of the processed email within the folder.
    pub uid: u32,

    /// Message-ID header of the email (for debugging/tracking).
    pub message_id: Option<String>,

    /// When this email was processed.
    pub processed_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Creates a unique ID for a processed email.
    pub fn make_id(source_name: &str, uidvalidity: u32, uid: u32) -> String {
        format!("{}:{}:{}", source_name, uidvalidity, uid)
    }
}
