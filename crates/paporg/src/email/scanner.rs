//! Email source scanner that orchestrates fetching and processing email attachments.

use std::path::PathBuf;

use crate::db::Database;
use chrono::{DateTime, NaiveDate, Utc};
use tracing::{debug, error, info, info_span, warn};

use crate::gitops::resource::EmailSourceConfig;
use crate::worker::job::EmailMetadata;
use crate::worker::Job;

use super::client::ImapClient;
use super::error::{EmailError, Result};
use super::parser::{EmailParser, ExtractedAttachment};
use super::tracker::EmailTracker;

/// Scanner for email attachment import sources.
pub struct EmailSourceScanner {
    config: EmailSourceConfig,
    source_name: String,
    temp_dir: PathBuf,
    db: Option<Database>,
}

impl EmailSourceScanner {
    /// Creates a new email source scanner.
    pub fn new(source_name: String, config: EmailSourceConfig, temp_dir: PathBuf) -> Self {
        Self {
            config,
            source_name,
            temp_dir,
            db: None,
        }
    }

    /// Sets the database connection for tracking processed emails.
    pub fn with_database(mut self, db: Database) -> Self {
        self.db = Some(db);
        self
    }

    /// Scans for new email attachments and returns jobs for processing.
    pub async fn scan(&self) -> Result<Vec<Job>> {
        let _span = info_span!("email_scan", source = %self.source_name).entered();
        info!("Scanning email source '{}'", self.source_name);

        // Create IMAP client
        let mut client = ImapClient::new(self.config.clone());

        // Connect to server
        client.connect().await?;

        // Open folder in read-only mode
        let uidvalidity = client.examine_folder(&self.config.folder).await?;

        // Set up tracker if database is available
        let tracker = if let Some(db) = &self.db {
            let mut tracker = EmailTracker::new(db.clone(), self.source_name.clone());
            tracker.set_uidvalidity(uidvalidity)?;
            Some(tracker)
        } else {
            warn!(
                "No database connection - email tracking disabled for '{}'",
                self.source_name
            );
            None
        };

        // Get UIDs to process
        let uids = self
            .get_uids_to_process(&mut client, tracker.as_ref())
            .await?;

        if uids.is_empty() {
            info!("No new emails to process in source '{}'", self.source_name);
            client.disconnect().await?;
            return Ok(Vec::new());
        }

        info!(
            "Found {} new emails to process in source '{}'",
            uids.len(),
            self.source_name
        );

        // Limit batch size
        let batch_size = self.config.batch_size as usize;
        let uids_to_process: Vec<u32> = uids.into_iter().take(batch_size).collect();

        // Create parser
        let parser = EmailParser::new(
            self.config.mime_filters.clone(),
            self.config.min_attachment_size,
            self.config.max_attachment_size,
        );

        // Fetch and process emails
        let mut jobs = Vec::new();
        let emails = client.fetch_emails_peek(&uids_to_process).await?;

        for (uid, raw_email) in emails {
            match self.process_email(uid, &raw_email, &parser).await {
                Ok(attachments) => {
                    // Save attachments and create jobs
                    for attachment in attachments {
                        match self.save_attachment(&attachment).await {
                            Ok(path) => {
                                // Convert email info to job metadata using explicit conversion
                                jobs.push(Job::from_email(
                                    path,
                                    self.source_name.clone(),
                                    attachment.mime_type.clone(),
                                    EmailMetadata::from(attachment.email_info.clone()),
                                ));
                            }
                            Err(e) => {
                                error!(
                                    "Failed to save attachment '{}' from UID {}: {}",
                                    attachment.filename, uid, e
                                );
                            }
                        }
                    }

                    // Mark as processed
                    if let Some(tracker) = tracker.as_ref() {
                        if let Err(e) = tracker.mark_processed(uid, None) {
                            error!("Failed to mark UID {} as processed: {}", uid, e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to process email UID {}: {}", uid, e);
                }
            }
        }

        // Disconnect cleanly
        client.disconnect().await?;

        info!(
            "Scan complete: {} jobs created from source '{}'",
            jobs.len(),
            self.source_name
        );

        Ok(jobs)
    }

    /// Gets the UIDs to process based on tracking and date filters.
    async fn get_uids_to_process(
        &self,
        client: &mut ImapClient,
        tracker: Option<&EmailTracker>,
    ) -> Result<Vec<u32>> {
        let uids = if let Some(since_date) = &self.config.since_date {
            // Search by date
            let imap_date = parse_since_date(since_date)?;
            client.search_since_date(&imap_date).await?
        } else if let Some(tracker) = tracker {
            // Search since last processed UID
            if let Some(last_uid) = tracker.last_processed_uid()? {
                client.search_since_uid(last_uid).await?
            } else {
                // First run - get all UIDs (limited by batch size later)
                client.search_since_uid(0).await?
            }
        } else {
            // No tracking, no date filter - get recent messages
            // This will be limited by batch size
            client.search_since_uid(0).await?
        };

        // Filter out already processed UIDs
        if let Some(tracker) = tracker {
            tracker.filter_unprocessed(uids)
        } else {
            Ok(uids)
        }
    }

    /// Processes a single email and extracts attachments.
    async fn process_email(
        &self,
        uid: u32,
        raw_email: &[u8],
        parser: &EmailParser,
    ) -> Result<Vec<ExtractedAttachment>> {
        debug!("Processing email UID {}", uid);
        parser.extract_attachments(raw_email, uid)
    }

    /// Saves an attachment to the temp directory and returns its path.
    async fn save_attachment(&self, attachment: &ExtractedAttachment) -> Result<PathBuf> {
        // Ensure temp directory exists
        tokio::fs::create_dir_all(&self.temp_dir).await?;

        // Generate unique filename
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let unique_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
        let filename = format!(
            "{}_{}_{}_{}",
            self.source_name, timestamp, unique_id, attachment.filename
        );

        let path = self.temp_dir.join(&filename);

        debug!("Saving attachment to {}", path.display());
        tokio::fs::write(&path, &attachment.content).await?;

        Ok(path)
    }

    /// Returns the source name.
    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    /// Returns the configuration.
    pub fn config(&self) -> &EmailSourceConfig {
        &self.config
    }
}

/// Parses a since_date string into IMAP date format (DD-Mon-YYYY).
fn parse_since_date(date_str: &str) -> Result<String> {
    // Try to parse as ISO 8601 (e.g., "2024-01-15T00:00:00Z")
    if let Ok(dt) = DateTime::parse_from_rfc3339(date_str) {
        return Ok(dt.format("%d-%b-%Y").to_string());
    }

    // Try to parse as simple date (e.g., "2024-01-15")
    if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        return Ok(date.format("%d-%b-%Y").to_string());
    }

    Err(EmailError::ConfigError(format!(
        "Invalid since_date format: '{}'. Expected ISO 8601 or YYYY-MM-DD",
        date_str
    )))
}

/// Result of a scan operation.
#[derive(Debug)]
pub struct ScanResult {
    /// Number of emails processed.
    pub emails_processed: usize,
    /// Number of attachments extracted.
    pub attachments_extracted: usize,
    /// Number of jobs created.
    pub jobs_created: usize,
    /// Errors encountered (non-fatal).
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_since_date_iso8601() {
        let result = parse_since_date("2024-01-15T00:00:00Z").unwrap();
        assert_eq!(result, "15-Jan-2024");
    }

    #[test]
    fn test_parse_since_date_simple() {
        let result = parse_since_date("2024-01-15").unwrap();
        assert_eq!(result, "15-Jan-2024");
    }

    #[test]
    fn test_parse_since_date_invalid() {
        let result = parse_since_date("invalid-date");
        assert!(result.is_err());
    }

    #[test]
    fn test_scanner_creation() {
        use crate::gitops::resource::{AttachmentFilters, EmailAuthSettings, EmailAuthType};

        let config = EmailSourceConfig {
            host: "imap.example.com".to_string(),
            port: 993,
            use_tls: true,
            username: "test@example.com".to_string(),
            auth: EmailAuthSettings {
                auth_type: EmailAuthType::Password,
                password_env_var: Some("TEST_PASSWORD".to_string()),
                password_insecure: None,
                password_file: None,
                oauth2: None,
            },
            folder: "INBOX".to_string(),
            since_date: None,
            mime_filters: AttachmentFilters::default(),
            min_attachment_size: 0,
            max_attachment_size: 52_428_800,
            poll_interval: 300,
            batch_size: 50,
        };

        let scanner = EmailSourceScanner::new(
            "test-source".to_string(),
            config,
            PathBuf::from("/tmp/test"),
        );

        assert_eq!(scanner.source_name(), "test-source");
        assert_eq!(scanner.config().host, "imap.example.com");
    }
}
