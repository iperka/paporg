//! Job store with persistent database storage.

use std::collections::HashMap;
use std::sync::{Arc, RwLock as StdRwLock};

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock as TokioRwLock;

use crate::broadcast::job_progress::{JobPhase, JobProgressEvent, JobStatus};
use crate::db::entities::job;

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
    #[serde(default)]
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
    /// Extracted text content (from OCR or embedded text).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ocr_text: Option<String>,
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
            ocr_text: event.ocr_text.clone(),
        }
    }

    /// Creates a StoredJob from a database model.
    pub fn from_db_model(model: &job::Model) -> Self {
        let status = match model.status.as_str() {
            "completed" => JobStatus::Completed,
            "failed" => JobStatus::Failed,
            "ignored" => JobStatus::Completed,
            "processing" => JobStatus::Processing,
            "superseded" => JobStatus::Completed, // Treat superseded as completed for display
            unknown => {
                log::warn!(
                    "Unknown job status '{}' for job {}, defaulting to Processing",
                    unknown,
                    model.id
                );
                JobStatus::Processing
            }
        };

        let phase = match model.current_phase.as_deref() {
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
            Some(unknown) => {
                log::warn!(
                    "Unknown job phase '{}' for job {}, defaulting to Queued",
                    unknown,
                    model.id
                );
                JobPhase::Queued
            }
        };

        let symlinks: Vec<String> = model
            .symlinks
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        let ignored = model.status == "ignored";

        Self {
            job_id: model.id.clone(),
            filename: model.filename.clone(),
            status,
            current_phase: phase,
            started_at: model.created_at,
            completed_at: model.completed_at,
            output_path: model.output_path.clone(),
            archive_path: model.archive_path.clone(),
            symlinks,
            category: Some(model.category.clone()),
            error: model.error.clone(),
            message: model.message.clone().unwrap_or_default(),
            source_path: Some(model.source_path.clone()),
            source_name: model.source_name.clone(),
            ignored,
            mime_type: model.mime_type.clone(),
            ocr_text: model.ocr_text.clone(),
        }
    }

    /// Updates the job from a progress event.
    pub fn update_from_event(&mut self, event: &JobProgressEvent) {
        self.status = event.status.clone();
        self.current_phase = event.phase.clone();
        self.message = event.message.clone();

        // Update completion time if finished
        if matches!(event.status, JobStatus::Completed | JobStatus::Failed) {
            self.completed_at = Some(event.timestamp);
        }

        // Update completion fields
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
        if event.ocr_text.is_some() {
            self.ocr_text = event.ocr_text.clone();
        }
    }

    /// Returns true if this job is finished (completed or failed).
    pub fn is_finished(&self) -> bool {
        matches!(self.status, JobStatus::Completed | JobStatus::Failed)
    }
}

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
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

/// Persistent job store backed by SeaORM database.
///
/// Uses std::sync::RwLock for the cache (fast, synchronous access)
/// and tokio::sync::RwLock for the database connection (async operations).
pub struct JobStore {
    /// Database connection (async access).
    db: Arc<TokioRwLock<Option<DatabaseConnection>>>,
    /// In-memory cache for real-time updates (synchronous access).
    cache: Arc<StdRwLock<HashMap<String, StoredJob>>>,
}

impl JobStore {
    /// Creates a new job store.
    pub fn new(_max_completed: usize) -> Self {
        Self {
            db: Arc::new(TokioRwLock::new(None)),
            cache: Arc::new(StdRwLock::new(HashMap::new())),
        }
    }

    /// Sets the database connection.
    pub async fn set_database(&self, db: DatabaseConnection) {
        let mut db_lock = self.db.write().await;
        *db_lock = Some(db);
    }

    /// Gets a cloned database connection if available.
    /// DatabaseConnection is internally Arc-based, so cloning is cheap.
    pub async fn get_database(&self) -> Option<DatabaseConnection> {
        let db = self.db.read().await;
        db.as_ref().cloned()
    }

    /// Updates the store with a progress event (synchronous).
    /// This only updates the in-memory cache, not the database.
    pub fn update(&self, event: &JobProgressEvent) {
        if let Ok(mut cache) = self.cache.write() {
            if let Some(job) = cache.get_mut(&event.job_id) {
                job.update_from_event(event);
            } else {
                cache.insert(event.job_id.clone(), StoredJob::from_event(event));
            }
        }
    }

