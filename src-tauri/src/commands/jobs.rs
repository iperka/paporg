//! Job query and operation commands.

use std::path::PathBuf;
use std::sync::Arc;

use paporg::broadcast::{JobListResponse, JobQueryParams, StoredJob};
use paporg::worker::job::Job;
use serde::Serialize;
use tauri::State;
use tokio::fs;
use tokio::sync::RwLock;

use super::ApiResponse;
use crate::state::TauriAppState;

/// OCR response for on-demand text extraction.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrResponse {
    pub text: String,
}

/// Rerun response for single job.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RerunResponse {
    pub job_id: String,
}

/// Rerun result for bulk operations.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RerunResult {
    pub submitted: u32,
    pub errors: u32,
}

/// Get all jobs.
#[tauri::command]
pub async fn get_jobs(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
) -> Result<ApiResponse<Vec<StoredJob>>, String> {
    let state = state.read().await;
    let jobs = state.job_store.get_all_from_db();
    Ok(ApiResponse::ok(jobs))
}

/// Query jobs with filters and pagination.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn query_jobs(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    status: Option<String>,
    category: Option<String>,
    source_name: Option<String>,
    from_date: Option<String>,
    to_date: Option<String>,
    limit: Option<u64>,
    offset: Option<u64>,
) -> Result<ApiResponse<JobListResponse>, String> {
    let state = state.read().await;

    let params = JobQueryParams {
        status,
        category,
        source_name,
        from_date,
        to_date,
        limit,
        offset,
    };

    match state.job_store.query(&params) {
        Ok(response) => Ok(ApiResponse::ok(response)),
        Err(e) => Ok(ApiResponse::err(format!("Database error: {}", e))),
    }
}

/// Get a single job by ID.
#[tauri::command]
pub async fn get_job(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    job_id: String,
) -> Result<ApiResponse<StoredJob>, String> {
    let state = state.read().await;

    match state.job_store.get_with_fallback(&job_id) {
        Some(job) => Ok(ApiResponse::ok(job)),
        None => Ok(ApiResponse::err(format!("Job not found: {}", job_id))),
    }
}

/// Get OCR text for a job (always re-processes from archive).
#[tauri::command]
pub async fn get_job_ocr(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    job_id: String,
) -> Result<ApiResponse<OcrResponse>, String> {
    let state = state.read().await;

    // Get job from store
    let job = match state.job_store.get_with_fallback(&job_id) {
        Some(j) => j,
        None => return Ok(ApiResponse::err("Job not found")),
    };

    // Get archive path (source of truth)
    let archive_path = match &job.archive_path {
        Some(p) => PathBuf::from(p),
        None => return Ok(ApiResponse::err("No archive file for this job")),
    };

    // Check if archive file exists
    match fs::try_exists(&archive_path).await {
        Ok(false) | Err(_) => {
            return Ok(ApiResponse::err(format!(
                "Archive file not found: {}",
                archive_path.display()
            )));
        }
        Ok(true) => {}
    }

    // Get OCR settings from config
    let config = state.config();
    let (ocr_enabled, languages, dpi) = config
        .map(|c| {
            let legacy = c.to_legacy_config();
            (legacy.ocr.enabled, legacy.ocr.languages, legacy.ocr.dpi)
        })
        .unwrap_or((true, vec!["eng".to_string()], 300));

    // Create processor and run OCR
    let processor = paporg::processor::ProcessorRegistry::new(ocr_enabled, &languages, dpi);

    match processor.process(&archive_path) {
        Ok(processed) => Ok(ApiResponse::ok(OcrResponse {
            text: processed.text,
        })),
        Err(e) => Ok(ApiResponse::err(format!("OCR failed: {}", e))),
    }
}

