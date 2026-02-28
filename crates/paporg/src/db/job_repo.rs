//! Job repository — CRUD operations for the `jobs` table.

use rusqlite::{params, Row};

use super::{Database, DatabaseError};

/// A raw job row from the database.
#[derive(Debug, Clone)]
pub struct JobRow {
    pub id: String,
    pub filename: String,
    pub source_path: String,
    pub archive_path: Option<String>,
    pub output_path: Option<String>,
    pub category: String,
    pub source_name: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub symlinks: Option<String>,
    pub current_phase: Option<String>,
    pub message: Option<String>,
    pub mime_type: Option<String>,
}

impl JobRow {
    fn from_row(row: &Row<'_>) -> Result<Self, rusqlite::Error> {
        Ok(Self {
            id: row.get("id")?,
            filename: row.get("filename")?,
            source_path: row.get("source_path")?,
            archive_path: row.get("archive_path")?,
            output_path: row.get("output_path")?,
            category: row.get("category")?,
            source_name: row.get("source_name")?,
            status: row.get("status")?,
            error: row.get("error")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
            completed_at: row.get("completed_at")?,
            symlinks: row.get("symlinks")?,
            current_phase: row.get("current_phase")?,
            message: row.get("message")?,
            mime_type: row.get("mime_type")?,
        })
    }
}

/// Query filter parameters for job listing.
#[derive(Debug, Default, Clone)]
pub struct JobFilter {
    pub status: Option<String>,
    pub category: Option<String>,
    pub source_name: Option<String>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub exclude_status: Option<String>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

/// Inserts a new job row.
pub fn insert(db: &Database, job: &JobRow) -> Result<(), DatabaseError> {
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO jobs (id, filename, source_path, archive_path, output_path, category,
             source_name, status, error, created_at, updated_at, completed_at, symlinks,
             current_phase, message, mime_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                job.id,
                job.filename,
                job.source_path,
                job.archive_path,
                job.output_path,
                job.category,
                job.source_name,
                job.status,
                job.error,
                job.created_at,
                job.updated_at,
                job.completed_at,
                job.symlinks,
                job.current_phase,
                job.message,
                job.mime_type,
            ],
        )?;
        Ok(())
    })
}

/// Updates an existing job row. All fields except `id` and `created_at` are overwritten.
pub fn update(db: &Database, job: &JobRow) -> Result<(), DatabaseError> {
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE jobs SET filename=?2, source_path=?3, archive_path=?4, output_path=?5,
             category=?6, source_name=?7, status=?8, error=?9, updated_at=?10,
             completed_at=?11, symlinks=?12, current_phase=?13, message=?14, mime_type=?15
             WHERE id=?1",
            params![
                job.id,
                job.filename,
                job.source_path,
                job.archive_path,
                job.output_path,
                job.category,
                job.source_name,
                job.status,
                job.error,
                job.updated_at,
                job.completed_at,
                job.symlinks,
                job.current_phase,
                job.message,
                job.mime_type,
            ],
        )?;
        Ok(())
    })
}

/// Finds a job by its ID.
pub fn find_by_id(db: &Database, id: &str) -> Result<Option<JobRow>, DatabaseError> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare("SELECT * FROM jobs WHERE id = ?1")?;
        let mut rows = stmt.query_map(params![id], JobRow::from_row)?;
        match rows.next() {
            Some(Ok(row)) => Ok(Some(row)),
            Some(Err(e)) => Err(DatabaseError::Sqlite(e)),
            None => Ok(None),
        }
    })
}

/// Queries jobs with filters, returning (rows, total_count).
pub fn query(db: &Database, filter: &JobFilter) -> Result<(Vec<JobRow>, u64), DatabaseError> {
    db.with_conn(|conn| {
        let mut conditions = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref status) = filter.status {
            conditions.push(format!("status = ?{}", param_values.len() + 1));
            param_values.push(Box::new(status.clone()));
        }
        if let Some(ref category) = filter.category {
            conditions.push(format!("category = ?{}", param_values.len() + 1));
            param_values.push(Box::new(category.clone()));
        }
        if let Some(ref source_name) = filter.source_name {
            conditions.push(format!("source_name = ?{}", param_values.len() + 1));
            param_values.push(Box::new(source_name.clone()));
        }
        if let Some(ref from_date) = filter.from_date {
            conditions.push(format!("created_at >= ?{}", param_values.len() + 1));
            param_values.push(Box::new(from_date.clone()));
        }
        if let Some(ref to_date) = filter.to_date {
            conditions.push(format!("created_at <= ?{}", param_values.len() + 1));
            param_values.push(Box::new(to_date.clone()));
        }
        if let Some(ref exclude_status) = filter.exclude_status {
            conditions.push(format!("status != ?{}", param_values.len() + 1));
            param_values.push(Box::new(exclude_status.clone()));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        // Count total matching rows.
        let count_sql = format!("SELECT COUNT(*) FROM jobs {}", where_clause);
        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let total: u64 = conn.query_row(&count_sql, params_ref.as_slice(), |r| r.get(0))?;

        // Fetch paginated results.
        let limit = filter.limit.unwrap_or(100) as i64;
        let offset = filter.offset.unwrap_or(0) as i64;
        param_values.push(Box::new(limit));
        param_values.push(Box::new(offset));
        let query_sql = format!(
            "SELECT * FROM jobs {} ORDER BY created_at DESC LIMIT ?{} OFFSET ?{}",
            where_clause,
            param_values.len() - 1,
            param_values.len()
        );

        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&query_sql)?;
        let rows: Vec<JobRow> = stmt
            .query_map(params_ref.as_slice(), JobRow::from_row)?
            .collect::<Result<Vec<_>, _>>()?;

        Ok((rows, total))
    })
}

