//! Git operation progress tracking for real-time SSE updates.

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock};
use tokio::sync::broadcast;
use uuid::Uuid;

// Pre-compiled regexes for parsing git progress output
static RE_PERCENTAGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+)%").unwrap());
static RE_COUNT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\((\d+)/(\d+)\)").unwrap());
static RE_BYTES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([\d.]+)\s*(bytes?|[KMGT]iB|[KMGT]B)").unwrap());
static RE_SPEED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\|\s*([\d.]+)\s*([KMGT]?i?B)/s").unwrap());

/// Type of git operation being performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GitOperationType {
    Commit,
    Push,
    Pull,
    Fetch,
    Merge,
    Checkout,
    Initialize,
}

impl std::fmt::Display for GitOperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitOperationType::Commit => write!(f, "commit"),
            GitOperationType::Push => write!(f, "push"),
            GitOperationType::Pull => write!(f, "pull"),
            GitOperationType::Fetch => write!(f, "fetch"),
            GitOperationType::Merge => write!(f, "merge"),
            GitOperationType::Checkout => write!(f, "checkout"),
            GitOperationType::Initialize => write!(f, "initialize"),
        }
    }
}

/// Phase of a git operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitOperationPhase {
    /// Operation is starting.
    Starting,
    /// Staging files for commit.
    StagingFiles,
    /// Creating commit.
    Committing,
    /// Counting objects (push/pull).
    Counting,
    /// Compressing objects (push).
    Compressing,
    /// Writing objects (push).
    Writing,
    /// Receiving objects (pull/fetch).
    Receiving,
    /// Resolving deltas (pull/fetch).
    Resolving,
    /// Unpacking objects (pull/fetch).
    Unpacking,
    /// Push in progress.
    Pushing,
    /// Pull in progress.
    Pulling,
    /// Fetching from remote.
    Fetching,
    /// Merging branches.
    Merging,
    /// Checking out branch.
    CheckingOut,
    /// Operation completed successfully.
    Completed,
    /// Operation failed.
    Failed,
}

impl std::fmt::Display for GitOperationPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitOperationPhase::Starting => write!(f, "Starting..."),
            GitOperationPhase::StagingFiles => write!(f, "Staging files..."),
            GitOperationPhase::Committing => write!(f, "Committing..."),
            GitOperationPhase::Counting => write!(f, "Counting objects..."),
            GitOperationPhase::Compressing => write!(f, "Compressing objects..."),
            GitOperationPhase::Writing => write!(f, "Writing objects..."),
            GitOperationPhase::Receiving => write!(f, "Receiving objects..."),
            GitOperationPhase::Resolving => write!(f, "Resolving deltas..."),
            GitOperationPhase::Unpacking => write!(f, "Unpacking objects..."),
            GitOperationPhase::Pushing => write!(f, "Pushing..."),
            GitOperationPhase::Pulling => write!(f, "Pulling..."),
            GitOperationPhase::Fetching => write!(f, "Fetching..."),
            GitOperationPhase::Merging => write!(f, "Merging..."),
            GitOperationPhase::CheckingOut => write!(f, "Checking out..."),
            GitOperationPhase::Completed => write!(f, "Completed"),
            GitOperationPhase::Failed => write!(f, "Failed"),
        }
    }
}

/// A git operation progress event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitProgressEvent {
    /// Unique identifier for this operation.
    pub operation_id: String,
    /// Type of git operation.
    pub operation_type: GitOperationType,
    /// Current phase of the operation.
    pub phase: GitOperationPhase,
    /// Human-readable status message.
    pub message: String,
    /// Progress percentage (0-100), if determinable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
    /// Current number of objects processed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<u64>,
    /// Total number of objects.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    /// Bytes transferred (for push/pull).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_transferred: Option<u64>,
    /// Transfer speed in bytes/second.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transfer_speed: Option<u64>,
    /// Raw git output line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_output: Option<String>,
    /// Error message if operation failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Event timestamp.
    pub timestamp: DateTime<Utc>,
}

