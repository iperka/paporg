//! Table-driven tests for variable extraction.
//!
//! Tests cover pattern matching, transforms, defaults, and edge cases.

mod common;

use std::collections::HashMap;

use common::VariableBuilder;
use paporg::VariableEngine;

/// Represents a single variable extraction test case.
struct VariableTestCase {
    /// Test case name for identification.
    name: &'static str,
    /// Text to extract variables from.
    text: &'static str,
    /// Variable name.
    var_name: &'static str,
    /// Regex pattern (must include named capture group matching var_name).
    pattern: &'static str,
    /// Transform to apply (None, "slugify", "uppercase", "lowercase", "trim").
    transform: Option<&'static str>,
    /// Default value if pattern doesn't match.
    default: Option<&'static str>,
    /// Expected extracted value (None if variable should not be present).
    expected_value: Option<&'static str>,
}

/// All variable extraction test cases.
const VARIABLE_TESTS: &[VariableTestCase] = &[
    // Basic extraction tests
    VariableTestCase {
        name: "simple_extraction",
        text: "Invoice from Acme Corporation",
        var_name: "vendor",
        pattern: r"from[:\s]+(?P<vendor>[A-Za-z]+)",
        transform: None,
        default: None,
        expected_value: Some("Acme"),
    },
    VariableTestCase {
        name: "extraction_with_colon",
        text: "Vendor: TechCorp Inc",
        var_name: "vendor",
        pattern: r"Vendor:\s*(?P<vendor>[A-Za-z]+)",
        transform: None,
        default: None,
        expected_value: Some("TechCorp"),
    },
    VariableTestCase {
        name: "extraction_invoice_number",
        text: "Invoice Number: INV-2026-001",
        var_name: "invoice_number",
        pattern: r"Invoice\s+Number:\s*(?P<invoice_number>[A-Z0-9-]+)",
        transform: None,
        default: None,
        expected_value: Some("INV-2026-001"),
    },
    VariableTestCase {
        name: "extraction_with_digits",
        text: "Order #12345 confirmed",
        var_name: "order",
        pattern: r"Order\s*#(?P<order>\d+)",
        transform: None,
        default: None,
        expected_value: Some("12345"),
    },
    // Transform tests
    VariableTestCase {
        name: "transform_slugify",
        text: "Invoice from Acme Corp Inc",
        var_name: "vendor",
        pattern: r"from\s+(?P<vendor>[A-Za-z\s]+?)(?:\s+Inc|\s*$)",
        transform: Some("slugify"),
        default: None,
        expected_value: Some("acme-corp"),
    },
    VariableTestCase {
        name: "transform_slugify_special_chars",
        text: "Vendor: Tech & Systems Ltd",
        var_name: "vendor",
        pattern: r"Vendor:\s*(?P<vendor>[^,]+)",
        transform: Some("slugify"),
        default: None,
        expected_value: Some("tech-systems-ltd"),
    },
    VariableTestCase {
        name: "transform_uppercase",
        text: "Category: invoices",
        var_name: "category",
        pattern: r"Category:\s*(?P<category>\w+)",
        transform: Some("uppercase"),
        default: None,
        expected_value: Some("INVOICES"),
    },
    VariableTestCase {
        name: "transform_lowercase",
        text: "Type: RECEIPT",
        var_name: "doctype",
        pattern: r"Type:\s*(?P<doctype>\w+)",
        transform: Some("lowercase"),
        default: None,
        expected_value: Some("receipt"),
    },
    VariableTestCase {
        name: "transform_trim",
        text: "Name:   Padded Value   ",
        var_name: "name",
        pattern: r"Name:\s*(?P<name>[^$]+)",
        transform: Some("trim"),
        default: None,
        expected_value: Some("Padded Value"),
    },
    // Default value tests
    VariableTestCase {
        name: "default_fallback_no_match",
        text: "Some text without vendor info",
        var_name: "vendor",
        pattern: r"from[:\s]+(?P<vendor>[A-Za-z]+)",
        transform: None,
        default: Some("unknown"),
        expected_value: Some("unknown"),
    },
    VariableTestCase {
        name: "default_not_used_when_matched",
        text: "Invoice from Acme",
        var_name: "vendor",
        pattern: r"from[:\s]+(?P<vendor>[A-Za-z]+)",
        transform: None,
        default: Some("unknown"),
        expected_value: Some("Acme"),
    },
    VariableTestCase {
        name: "no_default_no_match_absent",
        text: "Text without the pattern",
        var_name: "missing",
        pattern: r"(?P<missing>WONTMATCH)",
        transform: None,
        default: None,
        expected_value: None,
    },
    // Unicode and special character tests
    VariableTestCase {
        name: "unicode_extraction",
        text: "Firma: Müller GmbH",
        var_name: "firma",
        pattern: r"Firma:\s*(?P<firma>\p{L}+)",
        transform: None,
        default: None,
        expected_value: Some("Müller"),
    },
    VariableTestCase {
        name: "unicode_with_slugify",
        text: "Company: Café Berlin",
        var_name: "company",
        pattern: r"Company:\s*(?P<company>[^\n]+)",
        transform: Some("slugify"),
        default: None,
        // Slugify keeps accented chars (they're alphanumeric in unicode)
        expected_value: Some("café-berlin"),
    },
    // Edge cases
    VariableTestCase {
        name: "empty_capture_group",
        text: "prefixsuffix",
        var_name: "middle",
        pattern: r"prefix(?P<middle>.*?)suffix",
        transform: None,
        default: None,
        expected_value: Some(""),
    },
    VariableTestCase {
        name: "multiline_text",
        text: "Invoice\nFrom: TechCorp\nDate: 2026-01-15",
        var_name: "vendor",
        pattern: r"From:\s*(?P<vendor>\w+)",
        transform: None,
        default: None,
        expected_value: Some("TechCorp"),
    },
    VariableTestCase {
        name: "case_insensitive_pattern",
        text: "INVOICE FROM: lowercasecorp",
        var_name: "vendor",
        pattern: r"(?i)invoice\s+from:\s*(?P<vendor>\w+)",
        transform: None,
        default: None,
        expected_value: Some("lowercasecorp"),
    },
    VariableTestCase {
        name: "greedy_vs_lazy",
        text: "Amount: $100.00 USD",
        var_name: "amount",
        pattern: r"Amount:\s*\$(?P<amount>[\d.]+)",
        transform: None,
        default: None,
        expected_value: Some("100.00"),
    },
];

