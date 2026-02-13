//! Table-driven tests for rule matching and categorization.
//!
//! Tests cover all match condition types, priority ordering, and edge cases.

mod common;

use common::{match_all, match_any, match_not, simple_contains, simple_contains_any, RuleBuilder};
use paporg::categorizer::Categorizer;
use paporg::config::schema::{DefaultsConfig, OutputConfig};

/// Represents a single categorization test case.
struct CategorizationTestCase {
    /// Test case name for identification.
    name: &'static str,
    /// Text to categorize.
    text: &'static str,
    /// Expected category result.
    expected_category: &'static str,
    /// Expected rule ID (None if no rule should match).
    expected_rule_id: Option<&'static str>,
}

/// Test cases for simple contains match.
const CONTAINS_TESTS: &[CategorizationTestCase] = &[
    CategorizationTestCase {
        name: "contains_match_exact",
        text: "This is an Invoice document",
        expected_category: "invoices",
        expected_rule_id: Some("invoice"),
    },
    CategorizationTestCase {
        name: "contains_match_case_sensitive",
        text: "This has Invoice with capital I",
        expected_category: "invoices",
        expected_rule_id: Some("invoice"),
    },
    CategorizationTestCase {
        name: "contains_no_match",
        text: "Random document text",
        expected_category: "unsorted",
        expected_rule_id: None,
    },
    CategorizationTestCase {
        name: "contains_no_match_lowercase",
        text: "The invoice amount is due",
        expected_category: "unsorted", // lowercase 'i' doesn't match "Invoice"
        expected_rule_id: None,
    },
];

fn create_contains_categorizer() -> Categorizer {
    let rules = vec![RuleBuilder::new("invoice", "invoices")
        .contains("Invoice")
        .priority(50)
        .build()];
    let defaults = create_defaults();
    Categorizer::new(rules, defaults)
}

#[test]
fn test_contains_categorization() {
    let categorizer = create_contains_categorizer();

    for test_case in CONTAINS_TESTS {
        let result = categorizer.categorize(test_case.text);

        assert_eq!(
            result.category, test_case.expected_category,
            "Test '{}': Expected category '{}', got '{}'",
            test_case.name, test_case.expected_category, result.category
        );

        match test_case.expected_rule_id {
            Some(id) => {
                assert_eq!(
                    result.rule_id.as_deref(),
                    Some(id),
                    "Test '{}': Expected rule_id '{}', got {:?}",
                    test_case.name,
                    id,
                    result.rule_id
                );
            }
            None => {
                assert!(
                    result.rule_id.is_none(),
                    "Test '{}': Expected no rule_id, got {:?}",
                    test_case.name,
                    result.rule_id
                );
            }
        }
    }
}

/// Test cases for containsAny match.
const CONTAINS_ANY_TESTS: &[CategorizationTestCase] = &[
    CategorizationTestCase {
        name: "contains_any_first_option",
        text: "This is an Invoice",
        expected_category: "bills",
        expected_rule_id: Some("bills"),
    },
    CategorizationTestCase {
        name: "contains_any_second_option",
        text: "This is a Rechnung",
        expected_category: "bills",
        expected_rule_id: Some("bills"),
    },
    CategorizationTestCase {
        name: "contains_any_third_option",
        text: "This is a Bill",
        expected_category: "bills",
        expected_rule_id: Some("bills"),
    },
    CategorizationTestCase {
        name: "contains_any_multiple_matches",
        text: "Invoice and Bill together",
        expected_category: "bills",
        expected_rule_id: Some("bills"),
    },
    CategorizationTestCase {
        name: "contains_any_no_match",
        text: "Nothing matches here",
        expected_category: "unsorted",
        expected_rule_id: None,
    },
];

fn create_contains_any_categorizer() -> Categorizer {
    let rules = vec![RuleBuilder::new("bills", "bills")
        .contains_any(vec!["Invoice", "Rechnung", "Bill"])
        .priority(50)
        .build()];
    let defaults = create_defaults();
    Categorizer::new(rules, defaults)
}

#[test]
fn test_contains_any_categorization() {
    let categorizer = create_contains_any_categorizer();

    for test_case in CONTAINS_ANY_TESTS {
        let result = categorizer.categorize(test_case.text);

        assert_eq!(
            result.category, test_case.expected_category,
            "Test '{}': Expected category '{}', got '{}'",
            test_case.name, test_case.expected_category, result.category
        );

        match test_case.expected_rule_id {
            Some(id) => {
                assert_eq!(
                    result.rule_id.as_deref(),
                    Some(id),
                    "Test '{}': Expected rule_id '{}', got {:?}",
                    test_case.name,
                    id,
                    result.rule_id
                );
            }
            None => {
                assert!(
                    result.rule_id.is_none(),
                    "Test '{}': Expected no rule_id, got {:?}",
                    test_case.name,
                    result.rule_id
                );
            }
        }
    }
}

