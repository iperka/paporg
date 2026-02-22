// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod db;
mod events;
mod state;
mod tray;

use std::sync::{Arc, Mutex};

use tauri::{Manager, RunEvent};
use tokio::sync::RwLock;
use tracing::info;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter, Registry};

#[cfg(target_os = "macos")]
use tauri::window::EffectsBuilder;

use state::{default_config_dir, ensure_config_initialized, TauriAppState};

/// Max log file size before rotation (5 MB).
const MAX_LOG_SIZE: u64 = 5 * 1024 * 1024;

/// Set up persistent log file in the config directory.
/// Rotates the previous log to `paporg.log.1` if it exceeds `MAX_LOG_SIZE`.
fn open_log_file(config_dir: &std::path::Path) -> Option<std::fs::File> {
    let logs_dir = config_dir.join("logs");
    if std::fs::create_dir_all(&logs_dir).is_err() {
        return None;
    }

    let log_path = logs_dir.join("paporg.log");

    // Rotate if existing log is too large
    if let Ok(meta) = std::fs::metadata(&log_path) {
        if meta.len() > MAX_LOG_SIZE {
            let rotated = logs_dir.join("paporg.log.1");
            let _ = std::fs::rename(&log_path, rotated);
        }
    }

    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok()
}

/// Initialize the SeaORM database connection for the JobStore.
async fn init_job_store_database(
    config_dir: &std::path::Path,
    job_store: &paporg::broadcast::JobStore,
) {
    let _span = tracing::info_span!("init_job_store_database").entered();
    let db_url = paporg::db::default_database_path(config_dir);
    match paporg::db::init_database(&db_url).await {
        Ok(conn) => {
            job_store.set_database(conn).await;
            // Load existing jobs from database into cache
            job_store.load_from_database().await;
            info!("Job store database initialized successfully");
        }
        Err(e) => {
            tracing::error!("Failed to initialize job store database: {}", e);
        }
    }
}

