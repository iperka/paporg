//! Application state management for Tauri.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use log::{debug, error, info, warn};
use paporg::broadcast::{GitProgressBroadcaster, JobProgressBroadcaster, JobStore, LogBroadcaster};
use paporg::gitops::reconciler::GitReconciler;
use paporg::gitops::sync_scheduler::SyncScheduler;
use paporg::gitops::watcher::ConfigChangeEvent;
use paporg::gitops::LoadedConfig;
use paporg::pipeline::PipelineConfig;
use paporg::worker::{MultiSourceScanner, WorkerPool};
use tokio::sync::broadcast;
use tokio::sync::RwLock;

/// Application state managed by Tauri.
pub struct TauriAppState {
    /// Path to the configuration directory.
    pub config_dir: Option<PathBuf>,

    /// Loaded configuration from GitOps resources.
    pub loaded_config: Option<LoadedConfig>,

    /// Worker pool for document processing.
    pub worker_pool: Option<Arc<WorkerPool>>,

    /// Log broadcaster for UI updates.
    pub log_broadcaster: Arc<LogBroadcaster>,

    /// Job progress broadcaster for real-time job updates.
    pub job_broadcaster: Arc<JobProgressBroadcaster>,

    /// Git progress broadcaster for real-time git operation updates.
    pub git_broadcaster: Arc<GitProgressBroadcaster>,

    /// Job store for persisting and querying jobs.
    pub job_store: Arc<JobStore>,

    /// Reload signal channel.
    #[allow(dead_code)]
    pub reload_tx: broadcast::Sender<()>,

    /// Process trigger signal channel.
    pub process_tx: broadcast::Sender<()>,

    /// Whether workers are currently running.
    pub workers_running: bool,

    /// Scanner shutdown flag.
    pub scanner_shutdown: Arc<AtomicBool>,

    /// Config change broadcast sender (reconciler → listener).
    pub config_change_sender: broadcast::Sender<ConfigChangeEvent>,

    /// Git sync trigger channel.
    pub sync_trigger_tx: broadcast::Sender<()>,

    /// Git reconciler (pull → notify).
    pub reconciler: Option<Arc<GitReconciler>>,

    /// Background sync scheduler.
    pub sync_scheduler: Option<SyncScheduler>,

    /// Config change listener task handle (to prevent duplicates).
    config_listener_handle: Option<tokio::task::JoinHandle<()>>,
}

impl TauriAppState {
    /// Creates a new TauriAppState with default values.
    pub fn new() -> Self {
        let (reload_tx, _) = broadcast::channel(16);
        let (process_tx, _) = broadcast::channel(16);
        let (config_change_sender, _) = broadcast::channel(16);
        let (sync_trigger_tx, _) = broadcast::channel(16);

        Self {
            config_dir: None,
            loaded_config: None,
            worker_pool: None,
            log_broadcaster: Arc::new(LogBroadcaster::default()),
            job_broadcaster: Arc::new(JobProgressBroadcaster::default()),
            git_broadcaster: Arc::new(GitProgressBroadcaster::default()),
            job_store: Arc::new(JobStore::default()),
            reload_tx,
            process_tx,
            workers_running: false,
            scanner_shutdown: Arc::new(AtomicBool::new(false)),
            config_change_sender,
            sync_trigger_tx,
            reconciler: None,
            sync_scheduler: None,
            config_listener_handle: None,
        }
    }

    /// Sets the configuration directory and loads the configuration.
    pub fn set_config_dir(&mut self, path: PathBuf) -> Result<(), String> {
        use paporg::gitops::ConfigLoader;

        let loader = ConfigLoader::new(&path);
        let loaded = loader.load().map_err(|e| e.to_string())?;

        self.config_dir = Some(path);
        self.loaded_config = Some(loaded);

        Ok(())
    }

    /// Returns the loaded configuration, if available.
    pub fn config(&self) -> Option<&LoadedConfig> {
        self.loaded_config.as_ref()
    }

    /// Reloads the configuration from disk.
    pub fn reload(&mut self) -> Result<(), String> {
        let config_dir = self.config_dir.clone().ok_or("No config directory set")?;
        self.set_config_dir(config_dir)
    }

