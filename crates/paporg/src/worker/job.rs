use std::fmt::Write;
use std::path::{Path, PathBuf};

/// Email metadata extracted from the source email (if applicable).
#[derive(Debug, Clone, Default)]
pub struct EmailMetadata {
    /// The email's Subject header.
    pub subject: Option<String>,
    /// The email's From header (e.g., "John Doe <john@example.com>").
    pub from: Option<String>,
    /// The email's To header.
    pub to: Option<String>,
    /// The email's Date header in RFC3339 format.
    pub date: Option<String>,
    /// The email's Message-ID header.
    pub message_id: Option<String>,
}

impl EmailMetadata {
    /// Creates a formatted header block for prepending to extracted text.
    /// This allows rules to match on email metadata (from, to, subject, etc.).
    pub fn to_header_block(&self) -> String {
        // Start with empty string - Rust's String will efficiently grow as needed
        // Avoiding magic numbers for pre-allocation since field lengths vary
        let mut output = String::new();
        output.push_str("=== EMAIL METADATA ===\n");

        // Note: writeln! returns a Result, but writing to a String cannot fail,
        // so we intentionally discard the Result with `let _ = ...`
        if let Some(from) = &self.from {
            let _ = writeln!(output, "From: {}", from);
        }
        if let Some(to) = &self.to {
            let _ = writeln!(output, "To: {}", to);
        }
        if let Some(subject) = &self.subject {
            let _ = writeln!(output, "Subject: {}", subject);
        }
        if let Some(date) = &self.date {
            let _ = writeln!(output, "Date: {}", date);
        }
        if let Some(message_id) = &self.message_id {
            let _ = writeln!(output, "Message-ID: {}", message_id);
        }

        output.push_str("======================\n\n");
        output
    }

    /// Returns true if this metadata has any meaningful content.
    pub fn has_content(&self) -> bool {
        self.from.is_some()
            || self.to.is_some()
            || self.subject.is_some()
            || self.date.is_some()
            || self.message_id.is_some()
    }
}

