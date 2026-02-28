//! Database module for persistent storage.
//!
//! Uses rusqlite (SQLite) with a thread-safe `Database` handle.
//! All access is serialized through a `Mutex<Connection>`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

pub mod email_repo;
pub mod error;
pub mod job_repo;
pub mod migrations;
pub mod oauth_repo;
pub mod stats_repo;

pub use error::DatabaseError;

/// Thread-safe database handle wrapping a single rusqlite connection.
///
/// Cloning is cheap (inner `Arc`). All access is serialized through
/// a `Mutex`, which is fine for SQLite (which serializes writes anyway).
/// WAL mode is enabled for concurrent read performance.
#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Opens (or creates) the database at the given path and runs all
    /// pending migrations.
    pub fn open(path: &Path) -> Result<Self, DatabaseError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| DatabaseError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

        migrations::run_all(&conn)?;

        log::info!("Database opened at {}", path.display());

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Opens an in-memory database for testing. Runs all migrations.
    pub fn open_in_memory() -> Result<Self, DatabaseError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;

        migrations::run_all(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Provides locked access to the underlying connection.
    pub fn with_conn<F, T>(&self, f: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Connection) -> Result<T, DatabaseError>,
    {
        let conn = self.conn.lock().map_err(|_| DatabaseError::LockPoisoned)?;
        f(&conn)
    }
}

/// Returns the canonical database path: `~/.paporg/data/paporg.db`.
pub fn default_database_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".paporg").join("data").join("paporg.db"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_in_memory() {
        let db = Database::open_in_memory().unwrap();
        db.with_conn(|conn| {
            let count: u32 =
                conn.query_row("SELECT COUNT(*) FROM _migrations", [], |r| r.get(0))?;
            assert!(count > 0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_open_file_db() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let db = Database::open(&path).unwrap();
        db.with_conn(|conn| {
            let count: u32 =
                conn.query_row("SELECT COUNT(*) FROM _migrations", [], |r| r.get(0))?;
            assert!(count > 0);
            Ok(())
        })
        .unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_default_database_path() {
        let path = default_database_path();
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.ends_with("paporg.db"));
        assert!(path.to_string_lossy().contains(".paporg"));
    }

    #[test]
    fn test_database_is_clone() {
        let db = Database::open_in_memory().unwrap();
        let db2 = db.clone();
        // Both should access the same underlying connection.
        db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO jobs (id, filename, source_path, created_at, updated_at) VALUES ('t1', 'f.pdf', '/tmp/f.pdf', '2026-01-01', '2026-01-01')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        db2.with_conn(|conn| {
            let count: u32 = conn.query_row("SELECT COUNT(*) FROM jobs", [], |r| r.get(0))?;
            assert_eq!(count, 1);
            Ok(())
        })
        .unwrap();
    }
}
