//! Database error types.

use std::path::PathBuf;
use thiserror::Error;

/// Errors from database operations.
#[derive(Error, Debug)]
pub enum DatabaseError {
    /// SQLite error from rusqlite.
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// IO error when creating directories or files.
    #[error("IO error for path '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// A migration failed to apply.
    #[error("Migration failed at version {version}: {reason}")]
    Migration { version: u32, reason: String },

    /// The database lock was poisoned.
    #[error("Database lock poisoned")]
    LockPoisoned,
}