fn create_variable_engine(test_case: &VariableTestCase) -> VariableEngine {
    let mut builder = VariableBuilder::new(test_case.var_name, test_case.pattern);

    if let Some(transform) = test_case.transform {
        builder = match transform {
            "slugify" => builder.slugify(),
            "uppercase" => builder.uppercase(),
            "lowercase" => builder.lowercase(),
            "trim" => builder.trim(),
            other => panic!("Unknown transform: {}", other),
        };
    }

    if let Some(default) = test_case.default {
        builder = builder.default(default);
    }

    let var = builder.build();
    VariableEngine::new(&[var])
}

#[test]
fn test_variable_extraction() {
    for test_case in VARIABLE_TESTS {
        let engine = create_variable_engine(test_case);
        let extracted = engine.extract_variables(test_case.text);

        match test_case.expected_value {
            Some(expected) => {
                assert!(
                    extracted.contains_key(test_case.var_name),
                    "Test '{}': Expected variable '{}' to be extracted from '{}'\nExtracted: {:?}",
                    test_case.name,
                    test_case.var_name,
                    test_case.text,
                    extracted
                );
                assert_eq!(
                    extracted.get(test_case.var_name).unwrap(),
                    expected,
                    "Test '{}': Expected '{}' but got '{}'",
                    test_case.name,
                    expected,
                    extracted.get(test_case.var_name).unwrap()
                );
            }
            None => {
                assert!(
                    !extracted.contains_key(test_case.var_name),
                    "Test '{}': Expected variable '{}' to NOT be extracted, but got '{:?}'",
                    test_case.name,
                    test_case.var_name,
                    extracted.get(test_case.var_name)
                );
            }
        }
    }
}

/// Test count validation.
#[test]
fn test_variable_test_count() {
    const EXPECTED_VARIABLE_TESTS: usize = 18;
    assert_eq!(
        VARIABLE_TESTS.len(),
        EXPECTED_VARIABLE_TESTS,
        "Expected {} variable test cases, got {}. Update EXPECTED_VARIABLE_TESTS if adding/removing tests.",
        EXPECTED_VARIABLE_TESTS,
        VARIABLE_TESTS.len()
    );
}

/// Test multiple variables extraction.
#[test]
fn test_multiple_variables_extraction() {
    let vars = vec![
        VariableBuilder::new("vendor", r"(?i)from[:\s]+(?P<vendor>[A-Za-z]+)")
            .default("unknown")
            .build(),
        VariableBuilder::new(
            "invoice_number",
            r"Invoice\s+Number:\s*(?P<invoice_number>[A-Z0-9-]+)",
        )
        .build(),
        VariableBuilder::new("amount", r"Total:\s*\$(?P<amount>[\d.,]+)").build(),
    ];

    let engine = VariableEngine::new(&vars);
    let text = r#"
        INVOICE
        Invoice Number: INV-2026-001
        From: Acme Corporation
        Total: $1,500.00
    "#;

    let extracted = engine.extract_variables(text);

    assert_eq!(extracted.get("vendor"), Some(&"Acme".to_string()));
    assert_eq!(
        extracted.get("invoice_number"),
        Some(&"INV-2026-001".to_string())
    );
    assert_eq!(extracted.get("amount"), Some(&"1,500.00".to_string()));
}

