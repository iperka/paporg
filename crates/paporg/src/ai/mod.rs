//! AI module for embedded LLM inference and rule suggestions.
//!
//! This module is optionally compiled with the "ai" feature flag.
//! When the feature is disabled, stub implementations are provided.

pub mod model_manager;

#[cfg(feature = "ai")]
pub mod suggester;

#[cfg(not(feature = "ai"))]
pub mod suggester_stub;

pub use model_manager::{ModelError, ModelManager};

#[cfg(feature = "ai")]
pub use suggester::{
    CommitContext, ExistingRule, RuleSuggester, RuleSuggestion, SuggesterError, SuggesterPool,
};

#[cfg(not(feature = "ai"))]
pub use suggester_stub::{
    CommitContext, ExistingRule, RuleSuggester, RuleSuggestion, SuggesterError, SuggesterPool,
};
