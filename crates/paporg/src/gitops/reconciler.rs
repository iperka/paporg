//! Git reconciler: pull → reload → notify.
//!
//! Inspired by ArgoCD's reconciliation loop. Single responsibility:
//! pull changes from remote, then broadcast a config change event
//! so subscribers (Tauri state) can reload config and update the UI.

use serde::Serialize;
use tokio::sync::{broadcast, Mutex};

use super::git::repository::GitRepository;
use super::git::types::PullResult;
use super::progress::OperationProgress;
use super::watcher::{ChangeType, ConfigChangeEvent};
use crate::gitops::error::Result;

/// Result of a reconciliation cycle.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileResult {
    /// The underlying pull result.
    pub pull_result: PullResult,
    /// Whether a config reload was triggered.
    pub config_reloaded: bool,
}

/// Maximum number of retries for transient errors.
const MAX_RETRIES: u32 = 3;
/// Base delay for exponential backoff (in seconds).
const RETRY_BASE_DELAY_SECS: u64 = 2;

/// Reconciles git remote state with local config.
///
/// Atomic operation: pull → notify subscribers if files changed.
/// Uses a mutex to prevent concurrent reconciliation.
pub struct GitReconciler {
    repo: GitRepository,
    config_change_sender: broadcast::Sender<ConfigChangeEvent>,
    /// Prevents concurrent reconcile calls from corrupting the repo.
    reconcile_lock: Mutex<()>,
}

impl GitReconciler {
    /// Creates a new reconciler.
    pub fn new(
        repo: GitRepository,
        config_change_sender: broadcast::Sender<ConfigChangeEvent>,
    ) -> Self {
        Self {
            repo,
            config_change_sender,
            reconcile_lock: Mutex::new(()),
        }
    }

    /// Returns a reference to the underlying repository.
    pub fn repo(&self) -> &GitRepository {
        &self.repo
    }

    /// Pull from remote. If files changed, broadcast a Reloaded event
    /// so subscribers (Tauri state) can reload config and update UI.
    ///
    /// Uses a lock to prevent concurrent reconcile calls. Returns early
    /// if another reconcile is already in progress.
    /// Retries transient errors (network, timeout) with exponential backoff.
    pub async fn reconcile(&self, progress: &OperationProgress) -> Result<ReconcileResult> {
        // Try to acquire the lock; skip if already reconciling
        let _guard = match self.reconcile_lock.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                log::info!("Reconcile skipped: another reconcile is already in progress");
                return Ok(ReconcileResult {
                    pull_result: PullResult {
                        success: true,
                        message: "Skipped: reconcile already in progress".to_string(),
                        files_changed: 0,
                    },
                    config_reloaded: false,
                });
            }
        };

        let mut last_error = None;

        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                let delay = RETRY_BASE_DELAY_SECS * (1 << (attempt - 1)); // 2s, 4s, 8s
                log::info!(
                    "Retrying reconcile (attempt {}/{}) after {}s...",
                    attempt + 1,
                    MAX_RETRIES + 1,
                    delay
                );
                tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
            }

            match self.repo.pull_with_progress(progress).await {
                Ok(pull_result) => {
                    let config_reloaded = pull_result.files_changed > 0;
                    if config_reloaded {
                        if let Err(e) = self.config_change_sender.send(ConfigChangeEvent {
                            change_type: ChangeType::Reloaded,
                            path: String::new(),
                            resource_kind: None,
                            resource_name: None,
                        }) {
                            log::debug!(
                                "No config change listeners active (all receivers dropped): {}",
                                e
                            );
                        }
                    }

                    return Ok(ReconcileResult {
                        pull_result,
                        config_reloaded,
                    });
                }
                Err(e) => {
                    if e.is_retryable() && attempt < MAX_RETRIES {
                        log::warn!("Reconcile failed with retryable error: {}", e);
                        last_error = Some(e);
                        continue;
                    }
                    return Err(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            crate::gitops::error::GitOpsError::GitOperation(
                "Reconcile failed after all retries".to_string(),
            )
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gitops::resource::GitSettings;
    use tempfile::TempDir;

    fn setup_reconciler() -> (
        TempDir,
        GitReconciler,
        broadcast::Receiver<ConfigChangeEvent>,
    ) {
        let dir = TempDir::new().unwrap();
        let repo = GitRepository::new(dir.path(), GitSettings::default());
        let (tx, rx) = broadcast::channel(16);
        let reconciler = GitReconciler::new(repo, tx);
        (dir, reconciler, rx)
    }

    #[test]
    fn test_reconcile_pull_failure_no_event() {
        // Reconciling on a non-git directory should fail without sending events
        let (_dir, reconciler, mut rx) = setup_reconciler();
        let rt = tokio::runtime::Runtime::new().unwrap();

        let broadcaster = crate::broadcast::GitProgressBroadcaster::default();
        let progress = broadcaster.start_operation(crate::gitops::progress::GitOperationType::Pull);

        let result = rt.block_on(reconciler.reconcile(&progress));
        assert!(result.is_err());

        // No event should have been sent
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_reconcile_no_changes_no_event() {
        // Set up an actual git repo with no remote — pull will fail,
        // but we verify the event logic is correct
        let (dir, reconciler, mut rx) = setup_reconciler();

        // Initialize a git repo so is_git_repo() returns true
        std::process::Command::new("git")
            .current_dir(dir.path())
            .args(["init"])
            .output()
            .unwrap();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let broadcaster = crate::broadcast::GitProgressBroadcaster::default();
        let progress = broadcaster.start_operation(crate::gitops::progress::GitOperationType::Pull);

        // Pull will fail because no remote is configured, which is fine
        let result = rt.block_on(reconciler.reconcile(&progress));
        assert!(result.is_err());

        // No event should have been sent since pull failed
        assert!(rx.try_recv().is_err());
    }
}
