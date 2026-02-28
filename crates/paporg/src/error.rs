use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PaporgError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Processing error: {0}")]
    Process(#[from] ProcessError),

    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("Worker error: {0}")]
    Worker(#[from] WorkerError),

    #[error("Database error: {0}")]
    Database(#[from] crate::db::DatabaseError),
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file '{path}': {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse config JSON: {0}")]
    ParseJson(#[from] serde_json::Error),

    #[error("Config validation failed: {message}")]
    Validation { message: String },

    #[error("Schema validation failed: {errors}")]
    SchemaValidation { errors: String },

    #[error("Invalid variable pattern '{name}': {reason}")]
    InvalidPattern { name: String, reason: String },

    #[error("Invalid rule '{id}': {reason}")]
    InvalidRule { id: String, reason: String },
}

#[derive(Error, Debug)]
pub enum ProcessError {
    #[error("Unsupported document format: {0}")]
    UnsupportedFormat(String),

    #[error("Failed to read document '{path}': {source}")]
    ReadDocument {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to process PDF: {0}")]
    PdfProcessing(String),

    #[error("Failed to process DOCX: {0}")]
    DocxProcessing(String),

    #[error("Failed to process image: {0}")]
    ImageProcessing(String),

    #[error("OCR failed: {0}")]
    OcrFailed(String),

    #[error("Text extraction failed: {0}")]
    TextExtraction(String),
}

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Failed to create directory '{path}': {source}")]
    CreateDirectory {
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

    #[error("Failed to move file from '{from}' to '{to}': {source}")]
    MoveFile {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to create symlink from '{link}' to '{target}': {source}")]
    CreateSymlink {
        link: PathBuf,
        target: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("File already exists: {0}")]
    FileExists(PathBuf),
}

#[derive(Error, Debug)]
pub enum WorkerError {
    #[error("Failed to spawn worker: {0}")]
    SpawnFailed(String),

    #[error("Worker channel closed unexpectedly")]
    ChannelClosed,

    #[error("Job failed: {0}")]
    JobFailed(String),

    #[error("Directory scan failed for '{path}': {source}")]
    ScanFailed {
        path: PathBuf,
        #[source]
        source: walkdir::Error,
    },

    #[error("Watch error: {0}")]
    WatchError(String),

    #[error("Scan error: {0}")]
    ScanError(String),
}

pub type Result<T> = std::result::Result<T, PaporgError>;
