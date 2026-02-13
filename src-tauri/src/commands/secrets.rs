//! Secret management commands.

use std::sync::Arc;

use serde::Serialize;
use tauri::State;
use tokio::sync::RwLock;

use super::ApiResponse;
use crate::state::TauriAppState;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteSecretResponse {
    pub file_path: String,
}

// ============================================================================
// Validation
// ============================================================================

const VALID_SECRET_TYPES: &[&str] = &[
    "password",
    "client_id",
    "client_secret",
    "refresh_token",
    "token",
];
const SOURCE_NAME_PATTERN: &str = r"^[a-zA-Z0-9_-]+$";

fn validate_source_name(source_name: &str) -> Result<(), String> {
    if source_name.is_empty() {
        return Err("Source name is required".to_string());
    }

    let trimmed = source_name.trim();
    if trimmed != source_name {
        return Err("Source name must not have leading or trailing whitespace".to_string());
    }

    let re = regex::Regex::new(SOURCE_NAME_PATTERN).unwrap();
    if !re.is_match(trimmed) {
        return Err(
            "Source name must contain only letters, numbers, hyphens, and underscores".to_string(),
        );
    }

    Ok(())
}

fn validate_secret_type(secret_type: &str) -> Result<(), String> {
    if !VALID_SECRET_TYPES.contains(&secret_type) {
        return Err(format!(
            "Invalid secret type: {}. Must be one of: {}",
            secret_type,
            VALID_SECRET_TYPES.join(", ")
        ));
    }
    Ok(())
}

// ============================================================================
// Commands
// ============================================================================

/// Write a secret to a secure file.
#[tauri::command]
pub async fn write_secret(
    _state: State<'_, Arc<RwLock<TauriAppState>>>,
    source_name: String,
    secret_type: String,
    value: String,
) -> Result<ApiResponse<WriteSecretResponse>, String> {
    // Validate inputs
    if let Err(e) = validate_source_name(&source_name) {
        return Ok(ApiResponse::err(e));
    }
    if let Err(e) = validate_secret_type(&secret_type) {
        return Ok(ApiResponse::err(e));
    }

    let trimmed_value = value.trim();
    if trimmed_value.is_empty() {
        return Ok(ApiResponse::err("Secret value is required"));
    }

    // Get secrets directory
    let secrets_dir = match dirs::home_dir() {
        Some(home) => home.join(".paporg").join("secrets"),
        None => return Ok(ApiResponse::err("Could not determine home directory")),
    };

    // Create directory if needed
    if let Err(e) = tokio::fs::create_dir_all(&secrets_dir).await {
        return Ok(ApiResponse::err(format!(
            "Failed to create secrets directory: {}",
            e
        )));
    }

    // Write the secret file
    let filename = format!("{}-{}", source_name, secret_type);
    let file_path = secrets_dir.join(&filename);

    if let Err(e) = tokio::fs::write(&file_path, trimmed_value).await {
        return Ok(ApiResponse::err(format!("Failed to write secret: {}", e)));
    }

    // Set file permissions (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o600);
        if let Err(e) = std::fs::set_permissions(&file_path, permissions) {
            log::warn!("Failed to set secret file permissions: {}", e);
        }
    }

    Ok(ApiResponse::ok(WriteSecretResponse {
        file_path: file_path.display().to_string(),
    }))
}
