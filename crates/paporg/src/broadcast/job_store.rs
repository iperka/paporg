//! Job store with persistent database storage.

use std::collections::HashMap;
use std::sync::RwLock;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::broadcast::job_progress::{JobPhase, JobProgressEvent, JobStatus};
use crate::db::job_repo::{self, JobFilter, JobRow};
use crate::db::{stats_repo, Database, DatabaseError};

// ─── Helpers ────────────────────────────────────────────────────────────────

fn status_to_str(status: &JobStatus) -> &'static str {
    match status {
        JobStatus::Processing => "processing",
        JobStatus::Completed => "completed",
        JobStatus::Failed => "failed",
    }
}

fn phase_to_str(phase: &JobPhase) -> &'static str {
    match phase {
        JobPhase::Queued => "queued",
        JobPhase::Processing => "processing",
        JobPhase::ExtractVariables => "extract_variables",
        JobPhase::Categorizing => "categorizing",
        JobPhase::Substituting => "substituting",
        JobPhase::Storing => "storing",
        JobPhase::CreatingSymlinks => "creating_symlinks",
        JobPhase::Archiving => "archiving",
        JobPhase::Completed => "completed",
        JobPhase::Failed => "failed",
    }
}

fn parse_status(s: &str, job_id: &str) -> JobStatus {
    match s {
        "completed" | "ignored" | "superseded" => JobStatus::Completed,
        "failed" => JobStatus::Failed,
        "processing" => JobStatus::Processing,
        other => {
            log::warn!(
                "Unknown job status '{}' for job {}, defaulting to Processing",
                other,
                job_id
            );
            JobStatus::Processing
        }
    }
}

fn parse_phase(s: Option<&str>, job_id: &str) -> JobPhase {
    match s {
        Some("queued") => JobPhase::Queued,
        Some("processing") => JobPhase::Processing,
        Some("extract_variables") => JobPhase::ExtractVariables,
        Some("categorizing") => JobPhase::Categorizing,
        Some("substituting") => JobPhase::Substituting,
        Some("storing") => JobPhase::Storing,
        Some("creating_symlinks") => JobPhase::CreatingSymlinks,
        Some("archiving") => JobPhase::Archiving,
        Some("completed") => JobPhase::Completed,
        Some("failed") => JobPhase::Failed,
        None => JobPhase::Queued,
        Some(other) => {
            log::warn!(
                "Unknown job phase '{}' for job {}, defaulting to Queued",
                other,
                job_id
            );
            JobPhase::Queued
        }
    }
}

fn parse_timestamp(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|e| {
            log::warn!("parse_timestamp: failed to parse '{}': {}", s, e);
            Utc::now()
        })
}

fn format_timestamp(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339()
}

// ─── StoredJob ──────────────────────────────────────────────────────────────

/// A stored job with full history.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredJob {
    /// Unique job identifier.
    pub job_id: String,
    /// Original filename being processed.
    pub filename: String,
    /// Current status.
    pub status: JobStatus,
    /// Current phase.
    pub current_phase: JobPhase,
    /// When the job started.
    pub started_at: DateTime<Utc>,
    /// When the job completed (if finished).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    /// Output path (set on completion).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    /// Archive path (set on completion).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_path: Option<String>,
    /// Created symlinks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symlinks: Vec<String>,
    /// Detected category.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Error message (if failed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Current step message.
    pub message: String,
    /// Source path (original input file).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    /// Source name (import source).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
    /// Whether this job has been ignored by the user.
    #[serde(default)]
    pub ignored: bool,
    /// MIME type of the source file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

impl StoredJob {
    /// Creates a new stored job from a progress event.
    pub fn from_event(event: &JobProgressEvent) -> Self {
        let completed_at = match event.status {
            JobStatus::Completed | JobStatus::Failed => Some(event.timestamp),
            _ => None,
        };

        Self {
            job_id: event.job_id.clone(),
            filename: event.filename.clone(),
            status: event.status.clone(),
            current_phase: event.phase.clone(),
            started_at: event.timestamp,
            completed_at,
            output_path: event.output_path.clone(),
            archive_path: event.archive_path.clone(),
            symlinks: event.symlinks.clone(),
            category: event.category.clone(),
            error: event.error.clone(),
            message: event.message.clone(),
            source_path: event.source_path.clone(),
            source_name: event.source_name.clone(),
            ignored: false,
            mime_type: event.mime_type.clone(),
        }
    }