/// Re-run a single job.
#[tauri::command]
pub async fn rerun_job(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    job_id: String,
    source_name: Option<String>,
) -> Result<ApiResponse<RerunResponse>, String> {
    let state = state.read().await;

    // Get job from store
    let job = match state.job_store.get_with_fallback(&job_id) {
        Some(j) => j,
        None => return Ok(ApiResponse::err("Job not found")),
    };

    // Get archive path
    let archive_path = match &job.archive_path {
        Some(p) => PathBuf::from(p),
        None => return Ok(ApiResponse::err("No archive file - cannot re-run")),
    };

    // Check if archive file exists
    match fs::try_exists(&archive_path).await {
        Ok(false) | Err(_) => {
            return Ok(ApiResponse::err(format!(
                "Archive file not found: {}",
                archive_path.display()
            )));
        }
        Ok(true) => {}
    }

    // Mark old job as superseded
    if let Err(e) = state.job_store.mark_superseded(&job_id) {
        return Ok(ApiResponse::err(format!("Failed to update job: {}", e)));
    }

    // Create new job from archive
    let source = source_name.or(job.source_name.clone()).unwrap_or_default();
    let new_job = Job::new_with_source(archive_path.clone(), source.clone());
    let new_job_id = new_job.id.clone();

    // Insert new job record
    if let Err(e) = state.job_store.insert_job(
        &new_job.id,
        &job.filename,
        &archive_path.display().to_string(),
        if source.is_empty() {
            None
        } else {
            Some(&source)
        },
        job.mime_type.as_deref(),
    ) {
        return Ok(ApiResponse::err(format!("Failed to create job: {}", e)));
    }

    // Get worker pool
    let worker_pool = match &state.worker_pool {
        Some(p) => p.clone(),
        None => return Ok(ApiResponse::err("Worker pool not available")),
    };

    // Submit job to worker pool
    if let Err(e) = worker_pool.submit(new_job) {
        return Ok(ApiResponse::err(format!("Failed to submit job: {}", e)));
    }

    Ok(ApiResponse::ok(RerunResponse { job_id: new_job_id }))
}

/// Mark a job as ignored.
#[tauri::command]
pub async fn ignore_job(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    job_id: String,
) -> Result<ApiResponse<StoredJob>, String> {
    let state = state.read().await;

    match state.job_store.mark_ignored(&job_id) {
        Ok(Some(job)) => Ok(ApiResponse::ok(job)),
        Ok(None) => Ok(ApiResponse::err(format!("Job not found: {}", job_id))),
        Err(e) => Ok(ApiResponse::err(format!("Database error: {}", e))),
    }
}

/// Re-run all unsorted jobs.
#[tauri::command]
pub async fn rerun_unsorted(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
) -> Result<ApiResponse<RerunResult>, String> {
    let state = state.read().await;

    // Query all unsorted, completed jobs
    let params = JobQueryParams {
        category: Some("unsorted".to_string()),
        status: Some("completed".to_string()),
        limit: Some(1000), // Safety limit
        ..Default::default()
    };

    let unsorted = match state.job_store.query(&params) {
        Ok(r) => r.jobs,
        Err(e) => return Ok(ApiResponse::err(format!("Database error: {}", e))),
    };

    if unsorted.is_empty() {
        return Ok(ApiResponse::ok(RerunResult {
            submitted: 0,
            errors: 0,
        }));
    }

    // Get worker pool
    let worker_pool = match &state.worker_pool {
        Some(p) => p.clone(),
        None => return Ok(ApiResponse::err("Worker pool not available")),
    };

    let mut submitted = 0u32;
    let mut errors = 0u32;

    for job in unsorted {
        if let Some(archive_path_str) = &job.archive_path {
            let archive_path = PathBuf::from(archive_path_str);

            // Skip if archive doesn't exist
            match fs::try_exists(&archive_path).await {
                Ok(false) | Err(_) => {
                    log::warn!(
                        "Archive file not found for job {}: {}",
                        job.job_id,
                        archive_path.display()
                    );
                    errors += 1;
                    continue;
                }
                Ok(true) => {}
            }

            // Mark old job as superseded
            if let Err(e) = state.job_store.mark_superseded(&job.job_id) {
                log::error!("Failed to mark job {} as superseded: {}", job.job_id, e);
                errors += 1;
                continue;
            }

            // Create and submit new job
            let source_name = job.source_name.clone().unwrap_or_default();
            let new_job = Job::new_with_source(archive_path.clone(), source_name.clone());

            // Insert new job record
            if let Err(e) = state.job_store.insert_job(
                &new_job.id,
                &job.filename,
                &archive_path.display().to_string(),
                if source_name.is_empty() {
                    None
                } else {
                    Some(&source_name)
                },
                job.mime_type.as_deref(),
            ) {
                log::error!("Failed to insert job record: {}", e);
                errors += 1;
                continue;
            }

            if worker_pool.submit(new_job).is_ok() {
                submitted += 1;
            } else {
                errors += 1;
            }
        } else {
            errors += 1;
        }
    }

    Ok(ApiResponse::ok(RerunResult { submitted, errors }))
}
