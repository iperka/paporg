//! Email tracking for processed UIDs.

use chrono::Utc;
use log::{debug, info, warn};

use crate::db::{email_repo, Database};

use super::error::Result;

/// Tracks which email UIDs have been processed for each import source.
pub struct EmailTracker {
    db: Database,
    source_name: String,
    current_uidvalidity: Option<u32>,
}

impl EmailTracker {
    /// Creates a new email tracker for the given source.
    pub fn new(db: Database, source_name: String) -> Self {
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
    pub fn set_uidvalidity(&mut self, uidvalidity: u32) -> Result<()> {
        let last_uidvalidity = email_repo::find_last_uidvalidity(&self.db, &self.source_name)?;

        if let Some(last) = last_uidvalidity {
            if last != uidvalidity {
                warn!(
                    "UIDVALIDITY changed for source '{}': {} -> {}. Clearing tracking data.",
                    self.source_name, last, uidvalidity
                );
                let deleted = email_repo::delete_by_source_and_uidvalidity(
                    &self.db,
                    &self.source_name,
                    last,
                )?;
                info!(
                    "Cleared {} tracking records for source '{}' with UIDVALIDITY {}",
                    deleted, self.source_name, last
                );
            }
        }

        self.current_uidvalidity = Some(uidvalidity);
        Ok(())
    }

    /// Returns the last processed UID for the current UIDVALIDITY.
    pub fn last_processed_uid(&self) -> Result<Option<u32>> {
        let uidvalidity = self.require_uidvalidity()?;
        Ok(email_repo::find_last_uid(
            &self.db,
            &self.source_name,
            uidvalidity,
        )?)
    }

    /// Checks if a specific UID has been processed.
    pub fn is_processed(&self, uid: u32) -> Result<bool> {
        let uidvalidity = self.require_uidvalidity()?;
        let processed =
            email_repo::find_processed_uids(&self.db, &self.source_name, uidvalidity, &[uid])?;
        Ok(!processed.is_empty())
    }

    /// Marks a UID as processed.
    pub fn mark_processed(&self, uid: u32, message_id: Option<String>) -> Result<()> {
        let uidvalidity = self.require_uidvalidity()?;
        let id = email_repo::make_id(&self.source_name, uidvalidity, uid);

        let row = email_repo::ProcessedEmailRow {
            id,
            source_name: self.source_name.clone(),
            uidvalidity,
            uid,
            message_id,
            processed_at: Utc::now().to_rfc3339(),
        };

        email_repo::insert(&self.db, &row)?;
        debug!(
            "Marked UID {} as processed for source '{}' (UIDVALIDITY={})",
            uid, self.source_name, uidvalidity
        );

        Ok(())
    }

    /// Filters a list of UIDs to only include those that haven't been processed.
    pub fn filter_unprocessed(&self, uids: Vec<u32>) -> Result<Vec<u32>> {
        let uidvalidity = self.require_uidvalidity()?;

        if uids.is_empty() {
            return Ok(Vec::new());
        }

        let processed =
            email_repo::find_processed_uids(&self.db, &self.source_name, uidvalidity, &uids)?;

        let processed_set: std::collections::HashSet<u32> = processed.into_iter().collect();
        let unprocessed: Vec<u32> = uids
            .into_iter()
            .filter(|uid| !processed_set.contains(uid))
            .collect();

        debug!(
            "Filtered {} UIDs, {} unprocessed",
            processed_set.len() + unprocessed.len(),
            unprocessed.len()
        );

        Ok(unprocessed)
    }

    /// Gets statistics for this source.
    pub fn stats(&self) -> Result<TrackerStats> {
        let total_processed = email_repo::count_by_source(&self.db, &self.source_name)?;
        let last_processed_at = email_repo::find_last_processed_at(&self.db, &self.source_name)?;

        Ok(TrackerStats {
            source_name: self.source_name.clone(),
            total_processed,
            last_processed_at,
            current_uidvalidity: self.current_uidvalidity,
        })
    }

    /// Requires that `set_uidvalidity` has been called.
    fn require_uidvalidity(&self) -> Result<u32> {
        self.current_uidvalidity.ok_or_else(|| {
            super::error::EmailError::ConfigError(
                "UIDVALIDITY not set. Call set_uidvalidity first.".to_string(),
            )
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
    /// When the last email was processed (ISO 8601).
    pub last_processed_at: Option<String>,
    /// Current UIDVALIDITY.
    pub current_uidvalidity: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        Database::open_in_memory().expect("Failed to create test database")
    }

    #[test]
    fn test_make_id() {
        let id = email_repo::make_id("my-source", 12345, 100);
        assert_eq!(id, "my-source:12345:100");
    }

    #[test]
    fn test_make_id_special_chars() {
        let id = email_repo::make_id("source-with-dashes", 1, 2);
        assert_eq!(id, "source-with-dashes:1:2");
    }

    #[test]
    fn test_mark_and_check_processed() {
        let db = test_db();
        let mut tracker = EmailTracker::new(db, "test-source".to_string());
        tracker.set_uidvalidity(100).unwrap();

        assert!(!tracker.is_processed(42).unwrap());
        tracker.mark_processed(42, None).unwrap();
        assert!(tracker.is_processed(42).unwrap());
    }

    #[test]
    fn test_last_processed_uid() {
        let db = test_db();
        let mut tracker = EmailTracker::new(db, "test-source".to_string());
        tracker.set_uidvalidity(100).unwrap();

        assert_eq!(tracker.last_processed_uid().unwrap(), None);
        tracker.mark_processed(10, None).unwrap();
        tracker.mark_processed(20, None).unwrap();
        tracker.mark_processed(5, None).unwrap();
        assert_eq!(tracker.last_processed_uid().unwrap(), Some(20));
    }

    #[test]
    fn test_filter_unprocessed() {
        let db = test_db();
        let mut tracker = EmailTracker::new(db, "test-source".to_string());
        tracker.set_uidvalidity(100).unwrap();

        tracker.mark_processed(1, None).unwrap();
        tracker.mark_processed(3, None).unwrap();

        let unprocessed = tracker.filter_unprocessed(vec![1, 2, 3, 4]).unwrap();
        assert_eq!(unprocessed, vec![2, 4]);
    }

    #[test]
    fn test_stats() {
        let db = test_db();
        let mut tracker = EmailTracker::new(db, "test-source".to_string());
        tracker.set_uidvalidity(100).unwrap();

        tracker.mark_processed(1, None).unwrap();
        tracker.mark_processed(2, None).unwrap();

        let stats = tracker.stats().unwrap();
        assert_eq!(stats.total_processed, 2);
        assert!(stats.last_processed_at.is_some());
        assert_eq!(stats.current_uidvalidity, Some(100));
    }
}
