//! Broadcasting modules for real-time event streaming.
//!
//! This module contains broadcasters for various event types that can be used
//! by both Tauri desktop apps and any other integration.

pub mod git_progress;
pub mod job_progress;
pub mod job_store;
pub mod log_broadcaster;

pub use git_progress::GitProgressBroadcaster;
pub use job_progress::{
    JobPhase, JobProgressBroadcaster, JobProgressEvent, JobProgressTracker, JobStatus,
};
pub use job_store::{JobListResponse, JobQueryParams, JobStore, StoredJob};
pub use log_broadcaster::{BroadcastingLogWriter, LogBroadcaster, LogEvent};
