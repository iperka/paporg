//! Multi-source file scanner that discovers files from ImportSource configurations.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use glob::Pattern;
use notify::{Config as NotifyConfig, PollWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer_opt, Config as DebouncerConfig, DebouncedEventKind};
use sea_orm::DatabaseConnection;
use tracing::{debug, error, info, info_span, warn};
use walkdir::WalkDir;

use crate::config::DocumentFormat;
use crate::email::EmailSourceScanner;
use crate::error::WorkerError;
use crate::gitops::loader::LoadedConfig;
use crate::gitops::resource::{EmailSourceConfig, ImportSourceType};
use crate::worker::job::Job;

/// An enabled local import source with resolved configuration.
#[derive(Debug)]
struct LocalEnabledSource {
    /// Name of the import source.
    name: String,
    /// Resolved path to the source directory.
    path: PathBuf,
    /// Whether to scan subdirectories recursively.
    recursive: bool,
    /// Glob patterns for files to include.
    include_patterns: Vec<Pattern>,
    /// Glob patterns for files to exclude.
    exclude_patterns: Vec<Pattern>,
    /// Poll interval for watching.
    poll_interval: Duration,
}

/// An enabled email import source.
#[derive(Debug)]
struct EmailEnabledSource {
    /// Name of the import source.
    name: String,
    /// Email source configuration.
    config: EmailSourceConfig,
}

/// Enum representing all enabled source types.
#[derive(Debug)]
enum EnabledSource {
    Local(LocalEnabledSource),
    Email(Box<EmailEnabledSource>),
}

/// A scanner that discovers files from multiple ImportSource configurations.
pub struct MultiSourceScanner {
    sources: Vec<EnabledSource>,
    /// Temporary directory for saving email attachments.
    temp_dir: PathBuf,
    /// Database connection for email tracking.
    db: Option<DatabaseConnection>,
}

impl MultiSourceScanner {
    /// Creates a new multi-source scanner from the loaded configuration.
    pub fn from_config(config: &LoadedConfig) -> Self {
        Self::from_config_with_options(config, None, None)
    }

    /// Creates a new multi-source scanner with optional database and temp directory.
    pub fn from_config_with_options(
        config: &LoadedConfig,
        db: Option<DatabaseConnection>,
        temp_dir: Option<PathBuf>,
    ) -> Self {
        let mut sources = Vec::new();
        let temp_dir =
            temp_dir.unwrap_or_else(|| std::env::temp_dir().join("paporg_email_attachments"));

        for source_with_path in &config.import_sources {
            let source = &source_with_path.resource;

            // Skip disabled sources
            if !source.spec.enabled {
                debug!("Skipping disabled import source: {}", source.metadata.name);
                continue;
            }

            match source.spec.source_type {
                ImportSourceType::Local => {
                    if let Some(local) = &source.spec.local {
                        // Expand ~ in path
                        let expanded_path = expand_tilde(&local.path);

                        // Parse include patterns
                        let include_patterns: Vec<Pattern> = local
                            .filters
                            .include
                            .iter()
                            .filter_map(|p| match Pattern::new(p) {
                                Ok(pattern) => Some(pattern),
                                Err(e) => {
                                    warn!(
                                        "Invalid include pattern '{}' in source '{}': {}",
                                        p, source.metadata.name, e
                                    );
                                    None
                                }
                            })
                            .collect();

                        // Parse exclude patterns
                        let exclude_patterns: Vec<Pattern> = local
                            .filters
                            .exclude
                            .iter()
                            .filter_map(|p| match Pattern::new(p) {
                                Ok(pattern) => Some(pattern),
                                Err(e) => {
                                    warn!(
                                        "Invalid exclude pattern '{}' in source '{}': {}",
                                        p, source.metadata.name, e
                                    );
                                    None
                                }
                            })
                            .collect();

                        sources.push(EnabledSource::Local(LocalEnabledSource {
                            name: source.metadata.name.clone(),
                            path: expanded_path,
                            recursive: local.recursive,
                            include_patterns,
                            exclude_patterns,
                            poll_interval: Duration::from_secs(local.poll_interval),
                        }));

                        info!(
                            "Registered local import source '{}' at {}{}",
                            source.metadata.name,
                            local.path,
                            if local.recursive { " (recursive)" } else { "" }
                        );
                    }
                }
                ImportSourceType::Email => {
                    if let Some(email) = &source.spec.email {
                        sources.push(EnabledSource::Email(Box::new(EmailEnabledSource {
                            name: source.metadata.name.clone(),
                            config: email.clone(),
                        })));

                        info!(
                            "Registered email import source '{}' ({}@{}:{})",
                            source.metadata.name, email.username, email.host, email.port
                        );
                    }
                }
            }
        }

        if sources.is_empty() {
            info!("No enabled import sources configured");
        } else {
            let local_count = sources
                .iter()
                .filter(|s| matches!(s, EnabledSource::Local(_)))
                .count();
            let email_count = sources
                .iter()
                .filter(|s| matches!(s, EnabledSource::Email(_)))
                .count();
            info!(
                "Configured {} import source(s) ({} local, {} email)",
                sources.len(),
                local_count,
                email_count
            );
        }

        Self {
            sources,
            temp_dir,
            db,
        }
    }

