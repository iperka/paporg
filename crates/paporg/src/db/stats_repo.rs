//! Processing statistics repository — records and queries aggregate metrics.

use rusqlite::params;
use serde::Serialize;

use super::{Database, DatabaseError};

/// Records a completed job into the daily statistics.
///
/// Uses UPSERT to increment counters for the matching
/// `(date, category, source_name, mime_type)` combination.
pub fn record_job_completion(
    db: &Database,
    date: &str,
    category: Option<&str>,
    source_name: Option<&str>,
    mime_type: Option<&str>,
    succeeded: bool,
    duration_ms: i64,
) -> Result<(), DatabaseError> {
    db.with_conn(|conn| {
        let success_val: i64 = if succeeded { 1 } else { 0 };
        let failure_val: i64 = if succeeded { 0 } else { 1 };

        // Running-average formula: In SQLite's ON CONFLICT DO UPDATE, column
        // references on the right side resolve to the *pre-update* (old) values.
        // With old count N and old avg A, the correct update is:
        //   new_avg = (A * N + new_value) / (N + 1)
        conn.execute(
            "INSERT INTO processing_stats (date, category, source_name, mime_type,
             total_processed, total_succeeded, total_failed, avg_duration_ms)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7)
             ON CONFLICT(date, category, source_name, mime_type) DO UPDATE SET
               total_processed = total_processed + 1,
               total_succeeded = total_succeeded + ?5,
               total_failed = total_failed + ?6,
               avg_duration_ms = (avg_duration_ms * total_processed + ?7) / (total_processed + 1)",
            params![
                date,
                category,
                source_name,
                mime_type,
                success_val,
                failure_val,
                duration_ms,
            ],
        )?;
        Ok(())
    })
}

/// A single statistics row.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingStatRow {
    pub date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub total_processed: i64,
    pub total_succeeded: i64,
    pub total_failed: i64,
    pub avg_duration_ms: i64,
}

/// Queries statistics rows with optional filters.
pub fn query(
    db: &Database,
    from_date: Option<&str>,
    to_date: Option<&str>,
    category: Option<&str>,
    source_name: Option<&str>,
) -> Result<Vec<ProcessingStatRow>, DatabaseError> {
    db.with_conn(|conn| {
        let mut conditions = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(from) = from_date {
            conditions.push(format!("date >= ?{}", param_values.len() + 1));
            param_values.push(Box::new(from.to_string()));
        }
        if let Some(to) = to_date {
            conditions.push(format!("date <= ?{}", param_values.len() + 1));
            param_values.push(Box::new(to.to_string()));
        }
        if let Some(cat) = category {
            conditions.push(format!("category = ?{}", param_values.len() + 1));
            param_values.push(Box::new(cat.to_string()));
        }
        if let Some(src) = source_name {
            conditions.push(format!("source_name = ?{}", param_values.len() + 1));
            param_values.push(Box::new(src.to_string()));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT date, category, source_name, mime_type,
             total_processed, total_succeeded, total_failed, avg_duration_ms
             FROM processing_stats {} ORDER BY date DESC",
            where_clause
        );

        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows: Vec<ProcessingStatRow> = stmt
            .query_map(params_ref.as_slice(), |row| {
                Ok(ProcessingStatRow {
                    date: row.get(0)?,
                    category: row.get(1)?,
                    source_name: row.get(2)?,
                    mime_type: row.get(3)?,
                    total_processed: row.get(4)?,
                    total_succeeded: row.get(5)?,
                    total_failed: row.get(6)?,
                    avg_duration_ms: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    })
}

/// Aggregate summary across all dimensions for a date range.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsSummary {
    pub total_processed: i64,
    pub total_succeeded: i64,
    pub total_failed: i64,
    pub avg_duration_ms: i64,
    pub by_category: Vec<CategoryStat>,
    pub by_source: Vec<SourceStat>,
    pub by_date: Vec<DateStat>,
}

/// Per-category aggregate.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryStat {
    pub category: String,
    pub total_processed: i64,
    pub total_succeeded: i64,
    pub total_failed: i64,
}

/// Per-source aggregate.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceStat {
    pub source_name: String,
    pub total_processed: i64,
    pub total_succeeded: i64,
    pub total_failed: i64,
}

/// Per-date aggregate.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DateStat {
    pub date: String,
    pub total_processed: i64,
    pub total_succeeded: i64,
    pub total_failed: i64,
    pub avg_duration_ms: i64,
}