impl GitProgressEvent {
    /// Creates a new progress event.
    pub fn new(
        operation_id: &str,
        operation_type: GitOperationType,
        phase: GitOperationPhase,
        message: &str,
    ) -> Self {
        Self {
            operation_id: operation_id.to_string(),
            operation_type,
            phase,
            message: message.to_string(),
            progress: None,
            current: None,
            total: None,
            bytes_transferred: None,
            transfer_speed: None,
            raw_output: None,
            error: None,
            timestamp: Utc::now(),
        }
    }

    /// Creates a completion event.
    pub fn completed(operation_id: &str, operation_type: GitOperationType, message: &str) -> Self {
        Self::new(
            operation_id,
            operation_type,
            GitOperationPhase::Completed,
            message,
        )
    }

    /// Creates a failure event.
    pub fn failed(operation_id: &str, operation_type: GitOperationType, error: &str) -> Self {
        let mut event = Self::new(
            operation_id,
            operation_type,
            GitOperationPhase::Failed,
            "Operation failed",
        );
        event.error = Some(error.to_string());
        event
    }

    /// Sets progress information.
    pub fn with_progress(mut self, current: u64, total: u64) -> Self {
        self.current = Some(current);
        self.total = Some(total);
        if total > 0 {
            self.progress = Some(((current * 100) / total).min(100) as u8);
        }
        self
    }

    /// Sets transfer information.
    pub fn with_transfer(mut self, bytes: u64, speed: Option<u64>) -> Self {
        self.bytes_transferred = Some(bytes);
        self.transfer_speed = speed;
        self
    }

    /// Sets raw output.
    pub fn with_raw_output(mut self, output: &str) -> Self {
        self.raw_output = Some(output.to_string());
        self
    }
}

/// Parsed progress information from git output.
#[derive(Debug, Clone, Default)]
pub struct ParsedProgress {
    pub phase: Option<GitOperationPhase>,
    pub current: Option<u64>,
    pub total: Option<u64>,
    pub percentage: Option<u8>,
    pub bytes: Option<u64>,
    pub speed: Option<u64>,
}

/// Parses git stderr output to extract progress information.
///
/// Git progress output patterns:
/// - `Counting objects: 100% (10/10), done.`
/// - `Compressing objects:  50% (5/10)`
/// - `Writing objects:  33% (1/3), 256 bytes | 256.00 KiB/s`
/// - `Receiving objects:  75% (75/100), 1.00 MiB | 512.00 KiB/s`
/// - `Resolving deltas: 100% (5/5), done.`
pub fn parse_git_progress(line: &str) -> ParsedProgress {
    let mut result = ParsedProgress::default();

    // Detect phase from keywords
    let line_lower = line.to_lowercase();
    if line_lower.contains("counting") {
        result.phase = Some(GitOperationPhase::Counting);
    } else if line_lower.contains("compressing") {
        result.phase = Some(GitOperationPhase::Compressing);
    } else if line_lower.contains("writing") {
        result.phase = Some(GitOperationPhase::Writing);
    } else if line_lower.contains("receiving") {
        result.phase = Some(GitOperationPhase::Receiving);
    } else if line_lower.contains("resolving") {
        result.phase = Some(GitOperationPhase::Resolving);
    } else if line_lower.contains("unpacking") {
        result.phase = Some(GitOperationPhase::Unpacking);
    } else if line_lower.contains("enumerating") {
        result.phase = Some(GitOperationPhase::Counting);
    }

    // Parse percentage pattern: "50%" or "100%"
    if let Some(pct_match) = RE_PERCENTAGE.captures(line) {
        if let Some(pct_str) = pct_match.get(1) {
            result.percentage = pct_str.as_str().parse().ok();
        }
    }

    // Parse current/total pattern: "(5/10)" or "(75/100)"
    if let Some(count_match) = RE_COUNT.captures(line) {
        if let (Some(current), Some(total)) = (count_match.get(1), count_match.get(2)) {
            result.current = current.as_str().parse().ok();
            result.total = total.as_str().parse().ok();
        }
    }

    // Parse bytes pattern: "256 bytes" or "1.00 MiB" or "512.00 KiB"
    if let Some(bytes_match) = RE_BYTES.captures(line) {
        if let (Some(num_str), Some(unit)) = (bytes_match.get(1), bytes_match.get(2)) {
            if let Ok(num) = num_str.as_str().parse::<f64>() {
                let multiplier = match unit.as_str().to_lowercase().as_str() {
                    "bytes" | "byte" => 1.0,
                    "kib" | "kb" => 1024.0,
                    "mib" | "mb" => 1024.0 * 1024.0,
                    "gib" | "gb" => 1024.0 * 1024.0 * 1024.0,
                    "tib" | "tb" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
                    _ => 1.0,
                };
                result.bytes = Some((num * multiplier) as u64);
            }
        }
    }

    // Parse speed pattern: "| 512.00 KiB/s" or "| 1.00 MiB/s"
    if let Some(speed_match) = RE_SPEED.captures(line) {
        if let (Some(num_str), Some(unit)) = (speed_match.get(1), speed_match.get(2)) {
            if let Ok(num) = num_str.as_str().parse::<f64>() {
                let multiplier = match unit.as_str().to_lowercase().as_str() {
                    "b" => 1.0,
                    "kib" | "kb" => 1024.0,
                    "mib" | "mb" => 1024.0 * 1024.0,
                    "gib" | "gb" => 1024.0 * 1024.0 * 1024.0,
                    _ => 1024.0, // Default to KiB if unclear
                };
                result.speed = Some((num * multiplier) as u64);
            }
        }
    }

    result
}

