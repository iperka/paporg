use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crossbeam_channel::{bounded, Receiver, Sender};
use log::{debug, error, info};
use tokio::sync::broadcast;

use crate::broadcast::job_progress::{JobPhase, JobProgressEvent};
use crate::pipeline::progress::{BroadcastProgress, NoopProgress, ProgressReporter};
use crate::pipeline::{Pipeline, PipelineConfig, PipelineContext};
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
    pub fn new(config: Arc<PipelineConfig>, worker_count: usize) -> Self {
        Self::with_progress_sender(config, worker_count, None)
    }

    /// Creates a new worker pool with an optional job progress broadcaster.
    ///
    /// # Panics
    /// Panics if `worker_count` is 0.
    pub fn with_progress_sender(
        config: Arc<PipelineConfig>,
        worker_count: usize,
        job_progress_sender: Option<Arc<broadcast::Sender<JobProgressEvent>>>,
    ) -> Self {
        assert!(worker_count > 0, "worker_count must be > 0");
        let (job_sender, job_receiver) = bounded::<Job>(worker_count * 2);
        let (result_sender, result_receiver) = bounded::<JobResult>(worker_count * 2);
        let shutdown = Arc::new(AtomicBool::new(false));

        let mut workers = Vec::with_capacity(worker_count);

        for worker_id in 0..worker_count {
            let job_rx = job_receiver.clone();
            let result_tx = result_sender.clone();
            let shutdown_flag = Arc::clone(&shutdown);
            let worker_config = Arc::clone(&config);
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

fn run_worker(
    worker_id: usize,
    job_receiver: Receiver<Job>,
    result_sender: Sender<JobResult>,
    shutdown: Arc<AtomicBool>,
    config: Arc<PipelineConfig>,
    progress_sender: Option<Arc<broadcast::Sender<JobProgressEvent>>>,
) {
    debug!("Worker {} started", worker_id);

    let pipeline = Pipeline::from_config(config);

    loop {
        if shutdown.load(Ordering::Relaxed) {
            debug!("Worker {} received shutdown signal", worker_id);
            break;
        }

        match job_receiver.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(job) => {
                debug!("Worker {} processing job: {:?}", worker_id, job.source_path);

                let result = if let Some(ref sender) = progress_sender {
                    let filename = job
                        .source_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    let source_path = job.source_path.to_string_lossy().to_string();

                    let progress = BroadcastProgress::new(
                        &job.id,
                        &filename,
                        &source_path,
                        job.source_name.as_deref(),
                        job.mime_type.as_deref(),
                        Arc::clone(sender),
                    );

                    progress.report(crate::pipeline::ProgressEvent::Phase {
                        phase: JobPhase::Queued,
                        message: "Job queued for processing".to_string(),
                    });

                    let ctx = PipelineContext::new(job);
                    let (result, _ctx) = pipeline.run(ctx, &progress);
                    result
                } else {
                    let ctx = PipelineContext::new(job);
                    let (result, _ctx) = pipeline.run(ctx, &NoopProgress);
                    result
                };

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::Path;
    use tempfile::TempDir;

    fn create_test_config(input_dir: &Path, output_dir: &Path) -> Arc<PipelineConfig> {
        Arc::new(PipelineConfig {
            input_directory: input_dir.to_path_buf(),
            output_directory: output_dir.to_path_buf(),
            ocr_enabled: false,
            ocr_languages: vec![],
            ocr_dpi: 300,
            rules: vec![],
            defaults: crate::config::schema::DefaultsConfig::default(),
            extracted_variables: vec![],
        })
    }

    #[test]
    fn test_worker_pool_creation() {
        let temp_dir = TempDir::new().unwrap();
        let input_dir = temp_dir.path().join("input");
        let output_dir = temp_dir.path().join("output");
        std::fs::create_dir_all(&input_dir).unwrap();
        std::fs::create_dir_all(&output_dir).unwrap();

        let config = create_test_config(&input_dir, &output_dir);
        let pool = WorkerPool::new(config, 2);

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

        let config = create_test_config(&input_dir, &output_dir);
        let pool = WorkerPool::new(config, 2);

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