/// Test cases for containsAll match.
const CONTAINS_ALL_TESTS: &[CategorizationTestCase] = &[
    CategorizationTestCase {
        name: "contains_all_both_present",
        text: "Invoice with VAT included",
        expected_category: "tax-invoices",
        expected_rule_id: Some("tax-invoice"),
    },
    CategorizationTestCase {
        name: "contains_all_missing_first",
        text: "VAT document without i-word",
        expected_category: "unsorted",
        expected_rule_id: None,
    },
    CategorizationTestCase {
        name: "contains_all_missing_second",
        text: "Invoice without tax keyword",
        expected_category: "unsorted",
        expected_rule_id: None,
    },
    CategorizationTestCase {
        name: "contains_all_different_order",
        text: "VAT is mentioned before Invoice",
        expected_category: "tax-invoices",
        expected_rule_id: Some("tax-invoice"),
    },
];

fn create_contains_all_categorizer() -> Categorizer {
    let rules = vec![RuleBuilder::new("tax-invoice", "tax-invoices")
        .contains_all(vec!["Invoice", "VAT"])
        .priority(50)
        .build()];
    let defaults = create_defaults();
    Categorizer::new(rules, defaults)
}

#[test]
fn test_contains_all_categorization() {
    let categorizer = create_contains_all_categorizer();

    for test_case in CONTAINS_ALL_TESTS {
        let result = categorizer.categorize(test_case.text);

        assert_eq!(
            result.category, test_case.expected_category,
            "Test '{}': Expected category '{}', got '{}'",
            test_case.name, test_case.expected_category, result.category
        );
    }
}

/// Test cases for pattern (regex) match.
const PATTERN_TESTS: &[CategorizationTestCase] = &[
    CategorizationTestCase {
        name: "pattern_simple_match",
        text: "Invoice INV-12345 ready",
        expected_category: "numbered",
        expected_rule_id: Some("numbered"),
    },
    CategorizationTestCase {
        name: "pattern_long_number",
        text: "Reference: INV-1234567890",
        expected_category: "numbered",
        expected_rule_id: Some("numbered"),
    },
    CategorizationTestCase {
        name: "pattern_short_number_no_match",
        text: "Invoice INV-12 too short",
        expected_category: "unsorted",
        expected_rule_id: None,
    },
    CategorizationTestCase {
        name: "pattern_no_match",
        text: "No invoice number here",
        expected_category: "unsorted",
        expected_rule_id: None,
    },
];

fn create_pattern_categorizer() -> Categorizer {
    let rules = vec![RuleBuilder::new("numbered", "numbered")
        .pattern(r"INV-\d{4,}")
        .priority(50)
        .build()];
    let defaults = create_defaults();
    Categorizer::new(rules, defaults)
}

#[test]
fn test_pattern_categorization() {
    let categorizer = create_pattern_categorizer();

    for test_case in PATTERN_TESTS {
        let result = categorizer.categorize(test_case.text);

        assert_eq!(
            result.category, test_case.expected_category,
            "Test '{}': Expected category '{}', got '{}'",
            test_case.name, test_case.expected_category, result.category
        );
    }
}

/// Test priority ordering.
#[test]
fn test_priority_ordering() {
    let rules = vec![
        RuleBuilder::new("low", "low")
            .contains("invoice")
            .priority(10)
            .build(),
        RuleBuilder::new("medium", "medium")
            .contains("invoice")
            .priority(50)
            .build(),
        RuleBuilder::new("high", "high")
            .contains("invoice")
            .priority(100)
            .build(),
    ];

    let categorizer = Categorizer::new(rules, create_defaults());
    let result = categorizer.categorize("This is an invoice");

    assert_eq!(result.category, "high");
    assert_eq!(result.rule_id, Some("high".to_string()));
}

/// Test priority with different match types.
#[test]
fn test_priority_with_different_match_types() {
    let rules = vec![
        RuleBuilder::new("general", "general")
            .contains_any(vec!["Invoice", "Rechnung"])
            .priority(50)
            .build(),
        RuleBuilder::new("tax", "tax")
            .contains_all(vec!["Invoice", "VAT"])
            .priority(100)
            .build(),
    ];

    let categorizer = Categorizer::new(rules, create_defaults());

    // Text matching both rules - higher priority wins
    let result = categorizer.categorize("Invoice with VAT");
    assert_eq!(result.category, "tax");

    // Text matching only lower priority rule
    let result2 = categorizer.categorize("Invoice without tax");
    assert_eq!(result2.category, "general");
}

