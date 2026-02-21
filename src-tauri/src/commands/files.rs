//! File operation commands.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::State;
use tokio::fs;
use tokio::sync::RwLock;

use super::ApiResponse;
use crate::state::TauriAppState;

// ============================================================================
// Response Types
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileResponse {
    pub success: bool,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ============================================================================
// Path Validation
// ============================================================================

/// Validates that a path is within the config directory to prevent path traversal.
/// Rejects absolute paths to prevent escaping the config directory.
fn validate_path(config_dir: &std::path::Path, path: &str) -> Result<PathBuf, String> {
    // Reject absolute paths to prevent path traversal attacks
    let path_buf = std::path::Path::new(path);
    if path_buf.is_absolute() {
        return Err("Absolute paths are not allowed".to_string());
    }

    // Reject paths with .. components before joining
    if path.contains("..") {
        return Err("Path traversal (..) is not allowed".to_string());
    }

    let full_path = config_dir.join(path);

    // Canonicalize the config directory first
    let config_canonical = config_dir
        .canonicalize()
        .map_err(|e| format!("Config directory error: {}", e))?;

    // Canonicalize to resolve any remaining . components
    let canonical = full_path
        .canonicalize()
        .or_else(|_| {
            // If the file doesn't exist yet, check the parent
            if let Some(parent) = full_path.parent() {
                if let Some(filename) = full_path.file_name() {
                    parent.canonicalize().map(|p| p.join(filename))
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "Invalid filename",
                    ))
                }
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Invalid path",
                ))
            }
        })
        .map_err(|e| format!("Invalid path: {}", e))?;

    // Ensure the path is within the config directory
    if !canonical.starts_with(&config_canonical) {
        return Err("Path must be within the config directory".to_string());
    }

    Ok(canonical)
}

// ============================================================================
// Commands
// ============================================================================

/// Move a file.
#[tauri::command]
pub async fn move_file(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    source: String,
    destination: String,
) -> Result<ApiResponse<FileResponse>, String> {
    let state = state.read().await;

    let config_dir = match &state.config_dir {
        Some(dir) => dir.clone(),
        None => return Ok(ApiResponse::err("No config directory set")),
    };

    drop(state);

    let source_path = match validate_path(&config_dir, &source) {
        Ok(p) => p,
        Err(e) => {
            return Ok(ApiResponse::ok(FileResponse {
                success: false,
                path: source,
                error: Some(e),
            }))
        }
    };

    let dest_path = match validate_path(&config_dir, &destination) {
        Ok(p) => p,
        Err(e) => {
            return Ok(ApiResponse::ok(FileResponse {
                success: false,
                path: destination,
                error: Some(e),
            }))
        }
    };

    // Ensure destination directory exists
    if let Some(parent) = dest_path.parent() {
        if let Err(e) = fs::create_dir_all(parent).await {
            return Ok(ApiResponse::ok(FileResponse {
                success: false,
                path: destination,
                error: Some(format!("Failed to create directory: {}", e)),
            }));
        }
    }

    match fs::rename(&source_path, &dest_path).await {
        Ok(()) => Ok(ApiResponse::ok(FileResponse {
            success: true,
            path: dest_path.display().to_string(),
            error: None,
        })),
        Err(e) => Ok(ApiResponse::ok(FileResponse {
            success: false,
            path: destination,
            error: Some(format!("Failed to move file: {}", e)),
        })),
    }
}

/// Create a directory.
#[tauri::command]
pub async fn create_directory(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    path: String,
) -> Result<ApiResponse<FileResponse>, String> {
    let state = state.read().await;

    let config_dir = match &state.config_dir {
        Some(dir) => dir.clone(),
        None => return Ok(ApiResponse::err("No config directory set")),
    };

    drop(state);

    // Validate the path to prevent path traversal
    let path_buf = std::path::Path::new(&path);
    if path_buf.is_absolute() {
        return Ok(ApiResponse::ok(FileResponse {
            success: false,
            path,
            error: Some("Absolute paths are not allowed".to_string()),
        }));
    }
    if path.contains("..") {
        return Ok(ApiResponse::ok(FileResponse {
            success: false,
            path,
            error: Some("Path traversal (..) is not allowed".to_string()),
        }));
    }

    let full_path = config_dir.join(&path);

    // Ensure the resolved path stays within config directory
    // (the earlier checks already reject ".." and absolute paths)
    if !full_path.starts_with(&config_dir) {
        return Ok(ApiResponse::ok(FileResponse {
            success: false,
            path,
            error: Some("Path must be within the config directory".to_string()),
        }));
    }

    match fs::create_dir_all(&full_path).await {
        Ok(()) => Ok(ApiResponse::ok(FileResponse {
            success: true,
            path: full_path.display().to_string(),
            error: None,
        })),
        Err(e) => Ok(ApiResponse::ok(FileResponse {
            success: false,
            path,
            error: Some(format!("Failed to create directory: {}", e)),
        })),
    }
}

