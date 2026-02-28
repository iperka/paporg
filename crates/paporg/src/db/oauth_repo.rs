//! OAuth token repository — CRUD operations for the `oauth_tokens` table.

use rusqlite::params;

use super::{Database, DatabaseError};

/// A raw OAuth token row from the database.
#[derive(Debug, Clone)]
pub struct OAuthTokenRow {
    pub source_name: String,
    pub provider: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: String,
    pub created_at: String,
    pub updated_at: String,
}

impl OAuthTokenRow {
    /// Checks if the token is expired (or expires within `buffer_seconds`).
    pub fn is_expired(&self, buffer_seconds: u64) -> bool {
        let Ok(expires) = chrono::DateTime::parse_from_rfc3339(&self.expires_at) else {
            return true; // Treat unparseable expiry as expired.
        };
        let now = chrono::Utc::now();
        let buffer = chrono::Duration::seconds(buffer_seconds.min(365 * 24 * 3600) as i64);
        expires <= now + buffer
    }

    /// Checks if the token can be refreshed.
    pub fn can_refresh(&self) -> bool {
        self.refresh_token.is_some()
    }
}

/// Inserts or updates an OAuth token.
pub fn upsert(db: &Database, row: &OAuthTokenRow) -> Result<(), DatabaseError> {
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO oauth_tokens (source_name, provider, access_token, refresh_token, expires_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(source_name) DO UPDATE SET
               provider = ?2,
               access_token = ?3,
               refresh_token = ?4,
               expires_at = ?5,
               updated_at = ?7",
            params![
                row.source_name,
                row.provider,
                row.access_token,
                row.refresh_token,
                row.expires_at,
                row.created_at,
                row.updated_at,
            ],
        )?;
        Ok(())
    })
}

/// Finds a token by source name.
pub fn find(db: &Database, source_name: &str) -> Result<Option<OAuthTokenRow>, DatabaseError> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT source_name, provider, access_token, refresh_token, expires_at, created_at, updated_at
             FROM oauth_tokens WHERE source_name = ?1",
        )?;
        let mut rows = stmt.query_map(params![source_name], |row| {
            Ok(OAuthTokenRow {
                source_name: row.get(0)?,
                provider: row.get(1)?,
                access_token: row.get(2)?,
                refresh_token: row.get(3)?,
                expires_at: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        match rows.next() {
            Some(Ok(row)) => Ok(Some(row)),
            Some(Err(e)) => Err(DatabaseError::Sqlite(e)),
            None => Ok(None),
        }
    })
}

/// Deletes a token by source name.
pub fn delete(db: &Database, source_name: &str) -> Result<(), DatabaseError> {
    db.with_conn(|conn| {
        conn.execute(
            "DELETE FROM oauth_tokens WHERE source_name = ?1",
            params![source_name],
        )?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        Database::open_in_memory().expect("Failed to create test database")
    }

    fn sample_token(source: &str) -> OAuthTokenRow {
        OAuthTokenRow {
            source_name: source.to_string(),
            provider: "gmail".to_string(),
            access_token: "access-123".to_string(),
            refresh_token: Some("refresh-456".to_string()),
            expires_at: "2026-12-31T23:59:59Z".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_upsert_and_find() {
        let db = test_db();
        upsert(&db, &sample_token("inbox")).unwrap();

        let found = find(&db, "inbox").unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.provider, "gmail");
        assert_eq!(found.access_token, "access-123");
        assert_eq!(found.refresh_token.as_deref(), Some("refresh-456"));
    }

    #[test]
    fn test_upsert_overwrites() {
        let db = test_db();
        upsert(&db, &sample_token("inbox")).unwrap();

        let mut updated = sample_token("inbox");
        updated.access_token = "new-access".to_string();
        updated.updated_at = "2026-06-01T00:00:00Z".to_string();
        upsert(&db, &updated).unwrap();

        let found = find(&db, "inbox").unwrap().unwrap();
        assert_eq!(found.access_token, "new-access");
    }

    #[test]
    fn test_find_nonexistent() {
        let db = test_db();
        assert!(find(&db, "missing").unwrap().is_none());
    }

    #[test]
    fn test_delete() {
        let db = test_db();
        upsert(&db, &sample_token("inbox")).unwrap();
        delete(&db, "inbox").unwrap();
        assert!(find(&db, "inbox").unwrap().is_none());
    }

    #[test]
    fn test_is_expired() {
        let mut token = sample_token("t");
        // Far future — not expired.
        token.expires_at = "2099-12-31T23:59:59Z".to_string();
        assert!(!token.is_expired(60));

        // Past — expired.
        token.expires_at = "2020-01-01T00:00:00Z".to_string();
        assert!(token.is_expired(0));
    }

    #[test]
    fn test_can_refresh() {
        let mut token = sample_token("t");
        assert!(token.can_refresh());

        token.refresh_token = None;
        assert!(!token.can_refresh());
    }
}
