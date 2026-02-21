// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod db;
mod events;
mod state;
mod tray;

use std::sync::Arc;

use log::info;
use tauri::Manager;
use tokio::sync::RwLock;

#[cfg(target_os = "macos")]
use tauri::window::EffectsBuilder;

use state::{default_config_dir, ensure_config_initialized, TauriAppState};

/// Initialize the SeaORM database connection for the JobStore.
async fn init_job_store_database(
    config_dir: &std::path::Path,
    job_store: &paporg::broadcast::JobStore,
) {
    let db_url = paporg::db::default_database_path(config_dir);
    match paporg::db::init_database(&db_url).await {
        Ok(conn) => {
            job_store.set_database(conn).await;
            // Load existing jobs from database into cache
            job_store.load_from_database().await;
            info!("Job store database initialized successfully");
        }
        Err(e) => {
            log::error!("Failed to initialize job store database: {}", e);
        }
    }
}

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    info!("Starting Paporg Desktop v{}", env!("CARGO_PKG_VERSION"));

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
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Initialize app state
            let mut state = TauriAppState::new();

            // Auto-initialize default config directory
            if let Some(config_dir) = default_config_dir() {
                info!("Using config directory: {:?}", config_dir);

                // Ensure the directory exists with default files
                if let Err(e) = ensure_config_initialized(&config_dir) {
                    log::warn!("Failed to initialize config directory: {}", e);
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
                                    log::warn!("Failed to auto-start workers: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("Failed to load config from {:?}: {}", config_dir, e);
                            // Still set the config_dir so the UI knows where to save
                            state.config_dir = Some(config_dir);
                        }
                    }
                }
            } else {
                log::warn!("Could not determine default config directory");
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
