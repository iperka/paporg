//! Git operation commands.

use std::sync::Arc;

use paporg::gitops::git::{
    BranchInfo, CommitInfo, CommitResult, GitRepository, GitStatus, InitializeResult, MergeStatus,
    PullResult,
};
use paporg::gitops::progress::GitOperationType;
use std::path::PathBuf;
use tauri::State;
use tokio::sync::RwLock;

use super::ApiResponse;
use crate::state::TauriAppState;

// ============================================================================
// Helpers
// ============================================================================

/// Helper to reload config and emit a config-changed event.
/// Used by commands that modify the working tree (checkout, merge, initialize).
async fn reload_and_notify(app: &tauri::AppHandle, state: &State<'_, Arc<RwLock<TauriAppState>>>) {
    let mut state_write = state.write().await;
    if let Err(e) = state_write.reload() {
        log::error!("Failed to reload config after git operation: {}", e);
    }
    drop(state_write);
    crate::events::emit_config_changed(app);
}

/// Helper to create a GitRepository from current state.
/// Returns None with an error response if state is not ready.
fn make_repo(state: &TauriAppState) -> Result<GitRepository, ApiResponse<()>> {
    let config_dir = state
        .config_dir
        .as_ref()
        .ok_or_else(|| ApiResponse::err("No config directory set"))?;

    let git_settings = state
        .config()
        .map(|c| c.settings.resource.spec.git.clone())
        .ok_or_else(|| ApiResponse::err("Configuration not loaded"))?;

    Ok(GitRepository::new(config_dir, git_settings))
}

// ============================================================================
// Commands
// ============================================================================

/// Get git status.
#[tauri::command]
pub async fn git_status(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
) -> Result<ApiResponse<GitStatus>, String> {
    let state = state.read().await;

    let repo = match make_repo(&state) {
        Ok(r) => r,
        Err(e) => return Ok(ApiResponse::err(e.error.unwrap_or_default())),
    };

    if !repo.is_git_repo() {
        return Ok(ApiResponse::ok(GitStatus {
            is_repo: false,
            branch: None,
            is_clean: true,
            ahead: 0,
            behind: 0,
            modified_files: Vec::new(),
            untracked_files: Vec::new(),
            files: Vec::new(),
        }));
    }

    match repo.status() {
        Ok(status) => Ok(ApiResponse::ok(status)),
        Err(e) => Ok(ApiResponse::err(e.to_string())),
    }
}

/// Git pull — uses the reconciler so reload+notify happens automatically.
#[tauri::command]
pub async fn git_pull(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
) -> Result<ApiResponse<PullResult>, String> {
    let state_guard = state.read().await;

    let reconciler = match &state_guard.reconciler {
        Some(r) => Arc::clone(r),
        None => {
            return Ok(ApiResponse::err(
                "Git not initialized. Run initialize first.",
            ))
        }
    };

    let broadcaster = state_guard.git_broadcaster.clone();
    drop(state_guard);

    let progress = broadcaster.start_operation(GitOperationType::Pull);
    let op_id = progress.operation_id().to_string();
    let result = match reconciler.reconcile(&progress).await {
        Ok(result) => Ok(ApiResponse::ok(result.pull_result)),
        Err(e) => Ok(ApiResponse::err(e.to_string())),
    };
    broadcaster.complete_operation(&op_id);
    result
}