/// Test variable substitution in templates.
#[test]
fn test_variable_substitution() {
    let vars = vec![
        VariableBuilder::new("vendor", r"from[:\s]+(?P<vendor>[A-Za-z]+)")
            .slugify()
            .build(),
    ];

    let engine = VariableEngine::new(&vars);
    let text = "Invoice from Acme Corporation";
    let extracted = engine.extract_variables(text);

    // Test substitution in directory template
    let result = engine.substitute("$y/invoices/$vendor", "invoice.pdf", &extracted);

    // Should contain the slugified vendor
    assert!(
        result.contains("acme"),
        "Expected result to contain 'acme', got '{}'",
        result
    );
    // Should have year substituted
    assert!(
        !result.contains("$y"),
        "Expected $y to be substituted, got '{}'",
        result
    );
}

/// Test builtin variables.
#[test]
fn test_builtin_variables() {
    let engine = VariableEngine::new(&[]);
    let extracted = HashMap::new();

    // Test year
    let year_result = engine.substitute("$y", "test.pdf", &extracted);
    assert!(
        year_result.chars().all(|c| c.is_ascii_digit()),
        "Year should be numeric: {}",
        year_result
    );
    assert_eq!(year_result.len(), 4, "Year should be 4 digits");

    // Test month
    let month_result = engine.substitute("$m", "test.pdf", &extracted);
    assert!(month_result.chars().all(|c| c.is_ascii_digit()));
    assert_eq!(month_result.len(), 2, "Month should be 2 digits");

    // Test day
    let day_result = engine.substitute("$d", "test.pdf", &extracted);
    assert!(day_result.chars().all(|c| c.is_ascii_digit()));
    assert_eq!(day_result.len(), 2, "Day should be 2 digits");

    // Test timestamp
    let timestamp_result = engine.substitute("$timestamp", "test.pdf", &extracted);
    assert!(timestamp_result.chars().all(|c| c.is_ascii_digit()));

    // Test original filename (without extension)
    let original_result = engine.substitute("$original", "my_document.pdf", &extracted);
    assert_eq!(original_result, "my_document");

    // Test uuid
    let uuid_result = engine.substitute("$uuid", "test.pdf", &extracted);
    // UUID format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx (36 chars with hyphens)
    assert!(uuid_result.len() >= 32, "UUID should be at least 32 chars");
}

/// Test UUID uniqueness.
#[test]
fn test_uuid_uniqueness() {
    let engine = VariableEngine::new(&[]);
    let extracted = HashMap::new();

    let uuid1 = engine.substitute("$uuid", "test.pdf", &extracted);
    let uuid2 = engine.substitute("$uuid", "test.pdf", &extracted);

    assert_ne!(uuid1, uuid2, "Each UUID substitution should be unique");
}

/// Test filename sanitization.
#[test]
fn test_filename_sanitization() {
    let engine = VariableEngine::new(&[]);
    let extracted = HashMap::new();

    // Test various problematic characters
    let result = engine.substitute("$original", "file<>:\"|?*.pdf", &extracted);

    // Should not contain filesystem-unsafe characters
    assert!(!result.contains('<'));
    assert!(!result.contains('>'));
    assert!(!result.contains(':'));
    assert!(!result.contains('"'));
    assert!(!result.contains('|'));
    assert!(!result.contains('?'));
    assert!(!result.contains('*'));
}

/// Test path separator handling in substitution.
#[test]
fn test_path_with_variables() {
    let vars = vec![
        VariableBuilder::new("category", r"Category:\s*(?P<category>\w+)")
            .lowercase()
            .build(),
    ];

    let engine = VariableEngine::new(&vars);
    let text = "Category: INVOICES";
    let extracted = engine.extract_variables(text);

    let path = engine.substitute("$y/$m/$category", "doc.pdf", &extracted);

    // Path should contain the category (note: / is sanitized to _)
    // The substitute function sanitizes the entire result for filesystem safety
    assert!(
        path.contains("invoices"),
        "Path should contain 'invoices': {}",
        path
    );
}

/// Test combined transform and default.
#[test]
fn test_transform_with_default() {
    let vars = vec![
        VariableBuilder::new("vendor", r"from[:\s]+(?P<vendor>[A-Za-z\s]+)")
            .slugify()
            .default("unknown-vendor")
            .build(),
    ];

    let engine = VariableEngine::new(&vars);

    // Test when pattern matches
    let matched = engine.extract_variables("Invoice from Acme Corp");
    assert_eq!(matched.get("vendor"), Some(&"acme-corp".to_string()));

    // Test when pattern doesn't match - default is used as-is
    let unmatched = engine.extract_variables("Random text");
    assert_eq!(unmatched.get("vendor"), Some(&"unknown-vendor".to_string()));
}

/// Test variable extraction with overlapping patterns.
#[test]
fn test_overlapping_patterns() {
    let vars = vec![
        VariableBuilder::new("full_date", r"Date:\s*(?P<full_date>\d{4}-\d{2}-\d{2})").build(),
        VariableBuilder::new("year", r"Date:\s*(?P<year>\d{4})").build(),
    ];

    let engine = VariableEngine::new(&vars);
    let text = "Date: 2026-01-15";
    let extracted = engine.extract_variables(text);

    // Both patterns should match
    assert_eq!(extracted.get("full_date"), Some(&"2026-01-15".to_string()));
    assert_eq!(extracted.get("year"), Some(&"2026".to_string()));
}
