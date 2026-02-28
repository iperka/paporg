//! Database migration system.
//!
//! Tracks applied migrations in a `_migrations` table and applies
//! pending ones in order. Some migrations (ALTER TABLE ADD/DROP COLUMN)
//! are handled conditionally to support idempotent execution.

use rusqlite::Connection;

use super::error::DatabaseError;

/// A single migration definition.
struct Migration {
    version: u32,
    description: &'static str,
    sql: &'static str,
    /// Whether this migration needs conditional handling
    /// (e.g. ADD COLUMN that may already exist).
    kind: MigrationKind,
}

enum MigrationKind {
    /// Execute the SQL directly.
    Standard,
    /// ALTER TABLE ADD COLUMN — skip if column already exists.
    AddColumn {
        table: &'static str,
        column: &'static str,
    },
    /// ALTER TABLE DROP COLUMN — skip if column does not exist.
    DropColumn {
        table: &'static str,
        column: &'static str,
    },
}

/// All migrations in order. Each is applied at most once.
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        description: "create_jobs_table",
        sql: include_str!("sql/001_create_jobs.sql"),
        kind: MigrationKind::Standard,
    },
    Migration {
        version: 2,
        description: "create_processed_emails_table",
        sql: include_str!("sql/002_create_processed_emails.sql"),
        kind: MigrationKind::Standard,
    },
    Migration {
        version: 3,
        description: "create_oauth_tokens_table",
        sql: include_str!("sql/003_create_oauth_tokens.sql"),
        kind: MigrationKind::Standard,
    },
    Migration {
        version: 4,
        description: "add_mime_type_to_jobs",
        sql: include_str!("sql/004_add_mime_type.sql"),
        kind: MigrationKind::AddColumn {
            table: "jobs",
            column: "mime_type",
        },
    },
    Migration {
        version: 5,
        description: "drop_ocr_text_from_jobs",
        sql: include_str!("sql/005_drop_ocr_text.sql"),
        kind: MigrationKind::DropColumn {
            table: "jobs",
            column: "ocr_text",
        },
    },
    Migration {
        version: 6,
        description: "create_processing_stats_table",
        sql: include_str!("sql/006_create_processing_stats.sql"),
        kind: MigrationKind::Standard,
    },
];

/// Runs all pending migrations on the given connection.
pub fn run_all(conn: &Connection) -> Result<(), DatabaseError> {
    // Create the migrations tracking table.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            version INTEGER PRIMARY KEY,
            description TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;

    let current_version: u32 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM _migrations",
        [],
        |r| r.get(0),
    )?;

    for migration in MIGRATIONS {
        if migration.version <= current_version {
            continue;
        }

        log::info!(
            "Running migration v{}: {}",
            migration.version,
            migration.description
        );

        let should_run = match &migration.kind {
            MigrationKind::Standard => true,
            MigrationKind::AddColumn { table, column } => !column_exists(conn, table, column)?,
            MigrationKind::DropColumn { table, column } => column_exists(conn, table, column)?,
        };

        if should_run {
            conn.execute_batch(migration.sql)
                .map_err(|e| DatabaseError::Migration {
                    version: migration.version,
                    reason: e.to_string(),
                })?;
        } else {
            log::info!(
                "Skipping migration v{} (condition not met)",
                migration.version
            );
        }

        conn.execute(
            "INSERT INTO _migrations (version, description) VALUES (?1, ?2)",
            rusqlite::params![migration.version, migration.description],
        )?;
    }

    Ok(())
}

/// Checks whether a column exists on a table using `PRAGMA table_info`.
fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool, DatabaseError> {
    // Validate identifier — only alphanumeric and underscores allowed.
    if !table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(DatabaseError::Migration {
            version: 0,
            reason: format!("Invalid table name: {}", table),
        });
    }
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let exists = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .any(|r| r.map(|name| name == column).unwrap_or(false));
    Ok(exists)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_run_on_fresh_db() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_all(&conn).unwrap();

        let count: u32 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, MIGRATIONS.len() as u32);
    }

    #[test]
    fn test_migrations_are_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_all(&conn).unwrap();
        // Running again should be a no-op.
        run_all(&conn).unwrap();

        let count: u32 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, MIGRATIONS.len() as u32);
    }

    #[test]
    fn test_column_exists_check() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE test_tbl (id TEXT, name TEXT);")
            .unwrap();

        assert!(column_exists(&conn, "test_tbl", "id").unwrap());
        assert!(column_exists(&conn, "test_tbl", "name").unwrap());
        assert!(!column_exists(&conn, "test_tbl", "missing").unwrap());
    }

    #[test]
    fn test_jobs_table_has_no_ocr_text() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_all(&conn).unwrap();

        assert!(!column_exists(&conn, "jobs", "ocr_text").unwrap());
    }

    #[test]
    fn test_jobs_table_has_mime_type() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_all(&conn).unwrap();

        assert!(column_exists(&conn, "jobs", "mime_type").unwrap());
    }

    #[test]
    fn test_processing_stats_table_exists() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_all(&conn).unwrap();

        // Verify table exists by inserting a row.
        conn.execute(
            "INSERT INTO processing_stats (date, total_processed) VALUES ('2026-01-01', 1)",
            [],
        )
        .unwrap();
    }
}
