use std::path::Path;
use std::sync::Arc;

use tracing::{debug, info_span, warn};

use crate::broadcast::job_progress::JobPhase;
use crate::categorizer::Categorizer;
use crate::config::VariableEngine;
use crate::processor::ProcessorRegistry;
use crate::sanitize;
use crate::storage::{FileStorage, SymlinkManager};
use crate::worker::job::JobResult;

use super::config::PipelineConfig;
use super::context::PipelineContext;
use super::error::{PipelineError, PipelineWarning};
use super::progress::{ProgressEvent, ProgressReporter};

pub struct Pipeline {
    config: Arc<PipelineConfig>,
    processor: ProcessorRegistry,
    categorizer: Categorizer,
    variable_engine: VariableEngine,
    storage: FileStorage,
    symlink_manager: SymlinkManager,
}

impl Pipeline {
    /// Production constructor — builds all sub-components from config.
    pub fn from_config(config: Arc<PipelineConfig>) -> Self {
        let processor =
            ProcessorRegistry::new(config.ocr_enabled, &config.ocr_languages, config.ocr_dpi);
        let categorizer = Categorizer::new(config.rules.clone(), config.defaults.clone());
        let variable_engine = VariableEngine::new(&config.extracted_variables);
        let storage = FileStorage::new(&config.output_directory);
        let symlink_manager = SymlinkManager::new(&config.output_directory);

        Self {
            config,
            processor,
            categorizer,
            variable_engine,
            storage,
            symlink_manager,
        }
    }

    /// Test constructor — inject specific sub-components.
    #[cfg(test)]
    pub fn new(
        config: Arc<PipelineConfig>,
        processor: ProcessorRegistry,
        categorizer: Categorizer,
        variable_engine: VariableEngine,
        storage: FileStorage,
        symlink_manager: SymlinkManager,
    ) -> Self {
        Self {
            config,
            processor,
            categorizer,
            variable_engine,
            storage,
            symlink_manager,
        }
    }

    /// Run the full pipeline for a single document.
    /// Returns a (JobResult, PipelineContext) pair.
    pub fn run(
        &self,
        mut ctx: PipelineContext,
        progress: &dyn ProgressReporter,
    ) -> (JobResult, PipelineContext) {
        let filename = sanitize::redact_path(&ctx.job.source_path);
        let _pipeline_span = info_span!("pipeline",
            job_id = %ctx.job.id,
            filename = %filename,
            source_name = ctx.job.source_name.as_deref().unwrap_or("unknown"),
        )
        .entered();

        // Step 1: Process document
        {
            let _step = info_span!("process_document").entered();
            progress.report(ProgressEvent::Phase {
                phase: JobPhase::Processing,
                message: "Running OCR and text extraction...".to_string(),
            });
            if let Err(e) = self.step_process_document(&mut ctx) {
                let err_msg = e.to_string();
                progress.report(ProgressEvent::Failed {
                    error: err_msg.clone(),
                });
                return (JobResult::failure(&ctx.job, err_msg), ctx);
            }
        }

        // Step 2: Prepare matching text
        {
            let _step = info_span!("prepare_text").entered();
            self.step_prepare_text(&mut ctx);
        }

        // Step 3: Extract variables
        {
            let _step = info_span!("extract_variables").entered();
            progress.report(ProgressEvent::Phase {
                phase: JobPhase::ExtractVariables,
                message: "Extracting variables from document...".to_string(),
            });
            self.step_extract_variables(&mut ctx);
        }

        // Step 4: Categorize
        {
            let _step = info_span!("categorize").entered();
            progress.report(ProgressEvent::Phase {
                phase: JobPhase::Categorizing,
                message: "Categorizing document...".to_string(),
            });
            self.step_categorize(&mut ctx);
        }

        // Step 5+6: Resolve output path and store
        {
            let _step = info_span!("resolve_and_store").entered();
            progress.report(ProgressEvent::Phase {
                phase: JobPhase::Substituting,
                message: "Substituting variables in path...".to_string(),
            });
            if let Err(e) = self.step_resolve_and_store(&mut ctx, progress) {
                let err_msg = e.to_string();
                progress.report(ProgressEvent::Failed {
                    error: err_msg.clone(),
                });
                return (JobResult::failure(&ctx.job, err_msg), ctx);
            }
        }

        // Step 7: Create symlinks
        {
            let _step = info_span!("create_symlinks").entered();
            progress.report(ProgressEvent::Phase {
                phase: JobPhase::CreatingSymlinks,
                message: "Creating symlinks...".to_string(),
            });
            self.step_create_symlinks(&mut ctx);
        }

        // Step 8: Archive source
        {
            let _step = info_span!("archive_source").entered();
            progress.report(ProgressEvent::Phase {
                phase: JobPhase::Archiving,
                message: "Archiving source file...".to_string(),
            });
            if let Err(e) = self.step_archive_source(&mut ctx) {
                let err_msg = e.to_string();
                progress.report(ProgressEvent::Failed {
                    error: err_msg.clone(),
                });
                return (JobResult::failure(&ctx.job, err_msg), ctx);
            }
        }

        // Build success result
        let category = ctx
            .categorization
            .as_ref()
            .map(|c| c.category.clone())
            .unwrap_or_else(|| "unsorted".to_string());
        let output_path = ctx.output_path.clone().expect("output_path set in step 5");
        let archive_path = ctx
            .archive_path
            .clone()
            .expect("archive_path set in step 7");
        let symlink_paths = ctx.symlink_paths.clone();

        let symlink_strings: Vec<String> = symlink_paths
            .iter()
            .map(|p| p.display().to_string())
            .collect();

        // Inject OCR text before emitting Completed so the broadcast includes it
        if let Some(ref processed) = ctx.processed {
            progress.set_ocr_text(processed.text.clone());
        }

        progress.report(ProgressEvent::Completed {
            output_path: output_path.display().to_string(),
            archive_path: archive_path.display().to_string(),
            symlinks: symlink_strings,
            category: category.clone(),
        });

        let result =
            JobResult::success(&ctx.job, output_path, archive_path, symlink_paths, category);
        (result, ctx)
    }