    /// Initializes and starts the worker pool and scanner.
    pub fn start_workers(&mut self) -> Result<(), String> {
        let config = self
            .loaded_config
            .as_ref()
            .ok_or("Configuration not loaded")?;

        let legacy_config = config.to_legacy_config();
        let pipeline_config = Arc::new(PipelineConfig::from_config(&legacy_config));

        // Create worker pool with job progress broadcaster for UI updates
        let job_sender = self.job_broadcaster.sender();
        let pool = Arc::new(WorkerPool::with_progress_sender(
            pipeline_config,
            legacy_config.worker_count,
            Some(job_sender),
        ));
        self.worker_pool = Some(Arc::clone(&pool));
        self.workers_running = true;

        // Reset scanner shutdown flag
        self.scanner_shutdown.store(false, Ordering::Relaxed);

        // Create scanner from config
        let scanner = MultiSourceScanner::from_config(config);

        if scanner.has_sources() {
            info!(
                "Starting scanner with {} import source(s)",
                scanner.source_count()
            );

            // Clone necessary values for the scanner task
            let shutdown = Arc::clone(&self.scanner_shutdown);
            let pool_for_scanner = Arc::clone(&pool);
            let pool_for_results = Arc::clone(&pool);
            let shutdown_for_results = Arc::clone(&self.scanner_shutdown);
            let mut process_rx = self.process_tx.subscribe();

            // Spawn a thread to consume job results (prevents channel from filling up)
            std::thread::spawn(move || {
                while !shutdown_for_results.load(Ordering::Relaxed) {
                    // Try to receive results with a timeout
                    if let Some(result) = pool_for_results.try_recv_result() {
                        if result.success {
                            info!(
                                "Job completed: {} -> {:?}",
                                result.source_path.display(),
                                result.output_path
                            );
                        } else {
                            warn!(
                                "Job failed: {} - {:?}",
                                result.source_path.display(),
                                result.error
                            );
                        }
                    } else {
                        // Sleep a bit to avoid busy-waiting
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
                info!("Result consumer task shutting down");
            });

            // Spawn the scanner thread (runs in background)
            std::thread::spawn(move || {
                // Wait a moment for the event bridge to start up
                // This ensures job progress events are captured
                std::thread::sleep(Duration::from_millis(500));

                // Do an initial scan
                info!("Performing initial scan of import sources...");
                match scanner.scan() {
                    Ok(jobs) => {
                        info!("Initial scan found {} documents", jobs.len());
                        for job in jobs {
                            debug!("Submitting job: {:?}", job.source_path);
                            if let Err(e) = pool_for_scanner.submit(job) {
                                error!("Failed to submit job: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Initial scan failed: {}", e);
                    }
                }

                // Then watch for trigger signals and periodic scans
                let scan_interval = Duration::from_secs(60);
                let check_interval = Duration::from_millis(500);
                let mut time_since_last_scan = Duration::ZERO;

                loop {
                    if shutdown.load(Ordering::Relaxed) {
                        info!("Scanner task shutting down");
                        break;
                    }

                    // Check for manual trigger
                    let triggered = match process_rx.try_recv() {
                        Ok(()) => {
                            info!("Manual scan triggered");
                            true
                        }
                        Err(broadcast::error::TryRecvError::Empty) => false,
                        Err(broadcast::error::TryRecvError::Closed) => {
                            info!("Process trigger channel closed");
                            break;
                        }
                        Err(broadcast::error::TryRecvError::Lagged(_)) => false,
                    };

                    // Check if it's time for a periodic scan
                    let should_scan = triggered || time_since_last_scan >= scan_interval;

                    if should_scan {
                        // Perform scan
                        match scanner.scan() {
                            Ok(jobs) => {
                                if !jobs.is_empty() {
                                    info!("Scan found {} new documents", jobs.len());
                                    for job in jobs {
                                        debug!("Submitting job: {:?}", job.source_path);
                                        if let Err(e) = pool_for_scanner.submit(job) {
                                            error!("Failed to submit job: {}", e);
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Scan failed: {}", e);
                            }
                        }
                        time_since_last_scan = Duration::ZERO;
                    }

                    // Sleep for a bit
                    std::thread::sleep(check_interval);
                    time_since_last_scan += check_interval;
                }
            });
        } else {
            info!("No import sources configured, scanner not started");
        }

        Ok(())
    }

    /// Stops the worker pool and scanner.
    pub fn stop_workers(&mut self) {
        // Signal scanner to stop
        self.scanner_shutdown.store(true, Ordering::Relaxed);

        if let Some(pool) = self.worker_pool.take() {
            pool.shutdown();
            // Note: We can't call wait() here as we don't have exclusive ownership
            // The workers will finish in the background
        }
        self.workers_running = false;
    }

    /// Returns whether workers are currently running.
    pub fn is_workers_running(&self) -> bool {
        self.workers_running
    }

    /// Triggers document processing.
    pub fn trigger_processing(&self) {
        let _ = self.process_tx.send(());
    }

    /// Sets up git sync: creates the reconciler and optionally starts the background scheduler.
    /// Also wires up the config change listener so git-pulled changes auto-reload the UI.
    pub fn setup_git_sync(
        &mut self,
        app: &tauri::AppHandle,
        state_arc: Arc<RwLock<TauriAppState>>,
    ) -> Result<(), String> {
        use paporg::gitops::git::GitRepository;

        let config_dir = self
            .config_dir
            .as_ref()
            .ok_or("setup_git_sync: no config_dir set")?
            .clone();

        let git_settings = self
            .config()
            .ok_or("setup_git_sync: configuration not loaded")?
            .settings
            .resource
            .spec
            .git
            .clone();

        if !git_settings.enabled {
            info!("Git sync not started: git is disabled in settings");
            // Clean up existing sync artifacts from a previous enabled state
            if let Some(scheduler) = self.sync_scheduler.take() {
                scheduler.stop();
            }
            if let Some(handle) = self.config_listener_handle.take() {
                handle.abort();
            }
            self.reconciler = None;
            return Ok(());
        }

        // Stop existing scheduler to prevent thread leak on re-enable
        if let Some(scheduler) = self.sync_scheduler.take() {
            scheduler.stop();
        }

        let repo = GitRepository::new(&config_dir, git_settings.clone());
        let reconciler = Arc::new(GitReconciler::new(repo, self.config_change_sender.clone()));
        self.reconciler = Some(Arc::clone(&reconciler));

        // Start background sync if interval > 0
        if git_settings.sync_interval > 0 {
            let scheduler = SyncScheduler::new(
                reconciler,
                Duration::from_secs(git_settings.sync_interval),
                self.git_broadcaster.clone(),
                self.sync_trigger_tx.clone(),
            );
            scheduler.start(self.sync_trigger_tx.subscribe());
            self.sync_scheduler = Some(scheduler);
        }

        // Abort old config change listener before starting a new one
        if let Some(handle) = self.config_listener_handle.take() {
            handle.abort();
        }

        // Wire up the config change listener
        self.config_listener_handle = Some(Self::start_config_change_listener(
            app.clone(),
            state_arc,
            self.config_change_sender.subscribe(),
        ));

        Ok(())
    }

    /// Starts a config change listener that auto-reloads when git changes are detected.
    /// Subscribes to reconciler events and reloads config + notifies UI.
    /// Returns the JoinHandle so the caller can abort it to prevent duplicates.
    pub fn start_config_change_listener(
        app: tauri::AppHandle,
        state: Arc<RwLock<TauriAppState>>,
        mut change_rx: broadcast::Receiver<ConfigChangeEvent>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                match change_rx.recv().await {
                    Ok(event) => {
                        info!("Config change event: {:?}", event.change_type);
                        let mut state_write = state.write().await;
                        if let Err(e) = state_write.reload() {
                            error!("Failed to reload config after git sync: {}", e);
                        }
                        drop(state_write);
                        crate::events::emit_config_changed(&app);
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Config change listener lagged by {} events", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("Config change channel closed, stopping listener");
                        break;
                    }
                }
            }
        })
    }

    /// Gracefully shuts down all background tasks and threads.
    pub fn shutdown(&mut self) {
        info!("Shutting down app state...");

        // Stop sync scheduler (sets flag, wakes thread, joins)
        if let Some(scheduler) = self.sync_scheduler.take() {
            scheduler.stop();
        }

        // Abort config change listener
        if let Some(handle) = self.config_listener_handle.take() {
            handle.abort();
        }

        // Stop workers and scanner
        self.stop_workers();

        info!("App state shutdown complete");
    }
}

impl Default for TauriAppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the default config directory path for the current platform.
/// - macOS: ~/Library/Application Support/paporg
/// - Linux: ~/.config/paporg
/// - Windows: %APPDATA%/paporg
pub fn default_config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("paporg"))
}

/// Ensures the config directory exists and has a minimal settings file.
/// Creates the directory and default files if they don't exist.
pub fn ensure_config_initialized(config_dir: &PathBuf) -> Result<(), String> {
    // Create the config directory if it doesn't exist
    if !config_dir.exists() {
        info!("Creating config directory: {:?}", config_dir);
        fs::create_dir_all(config_dir)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    // Create subdirectories
    let rules_dir = config_dir.join("rules");
    let sources_dir = config_dir.join("sources");

    if !rules_dir.exists() {
        fs::create_dir_all(&rules_dir)
            .map_err(|e| format!("Failed to create rules directory: {}", e))?;
    }
    if !sources_dir.exists() {
        fs::create_dir_all(&sources_dir)
            .map_err(|e| format!("Failed to create sources directory: {}", e))?;
    }

    // Create default settings file if it doesn't exist
    let settings_path = config_dir.join("settings.yaml");
    if !settings_path.exists() {
        info!("Creating default settings.yaml");

        // Determine platform-appropriate default directories
        let (input_dir, output_dir) = get_default_directories();

        let default_settings = format!(
            r#"apiVersion: paporg.io/v1
kind: Settings
metadata:
  name: settings
spec:
  inputDirectory: "{}"
  outputDirectory: "{}"
  archiveDirectory: "archive"
  workerCount: 2
  ocr:
    enabled: true
    languages:
      - eng
    dpi: 300
  defaults:
    output:
      directory: unsorted
      filename: "$original"
"#,
            input_dir.replace('\\', "/"),
            output_dir.replace('\\', "/")
        );

        fs::write(&settings_path, default_settings)
            .map_err(|e| format!("Failed to write settings.yaml: {}", e))?;

        // Create the input and output directories
        let input_path = PathBuf::from(&input_dir);
        let output_path = PathBuf::from(&output_dir);

        if !input_path.exists() {
            info!("Creating input directory: {:?}", input_path);
            if let Err(e) = fs::create_dir_all(&input_path) {
                warn!("Failed to create input directory: {}", e);
            }
        }
        if !output_path.exists() {
            info!("Creating output directory: {:?}", output_path);
            if let Err(e) = fs::create_dir_all(&output_path) {
                warn!("Failed to create output directory: {}", e);
            }
        }
    }

    // Create .gitignore to prevent logs, database, and temp files from being
    // committed when git sync is enabled on the config directory.
    let gitignore_path = config_dir.join(".gitignore");
    if !gitignore_path.exists() {
        let gitignore = "\
# Logs
logs/

# SQLite database
*.db
*.db-wal
*.db-shm

# Temporary upload inbox
inbox/
";
        if let Err(e) = fs::write(&gitignore_path, gitignore) {
            warn!("Failed to write .gitignore: {}", e);
        }
    }

    // Create a sample rule file if no rules exist
    let sample_rule_path = rules_dir.join("sample-invoice.yaml");
    if !sample_rule_path.exists()
        && fs::read_dir(&rules_dir)
            .map(|mut d| d.next().is_none())
            .unwrap_or(true)
    {
        info!("Creating sample rule");
        let sample_rule = r#"apiVersion: paporg.io/v1
kind: Rule
metadata:
  name: sample-invoice
spec:
  priority: 50
  category: invoices
  match:
    containsAny:
      - Invoice
      - invoice
      - INVOICE
      - Rechnung
  output:
    directory: "$category/$y"
    filename: "$y-$m-$d_$original"
"#;
        if let Err(e) = fs::write(&sample_rule_path, sample_rule) {
            warn!("Failed to write sample rule: {}", e);
        }
    }

    Ok(())
}

/// Returns platform-appropriate default input and output directories.
fn get_default_directories() -> (String, String) {
    if let Some(home) = dirs::home_dir() {
        let documents = dirs::document_dir().unwrap_or_else(|| home.join("Documents"));
        let input = documents.join("Paporg").join("Input");
        let output = documents.join("Paporg").join("Output");
        (
            input.to_string_lossy().to_string(),
            output.to_string_lossy().to_string(),
        )
    } else {
        // Fallback for edge cases
        ("./input".to_string(), "./output".to_string())
    }
}
