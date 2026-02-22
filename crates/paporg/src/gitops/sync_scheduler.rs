//! Periodic git sync scheduler.
//!
//! Replaces the old `GitSyncManager` with a cleaner design that uses
//! the reconciler and supports manual trigger via broadcast channel.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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
    /// Stored thread handle so we can join on stop.
    handle: Mutex<Option<JoinHandle<()>>>,
    /// Trigger sender used to wake the select loop on stop.
    trigger_tx: broadcast::Sender<()>,
}

impl SyncScheduler {
    /// Creates a new sync scheduler.
    pub fn new(
        reconciler: Arc<GitReconciler>,
        interval: Duration,
        git_broadcaster: Arc<GitProgressBroadcaster>,
        trigger_tx: broadcast::Sender<()>,
    ) -> Self {
        Self {
            reconciler,
            interval,
            shutdown: Arc::new(AtomicBool::new(false)),
            git_broadcaster,
            handle: Mutex::new(None),
            trigger_tx,
        }
    }

    /// Start the sync loop in a background thread.
    /// Accepts a trigger receiver for manual sync requests.
    pub fn start(&self, mut trigger_rx: broadcast::Receiver<()>) {
        let reconciler = Arc::clone(&self.reconciler);
        let shutdown = Arc::clone(&self.shutdown);
        let interval = self.interval;
        let broadcaster = Arc::clone(&self.git_broadcaster);

        let handle = std::thread::spawn(move || {
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
        });

        let mut guard = self.handle.lock().unwrap();
        *guard = Some(handle);
    }

    /// Signals the scheduler to stop and waits for the thread to finish.
    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::Release);
        // Send a wakeup signal so the select loop sees the shutdown flag
        let _ = self.trigger_tx.send(());

        let handle = {
            let mut guard = self.handle.lock().unwrap();
            guard.take()
        };
        if let Some(handle) = handle {
            // Join with a timeout by spinning briefly
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                if handle.is_finished() {
                    let _ = handle.join();
                    break;
                }
                if std::time::Instant::now() >= deadline {
                    log::warn!("Scheduler thread did not stop within timeout");
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
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

        let (trigger_tx, trigger_rx) = broadcast::channel(16);
        let scheduler = SyncScheduler::new(
            reconciler,
            Duration::from_millis(50),
            broadcaster,
            trigger_tx,
        );

        scheduler.start(trigger_rx);

        // Let it run briefly then stop (stop() now sends wakeup + joins)
        std::thread::sleep(Duration::from_millis(100));
        scheduler.stop();
    }
}