    /// Returns true if there are any enabled sources configured.
    pub fn has_sources(&self) -> bool {
        !self.sources.is_empty()
    }

    /// Returns the number of enabled sources.
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    /// Scans all configured sources and returns discovered jobs.
    /// Note: For email sources, use `scan_async()` instead.
    pub fn scan(&self) -> Result<Vec<Job>, WorkerError> {
        let _span = info_span!("scan_cycle", source_count = self.sources.len()).entered();
        let mut jobs = Vec::new();

        for source in &self.sources {
            match source {
                EnabledSource::Local(local_source) => match self.scan_local_source(local_source) {
                    Ok(source_jobs) => {
                        info!(
                            "Found {} documents in local source '{}'",
                            source_jobs.len(),
                            local_source.name
                        );
                        jobs.extend(source_jobs);
                    }
                    Err(e) => {
                        warn!("Failed to scan local source '{}': {}", local_source.name, e);
                    }
                },
                EnabledSource::Email(email_source) => {
                    // Email sources need async scanning, skip in sync scan
                    debug!(
                        "Skipping email source '{}' in sync scan - use scan_async()",
                        email_source.name
                    );
                }
            }
        }

        info!("Total: {} documents found across local sources", jobs.len());
        Ok(jobs)
    }

    /// Scans all configured sources asynchronously (required for email sources).
    pub async fn scan_async(&self) -> Result<Vec<Job>, WorkerError> {
        let _span = info_span!("scan_cycle", source_count = self.sources.len()).entered();
        let mut jobs = Vec::new();

        for source in &self.sources {
            match source {
                EnabledSource::Local(local_source) => match self.scan_local_source(local_source) {
                    Ok(source_jobs) => {
                        info!(
                            "Found {} documents in local source '{}'",
                            source_jobs.len(),
                            local_source.name
                        );
                        jobs.extend(source_jobs);
                    }
                    Err(e) => {
                        warn!("Failed to scan local source '{}': {}", local_source.name, e);
                    }
                },
                EnabledSource::Email(email_source) => {
                    match self.scan_email_source(email_source).await {
                        Ok(source_jobs) => {
                            info!(
                                "Found {} attachments in email source '{}'",
                                source_jobs.len(),
                                email_source.name
                            );
                            jobs.extend(source_jobs);
                        }
                        Err(e) => {
                            warn!("Failed to scan email source '{}': {}", email_source.name, e);
                        }
                    }
                }
            }
        }

        info!("Total: {} documents found across all sources", jobs.len());
        Ok(jobs)
    }

