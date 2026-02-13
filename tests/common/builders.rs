//! Builder patterns for creating test data programmatically.
//!
//! These builders allow creating complex test configurations without
//! repetitive boilerplate code.

#![allow(dead_code)]

use paporg::config::schema::{
    AiConfig, CompoundMatch, Config, DefaultsConfig, ExtractedVariable, MatchCondition, OcrConfig,
    OutputConfig, Rule, SimpleMatch, SymlinkConfig, VariableTransform, VariablesConfig,
};

/// Builder for creating `Config` instances.
pub struct ConfigBuilder {
    version: String,
    input_directory: String,
    output_directory: String,
    worker_count: usize,
    ocr: OcrConfig,
    variables: VariablesConfig,
    rules: Vec<Rule>,
    defaults: DefaultsConfig,
    ai: AiConfig,
}

impl ConfigBuilder {
    /// Create a new builder with sensible defaults for testing.
    pub fn new() -> Self {
        Self {
            version: "1.0".to_string(),
            input_directory: "/tmp/input".to_string(),
            output_directory: "/tmp/output".to_string(),
            worker_count: 1,
            ocr: OcrConfig {
                enabled: false,
                languages: vec!["eng".to_string()],
                dpi: 300,
            },
            variables: VariablesConfig::default(),
            rules: vec![],
            defaults: DefaultsConfig {
                output: OutputConfig {
                    directory: "$y/unsorted".to_string(),
                    filename: "$original".to_string(),
                },
            },
            ai: AiConfig::default(),
        }
    }

    /// Set the config version.
    pub fn version(mut self, version: &str) -> Self {
        self.version = version.to_string();
        self
    }

    /// Set the input directory.
    pub fn input_directory(mut self, path: &str) -> Self {
        self.input_directory = path.to_string();
        self
    }

    /// Set the output directory.
    pub fn output_directory(mut self, path: &str) -> Self {
        self.output_directory = path.to_string();
        self
    }

    /// Set the worker count.
    pub fn worker_count(mut self, count: usize) -> Self {
        self.worker_count = count;
        self
    }

    /// Enable or disable OCR.
    pub fn ocr_enabled(mut self, enabled: bool) -> Self {
        self.ocr.enabled = enabled;
        self
    }

    /// Set OCR languages.
    pub fn ocr_languages(mut self, languages: Vec<String>) -> Self {
        self.ocr.languages = languages;
        self
    }

    /// Set OCR DPI.
    pub fn ocr_dpi(mut self, dpi: u32) -> Self {
        self.ocr.dpi = dpi;
        self
    }

