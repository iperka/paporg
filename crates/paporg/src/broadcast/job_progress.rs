//! Job progress broadcaster for real-time job status streaming.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// Phase of job processing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobPhase {
    Queued,
    Processing,
    ExtractVariables,
    Categorizing,
    Substituting,
    Storing,
    CreatingSymlinks,
    Archiving,
    Completed,
    Failed,
}

impl std::fmt::Display for JobPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobPhase::Queued => write!(f, "Queued"),
            JobPhase::Processing => write!(f, "Processing"),
            JobPhase::ExtractVariables => write!(f, "Extracting variables"),
            JobPhase::Categorizing => write!(f, "Categorizing"),
            JobPhase::Substituting => write!(f, "Substituting variables"),
            JobPhase::Storing => write!(f, "Storing"),
            JobPhase::CreatingSymlinks => write!(f, "Creating symlinks"),
            JobPhase::Archiving => write!(f, "Archiving"),
            JobPhase::Completed => write!(f, "Completed"),
            JobPhase::Failed => write!(f, "Failed"),
        }
    }
}

/// Status of a job.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Processing,
    Completed,
    Failed,
}

/// Progress event for a job.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobProgressEvent {
    /// Unique job identifier.
    pub job_id: String,
    /// Original filename being processed.
    pub filename: String,
    /// Current phase of processing.
    pub phase: JobPhase,
    /// Overall job status.
    pub status: JobStatus,
    /// Human-readable message describing current activity.
    pub message: String,
    /// Timestamp of this event.
    pub timestamp: DateTime<Utc>,
    /// Output path (set on completion).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    /// Archive path (set on completion).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_path: Option<String>,
    /// Created symlinks (set on completion).
    #[serde(default)]
    pub symlinks: Vec<String>,
    /// Detected category (set on completion).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Error message (set on failure).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Extracted OCR text (set on completion).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ocr_text: Option<String>,
    /// Source path of the file being processed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    /// Name of the import source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
    /// MIME type of the source file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

impl JobProgressEvent {
    /// Creates a new progress event.
    pub fn new(job_id: &str, filename: &str, phase: JobPhase, message: &str) -> Self {
        let status = match phase {
            JobPhase::Completed => JobStatus::Completed,
            JobPhase::Failed => JobStatus::Failed,
            _ => JobStatus::Processing,
        };

        Self {
            job_id: job_id.to_string(),
            filename: filename.to_string(),
            phase,
            status,
            message: message.to_string(),
            timestamp: Utc::now(),
            output_path: None,
            archive_path: None,
            symlinks: vec![],
            category: None,
            error: None,
            ocr_text: None,
            source_path: None,
            source_name: None,
            mime_type: None,
        }
    }

    /// Creates a completion event.
    pub fn completed(
        job_id: &str,
        filename: &str,
        output_path: &str,
        archive_path: &str,
        symlinks: &[String],
        category: &str,
        ocr_text: &str,
    ) -> Self {
        Self {
            job_id: job_id.to_string(),
            filename: filename.to_string(),
            phase: JobPhase::Completed,
            status: JobStatus::Completed,
            message: "Processing completed successfully".to_string(),
            timestamp: Utc::now(),
            output_path: Some(output_path.to_string()),
            archive_path: Some(archive_path.to_string()),
            symlinks: symlinks.to_vec(),
            category: Some(category.to_string()),
            error: None,
            ocr_text: Some(ocr_text.to_string()),
            source_path: None,
            source_name: None,
            mime_type: None,
        }
    }

    /// Creates a failure event.
    pub fn failed(job_id: &str, filename: &str, error: &str) -> Self {
        Self {
            job_id: job_id.to_string(),
            filename: filename.to_string(),
            phase: JobPhase::Failed,
            status: JobStatus::Failed,
            message: "Processing failed".to_string(),
            timestamp: Utc::now(),
            output_path: None,
            archive_path: None,
            symlinks: vec![],
            category: None,
            error: Some(error.to_string()),
            ocr_text: None,
            source_path: None,
            source_name: None,
            mime_type: None,
        }
    }
}

