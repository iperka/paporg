//! File upload commands using native file picker.

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
pub struct UploadResult {
    pub success: bool,
    pub files_uploaded: usize,
    pub uploaded_paths: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

// ============================================================================
// Commands
// ============================================================================

/// Upload files to the inbox directory.
/// Takes file paths (from native file picker) and copies them to the inbox.
#[tauri::command]
pub async fn upload_files(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    file_paths: Vec<String>,
) -> Result<ApiResponse<UploadResult>, String> {
    let state = state.read().await;

    let config_dir = match &state.config_dir {
        Some(dir) => dir.clone(),
        None => return Ok(ApiResponse::err("No config directory set")),
    };

    // Get input directory from config (this is where documents should be placed for processing)
    let inbox_path = state
        .config()
        .map(|c| c.settings.resource.spec.input_directory.clone())
        .unwrap_or_else(|| "inbox".to_string());

    drop(state);

    let inbox_dir = if inbox_path.starts_with('/') {
        PathBuf::from(&inbox_path)
    } else {
        config_dir.join(&inbox_path)
    };

    // Ensure inbox directory exists
    if let Err(e) = fs::create_dir_all(&inbox_dir).await {
        return Ok(ApiResponse::err(format!(
            "Failed to create inbox directory: {}",
            e
        )));
    }

    let mut uploaded_paths = Vec::new();
    let mut errors = Vec::new();

    for file_path in &file_paths {
        let source = PathBuf::from(file_path);

        // Get the filename
        let filename = match source.file_name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => {
                errors.push(format!("Invalid file path: {}", file_path));
                continue;
            }
        };

        let destination = inbox_dir.join(&filename);

        // Handle filename conflicts by appending number
        let final_destination = if destination.exists() {
            find_unique_filename(&destination).await
        } else {
            destination
        };

        // Copy the file
        match fs::copy(&source, &final_destination).await {
            Ok(_) => {
                uploaded_paths.push(final_destination.display().to_string());
            }
            Err(e) => {
                errors.push(format!("Failed to copy {}: {}", filename, e));
            }
        }
    }

    let files_uploaded = uploaded_paths.len();

    Ok(ApiResponse::ok(UploadResult {
        success: errors.is_empty(),
        files_uploaded,
        uploaded_paths,
        errors,
    }))
}

/// Find a unique filename by appending a number if the file already exists.
async fn find_unique_filename(path: &std::path::Path) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());

    let extension = path
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();

    let parent = path.parent().unwrap_or(path);

    let mut counter = 1;
    loop {
        let new_name = format!("{}_{}{}", stem, counter, extension);
        let new_path = parent.join(&new_name);

        if !new_path.exists() {
            return new_path;
        }

        counter += 1;

        // Safety limit
        if counter > 10000 {
            return parent.join(format!(
                "{}_{}_{}{}",
                stem,
                counter,
                chrono::Utc::now().timestamp(),
                extension
            ));
        }
    }
}

/// Open native file picker and upload selected files.
/// This is a convenience command that combines file selection with upload.
#[tauri::command]
pub async fn pick_and_upload_files(
    app: tauri::AppHandle,
    state: State<'_, Arc<RwLock<TauriAppState>>>,
) -> Result<ApiResponse<UploadResult>, String> {
    use tauri_plugin_dialog::DialogExt;

    // Open file picker
    let file_paths = app
        .dialog()
        .file()
        .add_filter(
            "Documents",
            &[
                "pdf", "png", "jpg", "jpeg", "gif", "tiff", "bmp", "webp", "docx", "doc", "txt",
            ],
        )
        .add_filter("All Files", &["*"])
        .blocking_pick_files();

    let paths = match file_paths {
        Some(files) => files
            .iter()
            .filter_map(|f| f.as_path().map(|p| p.to_string_lossy().to_string()))
            .collect::<Vec<_>>(),
        None => {
            return Ok(ApiResponse::ok(UploadResult {
                success: true,
                files_uploaded: 0,
                uploaded_paths: Vec::new(),
                errors: Vec::new(),
            }))
        }
    };

    if paths.is_empty() {
        return Ok(ApiResponse::ok(UploadResult {
            success: true,
            files_uploaded: 0,
            uploaded_paths: Vec::new(),
            errors: Vec::new(),
        }));
    }

    // Use the upload_files logic
    upload_files(state, paths).await
}
