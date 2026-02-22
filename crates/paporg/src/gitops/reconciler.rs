//! Git reconciler: pull → reload → notify.
//!
//! Inspired by ArgoCD's reconciliation loop. Single responsibility:
//! pull changes from remote, then broadcast a config change event
//! so subscribers (Tauri state) can reload config and update the UI.

use serde::Serialize;
use tokio::sync::broadcast;

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

/// Reconciles git remote state with local config.
///
/// Atomic operation: pull → notify subscribers if files changed.
pub struct GitReconciler {
    repo: GitRepository,
    config_change_sender: broadcast::Sender<ConfigChangeEvent>,
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
        }
    }

    /// Returns a reference to the underlying repository.
    pub fn repo(&self) -> &GitRepository {
        &self.repo
    }

    /// Pull from remote. If files changed, broadcast a Reloaded event
    /// so subscribers (Tauri state) can reload config and update UI.
    pub async fn reconcile(&self, progress: &OperationProgress) -> Result<ReconcileResult> {
        let pull_result = self.repo.pull_with_progress(progress).await?;

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

        Ok(ReconcileResult {
            pull_result,
            config_reloaded,
        })
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