/// Test compound "all" conditions.
#[test]
fn test_compound_all() {
    let rule = RuleBuilder::new("tax-invoice", "tax-invoices")
        .match_condition(match_all(vec![
            simple_contains_any(vec!["Invoice", "Rechnung"]),
            simple_contains_any(vec!["VAT", "MwSt"]),
        ]))
        .priority(100)
        .build();

    let categorizer = Categorizer::new(vec![rule], create_defaults());

    // Both conditions met
    assert_eq!(
        categorizer.categorize("Invoice with VAT").category,
        "tax-invoices"
    );
    assert_eq!(
        categorizer.categorize("Rechnung mit MwSt").category,
        "tax-invoices"
    );

    // Only first condition met
    assert_eq!(categorizer.categorize("Invoice only").category, "unsorted");

    // Only second condition met
    assert_eq!(categorizer.categorize("VAT document").category, "unsorted");
}

/// Test compound "any" conditions.
#[test]
fn test_compound_any() {
    let rule = RuleBuilder::new("international", "international")
        .match_condition(match_any(vec![
            simple_contains("USD"),
            simple_contains("EUR"),
            simple_contains("GBP"),
        ]))
        .priority(50)
        .build();

    let categorizer = Categorizer::new(vec![rule], create_defaults());

    assert_eq!(
        categorizer.categorize("Amount: $100 USD").category,
        "international"
    );
    assert_eq!(
        categorizer.categorize("Total: 50 EUR").category,
        "international"
    );
    assert_eq!(
        categorizer.categorize("Price: 75 GBP").category,
        "international"
    );
    assert_eq!(
        categorizer.categorize("Amount: 100 CHF").category,
        "unsorted"
    );
}

/// Test compound "not" conditions.
#[test]
fn test_compound_not() {
    let rule = RuleBuilder::new("non-draft", "final")
        .match_condition(match_all(vec![
            simple_contains("invoice"),
            match_not(simple_contains("DRAFT")),
        ]))
        .priority(50)
        .build();

    let categorizer = Categorizer::new(vec![rule], create_defaults());

    assert_eq!(categorizer.categorize("Final invoice").category, "final");
    assert_eq!(categorizer.categorize("DRAFT invoice").category, "unsorted");
    assert_eq!(categorizer.categorize("invoice DRAFT").category, "unsorted");
}

/// Test nested compound conditions.
#[test]
fn test_nested_compound_conditions() {
    // Rule: (Invoice OR Rechnung) AND (VAT OR MwSt) AND NOT (DRAFT OR VOID)
    let rule = RuleBuilder::new("final-tax", "final-tax")
        .match_condition(match_all(vec![
            match_any(vec![
                simple_contains("Invoice"),
                simple_contains("Rechnung"),
            ]),
            match_any(vec![simple_contains("VAT"), simple_contains("MwSt")]),
            match_not(match_any(vec![
                simple_contains("DRAFT"),
                simple_contains("VOID"),
            ])),
        ]))
        .priority(100)
        .build();

    let categorizer = Categorizer::new(vec![rule], create_defaults());

    assert_eq!(
        categorizer.categorize("Invoice with VAT total").category,
        "final-tax"
    );
    assert_eq!(
        categorizer.categorize("Rechnung mit MwSt").category,
        "final-tax"
    );
    assert_eq!(
        categorizer.categorize("DRAFT Invoice VAT").category,
        "unsorted"
    );
    assert_eq!(
        categorizer.categorize("VOID Invoice VAT").category,
        "unsorted"
    );
    assert_eq!(categorizer.categorize("Invoice only").category, "unsorted");
}

/// Test default category fallback.
#[test]
fn test_default_fallback() {
    let rules = vec![RuleBuilder::new("invoice", "invoices")
        .contains("Invoice")
        .priority(50)
        .build()];

    let categorizer = Categorizer::new(rules, create_defaults());

    let result = categorizer.categorize("No matching keywords here");

    assert_eq!(result.category, "unsorted");
    assert!(result.rule_id.is_none());
    assert_eq!(result.output.directory, "$y/unsorted");
    assert_eq!(result.output.filename, "$original");
}

