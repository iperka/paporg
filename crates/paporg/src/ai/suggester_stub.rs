//! Simple pattern-based rule suggester (fallback when "ai" feature is disabled).
//!
//! This provides basic keyword-based suggestions for common document types
//! without requiring the heavy llama-cpp dependency.

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during rule suggestion.
#[derive(Debug, Error)]
pub enum SuggesterError {
    #[error("No matching patterns found in document")]
    NoMatch,

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

    #[error("AI feature not enabled")]
    NotEnabled,
}

/// Common document patterns for keyword matching.
struct DocumentPattern {
    category: &'static str,
    keywords: &'static [&'static str],
    reasoning: &'static str,
    output_directory: &'static str,
}

/// Known document patterns for automatic categorization.
const PATTERNS: &[DocumentPattern] = &[
    DocumentPattern {
        category: "invoices",
        keywords: &[
            "invoice",
            "rechnung",
            "facture",
            "bill",
            "amount due",
            "total due",
            "payment due",
        ],
        reasoning: "Document contains invoice-related keywords",
        output_directory: "$category/$y",
    },
    DocumentPattern {
        category: "receipts",
        keywords: &[
            "receipt",
            "quittung",
            "reçu",
            "transaction",
            "purchase",
            "paid",
            "thank you for your purchase",
        ],
        reasoning: "Document contains receipt-related keywords",
        output_directory: "$category/$y/$m",
    },
    DocumentPattern {
        category: "bank-statements",
        keywords: &[
            "bank statement",
            "kontoauszug",
            "account summary",
            "balance",
            "transactions",
            "deposits",
            "withdrawals",
        ],
        reasoning: "Document contains bank statement keywords",
        output_directory: "$category/$y",
    },
    DocumentPattern {
        category: "contracts",
        keywords: &[
            "contract",
            "vertrag",
            "agreement",
            "terms and conditions",
            "parties agree",
            "hereby agrees",
        ],
        reasoning: "Document contains contract-related keywords",
        output_directory: "$category",
    },
    DocumentPattern {
        category: "insurance",
        keywords: &[
            "insurance",
            "versicherung",
            "policy",
            "coverage",
            "premium",
            "claim",
            "deductible",
        ],
        reasoning: "Document contains insurance-related keywords",
        output_directory: "$category/$y",
    },
    DocumentPattern {
        category: "tax-documents",
        keywords: &[
            "tax",
            "steuer",
            "irs",
            "w-2",
            "1099",
            "tax return",
            "steuererklärung",
        ],
        reasoning: "Document contains tax-related keywords",
        output_directory: "$category/$y",
    },
    DocumentPattern {
        category: "medical",
        keywords: &[
            "medical",
            "health",
            "doctor",
            "hospital",
            "diagnosis",
            "prescription",
            "patient",
        ],
        reasoning: "Document contains medical-related keywords",
        output_directory: "$category/$y",
    },
    DocumentPattern {
        category: "utilities",
        keywords: &[
            "utility",
            "electric",
            "gas",
            "water",
            "internet",
            "phone bill",
            "electricity",
        ],
        reasoning: "Document contains utility bill keywords",
        output_directory: "$category/$y/$m",
    },
];

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
    /// Suggested category name.
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

/// Pattern-based rule suggester (fallback when AI feature is disabled).
/// Uses keyword matching to suggest document categorization rules.
pub struct RuleSuggester;

impl RuleSuggester {
    /// Creates a new pattern-based rule suggester.
    /// The model_path parameter is ignored since this is a pattern-based suggester.
    pub fn new(_model_path: &Path) -> Result<Self, SuggesterError> {
        Ok(Self)
    }

    /// Suggests rules based on OCR text and filename using keyword pattern matching.
    pub fn suggest_rules(
        &self,
        ocr_text: &str,
        filename: &str,
    ) -> Result<Vec<RuleSuggestion>, SuggesterError> {
        self.suggest_rules_with_existing(ocr_text, filename, &[])
    }

    /// Generates a basic commit message from file paths.
    pub fn generate_commit_message(
        &self,
        context: &CommitContext,
    ) -> Result<String, SuggesterError> {
        if context.files.is_empty() {
            return Err(SuggesterError::NoMatch);
        }

        // Determine type from statuses
        let has_new = context.files.iter().any(|(s, _)| s == "A" || s == "?");
        let has_deleted = context.files.iter().any(|(s, _)| s == "D");
        let commit_type = if has_new && !has_deleted {
            "feat"
        } else {
            "chore"
        };

        // Build file list description
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

        Ok(format!("{}: update {}", commit_type, desc))
    }

