//! AI model and suggestion commands.

use std::sync::Arc;

#[cfg(feature = "ai")]
use paporg::ai::ModelManager;
use paporg::ai::{CommitContext, RuleSuggester, RuleSuggestion};
use serde::Serialize;
use tauri::State;
use tokio::sync::RwLock;

use super::ApiResponse;
use crate::state::TauriAppState;

// ============================================================================
// Response Types
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiStatusResponse {
    pub available: bool,
    pub model_loaded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub status: String,
    pub progress: Option<f64>,
    pub message: String,
}

/// Response type for rule suggestions that matches frontend expectations.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleSuggestionResponse {
    pub name: String,
    pub category: String,
    pub pattern: String,
    pub confidence: f64,
    pub explanation: String,
}

impl From<RuleSuggestion> for RuleSuggestionResponse {
    fn from(s: RuleSuggestion) -> Self {
        // Convert match_value to a pattern string
        let pattern = match &s.match_value {
            serde_json::Value::Array(arr) => {
                // For containsAny, join keywords
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join("|")
            }
            serde_json::Value::String(s) => s.clone(),
            _ => s.match_value.to_string(),
        };

        // Generate a name from category
        let name = format!("{}-rule", s.category);

        Self {
            name,
            category: s.category,
            pattern,
            confidence: s.confidence as f64,
            explanation: s.reasoning,
        }
    }
}

// ============================================================================
// Commands
// ============================================================================

/// Get AI model status.
#[tauri::command]
pub async fn ai_status(
    _state: State<'_, Arc<RwLock<TauriAppState>>>,
) -> Result<ApiResponse<AiStatusResponse>, String> {
    // Check if AI feature is available
    #[cfg(feature = "ai")]
    {
        // Try to get model status
        match ModelManager::new() {
            Ok(manager) => {
                let model_name = manager.current_model().map(|s| s.to_string());
                Ok(ApiResponse::ok(AiStatusResponse {
                    available: true,
                    model_loaded: model_name.is_some(),
                    model_name,
                    error: None,
                }))
            }
            Err(e) => Ok(ApiResponse::ok(AiStatusResponse {
                available: true,
                model_loaded: false,
                model_name: None,
                error: Some(e.to_string()),
            })),
        }
    }

    #[cfg(not(feature = "ai"))]
    {
        // Pattern-based suggestions are always available
        Ok(ApiResponse::ok(AiStatusResponse {
            available: true,
            model_loaded: true, // Pattern matcher is always "loaded"
            model_name: Some("pattern-matcher".to_string()),
            error: None,
        }))
    }
}

/// Download AI model.
#[tauri::command]
pub async fn download_ai_model(
    _state: State<'_, Arc<RwLock<TauriAppState>>>,
    model_id: String,
) -> Result<ApiResponse<DownloadProgress>, String> {
    #[cfg(feature = "ai")]
    {
        match ModelManager::new() {
            Ok(manager) => {
                // Start download (this is a blocking operation in the current implementation)
                match manager.download_model(&model_id) {
                    Ok(()) => Ok(ApiResponse::ok(DownloadProgress {
                        status: "completed".to_string(),
                        progress: Some(100.0),
                        message: format!("Model {} downloaded successfully", model_id),
                    })),
                    Err(e) => Ok(ApiResponse::ok(DownloadProgress {
                        status: "failed".to_string(),
                        progress: None,
                        message: e.to_string(),
                    })),
                }
            }
            Err(e) => Ok(ApiResponse::err(e.to_string())),
        }
    }

    #[cfg(not(feature = "ai"))]
    {
        let _ = model_id;
        // Pattern-based suggestions don't require model download
        Ok(ApiResponse::ok(DownloadProgress {
            status: "completed".to_string(),
            progress: Some(100.0),
            message: "Pattern-based suggestions are available (no model download required)"
                .to_string(),
        }))
    }
}