/// Delete a file or directory.
#[tauri::command]
pub async fn delete_file(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    path: String,
) -> Result<ApiResponse<FileResponse>, String> {
    let state = state.read().await;

    let config_dir = match &state.config_dir {
        Some(dir) => dir.clone(),
        None => return Ok(ApiResponse::err("No config directory set")),
    };

    drop(state);

    let full_path = match validate_path(&config_dir, &path) {
        Ok(p) => p,
        Err(e) => {
            return Ok(ApiResponse::ok(FileResponse {
                success: false,
                path,
                error: Some(e),
            }))
        }
    };

    let result = if full_path.is_dir() {
        fs::remove_dir_all(&full_path).await
    } else {
        fs::remove_file(&full_path).await
    };

    match result {
        Ok(()) => Ok(ApiResponse::ok(FileResponse {
            success: true,
            path: full_path.display().to_string(),
            error: None,
        })),
        Err(e) => Ok(ApiResponse::ok(FileResponse {
            success: false,
            path,
            error: Some(format!("Failed to delete: {}", e)),
        })),
    }
}

/// Read raw file content.
#[tauri::command]
pub async fn read_raw_file(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    path: String,
) -> Result<ApiResponse<String>, String> {
    let state = state.read().await;

    let config_dir = match &state.config_dir {
        Some(dir) => dir.clone(),
        None => return Ok(ApiResponse::err("No config directory set")),
    };

    drop(state);

    let full_path = match validate_path(&config_dir, &path) {
        Ok(p) => p,
        Err(e) => return Ok(ApiResponse::err(e)),
    };

    match fs::read_to_string(&full_path).await {
        Ok(content) => Ok(ApiResponse::ok(content)),
        Err(e) => Ok(ApiResponse::err(format!("Failed to read file: {}", e))),
    }
}

/// Open native folder picker dialog (no side effects).
#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle) -> Result<ApiResponse<Option<String>>, String> {
    use tauri_plugin_dialog::DialogExt;

    let folder_path = app.dialog().file().blocking_pick_folder();

    match folder_path {
        Some(path) => Ok(ApiResponse::ok(Some(path.to_string()))),
        None => Ok(ApiResponse::ok(None)),
    }
}

/// Open native file picker dialog (no side effects).
#[tauri::command]
pub async fn pick_file(app: tauri::AppHandle) -> Result<ApiResponse<Option<String>>, String> {
    use tauri_plugin_dialog::DialogExt;

    let file_path = app.dialog().file().blocking_pick_file();

    match file_path {
        Some(path) => {
            let path_str = path
                .as_path()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string());
            Ok(ApiResponse::ok(Some(path_str)))
        }
        None => Ok(ApiResponse::ok(None)),
    }
}

/// Write raw file content.
#[tauri::command]
pub async fn write_raw_file(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    path: String,
    content: String,
) -> Result<ApiResponse<FileResponse>, String> {
    let state = state.read().await;

    let config_dir = match &state.config_dir {
        Some(dir) => dir.clone(),
        None => return Ok(ApiResponse::err("No config directory set")),
    };

    drop(state);

    // Validate the path to prevent path traversal
    let path_buf = std::path::Path::new(&path);
    if path_buf.is_absolute() {
        return Ok(ApiResponse::ok(FileResponse {
            success: false,
            path,
            error: Some("Absolute paths are not allowed".to_string()),
        }));
    }
    if path.contains("..") {
        return Ok(ApiResponse::ok(FileResponse {
            success: false,
            path,
            error: Some("Path traversal (..) is not allowed".to_string()),
        }));
    }

    let full_path = config_dir.join(&path);

    // Validate that the resolved path is within config directory
    if !full_path.starts_with(&config_dir) {
        return Ok(ApiResponse::ok(FileResponse {
            success: false,
            path,
            error: Some("Path must be within the config directory".to_string()),
        }));
    }

    // Ensure parent directory exists
    if let Some(parent) = full_path.parent() {
        if let Err(e) = fs::create_dir_all(parent).await {
            return Ok(ApiResponse::ok(FileResponse {
                success: false,
                path,
                error: Some(format!("Failed to create directory: {}", e)),
            }));
        }
    }

    match fs::write(&full_path, &content).await {
        Ok(()) => Ok(ApiResponse::ok(FileResponse {
            success: true,
            path: full_path.display().to_string(),
            error: None,
        })),
        Err(e) => Ok(ApiResponse::ok(FileResponse {
            success: false,
            path,
            error: Some(format!("Failed to write file: {}", e)),
        })),
    }
}