/// Returns an aggregate summary for a date range.
pub fn summary(
    db: &Database,
    from_date: &str,
    to_date: &str,
) -> Result<StatsSummary, DatabaseError> {
    db.with_conn(|conn| {
        // Overall totals.
        let (total_processed, total_succeeded, total_failed, avg_duration_ms): (
            i64,
            i64,
            i64,
            i64,
        ) = conn.query_row(
            "SELECT COALESCE(SUM(total_processed), 0), COALESCE(SUM(total_succeeded), 0),
             COALESCE(SUM(total_failed), 0),
             CASE WHEN SUM(total_processed) > 0
                  THEN SUM(avg_duration_ms * total_processed) / SUM(total_processed)
                  ELSE 0 END
             FROM processing_stats WHERE date >= ?1 AND date <= ?2",
            params![from_date, to_date],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;

        // By category.
        let mut stmt = conn.prepare(
            "SELECT COALESCE(category, 'unsorted'), SUM(total_processed), SUM(total_succeeded), SUM(total_failed)
             FROM processing_stats WHERE date >= ?1 AND date <= ?2
             GROUP BY category ORDER BY SUM(total_processed) DESC",
        )?;
        let by_category: Vec<CategoryStat> = stmt
            .query_map(params![from_date, to_date], |row| {
                Ok(CategoryStat {
                    category: row.get(0)?,
                    total_processed: row.get(1)?,
                    total_succeeded: row.get(2)?,
                    total_failed: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // By source.
        let mut stmt = conn.prepare(
            "SELECT COALESCE(source_name, 'default'), SUM(total_processed), SUM(total_succeeded), SUM(total_failed)
             FROM processing_stats WHERE date >= ?1 AND date <= ?2
             GROUP BY source_name ORDER BY SUM(total_processed) DESC",
        )?;
        let by_source: Vec<SourceStat> = stmt
            .query_map(params![from_date, to_date], |row| {
                Ok(SourceStat {
                    source_name: row.get(0)?,
                    total_processed: row.get(1)?,
                    total_succeeded: row.get(2)?,
                    total_failed: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // By date.
        let mut stmt = conn.prepare(
            "SELECT date, SUM(total_processed), SUM(total_succeeded), SUM(total_failed),
             CASE WHEN SUM(total_processed) > 0
                  THEN SUM(avg_duration_ms * total_processed) / SUM(total_processed)
                  ELSE 0 END
             FROM processing_stats WHERE date >= ?1 AND date <= ?2
             GROUP BY date ORDER BY date",
        )?;
        let by_date: Vec<DateStat> = stmt
            .query_map(params![from_date, to_date], |row| {
                Ok(DateStat {
                    date: row.get(0)?,
                    total_processed: row.get(1)?,
                    total_succeeded: row.get(2)?,
                    total_failed: row.get(3)?,
                    avg_duration_ms: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(StatsSummary {
            total_processed,
            total_succeeded,
            total_failed,
            avg_duration_ms,
            by_category,
            by_source,
            by_date,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        Database::open_in_memory().expect("Failed to create test database")
    }

    #[test]
    fn test_record_and_query() {
        let db = test_db();

        record_job_completion(
            &db,
            "2026-01-01",
            Some("invoices"),
            Some("inbox"),
            Some("application/pdf"),
            true,
            1500,
        )
        .unwrap();
        record_job_completion(
            &db,
            "2026-01-01",
            Some("invoices"),
            Some("inbox"),
            Some("application/pdf"),
            true,
            2000,
        )
        .unwrap();
        record_job_completion(
            &db,
            "2026-01-01",
            Some("invoices"),
            Some("inbox"),
            Some("application/pdf"),
            false,
            500,
        )
        .unwrap();

        let rows = query(&db, Some("2026-01-01"), Some("2026-01-01"), None, None).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].total_processed, 3);
        assert_eq!(rows[0].total_succeeded, 2);
        assert_eq!(rows[0].total_failed, 1);
    }

    #[test]
    fn test_running_average_correctness() {
        let db = test_db();

        // Record 100ms then 200ms — average should be 150.
        // Use non-NULL values for all UNIQUE columns so the UPSERT triggers correctly
        // (SQLite treats NULLs as distinct in UNIQUE constraints).
        record_job_completion(
            &db,
            "2026-02-01",
            Some("cat"),
            Some("src"),
            Some("application/pdf"),
            true,
            100,
        )
        .unwrap();
        record_job_completion(
            &db,
            "2026-02-01",
            Some("cat"),
            Some("src"),
            Some("application/pdf"),
            true,
            200,
        )
        .unwrap();

        let rows = query(&db, Some("2026-02-01"), Some("2026-02-01"), None, None).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].total_processed, 2);
        assert_eq!(rows[0].avg_duration_ms, 150);

        // Record a third value of 300ms — average should be (100+200+300)/3 = 200.
        record_job_completion(
            &db,
            "2026-02-01",
            Some("cat"),
            Some("src"),
            Some("application/pdf"),
            true,
            300,
        )
        .unwrap();

        let rows = query(&db, Some("2026-02-01"), Some("2026-02-01"), None, None).unwrap();
        assert_eq!(rows[0].total_processed, 3);
        assert_eq!(rows[0].avg_duration_ms, 200);
    }

    #[test]
    fn test_different_categories_separate() {
        let db = test_db();

        record_job_completion(&db, "2026-01-01", Some("invoices"), None, None, true, 100).unwrap();
        record_job_completion(&db, "2026-01-01", Some("receipts"), None, None, true, 200).unwrap();

        let rows = query(&db, None, None, None, None).unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn test_query_with_category_filter() {
        let db = test_db();

        record_job_completion(&db, "2026-01-01", Some("invoices"), None, None, true, 100).unwrap();
        record_job_completion(&db, "2026-01-01", Some("receipts"), None, None, true, 200).unwrap();

        let rows = query(&db, None, None, Some("invoices"), None).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].category.as_deref(), Some("invoices"));
    }

    #[test]
    fn test_summary() {
        let db = test_db();

        record_job_completion(
            &db,
            "2026-01-01",
            Some("invoices"),
            Some("inbox"),
            None,
            true,
            1000,
        )
        .unwrap();
        record_job_completion(
            &db,
            "2026-01-01",
            Some("receipts"),
            Some("inbox"),
            None,
            true,
            2000,
        )
        .unwrap();
        record_job_completion(
            &db,
            "2026-01-02",
            Some("invoices"),
            Some("scanner"),
            None,
            false,
            500,
        )
        .unwrap();

        let s = summary(&db, "2026-01-01", "2026-01-02").unwrap();
        assert_eq!(s.total_processed, 3);
        assert_eq!(s.total_succeeded, 2);
        assert_eq!(s.total_failed, 1);

        // By category — invoices and receipts.
        assert_eq!(s.by_category.len(), 2);

        // By source — inbox and scanner.
        assert_eq!(s.by_source.len(), 2);

        // By date — 2 days.
        assert_eq!(s.by_date.len(), 2);
    }

    #[test]
    fn test_summary_empty() {
        let db = test_db();
        let s = summary(&db, "2026-01-01", "2026-12-31").unwrap();
        assert_eq!(s.total_processed, 0);
        assert!(s.by_category.is_empty());
        assert!(s.by_source.is_empty());
        assert!(s.by_date.is_empty());
    }
}
