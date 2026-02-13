//! GitOps configuration system for paporg.
//!
//! This module provides a Kubernetes-style configuration system with:
//! - Multi-file YAML configurations
//! - Three resource kinds: Settings, Variable, Rule
//! - File system watching for real-time updates
//! - Git integration for version control
//! - Cross-resource validation

pub mod error;
pub mod git;
pub mod loader;
pub mod progress;
pub mod resource;
pub mod validation;
pub mod watcher;

pub use error::{GitOpsError, Result};
pub use loader::FileTreeNode;
pub use loader::{ConfigLoader, LoadedConfig};
pub use resource::{
    AnyResource, CompoundMatch, FileFilters, GitAuthSettings, GitAuthType, GitSettings,
    ImportSourceResource, ImportSourceSpec, ImportSourceType, LocalSourceConfig, MatchCondition,
    ObjectMeta, OcrSettings, OutputSettings, Resource, ResourceKind, ResourceWithPath,
    RuleResource, RuleSpec, SettingsResource, SettingsSpec, SimpleMatch, SymlinkSettings,
    VariableResource, VariableSpec, VariableTransform, API_VERSION,
};
pub use validation::ConfigValidator;
pub use watcher::{ConfigChangeEvent, ConfigWatcher};