    /// Creates a StoredJob from a database row.
    pub fn from_job_row(row: &JobRow) -> Self {
        let status = parse_status(&row.status, &row.id);
        let phase = parse_phase(row.current_phase.as_deref(), &row.id);
        let symlinks: Vec<String> = row
            .symlinks
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let ignored = row.status == "ignored";
        let started_at = parse_timestamp(&row.created_at);
        let completed_at = row.completed_at.as_ref().map(|s| parse_timestamp(s));

        Self {
            job_id: row.id.clone(),
            filename: row.filename.clone(),
            status,
            current_phase: phase,
            started_at,
            completed_at,
            output_path: row.output_path.clone(),
            archive_path: row.archive_path.clone(),
            symlinks,
            category: if row.category.trim().is_empty() {
                None
            } else {
                Some(row.category.clone())
            },
            error: row.error.clone(),
            message: row.message.clone().unwrap_or_default(),
            source_path: Some(row.source_path.clone()),
            source_name: row.source_name.clone(),
            ignored,
            mime_type: row.mime_type.clone(),
        }
    }

    /// Updates the job from a progress event.
    pub fn update_from_event(&mut self, event: &JobProgressEvent) {
        self.status = event.status.clone();
        self.current_phase = event.phase.clone();
        self.message = event.message.clone();

        if matches!(event.status, JobStatus::Completed | JobStatus::Failed) {
            self.completed_at = Some(event.timestamp);
        }

        if event.output_path.is_some() {
            self.output_path = event.output_path.clone();
        }
        if event.archive_path.is_some() {
            self.archive_path = event.archive_path.clone();
        }
        if !event.symlinks.is_empty() {
            self.symlinks = event.symlinks.clone();
        }
        if event.category.is_some() {
            self.category = event.category.clone();
        }
        if event.error.is_some() {
            self.error = event.error.clone();
        }
    }

    /// Returns true if this job is finished (completed or failed).
    pub fn is_finished(&self) -> bool {
        matches!(self.status, JobStatus::Completed | JobStatus::Failed)
    }
}

// ─── Query types ────────────────────────────────────────────────────────────

/// Query parameters for job listing.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobQueryParams {
    pub status: Option<String>,
    pub category: Option<String>,
    pub source_name: Option<String>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

/// Response for job listing with pagination.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobListResponse {
    pub jobs: Vec<StoredJob>,
    pub total: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u64>,
}

// ─── JobStore ───────────────────────────────────────────────────────────────

/// Persistent job store backed by rusqlite.
///
/// Uses `std::sync::RwLock` for both database handle and cache.
/// All database operations are synchronous and sub-millisecond.
pub struct JobStore {
    /// Database handle (clone is cheap — inner `Arc`).
    db: RwLock<Option<Database>>,
    /// In-memory cache for real-time updates.
    cache: RwLock<HashMap<String, StoredJob>>,
}

