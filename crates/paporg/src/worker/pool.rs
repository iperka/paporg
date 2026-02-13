use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crossbeam_channel::{bounded, Receiver, Sender};
use log::{debug, error, info, warn};
use tokio::sync::broadcast;

use crate::broadcast::job_progress::{JobPhase, JobProgressEvent, JobProgressTracker};
use crate::categorizer::Categorizer;
use crate::config::{Config, VariableEngine};
use crate::processor::ProcessorRegistry;
use crate::storage::{FileStorage, SymlinkManager};
use crate::worker::job::{Job, JobResult};

pub struct WorkerPool {
    job_sender: Sender<Job>,
    result_receiver: Receiver<JobResult>,
    workers: Vec<JoinHandle<()>>,
    shutdown: Arc<AtomicBool>,
    /// Optional job progress broadcaster for SSE streaming.
    /// Note: This is stored to keep the sender alive; actual usage is via cloned Arcs in workers.
    #[allow(dead_code)]
    job_progress_sender: Option<Arc<broadcast::Sender<JobProgressEvent>>>,
}

impl WorkerPool {
    pub fn new(config: &Config) -> Self {
        Self::with_progress_sender(config, None)
    }

    /// Creates a new worker pool with an optional job progress broadcaster.
    pub fn with_progress_sender(
        config: &Config,
        job_progress_sender: Option<Arc<broadcast::Sender<JobProgressEvent>>>,
    ) -> Self {
        let worker_count = config.worker_count;
        let (job_sender, job_receiver) = bounded::<Job>(worker_count * 2);
        let (result_sender, result_receiver) = bounded::<JobResult>(worker_count * 2);
        let shutdown = Arc::new(AtomicBool::new(false));

        let mut workers = Vec::with_capacity(worker_count);

        for worker_id in 0..worker_count {
            let job_rx = job_receiver.clone();
            let result_tx = result_sender.clone();
            let shutdown_flag = Arc::clone(&shutdown);
            let worker_config = WorkerConfig::from_config(config);
            let progress_sender = job_progress_sender.clone();

            let handle = thread::spawn(move || {
                run_worker(
                    worker_id,
                    job_rx,
                    result_tx,
                    shutdown_flag,
                    worker_config,
                    progress_sender,
                );
            });

            workers.push(handle);
        }

        info!("Started {} workers", worker_count);

        Self {
            job_sender,
            result_receiver,
            workers,
            shutdown,
            job_progress_sender,
        }
    }

    pub fn submit(&self, job: Job) -> Result<(), crate::error::WorkerError> {
        if self.shutdown.load(Ordering::Relaxed) {
            return Err(crate::error::WorkerError::ChannelClosed);
        }

        self.job_sender
            .send(job)
            .map_err(|_| crate::error::WorkerError::ChannelClosed)
    }

    pub fn try_recv_result(&self) -> Option<JobResult> {
        self.result_receiver.try_recv().ok()
    }

    pub fn recv_result(&self) -> Option<JobResult> {
        self.result_receiver.recv().ok()
    }

    pub fn shutdown(&self) {
        info!("Shutting down worker pool...");
        self.shutdown.store(true, Ordering::Relaxed);
    }

    pub fn wait(self) {
        // Drop sender to signal workers to exit
        drop(self.job_sender);

        for (i, worker) in self.workers.into_iter().enumerate() {
            if let Err(e) = worker.join() {
                error!("Worker {} panicked: {:?}", i, e);
            } else {
                debug!("Worker {} finished", i);
            }
        }

        info!("All workers have stopped");
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Relaxed)
    }
}

struct WorkerConfig {
    input_directory: String,
    output_directory: String,
    ocr_enabled: bool,
    ocr_languages: Vec<String>,
    ocr_dpi: u32,
    rules: Vec<crate::config::Rule>,
    defaults: crate::config::schema::DefaultsConfig,
    extracted_variables: Vec<crate::config::schema::ExtractedVariable>,
}

impl WorkerConfig {
    fn from_config(config: &Config) -> Self {
        Self {
            input_directory: config.input_directory.clone(),
            output_directory: config.output_directory.clone(),
            ocr_enabled: config.ocr.enabled,
            ocr_languages: config.ocr.languages.clone(),
            ocr_dpi: config.ocr.dpi,
            rules: config.rules.clone(),
            defaults: config.defaults.clone(),
            extracted_variables: config.variables.extracted.clone(),
        }
    }
}