/// Broadcasts job progress events for streaming.
#[derive(Clone)]
pub struct JobProgressBroadcaster {
    sender: Arc<broadcast::Sender<JobProgressEvent>>,
}

impl JobProgressBroadcaster {
    /// Creates a new job progress broadcaster with the specified channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender: Arc::new(sender),
        }
    }

    /// Sends a progress event to all subscribers.
    pub fn send(&self, event: JobProgressEvent) {
        // Ignore errors - no active receivers is fine
        let _ = self.sender.send(event);
    }

    /// Creates a new subscriber for progress events.
    pub fn subscribe(&self) -> broadcast::Receiver<JobProgressEvent> {
        self.sender.subscribe()
    }

    /// Creates a new job progress tracker for a processing job.
    pub fn start_job(&self, job_id: &str, filename: &str) -> JobProgressTracker {
        let tracker = JobProgressTracker::new(job_id, filename, Arc::clone(&self.sender));

        // Send initial queued event
        tracker.update_phase(JobPhase::Queued, "Job queued for processing");

        tracker
    }

    /// Creates a new job progress tracker with source information.
    pub fn start_job_with_source(
        &self,
        job_id: &str,
        filename: &str,
        source_path: &str,
        source_name: Option<&str>,
        mime_type: Option<&str>,
    ) -> JobProgressTracker {
        let tracker = JobProgressTracker::with_source(
            job_id,
            filename,
            source_path,
            source_name,
            mime_type,
            Arc::clone(&self.sender),
        );

        // Send initial queued event
        tracker.update_phase(JobPhase::Queued, "Job queued for processing");

        tracker
    }

    /// Gets the inner sender for creating trackers.
    pub fn sender(&self) -> Arc<broadcast::Sender<JobProgressEvent>> {
        Arc::clone(&self.sender)
    }
}

impl Default for JobProgressBroadcaster {
    fn default() -> Self {
        Self::new(100)
    }
}

/// Tracks progress for a single job.
pub struct JobProgressTracker {
    job_id: String,
    filename: String,
    source_path: Option<String>,
    source_name: Option<String>,
    mime_type: Option<String>,
    sender: Arc<broadcast::Sender<JobProgressEvent>>,
}

impl JobProgressTracker {
    /// Creates a new job progress tracker.
    pub fn new(
        job_id: &str,
        filename: &str,
        sender: Arc<broadcast::Sender<JobProgressEvent>>,
    ) -> Self {
        Self {
            job_id: job_id.to_string(),
            filename: filename.to_string(),
            source_path: None,
            source_name: None,
            mime_type: None,
            sender,
        }
    }

    /// Creates a new job progress tracker with source information.
    pub fn with_source(
        job_id: &str,
        filename: &str,
        source_path: &str,
        source_name: Option<&str>,
        mime_type: Option<&str>,
        sender: Arc<broadcast::Sender<JobProgressEvent>>,
    ) -> Self {
        Self {
            job_id: job_id.to_string(),
            filename: filename.to_string(),
            source_path: Some(source_path.to_string()),
            source_name: source_name.map(|s| s.to_string()),
            mime_type: mime_type.map(|s| s.to_string()),
            sender,
        }
    }

    /// Adds source information to events.
    fn add_source_info(&self, mut event: JobProgressEvent) -> JobProgressEvent {
        event.source_path = self.source_path.clone();
        event.source_name = self.source_name.clone();
        event.mime_type = self.mime_type.clone();
        event
    }

    /// Updates the current phase with a message.
    pub fn update_phase(&self, phase: JobPhase, message: &str) {
        let event = JobProgressEvent::new(&self.job_id, &self.filename, phase, message);
        let event = self.add_source_info(event);
        let _ = self.sender.send(event);
    }

