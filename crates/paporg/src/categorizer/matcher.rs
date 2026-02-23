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
    /// For each pattern, also compiles a case-insensitive variant prefixed with `(?i)`.
    fn collect_patterns(condition: &MatchCondition, patterns: &mut HashMap<String, Regex>) {
        match condition {
            MatchCondition::Simple(simple) => {
                if let Some(pattern) = &simple.pattern {
                    if !patterns.contains_key(pattern) {
                        if let Ok(regex) = Regex::new(pattern) {
                            patterns.insert(pattern.clone(), regex);
                        }
                    }
                    let ci_key = format!("(?i){}", pattern);
                    if let std::collections::hash_map::Entry::Vacant(e) =
                        patterns.entry(ci_key.clone())
                    {
                        if let Ok(regex) = Regex::new(&ci_key) {
                            e.insert(regex);
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
        // Pre-compute lowercase text once for case-insensitive matching
        let text_lower = text.to_lowercase();

        // Find first matching rule (default: case-insensitive)
        for rule in &self.rules {
            if self.matches(&rule.match_condition, text, &text_lower, false) {
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

    fn matches(
        &self,
        condition: &MatchCondition,
        text: &str,
        text_lower: &str,
        case_sensitive: bool,
    ) -> bool {
        match condition {
            MatchCondition::Compound(compound) => {
                self.matches_compound(compound, text, text_lower, case_sensitive)
            }
            MatchCondition::Simple(simple) => {
                self.matches_simple(simple, text, text_lower, case_sensitive)
            }
        }
    }

    fn matches_compound(
        &self,
        compound: &CompoundMatch,
        text: &str,
        text_lower: &str,
        inherited_case_sensitive: bool,
    ) -> bool {
        let case_sensitive = compound.case_sensitive.unwrap_or(inherited_case_sensitive);

        // Handle 'all' - all conditions must match
        if let Some(all) = &compound.all {
            return all
                .iter()
                .all(|cond| self.matches(cond, text, text_lower, case_sensitive));
        }

        // Handle 'any' - at least one condition must match
        if let Some(any) = &compound.any {
            return any
                .iter()
                .any(|cond| self.matches(cond, text, text_lower, case_sensitive));
        }

        // Handle 'not' - condition must not match
        if let Some(not) = &compound.not {
            return !self.matches(not, text, text_lower, case_sensitive);
        }

        false
    }

    fn matches_simple(
        &self,
        simple: &SimpleMatch,
        text: &str,
        text_lower: &str,
        inherited_case_sensitive: bool,
    ) -> bool {
        let case_sensitive = simple.case_sensitive.unwrap_or(inherited_case_sensitive);

        // 'contains' - text contains the string
        if let Some(contains) = &simple.contains {
            if case_sensitive {
                return text.contains(contains.as_str());
            } else {
                return text_lower.contains(&contains.to_lowercase());
            }
        }

        // 'containsAny' - text contains at least one of the strings
        if let Some(contains_any) = &simple.contains_any {
            if case_sensitive {
                return contains_any.iter().any(|s| text.contains(s.as_str()));
            } else {
                return contains_any
                    .iter()
                    .any(|s| text_lower.contains(&s.to_lowercase()));
            }
        }

        // 'containsAll' - text contains all of the strings
        if let Some(contains_all) = &simple.contains_all {
            if case_sensitive {
                return contains_all.iter().all(|s| text.contains(s.as_str()));
            } else {
                return contains_all
                    .iter()
                    .all(|s| text_lower.contains(&s.to_lowercase()));
            }
        }

        // 'pattern' - regex pattern matches (use pre-compiled regex)
        if let Some(pattern) = &simple.pattern {
            if case_sensitive {
                if let Some(regex) = self.compiled_patterns.get(pattern) {
                    return regex.is_match(text);
                }
            } else {
                let ci_key = format!("(?i){}", pattern);
                if let Some(regex) = self.compiled_patterns.get(&ci_key) {
                    return regex.is_match(text);
                }
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

    fn simple(contains: Option<&str>, case_sensitive: Option<bool>) -> SimpleMatch {
        SimpleMatch {
            contains: contains.map(|s| s.to_string()),
            contains_any: None,
            contains_all: None,
            pattern: None,
            case_sensitive,
        }
    }

    fn make_rule(id: &str, condition: MatchCondition) -> Rule {
        Rule {
            id: id.to_string(),
            name: id.to_string(),
            priority: 0,
            match_condition: condition,
            category: id.to_string(),
            output: create_default_output(),
            symlinks: vec![],
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
                case_sensitive: None,
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
                    "Rechnung".to_string(),
                    "bill".to_string(),
                ]),
                contains_all: None,
                pattern: None,
                case_sensitive: None,
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
                contains_all: Some(vec!["Invoice".to_string(), "VAT".to_string()]),
                pattern: None,
                case_sensitive: None,
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
        // "tax" != "VAT" even case-insensitively
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
                case_sensitive: None,
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
        // Also matches case-insensitively by default
        assert_eq!(
            categorizer.categorize("Invoice inv-12345").rule_id,
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
                        contains_any: Some(vec!["Invoice".to_string(), "Rechnung".to_string()]),
                        contains: None,
                        contains_all: None,
                        pattern: None,
                        case_sensitive: None,
                    }),
                    MatchCondition::Simple(SimpleMatch {
                        contains_any: Some(vec!["VAT".to_string(), "MwSt".to_string()]),
                        contains: None,
                        contains_all: None,
                        pattern: None,
                        case_sensitive: None,
                    }),
                ]),
                any: None,
                not: None,
                case_sensitive: None,
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
        // "tax" != "VAT" even case-insensitively
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
                        case_sensitive: None,
                    }),
                    MatchCondition::Compound(CompoundMatch {
                        not: Some(Box::new(MatchCondition::Simple(SimpleMatch {
                            contains: Some("DRAFT".to_string()),
                            contains_any: None,
                            contains_all: None,
                            pattern: None,
                            case_sensitive: None,
                        }))),
                        all: None,
                        any: None,
                        case_sensitive: None,
                    }),
                ]),
                any: None,
                not: None,
                case_sensitive: None,
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
                    case_sensitive: None,
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
                    case_sensitive: None,
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
                case_sensitive: None,
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
    fn test_contains_default_case_insensitive() {
        // Default behavior: case-insensitive matching
        let rules = vec![Rule {
            id: "test".to_string(),
            name: "Test".to_string(),
            priority: 0,
            match_condition: MatchCondition::Simple(SimpleMatch {
                contains: Some("Invoice".to_string()),
                contains_any: None,
                contains_all: None,
                pattern: None,
                case_sensitive: None,
            }),
            category: "test".to_string(),
            output: create_default_output(),
            symlinks: vec![],
        }];

        let categorizer = Categorizer::new(rules, create_defaults());

        // All casings should match by default (case-insensitive)
        assert!(categorizer
            .categorize("This is an Invoice")
            .rule_id
            .is_some());
        assert!(categorizer
            .categorize("This is an invoice")
            .rule_id
            .is_some());
        assert!(categorizer
            .categorize("This is an INVOICE")
            .rule_id
            .is_some());
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
                case_sensitive: None,
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
                case_sensitive: None,
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
                                case_sensitive: None,
                            }))),
                            all: None,
                            any: None,
                            case_sensitive: None,
                        })]),
                        all: None,
                        not: None,
                        case_sensitive: None,
                    }),
                    MatchCondition::Simple(SimpleMatch {
                        contains: Some("include".to_string()),
                        contains_any: None,
                        contains_all: None,
                        pattern: None,
                        case_sensitive: None,
                    }),
                ]),
                any: None,
                not: None,
                case_sensitive: None,
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
                case_sensitive: None,
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
                        case_sensitive: None,
                    }),
                    MatchCondition::Simple(SimpleMatch {
                        contains: Some("beta".to_string()),
                        contains_any: None,
                        contains_all: None,
                        pattern: None,
                        case_sensitive: None,
                    }),
                ]),
                all: None,
                not: None,
                case_sensitive: None,
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
                case_sensitive: None,
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
                case_sensitive: None,
            }),
            category: "empty".to_string(),
            output: create_default_output(),
            symlinks: vec![],
        }];

        let categorizer = Categorizer::new(rules, create_defaults());

        // Empty simple match should not match anything
        assert!(categorizer.categorize("any text").rule_id.is_none());
    }

    // ===== Case sensitivity tests =====

    #[test]
    fn test_case_insensitive_contains_default() {
        // Default (caseSensitive: None) → case-insensitive
        let rule = make_rule(
            "test",
            MatchCondition::Simple(simple(Some("Invoice"), None)),
        );
        let categorizer = Categorizer::new(vec![rule], create_defaults());

        assert!(categorizer
            .categorize("invoice from acme")
            .rule_id
            .is_some());
    }

    #[test]
    fn test_case_sensitive_contains_explicit() {
        // caseSensitive: true → case-sensitive
        let rule = make_rule(
            "test",
            MatchCondition::Simple(simple(Some("Invoice"), Some(true))),
        );
        let categorizer = Categorizer::new(vec![rule], create_defaults());

        assert!(categorizer
            .categorize("invoice from acme")
            .rule_id
            .is_none());
        assert!(categorizer
            .categorize("Invoice from acme")
            .rule_id
            .is_some());
    }

    #[test]
    fn test_case_insensitive_contains_any() {
        let rule = make_rule(
            "test",
            MatchCondition::Simple(SimpleMatch {
                contains: None,
                contains_any: Some(vec!["Invoice".to_string(), "Bill".to_string()]),
                contains_all: None,
                pattern: None,
                case_sensitive: None,
            }),
        );
        let categorizer = Categorizer::new(vec![rule], create_defaults());

        assert!(categorizer
            .categorize("this is an invoice")
            .rule_id
            .is_some());
        assert!(categorizer.categorize("this is a BILL").rule_id.is_some());
        assert!(categorizer.categorize("random document").rule_id.is_none());
    }

    #[test]
    fn test_case_insensitive_contains_all() {
        let rule = make_rule(
            "test",
            MatchCondition::Simple(SimpleMatch {
                contains: None,
                contains_any: None,
                contains_all: Some(vec!["Invoice".to_string(), "VAT".to_string()]),
                pattern: None,
                case_sensitive: None,
            }),
        );
        let categorizer = Categorizer::new(vec![rule], create_defaults());

        assert!(categorizer
            .categorize("invoice with vat included")
            .rule_id
            .is_some());
        assert!(categorizer.categorize("INVOICE WITH VAT").rule_id.is_some());
        assert!(categorizer
            .categorize("invoice without tax")
            .rule_id
            .is_none());
    }

    #[test]
    fn test_case_insensitive_pattern() {
        let rule = make_rule(
            "test",
            MatchCondition::Simple(SimpleMatch {
                contains: None,
                contains_any: None,
                contains_all: None,
                pattern: Some(r"INV-\d+".to_string()),
                case_sensitive: None,
            }),
        );
        let categorizer = Categorizer::new(vec![rule], create_defaults());

        assert!(categorizer.categorize("inv-123").rule_id.is_some());
        assert!(categorizer.categorize("INV-123").rule_id.is_some());
    }

    #[test]
    fn test_case_sensitive_pattern() {
        let rule = make_rule(
            "test",
            MatchCondition::Simple(SimpleMatch {
                contains: None,
                contains_any: None,
                contains_all: None,
                pattern: Some(r"INV-\d+".to_string()),
                case_sensitive: Some(true),
            }),
        );
        let categorizer = Categorizer::new(vec![rule], create_defaults());

        assert!(categorizer.categorize("inv-123").rule_id.is_none());
        assert!(categorizer.categorize("INV-123").rule_id.is_some());
    }

    #[test]
    fn test_compound_inherits_case_sensitivity() {
        // Parent compound sets caseSensitive: true, children inherit
        let rule = make_rule(
            "test",
            MatchCondition::Compound(CompoundMatch {
                all: Some(vec![
                    MatchCondition::Simple(simple(Some("Hello"), None)),
                    MatchCondition::Simple(simple(Some("World"), None)),
                ]),
                any: None,
                not: None,
                case_sensitive: Some(true),
            }),
        );
        let categorizer = Categorizer::new(vec![rule], create_defaults());

        // Children inherit case_sensitive=true from parent
        assert!(categorizer.categorize("hello world").rule_id.is_none());
        assert!(categorizer.categorize("Hello World").rule_id.is_some());
    }

    #[test]
    fn test_child_overrides_parent_case_sensitivity() {
        // Parent compound sets caseSensitive: true, but child overrides with false
        let rule = make_rule(
            "test",
            MatchCondition::Compound(CompoundMatch {
                all: Some(vec![
                    // This child overrides to case-insensitive
                    MatchCondition::Simple(simple(Some("Hello"), Some(false))),
                    // This child inherits case-sensitive from parent
                    MatchCondition::Simple(simple(Some("World"), None)),
                ]),
                any: None,
                not: None,
                case_sensitive: Some(true),
            }),
        );
        let categorizer = Categorizer::new(vec![rule], create_defaults());

        // "hello" matches (child overrides to case-insensitive)
        // "World" must be exact case (inherits case-sensitive from parent)
        assert!(categorizer.categorize("hello World").rule_id.is_some());
        // "world" doesn't match (case-sensitive inherited)
        assert!(categorizer.categorize("hello world").rule_id.is_none());
    }
}