/// Counts jobs with the given status.
pub fn count_by_status(db: &Database, status: &str) -> Result<u64, DatabaseError> {
    db.with_conn(|conn| {
        let count: u64 = conn.query_row(
            "SELECT COUNT(*) FROM jobs WHERE status = ?1",
            params![status],
            |r| r.get(0),
        )?;
        Ok(count)
    })
}

/// Updates only the status and updated_at of a job.
pub fn update_status(
    db: &Database,
    id: &str,
    status: &str,
    updated_at: &str,
) -> Result<(), DatabaseError> {
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE jobs SET status = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, status, updated_at],
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

    fn sample_job(id: &str) -> JobRow {
        JobRow {
            id: id.to_string(),
            filename: "test.pdf".to_string(),
            source_path: "/tmp/test.pdf".to_string(),
            archive_path: None,
            output_path: None,
            category: "unsorted".to_string(),
            source_name: None,
            status: "processing".to_string(),
            error: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            completed_at: None,
            symlinks: None,
            current_phase: Some("queued".to_string()),
            message: Some("Queued".to_string()),
            mime_type: Some("application/pdf".to_string()),
        }
    }

    #[test]
    fn test_insert_and_find() {
        let db = test_db();
        let job = sample_job("job-1");
        insert(&db, &job).unwrap();

        let found = find_by_id(&db, "job-1").unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.filename, "test.pdf");
        assert_eq!(found.status, "processing");
        assert_eq!(found.mime_type.as_deref(), Some("application/pdf"));
    }

    #[test]
    fn test_find_nonexistent() {
        let db = test_db();
        let found = find_by_id(&db, "nonexistent").unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_update() {
        let db = test_db();
        let mut job = sample_job("job-2");
        insert(&db, &job).unwrap();

        job.status = "completed".to_string();
        job.category = "invoices".to_string();
        job.output_path = Some("/output/test.pdf".to_string());
        job.completed_at = Some("2026-01-01T01:00:00Z".to_string());
        update(&db, &job).unwrap();

        let found = find_by_id(&db, "job-2").unwrap().unwrap();
        assert_eq!(found.status, "completed");
        assert_eq!(found.category, "invoices");
        assert_eq!(found.output_path.as_deref(), Some("/output/test.pdf"));
        assert!(found.completed_at.is_some());
    }

    #[test]
    fn test_query_no_filter() {
        let db = test_db();
        insert(&db, &sample_job("q1")).unwrap();
        insert(&db, &sample_job("q2")).unwrap();
        insert(&db, &sample_job("q3")).unwrap();

        let (rows, total) = query(&db, &JobFilter::default()).unwrap();
        assert_eq!(total, 3);
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn test_query_with_status_filter() {
        let db = test_db();
        insert(&db, &sample_job("s1")).unwrap();

        let mut completed_job = sample_job("s2");
        completed_job.status = "completed".to_string();
        insert(&db, &completed_job).unwrap();

        let (rows, total) = query(
            &db,
            &JobFilter {
                status: Some("completed".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(total, 1);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "s2");
    }

    #[test]
    fn test_query_with_exclude_status() {
        let db = test_db();
        insert(&db, &sample_job("e1")).unwrap();

        let mut superseded = sample_job("e2");
        superseded.status = "superseded".to_string();
        insert(&db, &superseded).unwrap();

        let (rows, total) = query(
            &db,
            &JobFilter {
                exclude_status: Some("superseded".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(total, 1);
        assert_eq!(rows[0].id, "e1");
    }

    #[test]
    fn test_query_pagination() {
        let db = test_db();
        for i in 0..10 {
            let mut job = sample_job(&format!("p{}", i));
            job.created_at = format!("2026-01-{:02}T00:00:00Z", i + 1);
            insert(&db, &job).unwrap();
        }

        let (rows, total) = query(
            &db,
            &JobFilter {
                limit: Some(3),
                offset: Some(0),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(total, 10);
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn test_count_by_status() {
        let db = test_db();
        insert(&db, &sample_job("c1")).unwrap();
        insert(&db, &sample_job("c2")).unwrap();

        let mut failed = sample_job("c3");
        failed.status = "failed".to_string();
        insert(&db, &failed).unwrap();

        assert_eq!(count_by_status(&db, "processing").unwrap(), 2);
        assert_eq!(count_by_status(&db, "failed").unwrap(), 1);
        assert_eq!(count_by_status(&db, "completed").unwrap(), 0);
    }

    #[test]
    fn test_update_status() {
        let db = test_db();
        insert(&db, &sample_job("us1")).unwrap();

        update_status(&db, "us1", "completed", "2026-01-01T02:00:00Z").unwrap();

        let found = find_by_id(&db, "us1").unwrap().unwrap();
        assert_eq!(found.status, "completed");
    }
}
