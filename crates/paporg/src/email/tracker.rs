//! Email tracking for processed UIDs.

use chrono::Utc;
use log::{debug, info, warn};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::db::entities::processed_email;

use super::error::{EmailError, Result};

/// Tracks which email UIDs have been processed for each import source.
pub struct EmailTracker {
    db: DatabaseConnection,
    source_name: String,
    current_uidvalidity: Option<u32>,
}

impl EmailTracker {
    /// Creates a new email tracker for the given source.
    pub fn new(db: DatabaseConnection, source_name: String) -> Self {
        Self {
            db,
            source_name,
            current_uidvalidity: None,
        }
    }

    /// Sets the current UIDVALIDITY and checks for changes.
    ///
    /// If UIDVALIDITY has changed (folder was recreated), this clears the
    /// tracking data for this source since UIDs are no longer valid.
    pub async fn set_uidvalidity(&mut self, uidvalidity: u32) -> Result<()> {
        // Get the last known UIDVALIDITY for this source
        let last_uidvalidity = self.get_last_uidvalidity().await?;

        if let Some(last) = last_uidvalidity {
            if last != uidvalidity {
                warn!(
                    "UIDVALIDITY changed for source '{}': {} -> {}. Clearing tracking data.",
                    self.source_name, last, uidvalidity
                );
                self.clear_tracking_data_for_uidvalidity(last).await?;
            }
        }

        self.current_uidvalidity = Some(uidvalidity);
        Ok(())
    }

    /// Gets the last known UIDVALIDITY for this source.
    async fn get_last_uidvalidity(&self) -> Result<Option<u32>> {
        let record = processed_email::Entity::find()
            .filter(processed_email::Column::SourceName.eq(&self.source_name))
            .order_by_desc(processed_email::Column::ProcessedAt)
            .one(&self.db)
            .await?;

        Ok(record.map(|r| r.uidvalidity))
    }

    /// Clears tracking data for a specific UIDVALIDITY.
    async fn clear_tracking_data_for_uidvalidity(&self, uidvalidity: u32) -> Result<()> {
        let result = processed_email::Entity::delete_many()
            .filter(processed_email::Column::SourceName.eq(&self.source_name))
            .filter(processed_email::Column::Uidvalidity.eq(uidvalidity))
            .exec(&self.db)
            .await?;

        info!(
            "Cleared {} tracking records for source '{}' with UIDVALIDITY {}",
            result.rows_affected, self.source_name, uidvalidity
        );
        Ok(())
    }

    /// Returns the last processed UID for the current UIDVALIDITY.
    pub async fn last_processed_uid(&self) -> Result<Option<u32>> {
        let uidvalidity = self.current_uidvalidity.ok_or_else(|| {
            EmailError::ConfigError("UIDVALIDITY not set. Call set_uidvalidity first.".to_string())
        })?;

        let record = processed_email::Entity::find()
            .filter(processed_email::Column::SourceName.eq(&self.source_name))
            .filter(processed_email::Column::Uidvalidity.eq(uidvalidity))
            .order_by_desc(processed_email::Column::Uid)
            .one(&self.db)
            .await?;

        Ok(record.map(|r| r.uid))
    }

    /// Checks if a specific UID has been processed.
    pub async fn is_processed(&self, uid: u32) -> Result<bool> {
        let uidvalidity = self.current_uidvalidity.ok_or_else(|| {
            EmailError::ConfigError("UIDVALIDITY not set. Call set_uidvalidity first.".to_string())
        })?;

        let id = processed_email::Model::make_id(&self.source_name, uidvalidity, uid);
        let record = processed_email::Entity::find_by_id(&id)
            .one(&self.db)
            .await?;

        Ok(record.is_some())
    }

    /// Marks a UID as processed.
    pub async fn mark_processed(&self, uid: u32, message_id: Option<String>) -> Result<()> {
        let uidvalidity = self.current_uidvalidity.ok_or_else(|| {
            EmailError::ConfigError("UIDVALIDITY not set. Call set_uidvalidity first.".to_string())
        })?;

        let id = processed_email::Model::make_id(&self.source_name, uidvalidity, uid);

        let model = processed_email::ActiveModel {
            id: Set(id),
            source_name: Set(self.source_name.clone()),
            uidvalidity: Set(uidvalidity),
            uid: Set(uid),
            message_id: Set(message_id),
            processed_at: Set(Utc::now()),
        };

        model.insert(&self.db).await?;
        debug!(
            "Marked UID {} as processed for source '{}' (UIDVALIDITY={})",
            uid, self.source_name, uidvalidity
        );

        Ok(())
    }

    /// Filters a list of UIDs to only include those that haven't been processed.
    pub async fn filter_unprocessed(&self, uids: Vec<u32>) -> Result<Vec<u32>> {
        let uidvalidity = self.current_uidvalidity.ok_or_else(|| {
            EmailError::ConfigError("UIDVALIDITY not set. Call set_uidvalidity first.".to_string())
        })?;

        if uids.is_empty() {
            return Ok(Vec::new());
        }

        // Get all processed UIDs for this source and uidvalidity
        let processed_records = processed_email::Entity::find()
            .filter(processed_email::Column::SourceName.eq(&self.source_name))
            .filter(processed_email::Column::Uidvalidity.eq(uidvalidity))
            .filter(processed_email::Column::Uid.is_in(uids.clone()))
            .all(&self.db)
            .await?;

        let processed_uids: std::collections::HashSet<u32> =
            processed_records.into_iter().map(|r| r.uid).collect();

        let unprocessed: Vec<u32> = uids
            .into_iter()
            .filter(|uid| !processed_uids.contains(uid))
            .collect();

        debug!(
            "Filtered {} UIDs, {} unprocessed",
            processed_uids.len() + unprocessed.len(),
            unprocessed.len()
        );

        Ok(unprocessed)
    }

    /// Gets statistics for this source.
    pub async fn stats(&self) -> Result<TrackerStats> {
        use sea_orm::PaginatorTrait;

        let total_processed = processed_email::Entity::find()
            .filter(processed_email::Column::SourceName.eq(&self.source_name))
            .count(&self.db)
            .await?;

        let last_processed = processed_email::Entity::find()
            .filter(processed_email::Column::SourceName.eq(&self.source_name))
            .order_by_desc(processed_email::Column::ProcessedAt)
            .one(&self.db)
            .await?;

        Ok(TrackerStats {
            source_name: self.source_name.clone(),
            total_processed,
            last_processed_at: last_processed.map(|r| r.processed_at),
            current_uidvalidity: self.current_uidvalidity,
        })
    }
}

/// Statistics about processed emails for a source.
#[derive(Debug)]
pub struct TrackerStats {
    /// Name of the source.
    pub source_name: String,
    /// Total number of processed emails.
    pub total_processed: u64,
    /// When the last email was processed.
    pub last_processed_at: Option<chrono::DateTime<Utc>>,
    /// Current UIDVALIDITY.
    pub current_uidvalidity: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Integration tests would require a database connection.
    // These are basic unit tests for ID generation.

    #[test]
    fn test_make_id() {
        let id = processed_email::Model::make_id("my-source", 12345, 100);
        assert_eq!(id, "my-source:12345:100");
    }

    #[test]
    fn test_make_id_special_chars() {
        let id = processed_email::Model::make_id("source-with-dashes", 1, 2);
        assert_eq!(id, "source-with-dashes:1:2");
    }
}