fn run_worker(
    worker_id: usize,
    job_receiver: Receiver<Job>,
    result_sender: Sender<JobResult>,
    shutdown: Arc<AtomicBool>,
    config: WorkerConfig,
    progress_sender: Option<Arc<broadcast::Sender<JobProgressEvent>>>,
) {
    debug!("Worker {} started", worker_id);

    // Initialize processor registry for this worker
    let processor =
        ProcessorRegistry::new(config.ocr_enabled, &config.ocr_languages, config.ocr_dpi);

    let categorizer = Categorizer::new(config.rules.clone(), config.defaults.clone());
    let variable_engine = VariableEngine::new(&config.extracted_variables);
    let storage = FileStorage::new(&config.output_directory);
    let symlink_manager = SymlinkManager::new(&config.output_directory);
    let input_directory = Path::new(&config.input_directory);

    loop {
        if shutdown.load(Ordering::Relaxed) {
            debug!("Worker {} received shutdown signal", worker_id);
            break;
        }

        match job_receiver.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(job) => {
                debug!("Worker {} processing job: {:?}", worker_id, job.source_path);

                // Create progress tracker if broadcaster is available
                let tracker = progress_sender.as_ref().map(|sender| {
                    let filename = job
                        .source_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    let source_path = job.source_path.to_string_lossy().to_string();
                    JobProgressTracker::with_source(
                        &job.id,
                        &filename,
                        &source_path,
                        job.source_name.as_deref(),
                        job.mime_type.as_deref(),
                        Arc::clone(sender),
                    )
                });

                // Send initial queued event
                if let Some(ref t) = tracker {
                    t.update_phase(JobPhase::Queued, "Job queued for processing");
                }

                let result = process_job(
                    &job,
                    &processor,
                    &categorizer,
                    &variable_engine,
                    &storage,
                    &symlink_manager,
                    input_directory,
                    tracker.as_ref(),
                );

                if let Err(e) = result_sender.send(result) {
                    error!("Worker {} failed to send result: {}", worker_id, e);
                    break;
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                continue;
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                debug!("Worker {} job channel disconnected", worker_id);
                break;
            }
        }
    }

    debug!("Worker {} stopped", worker_id);
}