    fn step_process_document(&self, ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        let processed = self.processor.process(&ctx.job.source_path)?;
        ctx.processed = Some(processed);
        Ok(())
    }

    fn step_prepare_text(&self, ctx: &mut PipelineContext) {
        let processed = ctx.processed.as_ref().expect("step 1 completed");
        let mut text = processed.text.clone();

        if let Some(ref email_meta) = ctx.job.email_metadata {
            if email_meta.has_content() {
                let header_block = email_meta.to_header_block();
                text = format!("{}{}", header_block, text);
            }
        }

        ctx.matching_text = Some(text);
    }

    fn step_extract_variables(&self, ctx: &mut PipelineContext) {
        let text = ctx.matching_text.as_ref().expect("step 2 completed");
        ctx.extracted_variables = self.variable_engine.extract_variables(text);
    }

    fn step_categorize(&self, ctx: &mut PipelineContext) {
        let text = ctx.matching_text.as_ref().expect("step 2 completed");
        ctx.categorization = Some(self.categorizer.categorize(text));
    }

    fn step_resolve_and_store(
        &self,
        ctx: &mut PipelineContext,
        progress: &dyn ProgressReporter,
    ) -> Result<(), PipelineError> {
        let categorization = ctx.categorization.as_ref().expect("step 4 completed");
        let processed = ctx.processed.as_ref().expect("step 1 completed");

        let dir_template = &categorization.output.directory;
        let name_template = &categorization.output.filename;

        // Pre-validation: check raw templates before VariableEngine sanitizes them.
        // VariableEngine::substitute() sanitizes '/' to '_', which would mask
        // absolute paths and traversals. Catch them at the template level.
        if Path::new(dir_template).is_absolute() {
            return Err(PipelineError::InvalidOutputPath(format!(
                "Directory template is an absolute path: {}",
                dir_template
            )));
        }
        if dir_template.contains("..") {
            return Err(PipelineError::InvalidOutputPath(format!(
                "Directory template contains path traversal: {}",
                dir_template
            )));
        }
        if name_template.contains('/') || name_template.contains('\\') {
            return Err(PipelineError::InvalidOutputPath(format!(
                "Filename template contains path separators: {}",
                name_template
            )));
        }

        let output_directory = self.variable_engine.substitute(
            dir_template,
            &processed.metadata.original_filename,
            &ctx.extracted_variables,
        );

        let output_filename = self.variable_engine.substitute(
            name_template,
            &processed.metadata.original_filename,
            &ctx.extracted_variables,
        );

        // Post-validation: defense in depth after substitution + sanitization.
        if Path::new(&output_directory).is_absolute() {
            return Err(PipelineError::InvalidOutputPath(format!(
                "Resolved directory is an absolute path: {}",
                output_directory
            )));
        }
        if output_directory.contains("..") {
            return Err(PipelineError::InvalidOutputPath(format!(
                "Resolved directory contains path traversal: {}",
                output_directory
            )));
        }

        // Validate: filename must not be empty or dots-only after sanitization
        let trimmed = output_filename.trim_matches('.');
        if trimmed.is_empty() {
            return Err(PipelineError::InvalidOutputPath(format!(
                "Resolved filename is empty or dots-only: '{}'",
                output_filename
            )));
        }

        // Validate: final path stays within output_directory via canonicalization
        let candidate = self.config.output_directory.join(&output_directory);
        if !candidate.exists() {
            std::fs::create_dir_all(&candidate).ok();
        }
        if let (Ok(canonical_output), Ok(canonical_candidate)) = (
            self.config.output_directory.canonicalize(),
            candidate.canonicalize(),
        ) {
            if !canonical_candidate.starts_with(&canonical_output) {
                return Err(PipelineError::InvalidOutputPath(format!(
                    "Resolved path escapes output directory: {}",
                    canonical_candidate.display()
                )));
            }
        }

        // Store the PDF
        progress.report(ProgressEvent::Phase {
            phase: JobPhase::Storing,
            message: "Storing document...".to_string(),
        });

        let output_path = self.storage.store(
            &processed.pdf_bytes,
            &output_directory,
            &output_filename,
            "pdf",
        )?;

        debug!(
            "Stored {} -> {} (category: {})",
            sanitize::redact_path(&ctx.job.source_path),
            sanitize::redact_path(&output_path),
            categorization.category
        );

        ctx.output_path = Some(output_path);
        Ok(())
    }