    /// Updates the store with a progress event and persists to database.
    pub async fn update_and_persist(&self, event: &JobProgressEvent) {
        // Update cache synchronously
        self.update(event);

        // Persist to database asynchronously
        let db = self.db.read().await;
        if let Some(conn) = db.as_ref() {
            if let Err(e) = self.persist_event(conn, event).await {
                log::error!("Failed to persist job event to database: {}", e);
            }
        }
    }

    /// Persists a job progress event to the database.
    async fn persist_event(
        &self,
        conn: &DatabaseConnection,
        event: &JobProgressEvent,
    ) -> Result<(), sea_orm::DbErr> {
        use crate::db::entities::job::Entity as Job;

        let status = match event.status {
            JobStatus::Processing => "processing",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
        };

        let phase = match event.phase {
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
        };

        let symlinks_json = serde_json::to_string(&event.symlinks).ok();

        // Check if job exists
        let existing = Job::find_by_id(&event.job_id).one(conn).await?;

        if let Some(existing) = existing {
            // Update existing job using into_active_model to preserve unchanged fields
            let mut active: job::ActiveModel = existing.into();
            active.status = Set(status.to_string());
            active.current_phase = Set(Some(phase.to_string()));
            active.message = Set(Some(event.message.clone()));
            active.updated_at = Set(Utc::now());

            if let Some(ref output_path) = event.output_path {
                active.output_path = Set(Some(output_path.clone()));
            }
            if let Some(ref archive_path) = event.archive_path {
                active.archive_path = Set(Some(archive_path.clone()));
            }
            if let Some(ref category) = event.category {
                active.category = Set(category.clone());
            }
            if let Some(ref error) = event.error {
                active.error = Set(Some(error.clone()));
            }
            if !event.symlinks.is_empty() {
                active.symlinks = Set(symlinks_json);
            }
            if matches!(event.status, JobStatus::Completed | JobStatus::Failed) {
                active.completed_at = Set(Some(event.timestamp));
            }
            if let Some(ref ocr_text) = event.ocr_text {
                active.ocr_text = Set(Some(ocr_text.clone()));
            }

            active.update(conn).await?;
        } else {
            // Insert new job
            // source_path is required for valid jobs - return error if missing
            let source_path = match &event.source_path {
                Some(path) => path.clone(),
                None => {
                    log::error!(
                        "Job {} has no source_path - cannot persist to database",
                        event.job_id
                    );
                    return Err(sea_orm::DbErr::Custom(format!(
                        "Job {} missing required source_path",
                        event.job_id
                    )));
                }
            };
            let new_job = job::ActiveModel {
                id: Set(event.job_id.clone()),
                filename: Set(event.filename.clone()),
                source_path: Set(source_path),
                archive_path: Set(event.archive_path.clone()),
                output_path: Set(event.output_path.clone()),
                category: Set(event
                    .category
                    .clone()
                    .unwrap_or_else(|| "unsorted".to_string())),
                source_name: Set(event.source_name.clone()),
                status: Set(status.to_string()),
                error: Set(event.error.clone()),
                created_at: Set(event.timestamp),
                updated_at: Set(event.timestamp),
                completed_at: Set(
                    if matches!(event.status, JobStatus::Completed | JobStatus::Failed) {
                        Some(event.timestamp)
                    } else {
                        None
                    },
                ),
                symlinks: Set(symlinks_json),
                current_phase: Set(Some(phase.to_string())),
                message: Set(Some(event.message.clone())),
                mime_type: Set(event.mime_type.clone()),
                ocr_text: Set(event.ocr_text.clone()),
            };

            new_job.insert(conn).await?;
        }

        Ok(())
    }

    /// Returns all jobs sorted by started_at (newest first).
    /// This is synchronous and safe to call from any context.
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

    /// Returns all jobs asynchronously, including from database.
    pub async fn get_all_async(&self) -> Vec<StoredJob> {
        let db = self.db.read().await;
        if let Some(conn) = db.as_ref() {
            match self.query_jobs(conn, &JobQueryParams::default()).await {
                Ok(response) => return response.jobs,
                Err(e) => log::error!("Failed to query jobs from database: {}", e),
            }
        }

        // Fall back to cache
        self.get_all()
    }

    /// Query jobs with filters and pagination.
    pub async fn query(&self, params: &JobQueryParams) -> Result<JobListResponse, sea_orm::DbErr> {
        let db = self.db.read().await;
        if let Some(conn) = db.as_ref() {
            self.query_jobs(conn, params).await
        } else {
            // Return cached jobs if no database
            let cache = match self.cache.read() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    log::warn!("Job store cache lock was poisoned, recovering");
                    poisoned.into_inner()
                }
            };
            let mut jobs: Vec<StoredJob> = cache.values().cloned().collect();