impl JobStore {
    /// Creates a new job store.
    pub fn new(_max_completed: usize) -> Self {
        Self {
            db: RwLock::new(None),
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Sets the database connection.
    pub fn set_database(&self, db: Database) {
        let mut guard = match self.db.write() {
            Ok(g) => g,
            Err(poisoned) => {
                log::warn!("Job store DB lock was poisoned, recovering");
                poisoned.into_inner()
            }
        };
        *guard = Some(db);
    }

    /// Gets a cloned database handle if available.
    /// Database is internally `Arc`-based, so cloning is cheap.
    pub fn get_database(&self) -> Option<Database> {
        let guard = match self.db.read() {
            Ok(g) => g,
            Err(poisoned) => {
                log::warn!("Job store DB lock was poisoned, recovering");
                poisoned.into_inner()
            }
        };
        guard.clone()
    }

    /// Updates the in-memory cache with a progress event.
    pub fn update(&self, event: &JobProgressEvent) {
        if let Ok(mut cache) = self.cache.write() {
            if let Some(job) = cache.get_mut(&event.job_id) {
                job.update_from_event(event);
            } else {
                cache.insert(event.job_id.clone(), StoredJob::from_event(event));
            }
        }
    }

    /// Updates the cache and persists to database.
    pub fn update_and_persist(&self, event: &JobProgressEvent) {
        // Update cache first
        self.update(event);

        // Persist to database
        if let Some(db) = self.get_database() {
            if let Err(e) = self.persist_event(&db, event) {
                log::error!("Failed to persist job event to database: {}", e);
            }
        }
    }

    /// Persists a job progress event to the database.
    fn persist_event(&self, db: &Database, event: &JobProgressEvent) -> Result<(), DatabaseError> {
        let now = format_timestamp(Utc::now());
        let status = status_to_str(&event.status);
        let phase = phase_to_str(&event.phase);
        let symlinks_json = serde_json::to_string(&event.symlinks).ok();

        let existing = job_repo::find_by_id(db, &event.job_id)?;

        if let Some(mut row) = existing {
            // Update existing job
            row.status = status.to_string();
            row.current_phase = Some(phase.to_string());
            row.message = Some(event.message.clone());
            row.updated_at = now;

            if let Some(ref output_path) = event.output_path {
                row.output_path = Some(output_path.clone());
            }
            if let Some(ref archive_path) = event.archive_path {
                row.archive_path = Some(archive_path.clone());
            }
            if let Some(ref category) = event.category {
                row.category = category.clone();
            }
            if let Some(ref error) = event.error {
                row.error = Some(error.clone());
            }
            if !event.symlinks.is_empty() {
                row.symlinks = symlinks_json;
            }
            if matches!(event.status, JobStatus::Completed | JobStatus::Failed) {
                row.completed_at = Some(format_timestamp(event.timestamp));
            }

            job_repo::update(db, &row)?;
        } else {
            // Insert new job
            let source_path = match &event.source_path {
                Some(path) => path.clone(),
                None => {
                    log::error!(
                        "Job {} has no source_path - cannot persist to database",
                        event.job_id
                    );
                    return Ok(());
                }
            };

            let completed_at = if matches!(event.status, JobStatus::Completed | JobStatus::Failed) {
                Some(format_timestamp(event.timestamp))
            } else {
                None
            };

            let row = JobRow {
                id: event.job_id.clone(),
                filename: event.filename.clone(),
                source_path,
                archive_path: event.archive_path.clone(),
                output_path: event.output_path.clone(),
                category: event
                    .category
                    .clone()
                    .unwrap_or_else(|| "unsorted".to_string()),
                source_name: event.source_name.clone(),
                status: status.to_string(),
                error: event.error.clone(),
                created_at: format_timestamp(event.timestamp),
                updated_at: now,
                completed_at,
                symlinks: symlinks_json,
                current_phase: Some(phase.to_string()),
                message: Some(event.message.clone()),
                mime_type: event.mime_type.clone(),
            };

            job_repo::insert(db, &row)?;
        }

        // Record statistics on completion/failure
        if matches!(event.status, JobStatus::Completed | JobStatus::Failed) {
            self.record_stats(db, event);
        }

        Ok(())
    }

    /// Records processing statistics for a completed/failed job.
    fn record_stats(&self, db: &Database, event: &JobProgressEvent) {
        let duration_ms = self
            .cache
            .read()
            .ok()
            .and_then(|cache| {
                cache
                    .get(&event.job_id)
                    .map(|job| (event.timestamp - job.started_at).num_milliseconds())
            })
            .unwrap_or(0);

        let date = event.timestamp.format("%Y-%m-%d").to_string();
        let succeeded = matches!(event.status, JobStatus::Completed);

        if let Err(e) = stats_repo::record_job_completion(
            db,
            &date,
            event.category.as_deref(),
            event.source_name.as_deref(),
            event.mime_type.as_deref(),
            succeeded,
            duration_ms,
        ) {
            log::error!("Failed to record job statistics: {}", e);
        }
    }

    /// Returns all jobs sorted by started_at (newest first) from cache.
    pub fn get_all(&self) -> Vec<StoredJob> {
        let cache = match self.cache.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::warn!("Job store cache lock was poisoned, recovering");
                poisoned.into_inner()
            }
        };
        let mut result: Vec<StoredJob> = cache.values().cloned().collect();
        result.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        result
    }