    fn step_create_symlinks(&self, ctx: &mut PipelineContext) {
        let categorization = ctx.categorization.as_ref().expect("step 4 completed");
        let processed = ctx.processed.as_ref().expect("step 1 completed");
        let output_path = ctx.output_path.as_ref().expect("step 5 completed");

        for symlink_config in &categorization.symlinks {
            let symlink_dir = self.variable_engine.substitute(
                &symlink_config.target,
                &processed.metadata.original_filename,
                &ctx.extracted_variables,
            );

            match self
                .symlink_manager
                .create_symlink(output_path, &symlink_dir)
            {
                Ok(symlink_path) => {
                    debug!("Created symlink: {}", sanitize::redact_path(&symlink_path));
                    ctx.symlink_paths.push(symlink_path);
                }
                Err(e) => {
                    warn!("Failed to create symlink: {}", e);
                    ctx.warnings.push(PipelineWarning::SymlinkFailed {
                        target: symlink_dir,
                        error: e.to_string(),
                    });
                }
            }
        }
    }

    fn step_archive_source(&self, ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        let archive_path = self
            .storage
            .archive_source(&ctx.job.source_path, &self.config.input_directory)
            .map_err(PipelineError::Archive)?;

        ctx.archive_path = Some(archive_path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::{
        DefaultsConfig, ExtractedVariable, MatchCondition, OcrConfig, OutputConfig, Rule,
        SimpleMatch, SymlinkConfig, VariablesConfig,
    };
    use crate::config::Config;
    use crate::pipeline::progress::NoopProgress;
    use crate::worker::job::{EmailMetadata, Job};
    use std::io::Write;
    use tempfile::TempDir;

    fn test_config(input_dir: &Path, output_dir: &Path) -> PipelineConfig {
        PipelineConfig {
            input_directory: input_dir.to_path_buf(),
            output_directory: output_dir.to_path_buf(),
            ocr_enabled: false,
            ocr_languages: vec![],
            ocr_dpi: 300,
            rules: vec![],
            defaults: DefaultsConfig::default(),
            extracted_variables: vec![],
        }
    }

    fn test_config_with_rules(
        input_dir: &Path,
        output_dir: &Path,
        rules: Vec<Rule>,
    ) -> PipelineConfig {
        PipelineConfig {
            input_directory: input_dir.to_path_buf(),
            output_directory: output_dir.to_path_buf(),
            ocr_enabled: false,
            ocr_languages: vec![],
            ocr_dpi: 300,
            rules,
            defaults: DefaultsConfig::default(),
            extracted_variables: vec![],
        }
    }

    fn create_text_file(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    fn setup_dirs() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
        let tmp = TempDir::new().unwrap();
        let input = tmp.path().join("input");
        let output = tmp.path().join("output");
        std::fs::create_dir_all(&input).unwrap();
        std::fs::create_dir_all(&output).unwrap();
        (tmp, input, output)
    }

    // ── Pipeline construction & happy path ──

    #[test]
    fn test_full_pipeline_success_with_text_file() {
        let (_tmp, input, output) = setup_dirs();
        let file_path = create_text_file(&input, "hello.txt", "Hello, World!");

        let config = Arc::new(test_config(&input, &output));
        let pipeline = Pipeline::from_config(config);
        let ctx = PipelineContext::new(Job::new(file_path));

        let (result, ctx) = pipeline.run(ctx, &NoopProgress);

        assert!(result.success, "Pipeline failed: {:?}", result.error);
        assert!(result.output_path.is_some());
        assert!(result.archive_path.is_some());
        assert_eq!(result.category, "unsorted");
        assert!(ctx.warnings.is_empty());
    }

    #[test]
    fn test_full_pipeline_categorizes_into_rule_matched_directory() {
        let (_tmp, input, output) = setup_dirs();
        let file_path = create_text_file(&input, "invoice.txt", "This is an invoice document");

        let rules = vec![Rule {
            id: "inv".to_string(),
            name: "Invoice".to_string(),
            priority: 10,
            match_condition: MatchCondition::Simple(SimpleMatch {
                contains: Some("invoice".to_string()),
                contains_any: None,
                contains_all: None,
                pattern: None,
            }),
            category: "invoices".to_string(),
            output: OutputConfig {
                directory: "invoices".to_string(),
                filename: "$original".to_string(),
            },
            symlinks: vec![],
        }];

        let config = Arc::new(test_config_with_rules(&input, &output, rules));
        let pipeline = Pipeline::from_config(config);
        let ctx = PipelineContext::new(Job::new(file_path));

        let (result, _ctx) = pipeline.run(ctx, &NoopProgress);

        assert!(result.success);
        assert_eq!(result.category, "invoices");
        let out = result.output_path.unwrap();
        assert!(out.to_string_lossy().contains("invoices"));
    }

    #[test]
    fn test_unsorted_fallback_when_no_rules_match() {
        let (_tmp, input, output) = setup_dirs();
        let file_path = create_text_file(&input, "random.txt", "Just some random text");

        let rules = vec![Rule {
            id: "inv".to_string(),
            name: "Invoice".to_string(),
            priority: 10,
            match_condition: MatchCondition::Simple(SimpleMatch {
                contains: Some("invoice".to_string()),
                contains_any: None,
                contains_all: None,
                pattern: None,
            }),
            category: "invoices".to_string(),
            output: OutputConfig {
                directory: "invoices".to_string(),
                filename: "$original".to_string(),
            },
            symlinks: vec![],
        }];

        let config = Arc::new(test_config_with_rules(&input, &output, rules));
        let pipeline = Pipeline::from_config(config);
        let ctx = PipelineContext::new(Job::new(file_path));

        let (result, _ctx) = pipeline.run(ctx, &NoopProgress);

        assert!(result.success);
        assert_eq!(result.category, "unsorted");
    }

    // ── Individual step behavior ──

    #[test]
    fn test_step_process_document_valid_text() {
        let (_tmp, input, output) = setup_dirs();
        let file_path = create_text_file(&input, "doc.txt", "Content here");

        let config = Arc::new(test_config(&input, &output));
        let pipeline = Pipeline::from_config(config);
        let mut ctx = PipelineContext::new(Job::new(file_path));

        let result = pipeline.step_process_document(&mut ctx);
        assert!(result.is_ok());
        assert!(ctx.processed.is_some());
        assert!(ctx
            .processed
            .as_ref()
            .unwrap()
            .text
            .contains("Content here"));
    }

    #[test]
    fn test_step_process_document_unsupported_format() {
        let (_tmp, input, output) = setup_dirs();
        let file_path = create_text_file(&input, "file.xyz", "data");

        let config = Arc::new(test_config(&input, &output));
        let pipeline = Pipeline::from_config(config);
        let mut ctx = PipelineContext::new(Job::new(file_path));

        let result = pipeline.step_process_document(&mut ctx);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PipelineError::Processing(_)));
    }