            // Apply basic filters
            if let Some(ref status) = params.status {
                jobs.retain(|j| {
                    let job_status = match j.status {
                        JobStatus::Processing => "processing",
                        JobStatus::Completed => "completed",
                        JobStatus::Failed => "failed",
                    };
                    job_status == status
                });
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
    }

    /// Query jobs from database.
    async fn query_jobs(
        &self,
        conn: &DatabaseConnection,
        params: &JobQueryParams,
    ) -> Result<JobListResponse, sea_orm::DbErr> {
        use crate::db::entities::job::{Column, Entity as Job};

        let mut query = Job::find();

        // Apply filters
        if let Some(ref status) = params.status {
            query = query.filter(Column::Status.eq(status.as_str()));
        }
        if let Some(ref category) = params.category {
            query = query.filter(Column::Category.eq(category.as_str()));
        }
        if let Some(ref source_name) = params.source_name {
            query = query.filter(Column::SourceName.eq(source_name.as_str()));
        }
        if let Some(ref from_date) = params.from_date {
            match DateTime::parse_from_rfc3339(from_date) {
                Ok(dt) => query = query.filter(Column::CreatedAt.gte(dt.with_timezone(&Utc))),
                Err(e) => {
                    log::warn!("Invalid from_date '{}': {}", from_date, e);
                    return Err(sea_orm::DbErr::Custom(format!(
                        "Invalid from_date format: {}",
                        from_date
                    )));
                }
            }
        }
        if let Some(ref to_date) = params.to_date {
            match DateTime::parse_from_rfc3339(to_date) {
                Ok(dt) => query = query.filter(Column::CreatedAt.lte(dt.with_timezone(&Utc))),
                Err(e) => {
                    log::warn!("Invalid to_date '{}': {}", to_date, e);
                    return Err(sea_orm::DbErr::Custom(format!(
                        "Invalid to_date format: {}",
                        to_date
                    )));
                }
            }
        }

        // Don't show superseded jobs by default
        query = query.filter(Column::Status.ne("superseded"));

        // Count total before pagination
        let total = query.clone().count(conn).await?;

        // Apply pagination and ordering
        let offset = params.offset.unwrap_or(0);
        let limit = params.limit.unwrap_or(100);

        let models = query
            .order_by_desc(Column::CreatedAt)
            .offset(offset)
            .limit(limit)
            .all(conn)
            .await?;

        let jobs: Vec<StoredJob> = models.iter().map(StoredJob::from_db_model).collect();

        Ok(JobListResponse {
            jobs,
            total,
            limit: params.limit,
            offset: params.offset,
        })
    }

    /// Returns a specific job by ID (synchronous, from cache).
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

