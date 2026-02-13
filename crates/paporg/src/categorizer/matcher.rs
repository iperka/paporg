use regex::Regex;
use std::collections::HashMap;

use crate::config::schema::{
    CompoundMatch, DefaultsConfig, MatchCondition, OutputConfig, Rule, SimpleMatch, SymlinkConfig,
};

pub struct Categorizer {
    rules: Vec<Rule>,
    defaults: DefaultsConfig,
    /// Pre-compiled regex patterns, indexed by pattern string
    compiled_patterns: HashMap<String, Regex>,
}

#[derive(Debug, Clone)]
pub struct CategorizationResult {
    pub rule_id: Option<String>,
    pub category: String,
    pub output: OutputConfig,
    pub symlinks: Vec<SymlinkConfig>,
}

impl Categorizer {
    pub fn new(mut rules: Vec<Rule>, defaults: DefaultsConfig) -> Self {
        // Sort rules by priority (descending)
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));

        // Pre-compile all regex patterns
        let mut compiled_patterns = HashMap::new();
        for rule in &rules {
            Self::collect_patterns(&rule.match_condition, &mut compiled_patterns);
        }

        Self {
            rules,
            defaults,
            compiled_patterns,
        }
    }

    /// Recursively collects and compiles regex patterns from match conditions.
    fn collect_patterns(condition: &MatchCondition, patterns: &mut HashMap<String, Regex>) {
        match condition {
            MatchCondition::Simple(simple) => {
                if let Some(pattern) = &simple.pattern {
                    if !patterns.contains_key(pattern) {
                        if let Ok(regex) = Regex::new(pattern) {
                            patterns.insert(pattern.clone(), regex);
                        }
                    }
                }
            }
            MatchCondition::Compound(compound) => {
                if let Some(all) = &compound.all {
                    for cond in all {
                        Self::collect_patterns(cond, patterns);
                    }
                }
                if let Some(any) = &compound.any {
                    for cond in any {
                        Self::collect_patterns(cond, patterns);
                    }
                }
                if let Some(not) = &compound.not {
                    Self::collect_patterns(not, patterns);
                }
            }
        }
    }

    pub fn categorize(&self, text: &str) -> CategorizationResult {
        // Find first matching rule
        for rule in &self.rules {
            if self.matches(&rule.match_condition, text) {
                return CategorizationResult {
                    rule_id: Some(rule.id.clone()),
                    category: rule.category.clone(),
                    output: rule.output.clone(),
                    symlinks: rule.symlinks.clone(),
                };
            }
        }

        // Return defaults if no rule matches
        CategorizationResult {
            rule_id: None,
            category: "unsorted".to_string(),
            output: self.defaults.output.clone(),
            symlinks: vec![],
        }
    }

    fn matches(&self, condition: &MatchCondition, text: &str) -> bool {
        match condition {
            MatchCondition::Compound(compound) => self.matches_compound(compound, text),
            MatchCondition::Simple(simple) => self.matches_simple(simple, text),
        }
    }

    fn matches_compound(&self, compound: &CompoundMatch, text: &str) -> bool {
        // Handle 'all' - all conditions must match
        if let Some(all) = &compound.all {
            return all.iter().all(|cond| self.matches(cond, text));
        }

        // Handle 'any' - at least one condition must match
        if let Some(any) = &compound.any {
            return any.iter().any(|cond| self.matches(cond, text));
        }

        // Handle 'not' - condition must not match
        if let Some(not) = &compound.not {
            return !self.matches(not, text);
        }

        false
    }

    fn matches_simple(&self, simple: &SimpleMatch, text: &str) -> bool {
        // 'contains' - text contains the string
        if let Some(contains) = &simple.contains {
            return text.contains(contains);
        }

        // 'containsAny' - text contains at least one of the strings
        if let Some(contains_any) = &simple.contains_any {
            return contains_any.iter().any(|s| text.contains(s));
        }

        // 'containsAll' - text contains all of the strings
        if let Some(contains_all) = &simple.contains_all {
            return contains_all.iter().all(|s| text.contains(s));
        }

        // 'pattern' - regex pattern matches (use pre-compiled regex)
        if let Some(pattern) = &simple.pattern {
            if let Some(regex) = self.compiled_patterns.get(pattern) {
                return regex.is_match(text);
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::OutputConfig;

    fn create_default_output() -> OutputConfig {
        OutputConfig {
            directory: "$y/unsorted".to_string(),
            filename: "$original".to_string(),
        }
    }

    fn create_defaults() -> DefaultsConfig {
        DefaultsConfig {
            output: create_default_output(),
        }
    }

    #[test]
    fn test_simple_contains_match() {
        let rules = vec![Rule {
            id: "invoice".to_string(),
            name: "Invoice".to_string(),
            priority: 0,
            match_condition: MatchCondition::Simple(SimpleMatch {
                contains: Some("invoice".to_string()),
                contains_any: None,
                contains_all: None,
                pattern: None,
            }),
            category: "invoices".to_string(),
            output: OutputConfig {
                directory: "$y/invoices".to_string(),
                filename: "$original".to_string(),
            },
            symlinks: vec![],
        }];

        let categorizer = Categorizer::new(rules, create_defaults());

        let result = categorizer.categorize("This is an invoice document");
        assert_eq!(result.rule_id, Some("invoice".to_string()));
        assert_eq!(result.category, "invoices");
    }

    #[test]
    fn test_contains_any_match() {
        let rules = vec![Rule {
            id: "invoice".to_string(),
            name: "Invoice".to_string(),
            priority: 0,
            match_condition: MatchCondition::Simple(SimpleMatch {
                contains: None,
                contains_any: Some(vec![
                    "invoice".to_string(),
                    "Rechnung".to_string(), // Match exact case
                    "bill".to_string(),
                ]),
                contains_all: None,
                pattern: None,
            }),
            category: "invoices".to_string(),
            output: OutputConfig {
                directory: "$y/invoices".to_string(),
                filename: "$original".to_string(),
            },
            symlinks: vec![],
        }];

        let categorizer = Categorizer::new(rules, create_defaults());

        assert_eq!(
            categorizer.categorize("This is a Rechnung").rule_id,
            Some("invoice".to_string())
        );
        assert_eq!(
            categorizer.categorize("This is a bill").rule_id,
            Some("invoice".to_string())
        );
        assert_eq!(categorizer.categorize("Random document").rule_id, None);
    }

    #[test]
    fn test_contains_all_match() {
        let rules = vec![Rule {
            id: "tax-invoice".to_string(),
            name: "Tax Invoice".to_string(),
            priority: 0,
            match_condition: MatchCondition::Simple(SimpleMatch {
                contains: None,
                contains_any: None,
                contains_all: Some(vec!["Invoice".to_string(), "VAT".to_string()]), // Match exact case
                pattern: None,
            }),
            category: "tax-invoices".to_string(),
            output: OutputConfig {
                directory: "$y/tax".to_string(),
                filename: "$original".to_string(),
            },
            symlinks: vec![],
        }];

        let categorizer = Categorizer::new(rules, create_defaults());

        assert_eq!(
            categorizer.categorize("Invoice with VAT included").rule_id,
            Some("tax-invoice".to_string())
        );
        assert_eq!(categorizer.categorize("Invoice without tax").rule_id, None);
    }

    #[test]
    fn test_pattern_match() {
        let rules = vec![Rule {
            id: "invoice-number".to_string(),
            name: "Invoice with number".to_string(),
            priority: 0,
            match_condition: MatchCondition::Simple(SimpleMatch {
                contains: None,
                contains_any: None,
                contains_all: None,
                pattern: Some(r"INV-\d{4,}".to_string()),
            }),
            category: "numbered-invoices".to_string(),
            output: OutputConfig {
                directory: "$y/invoices".to_string(),
                filename: "$original".to_string(),
            },
            symlinks: vec![],
        }];

        let categorizer = Categorizer::new(rules, create_defaults());

        assert_eq!(
            categorizer.categorize("Invoice INV-12345").rule_id,
            Some("invoice-number".to_string())
        );
        assert_eq!(categorizer.categorize("Invoice INV-12").rule_id, None);
    }

    #[test]
    fn test_compound_all_match() {
        let rules = vec![Rule {
            id: "tax-invoice".to_string(),
            name: "Tax Invoice".to_string(),
            priority: 0,
            match_condition: MatchCondition::Compound(CompoundMatch {
                all: Some(vec![
                    MatchCondition::Simple(SimpleMatch {
                        contains_any: Some(vec!["Invoice".to_string(), "Rechnung".to_string()]), // Match exact case
                        contains: None,
                        contains_all: None,
                        pattern: None,
                    }),
                    MatchCondition::Simple(SimpleMatch {
                        contains_any: Some(vec!["VAT".to_string(), "MwSt".to_string()]),
                        contains: None,
                        contains_all: None,
                        pattern: None,
                    }),
                ]),
                any: None,
                not: None,
            }),
            category: "tax-invoices".to_string(),
            output: OutputConfig {
                directory: "$y/tax".to_string(),
                filename: "$original".to_string(),
            },
            symlinks: vec![],
        }];

        let categorizer = Categorizer::new(rules, create_defaults());

        assert_eq!(
            categorizer.categorize("Invoice with VAT").rule_id,
            Some("tax-invoice".to_string())
        );
        assert_eq!(
            categorizer.categorize("Rechnung mit MwSt").rule_id,
            Some("tax-invoice".to_string())
        );
        assert_eq!(categorizer.categorize("Invoice without tax").rule_id, None);
    }

    #[test]
    fn test_compound_not_match() {
        let rules = vec![Rule {
            id: "non-draft".to_string(),
            name: "Non-Draft Invoice".to_string(),
            priority: 0,
            match_condition: MatchCondition::Compound(CompoundMatch {
                all: Some(vec![
                    MatchCondition::Simple(SimpleMatch {
                        contains: Some("invoice".to_string()),
                        contains_any: None,
                        contains_all: None,
                        pattern: None,
                    }),
                    MatchCondition::Compound(CompoundMatch {
                        not: Some(Box::new(MatchCondition::Simple(SimpleMatch {
                            contains: Some("DRAFT".to_string()),
                            contains_any: None,
                            contains_all: None,
                            pattern: None,
                        }))),
                        all: None,
                        any: None,
                    }),
                ]),
                any: None,
                not: None,
            }),
            category: "invoices".to_string(),
            output: OutputConfig {
                directory: "$y/invoices".to_string(),
                filename: "$original".to_string(),
            },
            symlinks: vec![],
        }];

        let categorizer = Categorizer::new(rules, create_defaults());

        assert_eq!(
            categorizer.categorize("Final invoice").rule_id,
            Some("non-draft".to_string())
        );
        assert_eq!(categorizer.categorize("DRAFT invoice").rule_id, None);
    }

    #[test]
    fn test_priority_ordering() {
        let rules = vec![
            Rule {
                id: "low-priority".to_string(),
                name: "Low Priority".to_string(),
                priority: 10,
                match_condition: MatchCondition::Simple(SimpleMatch {
                    contains: Some("invoice".to_string()),
                    contains_any: None,
                    contains_all: None,
                    pattern: None,
                }),
                category: "low".to_string(),
                output: OutputConfig {
                    directory: "low".to_string(),
                    filename: "$original".to_string(),
                },
                symlinks: vec![],
            },
            Rule {
                id: "high-priority".to_string(),
                name: "High Priority".to_string(),
                priority: 100,
                match_condition: MatchCondition::Simple(SimpleMatch {
                    contains: Some("invoice".to_string()),
                    contains_any: None,
                    contains_all: None,
                    pattern: None,
                }),
                category: "high".to_string(),
                output: OutputConfig {
                    directory: "high".to_string(),
                    filename: "$original".to_string(),
                },
                symlinks: vec![],
            },
        ];

        let categorizer = Categorizer::new(rules, create_defaults());

        // High priority should match first
        let result = categorizer.categorize("This is an invoice");
        assert_eq!(result.rule_id, Some("high-priority".to_string()));
        assert_eq!(result.category, "high");
    }

    #[test]
    fn test_default_categorization() {
        let rules = vec![Rule {
            id: "invoice".to_string(),
            name: "Invoice".to_string(),
            priority: 0,
            match_condition: MatchCondition::Simple(SimpleMatch {
                contains: Some("invoice".to_string()),
                contains_any: None,
                contains_all: None,
                pattern: None,
            }),
            category: "invoices".to_string(),
            output: OutputConfig {
                directory: "$y/invoices".to_string(),
                filename: "$original".to_string(),
            },
            symlinks: vec![],
        }];

        let categorizer = Categorizer::new(rules, create_defaults());

        let result = categorizer.categorize("Random unmatched document");
        assert_eq!(result.rule_id, None);
        assert_eq!(result.category, "unsorted");
        assert_eq!(result.output.directory, "$y/unsorted");
    }

    #[test]
    fn test_contains_case_sensitivity() {
        // 'contains' is case-sensitive
        let rules = vec![Rule {
            id: "test".to_string(),
            name: "Test".to_string(),
            priority: 0,
            match_condition: MatchCondition::Simple(SimpleMatch {
                contains: Some("Invoice".to_string()),
                contains_any: None,
                contains_all: None,
                pattern: None,
            }),
            category: "test".to_string(),
            output: create_default_output(),
            symlinks: vec![],
        }];

        let categorizer = Categorizer::new(rules, create_defaults());

        // Should match exact case
        assert!(categorizer
            .categorize("This is an Invoice")
            .rule_id
            .is_some());
        // Should NOT match different case
        assert!(categorizer
            .categorize("This is an invoice")
            .rule_id
            .is_none());
        assert!(categorizer
            .categorize("This is an INVOICE")
            .rule_id
            .is_none());
    }

    #[test]
    fn test_empty_contains_any_matches_nothing() {
        let rules = vec![Rule {
            id: "test".to_string(),
            name: "Test".to_string(),
            priority: 0,
            match_condition: MatchCondition::Simple(SimpleMatch {
                contains: None,
                contains_any: Some(vec![]),
                contains_all: None,
                pattern: None,
            }),
            category: "test".to_string(),
            output: create_default_output(),
            symlinks: vec![],
        }];

        let categorizer = Categorizer::new(rules, create_defaults());

        // Empty contains_any should never match
        assert!(categorizer.categorize("any text").rule_id.is_none());
    }

    #[test]
    fn test_empty_contains_all_matches_everything() {
        let rules = vec![Rule {
            id: "test".to_string(),
            name: "Test".to_string(),
            priority: 0,
            match_condition: MatchCondition::Simple(SimpleMatch {
                contains: None,
                contains_any: None,
                contains_all: Some(vec![]),
                pattern: None,
            }),
            category: "test".to_string(),
            output: create_default_output(),
            symlinks: vec![],
        }];

        let categorizer = Categorizer::new(rules, create_defaults());

        // Empty contains_all returns true (all zero conditions are met)
        assert!(categorizer.categorize("any text").rule_id.is_some());
    }

    #[test]
    fn test_deeply_nested_conditions() {
        // all -> any -> not -> contains
        let rules = vec![Rule {
            id: "nested".to_string(),
            name: "Nested".to_string(),
            priority: 0,
            match_condition: MatchCondition::Compound(CompoundMatch {
                all: Some(vec![
                    MatchCondition::Compound(CompoundMatch {
                        any: Some(vec![MatchCondition::Compound(CompoundMatch {
                            not: Some(Box::new(MatchCondition::Simple(SimpleMatch {
                                contains: Some("exclude".to_string()),
                                contains_any: None,
                                contains_all: None,
                                pattern: None,
                            }))),
                            all: None,
                            any: None,
                        })]),
                        all: None,
                        not: None,
                    }),
                    MatchCondition::Simple(SimpleMatch {
                        contains: Some("include".to_string()),
                        contains_any: None,
                        contains_all: None,
                        pattern: None,
                    }),
                ]),
                any: None,
                not: None,
            }),
            category: "nested".to_string(),
            output: create_default_output(),
            symlinks: vec![],
        }];

        let categorizer = Categorizer::new(rules, create_defaults());

        // Must include "include" AND NOT contain "exclude"
        assert!(categorizer.categorize("include this").rule_id.is_some());
        assert!(categorizer.categorize("include exclude").rule_id.is_none());
        assert!(categorizer.categorize("just text").rule_id.is_none());
    }

    #[test]
    fn test_pattern_with_regex_special_chars() {
        let rules = vec![Rule {
            id: "special".to_string(),
            name: "Special".to_string(),
            priority: 0,
            match_condition: MatchCondition::Simple(SimpleMatch {
                contains: None,
                contains_any: None,
                contains_all: None,
                // Match literal "Price: $100.00" with escaped special chars
                pattern: Some(r"Price:\s+\$\d+\.\d{2}".to_string()),
            }),
            category: "price".to_string(),
            output: create_default_output(),
            symlinks: vec![],
        }];

        let categorizer = Categorizer::new(rules, create_defaults());

        assert!(categorizer.categorize("Price: $100.00").rule_id.is_some());
        assert!(categorizer.categorize("Price: $1.99").rule_id.is_some());
        assert!(categorizer.categorize("Price: 100").rule_id.is_none());
    }

    #[test]
    fn test_compound_any_match() {
        let rules = vec![Rule {
            id: "any-compound".to_string(),
            name: "Any Compound".to_string(),
            priority: 0,
            match_condition: MatchCondition::Compound(CompoundMatch {
                any: Some(vec![
                    MatchCondition::Simple(SimpleMatch {
                        contains: Some("alpha".to_string()),
                        contains_any: None,
                        contains_all: None,
                        pattern: None,
                    }),
                    MatchCondition::Simple(SimpleMatch {
                        contains: Some("beta".to_string()),
                        contains_any: None,
                        contains_all: None,
                        pattern: None,
                    }),
                ]),
                all: None,
                not: None,
            }),
            category: "compound-any".to_string(),
            output: create_default_output(),
            symlinks: vec![],
        }];

        let categorizer = Categorizer::new(rules, create_defaults());

        assert!(categorizer.categorize("contains alpha").rule_id.is_some());
        assert!(categorizer.categorize("contains beta").rule_id.is_some());
        assert!(categorizer
            .categorize("contains alpha and beta")
            .rule_id
            .is_some());
        assert!(categorizer.categorize("contains neither").rule_id.is_none());
    }

    #[test]
    fn test_invalid_regex_pattern_no_match() {
        let rules = vec![Rule {
            id: "bad-regex".to_string(),
            name: "Bad Regex".to_string(),
            priority: 0,
            match_condition: MatchCondition::Simple(SimpleMatch {
                contains: None,
                contains_any: None,
                contains_all: None,
                pattern: Some("[invalid".to_string()), // Invalid regex
            }),
            category: "bad".to_string(),
            output: create_default_output(),
            symlinks: vec![],
        }];

        let categorizer = Categorizer::new(rules, create_defaults());

        // Invalid regex should just not match (fail silently)
        assert!(categorizer.categorize("any text").rule_id.is_none());
    }

    #[test]
    fn test_empty_simple_match_no_match() {
        // A SimpleMatch with all None fields
        let rules = vec![Rule {
            id: "empty".to_string(),
            name: "Empty".to_string(),
            priority: 0,
            match_condition: MatchCondition::Simple(SimpleMatch {
                contains: None,
                contains_any: None,
                contains_all: None,
                pattern: None,
            }),
            category: "empty".to_string(),
            output: create_default_output(),
            symlinks: vec![],
        }];

        let categorizer = Categorizer::new(rules, create_defaults());

        // Empty simple match should not match anything
        assert!(categorizer.categorize("any text").rule_id.is_none());
    }
}
