pub mod ai;
pub mod broadcast;
pub mod categorizer;
pub mod config;
pub mod db;
pub mod email;
pub mod error;
pub mod gitops;
pub mod pipeline;
pub mod processor;
#[cfg(feature = "otel")]
pub mod profiling;
pub mod sanitize;
pub mod secrets;
pub mod storage;
pub mod worker;

pub use ai::{ModelManager, RuleSuggester, RuleSuggestion};
pub use broadcast::{GitProgressBroadcaster, JobProgressBroadcaster, JobStore, LogBroadcaster};
pub use config::{load_config, Config, DocumentFormat, VariableEngine};
pub use error::{ConfigError, PaporgError, ProcessError, Result, StorageError, WorkerError};
pub use gitops::{ConfigLoader, GitOpsError, LoadedConfig};
pub use pipeline::{Pipeline, PipelineConfig, PipelineContext};
pub use secrets::{resolve_secret, resolve_secret_optional, SecretError, TokenEncryptor};
