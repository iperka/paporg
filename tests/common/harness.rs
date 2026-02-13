//! Test harness for isolated test execution.
//!
//! The `TestHarness` struct provides a complete isolated environment for testing
//! the document processing pipeline, including:
//! - Temporary directories for input/output/config
//! - ProcessorRegistry, Categorizer, VariableEngine, FileStorage setup
//! - Full pipeline execution with result capture

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use paporg::categorizer::{CategorizationResult, Categorizer};
use paporg::config::schema::{Config, DefaultsConfig, OutputConfig, VariablesConfig};
use paporg::processor::{ProcessedContent, ProcessorRegistry};
use paporg::storage::FileStorage;
use paporg::VariableEngine;

/// Result of running a document through the test harness pipeline.
pub struct PipelineResult {
    /// The processed document with extracted text and PDF bytes.
    pub processed: ProcessedContent,
    /// The categorization result with matched rule and output config.
    pub categorization: CategorizationResult,
    /// Variables extracted from the document text.
    pub extracted_variables: HashMap<String, String>,
    /// The substituted output directory path.
    pub output_directory: String,
    /// The substituted output filename.
    pub output_filename: String,
    /// Path where the document was stored (if storage was performed).
    pub stored_path: Option<PathBuf>,
}

impl std::fmt::Debug for PipelineResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PipelineResult")
            .field("processed.text_len", &self.processed.text.len())
            .field("categorization", &self.categorization)
            .field("extracted_variables", &self.extracted_variables)
            .field("output_directory", &self.output_directory)
            .field("output_filename", &self.output_filename)
            .field("stored_path", &self.stored_path)
            .finish()
    }
}

/// Test harness providing isolated execution environment for integration tests.
pub struct TestHarness {
    /// Temporary directory containing input/output/config subdirectories.
    temp_dir: TempDir,
    /// Path to the input directory within temp_dir.
    pub input_dir: PathBuf,
    /// Path to the output directory within temp_dir.
    pub output_dir: PathBuf,
    /// Path to the config directory within temp_dir.
    pub config_dir: PathBuf,
    /// Whether OCR is enabled for processing.
    ocr_enabled: bool,
    /// OCR languages to use.
    ocr_languages: Vec<String>,
    /// OCR DPI setting.
    ocr_dpi: u32,
}

impl TestHarness {
    /// Create a new test harness with default settings (OCR disabled).
    pub fn new() -> Self {
        Self::with_ocr(false, vec!["eng".to_string()], 300)
    }

    /// Create a new test harness with custom OCR settings.
    pub fn with_ocr(enabled: bool, languages: Vec<String>, dpi: u32) -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let base = temp_dir.path();

        let input_dir = base.join("input");
        let output_dir = base.join("output");
        let config_dir = base.join("config");

        std::fs::create_dir_all(&input_dir).expect("Failed to create input dir");
        std::fs::create_dir_all(&output_dir).expect("Failed to create output dir");
        std::fs::create_dir_all(&config_dir).expect("Failed to create config dir");

