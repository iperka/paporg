use std::path::PathBuf;

use crate::config::schema::{DefaultsConfig, ExtractedVariable, Rule};
use crate::config::Config;

pub struct PipelineConfig {
    pub input_directory: PathBuf,
    pub output_directory: PathBuf,
    pub ocr_enabled: bool,
    pub ocr_languages: Vec<String>,
    pub ocr_dpi: u32,
    pub rules: Vec<Rule>,
    pub defaults: DefaultsConfig,
    pub extracted_variables: Vec<ExtractedVariable>,
}

impl PipelineConfig {
    pub fn from_config(config: &Config) -> Self {
        Self {
            input_directory: PathBuf::from(&config.input_directory),
            output_directory: PathBuf::from(&config.output_directory),
            ocr_enabled: config.ocr.enabled,
            ocr_languages: config.ocr.languages.clone(),
            ocr_dpi: config.ocr.dpi,
            rules: config.rules.clone(),
            defaults: config.defaults.clone(),
            extracted_variables: config.variables.extracted.clone(),
        }
    }
}