    /// Scans a local directory source and returns discovered jobs.
    fn scan_local_source(&self, source: &LocalEnabledSource) -> Result<Vec<Job>, WorkerError> {
        let _span = info_span!("scan_source", name = %source.name, kind = "local").entered();
        let mut jobs = Vec::new();

        debug!(
            "Scanning local source '{}' at path: {}",
            source.name,
            source.path.display()
        );

        if !source.path.exists() {
            warn!(
                "Import source '{}' path does not exist: {}",
                source.name,
                source.path.display()
            );
            return Ok(jobs);
        }

        let max_depth = if source.recursive { usize::MAX } else { 1 };

        debug!(
            "Source '{}' has {} include patterns, {} exclude patterns",
            source.name,
            source.include_patterns.len(),
            source.exclude_patterns.len()
        );

        let mut scanned_count = 0;
        let mut filtered_count = 0;
        let mut unsupported_count = 0;

        for entry in WalkDir::new(&source.path)
            .min_depth(1)
            .max_depth(max_depth)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Skip directories
            if path.is_dir() {
                continue;
            }

            scanned_count += 1;

            // Skip archive directories
            if Self::is_in_archive_dir(path) {
                debug!("Skipping archived file: {}", path.display());
                continue;
            }

            // Check if file matches filters
            if !self.matches_local_filters(path, source) {
                filtered_count += 1;
                debug!("File filtered out: {}", path.display());
                continue;
            }

            // Check if file format is supported
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if DocumentFormat::from_extension(ext).is_some() {
                    debug!("Found document in '{}': {}", source.name, path.display());
                    jobs.push(Job::new_with_source(
                        path.to_path_buf(),
                        source.name.clone(),
                    ));
                } else {
                    unsupported_count += 1;
                    debug!("Unsupported format: {} (ext: {})", path.display(), ext);
                }
            } else {
                unsupported_count += 1;
                debug!("No extension: {}", path.display());
            }
        }

        info!(
            "Source '{}' scan complete: {} files scanned, {} filtered, {} unsupported, {} jobs created",
            source.name, scanned_count, filtered_count, unsupported_count, jobs.len()
        );

        Ok(jobs)
    }

    /// Scans an email source for attachments.
    async fn scan_email_source(
        &self,
        source: &EmailEnabledSource,
    ) -> Result<Vec<Job>, WorkerError> {
        let mut scanner = EmailSourceScanner::new(
            source.name.clone(),
            source.config.clone(),
            self.temp_dir.clone(),
        );

        if let Some(db) = &self.db {
            scanner = scanner.with_database(db.clone());
        }

        scanner
            .scan()
            .await
            .map_err(|e| WorkerError::ScanError(e.to_string()))
    }

    /// Checks if a path is inside an archive directory.
    fn is_in_archive_dir(path: &Path) -> bool {
        path.ancestors()
            .any(|p| p.file_name().map(|name| name == "archive").unwrap_or(false))
    }

    /// Checks if a file matches the include/exclude filters for a local source.
    fn matches_local_filters(&self, path: &Path, source: &LocalEnabledSource) -> bool {
        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => {
                debug!("Could not get filename for: {}", path.display());
                return false;
            }
        };

        // Check exclude patterns first (if any match, reject)
        for pattern in &source.exclude_patterns {
            if pattern.matches(filename) {
                debug!("File '{}' excluded by pattern '{}'", filename, pattern);
                return false;
            }
        }

        // Check include patterns (if any match, accept)
        // If no include patterns, default to accepting all
        if source.include_patterns.is_empty() {
            debug!("No include patterns, accepting: {}", filename);
            return true;
        }

        for pattern in &source.include_patterns {
            if pattern.matches(filename) {
                debug!("File '{}' matched include pattern '{}'", filename, pattern);
                return true;
            }
        }

        debug!(
            "File '{}' did not match any include pattern (patterns: {:?})",
            filename,
            source
                .include_patterns
                .iter()
                .map(|p| p.as_str())
                .collect::<Vec<_>>()
        );
        false
    }

    /// Returns true if there are any email sources configured.
    pub fn has_email_sources(&self) -> bool {
        self.sources
            .iter()
            .any(|s| matches!(s, EnabledSource::Email(_)))
    }

    /// Returns true if there are any local sources configured.
    pub fn has_local_sources(&self) -> bool {
        self.sources
            .iter()
            .any(|s| matches!(s, EnabledSource::Local(_)))
    }

    /// Watches all configured local sources for new files.
    /// Note: Email sources use polling-based scanning via `scan_async()`.
    pub fn watch<F>(&self, callback: F, shutdown: Arc<AtomicBool>) -> Result<(), WorkerError>
    where
        F: Fn(PathBuf, String) + Send + Sync + Clone + 'static,
    {
        // Filter to only local sources
        let local_sources: Vec<&LocalEnabledSource> = self
            .sources
            .iter()
            .filter_map(|s| match s {
                EnabledSource::Local(local) => Some(local),
                EnabledSource::Email(_) => None,
            })
            .collect();

        if local_sources.is_empty() {
            info!("No local import sources to watch");
            // Just wait for shutdown
            while !shutdown.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(100));
            }
            return Ok(());
        }

        // Use the smallest poll interval among local sources
        let min_poll_interval = local_sources
            .iter()
            .map(|s| s.poll_interval)
            .min()
            .unwrap_or(Duration::from_secs(2));

        let poll_config = NotifyConfig::default().with_poll_interval(min_poll_interval);

        let debouncer_config = DebouncerConfig::default()
            .with_timeout(Duration::from_millis(500))
            .with_notify_config(poll_config);

        let (tx, rx) = std::sync::mpsc::channel();

        let mut debouncer = new_debouncer_opt::<_, PollWatcher>(debouncer_config, tx)
            .map_err(|e| WorkerError::WatchError(e.to_string()))?;

        // Create a mapping from watched paths to source names
        let mut path_to_source: std::collections::HashMap<
            PathBuf,
            (String, bool, Vec<Pattern>, Vec<Pattern>),
        > = std::collections::HashMap::new();

        // Add watches for local sources only
        for source in &local_sources {
            if !source.path.exists() {
                warn!(
                    "Skipping watch for source '{}': path does not exist",
                    source.name
                );
                continue;
            }

            let mode = if source.recursive {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            };

            match debouncer.watcher().watch(&source.path, mode) {
                Ok(()) => {
                    info!(
                        "Watching import source '{}' at {}",
                        source.name,
                        source.path.display()
                    );
                    path_to_source.insert(
                        source.path.clone(),
                        (
                            source.name.clone(),
                            source.recursive,
                            source.include_patterns.clone(),
                            source.exclude_patterns.clone(),
                        ),
                    );
                }
                Err(e) => {
                    warn!(
                        "Failed to watch source '{}' at {}: {}",
                        source.name,
                        source.path.display(),
                        e
                    );
                }
            }
        }

        if path_to_source.is_empty() {
            warn!("No local import sources could be watched");
            while !shutdown.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(100));
            }
            return Ok(());
        }

        info!("Watching {} import source(s)", path_to_source.len());

        loop {
            if shutdown.load(Ordering::Relaxed) {
                info!("Multi-source watch mode shutting down...");
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

                            // Skip archive directories
                            if Self::is_in_archive_dir(path) {
                                continue;
                            }

                            // Find which source this file belongs to
                            let source_info = path_to_source
                                .iter()
                                .find(|(source_path, _)| path.starts_with(source_path));

                            if let Some((
                                source_path,
                                (source_name, recursive, include_patterns, exclude_patterns),
                            )) = source_info
                            {
                                // Check if file exists
                                if !path.exists() {
                                    continue;
                                }

                                // For non-recursive sources, check depth
                                if !recursive {
                                    if let Ok(relative) = path.strip_prefix(source_path) {
                                        if relative.components().count() > 1 {
                                            continue;
                                        }
                                    }
                                }

                                // Check filters
                                let filename = match path.file_name().and_then(|n| n.to_str()) {
                                    Some(name) => name,
                                    None => continue,
                                };

                                // Check exclude patterns
                                let excluded = exclude_patterns.iter().any(|p| p.matches(filename));
                                if excluded {
                                    continue;
                                }

                                // Check include patterns
                                let included = if include_patterns.is_empty() {
                                    true
                                } else {
                                    include_patterns.iter().any(|p| p.matches(filename))
                                };

                                if !included {
                                    continue;
                                }

                                // Check if file format is supported
                                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                                    if DocumentFormat::from_extension(ext).is_some() {
                                        info!(
                                            "New document detected in '{}': {}",
                                            source_name,
                                            path.display()
                                        );
                                        callback(path.to_path_buf(), source_name.clone());
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(Err(errors)) => {
                    warn!("Watch errors: {:?}", errors);
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

/// Expands ~ to the home directory in a path string.
fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    } else if path == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home);
        }
    }
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_expand_tilde() {
        // Test with explicit path
        assert_eq!(
            expand_tilde("/absolute/path"),
            PathBuf::from("/absolute/path")
        );

        // Test tilde expansion (result depends on HOME env var)
        let expanded = expand_tilde("~/Documents");
        if let Some(home) = std::env::var_os("HOME") {
            assert_eq!(expanded, PathBuf::from(home).join("Documents"));
        }
    }

    #[test]
    fn test_is_in_archive_dir() {
        assert!(MultiSourceScanner::is_in_archive_dir(Path::new(
            "/data/archive/file.pdf"
        )));
        assert!(MultiSourceScanner::is_in_archive_dir(Path::new(
            "/data/inbox/archive/file.pdf"
        )));
        assert!(!MultiSourceScanner::is_in_archive_dir(Path::new(
            "/data/inbox/file.pdf"
        )));
        assert!(!MultiSourceScanner::is_in_archive_dir(Path::new(
            "/data/archived/file.pdf"
        )));
    }

    #[test]
    fn test_empty_scanner() {
        use crate::gitops::loader::LoadedConfig;
        use crate::gitops::resource::ResourceWithPath;
        use crate::gitops::resource::{
            AiSettings, DefaultOutputSettings, GitSettings, ObjectMeta, OcrSettings, ResourceKind,
            SettingsResource, SettingsSpec, API_VERSION,
        };

        let settings = SettingsResource {
            api_version: API_VERSION.to_string(),
            kind: ResourceKind::Settings,
            metadata: ObjectMeta::new("default"),
            spec: SettingsSpec {
                input_directory: "/data/inbox".to_string(),
                output_directory: "/data/output".to_string(),
                worker_count: 4,
                ocr: OcrSettings::default(),
                defaults: DefaultOutputSettings::default(),
                git: GitSettings::default(),
                ai: AiSettings::default(),
            },
        };

        let config = LoadedConfig {
            settings: ResourceWithPath::new(settings, "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![],
        };

        let scanner = MultiSourceScanner::from_config(&config);
        assert!(!scanner.has_sources());
        assert_eq!(scanner.source_count(), 0);

        let jobs = scanner.scan().unwrap();
        assert!(jobs.is_empty());
    }

    #[test]
    fn test_scan_with_source() {
        use crate::gitops::loader::LoadedConfig;
        use crate::gitops::resource::ResourceWithPath;
        use crate::gitops::resource::{
            AiSettings, DefaultOutputSettings, FileFilters, GitSettings, ImportSourceResource,
            ImportSourceSpec, ImportSourceType, LocalSourceConfig, ObjectMeta, OcrSettings,
            ResourceKind, SettingsResource, SettingsSpec, API_VERSION,
        };

        let temp_dir = TempDir::new().unwrap();

        // Create test files
        std::fs::write(temp_dir.path().join("doc1.pdf"), b"PDF content").unwrap();
        std::fs::write(temp_dir.path().join("doc2.txt"), b"Text content").unwrap();
        std::fs::write(temp_dir.path().join("unknown.xyz"), b"Unknown").unwrap();

        let settings = SettingsResource {
            api_version: API_VERSION.to_string(),
            kind: ResourceKind::Settings,
            metadata: ObjectMeta::new("default"),
            spec: SettingsSpec {
                input_directory: "/data/inbox".to_string(),
                output_directory: "/data/output".to_string(),
                worker_count: 4,
                ocr: OcrSettings::default(),
                defaults: DefaultOutputSettings::default(),
                git: GitSettings::default(),
                ai: AiSettings::default(),
            },
        };

        let import_source = ImportSourceResource {
            api_version: API_VERSION.to_string(),
            kind: ResourceKind::ImportSource,
            metadata: ObjectMeta::new("test-source"),
            spec: ImportSourceSpec {
                source_type: ImportSourceType::Local,
                enabled: true,
                local: Some(LocalSourceConfig {
                    path: temp_dir.path().to_string_lossy().to_string(),
                    recursive: false,
                    filters: FileFilters::default(),
                    poll_interval: 60,
                }),
                email: None,
            },
        };

        let config = LoadedConfig {
            settings: ResourceWithPath::new(settings, "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(import_source, "sources/test.yaml")],
        };

        let scanner = MultiSourceScanner::from_config(&config);
        assert!(scanner.has_sources());
        assert_eq!(scanner.source_count(), 1);

        let jobs = scanner.scan().unwrap();
        // Should find pdf and txt (both supported formats)
        assert_eq!(jobs.len(), 2);

        // All jobs should have the source name
        for job in &jobs {
            assert_eq!(job.source_name, Some("test-source".to_string()));
        }
    }

    #[test]
    fn test_scan_with_filters() {
        use crate::gitops::loader::LoadedConfig;
        use crate::gitops::resource::ResourceWithPath;
        use crate::gitops::resource::{
            AiSettings, DefaultOutputSettings, FileFilters, GitSettings, ImportSourceResource,
            ImportSourceSpec, ImportSourceType, LocalSourceConfig, ObjectMeta, OcrSettings,
            ResourceKind, SettingsResource, SettingsSpec, API_VERSION,
        };

        let temp_dir = TempDir::new().unwrap();

        // Create test files
        std::fs::write(temp_dir.path().join("doc1.pdf"), b"PDF content").unwrap();
        std::fs::write(temp_dir.path().join("doc2.pdf"), b"PDF content 2").unwrap();
        std::fs::write(temp_dir.path().join("doc3.txt"), b"Text content").unwrap();

        let settings = SettingsResource {
            api_version: API_VERSION.to_string(),
            kind: ResourceKind::Settings,
            metadata: ObjectMeta::new("default"),
            spec: SettingsSpec {
                input_directory: "/data/inbox".to_string(),
                output_directory: "/data/output".to_string(),
                worker_count: 4,
                ocr: OcrSettings::default(),
                defaults: DefaultOutputSettings::default(),
                git: GitSettings::default(),
                ai: AiSettings::default(),
            },
        };

        let import_source = ImportSourceResource {
            api_version: API_VERSION.to_string(),
            kind: ResourceKind::ImportSource,
            metadata: ObjectMeta::new("filtered-source"),
            spec: ImportSourceSpec {
                source_type: ImportSourceType::Local,
                enabled: true,
                local: Some(LocalSourceConfig {
                    path: temp_dir.path().to_string_lossy().to_string(),
                    recursive: false,
                    filters: FileFilters {
                        include: vec!["*.pdf".to_string()],
                        exclude: vec![],
                    },
                    poll_interval: 60,
                }),
                email: None,
            },
        };

        let config = LoadedConfig {
            settings: ResourceWithPath::new(settings, "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(
                import_source,
                "sources/filtered.yaml",
            )],
        };

        let scanner = MultiSourceScanner::from_config(&config);
        let jobs = scanner.scan().unwrap();

        // Should only find PDF files
        assert_eq!(jobs.len(), 2);
        for job in &jobs {
            assert!(job.source_path.extension().unwrap() == "pdf");
        }
    }

    #[test]
    fn test_scan_recursive() {
        use crate::gitops::loader::LoadedConfig;
        use crate::gitops::resource::ResourceWithPath;
        use crate::gitops::resource::{
            AiSettings, DefaultOutputSettings, FileFilters, GitSettings, ImportSourceResource,
            ImportSourceSpec, ImportSourceType, LocalSourceConfig, ObjectMeta, OcrSettings,
            ResourceKind, SettingsResource, SettingsSpec, API_VERSION,
        };

        let temp_dir = TempDir::new().unwrap();

        // Create test files at different depths
        std::fs::write(temp_dir.path().join("top.pdf"), b"PDF").unwrap();
        std::fs::create_dir(temp_dir.path().join("subdir")).unwrap();
        std::fs::write(temp_dir.path().join("subdir/nested.pdf"), b"PDF").unwrap();

        let settings = SettingsResource {
            api_version: API_VERSION.to_string(),
            kind: ResourceKind::Settings,
            metadata: ObjectMeta::new("default"),
            spec: SettingsSpec {
                input_directory: "/data/inbox".to_string(),
                output_directory: "/data/output".to_string(),
                worker_count: 4,
                ocr: OcrSettings::default(),
                defaults: DefaultOutputSettings::default(),
                git: GitSettings::default(),
                ai: AiSettings::default(),
            },
        };

        // Test non-recursive
        let import_source_non_recursive = ImportSourceResource {
            api_version: API_VERSION.to_string(),
            kind: ResourceKind::ImportSource,
            metadata: ObjectMeta::new("non-recursive"),
            spec: ImportSourceSpec {
                source_type: ImportSourceType::Local,
                enabled: true,
                local: Some(LocalSourceConfig {
                    path: temp_dir.path().to_string_lossy().to_string(),
                    recursive: false,
                    filters: FileFilters::default(),
                    poll_interval: 60,
                }),
                email: None,
            },
        };

        let config = LoadedConfig {
            settings: ResourceWithPath::new(settings.clone(), "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(
                import_source_non_recursive,
                "sources/nr.yaml",
            )],
        };

        let scanner = MultiSourceScanner::from_config(&config);
        let jobs = scanner.scan().unwrap();
        assert_eq!(jobs.len(), 1); // Only top.pdf

        // Test recursive
        let import_source_recursive = ImportSourceResource {
            api_version: API_VERSION.to_string(),
            kind: ResourceKind::ImportSource,
            metadata: ObjectMeta::new("recursive"),
            spec: ImportSourceSpec {
                source_type: ImportSourceType::Local,
                enabled: true,
                local: Some(LocalSourceConfig {
                    path: temp_dir.path().to_string_lossy().to_string(),
                    recursive: true,
                    filters: FileFilters::default(),
                    poll_interval: 60,
                }),
                email: None,
            },
        };

        let config = LoadedConfig {
            settings: ResourceWithPath::new(settings, "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(
                import_source_recursive,
                "sources/r.yaml",
            )],
        };

        let scanner = MultiSourceScanner::from_config(&config);
        let jobs = scanner.scan().unwrap();
        assert_eq!(jobs.len(), 2); // top.pdf and subdir/nested.pdf
    }

    #[test]
    fn test_disabled_source_ignored() {
        use crate::gitops::loader::LoadedConfig;
        use crate::gitops::resource::ResourceWithPath;
        use crate::gitops::resource::{
            AiSettings, DefaultOutputSettings, FileFilters, GitSettings, ImportSourceResource,
            ImportSourceSpec, ImportSourceType, LocalSourceConfig, ObjectMeta, OcrSettings,
            ResourceKind, SettingsResource, SettingsSpec, API_VERSION,
        };

        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("doc.pdf"), b"PDF").unwrap();

        let settings = SettingsResource {
            api_version: API_VERSION.to_string(),
            kind: ResourceKind::Settings,
            metadata: ObjectMeta::new("default"),
            spec: SettingsSpec {
                input_directory: "/data/inbox".to_string(),
                output_directory: "/data/output".to_string(),
                worker_count: 4,
                ocr: OcrSettings::default(),
                defaults: DefaultOutputSettings::default(),
                git: GitSettings::default(),
                ai: AiSettings::default(),
            },
        };

        let import_source = ImportSourceResource {
            api_version: API_VERSION.to_string(),
            kind: ResourceKind::ImportSource,
            metadata: ObjectMeta::new("disabled-source"),
            spec: ImportSourceSpec {
                source_type: ImportSourceType::Local,
                enabled: false, // Disabled!
                local: Some(LocalSourceConfig {
                    path: temp_dir.path().to_string_lossy().to_string(),
                    recursive: false,
                    filters: FileFilters::default(),
                    poll_interval: 60,
                }),
                email: None,
            },
        };

        let config = LoadedConfig {
            settings: ResourceWithPath::new(settings, "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(
                import_source,
                "sources/disabled.yaml",
            )],
        };

        let scanner = MultiSourceScanner::from_config(&config);
        assert!(!scanner.has_sources());
        assert_eq!(scanner.source_count(), 0);
    }
}