        Self {
            temp_dir,
            input_dir,
            output_dir,
            config_dir,
            ocr_enabled: enabled,
            ocr_languages: languages,
            ocr_dpi: dpi,
        }
    }

    /// Get the base temp directory path.
    pub fn temp_path(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Write a test input file to the input directory.
    pub fn write_input(&self, filename: &str, content: &[u8]) -> PathBuf {
        let path = self.input_dir.join(filename);
        std::fs::write(&path, content).expect("Failed to write input file");
        path
    }

    /// Write a text input file to the input directory.
    pub fn write_text_input(&self, filename: &str, content: &str) -> PathBuf {
        self.write_input(filename, content.as_bytes())
    }

    /// Write a config file to the config directory.
    pub fn write_config(&self, filename: &str, config: &Config) -> PathBuf {
        let path = self.config_dir.join(filename);
        let json = serde_json::to_string_pretty(config).expect("Failed to serialize config");
        std::fs::write(&path, json).expect("Failed to write config file");
        path
    }

    /// Create a processor registry with the harness's OCR settings.
    pub fn create_processor_registry(&self) -> ProcessorRegistry {
        ProcessorRegistry::new(self.ocr_enabled, &self.ocr_languages, self.ocr_dpi)
    }

    /// Create a categorizer from the given config.
    pub fn create_categorizer(&self, config: &Config) -> Categorizer {
        Categorizer::new(config.rules.clone(), config.defaults.clone())
    }

    /// Create a variable engine from the given config.
    pub fn create_variable_engine(&self, config: &Config) -> VariableEngine {
        VariableEngine::new(&config.variables.extracted)
    }

    /// Create a file storage pointing to the output directory.
    pub fn create_storage(&self) -> FileStorage {
        FileStorage::new(&self.output_dir)
    }

    /// Process a single document through the full pipeline.
    ///
    /// This runs:
    /// 1. Document processing (text extraction, PDF generation)
    /// 2. Categorization (rule matching)
    /// 3. Variable extraction
    /// 4. Path substitution
    ///
    /// Does NOT store the document - use `run_pipeline_with_storage` for that.
    pub fn run_pipeline(&self, input_path: &Path, config: &Config) -> PipelineResult {
        self.run_pipeline_internal(input_path, config, false)
    }

    /// Process a document through the full pipeline including storage.
    pub fn run_pipeline_with_storage(&self, input_path: &Path, config: &Config) -> PipelineResult {
        self.run_pipeline_internal(input_path, config, true)
    }

    fn run_pipeline_internal(
        &self,
        input_path: &Path,
        config: &Config,
        store: bool,
    ) -> PipelineResult {
        // Process the document
        let registry = self.create_processor_registry();
        let processed = registry
            .process(input_path)
            .expect("Failed to process document");

        // Categorize
        let categorizer = self.create_categorizer(config);
        let categorization = categorizer.categorize(&processed.text);

        // Extract variables
        let variable_engine = self.create_variable_engine(config);
        let extracted_variables = variable_engine.extract_variables(&processed.text);

        // Substitute paths
        let output_directory = variable_engine.substitute(
            &categorization.output.directory,
            &processed.metadata.original_filename,
            &extracted_variables,
        );
        let output_filename = variable_engine.substitute(
            &categorization.output.filename,
            &processed.metadata.original_filename,
            &extracted_variables,
        );

        // Optionally store
        let stored_path = if store {
            let storage = self.create_storage();
            Some(
                storage
                    .store(
                        &processed.pdf_bytes,
                        &output_directory,
                        &output_filename,
                        "pdf",
                    )
                    .expect("Failed to store document"),
            )
        } else {
            None
        };

        PipelineResult {
            processed,
            categorization,
            extracted_variables,
            output_directory,
            output_filename,
            stored_path,
        }
    }

    /// Verify a file exists at the expected output path.
    pub fn assert_output_exists(&self, relative_path: &str) {
        let path = self.output_dir.join(relative_path);
        assert!(
            path.exists(),
            "Expected output file does not exist: {:?}",
            path
        );
    }

    /// Read a stored output file's contents.
    pub fn read_output(&self, relative_path: &str) -> Vec<u8> {
        let path = self.output_dir.join(relative_path);
        std::fs::read(&path).expect("Failed to read output file")
    }

    /// List all files in the output directory (recursively).
    pub fn list_outputs(&self) -> Vec<PathBuf> {
        walkdir::WalkDir::new(&self.output_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| {
                e.path()
                    .strip_prefix(&self.output_dir)
                    .ok()
                    .map(|p| p.to_path_buf())
            })
            .collect()
    }
}

impl Default for TestHarness {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a minimal config suitable for tests with configurable paths.
pub fn minimal_config_with_paths(input_dir: &Path, output_dir: &Path) -> Config {
    Config {
        version: "1.0".to_string(),
        input_directory: input_dir.to_string_lossy().to_string(),
        output_directory: output_dir.to_string_lossy().to_string(),
        worker_count: 1,
        ocr: Default::default(),
        variables: VariablesConfig::default(),
        rules: vec![],
        defaults: DefaultsConfig {
            output: OutputConfig {
                directory: "$y/unsorted".to_string(),
                filename: "$original".to_string(),
            },
        },
        ai: Default::default(),
    }
}

/// Create a minimal config with default temp paths (for backward compatibility).
pub fn minimal_config() -> Config {
    let temp = std::env::temp_dir();
    minimal_config_with_paths(&temp.join("input"), &temp.join("output"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_harness_creates_directories() {
        let harness = TestHarness::new();

        assert!(harness.input_dir.exists());
        assert!(harness.output_dir.exists());
        assert!(harness.config_dir.exists());
    }

    #[test]
    fn test_write_input_file() {
        let harness = TestHarness::new();
        let path = harness.write_text_input("test.txt", "Hello, World!");

        assert!(path.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "Hello, World!");
    }

    #[test]
    fn test_minimal_config() {
        let config = minimal_config();

        assert_eq!(config.version, "1.0");
        assert!(config.rules.is_empty());
    }
}
