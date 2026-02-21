use std::sync::Arc;

use tokio::sync::broadcast;

use crate::broadcast::job_progress::{JobPhase, JobProgressEvent, JobProgressTracker};

/// Events emitted by the pipeline during processing.
/// OCR text is omitted from broadcast events (can be large).
pub enum ProgressEvent {
    Phase {
        phase: JobPhase,
        message: String,
    },
    Completed {
        output_path: String,
        archive_path: String,
        symlinks: Vec<String>,
        category: String,
    },
    Failed {
        error: String,
    },
}

pub trait ProgressReporter: Send + Sync {
    fn report(&self, event: ProgressEvent);
}

/// No-op reporter for unit tests.
pub struct NoopProgress;

impl ProgressReporter for NoopProgress {
    fn report(&self, _event: ProgressEvent) {}
}

/// Wraps existing JobProgressTracker to bridge pipeline events to broadcast channel.
pub struct BroadcastProgress {
    tracker: JobProgressTracker,
    ocr_text: std::sync::Mutex<Option<String>>,
}

impl BroadcastProgress {
    pub fn new(
        job_id: &str,
        filename: &str,
        source_path: &str,
        source_name: Option<&str>,
        mime_type: Option<&str>,
        sender: Arc<broadcast::Sender<JobProgressEvent>>,
    ) -> Self {
        let tracker = JobProgressTracker::with_source(
            job_id,
            filename,
            source_path,
            source_name,
            mime_type,
            sender,
        );
        Self {
            tracker,
            ocr_text: std::sync::Mutex::new(None),
        }
    }

    /// Store OCR text separately (not sent via broadcast, can be large).
    pub fn set_ocr_text(&self, text: String) {
        if let Ok(mut guard) = self.ocr_text.lock() {
            *guard = Some(text);
        }
    }

    /// Retrieve stored OCR text for persistence.
    pub fn take_ocr_text(&self) -> Option<String> {
        self.ocr_text.lock().ok().and_then(|mut g| g.take())
    }
}

impl ProgressReporter for BroadcastProgress {
    fn report(&self, event: ProgressEvent) {
        match event {
            ProgressEvent::Phase { phase, message } => {
                self.tracker.update_phase(phase, &message);
            }
            ProgressEvent::Completed {
                output_path,
                archive_path,
                symlinks,
                category,
            } => {
                let ocr_text = self
                    .ocr_text
                    .lock()
                    .ok()
                    .and_then(|g| g.clone())
                    .unwrap_or_default();
                self.tracker.completed(
                    &output_path,
                    &archive_path,
                    &symlinks,
                    &category,
                    &ocr_text,
                );
            }
            ProgressEvent::Failed { error } => {
                self.tracker.failed(&error);
            }
        }
    }
}
