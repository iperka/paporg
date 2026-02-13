//! GitOps-specific error types.

use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during GitOps operations.
#[derive(Error, Debug)]
pub enum GitOpsError {
    #[error("Failed to read config directory '{path}': {source}")]
    ReadDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to read file '{path}': {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to write file '{path}': {source}")]
    WriteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse YAML in '{path}': {message}")]
    ParseYaml { path: PathBuf, message: String },

    #[error("Failed to serialize YAML: {0}")]
    SerializeYaml(String),

    #[error("Invalid resource in '{path}': {message}")]
    InvalidResource { path: PathBuf, message: String },

    #[error("Resource not found: {kind}/{name}")]
    ResourceNotFound { kind: String, name: String },

    #[error("Resource already exists: {kind}/{name}")]
    ResourceAlreadyExists { kind: String, name: String },

    #[error("Missing required resource: {0}")]
    MissingRequired(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Invalid API version '{version}', expected '{expected}'")]
    InvalidApiVersion { version: String, expected: String },

    #[error("Unknown resource kind: {0}")]
    UnknownKind(String),

    #[error("Duplicate resource name '{name}' for kind '{kind}'")]
    DuplicateName { kind: String, name: String },

    #[error("Invalid regex pattern '{pattern}': {reason}")]
    InvalidPattern { pattern: String, reason: String },

    #[error("File operation failed: {0}")]
    FileOperation(String),

    #[error("Git operation failed: {0}")]
    GitOperation(String),

    #[error("Git repository not initialized")]
    GitNotInitialized,

    #[error("Git authentication failed: {0}")]
    GitAuthFailed(String),

    #[error("Watch error: {0}")]
    WatchError(String),

    #[error("Config directory not found: {0}")]
    ConfigDirNotFound(PathBuf),

    #[error("Settings resource is required but not found")]
    MissingSettings,

    #[error("Path traversal detected: {0}")]
    PathTraversal(String),

    #[error("Invalid file path: {0}")]
    InvalidPath(String),
}

impl From<serde_yaml::Error> for GitOpsError {
    fn from(err: serde_yaml::Error) -> Self {
        GitOpsError::ParseYaml {
            path: PathBuf::new(),
            message: err.to_string(),
        }
    }
}

impl From<std::io::Error> for GitOpsError {
    fn from(err: std::io::Error) -> Self {
        GitOpsError::FileOperation(err.to_string())
    }
}

/// Result type for GitOps operations.
pub type Result<T> = std::result::Result<T, GitOpsError>;
