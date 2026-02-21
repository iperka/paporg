use thiserror::Error;

#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("Document processing failed: {0}")]
    Processing(#[from] crate::error::ProcessError),

    #[error("Storage failed: {0}")]
    Storage(#[from] crate::error::StorageError),

    #[error("Archival failed: {0}")]
    Archive(crate::error::StorageError),

    #[error("Invalid output path: {0}")]
    InvalidOutputPath(String),
}

#[derive(Debug, Clone)]
pub enum PipelineWarning {
    SymlinkFailed { target: String, error: String },
}