/// Get rule suggestion for document text.
#[tauri::command]
pub async fn suggest_rule(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    ocr_text: String,
    filename: String,
) -> Result<ApiResponse<RuleSuggestionResponse>, String> {
    #[cfg(feature = "ai")]
    {
        let state = state.read().await;

        // Get existing rules for context
        let existing_rules: Vec<String> = state
            .config()
            .map(|c| {
                c.rules
                    .iter()
                    .filter_map(|r| serde_yaml::to_string(&r.resource).ok())
                    .collect()
            })
            .unwrap_or_default();

        drop(state);

        // Create suggester and generate suggestion
        match RuleSuggester::new() {
            Ok(suggester) => match suggester.suggest(&ocr_text, &filename, &existing_rules) {
                Ok(suggestion) => Ok(ApiResponse::ok(suggestion.into())),
                Err(e) => Ok(ApiResponse::err(e.to_string())),
            },
            Err(e) => Ok(ApiResponse::err(e.to_string())),
        }
    }

    #[cfg(not(feature = "ai"))]
    {
        use std::path::PathBuf;

        let _ = state;

        // Use pattern-based suggester
        let suggester = RuleSuggester::new(&PathBuf::new()).map_err(|e| e.to_string())?;

        match suggester.suggest_rules(&ocr_text, &filename) {
            Ok(suggestions) => {
                if let Some(suggestion) = suggestions.into_iter().next() {
                    Ok(ApiResponse::ok(suggestion.into()))
                } else {
                    Ok(ApiResponse::err("No matching patterns found in document"))
                }
            }
            Err(e) => Ok(ApiResponse::err(e.to_string())),
        }
    }
}

/// Generate a commit message from changed files and diff.
#[tauri::command]
pub async fn suggest_commit_message(
    _state: State<'_, Arc<RwLock<TauriAppState>>>,
    files: Vec<(String, String)>,
    diff: String,
) -> Result<ApiResponse<String>, String> {
    #[cfg(feature = "ai")]
    {
        use paporg::ai::SuggesterPool;

        let context = CommitContext { files, diff };

        // Try to use the LLM-backed suggester if a model is available
        match ModelManager::new() {
            Ok(manager) if manager.is_model_available() => {
                let pool = SuggesterPool::new();
                if let Err(e) = pool.get_or_create(&manager.model_path()) {
                    log::warn!("Failed to init suggester, falling back: {}", e);
                    return Ok(ApiResponse::ok(generate_fallback_commit_message(&context)));
                }
                match pool.generate_commit_message(&context) {
                    Ok(msg) => Ok(ApiResponse::ok(msg)),
                    Err(e) => {
                        log::warn!("AI commit suggestion failed, falling back: {}", e);
                        Ok(ApiResponse::ok(generate_fallback_commit_message(&context)))
                    }
                }
            }
            _ => Ok(ApiResponse::ok(generate_fallback_commit_message(&context))),
        }
    }

    #[cfg(not(feature = "ai"))]
    {
        let context = CommitContext { files, diff };
        let suggester =
            RuleSuggester::new(&std::path::PathBuf::new()).map_err(|e| e.to_string())?;
        match suggester.generate_commit_message(&context) {
            Ok(msg) => Ok(ApiResponse::ok(msg)),
            Err(e) => Ok(ApiResponse::err(e.to_string())),
        }
    }
}

/// Fallback commit message generation without AI.
#[cfg(feature = "ai")]
fn generate_fallback_commit_message(context: &CommitContext) -> String {
    let has_new = context.files.iter().any(|(s, _)| s == "A" || s == "?");
    let has_deleted = context.files.iter().any(|(s, _)| s == "D");
    let commit_type = if has_new && !has_deleted {
        "feat"
    } else {
        "chore"
    };

    let filenames: Vec<&str> = context
        .files
        .iter()
        .map(|(_, path)| path.rsplit('/').next().unwrap_or(path.as_str()))
        .collect();

    let desc = if filenames.len() <= 3 {
        filenames.join(", ")
    } else {
        let first_three = filenames[..3].join(", ");
        format!("{} and {} more", first_three, filenames.len() - 3)
    };

    format!("{}: update {}", commit_type, desc)
}
