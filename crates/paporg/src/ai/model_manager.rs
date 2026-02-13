//! Model download and cache management using Hugging Face Hub.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[cfg(feature = "ai")]
use hf_hub::api::sync::Api;
#[cfg(feature = "ai")]
use hf_hub::{Repo, RepoType};
#[allow(unused_imports)]
use log::{debug, info, warn};
use thiserror::Error;

/// Errors that can occur during model management.
#[derive(Debug, Error)]
pub enum ModelError {
    #[error("Failed to create cache directory: {0}")]
    CacheDirectoryCreation(#[from] std::io::Error),

    #[error("Failed to download model from Hugging Face: {0}")]
    HuggingFaceDownload(String),

    #[error("Model file not found: {0}")]
    ModelNotFound(String),

    #[error("Invalid model path: {0}")]
    InvalidPath(String),
}

/// Progress callback for model downloads.
pub type ProgressCallback = Arc<dyn Fn(u64, u64) + Send + Sync>;

/// Manages model downloads and caching.
pub struct ModelManager {
    cache_dir: PathBuf,
    model_repo: String,
    model_file: String,
    download_progress: Arc<AtomicU64>,
}

impl ModelManager {
    /// Creates a new model manager.
    pub fn new(cache_dir: impl AsRef<Path>, model_repo: &str, model_file: &str) -> Self {
        Self {
            cache_dir: cache_dir.as_ref().to_path_buf(),
            model_repo: model_repo.to_string(),
            model_file: model_file.to_string(),
            download_progress: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Creates a model manager from AI config.
    pub fn from_config(config: &crate::config::schema::AiConfig) -> Self {
        Self::new(
            &config.model_cache_dir,
            &config.model_repo,
            &config.model_file,
        )
    }

    /// Returns the path to the cached model file.
    pub fn model_path(&self) -> PathBuf {
        self.cache_dir.join(&self.model_file)
    }

    /// Checks if the model is already downloaded.
    pub fn is_model_available(&self) -> bool {
        self.model_path().exists()
    }

    /// Returns the model size in bytes if available.
    pub fn model_size(&self) -> Option<u64> {
        std::fs::metadata(self.model_path()).ok().map(|m| m.len())
    }

    /// Returns current download progress (0-100).
    pub fn download_progress(&self) -> u64 {
        self.download_progress.load(Ordering::Relaxed)
    }

    /// Ensures the model is available, downloading if necessary.
    #[cfg(feature = "ai")]
    pub fn ensure_model(&self) -> Result<PathBuf, ModelError> {
        let model_path = self.model_path();

        if model_path.exists() {
            debug!("Model already cached at: {}", model_path.display());
            return Ok(model_path);
        }

        // Create cache directory if needed
        std::fs::create_dir_all(&self.cache_dir)?;

        info!(
            "Downloading model {} from {}...",
            self.model_file, self.model_repo
        );

        // Download from Hugging Face
        let api = Api::new().map_err(|e| ModelError::HuggingFaceDownload(e.to_string()))?;
        let repo = api.repo(Repo::new(self.model_repo.clone(), RepoType::Model));

        let downloaded_path = repo
            .get(&self.model_file)
            .map_err(|e| ModelError::HuggingFaceDownload(e.to_string()))?;

        // The hf-hub crate caches files in its own location, so we need to copy or symlink
        // For simplicity, we'll use the path directly from hf-hub's cache
        // This avoids duplicating the ~900MB file
        info!("Model downloaded to: {}", downloaded_path.display());

        // Create a symlink in our cache directory pointing to hf-hub's cache
        #[cfg(unix)]
        {
            if let Err(e) = std::os::unix::fs::symlink(&downloaded_path, &model_path) {
                warn!("Failed to create symlink, copying instead: {}", e);
                std::fs::copy(&downloaded_path, &model_path)?;
            }
        }

        #[cfg(not(unix))]
        {
            std::fs::copy(&downloaded_path, &model_path)?;
        }

        self.download_progress.store(100, Ordering::Relaxed);
        Ok(model_path)
    }

    /// Ensures the model is available (stub when AI feature is disabled).
    #[cfg(not(feature = "ai"))]
    pub fn ensure_model(&self) -> Result<PathBuf, ModelError> {
        let model_path = self.model_path();

        if model_path.exists() {
            debug!("Model already cached at: {}", model_path.display());
            return Ok(model_path);
        }

        Err(ModelError::HuggingFaceDownload(
            "AI feature is not enabled. Rebuild with --features ai".to_string(),
        ))
    }

    /// Returns the model repository name.
    pub fn model_repo(&self) -> &str {
        &self.model_repo
    }

    /// Returns the model filename.
    pub fn model_file(&self) -> &str {
        &self.model_file
    }

    /// Returns the friendly model name.
    pub fn model_name(&self) -> String {
        // Extract name from repo, e.g., "Qwen/Qwen2.5-1.5B-Instruct-GGUF" -> "Qwen2.5-1.5B-Instruct"
        self.model_repo
            .split('/')
            .next_back()
            .unwrap_or(&self.model_repo)
            .replace("-GGUF", "")
    }

    /// Default expected model size in MB (Qwen2.5-1.5B-Instruct Q4_K_M).
    /// This is an approximation used for progress reporting during download.
    const DEFAULT_EXPECTED_MODEL_SIZE_MB: u64 = 900;

    /// Returns the expected model size in MB.
    /// This is an approximation used for progress reporting.
    /// TODO: Could be made configurable via AiConfig if needed for different models.
    pub fn expected_model_size_mb(&self) -> u64 {
        Self::DEFAULT_EXPECTED_MODEL_SIZE_MB
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_model_manager_creation() {
        let temp = TempDir::new().unwrap();
        let manager = ModelManager::new(
            temp.path(),
            "Qwen/Qwen2.5-1.5B-Instruct-GGUF",
            "qwen2.5-1.5b-instruct-q4_k_m.gguf",
        );

        assert_eq!(manager.model_repo(), "Qwen/Qwen2.5-1.5B-Instruct-GGUF");
        assert_eq!(manager.model_file(), "qwen2.5-1.5b-instruct-q4_k_m.gguf");
        assert!(!manager.is_model_available());
    }

    #[test]
    fn test_model_name_extraction() {
        let temp = TempDir::new().unwrap();
        let manager = ModelManager::new(
            temp.path(),
            "Qwen/Qwen2.5-1.5B-Instruct-GGUF",
            "qwen2.5-1.5b-instruct-q4_k_m.gguf",
        );

        assert_eq!(manager.model_name(), "Qwen2.5-1.5B-Instruct");
    }

    #[test]
    fn test_model_path() {
        let temp = TempDir::new().unwrap();
        let manager =
            ModelManager::new(temp.path(), "Qwen/Qwen2.5-1.5B-Instruct-GGUF", "test.gguf");

        let expected = temp.path().join("test.gguf");
        assert_eq!(manager.model_path(), expected);
    }
}