#[allow(clippy::too_many_arguments)]
fn process_job(
    job: &Job,
    processor: &ProcessorRegistry,
    categorizer: &Categorizer,
    variable_engine: &VariableEngine,
    storage: &FileStorage,
    symlink_manager: &SymlinkManager,
    input_directory: &Path,
    tracker: Option<&JobProgressTracker>,
) -> JobResult {
    // Process the document (OCR/text extraction)
    if let Some(t) = tracker {
        t.update_phase(JobPhase::Processing, "Running OCR and text extraction...");
    }

    let mut processed = match processor.process(&job.source_path) {
        Ok(p) => p,
        Err(e) => {
            warn!("Failed to process {}: {}", job.source_path.display(), e);
            if let Some(t) = tracker {
                t.failed(&e.to_string());
            }
            return JobResult::failure(job, e.to_string());
        }
    };

    // Prepend email metadata to text if available
    // This allows rules to match on email headers (From, To, Subject, etc.)
    if let Some(ref email_meta) = job.email_metadata {
        if email_meta.has_content() {
            let header_block = email_meta.to_header_block();
            processed.text = format!("{}{}", header_block, processed.text);
        }
    }

    // Extract variables from text
    if let Some(t) = tracker {
        t.update_phase(
            JobPhase::ExtractVariables,
            "Extracting variables from document...",
        );
    }

    let extracted_vars = variable_engine.extract_variables(&processed.text);

    // Categorize based on text content
    if let Some(t) = tracker {
        t.update_phase(JobPhase::Categorizing, "Categorizing document...");
    }

    let categorization = categorizer.categorize(&processed.text);

    // Substitute variables in output path
    if let Some(t) = tracker {
        t.update_phase(JobPhase::Substituting, "Substituting variables in path...");
    }

    let output_directory = variable_engine.substitute(
        &categorization.output.directory,
        &processed.metadata.original_filename,
        &extracted_vars,
    );

    let output_filename = variable_engine.substitute(
        &categorization.output.filename,
        &processed.metadata.original_filename,
        &extracted_vars,
    );

    // Store the PDF
    if let Some(t) = tracker {
        t.update_phase(JobPhase::Storing, "Storing document...");
    }

    let output_path = match storage.store(
        &processed.pdf_bytes,
        &output_directory,
        &output_filename,
        "pdf",
    ) {
        Ok(p) => p,
        Err(e) => {
            warn!("Failed to store {}: {}", job.source_path.display(), e);
            if let Some(t) = tracker {
                t.failed(&e.to_string());
            }
            return JobResult::failure(job, e.to_string());
        }
    };

    info!(
        "Stored {} -> {} (category: {})",
        job.source_path.display(),
        output_path.display(),
        categorization.category
    );

    // Create symlinks
    if let Some(t) = tracker {
        t.update_phase(JobPhase::CreatingSymlinks, "Creating symlinks...");
    }

    let mut symlink_paths = Vec::new();
    for symlink_config in &categorization.symlinks {
        let symlink_dir = variable_engine.substitute(
            &symlink_config.target,
            &processed.metadata.original_filename,
            &extracted_vars,
        );

        match symlink_manager.create_symlink(&output_path, &symlink_dir) {
            Ok(symlink_path) => {
                info!("Created symlink: {}", symlink_path.display());
                symlink_paths.push(symlink_path);
            }
            Err(e) => {
                warn!("Failed to create symlink: {}", e);
            }
        }
    }

    // Archive the source file
    if let Some(t) = tracker {
        t.update_phase(JobPhase::Archiving, "Archiving source file...");
    }

    let archive_path = match storage.archive_source(&job.source_path, input_directory) {
        Ok(p) => p,
        Err(e) => {
            warn!("Failed to archive {}: {}", job.source_path.display(), e);
            if let Some(t) = tracker {
                t.failed(&e.to_string());
            }
            return JobResult::failure(job, e.to_string());
        }
    };

    // Send completion event
    if let Some(t) = tracker {
        let symlink_strings: Vec<String> = symlink_paths
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        t.completed(
            &output_path.display().to_string(),
            &archive_path.display().to_string(),
            &symlink_strings,
            &categorization.category,
            &processed.text,
        );
    }

    JobResult::success(
        job,
        output_path,
        archive_path,
        symlink_paths,
        categorization.category,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_config(input_dir: &Path, output_dir: &Path) -> Config {
        Config {
            version: "1.0".to_string(),
            input_directory: input_dir.to_string_lossy().to_string(),
            output_directory: output_dir.to_string_lossy().to_string(),
            worker_count: 2,
            ocr: crate::config::schema::OcrConfig::default(),
            variables: crate::config::schema::VariablesConfig::default(),
            rules: vec![],
            defaults: crate::config::schema::DefaultsConfig::default(),
            ai: crate::config::schema::AiConfig::default(),
        }
    }

    #[test]
    fn test_worker_pool_creation() {
        let temp_dir = TempDir::new().unwrap();
        let input_dir = temp_dir.path().join("input");
        let output_dir = temp_dir.path().join("output");
        std::fs::create_dir_all(&input_dir).unwrap();
        std::fs::create_dir_all(&output_dir).unwrap();

        let config = create_test_config(&input_dir, &output_dir);
        let pool = WorkerPool::new(&config);

        assert!(!pool.is_shutdown());

        pool.shutdown();
        assert!(pool.is_shutdown());

        pool.wait();
    }

    #[test]
    fn test_submit_and_process_text_job() {
        let temp_dir = TempDir::new().unwrap();
        let input_dir = temp_dir.path().join("input");
        let output_dir = temp_dir.path().join("output");
        std::fs::create_dir_all(&input_dir).unwrap();
        std::fs::create_dir_all(&output_dir).unwrap();

        // Disable OCR for this test
        let mut config = create_test_config(&input_dir, &output_dir);
        config.ocr.enabled = false;

        let pool = WorkerPool::new(&config);

        // Create a test text file
        let test_file = input_dir.join("test.txt");
        let mut file = std::fs::File::create(&test_file).unwrap();
        writeln!(file, "Hello, World!").unwrap();

        // Submit job
        let job = Job::new(test_file);
        pool.submit(job).unwrap();

        // Wait for result
        let result = pool.recv_result().unwrap();
        assert!(result.success, "Job failed: {:?}", result.error);
        assert!(result.output_path.is_some());
        assert!(result.archive_path.is_some());

        pool.shutdown();
        pool.wait();
    }
}
