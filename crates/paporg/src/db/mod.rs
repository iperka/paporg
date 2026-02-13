//! Database module for persistent job storage.
//!
//! Uses SeaORM for database access with support for SQLite and PostgreSQL.

use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
use sea_orm_migration::MigratorTrait;

pub mod entities;
pub mod migrations;

pub use entities::Job;

/// Initialize database connection and run migrations.
pub async fn init_database(database_url: &str) -> Result<DatabaseConnection, DbErr> {
    log::info!("Connecting to database: {}", redact_url(database_url));

    let mut opt = ConnectOptions::new(database_url);
    opt.sqlx_logging(false); // Reduce noise in logs

    let db = Database::connect(opt).await?;

    log::info!("Running database migrations...");
    migrations::Migrator::up(&db, None).await?;

    log::info!("Database initialized successfully");
    Ok(db)
}

/// Redact password from database URL for logging.
fn redact_url(url: &str) -> String {
    // Simple redaction - hide password if present
    if let Some(at_pos) = url.find('@') {
        if let Some(colon_pos) = url[..at_pos].rfind(':') {
            if let Some(slash_pos) = url[..colon_pos].rfind('/') {
                let prefix = &url[..slash_pos + 1];
                let suffix = &url[at_pos..];
                return format!("{}***{}", prefix, suffix);
            }
        }
    }
    url.to_string()
}

/// Get the default database path based on config directory.
pub fn default_database_path(config_dir: &std::path::Path) -> String {
    let db_path = config_dir.join("paporg.db");
    format!("sqlite:{}?mode=rwc", db_path.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_url_postgres() {
        let url = "postgres://user:password@localhost/paporg";
        let redacted = redact_url(url);
        assert!(redacted.contains("***"));
        assert!(!redacted.contains("password"));
    }

    #[test]
    fn test_redact_url_sqlite() {
        let url = "sqlite:./paporg.db?mode=rwc";
        let redacted = redact_url(url);
        assert_eq!(redacted, url);
    }

    #[test]
    fn test_default_database_path() {
        let config_dir = std::path::Path::new("/home/user/.config/paporg");
        let path = default_database_path(config_dir);
        assert!(path.starts_with("sqlite:"));
        assert!(path.contains("paporg.db"));
    }
}
