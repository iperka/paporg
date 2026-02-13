//! Configuration management commands.

use std::sync::Arc;

use serde::Serialize;
use tauri::State;
use tokio::sync::RwLock;

use super::ApiResponse;
use crate::state::TauriAppState;

/// Configuration summary for the frontend.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSummary {
    pub config_dir: Option<String>,
    pub input_directory: Option<String>,
    pub output_directory: Option<String>,
    pub worker_count: Option<usize>,
    pub rules_count: usize,
    pub import_sources_count: usize,
    pub ocr_enabled: bool,
}

/// Health check response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthStatus {
    pub status: String,
    pub version: String,
    pub config_loaded: bool,
    pub workers_running: bool,
}

/// Get current configuration summary.
#[tauri::command]
pub async fn get_config(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
) -> Result<ApiResponse<ConfigSummary>, String> {
    let state = state.read().await;

    let config_dir = state.config_dir.as_ref().map(|p| p.display().to_string());

    if let Some(config) = state.config() {
        let legacy = config.to_legacy_config();
        Ok(ApiResponse::ok(ConfigSummary {
            config_dir,
            input_directory: Some(legacy.input_directory),
            output_directory: Some(legacy.output_directory),
            worker_count: Some(legacy.worker_count),
            rules_count: config.rules.len(),
            import_sources_count: config.import_sources.len(),
            ocr_enabled: legacy.ocr.enabled,
        }))
    } else {
        Ok(ApiResponse::ok(ConfigSummary {
            config_dir,
            input_directory: None,
            output_directory: None,
            worker_count: None,
            rules_count: 0,
            import_sources_count: 0,
            ocr_enabled: false,
        }))
    }
}

/// Reload configuration from disk.
#[tauri::command]
pub async fn reload_config(
    app: tauri::AppHandle,
    state: State<'_, Arc<RwLock<TauriAppState>>>,
) -> Result<ApiResponse<()>, String> {
    let mut state = state.write().await;

    match state.reload() {
        Ok(()) => {
            crate::events::emit_config_changed(&app);
            Ok(ApiResponse::ok(()))
        }
        Err(e) => Ok(ApiResponse::err(e)),
    }
}

/// Open native folder picker for config directory.
#[tauri::command]
pub async fn select_config_directory(
    app: tauri::AppHandle,
    state: State<'_, Arc<RwLock<TauriAppState>>>,
) -> Result<ApiResponse<Option<String>>, String> {
    use tauri_plugin_dialog::DialogExt;

    // Use the blocking folder picker
    let folder_path = app.dialog().file().blocking_pick_folder();

    match folder_path {
        Some(path) => {
            let path_str = path.to_string();

            // Try to set the config directory
            let mut app_state = state.write().await;
            match app_state.set_config_dir(std::path::PathBuf::from(&path_str)) {
                Ok(()) => {
                    crate::events::emit_config_changed(&app);
                    Ok(ApiResponse::ok(Some(path_str)))
                }
                Err(e) => Ok(ApiResponse::err(e)),
            }
        }
        None => Ok(ApiResponse::ok(None)),
    }
}

/// Health check endpoint.
#[tauri::command]
pub async fn health_check(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
) -> Result<ApiResponse<HealthStatus>, String> {
    let state = state.read().await;

    Ok(ApiResponse::ok(HealthStatus {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        config_loaded: state.config().is_some(),
        workers_running: state.is_workers_running(),
    }))
}
