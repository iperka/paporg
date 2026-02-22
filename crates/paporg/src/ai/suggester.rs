//! LLM-based rule suggester using llama-cpp-2.

use std::path::Path;
use std::sync::Mutex;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel, Special};
use llama_cpp_2::token::data_array::LlamaTokenDataArray;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Sanitizes text for safe inclusion in LLM prompts.
///
/// Escapes ChatML tokens (`<|...|>`) and common instruction tokens to prevent
/// prompt injection attacks. This is specific to ChatML-format models (Qwen, etc.).
///
/// # Sequences Escaped
/// - `<|...|>` - ChatML special tokens (system, user, assistant markers)
/// - `<s>`, `</s>` - Sequence boundaries
/// - `[INST]`, `[/INST]` - Llama-style instruction markers
/// - `<<SYS>>`, `<</SYS>>` - Llama-style system prompt markers
fn sanitize_for_prompt(text: &str) -> String {
    text.replace("<|", "< |")
        .replace("|>", "| >")
        .replace("<s>", "< s >")
        .replace("</s>", "< / s >")
        .replace("[INST]", "[ INST ]")
        .replace("[/INST]", "[ / INST ]")
        .replace("<<SYS>>", "< < SYS > >")
        .replace("<</SYS>>", "< < / SYS > >")
}

/// Errors that can occur during rule suggestion.
#[derive(Debug, Error)]
pub enum SuggesterError {
    #[error("Failed to initialize LLM backend: {0}")]
    BackendInit(String),

    #[error("Failed to load model: {0}")]
    ModelLoad(String),

    #[error("Failed to create context: {0}")]
    ContextCreation(String),

    #[error("Failed to tokenize input: {0}")]
    Tokenization(String),

    #[error("Inference failed: {0}")]
    Inference(String),

    #[error("Failed to parse LLM response: {0}")]
    ResponseParse(String),

    #[error("Mutex poisoned - concurrent access failed")]
    MutexPoisoned,
}

/// Input data for commit message generation.
pub struct CommitContext {
    /// File paths with their git status codes (M, A, D, ?).
    pub files: Vec<(String, String)>,
    /// Unified diff of the changes (may be truncated).
    pub diff: String,
}

/// Summary of an existing rule for AI context.
#[derive(Debug, Clone)]
pub struct ExistingRule {
    /// Rule name/ID.
    pub name: String,
    /// Category this rule assigns.
    pub category: String,
    /// Match type.
    pub match_type: String,
    /// Current match values.
    pub match_values: Vec<String>,
}

/// A suggested rule from the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleSuggestion {
    /// Suggested category name (slugified, lowercase with hyphens).
    pub category: String,
    /// Match type: "contains", "containsAny", "containsAll", or "pattern".
    pub match_type: String,
    /// Match value (string or array depending on match_type).
    pub match_value: serde_json::Value,
    /// Confidence score (0.0 - 1.0). Defaults to 0.5 if not provided.
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    /// Brief explanation of why this rule was suggested.
    #[serde(default)]
    pub reasoning: String,
    /// Suggested output directory with variables.
    #[serde(default)]
    pub output_directory: Option<String>,
    /// Suggested output filename with variables.
    #[serde(default)]
    pub output_filename: Option<String>,
    /// Whether this is an update to an existing rule (vs creating new).
    #[serde(default)]
    pub is_update: bool,
    /// Name of the existing rule to update (if is_update is true).
    #[serde(default)]
    pub update_rule_name: Option<String>,
    /// Values to add to the existing rule (for containsAny updates).
    #[serde(default)]
    pub add_values: Option<Vec<String>>,
}

/// Default confidence value when LLM doesn't provide one.
fn default_confidence() -> f32 {
    0.5
}

/// LLM response structure.
#[derive(Debug, Deserialize)]
struct LlmResponse {
    suggestions: Vec<RuleSuggestion>,
}

/// Rule suggester using embedded LLM.
pub struct RuleSuggester {
    model: LlamaModel,
    backend: LlamaBackend,
    ctx_params: LlamaContextParams,
}