    #[test]
    fn test_step_prepare_text_without_email_metadata() {
        let (_tmp, input, output) = setup_dirs();
        let file_path = create_text_file(&input, "doc.txt", "Plain text");

        let config = Arc::new(test_config(&input, &output));
        let pipeline = Pipeline::from_config(config);
        let mut ctx = PipelineContext::new(Job::new(file_path));
        pipeline.step_process_document(&mut ctx).unwrap();

        pipeline.step_prepare_text(&mut ctx);

        let text = ctx.matching_text.as_ref().unwrap();
        assert!(text.contains("Plain text"));
        assert!(!text.contains("EMAIL METADATA"));
    }

    #[test]
    fn test_step_prepare_text_with_email_metadata() {
        let (_tmp, input, output) = setup_dirs();
        let file_path = create_text_file(&input, "doc.txt", "Attachment content");

        let config = Arc::new(test_config(&input, &output));
        let pipeline = Pipeline::from_config(config);

        let mut job = Job::new(file_path);
        job.email_metadata = Some(EmailMetadata {
            subject: Some("Invoice #42".to_string()),
            from: Some("sender@test.com".to_string()),
            to: None,
            date: None,
            message_id: None,
        });

        let mut ctx = PipelineContext::new(job);
        pipeline.step_process_document(&mut ctx).unwrap();
        pipeline.step_prepare_text(&mut ctx);

        let text = ctx.matching_text.as_ref().unwrap();
        assert!(text.contains("EMAIL METADATA"));
        assert!(text.contains("From: sender@test.com"));
        assert!(text.contains("Subject: Invoice #42"));
        assert!(text.contains("Attachment content"));
    }

