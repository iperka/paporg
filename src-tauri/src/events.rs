//! Event bridge between paporg library and Tauri frontend.

use std::sync::Arc;

use log::{debug, info, warn};
use paporg::broadcast::{JobProgressEvent, LogEvent};
use paporg::gitops::progress::GitProgressEvent;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::RwLock;

use crate::state::TauriAppState;

/// Event names for Tauri events.
pub mod event_names {
    pub const LOG: &str = "paporg://log";
    pub const JOB_PROGRESS: &str = "paporg://job-progress";
    pub const GIT_PROGRESS: &str = "paporg://git-progress";
    pub const CONFIG_CHANGED: &str = "paporg://config-changed";
    pub const WORKER_STATUS: &str = "paporg://worker-status";
}

/// Log event payload for the frontend (serializable wrapper).
#[derive(Debug, Clone, Serialize)]
pub struct LogEventPayload {
    pub level: String,
    pub target: String,
    pub message: String,
    pub timestamp: String,
}

impl From<LogEvent> for LogEventPayload {
    fn from(event: LogEvent) -> Self {
        Self {
            level: event.level,
            target: event.target,
            message: event.message,
            timestamp: event.timestamp.to_rfc3339(),
        }
    }
}

/// Starts the event bridge that listens to paporg events and emits Tauri events.
pub async fn start_event_bridge(app_handle: AppHandle) {
    info!("Starting event bridge");

    let state: &Arc<RwLock<TauriAppState>> =
        app_handle.state::<Arc<RwLock<TauriAppState>>>().inner();

    // Clone what we need for the async tasks
    let (log_broadcaster, job_broadcaster, git_broadcaster) = {
        let state = state.read().await;
        (
            state.log_broadcaster.clone(),
            state.job_broadcaster.clone(),
            state.git_broadcaster.clone(),
        )
    };

    // Spawn log event listener
    let app_clone = app_handle.clone();
    let mut log_rx = log_broadcaster.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match log_rx.recv().await {
                Ok(log_entry) => {
                    let payload = LogEventPayload::from(log_entry);
                    if let Err(e) = app_clone.emit(event_names::LOG, &payload) {
                        debug!("Failed to emit log event: {}", e);
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!("Log event bridge lagged, missed {} events", n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    info!("Log broadcaster closed, stopping log event bridge");
                    break;
                }
            }
        }
    });

    // Spawn job progress event listener
    let app_clone = app_handle.clone();
    let job_store = {
        let state = state.read().await;
        state.job_store.clone()
    };
    let mut job_rx = job_broadcaster.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match job_rx.recv().await {
                Ok(event) => {
                    // Update the job store with the event and persist to database
                    job_store.update_and_persist(&event).await;

                    // Emit to frontend
                    if let Err(e) = app_clone.emit(event_names::JOB_PROGRESS, &event) {
                        debug!("Failed to emit job progress event: {}", e);
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!("Job progress event bridge lagged, missed {} events", n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    info!("Job progress broadcaster closed, stopping job event bridge");
                    break;
                }
            }
        }
    });

    // Spawn git progress event listener
    let app_clone = app_handle.clone();
    let mut git_rx = git_broadcaster.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match git_rx.recv().await {
                Ok(event) => {
                    if let Err(e) = app_clone.emit(event_names::GIT_PROGRESS, &event) {
                        debug!("Failed to emit git progress event: {}", e);
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!("Git progress event bridge lagged, missed {} events", n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    info!("Git progress broadcaster closed, stopping git event bridge");
                    break;
                }
            }
        }
    });

    info!("Event bridge started");
}

/// Emits a job progress event to the frontend.
#[allow(dead_code)]
pub fn emit_job_progress(app_handle: &AppHandle, event: JobProgressEvent) {
    if let Err(e) = app_handle.emit(event_names::JOB_PROGRESS, &event) {
        debug!("Failed to emit job progress event: {}", e);
    }
}

/// Emits a git progress event to the frontend.
#[allow(dead_code)]
pub fn emit_git_progress(app_handle: &AppHandle, event: GitProgressEvent) {
    if let Err(e) = app_handle.emit(event_names::GIT_PROGRESS, &event) {
        debug!("Failed to emit git progress event: {}", e);
    }
}

/// Emits a config changed event to the frontend.
pub fn emit_config_changed(app_handle: &AppHandle) {
    if let Err(e) = app_handle.emit(event_names::CONFIG_CHANGED, ()) {
        debug!("Failed to emit config changed event: {}", e);
    }
}

/// Emits a worker status change event to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct WorkerStatusEvent {
    pub running: bool,
}

pub fn emit_worker_status(app_handle: &AppHandle, running: bool) {
    let event = WorkerStatusEvent { running };
    if let Err(e) = app_handle.emit(event_names::WORKER_STATUS, &event) {
        debug!("Failed to emit worker status event: {}", e);
    }
}