/// Git commit and push.
#[tauri::command]
pub async fn git_commit(
    app: tauri::AppHandle,
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    message: String,
    files: Option<Vec<String>>,
) -> Result<ApiResponse<CommitResult>, String> {
    let state_guard = state.read().await;

    let config_dir = match &state_guard.config_dir {
        Some(dir) => dir.clone(),
        None => return Ok(ApiResponse::err("No config directory set")),
    };

    let git_settings = match state_guard.config() {
        Some(c) => c.settings.resource.spec.git.clone(),
        None => return Ok(ApiResponse::err("Configuration not loaded")),
    };

    if !git_settings.enabled {
        return Ok(ApiResponse::err("Git is not enabled in settings"));
    }

    let git_broadcaster = state_guard.git_broadcaster.clone();
    drop(state_guard);

    let repo = GitRepository::new(&config_dir, git_settings);
    let progress = git_broadcaster.start_operation(GitOperationType::Commit);
    let op_id = progress.operation_id().to_string();

    let files_ref: Option<Vec<&str>> = files
        .as_ref()
        .map(|f| f.iter().map(|s| s.as_str()).collect());

    let result = match repo
        .commit_and_push_with_progress(&message, files_ref.as_deref(), &progress)
        .await
    {
        Ok(result) => {
            if result.commit_hash.is_some() {
                crate::events::emit_config_changed(&app);
            }
            Ok(ApiResponse::ok(result))
        }
        Err(e) => Ok(ApiResponse::err(e.to_string())),
    };
    git_broadcaster.complete_operation(&op_id);
    result
}

/// List branches.
#[tauri::command]
pub async fn git_branches(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
) -> Result<ApiResponse<Vec<BranchInfo>>, String> {
    let state = state.read().await;

    let repo = match make_repo(&state) {
        Ok(r) => r,
        Err(e) => return Ok(ApiResponse::err(e.error.unwrap_or_default())),
    };

    if !repo.is_git_repo() {
        return Ok(ApiResponse::ok(Vec::new()));
    }

    match repo.list_branches() {
        Ok(branches) => Ok(ApiResponse::ok(branches)),
        Err(e) => Ok(ApiResponse::err(e.to_string())),
    }
}

/// Checkout branch.
#[tauri::command]
pub async fn git_checkout(
    app: tauri::AppHandle,
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    branch: String,
) -> Result<ApiResponse<()>, String> {
    let state_guard = state.read().await;

    let repo = match make_repo(&state_guard) {
        Ok(r) => r,
        Err(e) => return Ok(ApiResponse::err(e.error.unwrap_or_default())),
    };
    drop(state_guard);

    match repo.checkout(&branch) {
        Ok(()) => {
            reload_and_notify(&app, &state).await;
            Ok(ApiResponse::ok(()))
        }
        Err(e) => Ok(ApiResponse::err(e.to_string())),
    }
}

/// Create branch.
#[tauri::command]
pub async fn git_create_branch(
    app: tauri::AppHandle,
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    name: String,
    checkout: bool,
) -> Result<ApiResponse<()>, String> {
    let state_guard = state.read().await;

    let repo = match make_repo(&state_guard) {
        Ok(r) => r,
        Err(e) => return Ok(ApiResponse::err(e.error.unwrap_or_default())),
    };
    drop(state_guard);

    match repo.create_branch(&name, checkout) {
        Ok(()) => {
            if checkout {
                reload_and_notify(&app, &state).await;
            }
            // No event when checkout=false — only branch metadata changed, not config
            Ok(ApiResponse::ok(()))
        }
        Err(e) => Ok(ApiResponse::err(e.to_string())),
    }
}

/// Get merge status.
#[tauri::command]
pub async fn git_merge_status(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
) -> Result<ApiResponse<MergeStatus>, String> {
    let state = state.read().await;

    let config_dir = match &state.config_dir {
        Some(dir) => dir.clone(),
        None => return Ok(ApiResponse::err("No config directory set")),
    };

    let git_settings = match state.config() {
        Some(c) => c.settings.resource.spec.git.clone(),
        None => return Ok(ApiResponse::err("Configuration not loaded")),
    };

    if !git_settings.enabled {
        return Ok(ApiResponse::err("Git is not enabled in settings"));
    }

    let branch = git_settings.branch.clone();
    let repo = GitRepository::new(&config_dir, git_settings);

    match repo.merge_status(&branch) {
        Ok(status) => Ok(ApiResponse::ok(status)),
        Err(e) => Ok(ApiResponse::err(e.to_string())),
    }
}

