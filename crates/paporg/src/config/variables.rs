use std::collections::HashMap;

use chrono::{Datelike, Utc};
use regex::Regex;

use crate::config::schema::{ExtractedVariable, VariableTransform};

pub struct VariableEngine {
    extracted_patterns: Vec<CompiledPattern>,
}

struct CompiledPattern {
    name: String,
    regex: Regex,
    transform: Option<VariableTransform>,
    default: Option<String>,
}

impl VariableEngine {
    pub fn new(extracted: &[ExtractedVariable]) -> Self {
        let extracted_patterns = extracted
            .iter()
            .filter_map(|var| {
                Regex::new(&var.pattern).ok().map(|regex| CompiledPattern {
                    name: var.name.clone(),
                    regex,
                    transform: var.transform.clone(),
                    default: var.default.clone(),
                })
            })
            .collect();

        Self { extracted_patterns }
    }

    pub fn extract_variables(&self, text: &str) -> HashMap<String, String> {
        let mut variables = HashMap::new();

        for pattern in &self.extracted_patterns {
            if let Some(caps) = pattern.regex.captures(text) {
                if let Some(matched) = caps.name(&pattern.name) {
                    let mut value = matched.as_str().to_string();

                    if let Some(transform) = &pattern.transform {
                        value = apply_transform(&value, transform);
                    }

                    variables.insert(pattern.name.clone(), value);
                }
            } else if let Some(default) = &pattern.default {
                variables.insert(pattern.name.clone(), default.clone());
            }
        }

        variables
    }

    pub fn substitute(
        &self,
        template: &str,
        original_filename: &str,
        extracted: &HashMap<String, String>,
    ) -> String {
        let now = Utc::now();
        let mut result = template.to_string();

        // Built-in variables
        let builtins = self.get_builtin_variables(original_filename, &now);

        // First substitute built-in variables
        for (name, value) in &builtins {
            let pattern = format!("${}", name);
            result = result.replace(&pattern, value);
        }

        // Then substitute extracted variables
        for (name, value) in extracted {
            let pattern = format!("${}", name);
            result = result.replace(&pattern, value);
        }

        // Sanitize for filesystem
        sanitize_filename(&result)
    }

    fn get_builtin_variables(
        &self,
        original_filename: &str,
        now: &chrono::DateTime<Utc>,
    ) -> HashMap<String, String> {
        let mut vars = HashMap::new();

        vars.insert("y".to_string(), format!("{:04}", now.year()));
        vars.insert("m".to_string(), format!("{:02}", now.month()));
        vars.insert("d".to_string(), format!("{:02}", now.day()));
        vars.insert("timestamp".to_string(), now.timestamp().to_string());
        vars.insert("uuid".to_string(), uuid::Uuid::new_v4().to_string());

        // Original filename without extension
        let original = std::path::Path::new(original_filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(original_filename);
        vars.insert("original".to_string(), original.to_string());

        vars
    }
}

fn apply_transform(value: &str, transform: &VariableTransform) -> String {
    match transform {
        VariableTransform::Slugify => slugify(value),
        VariableTransform::Uppercase => value.to_uppercase(),
        VariableTransform::Lowercase => value.to_lowercase(),
        VariableTransform::Trim => value.trim().to_string(),
    }
}

fn slugify(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_variables() {
        let extracted = vec![ExtractedVariable {
            name: "vendor".to_string(),
            pattern: r"(?i)from[:\s]+(?P<vendor>[A-Za-z]+)".to_string(),
            transform: None,
            default: None,
        }];

        let engine = VariableEngine::new(&extracted);
        let vars = engine.extract_variables("Invoice from Acme Corporation");

        assert_eq!(vars.get("vendor"), Some(&"Acme".to_string()));
    }

    #[test]
    fn test_extract_variables_with_transform() {
        let extracted = vec![ExtractedVariable {
            name: "vendor".to_string(),
            pattern: r"(?i)from[:\s]+(?P<vendor>[A-Za-z\s]+?)(?:\s+Corporation|\s*$)".to_string(),
            transform: Some(VariableTransform::Slugify),
            default: None,
        }];

        let engine = VariableEngine::new(&extracted);
        let vars = engine.extract_variables("Invoice from Acme Corp Inc");

        assert_eq!(vars.get("vendor"), Some(&"acme-corp-inc".to_string()));
    }

    #[test]
    fn test_extract_variables_with_default() {
        let extracted = vec![ExtractedVariable {
            name: "vendor".to_string(),
            pattern: r"(?i)from[:\s]+(?P<vendor>[A-Za-z]+)".to_string(),
            transform: None,
            default: Some("unknown".to_string()),
        }];

        let engine = VariableEngine::new(&extracted);
        let vars = engine.extract_variables("Some text without vendor");

        assert_eq!(vars.get("vendor"), Some(&"unknown".to_string()));
    }

    #[test]
    fn test_substitute_builtin_variables() {
        let engine = VariableEngine::new(&[]);
        let extracted = HashMap::new();

        let result = engine.substitute("$y/$m/$d/$original", "test.pdf", &extracted);

        // Check that year, month, day are substituted (they should be current date)
        assert!(!result.contains("$y"));
        assert!(!result.contains("$m"));
        assert!(!result.contains("$d"));
        assert!(result.contains("test"));
    }

    #[test]
    fn test_substitute_extracted_variables() {
        let engine = VariableEngine::new(&[]);
        let mut extracted = HashMap::new();
        extracted.insert("vendor".to_string(), "acme".to_string());

        let result = engine.substitute("$y/invoices/$vendor", "test.pdf", &extracted);

        assert!(result.contains("acme"));
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("  Multiple   Spaces  "), "multiple-spaces");
        assert_eq!(slugify("Special@#$Characters"), "special-characters");
        assert_eq!(slugify("Already-slugified"), "already-slugified");
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(
            sanitize_filename("normal_file-name.pdf"),
            "normal_file-name.pdf"
        );
        assert_eq!(sanitize_filename("file with spaces"), "file_with_spaces");
        assert_eq!(sanitize_filename("file/with\\slashes"), "file_with_slashes");
        assert_eq!(
            sanitize_filename("__leading_underscores__"),
            "leading_underscores"
        );
    }