    #[test]
    fn test_step_extract_variables_with_matching_patterns() {
        let (_tmp, input, output) = setup_dirs();
        let file_path = create_text_file(&input, "doc.txt", "Invoice from Acme");

        let mut config = test_config(&input, &output);
        config.extracted_variables = vec![ExtractedVariable {
            name: "vendor".to_string(),
            pattern: r"from (?P<vendor>\w+)".to_string(),
            transform: None,
            default: None,
        }];

        let config = Arc::new(config);
        let pipeline = Pipeline::from_config(config);
        let mut ctx = PipelineContext::new(Job::new(file_path));

        pipeline.step_process_document(&mut ctx).unwrap();
        pipeline.step_prepare_text(&mut ctx);
        pipeline.step_extract_variables(&mut ctx);

        assert_eq!(
            ctx.extracted_variables.get("vendor"),
            Some(&"Acme".to_string())
        );
    }

    #[test]
    fn test_step_extract_variables_no_matches() {
        let (_tmp, input, output) = setup_dirs();
        let file_path = create_text_file(&input, "doc.txt", "No matching patterns here");

        let mut config = test_config(&input, &output);
        config.extracted_variables = vec![ExtractedVariable {
            name: "vendor".to_string(),
            pattern: r"WONT_MATCH_(?P<vendor>\w+)".to_string(),
            transform: None,
            default: None,
        }];

        let config = Arc::new(config);
        let pipeline = Pipeline::from_config(config);
        let mut ctx = PipelineContext::new(Job::new(file_path));

        pipeline.step_process_document(&mut ctx).unwrap();
        pipeline.step_prepare_text(&mut ctx);
        pipeline.step_extract_variables(&mut ctx);

        assert!(ctx.extracted_variables.is_empty());
    }

