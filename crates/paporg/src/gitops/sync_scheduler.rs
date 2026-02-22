//! Periodic git sync scheduler.
//!
//! Replaces the old `GitSyncManager` with a cleaner design that uses
//! the reconciler and supports manual trigger via broadcast channel.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use tokio::sync::broadcast;

use crate::broadcast::GitProgressBroadcaster;
use crate::gitops::progress::GitOperationType;
use crate::gitops::reconciler::GitReconciler;

/// Periodic git sync scheduler using the reconciler.
pub struct SyncScheduler {
    reconciler: Arc<GitReconciler>,
    interval: Duration,
    shutdown: Arc<AtomicBool>,
    git_broadcaster: Arc<GitProgressBroadcaster>,
}

impl SyncScheduler {
    /// Creates a new sync scheduler.
    pub fn new(
        reconciler: Arc<GitReconciler>,
        interval: Duration,
        git_broadcaster: Arc<GitProgressBroadcaster>,
    ) -> Self {
        Self {
            reconciler,
            interval,
            shutdown: Arc::new(AtomicBool::new(false)),
            git_broadcaster,
        }
    }

    /// Start the sync loop in a background thread.
    /// Accepts a trigger receiver for manual sync requests.
    pub fn start(&self, mut trigger_rx: broadcast::Receiver<()>) -> JoinHandle<()> {
        let reconciler = Arc::clone(&self.reconciler);
        let shutdown = Arc::clone(&self.shutdown);
        let interval = self.interval;
        let broadcaster = Arc::clone(&self.git_broadcaster);

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                let mut interval_timer = tokio::time::interval(interval);
                interval_timer.tick().await; // skip immediate first tick

                loop {
                    if shutdown.load(Ordering::Acquire) {
                        break;
                    }

                    tokio::select! {
                        _ = interval_timer.tick() => {},
                        Ok(()) = trigger_rx.recv() => {
                            log::info!("Manual git sync triggered");
                        },
                    }

                    if shutdown.load(Ordering::Acquire) {
                        break;
                    }

                    let progress = broadcaster.start_operation(GitOperationType::Pull);
                    match reconciler.reconcile(&progress).await {
                        Ok(result) if result.pull_result.files_changed > 0 => {
                            log::info!(
                                "Git sync: {} files changed",
                                result.pull_result.files_changed
                            );
                        }
                        Err(e) => log::error!("Git sync failed: {}", e),
                        _ => {}
                    }
                }
            });
        })
    }

    /// Signals the scheduler to stop.
    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gitops::git::GitRepository;
    use crate::gitops::resource::GitSettings;
    use crate::gitops::watcher::ConfigChangeEvent;
    use tempfile::TempDir;

    #[test]
    fn test_scheduler_shutdown() {
        let dir = TempDir::new().unwrap();
        let repo = GitRepository::new(dir.path(), GitSettings::default());
        let (change_tx, _change_rx) = broadcast::channel::<ConfigChangeEvent>(16);
        let reconciler = Arc::new(GitReconciler::new(repo, change_tx));
        let broadcaster = Arc::new(GitProgressBroadcaster::default());

        let scheduler = SyncScheduler::new(
            reconciler,
            Duration::from_millis(50),
            broadcaster,
        );

        let (trigger_tx, trigger_rx) = broadcast::channel(16);
        let handle = scheduler.start(trigger_rx);

        // Let it run briefly then stop
        std::thread::sleep(Duration::from_millis(100));
        scheduler.stop();

        // Send a trigger to wake up the select loop so it sees the shutdown
        let _ = trigger_tx.send(());

        // Should join within a reasonable time
        handle.join().expect("scheduler thread panicked");
    }
}