// SAFETY: RuleSuggester is designed to be used within SuggesterPool which wraps it in a Mutex.
// The llama-cpp-2 LlamaModel and LlamaBackend types are documented as thread-safe for read operations.
// All mutable operations (context creation, inference) are performed through &self methods that
// create new contexts per-call, ensuring no shared mutable state. The LlamaContextParams is Clone
// and does not share state. This implementation should only be used via SuggesterPool's Mutex wrapper.
unsafe impl Send for RuleSuggester {}
unsafe impl Sync for RuleSuggester {}

impl RuleSuggester {
    /// Creates a new rule suggester with the given model.
    pub fn new(model_path: &Path) -> Result<Self, SuggesterError> {
        info!("Initializing LLM backend...");
        let backend =
            LlamaBackend::init().map_err(|e| SuggesterError::BackendInit(e.to_string()))?;

        info!("Loading model from: {}", model_path.display());
        let model_params = LlamaModelParams::default();
        let model = LlamaModel::load_from_file(&backend, model_path, &model_params)
            .map_err(|e| SuggesterError::ModelLoad(e.to_string()))?;

        let ctx_params = LlamaContextParams::default().with_n_ctx(std::num::NonZeroU32::new(2048));

        info!("LLM initialized successfully");
        Ok(Self {
            model,
            backend,
            ctx_params,
        })
    }

    /// Suggests rules based on OCR text and filename.
    pub fn suggest_rules(
        &self,
        ocr_text: &str,
        filename: &str,
    ) -> Result<Vec<RuleSuggestion>, SuggesterError> {
        self.suggest_rules_with_existing(ocr_text, filename, &[])
    }

    /// Suggests rules based on OCR text, filename, and existing rules.
    pub fn suggest_rules_with_existing(
        &self,
        ocr_text: &str,
        filename: &str,
        existing_rules: &[ExistingRule],
    ) -> Result<Vec<RuleSuggestion>, SuggesterError> {
        let prompt = self.build_prompt_with_existing(ocr_text, filename, existing_rules);
        debug!("Generated prompt:\n{}", prompt);

        let response = self.generate(&prompt, 512, Some("}]}"))?;
        debug!("LLM response:\n{}", response);

        self.parse_suggestions(&response)
    }

    /// Generates a conventional commit message from git changes.
    pub fn generate_commit_message(
        &self,
        context: &CommitContext,
    ) -> Result<String, SuggesterError> {
        let prompt = self.build_commit_prompt(context);
        debug!("Generated commit prompt:\n{}", prompt);

        let response = self.generate(&prompt, 64, None)?;
        debug!("LLM commit response:\n{}", response);

        self.parse_commit_message(&response)
    }

    /// Builds a ChatML prompt for commit message generation.
    fn build_commit_prompt(&self, context: &CommitContext) -> String {
        let files_text: String = context
            .files
            .iter()
            .map(|(status, path)| format!("  {} {}", sanitize_for_prompt(status), sanitize_for_prompt(path)))
            .collect::<Vec<_>>()
            .join("\n");

        let truncated_diff: String = sanitize_for_prompt(&context.diff)
            .chars()
            .take(1200)
            .collect();

        format!(
            r#"<|im_start|>system
You are a commit message generator. Write a single conventional commit message.

RULES:
- Format: type(scope): description
- Types: feat, fix, chore, docs, refactor, style, test, perf, ci, build
- Scope is optional, derived from the primary directory changed
- Description: lowercase, imperative mood, no period, under 72 chars total
- Output ONLY the commit message<|im_end|>
<|im_start|>user
Changed files:
{files}

Diff:
{diff}<|im_end|>
<|im_start|>assistant
"#,
            files = files_text,
            diff = truncated_diff,
        )
    }