    /// Marks the job as completed with result details.
    pub fn completed(
        &self,
        output_path: &str,
        archive_path: &str,
        symlinks: &[String],
        category: &str,
        ocr_text: &str,
    ) {
        let event = JobProgressEvent::completed(
            &self.job_id,
            &self.filename,
            output_path,
            archive_path,
            symlinks,
            category,
            ocr_text,
        );
        let event = self.add_source_info(event);
        let _ = self.sender.send(event);
    }

    /// Marks the job as failed with an error message.
    pub fn failed(&self, error: &str) {
        let event = JobProgressEvent::failed(&self.job_id, &self.filename, error);
        let event = self.add_source_info(event);
        let _ = self.sender.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_broadcaster_creation() {
        let broadcaster = JobProgressBroadcaster::new(10);
        let _rx = broadcaster.subscribe();
    }

    #[test]
    fn test_broadcaster_send_receive() {
        let broadcaster = JobProgressBroadcaster::new(10);
        let mut rx = broadcaster.subscribe();

        let event = JobProgressEvent::new("test-job", "test.pdf", JobPhase::Processing, "Testing");

        broadcaster.send(event);

        let received = rx.try_recv().unwrap();
        assert_eq!(received.job_id, "test-job");
        assert_eq!(received.filename, "test.pdf");
        assert_eq!(received.phase, JobPhase::Processing);
        assert_eq!(received.status, JobStatus::Processing);
    }

    #[test]
    fn test_start_job() {
        let broadcaster = JobProgressBroadcaster::new(10);
        let mut rx = broadcaster.subscribe();

        let tracker = broadcaster.start_job("job-1", "document.pdf");

        // Should receive queued event
        let received = rx.try_recv().unwrap();
        assert_eq!(received.job_id, "job-1");
        assert_eq!(received.phase, JobPhase::Queued);

        // Update phase
        tracker.update_phase(JobPhase::Processing, "Running OCR...");

        let received = rx.try_recv().unwrap();
        assert_eq!(received.phase, JobPhase::Processing);
        assert_eq!(received.message, "Running OCR...");
    }

    #[test]
    fn test_job_completion() {
        let broadcaster = JobProgressBroadcaster::new(10);
        let mut rx = broadcaster.subscribe();

        let tracker = broadcaster.start_job("job-2", "invoice.pdf");
        let _ = rx.try_recv(); // Consume queued event

        tracker.completed(
            "/output/invoices/invoice.pdf",
            "/archive/invoice.pdf",
            &["/symlinks/2024/invoice.pdf".to_string()],
            "invoices",
            "Invoice #123\nTotal: $100.00",
        );

        let received = rx.try_recv().unwrap();
        assert_eq!(received.phase, JobPhase::Completed);
        assert_eq!(received.status, JobStatus::Completed);
        assert_eq!(
            received.output_path,
            Some("/output/invoices/invoice.pdf".to_string())
        );
        assert_eq!(received.category, Some("invoices".to_string()));
        assert_eq!(received.symlinks.len(), 1);
        assert_eq!(
            received.ocr_text,
            Some("Invoice #123\nTotal: $100.00".to_string())
        );
    }

    #[test]
    fn test_job_failure() {
        let broadcaster = JobProgressBroadcaster::new(10);
        let mut rx = broadcaster.subscribe();

        let tracker = broadcaster.start_job("job-3", "corrupt.pdf");
        let _ = rx.try_recv(); // Consume queued event

        tracker.failed("Failed to parse PDF: invalid header");

        let received = rx.try_recv().unwrap();
        assert_eq!(received.phase, JobPhase::Failed);
        assert_eq!(received.status, JobStatus::Failed);
        assert_eq!(
            received.error,
            Some("Failed to parse PDF: invalid header".to_string())
        );
    }

    #[test]
    fn test_default_capacity() {
        let broadcaster = JobProgressBroadcaster::default();
        let _rx = broadcaster.subscribe();
    }
}