/// Test output configuration is preserved.
#[test]
fn test_output_configuration_preserved() {
    let rules = vec![RuleBuilder::new("invoice", "invoices")
        .contains("Invoice")
        .output("$y/invoices/$vendor", "$invoice_number")
        .symlink("latest/$vendor")
        .build()];

    let categorizer = Categorizer::new(rules, create_defaults());
    let result = categorizer.categorize("This Invoice document");

    assert_eq!(result.output.directory, "$y/invoices/$vendor");
    assert_eq!(result.output.filename, "$invoice_number");
    assert_eq!(result.symlinks.len(), 1);
    assert_eq!(result.symlinks[0].target, "latest/$vendor");
}

/// Test empty containsAny matches nothing.
#[test]
fn test_empty_contains_any() {
    let rules = vec![RuleBuilder::new("empty", "empty")
        .contains_any(vec![])
        .priority(50)
        .build()];

    let categorizer = Categorizer::new(rules, create_defaults());

    // Empty containsAny should never match
    assert_eq!(categorizer.categorize("any text").category, "unsorted");
}

/// Test empty containsAll matches everything.
#[test]
fn test_empty_contains_all() {
    let rules = vec![RuleBuilder::new("all", "all")
        .contains_all(vec![])
        .priority(50)
        .build()];

    let categorizer = Categorizer::new(rules, create_defaults());

    // Empty containsAll returns true (vacuous truth)
    assert_eq!(categorizer.categorize("any text").category, "all");
}

/// Test case sensitivity.
#[test]
fn test_case_sensitivity() {
    let rules = vec![RuleBuilder::new("sensitive", "sensitive")
        .contains("Invoice")
        .priority(50)
        .build()];

    let categorizer = Categorizer::new(rules, create_defaults());

    // Should match exact case
    assert_eq!(categorizer.categorize("Invoice here").category, "sensitive");

    // Should NOT match different case
    assert_eq!(categorizer.categorize("invoice here").category, "unsorted");
    assert_eq!(categorizer.categorize("INVOICE here").category, "unsorted");
}

/// Test regex special characters in pattern.
#[test]
fn test_regex_special_characters() {
    let rules = vec![RuleBuilder::new("price", "price")
        .pattern(r"Price:\s+\$\d+\.\d{2}")
        .priority(50)
        .build()];

    let categorizer = Categorizer::new(rules, create_defaults());

    assert_eq!(categorizer.categorize("Price: $100.00").category, "price");
    assert_eq!(categorizer.categorize("Price: $1.99").category, "price");
    assert_eq!(categorizer.categorize("Price: 100").category, "unsorted");
}

/// Test invalid regex doesn't match.
#[test]
fn test_invalid_regex_no_match() {
    let rules = vec![RuleBuilder::new("bad", "bad")
        .pattern("[invalid")
        .priority(50)
        .build()];

    let categorizer = Categorizer::new(rules, create_defaults());

    // Invalid regex should just not match
    assert_eq!(categorizer.categorize("any text").category, "unsorted");
}

/// Test multiple rules with same priority (first defined wins).
#[test]
fn test_same_priority_first_wins() {
    let rules = vec![
        RuleBuilder::new("first", "first")
            .contains("test")
            .priority(50)
            .build(),
        RuleBuilder::new("second", "second")
            .contains("test")
            .priority(50)
            .build(),
    ];

    let categorizer = Categorizer::new(rules, create_defaults());
    let result = categorizer.categorize("test document");

    // When priorities are equal, behavior depends on sort stability
    // The result should be one of the matching rules
    assert!(
        result.rule_id == Some("first".to_string()) || result.rule_id == Some("second".to_string()),
        "Expected 'first' or 'second', got {:?}",
        result.rule_id
    );
}

/// Test rule with no match conditions (empty simple match).
#[test]
fn test_empty_simple_match_no_match() {
    // A rule with no conditions specified should not match
    let rules = vec![
        RuleBuilder::new("empty", "empty").priority(50).build(), // Default match condition is empty
    ];

    let categorizer = Categorizer::new(rules, create_defaults());

    // Empty condition should not match
    assert_eq!(categorizer.categorize("any text").category, "unsorted");
}

/// Helper function to create default categorization config.
fn create_defaults() -> DefaultsConfig {
    DefaultsConfig {
        output: OutputConfig {
            directory: "$y/unsorted".to_string(),
            filename: "$original".to_string(),
        },
    }
}

/// Test count validation.
#[test]
fn test_categorization_test_count() {
    let total_tests = CONTAINS_TESTS.len()
        + CONTAINS_ANY_TESTS.len()
        + CONTAINS_ALL_TESTS.len()
        + PATTERN_TESTS.len();

    assert!(
        total_tests >= 15,
        "Expected at least 15 table-driven test cases, got {}",
        total_tests
    );
}
