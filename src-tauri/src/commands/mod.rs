//! Tauri commands for the Paporg desktop application.
//!
//! Commands are organized by domain:
//! - `config`: Configuration management
//! - `workers`: Worker pool control
//! - `jobs`: Job queries and operations
//! - `gitops`: GitOps resource management
//! - `git`: Git operations
//! - `email`: Email OAuth authorization
//! - `secrets`: Secret management
//! - `ai`: AI model and suggestions
//! - `files`: File operations
//! - `upload`: File upload handling

pub mod ai;
pub mod analytics;
pub mod config;
pub mod email;
pub mod files;
pub mod git;
pub mod gitops;
pub mod jobs;
pub mod secrets;
pub mod upload;
pub mod workers;

// Re-export all commands for convenient registration
pub use ai::*;
pub use analytics::*;
pub use config::*;
pub use email::*;
pub use files::*;
pub use git::*;
pub use gitops::*;
pub use jobs::*;
pub use secrets::*;
pub use upload::*;
pub use workers::*;

use serde::Serialize;

/// Response wrapper for API calls.
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }
}