    /// Returns all jobs, preferring database when available.
    pub fn get_all_from_db(&self) -> Vec<StoredJob> {
        if let Some(db) = self.get_database() {
            let filter = JobFilter {
                exclude_status: Some("superseded".to_string()),
                ..Default::default()
            };
            match job_repo::query(&db, &filter) {
                Ok((rows, _)) => return rows.iter().map(StoredJob::from_job_row).collect(),
                Err(e) => log::error!("Failed to query jobs from database: {}", e),
            }
        }
        self.get_all()
    }

    /// Query jobs with filters and pagination.
    pub fn query(&self, params: &JobQueryParams) -> Result<JobListResponse, DatabaseError> {
        if let Some(db) = self.get_database() {
            let filter = JobFilter {
                status: params.status.clone(),
                category: params.category.clone(),
                source_name: params.source_name.clone(),
                from_date: params.from_date.clone(),
                to_date: params.to_date.clone(),
                exclude_status: Some("superseded".to_string()),
                limit: params.limit,
                offset: params.offset,
            };
            let (rows, total) = job_repo::query(&db, &filter)?;
            let jobs = rows.iter().map(StoredJob::from_job_row).collect();
            Ok(JobListResponse {
                jobs,
                total,
                limit: params.limit,
                offset: params.offset,
            })
        } else {
            self.query_cache(params)
        }
    }