impl From<crate::email::parser::EmailInfo> for EmailMetadata {
    fn from(info: crate::email::parser::EmailInfo) -> Self {
        Self {
            subject: info.subject,
            from: info.from,
            to: info.to,
            date: info.date,
            message_id: info.message_id,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Job {
    pub id: String,
    pub source_path: PathBuf,
    /// The name of the ImportSource that discovered this job (if any).
    pub source_name: Option<String>,
    /// MIME type of the source file (e.g., "application/pdf", "image/png").
    pub mime_type: Option<String>,
    /// Email metadata if this job originated from an email source.
    pub email_metadata: Option<EmailMetadata>,
}

impl Job {
    /// Internal constructor used by all public constructors.
    fn new_internal(
        source_path: PathBuf,
        source_name: Option<String>,
        mime_type: Option<String>,
        email_metadata: Option<EmailMetadata>,
    ) -> Self {
        // Use provided mime_type or detect from path
        let mime_type = mime_type.or_else(|| Self::detect_mime_type(&source_path));
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            source_path,
            source_name,
            mime_type,
            email_metadata,
        }
    }

    /// Creates a new job without a source name.
    pub fn new(source_path: PathBuf) -> Self {
        Self::new_internal(source_path, None, None, None)
    }

    /// Creates a new job with an associated source name.
    pub fn new_with_source(source_path: PathBuf, source_name: String) -> Self {
        Self::new_internal(source_path, Some(source_name), None, None)
    }

    /// Creates a new job with an associated source name.
    ///
    /// # Deprecated
    /// Use `new_with_source` instead. This method is kept for backwards compatibility.
    #[inline]
    #[deprecated(since = "0.2.0", note = "Use `new_with_source` instead")]
    pub fn with_source(source_path: PathBuf, source_name: String) -> Self {
        Self::new_with_source(source_path, source_name)
    }

    /// Creates a new job with an associated source name and explicit MIME type.
    pub fn with_source_and_mime(
        source_path: PathBuf,
        source_name: String,
        mime_type: String,
    ) -> Self {
        Self::new_internal(source_path, Some(source_name), Some(mime_type), None)
    }

    /// Creates a new job from an email source with full metadata.
    pub fn from_email(
        source_path: PathBuf,
        source_name: String,
        mime_type: String,
        email_metadata: EmailMetadata,
    ) -> Self {
        Self::new_internal(
            source_path,
            Some(source_name),
            Some(mime_type),
            Some(email_metadata),
        )
    }

    /// Detects MIME type from file path using the mime_guess crate.
    /// Returns `None` for unknown extensions.
    fn detect_mime_type(path: &Path) -> Option<String> {
        mime_guess::from_path(path).first().map(|m| m.to_string())
    }
}

#[derive(Debug)]
pub struct JobResult {
    pub job_id: String,
    pub source_path: PathBuf,
    pub success: bool,
    pub output_path: Option<PathBuf>,
    pub archive_path: Option<PathBuf>,
    pub symlinks: Vec<PathBuf>,
    pub category: String,
    pub error: Option<String>,
}

impl JobResult {
    pub fn success(
        job: &Job,
        output_path: PathBuf,
        archive_path: PathBuf,
        symlinks: Vec<PathBuf>,
        category: String,
    ) -> Self {
        Self {
            job_id: job.id.clone(),
            source_path: job.source_path.clone(),
            success: true,
            output_path: Some(output_path),
            archive_path: Some(archive_path),
            symlinks,
            category,
            error: None,
        }
    }

    pub fn failure(job: &Job, error: String) -> Self {
        Self {
            job_id: job.id.clone(),
            source_path: job.source_path.clone(),
            success: false,
            output_path: None,
            archive_path: None,
            symlinks: vec![],
            category: String::new(),
            error: Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_metadata_default() {
        let metadata = EmailMetadata::default();
        assert!(metadata.subject.is_none());
        assert!(metadata.from.is_none());
        assert!(metadata.to.is_none());
        assert!(metadata.date.is_none());
        assert!(metadata.message_id.is_none());
        assert!(!metadata.has_content());
    }

    #[test]
    fn test_email_metadata_has_content() {
        let mut metadata = EmailMetadata::default();
        assert!(!metadata.has_content());

        metadata.from = Some("test@example.com".to_string());
        assert!(metadata.has_content());
    }

    #[test]
    fn test_email_metadata_header_block() {
        let metadata = EmailMetadata {
            subject: Some("Test Subject".to_string()),
            from: Some("sender@example.com".to_string()),
            to: Some("recipient@example.com".to_string()),
            date: Some("2024-01-15T10:30:00Z".to_string()),
            message_id: Some("<msg123@example.com>".to_string()),
        };

        let header = metadata.to_header_block();
        assert!(header.contains("=== EMAIL METADATA ==="));
        assert!(header.contains("From: sender@example.com"));
        assert!(header.contains("To: recipient@example.com"));
        assert!(header.contains("Subject: Test Subject"));
        assert!(header.contains("Date: 2024-01-15T10:30:00Z"));
        assert!(header.contains("Message-ID: <msg123@example.com>"));
        assert!(header.contains("======================"));
    }

    #[test]
    fn test_email_metadata_header_block_partial() {
        let metadata = EmailMetadata {
            subject: Some("Test".to_string()),
            from: None,
            to: None,
            date: None,
            message_id: None,
        };

        let header = metadata.to_header_block();
        assert!(header.contains("Subject: Test"));
        assert!(!header.contains("From:"));
        assert!(!header.contains("To:"));
    }

    #[test]
    fn test_job_new() {
        let job = Job::new(PathBuf::from("/test/document.pdf"));
        assert!(!job.id.is_empty());
        assert_eq!(job.source_path, PathBuf::from("/test/document.pdf"));
        assert!(job.source_name.is_none());
        assert_eq!(job.mime_type, Some("application/pdf".to_string()));
        assert!(job.email_metadata.is_none());
    }

    #[test]
    fn test_job_with_source() {
        let job = Job::with_source(PathBuf::from("/test/image.png"), "test-source".to_string());
        assert_eq!(job.source_name, Some("test-source".to_string()));
        assert_eq!(job.mime_type, Some("image/png".to_string()));
    }

    #[test]
    fn test_job_with_source_and_mime() {
        let job = Job::with_source_and_mime(
            PathBuf::from("/test/file"),
            "test-source".to_string(),
            "application/octet-stream".to_string(),
        );
        // Explicit mime type overrides detection
        assert_eq!(job.mime_type, Some("application/octet-stream".to_string()));
    }

    #[test]
    fn test_job_from_email() {
        let metadata = EmailMetadata {
            subject: Some("Invoice".to_string()),
            from: Some("sender@test.com".to_string()),
            to: None,
            date: None,
            message_id: None,
        };

        let job = Job::from_email(
            PathBuf::from("/tmp/invoice.pdf"),
            "gmail-inbox".to_string(),
            "application/pdf".to_string(),
            metadata,
        );

        assert_eq!(job.source_name, Some("gmail-inbox".to_string()));
        assert_eq!(job.mime_type, Some("application/pdf".to_string()));
        assert!(job.email_metadata.is_some());
        let meta = job.email_metadata.unwrap();
        assert_eq!(meta.subject, Some("Invoice".to_string()));
    }

    #[test]
    fn test_job_mime_type_detection() {
        // PDF
        let job = Job::new(PathBuf::from("test.pdf"));
        assert_eq!(job.mime_type, Some("application/pdf".to_string()));

        // PNG
        let job = Job::new(PathBuf::from("test.png"));
        assert_eq!(job.mime_type, Some("image/png".to_string()));

        // JPEG
        let job = Job::new(PathBuf::from("test.jpg"));
        assert_eq!(job.mime_type, Some("image/jpeg".to_string()));

        // Unknown extension
        let job = Job::new(PathBuf::from("test.xyz123"));
        assert!(job.mime_type.is_none());
    }

    #[test]
    fn test_job_result_success() {
        let job = Job::new(PathBuf::from("/test/doc.pdf"));
        let result = JobResult::success(
            &job,
            PathBuf::from("/output/doc.pdf"),
            PathBuf::from("/archive/doc.pdf"),
            vec![PathBuf::from("/link/doc.pdf")],
            "invoices".to_string(),
        );

        assert!(result.success);
        assert_eq!(result.job_id, job.id);
        assert!(result.output_path.is_some());
        assert!(result.archive_path.is_some());
        assert_eq!(result.category, "invoices");
        assert!(result.error.is_none());
    }

    #[test]
    fn test_job_result_failure() {
        let job = Job::new(PathBuf::from("/test/doc.pdf"));
        let result = JobResult::failure(&job, "Test error".to_string());

        assert!(!result.success);
        assert!(result.output_path.is_none());
        assert_eq!(result.error, Some("Test error".to_string()));
    }
}
