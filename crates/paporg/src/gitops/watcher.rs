//! File system watcher for config directory changes.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebouncedEvent, Debouncer};
use tokio::sync::broadcast;

use super::error::{GitOpsError, Result};
use super::loader::ConfigLoader;
use super::resource::ResourceKind;

/// Event emitted when a configuration file changes.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigChangeEvent {
    /// The type of change.
    pub change_type: ChangeType,
    /// The file path relative to the config directory.
    pub path: String,
    /// The resource kind, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_kind: Option<ResourceKind>,
    /// The resource name, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_name: Option<String>,
}

/// The type of configuration change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeType {
    /// A file was created.
    Created,
    /// A file was modified.
    Modified,
    /// A file was deleted.
    Deleted,
    /// A file was renamed/moved.
    Renamed,
    /// The config was reloaded (e.g., after git pull).
    Reloaded,
}

/// Configuration file watcher.
pub struct ConfigWatcher {
    /// Root directory for configuration files.
    config_dir: PathBuf,
    /// Channel for broadcasting change events.
    sender: broadcast::Sender<ConfigChangeEvent>,
    /// Shutdown flag.
    shutdown: Arc<AtomicBool>,
}

impl ConfigWatcher {
    /// Creates a new config watcher.
    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        let (sender, _) = broadcast::channel(100);
        Self {
            config_dir: config_dir.into(),
            sender,
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Returns a receiver for config change events.
    pub fn subscribe(&self) -> broadcast::Receiver<ConfigChangeEvent> {
        self.sender.subscribe()
    }

    /// Returns the sender for broadcasting events.
    pub fn sender(&self) -> broadcast::Sender<ConfigChangeEvent> {
        self.sender.clone()
    }

    /// Returns the config directory path.
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Broadcasts a config change event.
    pub fn broadcast(&self, event: ConfigChangeEvent) {
        let _ = self.sender.send(event);
    }

    /// Broadcasts a reload event.
    pub fn broadcast_reload(&self) {
        self.broadcast(ConfigChangeEvent {
            change_type: ChangeType::Reloaded,
            path: "".to_string(),
            resource_kind: None,
            resource_name: None,
        });
    }

    /// Starts watching the config directory.
    ///
    /// This function blocks until the shutdown flag is set.
    pub fn watch(&self) -> Result<()> {
        let config_dir = self.config_dir.clone();
        let sender = self.sender.clone();
        let shutdown = Arc::clone(&self.shutdown);

        // Create a channel for debounced events
        let (tx, rx) = std::sync::mpsc::channel();

        // Create debouncer with 500ms delay
        let mut debouncer: Debouncer<RecommendedWatcher> =
            new_debouncer(Duration::from_millis(500), tx)
                .map_err(|e| GitOpsError::WatchError(e.to_string()))?;

        // Start watching
        debouncer
            .watcher()
            .watch(&config_dir, RecursiveMode::Recursive)
            .map_err(|e| GitOpsError::WatchError(e.to_string()))?;

        log::info!(
            "Started watching config directory: {}",
            config_dir.display()
        );

        // Process events
        loop {
            if shutdown.load(Ordering::Relaxed) {
                break;
            }

            // Use timeout to allow checking shutdown flag
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(Ok(events)) => {
                    for event in events {
                        if let Some(change_event) = self.process_event(&config_dir, event) {
                            let _ = sender.send(change_event);
                        }
                    }
                }
                Ok(Err(e)) => {
                    log::error!("Watch error: {}", e);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Continue loop
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    break;
                }
            }
        }

        log::info!("Stopped watching config directory");
        Ok(())
    }

    /// Processes a raw file system event into a config change event.
    fn process_event(&self, config_dir: &Path, event: DebouncedEvent) -> Option<ConfigChangeEvent> {
        let path = &event.path;

        // Only process YAML files
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "yaml" && ext != "yml" {
            // Could be a directory change
            if !path.is_dir() {
                return None;
            }
        }

        // Get relative path
        let relative_path = path.strip_prefix(config_dir).ok()?;
        let relative_str = relative_path.to_string_lossy().to_string();

        // Determine change type
        let change_type = if path.exists() {
            ChangeType::Modified // Could be Created or Modified, debouncer combines them
        } else {
            ChangeType::Deleted
        };

        // Try to determine resource info
        let (resource_kind, resource_name) = if path.is_file() && (ext == "yaml" || ext == "yml") {
            let loader = ConfigLoader::new(config_dir);
            match loader.load_file(path) {
                Ok(resource) => (Some(resource.kind()), Some(resource.name().to_string())),
                Err(_) => (None, None),
            }
        } else {
            (None, None)
        };

        Some(ConfigChangeEvent {
            change_type,
            path: relative_str,
            resource_kind,
            resource_name,
        })
    }

    /// Signals the watcher to stop.
    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }

    /// Returns whether the watcher has been signaled to stop.
    pub fn is_stopped(&self) -> bool {
        self.shutdown.load(Ordering::Relaxed)
    }
}

/// Async wrapper for the config watcher.
pub struct AsyncConfigWatcher {
    watcher: Arc<ConfigWatcher>,
    watch_handle: Option<std::thread::JoinHandle<Result<()>>>,
}