    /// Falls back to querying the in-memory cache.
    fn query_cache(&self, params: &JobQueryParams) -> Result<JobListResponse, DatabaseError> {
        let cache = match self.cache.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::warn!("Job store cache lock was poisoned, recovering");
                poisoned.into_inner()
            }
        };
        let mut jobs: Vec<StoredJob> = cache.values().cloned().collect();

        if let Some(ref status) = params.status {
            jobs.retain(|j| status_to_str(&j.status) == status);
        }
        if let Some(ref category) = params.category {
            jobs.retain(|j| j.category.as_deref() == Some(category.as_str()));
        }

        jobs.sort_by(|a, b| b.started_at.cmp(&a.started_at));

        let total = jobs.len() as u64;
        let offset = params.offset.unwrap_or(0) as usize;
        let limit = params.limit.unwrap_or(100) as usize;
        let jobs: Vec<StoredJob> = jobs.into_iter().skip(offset).take(limit).collect();

        Ok(JobListResponse {
            jobs,
            total,
            limit: params.limit,
            offset: params.offset,
        })
    }

    /// Returns a specific job by ID (from cache).
    pub fn get(&self, job_id: &str) -> Option<StoredJob> {
        let cache = match self.cache.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::warn!("Job store cache lock was poisoned, recovering");
                poisoned.into_inner()
            }
        };
        cache.get(job_id).cloned()
    }

    /// Returns a specific job by ID, checking cache then database.
    pub fn get_with_fallback(&self, job_id: &str) -> Option<StoredJob> {
        if let Some(job) = self.get(job_id) {
            return Some(job);
        }
        if let Some(db) = self.get_database() {
            if let Ok(Some(row)) = job_repo::find_by_id(&db, job_id) {
                return Some(StoredJob::from_job_row(&row));
            }
        }
        None
    }

    /// Returns all processing jobs.
    pub fn get_processing(&self) -> Vec<StoredJob> {
        let cache = match self.cache.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::warn!("Job store cache lock was poisoned, recovering");
                poisoned.into_inner()
            }
        };
        cache
            .values()
            .filter(|j| matches!(j.status, JobStatus::Processing))
            .cloned()
            .collect()
    }

    /// Returns the count of jobs by status (from cache).
    pub fn counts(&self) -> (usize, usize, usize) {
        let cache = match self.cache.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::warn!("Job store cache lock was poisoned, recovering");
                poisoned.into_inner()
            }
        };
        let mut processing = 0;
        let mut completed = 0;
        let mut failed = 0;

        for job in cache.values() {
            match job.status {
                JobStatus::Processing => processing += 1,
                JobStatus::Completed => completed += 1,
                JobStatus::Failed => failed += 1,
            }
        }

        (processing, completed, failed)
    }

    /// Returns counts from database when available.
    pub fn counts_from_db(&self) -> (u64, u64, u64) {
        if let Some(db) = self.get_database() {
            let processing = job_repo::count_by_status(&db, "processing").unwrap_or(0);
            let completed = job_repo::count_by_status(&db, "completed").unwrap_or(0);
            let failed = job_repo::count_by_status(&db, "failed").unwrap_or(0);
            (processing, completed, failed)
        } else {
            let (p, c, f) = self.counts();
            (p as u64, c as u64, f as u64)
        }
    }

    /// Marks a job as superseded (for re-run).
    pub fn mark_superseded(&self, job_id: &str) -> Result<(), DatabaseError> {
        if let Some(db) = self.get_database() {
            let now = format_timestamp(Utc::now());
            job_repo::update_status(&db, job_id, "superseded", &now)?;
        }

        if let Ok(mut cache) = self.cache.write() {
            cache.remove(job_id);
        }

        Ok(())
    }

    /// Marks a job as ignored.
    pub fn mark_ignored(&self, job_id: &str) -> Result<Option<StoredJob>, DatabaseError> {
        if let Some(db) = self.get_database() {
            let now = format_timestamp(Utc::now());
            job_repo::update_status(&db, job_id, "ignored", &now)?;
        }

        // Update cache
        if let Ok(mut cache) = self.cache.write() {
            if let Some(job) = cache.get_mut(job_id) {
                job.ignored = true;
                job.status = JobStatus::Completed;
                return Ok(Some(job.clone()));
            }
        }

        // If not in cache, try database
        Ok(self.get_with_fallback(job_id))
    }

    /// Inserts a new job directly (for re-run jobs).
    pub fn insert_job(
        &self,
        job_id: &str,
        filename: &str,
        source_path: &str,
        source_name: Option<&str>,
        mime_type: Option<&str>,
    ) -> Result<(), DatabaseError> {
        let now = Utc::now();
        let now_str = format_timestamp(now);

        if let Some(db) = self.get_database() {
            let row = JobRow {
                id: job_id.to_string(),
                filename: filename.to_string(),
                source_path: source_path.to_string(),
                archive_path: None,
                output_path: None,
                category: "unsorted".to_string(),
                source_name: source_name.map(|s| s.to_string()),
                status: "processing".to_string(),
                error: None,
                created_at: now_str.clone(),
                updated_at: now_str,
                completed_at: None,
                symlinks: None,
                current_phase: Some("queued".to_string()),
                message: Some("Job queued for processing".to_string()),
                mime_type: mime_type.map(|s| s.to_string()),
            };
            job_repo::insert(&db, &row)?;
        } else {
            log::warn!(
                "insert_job: database not available, job {} only cached",
                job_id
            );
        }

        // Also add to cache
        if let Ok(mut cache) = self.cache.write() {
            let job = StoredJob {
                job_id: job_id.to_string(),
                filename: filename.to_string(),
                status: JobStatus::Processing,
                current_phase: JobPhase::Queued,
                started_at: now,
                completed_at: None,
                output_path: None,
                archive_path: None,
                symlinks: vec![],
                category: Some("unsorted".to_string()),
                error: None,
                message: "Job queued for processing".to_string(),
                source_path: Some(source_path.to_string()),
                source_name: source_name.map(|s| s.to_string()),
                ignored: false,
                mime_type: mime_type.map(|s| s.to_string()),
            };
            cache.insert(job_id.to_string(), job);
        }

        Ok(())
    }

    /// Loads historical jobs from database into cache on startup.
    pub fn load_from_database(&self) {
        let db = match self.get_database() {
            Some(db) => db,
            None => return,
        };

        // Load all processing jobs (no limit)
        let processing_result = job_repo::query(
            &db,
            &JobFilter {
                status: Some("processing".to_string()),
                ..Default::default()
            },
        );

        // Load recent non-superseded jobs
        let recent_result = job_repo::query(
            &db,
            &JobFilter {
                exclude_status: Some("superseded".to_string()),
                limit: Some(100),
                ..Default::default()
            },
        );

        let mut loaded = 0;
        if let Ok(mut cache) = self.cache.write() {
            if let Ok((rows, _)) = processing_result {
                for row in &rows {
                    let job = StoredJob::from_job_row(row);
                    cache.insert(job.job_id.clone(), job);
                    loaded += 1;
                }
            }

            if let Ok((rows, _)) = recent_result {
                for row in &rows {
                    if !cache.contains_key(&row.id) {
                        let job = StoredJob::from_job_row(row);
                        cache.insert(job.job_id.clone(), job);
                        loaded += 1;
                    }
                }
            }
        }

        log::info!("Loaded {} jobs from database into cache", loaded);
    }
}

