use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use log::{debug, error, info, warn};
use notify::{Config as NotifyConfig, PollWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer_opt, Config as DebouncerConfig, DebouncedEventKind};
use walkdir::WalkDir;

use crate::config::DocumentFormat;
use crate::error::WorkerError;
use crate::worker::job::Job;

pub struct DirectoryScanner {
    input_directory: PathBuf,
}

impl DirectoryScanner {
    pub fn new<P: AsRef<Path>>(input_directory: P) -> Self {
        Self {
            input_directory: input_directory.as_ref().to_path_buf(),
        }
    }

    pub fn input_directory(&self) -> &Path {
        &self.input_directory
    }

    pub fn scan(&self) -> Result<Vec<Job>, WorkerError> {
        let mut jobs = Vec::new();

        for entry in WalkDir::new(&self.input_directory)
            .min_depth(1)
            .max_depth(1) // Only scan top level, not subdirectories or archive
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Skip directories
            if path.is_dir() {
                continue;
            }

            // Skip archive directory
            if path
                .parent()
                .map(|p| p.ends_with("archive"))
                .unwrap_or(false)
            {
                continue;
            }

            // Check if file format is supported
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if DocumentFormat::from_extension(ext).is_some() {
                    debug!("Found document: {}", path.display());
                    jobs.push(Job::new(path.to_path_buf()));
                }
            }
        }

        info!(
            "Scanned {} documents in {}",
            jobs.len(),
            self.input_directory.display()
        );
        Ok(jobs)
    }

    pub fn watch<F>(&self, callback: F, shutdown: Arc<AtomicBool>) -> Result<(), WorkerError>
    where
        F: Fn(PathBuf) + Send + 'static,
    {
        let input_dir = self.input_directory.clone();

        // Use PollWatcher for Docker/NFS compatibility
        let poll_config = NotifyConfig::default().with_poll_interval(Duration::from_secs(2));

        let debouncer_config = DebouncerConfig::default()
            .with_timeout(Duration::from_millis(500))
            .with_notify_config(poll_config);

        let (tx, rx) = std::sync::mpsc::channel();

        let mut debouncer = new_debouncer_opt::<_, PollWatcher>(debouncer_config, tx)
            .map_err(|e| WorkerError::WatchError(e.to_string()))?;

        debouncer
            .watcher()
            .watch(&input_dir, RecursiveMode::NonRecursive)
            .map_err(|e| WorkerError::WatchError(e.to_string()))?;

        info!("Watching directory: {}", input_dir.display());

        loop {
            if shutdown.load(Ordering::Relaxed) {
                info!("Watch mode shutting down...");
                break;
            }

            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(Ok(events)) => {
                    for event in events {
                        if matches!(event.kind, DebouncedEventKind::Any) {
                            let path = &event.path;

                            // Skip directories
                            if path.is_dir() {
                                continue;
                            }

                            // Skip archive directory
                            if path
                                .parent()
                                .map(|p| p.ends_with("archive"))
                                .unwrap_or(false)
                            {
                                continue;
                            }

                            // Check if file exists and is a supported format
                            if path.exists() {
                                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                                    if DocumentFormat::from_extension(ext).is_some() {
                                        info!("New document detected: {}", path.display());
                                        callback(path.to_path_buf());
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(Err(errors)) => {
                    warn!("Watch error: {:?}", errors);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    continue;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    error!("Watch channel disconnected");
                    break;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_scan_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let scanner = DirectoryScanner::new(temp_dir.path());

        let jobs = scanner.scan().unwrap();
        assert!(jobs.is_empty());
    }

    #[test]
    fn test_scan_with_documents() {
        let temp_dir = TempDir::new().unwrap();

        // Create some test files
        std::fs::write(temp_dir.path().join("doc1.pdf"), b"PDF content").unwrap();
        std::fs::write(temp_dir.path().join("doc2.txt"), b"Text content").unwrap();
        std::fs::write(temp_dir.path().join("image.png"), b"PNG content").unwrap();
        std::fs::write(temp_dir.path().join("unknown.xyz"), b"Unknown").unwrap();

        let scanner = DirectoryScanner::new(temp_dir.path());
        let jobs = scanner.scan().unwrap();

        // Should find 3 supported documents
        assert_eq!(jobs.len(), 3);
    }

    #[test]
    fn test_scan_ignores_archive_directory() {
        let temp_dir = TempDir::new().unwrap();

        // Create archive directory with files
        let archive_dir = temp_dir.path().join("archive");
        std::fs::create_dir(&archive_dir).unwrap();
        std::fs::write(archive_dir.join("archived.pdf"), b"Archived").unwrap();

        // Create a non-archived file
        std::fs::write(temp_dir.path().join("new.pdf"), b"New").unwrap();

        let scanner = DirectoryScanner::new(temp_dir.path());
        let jobs = scanner.scan().unwrap();

        // Should only find the non-archived file
        assert_eq!(jobs.len(), 1);
        assert!(jobs[0].source_path.ends_with("new.pdf"));
    }

    #[test]
    fn test_scan_ignores_subdirectories() {
        let temp_dir = TempDir::new().unwrap();

        // Create subdirectory with files
        let sub_dir = temp_dir.path().join("subdir");
        std::fs::create_dir(&sub_dir).unwrap();
        std::fs::write(sub_dir.join("nested.pdf"), b"Nested").unwrap();

        // Create a top-level file
        std::fs::write(temp_dir.path().join("top.pdf"), b"Top").unwrap();

        let scanner = DirectoryScanner::new(temp_dir.path());
        let jobs = scanner.scan().unwrap();

        // Should only find the top-level file
        assert_eq!(jobs.len(), 1);
        assert!(jobs[0].source_path.ends_with("top.pdf"));
    }
}