/// Get git log (recent commits).
#[tauri::command]
pub async fn git_log(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    limit: Option<u32>,
) -> Result<ApiResponse<Vec<CommitInfo>>, String> {
    let state = state.read().await;

    let repo = match make_repo(&state) {
        Ok(r) => r,
        Err(e) => return Ok(ApiResponse::err(e.error.unwrap_or_default())),
    };

    if !repo.is_git_repo() {
        return Ok(ApiResponse::ok(Vec::new()));
    }

    match repo.log(limit.unwrap_or(20)) {
        Ok(commits) => Ok(ApiResponse::ok(commits)),
        Err(e) => Ok(ApiResponse::err(e.to_string())),
    }
}

/// Cancel an active git operation.
#[tauri::command]
pub async fn git_cancel_operation(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    operation_id: String,
) -> Result<ApiResponse<bool>, String> {
    let state = state.read().await;
    let cancelled = state.git_broadcaster.cancel_operation(&operation_id);
    Ok(ApiResponse::ok(cancelled))
}

/// Get diff for a file or all files.
#[tauri::command]
pub async fn git_diff(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    file: Option<String>,
    cached: Option<bool>,
) -> Result<ApiResponse<String>, String> {
    let state = state.read().await;

    let repo = match make_repo(&state) {
        Ok(r) => r,
        Err(e) => return Ok(ApiResponse::err(e.error.unwrap_or_default())),
    };

    if !repo.is_git_repo() {
        return Ok(ApiResponse::ok(String::new()));
    }

    match repo.diff(file.as_deref(), cached.unwrap_or(false)) {
        Ok(diff) => Ok(ApiResponse::ok(diff)),
        Err(e) => Ok(ApiResponse::err(e.to_string())),
    }
}

