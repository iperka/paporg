//! Database path and processing statistics commands.

use paporg::db::stats_repo::{self, StatsSummary};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

use super::ApiResponse;
use crate::state::TauriAppState;

/// Response containing the absolute database path.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabasePathResponse {
    pub path: String,
}

/// Returns the absolute path to the database file.
#[tauri::command]
pub async fn get_database_path() -> Result<ApiResponse<DatabasePathResponse>, String> {
    match paporg::db::default_database_path() {
        Some(path) => Ok(ApiResponse::ok(DatabasePathResponse {
            path: path.to_string_lossy().to_string(),
        })),
        None => Ok(ApiResponse::err("Could not determine database path")),
    }
}

/// Returns processing statistics for a date range.
#[tauri::command]
pub async fn get_processing_stats(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    from_date: String,
    to_date: String,
) -> Result<ApiResponse<StatsSummary>, String> {
    let state = state.read().await;

    let db = match state.job_store.get_database() {
        Some(db) => db,
        None => return Ok(ApiResponse::err("Database not initialized")),
    };

    match stats_repo::summary(&db, &from_date, &to_date) {
        Ok(summary) => Ok(ApiResponse::ok(summary)),
        Err(e) => {
            log::error!("Failed to query processing stats: {}", e);
            Ok(ApiResponse::err(format!("Failed to query stats: {}", e)))
        }
    }
}