    /// Suggests rules with existing rules context using keyword pattern matching.
    pub fn suggest_rules_with_existing(
        &self,
        ocr_text: &str,
        filename: &str,
        existing_rules: &[ExistingRule],
    ) -> Result<Vec<RuleSuggestion>, SuggesterError> {
        let text_lower = ocr_text.to_lowercase();
        let filename_lower = filename.to_lowercase();
        let combined = format!("{} {}", text_lower, filename_lower);

        let mut suggestions = Vec::new();

        for pattern in PATTERNS {
            let mut match_count = 0;
            let mut matched_keywords = Vec::new();

            for keyword in pattern.keywords {
                if combined.contains(&keyword.to_lowercase()) {
                    match_count += 1;
                    matched_keywords.push((*keyword).to_string());
                }
            }

            if match_count > 0 {
                // Check if an existing rule already handles this category
                let existing_rule = existing_rules
                    .iter()
                    .find(|r| r.category == pattern.category);

                // Calculate confidence based on number of matching keywords
                let confidence = (match_count as f32 / pattern.keywords.len() as f32).min(0.9);

                if let Some(rule) = existing_rule {
                    // Suggest updating the existing rule with new keywords
                    let new_values: Vec<String> = matched_keywords
                        .iter()
                        .filter(|k| {
                            !rule
                                .match_values
                                .iter()
                                .any(|v| v.to_lowercase() == k.to_lowercase())
                        })
                        .cloned()
                        .collect();

                    if !new_values.is_empty() {
                        suggestions.push(RuleSuggestion {
                            category: pattern.category.to_string(),
                            match_type: "containsAny".to_string(),
                            match_value: serde_json::json!(new_values.clone()),
                            confidence,
                            reasoning: format!(
                                "{}. Found additional keywords that could be added to existing rule '{}'.",
                                pattern.reasoning, rule.name
                            ),
                            output_directory: Some(pattern.output_directory.to_string()),
                            output_filename: None,
                            is_update: true,
                            update_rule_name: Some(rule.name.clone()),
                            add_values: Some(new_values),
                        });
                    }
                } else {
                    // Suggest creating a new rule
                    suggestions.push(RuleSuggestion {
                        category: pattern.category.to_string(),
                        match_type: "containsAny".to_string(),
                        match_value: serde_json::json!(matched_keywords),
                        confidence,
                        reasoning: pattern.reasoning.to_string(),
                        output_directory: Some(pattern.output_directory.to_string()),
                        output_filename: None,
                        is_update: false,
                        update_rule_name: None,
                        add_values: None,
                    });
                }
            }
        }

        // Sort by confidence (highest first)
        suggestions.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if suggestions.is_empty() {
            Err(SuggesterError::NoMatch)
        } else {
            Ok(suggestions)
        }
    }
}

/// Thread-safe wrapper for RuleSuggester.
/// Uses an internal pattern-based suggester that's always available.
pub struct SuggesterPool {
    suggester: RuleSuggester,
}

impl SuggesterPool {
    pub fn new() -> Self {
        Self {
            suggester: RuleSuggester,
        }
    }

    /// Gets or creates a suggester for the given model path.
    /// The model path is ignored since this is a pattern-based suggester.
    pub fn get_or_create(&self, _model_path: &Path) -> Result<(), SuggesterError> {
        Ok(())
    }

    /// Suggests rules using keyword pattern matching.
    pub fn suggest_rules(
        &self,
        ocr_text: &str,
        filename: &str,
    ) -> Result<Vec<RuleSuggestion>, SuggesterError> {
        self.suggester.suggest_rules(ocr_text, filename)
    }

    /// Suggests rules with existing rules context using keyword pattern matching.
    pub fn suggest_rules_with_existing(
        &self,
        ocr_text: &str,
        filename: &str,
        existing_rules: &[ExistingRule],
    ) -> Result<Vec<RuleSuggestion>, SuggesterError> {
        self.suggester
            .suggest_rules_with_existing(ocr_text, filename, existing_rules)
    }

    /// Generates a commit message using the pattern-based suggester.
    pub fn generate_commit_message(
        &self,
        context: &CommitContext,
    ) -> Result<String, SuggesterError> {
        self.suggester.generate_commit_message(context)
    }

    /// Checks if the suggester is initialized (always true for pattern-based suggester).
    pub fn is_initialized(&self) -> bool {
        true
    }
}

impl Default for SuggesterPool {
    fn default() -> Self {
        Self::new()
    }
}