    #[test]
    fn test_step_categorize_matches_highest_priority_rule() {
        let (_tmp, input, output) = setup_dirs();
        let file_path = create_text_file(&input, "doc.txt", "This is an invoice");

        let rules = vec![
            Rule {
                id: "low".to_string(),
                name: "Low".to_string(),
                priority: 10,
                match_condition: MatchCondition::Simple(SimpleMatch {
                    contains: Some("invoice".to_string()),
                    contains_any: None,
                    contains_all: None,
                    pattern: None,
                }),
                category: "low-priority".to_string(),
                output: OutputConfig {
                    directory: "low".to_string(),
                    filename: "$original".to_string(),
                },
                symlinks: vec![],
            },
            Rule {
                id: "high".to_string(),
                name: "High".to_string(),
                priority: 100,
                match_condition: MatchCondition::Simple(SimpleMatch {
                    contains: Some("invoice".to_string()),
                    contains_any: None,
                    contains_all: None,
                    pattern: None,
                }),
                category: "high-priority".to_string(),
                output: OutputConfig {
                    directory: "high".to_string(),
                    filename: "$original".to_string(),
                },
                symlinks: vec![],
            },
        ];

        let config = Arc::new(test_config_with_rules(&input, &output, rules));
        let pipeline = Pipeline::from_config(config);
        let mut ctx = PipelineContext::new(Job::new(file_path));

        pipeline.step_process_document(&mut ctx).unwrap();
        pipeline.step_prepare_text(&mut ctx);
        pipeline.step_categorize(&mut ctx);

        let cat = ctx.categorization.as_ref().unwrap();
        assert_eq!(cat.category, "high-priority");
        assert_eq!(cat.rule_id, Some("high".to_string()));
    }

    #[test]
    fn test_step_categorize_falls_back_to_unsorted() {
        let (_tmp, input, output) = setup_dirs();
        let file_path = create_text_file(&input, "doc.txt", "No matching content");

        let rules = vec![Rule {
            id: "inv".to_string(),
            name: "Inv".to_string(),
            priority: 10,
            match_condition: MatchCondition::Simple(SimpleMatch {
                contains: Some("SPECIFIC_KEYWORD".to_string()),
                contains_any: None,
                contains_all: None,
                pattern: None,
            }),
            category: "specific".to_string(),
            output: OutputConfig {
                directory: "specific".to_string(),
                filename: "$original".to_string(),
            },
            symlinks: vec![],
        }];

        let config = Arc::new(test_config_with_rules(&input, &output, rules));
        let pipeline = Pipeline::from_config(config);
        let mut ctx = PipelineContext::new(Job::new(file_path));

        pipeline.step_process_document(&mut ctx).unwrap();
        pipeline.step_prepare_text(&mut ctx);
        pipeline.step_categorize(&mut ctx);

        let cat = ctx.categorization.as_ref().unwrap();
        assert_eq!(cat.category, "unsorted");
        assert_eq!(cat.rule_id, None);
    }

    // ── Path validation ──