impl Default for JobStore {
    fn default() -> Self {
        Self::new(10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_event(job_id: &str, phase: JobPhase) -> JobProgressEvent {
        JobProgressEvent::new(job_id, "test.pdf", phase, "Test message")
    }

    fn create_event_with_source(
        job_id: &str,
        phase: JobPhase,
        source_path: &str,
    ) -> JobProgressEvent {
        let mut event = JobProgressEvent::new(job_id, "test.pdf", phase, "Test message");
        event.source_path = Some(source_path.to_string());
        event
    }

    #[test]
    fn test_store_creation() {
        let store = JobStore::new(5);
        assert_eq!(store.get_all().len(), 0);
    }

    #[test]
    fn test_stored_job_from_event() {
        let event = create_event("job-1", JobPhase::Processing);
        let job = StoredJob::from_event(&event);

        assert_eq!(job.job_id, "job-1");
        assert_eq!(job.filename, "test.pdf");
        assert_eq!(job.current_phase, JobPhase::Processing);
        assert_eq!(job.status, JobStatus::Processing);
        assert!(!job.ignored);
    }

    #[test]
    fn test_stored_job_from_job_row() {
        let row = JobRow {
            id: "row-1".to_string(),
            filename: "invoice.pdf".to_string(),
            source_path: "/tmp/invoice.pdf".to_string(),
            archive_path: Some("/archive/invoice.pdf".to_string()),
            output_path: Some("/output/invoice.pdf".to_string()),
            category: "invoices".to_string(),
            source_name: Some("inbox".to_string()),
            status: "completed".to_string(),
            error: None,
            created_at: "2026-01-15T10:30:00+00:00".to_string(),
            updated_at: "2026-01-15T10:31:00+00:00".to_string(),
            completed_at: Some("2026-01-15T10:31:00+00:00".to_string()),
            symlinks: Some(r#"["/link/invoice.pdf"]"#.to_string()),
            current_phase: Some("completed".to_string()),
            message: Some("Done".to_string()),
            mime_type: Some("application/pdf".to_string()),
        };

        let job = StoredJob::from_job_row(&row);
        assert_eq!(job.job_id, "row-1");
        assert_eq!(job.status, JobStatus::Completed);
        assert_eq!(job.current_phase, JobPhase::Completed);
        assert_eq!(job.category.as_deref(), Some("invoices"));
        assert_eq!(job.symlinks.len(), 1);
        assert!(job.completed_at.is_some());
        assert_eq!(job.mime_type.as_deref(), Some("application/pdf"));
    }

    #[test]
    fn test_stored_job_from_job_row_ignored() {
        let row = JobRow {
            id: "ign-1".to_string(),
            filename: "f.pdf".to_string(),
            source_path: "/tmp/f.pdf".to_string(),
            archive_path: None,
            output_path: None,
            category: "unsorted".to_string(),
            source_name: None,
            status: "ignored".to_string(),
            error: None,
            created_at: "2026-01-01T00:00:00+00:00".to_string(),
            updated_at: "2026-01-01T00:00:00+00:00".to_string(),
            completed_at: None,
            symlinks: None,
            current_phase: None,
            message: None,
            mime_type: None,
        };

        let job = StoredJob::from_job_row(&row);
        assert!(job.ignored);
        assert_eq!(job.status, JobStatus::Completed);
    }

    #[test]
    fn test_stored_job_update() {
        let event1 = create_event("job-1", JobPhase::Queued);
        let mut job = StoredJob::from_event(&event1);

        let event2 = create_event("job-1", JobPhase::Processing);
        job.update_from_event(&event2);

        assert_eq!(job.current_phase, JobPhase::Processing);
    }

    #[test]
    fn test_job_is_finished() {
        let mut event = create_event("job-1", JobPhase::Completed);
        event.status = JobStatus::Completed;
        let job = StoredJob::from_event(&event);

        assert!(job.is_finished());
    }

    #[test]
    fn test_store_update() {
        let store = JobStore::new(5);
        let event = create_event("job-1", JobPhase::Processing);

        store.update(&event);

        let jobs = store.get_all();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_id, "job-1");
    }

    #[test]
    fn test_store_counts() {
        let store = JobStore::new(10);

        store.update(&create_event("p1", JobPhase::Processing));
        store.update(&create_event("p2", JobPhase::Processing));

        let mut completed = create_event("c1", JobPhase::Completed);
        completed.status = JobStatus::Completed;
        store.update(&completed);

        let mut failed = create_event("f1", JobPhase::Failed);
        failed.status = JobStatus::Failed;
        store.update(&failed);

        let (processing, completed, failed) = store.counts();
        assert_eq!(processing, 2);
        assert_eq!(completed, 1);
        assert_eq!(failed, 1);
    }

    #[test]
    fn test_update_and_persist_with_db() {
        let db = Database::open_in_memory().expect("open in-memory DB");
        let store = JobStore::new(10);
        store.set_database(db.clone());

        // Insert a processing event
        let mut event = create_event_with_source("db-1", JobPhase::Queued, "/tmp/test.pdf");
        event.source_name = Some("inbox".to_string());
        store.update_and_persist(&event);

        // Verify it's in the DB
        let row = job_repo::find_by_id(&db, "db-1").unwrap();
        assert!(row.is_some());
        let row = row.unwrap();
        assert_eq!(row.status, "processing");
        assert_eq!(row.source_path, "/tmp/test.pdf");

        // Update to completed
        let mut completion = create_event("db-1", JobPhase::Completed);
        completion.status = JobStatus::Completed;
        completion.output_path = Some("/output/test.pdf".to_string());
        completion.category = Some("invoices".to_string());
        store.update_and_persist(&completion);

        // Verify update in DB
        let row = job_repo::find_by_id(&db, "db-1").unwrap().unwrap();
        assert_eq!(row.status, "completed");
        assert_eq!(row.output_path.as_deref(), Some("/output/test.pdf"));
        assert_eq!(row.category, "invoices");
        assert!(row.completed_at.is_some());
    }

    #[test]
    fn test_query_with_db() {
        let db = Database::open_in_memory().expect("open in-memory DB");
        let store = JobStore::new(10);
        store.set_database(db);

        // Insert some jobs
        store
            .insert_job("q1", "a.pdf", "/tmp/a.pdf", None, None)
            .unwrap();
        store
            .insert_job("q2", "b.pdf", "/tmp/b.pdf", None, None)
            .unwrap();

        // Query all
        let result = store.query(&JobQueryParams::default()).unwrap();
        assert_eq!(result.total, 2);
        assert_eq!(result.jobs.len(), 2);
    }

    #[test]
    fn test_mark_superseded() {
        let db = Database::open_in_memory().expect("open in-memory DB");
        let store = JobStore::new(10);
        store.set_database(db.clone());

        store
            .insert_job("sup-1", "f.pdf", "/tmp/f.pdf", None, None)
            .unwrap();
        assert!(store.get("sup-1").is_some());

        store.mark_superseded("sup-1").unwrap();

        // Removed from cache
        assert!(store.get("sup-1").is_none());
        // Updated in DB
        let row = job_repo::find_by_id(&db, "sup-1").unwrap().unwrap();
        assert_eq!(row.status, "superseded");
    }

    #[test]
    fn test_mark_ignored() {
        let db = Database::open_in_memory().expect("open in-memory DB");
        let store = JobStore::new(10);
        store.set_database(db.clone());

        store
            .insert_job("ign-1", "f.pdf", "/tmp/f.pdf", None, None)
            .unwrap();

        let result = store.mark_ignored("ign-1").unwrap();
        assert!(result.is_some());
        let job = result.unwrap();
        assert!(job.ignored);
        assert_eq!(job.status, JobStatus::Completed);

        // DB should also reflect the change
        let row = job_repo::find_by_id(&db, "ign-1").unwrap().unwrap();
        assert_eq!(row.status, "ignored");
    }

    #[test]
    fn test_insert_job() {
        let db = Database::open_in_memory().expect("open in-memory DB");
        let store = JobStore::new(10);
        store.set_database(db.clone());

        store
            .insert_job(
                "ins-1",
                "doc.pdf",
                "/tmp/doc.pdf",
                Some("email-inbox"),
                Some("application/pdf"),
            )
            .unwrap();

        // Verify cache
        let cached = store.get("ins-1").unwrap();
        assert_eq!(cached.filename, "doc.pdf");
        assert_eq!(cached.source_name.as_deref(), Some("email-inbox"));
        assert_eq!(cached.mime_type.as_deref(), Some("application/pdf"));

        // Verify DB
        let row = job_repo::find_by_id(&db, "ins-1").unwrap().unwrap();
        assert_eq!(row.filename, "doc.pdf");
        assert_eq!(row.source_name.as_deref(), Some("email-inbox"));
    }

    #[test]
    fn test_load_from_database() {
        let db = Database::open_in_memory().expect("open in-memory DB");

        // Insert directly into DB
        let row = JobRow {
            id: "load-1".to_string(),
            filename: "loaded.pdf".to_string(),
            source_path: "/tmp/loaded.pdf".to_string(),
            archive_path: None,
            output_path: None,
            category: "unsorted".to_string(),
            source_name: None,
            status: "completed".to_string(),
            error: None,
            created_at: "2026-01-01T00:00:00+00:00".to_string(),
            updated_at: "2026-01-01T00:00:00+00:00".to_string(),
            completed_at: Some("2026-01-01T00:01:00+00:00".to_string()),
            symlinks: None,
            current_phase: Some("completed".to_string()),
            message: Some("Done".to_string()),
            mime_type: None,
        };
        job_repo::insert(&db, &row).unwrap();

        // Create store and load
        let store = JobStore::new(10);
        store.set_database(db);
        store.load_from_database();

        let cached = store.get("load-1");
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().filename, "loaded.pdf");
    }

    #[test]
    fn test_stats_recording_on_completion() {
        let db = Database::open_in_memory().expect("open in-memory DB");
        let store = JobStore::new(10);
        store.set_database(db.clone());

        // Insert a processing job
        let mut event = create_event_with_source("stats-1", JobPhase::Queued, "/tmp/stats.pdf");
        event.source_name = Some("inbox".to_string());
        event.mime_type = Some("application/pdf".to_string());
        store.update_and_persist(&event);

        // Complete it
        let mut completion = create_event("stats-1", JobPhase::Completed);
        completion.status = JobStatus::Completed;
        completion.category = Some("invoices".to_string());
        completion.source_name = Some("inbox".to_string());
        completion.mime_type = Some("application/pdf".to_string());
        store.update_and_persist(&completion);

        // Verify stats were recorded
        let stats = stats_repo::query(&db, None, None, None, None).unwrap();
        assert!(!stats.is_empty());
        assert_eq!(stats[0].total_processed, 1);
        assert_eq!(stats[0].total_succeeded, 1);
    }

    #[test]
    fn test_get_with_fallback() {
        let db = Database::open_in_memory().expect("open in-memory DB");
        let store = JobStore::new(10);
        store.set_database(db.clone());

        // Insert directly into DB (not in cache)
        let row = JobRow {
            id: "fb-1".to_string(),
            filename: "fallback.pdf".to_string(),
            source_path: "/tmp/fallback.pdf".to_string(),
            archive_path: None,
            output_path: None,
            category: "unsorted".to_string(),
            source_name: None,
            status: "completed".to_string(),
            error: None,
            created_at: "2026-01-01T00:00:00+00:00".to_string(),
            updated_at: "2026-01-01T00:00:00+00:00".to_string(),
            completed_at: None,
            symlinks: None,
            current_phase: None,
            message: None,
            mime_type: None,
        };
        job_repo::insert(&db, &row).unwrap();

        // Cache miss, DB hit
        assert!(store.get("fb-1").is_none());
        let job = store.get_with_fallback("fb-1");
        assert!(job.is_some());
        assert_eq!(job.unwrap().filename, "fallback.pdf");

        // Nonexistent
        assert!(store.get_with_fallback("nonexistent").is_none());
    }
}