impl AsyncConfigWatcher {
    /// Creates a new async config watcher.
    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        Self {
            watcher: Arc::new(ConfigWatcher::new(config_dir)),
            watch_handle: None,
        }
    }

    /// Starts watching in a background thread.
    pub fn start(&mut self) {
        if self.watch_handle.is_some() {
            return;
        }

        let watcher = Arc::clone(&self.watcher);
        self.watch_handle = Some(std::thread::spawn(move || watcher.watch()));
    }

    /// Returns a receiver for config change events.
    pub fn subscribe(&self) -> broadcast::Receiver<ConfigChangeEvent> {
        self.watcher.subscribe()
    }

    /// Returns the sender for broadcasting events.
    pub fn sender(&self) -> broadcast::Sender<ConfigChangeEvent> {
        self.watcher.sender()
    }

    /// Broadcasts an event.
    pub fn broadcast(&self, event: ConfigChangeEvent) {
        self.watcher.broadcast(event);
    }

    /// Broadcasts a reload event.
    pub fn broadcast_reload(&self) {
        self.watcher.broadcast_reload();
    }

    /// Stops the watcher.
    pub fn stop(&mut self) {
        self.watcher.stop();
        if let Some(handle) = self.watch_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for AsyncConfigWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_config_change_event_serialization() {
        let event = ConfigChangeEvent {
            change_type: ChangeType::Created,
            path: "rules/test.yaml".to_string(),
            resource_kind: Some(ResourceKind::Rule),
            resource_name: Some("test-rule".to_string()),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("created"));
        assert!(json.contains("rules/test.yaml"));
        assert!(json.contains("Rule"));
    }

    #[test]
    fn test_change_type_serialization() {
        assert_eq!(
            serde_json::to_string(&ChangeType::Created).unwrap(),
            "\"created\""
        );
        assert_eq!(
            serde_json::to_string(&ChangeType::Modified).unwrap(),
            "\"modified\""
        );
        assert_eq!(
            serde_json::to_string(&ChangeType::Deleted).unwrap(),
            "\"deleted\""
        );
        assert_eq!(
            serde_json::to_string(&ChangeType::Renamed).unwrap(),
            "\"renamed\""
        );
        assert_eq!(
            serde_json::to_string(&ChangeType::Reloaded).unwrap(),
            "\"reloaded\""
        );
    }

    #[test]
    fn test_watcher_subscribe() {
        let dir = TempDir::new().unwrap();
        let watcher = ConfigWatcher::new(dir.path());

        let mut rx = watcher.subscribe();

        // Broadcast an event
        watcher.broadcast(ConfigChangeEvent {
            change_type: ChangeType::Modified,
            path: "test.yaml".to_string(),
            resource_kind: None,
            resource_name: None,
        });

        // Should receive it
        let event = rx.try_recv().unwrap();
        assert_eq!(event.change_type, ChangeType::Modified);
        assert_eq!(event.path, "test.yaml");
    }

    #[test]
    fn test_watcher_broadcast_reload() {
        let dir = TempDir::new().unwrap();
        let watcher = ConfigWatcher::new(dir.path());

        let mut rx = watcher.subscribe();
        watcher.broadcast_reload();

        let event = rx.try_recv().unwrap();
        assert_eq!(event.change_type, ChangeType::Reloaded);
        assert_eq!(event.path, "");
    }

    #[test]
    fn test_watcher_stop() {
        let dir = TempDir::new().unwrap();
        let watcher = ConfigWatcher::new(dir.path());

        assert!(!watcher.is_stopped());
        watcher.stop();
        assert!(watcher.is_stopped());
    }

    #[test]
    fn test_process_event_yaml_file() {
        let dir = TempDir::new().unwrap();
        let watcher = ConfigWatcher::new(dir.path());

        // Create a test YAML file
        let yaml = r#"
apiVersion: paporg.io/v1
kind: Variable
metadata:
  name: test-var
spec:
  pattern: "test"
"#;
        let file_path = dir.path().join("variables/test.yaml");
        fs::create_dir_all(dir.path().join("variables")).unwrap();
        fs::write(&file_path, yaml).unwrap();

        let event = DebouncedEvent {
            path: file_path.clone(),
            kind: notify_debouncer_mini::DebouncedEventKind::Any,
        };

        let change_event = watcher.process_event(dir.path(), event);
        assert!(change_event.is_some());

        let change_event = change_event.unwrap();
        assert_eq!(change_event.change_type, ChangeType::Modified);
        assert_eq!(change_event.path, "variables/test.yaml");
        assert_eq!(change_event.resource_kind, Some(ResourceKind::Variable));
        assert_eq!(change_event.resource_name, Some("test-var".to_string()));
    }

    #[test]
    fn test_process_event_non_yaml_file() {
        let dir = TempDir::new().unwrap();
        let watcher = ConfigWatcher::new(dir.path());

        // Create a non-YAML file
        let file_path = dir.path().join("readme.txt");
        fs::write(&file_path, "test").unwrap();

        let event = DebouncedEvent {
            path: file_path.clone(),
            kind: notify_debouncer_mini::DebouncedEventKind::Any,
        };

        let change_event = watcher.process_event(dir.path(), event);
        assert!(change_event.is_none());
    }

    #[test]
    fn test_async_watcher_lifecycle() {
        let dir = TempDir::new().unwrap();
        let mut watcher = AsyncConfigWatcher::new(dir.path());

        // Should be able to subscribe before starting
        let mut rx = watcher.subscribe();

        // Start watching (in background thread)
        watcher.start();

        // Broadcast an event
        watcher.broadcast(ConfigChangeEvent {
            change_type: ChangeType::Created,
            path: "test.yaml".to_string(),
            resource_kind: None,
            resource_name: None,
        });

        // Should receive the event
        std::thread::sleep(Duration::from_millis(50));
        let event = rx.try_recv().unwrap();
        assert_eq!(event.change_type, ChangeType::Created);

        // Stop
        watcher.stop();
    }
}