    /// Returns a specific job by ID asynchronously (checks database too).
    pub async fn get_async(&self, job_id: &str) -> Option<StoredJob> {
        // Check cache first
        if let Some(job) = self.get(job_id) {
            return Some(job);
        }

        // Check database
        let db = self.db.read().await;
        if let Some(conn) = db.as_ref() {
            use crate::db::entities::job::Entity as Job;
            if let Ok(Some(model)) = Job::find_by_id(job_id).one(conn).await {
                return Some(StoredJob::from_db_model(&model));
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

    /// Returns the count of jobs by status.
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

    /// Returns counts asynchronously from database.
    pub async fn counts_async(&self) -> (u64, u64, u64) {
        let db = self.db.read().await;
        if let Some(conn) = db.as_ref() {
            use crate::db::entities::job::{Column, Entity as Job};

            let processing = Job::find()
                .filter(Column::Status.eq("processing"))
                .count(conn)
                .await
                .unwrap_or(0);

            let completed = Job::find()
                .filter(Column::Status.eq("completed"))
                .count(conn)
                .await
                .unwrap_or(0);

            let failed = Job::find()
                .filter(Column::Status.eq("failed"))
                .count(conn)
                .await
                .unwrap_or(0);

            (processing, completed, failed)
        } else {
            let (p, c, f) = self.counts();
            (p as u64, c as u64, f as u64)
        }
    }

    /// Marks a job as superseded (for re-run).
    pub async fn mark_superseded(&self, job_id: &str) -> Result<(), sea_orm::DbErr> {
        let db = self.db.read().await;
        if let Some(conn) = db.as_ref() {
            use crate::db::entities::job::{Column, Entity as Job};

            Job::update_many()
                .col_expr(
                    Column::Status,
                    sea_orm::sea_query::Expr::value("superseded"),
                )
                .col_expr(
                    Column::UpdatedAt,
                    sea_orm::sea_query::Expr::value(Utc::now()),
                )
                .filter(Column::Id.eq(job_id))
                .exec(conn)
                .await?;
        }

        // Also update cache
        if let Ok(mut cache) = self.cache.write() {
            cache.remove(job_id);
        }

        Ok(())
    }

    /// Marks a job as ignored.
    pub async fn mark_ignored(&self, job_id: &str) -> Result<Option<StoredJob>, sea_orm::DbErr> {
        let db = self.db.read().await;
        if let Some(conn) = db.as_ref() {
            use crate::db::entities::job::{Column, Entity as Job};

            Job::update_many()
                .col_expr(Column::Status, sea_orm::sea_query::Expr::value("ignored"))
                .col_expr(
                    Column::UpdatedAt,
                    sea_orm::sea_query::Expr::value(Utc::now()),
                )
                .filter(Column::Id.eq(job_id))
                .exec(conn)
                .await?;
        }

        // Update cache
        if let Ok(mut cache) = self.cache.write() {
            if let Some(job) = cache.get_mut(job_id) {
                job.ignored = true;
                job.status = JobStatus::Completed; // "ignored" is a variant of completed
                return Ok(Some(job.clone()));
            }
        } else {
            log::warn!("Failed to acquire cache write lock for job {}", job_id);
        }

        // If not in cache, fetch from db
        drop(db);
        Ok(self.get_async(job_id).await)
    }

    /// Inserts a new job directly (for re-run jobs).
    pub async fn insert_job(
        &self,
        job_id: &str,
        filename: &str,
        source_path: &str,
        source_name: Option<&str>,
        mime_type: Option<&str>,
    ) -> Result<(), sea_orm::DbErr> {
        let db = self.db.read().await;
        if let Some(conn) = db.as_ref() {
            let now = Utc::now();
            let new_job = job::ActiveModel {
                id: Set(job_id.to_string()),
                filename: Set(filename.to_string()),
                source_path: Set(source_path.to_string()),
                archive_path: Set(None),
                output_path: Set(None),
                category: Set("unsorted".to_string()),
                source_name: Set(source_name.map(|s| s.to_string())),
                status: Set("processing".to_string()),
                error: Set(None),
                created_at: Set(now),
                updated_at: Set(now),
                completed_at: Set(None),
                symlinks: Set(None),
                current_phase: Set(Some("queued".to_string())),
                message: Set(Some("Job queued for processing".to_string())),
                mime_type: Set(mime_type.map(|s| s.to_string())),
                ocr_text: Set(None),
            };

            new_job.insert(conn).await?;
        }

        // Also add to cache
        if let Ok(mut cache) = self.cache.write() {
            let job = StoredJob {
                job_id: job_id.to_string(),
                filename: filename.to_string(),
                status: JobStatus::Processing,
                current_phase: JobPhase::Queued,
                started_at: Utc::now(),
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
                ocr_text: None,
            };
            cache.insert(job_id.to_string(), job);
        }

        Ok(())
    }

    /// Loads historical jobs from database into cache on startup.
    pub async fn load_from_database(&self) {
        let db = self.db.read().await;
        if let Some(conn) = db.as_ref() {
            use crate::db::entities::job::{Column, Entity as Job};

            let mut loaded_count = 0;

            // Load active (processing) jobs into cache
            let processing_jobs = Job::find()
                .filter(Column::Status.eq("processing"))
                .order_by_desc(Column::CreatedAt)
                .all(conn)
                .await;

            // Also load recent completed/failed jobs for display
            let recent_jobs = Job::find()
                .filter(Column::Status.ne("superseded"))
                .filter(Column::Status.ne("processing"))
                .order_by_desc(Column::CreatedAt)
                .limit(100)
                .all(conn)
                .await;

            // Acquire cache lock once and insert all jobs
            if let Ok(mut cache) = self.cache.write() {
                if let Ok(models) = processing_jobs {
                    for model in models {
                        let job = StoredJob::from_db_model(&model);
                        cache.insert(job.job_id.clone(), job);
                        loaded_count += 1;
                    }
                }

                if let Ok(models) = recent_jobs {
                    for model in models {
                        let job = StoredJob::from_db_model(&model);
                        cache.insert(job.job_id.clone(), job);
                        loaded_count += 1;
                    }
                }
            }

            log::info!("Loaded {} jobs from database into cache", loaded_count);
        }
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
}
