pub mod loader;
pub mod schema;
pub mod variables;

pub use loader::{load_config, load_config_from_str};
pub use schema::{
    Config, DefaultsConfig, DocumentFormat, DocumentMetadata, ExtractedVariable, MatchCondition,
    OcrConfig, OutputConfig, Rule, SymlinkConfig, VariablesConfig,
};
pub use variables::VariableEngine;