/// Tracks progress for a single git operation, with optional cancellation support.
pub struct OperationProgress {
    operation_id: String,
    operation_type: GitOperationType,
    broadcaster: Arc<broadcast::Sender<GitProgressEvent>>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
}

impl OperationProgress {
    /// Creates a new operation progress tracker.
    pub fn new(
        operation_type: GitOperationType,
        broadcaster: Arc<broadcast::Sender<GitProgressEvent>>,
    ) -> Self {
        Self {
            operation_id: Uuid::new_v4().to_string(),
            operation_type,
            broadcaster,
            cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Gets the operation ID.
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    /// Broadcasts a phase update.
    pub fn phase(&self, phase: GitOperationPhase, message: &str) {
        let event = GitProgressEvent::new(&self.operation_id, self.operation_type, phase, message);
        let _ = self.broadcaster.send(event);
    }

    /// Broadcasts a progress update from parsed git output.
    pub fn update_from_output(&self, line: &str) {
        let parsed = parse_git_progress(line);

        if let Some(phase) = parsed.phase {
            let mut event = GitProgressEvent::new(
                &self.operation_id,
                self.operation_type,
                phase,
                &phase.to_string(),
            );

            if let (Some(current), Some(total)) = (parsed.current, parsed.total) {
                event = event.with_progress(current, total);
            } else if let Some(pct) = parsed.percentage {
                event.progress = Some(pct);
            }

            if let Some(bytes) = parsed.bytes {
                event = event.with_transfer(bytes, parsed.speed);
            }

            event = event.with_raw_output(line);
            let _ = self.broadcaster.send(event);
        }
    }

    /// Broadcasts raw output line for logging purposes.
    pub fn raw_output(&self, line: &str) {
        // First try to parse as progress
        self.update_from_output(line);
    }

    /// Broadcasts completion.
    pub fn completed(&self, message: &str) {
        let event = GitProgressEvent::completed(&self.operation_id, self.operation_type, message);
        let _ = self.broadcaster.send(event);
    }

    /// Broadcasts failure.
    pub fn failed(&self, error: &str) {
        let event = GitProgressEvent::failed(&self.operation_id, self.operation_type, error);
        let _ = self.broadcaster.send(event);
    }

    /// Marks this operation as cancelled.
    pub fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Release);
        self.failed("Operation cancelled");
    }