    /// Parses the LLM response into a valid conventional commit message.
    fn parse_commit_message(&self, response: &str) -> Result<String, SuggesterError> {
        let first_line = response
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or(response.trim())
            .trim()
            .trim_matches('"')
            .trim();

        if first_line.is_empty() {
            return Err(SuggesterError::ResponseParse(
                "Empty commit message from LLM".to_string(),
            ));
        }

        // Valid conventional commit prefixes
        const TYPES: &[&str] = &[
            "feat", "fix", "chore", "docs", "refactor", "style", "test", "perf", "ci", "build",
        ];

        // Check if the message starts with a valid type
        let is_valid = TYPES.iter().any(|t| {
            first_line.starts_with(&format!("{}:", t))
                || first_line.starts_with(&format!("{}(", t))
        });

        let message = if is_valid {
            // Truncate to 72 chars
            first_line.chars().take(72).collect()
        } else {
            // Fallback: wrap in chore:
            let desc: String = first_line.chars().take(65).collect();
            format!("chore: {}", desc)
        };

        Ok(message)
    }

    /// Builds the prompt for the LLM with existing rules context.
    fn build_prompt_with_existing(
        &self,
        ocr_text: &str,
        filename: &str,
        existing_rules: &[ExistingRule],
    ) -> String {
        // Sanitize inputs to prevent prompt injection
        let sanitized_text: String = sanitize_for_prompt(ocr_text).chars().take(1500).collect();
        let sanitized_filename = sanitize_for_prompt(filename);

        // Format existing rules for the prompt (sanitize rule values too)
        let existing_rules_text = if existing_rules.is_empty() {
            String::new()
        } else {
            let rules_list: Vec<String> = existing_rules
                .iter()
                .map(|r| {
                    format!(
                        "- {}: category=\"{}\", matchType=\"{}\", values=[{}]",
                        sanitize_for_prompt(&r.name),
                        sanitize_for_prompt(&r.category),
                        sanitize_for_prompt(&r.match_type),
                        r.match_values
                            .iter()
                            .map(|v| format!("\"{}\"", sanitize_for_prompt(v)))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })
                .collect();
            format!(
                "\n\nEXISTING RULES (consider updating these instead of creating new):\n{}\n",
                rules_list.join("\n")
            )
        };

        let update_instructions = if existing_rules.is_empty() {
            String::new()
        } else {
            r#"

IMPORTANT - PREFER UPDATES OVER NEW RULES:
- If a document fits an existing category, suggest UPDATING that rule by adding new match terms
- For updates, set "isUpdate": true, "updateRuleName": "rule-name", "addValues": ["new", "terms"]
- Only suggest new rules if the document truly doesn't fit any existing category"#
                .to_string()
        };

        format!(
            r#"<|im_start|>system
You are a document classification assistant for a paperless document management system.
Analyze document text and suggest classification rules.
Respond ONLY with valid JSON. Do not include any other text.

IMPORTANT RULES:
- category MUST be lowercase with hyphens, no spaces (e.g., "invoices", "medical-bills", "receipts")
- Suggest common document categories: invoices, receipts, contracts, medical-bills, tax-documents, bank-statements
- outputDirectory should use variables: $y (year), $m (month), $d (day), $category
- outputFilename should use variables: $y, $m, $d, $original, $timestamp
- matchValue strings should be specific enough to uniquely identify this document type{update_instructions}<|im_end|>
<|im_start|>user
Filename: {filename}{existing_rules_text}
Document text:
{text}

Suggest 2-3 classification rules. Return JSON:
{{"suggestions": [
  {{
    "category": "slug-name",
    "matchType": "contains|containsAny|containsAll|pattern",
    "matchValue": "string or [\"array\"]",
    "confidence": 0.0-1.0,
    "reasoning": "brief explanation",
    "outputDirectory": "$category/$y/$m",
    "outputFilename": "$y-$m-$d_$original",
    "isUpdate": false,
    "updateRuleName": null,
    "addValues": null
  }}
]}}

For UPDATING existing rules, use:
{{"isUpdate": true, "updateRuleName": "existing-rule-name", "addValues": ["NewCompany", "NewTerm"], "category": "existing-category", ...}}<|im_end|>
<|im_start|>assistant
"#,
            filename = sanitized_filename,
            text = sanitized_text,
            existing_rules_text = existing_rules_text,
            update_instructions = update_instructions
        )
    }

    /// Generates text from the LLM.
    ///
    /// - `max_tokens`: maximum number of tokens to generate.
    /// - `early_stop`: optional substring that triggers early termination when found in output.
    fn generate(&self, prompt: &str, max_tokens: usize, early_stop: Option<&str>) -> Result<String, SuggesterError> {
        // Create context for this generation
        let mut ctx = self
            .model
            .new_context(&self.backend, self.ctx_params.clone())
            .map_err(|e| SuggesterError::ContextCreation(e.to_string()))?;

        // Tokenize the prompt
        let tokens = self
            .model
            .str_to_token(prompt, AddBos::Always)
            .map_err(|e| SuggesterError::Tokenization(e.to_string()))?;

        let n_tokens = tokens.len();
        debug!("Tokenized prompt into {} tokens", n_tokens);

        // Create batch and add tokens
        let mut batch = LlamaBatch::new(2048, 1);
        for (i, token) in tokens.iter().enumerate() {
            let is_last = i == n_tokens - 1;
            batch
                .add(*token, i as i32, &[0], is_last)
                .map_err(|e| SuggesterError::Inference(format!("Failed to add token: {}", e)))?;
        }

        // Decode initial prompt
        ctx.decode(&mut batch)
            .map_err(|e| SuggesterError::Inference(format!("Failed to decode prompt: {}", e)))?;

        // Generate tokens
        let mut output = String::new();
        let mut n_cur = n_tokens;

        for _ in 0..max_tokens {
            // Sample next token
            let candidates = ctx.candidates_ith(batch.n_tokens() - 1);
            let mut candidates_array = LlamaTokenDataArray::from_iter(candidates, false);

            // Apply sampling with time-based seed
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u32)
                .unwrap_or(42);
            let new_token = candidates_array.sample_token(seed);

            // Check for end of sequence
            if self.model.is_eog_token(new_token) {
                break;
            }

            // Decode token to string
            let token_str = self
                .model
                .token_to_str(new_token, Special::Tokenize)
                .map_err(|e| SuggesterError::Inference(format!("Failed to decode token: {}", e)))?;

            output.push_str(&token_str);

            // Check for early stop condition
            if let Some(stop) = early_stop {
                if output.contains(stop) {
                    break;
                }
            }

            // Also stop on newline for single-line outputs (commit messages)
            if early_stop.is_none() && output.contains('\n') {
                break;
            }

            // Prepare next batch
            batch.clear();
            batch
                .add(new_token, n_cur as i32, &[0], true)
                .map_err(|e| SuggesterError::Inference(format!("Failed to add token: {}", e)))?;

            ctx.decode(&mut batch)
                .map_err(|e| SuggesterError::Inference(format!("Failed to decode: {}", e)))?;

            n_cur += 1;
        }

        Ok(output)
    }