fn main() {
    // Create a Tokio runtime early — the OTel batch exporter needs one to spawn
    // its background flush task. This runtime is also used by Tauri.
    #[cfg(feature = "otel")]
    let _otel_runtime = {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
        // Enter the runtime context so tokio::spawn works during OTel init.
        // SAFETY: We leak the EnterGuard so the context stays active for the
        // lifetime of the process. The runtime itself lives until main() returns.
        std::mem::forget(rt.enter());
        rt
    };

    // Create log broadcaster early so we can wire it into the tracing subscriber
    let log_broadcaster = Arc::new(paporg::LogBroadcaster::default());

    // Bridge log:: macros from third-party crates (sea-orm, notify, etc.) into tracing
    tracing_log::LogTracer::init().expect("Failed to initialize LogTracer");

    // Build env filter — honours RUST_LOG, defaults to info
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Stderr layer (with ANSI, timestamps)
    let fmt_stderr = fmt::layer().with_target(true).with_ansi(true);

    // File layer (no ANSI, append mode) — wrapped in Option so types align
    let fmt_file = default_config_dir()
        .and_then(|dir| open_log_file(&dir))
        .map(|file| {
            fmt::layer()
                .with_target(true)
                .with_ansi(false)
                .with_writer(Mutex::new(file))
        });

    // Broadcast layer — forwards events to the LogBroadcaster for the frontend
    let broadcast_layer = paporg::broadcast::BroadcastLayer::new(log_broadcaster.clone());

    // Assemble the subscriber
    let subscriber = Registry::default()
        .with(env_filter)
        .with(fmt_stderr)
        .with(fmt_file)
        .with(broadcast_layer);

    // Conditionally add OTel trace + Loki layers
    #[cfg(feature = "otel")]
    let subscriber = {
        use opentelemetry::trace::TracerProvider;

        let provider = build_otel_provider();
        let tracer = provider.tracer("paporg-desktop");
        let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
        let (loki_layer, loki_task) = build_loki_layer();

        // Store provider for graceful shutdown and Loki task for deferred spawning
        OTEL_PROVIDER.lock().unwrap().replace(provider);
        LOKI_TASK.lock().unwrap().replace(Box::pin(loki_task));

        subscriber.with(otel_layer).with(loki_layer)
    };

    // Use set_global_default instead of .init() because we already called
    // LogTracer::init() above — .init() would call it again and panic.
    tracing::subscriber::set_global_default(subscriber)
        .expect("failed to set global default subscriber");

    info!("Starting Paporg Desktop v{}", env!("CARGO_PKG_VERSION"));

    // Start continuous profiling if otel feature is enabled
    #[cfg(feature = "otel")]
    let _profiler = paporg::profiling::start_continuous_profiling();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations("sqlite:paporg.db", db::migrations())
                .build(),
        )
        .setup(move |app| {
            let app_handle = app.handle().clone();

            // Initialize app state
            let mut state = TauriAppState::new();

            // Inject the pre-created log broadcaster so the event bridge uses the same one
            state.log_broadcaster = log_broadcaster;

            // Spawn Loki background task if otel is enabled
            #[cfg(feature = "otel")]
            {
                if let Some(task) = LOKI_TASK.lock().unwrap().take() {
                    tauri::async_runtime::spawn(task);
                }
            }

            // Auto-initialize default config directory
            if let Some(config_dir) = default_config_dir() {
                info!("Using config directory: {:?}", config_dir);

                // Ensure the directory exists with default files
                if let Err(e) = ensure_config_initialized(&config_dir) {
                    tracing::warn!("Failed to initialize config directory: {}", e);
                } else {
                    // Try to load the configuration
                    match state.set_config_dir(config_dir.clone()) {
                        Ok(()) => {
                            info!("Configuration loaded successfully from {:?}", config_dir);

                            // Auto-start workers after config is loaded
                            match state.start_workers() {
                                Ok(()) => {
                                    info!("Workers started automatically");
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to auto-start workers: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to load config from {:?}: {}", config_dir, e);
                            // Still set the config_dir so the UI knows where to save
                            state.config_dir = Some(config_dir);
                        }
                    }
                }
            } else {
                tracing::warn!("Could not determine default config directory");
            }

            // Get a reference to job_store and config_dir for database initialization
            let job_store = state.job_store.clone();
            let db_config_dir = state.config_dir.clone();

            app.manage(Arc::new(RwLock::new(state)));

            // Set up system tray
            tray::setup_tray(&app_handle)?;

            // Apply macOS vibrancy effect for transparent background
            #[cfg(target_os = "macos")]
            {
                use tauri::window::Effect;
                if let Some(window) = app.get_webview_window("main") {
                    let effects = EffectsBuilder::new()
                        .effect(Effect::UnderWindowBackground)
                        .state(tauri::window::EffectState::FollowsWindowActiveState)
                        .build();
                    let _ = window.set_effects(effects);
                    info!("Applied macOS vibrancy effect");
                }
            }

            // Initialize job store database (blocking to ensure DB is ready before event bridge starts)
            if let Some(config_dir) = db_config_dir {
                let job_store_clone = job_store.clone();
                tauri::async_runtime::block_on(async move {
                    init_job_store_database(&config_dir, &job_store_clone).await;
                });
            }

            // Start event bridge
            let handle_clone = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                events::start_event_bridge(handle_clone).await;
            });

            info!("Paporg Desktop initialized");
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Hide window instead of closing for background processing
                info!("Window close requested, hiding to system tray");
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            // Config commands
            commands::get_config,
            commands::reload_config,
            commands::select_config_directory,
            commands::health_check,
            // Worker commands
            commands::get_worker_status,
            commands::start_workers,
            commands::stop_workers,
            commands::trigger_processing,
            // Job commands
            commands::get_jobs,
            commands::query_jobs,
            commands::get_job,
            commands::get_job_ocr,
            commands::rerun_job,
            commands::ignore_job,
            commands::rerun_unsorted,
            // GitOps commands
            commands::get_file_tree,
            commands::list_gitops_resources,
            commands::get_gitops_resource,
            commands::create_gitops_resource,
            commands::update_gitops_resource,
            commands::delete_gitops_resource,
            commands::simulate_rule,
            commands::validate_config,
            // Git commands
            commands::git_status,
            commands::git_pull,
            commands::git_commit,
            commands::git_branches,
            commands::git_checkout,
            commands::git_create_branch,
            commands::git_merge_status,
            commands::git_diff,
            commands::git_log,
            commands::git_cancel_operation,
            commands::git_initialize,
            // Email OAuth commands
            commands::start_email_authorization,
            commands::check_authorization_status,
            commands::get_token_status,
            commands::revoke_token,
            // Secret commands
            commands::write_secret,
            // AI commands
            commands::ai_status,
            commands::download_ai_model,
            commands::suggest_rule,
            // Analytics commands
            commands::track_event,
            // File commands
            commands::move_file,
            commands::create_directory,
            commands::delete_file,
            commands::read_raw_file,
            commands::write_raw_file,
            commands::pick_folder,
            commands::pick_file,
            // Upload commands
            commands::upload_files,
            commands::pick_and_upload_files,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let RunEvent::Exit = event {
                info!("Application exiting, performing graceful shutdown...");

                // Flush OTel traces before exit
                #[cfg(feature = "otel")]
                if let Some(provider) = OTEL_PROVIDER.lock().unwrap().take() {
                    let _ = provider.shutdown();
                }

                let state: &Arc<RwLock<TauriAppState>> =
                    app_handle.state::<Arc<RwLock<TauriAppState>>>().inner();
                // Use block_on since we're in the exit handler
                let mut state_write = tauri::async_runtime::block_on(state.write());
                state_write.shutdown();
            }
        });
}

// ========================================================================
// OTel helpers (behind feature gate)
// ========================================================================

#[cfg(feature = "otel")]
use std::future::Future;
#[cfg(feature = "otel")]
use std::pin::Pin;

#[cfg(feature = "otel")]
static LOKI_TASK: std::sync::Mutex<Option<Pin<Box<dyn Future<Output = ()> + Send>>>> =
    std::sync::Mutex::new(None);

#[cfg(feature = "otel")]
static OTEL_PROVIDER: std::sync::Mutex<Option<opentelemetry_sdk::trace::SdkTracerProvider>> =
    std::sync::Mutex::new(None);

#[cfg(feature = "otel")]
fn build_otel_provider() -> opentelemetry_sdk::trace::SdkTracerProvider {
    use opentelemetry::KeyValue;
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use opentelemetry_sdk::Resource;

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .build()
        .expect("Failed to build OTLP span exporter");

    let resource = Resource::builder()
        .with_attributes([
            KeyValue::new("service.name", "paporg-desktop"),
            KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        ])
        .build();

    SdkTracerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(exporter)
        .build()
}

#[cfg(feature = "otel")]
fn build_loki_layer() -> (
    tracing_loki::Layer,
    impl Future<Output = ()> + Send + 'static,
) {
    let (layer, task) = tracing_loki::builder()
        .label("service_name", "paporg-desktop")
        .expect("invalid label")
        .build_url(tracing_loki::url::Url::parse("http://localhost:3100").unwrap())
        .expect("Failed to build Loki layer");

    (layer, task)
}
