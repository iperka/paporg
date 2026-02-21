use std::collections::HashMap;
use std::path::PathBuf;

use crate::categorizer::CategorizationResult;
use crate::processor::ProcessedContent;
use crate::worker::job::Job;

use super::error::PipelineWarning;

pub struct PipelineContext {
    // Input
    pub job: Job,

    // Step 1 result — guaranteed Some after step_process_document
    pub processed: Option<ProcessedContent>,

    // Step 2 result — guaranteed Some after step_prepare_text
    pub matching_text: Option<String>,

    // Step 3 result
    pub extracted_variables: HashMap<String, String>,

    // Step 4 result — guaranteed Some after step_categorize
    pub categorization: Option<CategorizationResult>,

    // Step 5+6 result — the final stored path (FileStorage handles conflict resolution)
    pub output_path: Option<PathBuf>,

    // Step 6 results
    pub symlink_paths: Vec<PathBuf>,

    // Step 7 result
    pub archive_path: Option<PathBuf>,

    // Non-fatal warnings
    pub warnings: Vec<PipelineWarning>,
}

impl PipelineContext {
    pub fn new(job: Job) -> Self {
        Self {
            job,
            processed: None,
            matching_text: None,
            extracted_variables: HashMap::new(),
            categorization: None,
            output_path: None,
            symlink_paths: Vec::new(),
            archive_path: None,
            warnings: Vec::new(),
        }
    }
}
