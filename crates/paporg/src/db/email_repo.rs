//! Processed email repository — CRUD operations for the `processed_emails` table.

use rusqlite::params;

use super::{Database, DatabaseError};

/// A raw processed email row from the database.
#[derive(Debug, Clone)]
pub struct ProcessedEmailRow {
    pub id: String,
    pub source_name: String,
    pub uidvalidity: u32,
    pub uid: u32,
    pub message_id: Option<String>,
    pub processed_at: String,
}

/// Creates a unique ID for a processed email.
pub fn make_id(source_name: &str, uidvalidity: u32, uid: u32) -> String {
    format!("{}:{}:{}", source_name, uidvalidity, uid)
}

/// Inserts a processed email record.
pub fn insert(db: &Database, row: &ProcessedEmailRow) -> Result<(), DatabaseError> {
    db.with_conn(|conn| {
        conn.execute(
            "INSERT OR IGNORE INTO processed_emails (id, source_name, uidvalidity, uid, message_id, processed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                row.id,
                row.source_name,
                row.uidvalidity,
                row.uid,
                row.message_id,
                row.processed_at,
            ],
        )?;
        Ok(())
    })
}

/// Finds the last known UIDVALIDITY for a source (from the most recent record).
pub fn find_last_uidvalidity(
    db: &Database,
    source_name: &str,
) -> Result<Option<u32>, DatabaseError> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT uidvalidity FROM processed_emails WHERE source_name = ?1
             ORDER BY processed_at DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![source_name], |row| row.get::<_, u32>(0))?;
        match rows.next() {
            Some(Ok(val)) => Ok(Some(val)),
            Some(Err(e)) => Err(DatabaseError::Sqlite(e)),
            None => Ok(None),
        }
    })
}

/// Finds the highest processed UID for a source and UIDVALIDITY.
pub fn find_last_uid(
    db: &Database,
    source_name: &str,
    uidvalidity: u32,
) -> Result<Option<u32>, DatabaseError> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT uid FROM processed_emails
             WHERE source_name = ?1 AND uidvalidity = ?2
             ORDER BY uid DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![source_name, uidvalidity], |row| {
            row.get::<_, u32>(0)
        })?;
        match rows.next() {
            Some(Ok(val)) => Ok(Some(val)),
            Some(Err(e)) => Err(DatabaseError::Sqlite(e)),
            None => Ok(None),
        }
    })
}