    /// Returns true if the operation has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Returns a clone of the cancellation flag for sharing with async tasks.
    pub fn cancellation_token(&self) -> Arc<std::sync::atomic::AtomicBool> {
        Arc::clone(&self.cancelled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_counting_objects() {
        let parsed = parse_git_progress("Counting objects: 100% (10/10), done.");
        assert_eq!(parsed.phase, Some(GitOperationPhase::Counting));
        assert_eq!(parsed.percentage, Some(100));
        assert_eq!(parsed.current, Some(10));
        assert_eq!(parsed.total, Some(10));
    }

    #[test]
    fn test_parse_compressing_objects() {
        let parsed = parse_git_progress("Compressing objects:  50% (5/10)");
        assert_eq!(parsed.phase, Some(GitOperationPhase::Compressing));
        assert_eq!(parsed.percentage, Some(50));
        assert_eq!(parsed.current, Some(5));
        assert_eq!(parsed.total, Some(10));
    }

    #[test]
    fn test_parse_writing_with_speed() {
        let parsed = parse_git_progress("Writing objects:  33% (1/3), 256 bytes | 256.00 KiB/s");
        assert_eq!(parsed.phase, Some(GitOperationPhase::Writing));
        assert_eq!(parsed.percentage, Some(33));
        assert_eq!(parsed.current, Some(1));
        assert_eq!(parsed.total, Some(3));
        assert_eq!(parsed.bytes, Some(256));
    }

    #[test]
    fn test_parse_receiving_mib() {
        let parsed =
            parse_git_progress("Receiving objects:  75% (75/100), 1.00 MiB | 512.00 KiB/s");
        assert_eq!(parsed.phase, Some(GitOperationPhase::Receiving));
        assert_eq!(parsed.percentage, Some(75));
        assert_eq!(parsed.current, Some(75));
        assert_eq!(parsed.total, Some(100));
        assert_eq!(parsed.bytes, Some(1024 * 1024));
    }

    #[test]
    fn test_parse_resolving_deltas() {
        let parsed = parse_git_progress("Resolving deltas: 100% (5/5), done.");
        assert_eq!(parsed.phase, Some(GitOperationPhase::Resolving));
        assert_eq!(parsed.percentage, Some(100));
        assert_eq!(parsed.current, Some(5));
        assert_eq!(parsed.total, Some(5));
    }

    #[test]
    fn test_operation_type_display() {
        assert_eq!(GitOperationType::Commit.to_string(), "commit");
        assert_eq!(GitOperationType::Push.to_string(), "push");
        assert_eq!(GitOperationType::Pull.to_string(), "pull");
    }

    #[test]
    fn test_operation_phase_display() {
        assert_eq!(
            GitOperationPhase::StagingFiles.to_string(),
            "Staging files..."
        );
        assert_eq!(GitOperationPhase::Completed.to_string(), "Completed");
    }

    #[test]
    fn test_progress_event_creation() {
        let event = GitProgressEvent::new(
            "op-123",
            GitOperationType::Push,
            GitOperationPhase::Writing,
            "Test message",
        );
        assert_eq!(event.operation_id, "op-123");
        assert_eq!(event.operation_type, GitOperationType::Push);
        assert_eq!(event.phase, GitOperationPhase::Writing);
        assert_eq!(event.message, "Test message");
    }

    #[test]
    fn test_progress_event_with_progress() {
        let event = GitProgressEvent::new(
            "op-123",
            GitOperationType::Push,
            GitOperationPhase::Writing,
            "Test",
        )
        .with_progress(50, 100);
        assert_eq!(event.progress, Some(50));
        assert_eq!(event.current, Some(50));
        assert_eq!(event.total, Some(100));
    }

    #[test]
    fn test_progress_event_completed() {
        let event = GitProgressEvent::completed("op-123", GitOperationType::Commit, "Success");
        assert_eq!(event.phase, GitOperationPhase::Completed);
        assert_eq!(event.message, "Success");
    }

    #[test]
    fn test_progress_event_failed() {
        let event = GitProgressEvent::failed("op-123", GitOperationType::Push, "Network error");
        assert_eq!(event.phase, GitOperationPhase::Failed);
        assert_eq!(event.error, Some("Network error".to_string()));
    }
}