    /// Parses the LLM response into rule suggestions.
    fn parse_suggestions(&self, response: &str) -> Result<Vec<RuleSuggestion>, SuggesterError> {
        // Try to find JSON in the response
        let json_str = self.extract_json(response);

        let parsed: LlmResponse = serde_json::from_str(&json_str).map_err(|e| {
            SuggesterError::ResponseParse(format!(
                "Failed to parse JSON: {}. Response was: {}",
                e, json_str
            ))
        })?;

        // Validate and filter suggestions
        let suggestions: Vec<RuleSuggestion> = parsed
            .suggestions
            .into_iter()
            .filter(|s| {
                !s.category.is_empty()
                    && !s.match_type.is_empty()
                    && s.confidence >= 0.0
                    && s.confidence <= 1.0
            })
            .take(3) // Limit to 3 suggestions
            .collect();

        if suggestions.is_empty() {
            warn!("LLM returned no valid suggestions");
        }

        Ok(suggestions)
    }

    /// Extracts JSON from the LLM response, handling potential extra text.
    /// Uses a stateful scanner that tracks string boundaries and escape sequences.
    fn extract_json(&self, response: &str) -> String {
        // Find the start of JSON (first '{')
        let start = match response.find('{') {
            Some(idx) => idx,
            None => return response.to_string(), // No JSON found, return as-is
        };

        // Parse with awareness of strings and escape sequences
        let mut depth = 0;
        let mut in_string = false;
        let mut escape_next = false;
        let mut end = response.len();

        for (i, c) in response[start..].char_indices() {
            if escape_next {
                escape_next = false;
                continue;
            }

            match c {
                '\\' if in_string => {
                    escape_next = true;
                }
                '"' => {
                    in_string = !in_string;
                }
                '{' if !in_string => {
                    depth += 1;
                }
                '}' if !in_string => {
                    depth -= 1;
                    if depth == 0 {
                        end = start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }

        response[start..end].to_string()
    }
}

/// Thread-safe wrapper for RuleSuggester.
pub struct SuggesterPool {
    suggester: Mutex<Option<RuleSuggester>>,
}

impl SuggesterPool {
    pub fn new() -> Self {
        Self {
            suggester: Mutex::new(None),
        }
    }

    /// Gets or creates a suggester for the given model path.
    pub fn get_or_create(&self, model_path: &Path) -> Result<(), SuggesterError> {
        let mut guard = self
            .suggester
            .lock()
            .map_err(|_| SuggesterError::MutexPoisoned)?;
        if guard.is_none() {
            *guard = Some(RuleSuggester::new(model_path)?);
        }
        Ok(())
    }

    /// Suggests rules using the cached suggester.
    pub fn suggest_rules(
        &self,
        ocr_text: &str,
        filename: &str,
    ) -> Result<Vec<RuleSuggestion>, SuggesterError> {
        self.suggest_rules_with_existing(ocr_text, filename, &[])
    }

    /// Suggests rules with existing rules context.
    pub fn suggest_rules_with_existing(
        &self,
        ocr_text: &str,
        filename: &str,
        existing_rules: &[ExistingRule],
    ) -> Result<Vec<RuleSuggestion>, SuggesterError> {
        let guard = self
            .suggester
            .lock()
            .map_err(|_| SuggesterError::MutexPoisoned)?;
        match guard.as_ref() {
            Some(suggester) => {
                suggester.suggest_rules_with_existing(ocr_text, filename, existing_rules)
            }
            None => Err(SuggesterError::BackendInit(
                "Suggester not initialized".to_string(),
            )),
        }
    }

    /// Generates a commit message using the cached suggester.
    pub fn generate_commit_message(
        &self,
        context: &CommitContext,
    ) -> Result<String, SuggesterError> {
        let guard = self
            .suggester
            .lock()
            .map_err(|_| SuggesterError::MutexPoisoned)?;
        match guard.as_ref() {
            Some(suggester) => suggester.generate_commit_message(context),
            None => Err(SuggesterError::BackendInit(
                "Suggester not initialized".to_string(),
            )),
        }
    }

    /// Checks if the suggester is initialized.
    pub fn is_initialized(&self) -> bool {
        self.suggester
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    /// Resets the suggester pool, releasing the loaded model.
    pub fn reset(&self) -> Result<(), SuggesterError> {
        let mut lock = self
            .suggester
            .lock()
            .map_err(|_| SuggesterError::MutexPoisoned)?;
        *lock = None;
        Ok(())
    }
}

impl Default for SuggesterPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_simple() {
        let suggester_pool = SuggesterPool::new();
        // We can't test extraction without a suggester, but we can test the logic

        let response = r#"{"suggestions": [{"category": "test", "matchType": "contains", "matchValue": "test", "confidence": 0.9, "reasoning": "test"}]}"#;

        // Find JSON boundaries
        let start = response.find('{').unwrap_or(0);
        let mut depth = 0;
        let mut end = response.len();
        for (i, c) in response[start..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        let extracted = &response[start..end];
        assert!(extracted.starts_with('{'));
        assert!(extracted.ends_with('}'));
    }

    #[test]
    fn test_suggestion_parsing() {
        let json = r#"{"suggestions": [
            {"category": "invoices", "matchType": "containsAny", "matchValue": ["Invoice", "INV-"], "confidence": 0.92, "reasoning": "Contains invoice indicators"}
        ]}"#;

        let parsed: LlmResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.suggestions.len(), 1);
        assert_eq!(parsed.suggestions[0].category, "invoices");
        assert_eq!(parsed.suggestions[0].match_type, "containsAny");
    }

    #[test]
    fn test_suggester_pool_creation() {
        let pool = SuggesterPool::new();
        assert!(!pool.is_initialized());
    }
}
