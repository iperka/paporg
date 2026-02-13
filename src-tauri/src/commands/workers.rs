//! Worker pool control commands.

use std::sync::Arc;

use serde::Serialize;
use tauri::State;
use tokio::sync::RwLock;

use super::ApiResponse;
use crate::state::TauriAppState;

/// Worker status information.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerStatus {
    pub running: bool,
    pub worker_count: usize,
}

/// Get worker status.
#[tauri::command]
pub async fn get_worker_status(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
) -> Result<ApiResponse<WorkerStatus>, String> {
    let state = state.read().await;

    let worker_count = state
        .config()
        .map(|c| c.to_legacy_config().worker_count)
        .unwrap_or(0);

    Ok(ApiResponse::ok(WorkerStatus {
        running: state.is_workers_running(),
        worker_count,
    }))
}

/// Start workers.
#[tauri::command]
pub async fn start_workers(
    app: tauri::AppHandle,
    state: State<'_, Arc<RwLock<TauriAppState>>>,
) -> Result<ApiResponse<()>, String> {
    let mut state = state.write().await;

    match state.start_workers() {
        Ok(()) => {
            crate::events::emit_worker_status(&app, true);
            Ok(ApiResponse::ok(()))
        }
        Err(e) => Ok(ApiResponse::err(e)),
    }
}

/// Stop workers.
#[tauri::command]
pub async fn stop_workers(
    app: tauri::AppHandle,
    state: State<'_, Arc<RwLock<TauriAppState>>>,
) -> Result<ApiResponse<()>, String> {
    let mut state = state.write().await;
    state.stop_workers();
    crate::events::emit_worker_status(&app, false);
    Ok(ApiResponse::ok(()))
}

/// Trigger document processing.
#[tauri::command]
pub async fn trigger_processing(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
) -> Result<ApiResponse<()>, String> {
    let state = state.read().await;

    if !state.is_workers_running() {
        return Ok(ApiResponse::err("Workers not running"));
    }

    state.trigger_processing();
    Ok(ApiResponse::ok(()))
}