/// Returns all UIDs from `uids` that have already been processed.
pub fn find_processed_uids(
    db: &Database,
    source_name: &str,
    uidvalidity: u32,
    uids: &[u32],
) -> Result<Vec<u32>, DatabaseError> {
    if uids.is_empty() {
        return Ok(Vec::new());
    }

    db.with_conn(|conn| {
        // Build IN clause with positional params.
        let placeholders: Vec<String> = (0..uids.len()).map(|i| format!("?{}", i + 3)).collect();
        let sql = format!(
            "SELECT uid FROM processed_emails
             WHERE source_name = ?1 AND uidvalidity = ?2 AND uid IN ({})",
            placeholders.join(", ")
        );

        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        param_values.push(Box::new(source_name.to_string()));
        param_values.push(Box::new(uidvalidity));
        for &uid in uids {
            param_values.push(Box::new(uid));
        }

        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let result: Vec<u32> = stmt
            .query_map(params_ref.as_slice(), |row| row.get::<_, u32>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(result)
    })
}

/// Deletes all records for a source with a specific UIDVALIDITY.
/// Returns the number of rows deleted.
pub fn delete_by_source_and_uidvalidity(
    db: &Database,
    source_name: &str,
    uidvalidity: u32,
) -> Result<u64, DatabaseError> {
    db.with_conn(|conn| {
        let count = conn.execute(
            "DELETE FROM processed_emails WHERE source_name = ?1 AND uidvalidity = ?2",
            params![source_name, uidvalidity],
        )?;
        Ok(count as u64)
    })
}

/// Counts total processed emails for a source.
pub fn count_by_source(db: &Database, source_name: &str) -> Result<u64, DatabaseError> {
    db.with_conn(|conn| {
        let count: u64 = conn.query_row(
            "SELECT COUNT(*) FROM processed_emails WHERE source_name = ?1",
            params![source_name],
            |r| r.get(0),
        )?;
        Ok(count)
    })
}

/// Finds the timestamp of the last processed email for a source.
pub fn find_last_processed_at(
    db: &Database,
    source_name: &str,
) -> Result<Option<String>, DatabaseError> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT processed_at FROM processed_emails WHERE source_name = ?1
             ORDER BY processed_at DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![source_name], |row| row.get::<_, String>(0))?;
        match rows.next() {
            Some(Ok(val)) => Ok(Some(val)),
            Some(Err(e)) => Err(DatabaseError::Sqlite(e)),
            None => Ok(None),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        Database::open_in_memory().expect("Failed to create test database")
    }

    fn sample_email(source: &str, uidvalidity: u32, uid: u32) -> ProcessedEmailRow {
        let id = make_id(source, uidvalidity, uid);
        ProcessedEmailRow {
            id,
            source_name: source.to_string(),
            uidvalidity,
            uid,
            message_id: Some(format!("<msg-{}>", uid)),
            processed_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_make_id() {
        assert_eq!(make_id("inbox", 100, 42), "inbox:100:42");
    }

    #[test]
    fn test_insert_and_count() {
        let db = test_db();
        insert(&db, &sample_email("inbox", 100, 1)).unwrap();
        insert(&db, &sample_email("inbox", 100, 2)).unwrap();
        insert(&db, &sample_email("other", 200, 1)).unwrap();

        assert_eq!(count_by_source(&db, "inbox").unwrap(), 2);
        assert_eq!(count_by_source(&db, "other").unwrap(), 1);
        assert_eq!(count_by_source(&db, "missing").unwrap(), 0);
    }

    #[test]
    fn test_insert_duplicate_is_ignored() {
        let db = test_db();
        insert(&db, &sample_email("inbox", 100, 1)).unwrap();
        // Inserting the same record again should not fail.
        insert(&db, &sample_email("inbox", 100, 1)).unwrap();
        assert_eq!(count_by_source(&db, "inbox").unwrap(), 1);
    }

    #[test]
    fn test_find_last_uidvalidity() {
        let db = test_db();
        assert_eq!(find_last_uidvalidity(&db, "inbox").unwrap(), None);

        insert(&db, &sample_email("inbox", 100, 1)).unwrap();
        assert_eq!(find_last_uidvalidity(&db, "inbox").unwrap(), Some(100));
    }

    #[test]
    fn test_find_last_uid() {
        let db = test_db();
        insert(&db, &sample_email("inbox", 100, 5)).unwrap();
        insert(&db, &sample_email("inbox", 100, 10)).unwrap();
        insert(&db, &sample_email("inbox", 100, 3)).unwrap();

        assert_eq!(find_last_uid(&db, "inbox", 100).unwrap(), Some(10));
        assert_eq!(find_last_uid(&db, "inbox", 999).unwrap(), None);
    }

    #[test]
    fn test_find_processed_uids() {
        let db = test_db();
        insert(&db, &sample_email("inbox", 100, 1)).unwrap();
        insert(&db, &sample_email("inbox", 100, 3)).unwrap();
        insert(&db, &sample_email("inbox", 100, 5)).unwrap();

        let mut processed = find_processed_uids(&db, "inbox", 100, &[1, 2, 3, 4, 5]).unwrap();
        processed.sort();
        assert_eq!(processed, vec![1, 3, 5]);

        let empty = find_processed_uids(&db, "inbox", 100, &[]).unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_delete_by_source_and_uidvalidity() {
        let db = test_db();
        insert(&db, &sample_email("inbox", 100, 1)).unwrap();
        insert(&db, &sample_email("inbox", 100, 2)).unwrap();
        insert(&db, &sample_email("inbox", 200, 1)).unwrap();

        let deleted = delete_by_source_and_uidvalidity(&db, "inbox", 100).unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(count_by_source(&db, "inbox").unwrap(), 1);
    }

    #[test]
    fn test_find_last_processed_at() {
        let db = test_db();
        assert_eq!(find_last_processed_at(&db, "inbox").unwrap(), None);

        let mut email = sample_email("inbox", 100, 1);
        email.processed_at = "2026-01-01T00:00:00Z".to_string();
        insert(&db, &email).unwrap();

        let mut email2 = sample_email("inbox", 100, 2);
        email2.processed_at = "2026-01-02T00:00:00Z".to_string();
        insert(&db, &email2).unwrap();

        assert_eq!(
            find_last_processed_at(&db, "inbox").unwrap(),
            Some("2026-01-02T00:00:00Z".to_string())
        );
    }
}