    /// Add a rule to the config.
    pub fn rule(mut self, rule: Rule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Add multiple rules to the config.
    pub fn rules(mut self, rules: Vec<Rule>) -> Self {
        self.rules.extend(rules);
        self
    }

    /// Add an extracted variable to the config.
    pub fn variable(mut self, var: ExtractedVariable) -> Self {
        self.variables.extracted.push(var);
        self
    }

    /// Add multiple extracted variables.
    pub fn variables(mut self, vars: Vec<ExtractedVariable>) -> Self {
        self.variables.extracted.extend(vars);
        self
    }

    /// Set the default output configuration.
    pub fn default_output(mut self, directory: &str, filename: &str) -> Self {
        self.defaults.output = OutputConfig {
            directory: directory.to_string(),
            filename: filename.to_string(),
        };
        self
    }

    /// Build the final Config.
    pub fn build(self) -> Config {
        Config {
            version: self.version,
            input_directory: self.input_directory,
            output_directory: self.output_directory,
            worker_count: self.worker_count,
            ocr: self.ocr,
            variables: self.variables,
            rules: self.rules,
            defaults: self.defaults,
            ai: self.ai,
        }
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating `Rule` instances.
pub struct RuleBuilder {
    id: String,
    name: String,
    priority: i32,
    match_condition: MatchCondition,
    category: String,
    output: OutputConfig,
    symlinks: Vec<SymlinkConfig>,
}

impl RuleBuilder {
    /// Create a new rule builder with the given ID and category.
    pub fn new(id: &str, category: &str) -> Self {
        Self {
            id: id.to_string(),
            name: id.to_string(), // Default name to ID
            priority: 0,
            match_condition: MatchCondition::Simple(SimpleMatch {
                contains: None,
                contains_any: None,
                contains_all: None,
                pattern: None,
            }),
            category: category.to_string(),
            output: OutputConfig {
                directory: format!("$y/{}", category),
                filename: "$original".to_string(),
            },
            symlinks: vec![],
        }
    }

    /// Set the rule name.
    pub fn name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    /// Set the rule priority.
    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set a simple "contains" match condition.
    ///
    /// Note: `contains`, `contains_any`, `contains_all`, and `pattern` are mutually exclusive.
    /// Each call replaces any previous match condition.
    pub fn contains(mut self, text: &str) -> Self {
        self.match_condition = MatchCondition::Simple(SimpleMatch {
            contains: Some(text.to_string()),
            contains_any: None,
            contains_all: None,
            pattern: None,
        });
        self
    }

    /// Set a "containsAny" match condition.
    ///
    /// Note: `contains`, `contains_any`, `contains_all`, and `pattern` are mutually exclusive.
    /// Each call replaces any previous match condition.
    pub fn contains_any(mut self, texts: Vec<&str>) -> Self {
        self.match_condition = MatchCondition::Simple(SimpleMatch {
            contains: None,
            contains_any: Some(texts.into_iter().map(|s| s.to_string()).collect()),
            contains_all: None,
            pattern: None,
        });
        self
    }

    /// Set a "containsAll" match condition.
    ///
    /// Note: `contains`, `contains_any`, `contains_all`, and `pattern` are mutually exclusive.
    /// Each call replaces any previous match condition.
    pub fn contains_all(mut self, texts: Vec<&str>) -> Self {
        self.match_condition = MatchCondition::Simple(SimpleMatch {
            contains: None,
            contains_any: None,
            contains_all: Some(texts.into_iter().map(|s| s.to_string()).collect()),
            pattern: None,
        });
        self
    }

    /// Set a regex pattern match condition.
    ///
    /// Note: `contains`, `contains_any`, `contains_all`, and `pattern` are mutually exclusive.
    /// Each call replaces any previous match condition.
    pub fn pattern(mut self, pattern: &str) -> Self {
        self.match_condition = MatchCondition::Simple(SimpleMatch {
            contains: None,
            contains_any: None,
            contains_all: None,
            pattern: Some(pattern.to_string()),
        });
        self
    }

    /// Set a custom match condition.
    pub fn match_condition(mut self, condition: MatchCondition) -> Self {
        self.match_condition = condition;
        self
    }

    /// Set the output directory.
    pub fn output_directory(mut self, directory: &str) -> Self {
        self.output.directory = directory.to_string();
        self
    }

    /// Set the output filename.
    pub fn output_filename(mut self, filename: &str) -> Self {
        self.output.filename = filename.to_string();
        self
    }

    /// Set both output directory and filename.
    pub fn output(mut self, directory: &str, filename: &str) -> Self {
        self.output = OutputConfig {
            directory: directory.to_string(),
            filename: filename.to_string(),
        };
        self
    }

    /// Add a symlink to the rule.
    pub fn symlink(mut self, target: &str) -> Self {
        self.symlinks.push(SymlinkConfig {
            target: target.to_string(),
        });
        self
    }

    /// Build the final Rule.
    pub fn build(self) -> Rule {
        Rule {
            id: self.id,
            name: self.name,
            priority: self.priority,
            match_condition: self.match_condition,
            category: self.category,
            output: self.output,
            symlinks: self.symlinks,
        }
    }
}

/// Builder for creating `ExtractedVariable` instances.
pub struct VariableBuilder {
    name: String,
    pattern: String,
    transform: Option<VariableTransform>,
    default: Option<String>,
}

impl VariableBuilder {
    /// Create a new variable builder with name and pattern.
    ///
    /// Note: The pattern must include a named capture group matching the variable name.
    /// Use `?P<name>` or `?<name>` syntax.
    pub fn new(name: &str, pattern: &str) -> Self {
        Self {
            name: name.to_string(),
            pattern: pattern.to_string(),
            transform: None,
            default: None,
        }
    }

    /// Apply the slugify transform.
    pub fn slugify(mut self) -> Self {
        self.transform = Some(VariableTransform::Slugify);
        self
    }

    /// Apply the uppercase transform.
    pub fn uppercase(mut self) -> Self {
        self.transform = Some(VariableTransform::Uppercase);
        self
    }

    /// Apply the lowercase transform.
    pub fn lowercase(mut self) -> Self {
        self.transform = Some(VariableTransform::Lowercase);
        self
    }

    /// Apply the trim transform.
    pub fn trim(mut self) -> Self {
        self.transform = Some(VariableTransform::Trim);
        self
    }

    /// Set a custom transform.
    pub fn transform(mut self, transform: VariableTransform) -> Self {
        self.transform = Some(transform);
        self
    }

    /// Set a default value if the pattern doesn't match.
    pub fn default(mut self, default: &str) -> Self {
        self.default = Some(default.to_string());
        self
    }

    /// Build the final ExtractedVariable.
    pub fn build(self) -> ExtractedVariable {
        ExtractedVariable {
            name: self.name,
            pattern: self.pattern,
            transform: self.transform,
            default: self.default,
        }
    }
}

/// Helper to build compound "all" match conditions.
pub fn match_all(conditions: Vec<MatchCondition>) -> MatchCondition {
    MatchCondition::Compound(CompoundMatch {
        all: Some(conditions),
        any: None,
        not: None,
    })
}

/// Helper to build compound "any" match conditions.
pub fn match_any(conditions: Vec<MatchCondition>) -> MatchCondition {
    MatchCondition::Compound(CompoundMatch {
        all: None,
        any: Some(conditions),
        not: None,
    })
}

/// Helper to build compound "not" match conditions.
pub fn match_not(condition: MatchCondition) -> MatchCondition {
    MatchCondition::Compound(CompoundMatch {
        all: None,
        any: None,
        not: Some(Box::new(condition)),
    })
}

/// Helper to build a simple "contains" condition.
pub fn simple_contains(text: &str) -> MatchCondition {
    MatchCondition::Simple(SimpleMatch {
        contains: Some(text.to_string()),
        contains_any: None,
        contains_all: None,
        pattern: None,
    })
}

/// Helper to build a simple "containsAny" condition.
pub fn simple_contains_any(texts: Vec<&str>) -> MatchCondition {
    MatchCondition::Simple(SimpleMatch {
        contains: None,
        contains_any: Some(texts.into_iter().map(|s| s.to_string()).collect()),
        contains_all: None,
        pattern: None,
    })
}

/// Helper to build a simple "containsAll" condition.
pub fn simple_contains_all(texts: Vec<&str>) -> MatchCondition {
    MatchCondition::Simple(SimpleMatch {
        contains: None,
        contains_any: None,
        contains_all: Some(texts.into_iter().map(|s| s.to_string()).collect()),
        pattern: None,
    })
}

/// Helper to build a simple "pattern" condition.
pub fn simple_pattern(pattern: &str) -> MatchCondition {
    MatchCondition::Simple(SimpleMatch {
        contains: None,
        contains_any: None,
        contains_all: None,
        pattern: Some(pattern.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder_defaults() {
        let config = ConfigBuilder::new().build();

        assert_eq!(config.version, "1.0");
        assert_eq!(config.worker_count, 1);
        assert!(!config.ocr.enabled);
        assert!(config.rules.is_empty());
    }

    #[test]
    fn test_config_builder_with_rules() {
        let rule = RuleBuilder::new("test", "test-category")
            .contains("hello")
            .priority(50)
            .build();

        let config = ConfigBuilder::new().rule(rule).build();

        assert_eq!(config.rules.len(), 1);
        assert_eq!(config.rules[0].id, "test");
        assert_eq!(config.rules[0].priority, 50);
    }

    #[test]
    fn test_rule_builder_simple_contains() {
        let rule = RuleBuilder::new("invoice", "invoices")
            .name("Invoice Rule")
            .priority(100)
            .contains("invoice")
            .output("$y/invoices", "$original")
            .build();

        assert_eq!(rule.id, "invoice");
        assert_eq!(rule.name, "Invoice Rule");
        assert_eq!(rule.priority, 100);
        assert_eq!(rule.category, "invoices");
    }

    #[test]
    fn test_rule_builder_contains_any() {
        let rule = RuleBuilder::new("test", "test")
            .contains_any(vec!["a", "b", "c"])
            .build();

        match &rule.match_condition {
            MatchCondition::Simple(s) => {
                assert!(s.contains_any.is_some());
                assert_eq!(s.contains_any.as_ref().unwrap().len(), 3);
            }
            _ => panic!("Expected Simple match condition"),
        }
    }

    #[test]
    fn test_variable_builder() {
        let var = VariableBuilder::new("vendor", r"from:\s*(?P<vendor>\w+)")
            .slugify()
            .default("unknown")
            .build();

        assert_eq!(var.name, "vendor");
        assert!(matches!(var.transform, Some(VariableTransform::Slugify)));
        assert_eq!(var.default, Some("unknown".to_string()));
    }

    #[test]
    fn test_compound_match_helpers() {
        let condition = match_all(vec![
            simple_contains("invoice"),
            match_not(simple_contains("draft")),
        ]);

        match condition {
            MatchCondition::Compound(c) => {
                assert!(c.all.is_some());
                assert_eq!(c.all.as_ref().unwrap().len(), 2);
            }
            _ => panic!("Expected Compound match condition"),
        }
    }
}