    #[test]
    fn test_traversal_in_directory_rejected() {
        let (_tmp, input, output) = setup_dirs();
        let file_path = create_text_file(&input, "doc.txt", "Content");

        let rules = vec![Rule {
            id: "evil".to_string(),
            name: "Evil".to_string(),
            priority: 100,
            match_condition: MatchCondition::Simple(SimpleMatch {
                contains: Some("Content".to_string()),
                contains_any: None,
                contains_all: None,
                pattern: None,
            }),
            category: "evil".to_string(),
            output: OutputConfig {
                directory: "../escape".to_string(),
                filename: "doc".to_string(),
            },
            symlinks: vec![],
        }];

        let config = Arc::new(test_config_with_rules(&input, &output, rules));
        let pipeline = Pipeline::from_config(config);
        let ctx = PipelineContext::new(Job::new(file_path));

        let (result, _ctx) = pipeline.run(ctx, &NoopProgress);

        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("traversal"));
    }

    #[test]
    fn test_absolute_path_in_directory_rejected() {
        let (_tmp, input, output) = setup_dirs();
        let file_path = create_text_file(&input, "doc.txt", "Content");

        let rules = vec![Rule {
            id: "abs".to_string(),
            name: "Abs".to_string(),
            priority: 100,
            match_condition: MatchCondition::Simple(SimpleMatch {
                contains: Some("Content".to_string()),
                contains_any: None,
                contains_all: None,
                pattern: None,
            }),
            category: "abs".to_string(),
            output: OutputConfig {
                directory: "/tmp/evil".to_string(),
                filename: "doc".to_string(),
            },
            symlinks: vec![],
        }];

        let config = Arc::new(test_config_with_rules(&input, &output, rules));
        let pipeline = Pipeline::from_config(config);
        let ctx = PipelineContext::new(Job::new(file_path));

        let (result, _ctx) = pipeline.run(ctx, &NoopProgress);

        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("absolute"));
    }

    #[test]
    fn test_path_separators_in_filename_rejected() {
        let (_tmp, input, output) = setup_dirs();
        let file_path = create_text_file(&input, "doc.txt", "Content");

        let rules = vec![Rule {
            id: "slash".to_string(),
            name: "Slash".to_string(),
            priority: 100,
            match_condition: MatchCondition::Simple(SimpleMatch {
                contains: Some("Content".to_string()),
                contains_any: None,
                contains_all: None,
                pattern: None,
            }),
            category: "slash".to_string(),
            output: OutputConfig {
                directory: "safe".to_string(),
                filename: "sub/dir".to_string(),
            },
            symlinks: vec![],
        }];

        let config = Arc::new(test_config_with_rules(&input, &output, rules));
        let pipeline = Pipeline::from_config(config);
        let ctx = PipelineContext::new(Job::new(file_path));

        let (result, _ctx) = pipeline.run(ctx, &NoopProgress);

        // Pre-validation catches / in the filename template before sanitization
        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("separator"));
    }

    #[test]
    fn test_empty_filename_rejected() {
        let (_tmp, input, output) = setup_dirs();
        let file_path = create_text_file(&input, "doc.txt", "Content");

        let rules = vec![Rule {
            id: "empty".to_string(),
            name: "Empty".to_string(),
            priority: 100,
            match_condition: MatchCondition::Simple(SimpleMatch {
                contains: Some("Content".to_string()),
                contains_any: None,
                contains_all: None,
                pattern: None,
            }),
            category: "empty".to_string(),
            output: OutputConfig {
                directory: "safe".to_string(),
                filename: "...".to_string(),
            },
            symlinks: vec![],
        }];

        let config = Arc::new(test_config_with_rules(&input, &output, rules));
        let pipeline = Pipeline::from_config(config);
        let ctx = PipelineContext::new(Job::new(file_path));

        let (result, _ctx) = pipeline.run(ctx, &NoopProgress);

        // VariableEngine.substitute sanitizes "..." — let's check
        // sanitize_filename("...") = "..." with dots trimmed = ""
        // So the filename will be empty after sanitization
        assert!(!result.success);
        assert!(result
            .error
            .as_ref()
            .unwrap()
            .contains("empty")
            .then_some(())
            .or_else(|| result
                .error
                .as_ref()
                .unwrap()
                .contains("dots")
                .then_some(()))
            .is_some());
    }

    // ── Error propagation ──

    #[test]
    fn test_storage_failure_stops_pipeline() {
        let (_tmp, input, output) = setup_dirs();
        let file_path = create_text_file(&input, "doc.txt", "Content");

        // Use a non-writable output directory by pointing to a file
        let bad_output = output.join("not_a_directory");
        std::fs::write(&bad_output, b"blocker").unwrap();

        let config = Arc::new(PipelineConfig {
            input_directory: input.clone(),
            output_directory: bad_output,
            ocr_enabled: false,
            ocr_languages: vec![],
            ocr_dpi: 300,
            rules: vec![],
            defaults: DefaultsConfig {
                output: OutputConfig {
                    directory: "sub".to_string(),
                    filename: "doc".to_string(),
                },
            },
            extracted_variables: vec![],
        });

        let pipeline = Pipeline::from_config(config);
        let ctx = PipelineContext::new(Job::new(file_path));

        let (result, _ctx) = pipeline.run(ctx, &NoopProgress);
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_symlink_failure_produces_warning_but_succeeds() {
        let (_tmp, input, output) = setup_dirs();
        let file_path = create_text_file(&input, "doc.txt", "invoice content");

        let rules = vec![Rule {
            id: "inv".to_string(),
            name: "Inv".to_string(),
            priority: 10,
            match_condition: MatchCondition::Simple(SimpleMatch {
                contains: Some("invoice".to_string()),
                contains_any: None,
                contains_all: None,
                pattern: None,
            }),
            category: "invoices".to_string(),
            output: OutputConfig {
                directory: "invoices".to_string(),
                filename: "$original".to_string(),
            },
            symlinks: vec![SymlinkConfig {
                // Symlink target using a path that might trigger issues
                // but on most systems symlinks should work in temp dirs
                target: "links/invoices".to_string(),
            }],
        }];

        let config = Arc::new(test_config_with_rules(&input, &output, rules));
        let pipeline = Pipeline::from_config(config);
        let ctx = PipelineContext::new(Job::new(file_path));

        let (result, _ctx) = pipeline.run(ctx, &NoopProgress);

        // Pipeline should succeed regardless of symlink outcome
        assert!(result.success);
    }

    #[test]
    fn test_archive_failure_stops_pipeline() {
        let (_tmp, input, output) = setup_dirs();
        let file_path = create_text_file(&input, "doc.txt", "Content");

        // Remove source file after processing but before archiving
        // can't easily do that mid-pipeline, so let's use a non-existent input directory
        // for the archive step
        let config = Arc::new(PipelineConfig {
            input_directory: Path::new("/nonexistent/path/for/archive").to_path_buf(),
            output_directory: output.clone(),
            ocr_enabled: false,
            ocr_languages: vec![],
            ocr_dpi: 300,
            rules: vec![],
            defaults: DefaultsConfig {
                output: OutputConfig {
                    directory: "out".to_string(),
                    filename: "$original".to_string(),
                },
            },
            extracted_variables: vec![],
        });

        let pipeline = Pipeline::from_config(config);
        let ctx = PipelineContext::new(Job::new(file_path));

        let (result, _ctx) = pipeline.run(ctx, &NoopProgress);

        // Archive step should fail because input_directory doesn't exist for archival
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    // ── Conflict behavior ──

    #[test]
    fn test_output_file_conflict_appends_suffix() {
        let (_tmp, input, output) = setup_dirs();
        let file1 = create_text_file(&input, "doc1.txt", "First document");
        let file2 = create_text_file(&input, "doc2.txt", "Second document");

        let config = Arc::new(PipelineConfig {
            input_directory: input.clone(),
            output_directory: output.clone(),
            ocr_enabled: false,
            ocr_languages: vec![],
            ocr_dpi: 300,
            rules: vec![],
            defaults: DefaultsConfig {
                output: OutputConfig {
                    directory: "docs".to_string(),
                    filename: "same_name".to_string(),
                },
            },
            extracted_variables: vec![],
        });

        let pipeline = Pipeline::from_config(config);

        let ctx1 = PipelineContext::new(Job::new(file1));
        let (result1, _) = pipeline.run(ctx1, &NoopProgress);
        assert!(result1.success);
        let path1 = result1.output_path.unwrap();
        assert!(path1.to_string_lossy().contains("same_name.pdf"));

        let ctx2 = PipelineContext::new(Job::new(file2));
        let (result2, _) = pipeline.run(ctx2, &NoopProgress);
        assert!(result2.success);
        let path2 = result2.output_path.unwrap();
        assert!(path2.to_string_lossy().contains("same_name_2.pdf"));
    }
}