/// Initialize git repository.
#[tauri::command]
pub async fn git_initialize(
    app: tauri::AppHandle,
    state: State<'_, Arc<RwLock<TauriAppState>>>,
) -> Result<ApiResponse<InitializeResult>, String> {
    let state_guard = state.read().await;

    let config_dir = match &state_guard.config_dir {
        Some(dir) => dir.clone(),
        None => return Ok(ApiResponse::err("No config directory set")),
    };

    let git_settings = match state_guard.config() {
        Some(c) => c.settings.resource.spec.git.clone(),
        None => return Ok(ApiResponse::err("Configuration not loaded")),
    };

    if !git_settings.enabled {
        return Ok(ApiResponse::err("Git is not enabled in settings"));
    }

    if git_settings.repository.is_empty() {
        return Ok(ApiResponse::err("Git repository URL is not configured"));
    }

    let remote_url = git_settings.repository.clone();
    let branch = git_settings.branch.clone();
    drop(state_guard);

    let repo = GitRepository::new(&config_dir, git_settings);

    // Step 1: Initialize if not a repo
    if !repo.is_git_repo() {
        if let Err(e) = repo.init() {
            return Ok(ApiResponse::err(format!("Failed to initialize git: {}", e)));
        }
    }

    // Step 2: Set remote
    if let Err(e) = repo.set_remote(&remote_url) {
        return Ok(ApiResponse::err(format!("Failed to set remote: {}", e)));
    }

    // Step 3: Fetch from remote
    if let Err(e) = repo.fetch(&branch) {
        let error_msg = e.to_string();
        if error_msg.contains("couldn't find remote ref") || error_msg.contains("not found") {
            // Set up reconciler now that git is initialized
            let mut state_write = state.write().await;
            if let Err(e) = state_write.setup_git_sync(&app, Arc::clone(&*state)) {
                log::warn!("Failed to setup git sync: {}", e);
            }
            drop(state_write);

            return Ok(ApiResponse::ok(InitializeResult {
                initialized: true,
                merged: false,
                message: "Repository initialized. Remote branch will be created on first push."
                    .to_string(),
                conflicting_files: Vec::new(),
            }));
        }
        return Ok(ApiResponse::err(format!(
            "Failed to fetch from remote: {}",
            e
        )));
    }

    // Step 4: Check if local repo has any commits
    let has_local_commits = repo.has_commits();

    if !has_local_commits {
        // No local commits — checkout from remote.  Use force checkout so
        // existing local files (e.g. after a disconnect+reconnect) don't
        // block the operation.
        match repo.force_checkout_remote_branch(&branch) {
            Ok(()) => {
                reload_and_notify(&app, &state).await;

                // Set up reconciler now that git is initialized
                let mut state_write = state.write().await;
                if let Err(e) = state_write.setup_git_sync(&app, Arc::clone(&*state)) {
                    log::warn!("Failed to setup git sync: {}", e);
                }
                drop(state_write);

                return Ok(ApiResponse::ok(InitializeResult {
                    initialized: true,
                    merged: true,
                    message: "Repository initialized from remote.".to_string(),
                    conflicting_files: Vec::new(),
                }));
            }
            Err(e) => {
                return Ok(ApiResponse::err(format!(
                    "Failed to initialize from remote: {}",
                    e
                )));
            }
        }
    }

    // Step 5: Check merge status
    let merge_status = match repo.merge_status(&branch) {
        Ok(status) => status,
        Err(e) => {
            return Ok(ApiResponse::err(format!(
                "Failed to check merge status: {}",
                e
            )));
        }
    };

    if merge_status.behind == 0 {
        // Set up reconciler
        let mut state_write = state.write().await;
        if let Err(e) = state_write.setup_git_sync(&app, Arc::clone(&*state)) {
            log::warn!("Failed to setup git sync: {}", e);
        }
        drop(state_write);

        return Ok(ApiResponse::ok(InitializeResult {
            initialized: true,
            merged: false,
            message: "Repository initialized and up to date.".to_string(),
            conflicting_files: Vec::new(),
        }));
    }

    if merge_status.has_conflicts {
        return Ok(ApiResponse::ok(InitializeResult {
            initialized: true,
            merged: false,
            message: "Repository initialized but merge has conflicts.".to_string(),
            conflicting_files: merge_status.conflicting_files,
        }));
    }

    // Step 6: Perform merge
    match repo.merge(&branch) {
        Ok(result) => {
            if result.success {
                reload_and_notify(&app, &state).await;

                // Set up reconciler
                let mut state_write = state.write().await;
                if let Err(e) = state_write.setup_git_sync(&app, Arc::clone(&*state)) {
                    log::warn!("Failed to setup git sync: {}", e);
                }
                drop(state_write);

                Ok(ApiResponse::ok(InitializeResult {
                    initialized: true,
                    merged: true,
                    message: format!(
                        "Repository initialized and merged {} files.",
                        result.merged_files
                    ),
                    conflicting_files: Vec::new(),
                }))
            } else {
                let _ = repo.merge_abort();

                Ok(ApiResponse::ok(InitializeResult {
                    initialized: true,
                    merged: false,
                    message: "Repository initialized but merge has conflicts.".to_string(),
                    conflicting_files: result.conflicting_files,
                }))
            }
        }
        Err(e) => {
            let _ = repo.merge_abort();
            Ok(ApiResponse::err(format!("Failed to merge: {}", e)))
        }
    }
}

/// Disconnect git repository — tears down sync and removes the .git directory.
#[tauri::command]
pub async fn git_disconnect(
    app: tauri::AppHandle,
    state: State<'_, Arc<RwLock<TauriAppState>>>,
) -> Result<ApiResponse<()>, String> {
    let mut state_write = state.write().await;

    let config_dir: PathBuf = match &state_write.config_dir {
        Some(dir) => dir.clone(),
        None => return Ok(ApiResponse::err("No config directory set")),
    };

    // Tear down git sync first so nothing accesses .git while we delete it
    state_write.tear_down_git_sync();
    drop(state_write);

    // Remove the .git directory
    let git_dir = config_dir.join(".git");
    if git_dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&git_dir) {
            return Ok(ApiResponse::err(format!(
                "Failed to remove .git directory: {}",
                e
            )));
        }
    }

    crate::events::emit_config_changed(&app);
    Ok(ApiResponse::ok(()))
}