    #[test]
    fn test_apply_transforms() {
        assert_eq!(
            apply_transform("Hello World", &VariableTransform::Uppercase),
            "HELLO WORLD"
        );
        assert_eq!(
            apply_transform("Hello World", &VariableTransform::Lowercase),
            "hello world"
        );
        assert_eq!(
            apply_transform("  trimmed  ", &VariableTransform::Trim),
            "trimmed"
        );
        assert_eq!(
            apply_transform("Hello World", &VariableTransform::Slugify),
            "hello-world"
        );
    }

    #[test]
    fn test_unicode_in_extracted_text() {
        let extracted = vec![ExtractedVariable {
            name: "vendor".to_string(),
            pattern: r"(?P<vendor>\p{L}+)".to_string(), // Unicode letter
            transform: None,
            default: None,
        }];

        let engine = VariableEngine::new(&extracted);
        let vars = engine.extract_variables("Invoice from Müller");

        // Should extract unicode characters
        assert!(vars.contains_key("vendor"));
    }

    #[test]
    fn test_empty_capture_group() {
        let extracted = vec![ExtractedVariable {
            name: "optional".to_string(),
            pattern: r"prefix(?P<optional>.*?)suffix".to_string(),
            transform: None,
            default: None,
        }];

        let engine = VariableEngine::new(&extracted);
        let vars = engine.extract_variables("prefixsuffix");

        // Empty capture should still be inserted
        assert_eq!(vars.get("optional"), Some(&"".to_string()));
    }

    #[test]
    fn test_special_chars_in_filename() {
        let engine = VariableEngine::new(&[]);
        let extracted = HashMap::new();

        let result = engine.substitute("$original", "test<>:\"|?*.pdf", &extracted);

        // Special characters should be sanitized
        assert!(!result.contains('<'));
        assert!(!result.contains('>'));
        assert!(!result.contains(':'));
        assert!(!result.contains('"'));
        assert!(!result.contains('|'));
        assert!(!result.contains('?'));
        assert!(!result.contains('*'));
    }

    #[test]
    fn test_very_long_filename_sanitization() {
        let engine = VariableEngine::new(&[]);
        let extracted = HashMap::new();

        let long_name = "a".repeat(500);
        let result = engine.substitute("$original", &format!("{}.pdf", long_name), &extracted);

        // Should produce a valid filename (no crashes)
        assert!(!result.is_empty());
    }

    #[test]
    fn test_slugify_unicode() {
        // Slugify should handle unicode gracefully
        assert!(!slugify("Hëllö Wörld").is_empty());
        assert!(!slugify("日本語").is_empty());
    }

    #[test]
    fn test_slugify_multiple_consecutive_special_chars() {
        assert_eq!(slugify("hello---world"), "hello-world");
        assert_eq!(slugify("hello___world"), "hello-world");
        assert_eq!(slugify("hello   world"), "hello-world");
    }

    #[test]
    fn test_substitute_uuid_uniqueness() {
        let engine = VariableEngine::new(&[]);
        let extracted = HashMap::new();

        let result1 = engine.substitute("$uuid", "test.pdf", &extracted);
        let result2 = engine.substitute("$uuid", "test.pdf", &extracted);

        // Each call should generate a different UUID
        assert_ne!(result1, result2);
    }

    #[test]
    fn test_substitute_timestamp_format() {
        let engine = VariableEngine::new(&[]);
        let extracted = HashMap::new();

        let result = engine.substitute("$timestamp", "test.pdf", &extracted);

        // Timestamp should be numeric
        assert!(result.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_extracted_variable_no_match_no_default() {
        let extracted = vec![ExtractedVariable {
            name: "missing".to_string(),
            pattern: r"(?P<missing>WONT_MATCH)".to_string(),
            transform: None,
            default: None,
        }];

        let engine = VariableEngine::new(&extracted);
        let vars = engine.extract_variables("some text without match");

        // Variable should not be present
        assert!(!vars.contains_key("missing"));
    }

    #[test]
    fn test_sanitize_preserves_valid_chars() {
        assert_eq!(
            sanitize_filename("valid-file_name.pdf"),
            "valid-file_name.pdf"
        );
        assert_eq!(sanitize_filename("123_abc-DEF.txt"), "123_abc-DEF.txt");
    }

    #[test]
    fn test_sanitize_empty_string() {
        assert_eq!(sanitize_filename(""), "");
    }

    #[test]
    fn test_sanitize_only_underscores() {
        assert_eq!(sanitize_filename("___"), "");
    }

    #[test]
    fn test_multiple_extracted_variables() {
        let extracted = vec![
            ExtractedVariable {
                name: "first".to_string(),
                pattern: r"first:(?P<first>\w+)".to_string(),
                transform: None,
                default: None,
            },
            ExtractedVariable {
                name: "second".to_string(),
                pattern: r"second:(?P<second>\w+)".to_string(),
                transform: None,
                default: None,
            },
        ];

        let engine = VariableEngine::new(&extracted);
        let vars = engine.extract_variables("first:alpha second:beta");

        assert_eq!(vars.get("first"), Some(&"alpha".to_string()));
        assert_eq!(vars.get("second"), Some(&"beta".to_string()));
    }
}
