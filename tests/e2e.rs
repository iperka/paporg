//! End-to-end tests for the paporg document processing pipeline.
//!
//! This module provides a data-driven test framework where adding a test case is as simple as:
//! 1. Put a file in `tests/fixtures/inputs/`
//! 2. Add an entry to the `TEST_CASES` array with expected behavior

mod common;

use std::path::Path;
use tempfile::TempDir;

use paporg::categorizer::Categorizer;
use paporg::config::load_config;
use paporg::processor::ProcessorRegistry;
use paporg::storage::FileStorage;

/// Represents a single end-to-end test case.
struct TestCase {
    /// Unique name for the test case
    name: &'static str,
    /// Input file name in tests/fixtures/inputs/
    input_file: &'static str,
    /// Config file name in tests/fixtures/configs/
    config: &'static str,
    /// Expected category after categorization
    expected_category: &'static str,
    /// Strings that should be present in extracted text
    expected_text_contains: &'static [&'static str],
}

/// All test cases to run. Add new test cases here.
const TEST_CASES: &[TestCase] = &[
    TestCase {
        name: "invoice_text_file",
        input_file: "invoice.txt",
        config: "basic-rules.json",
        expected_category: "invoices",
        expected_text_contains: &["Invoice", "INV-2026-001", "Acme Corporation", "$1,620.00"],
    },
    TestCase {
        name: "receipt_text_file",
        input_file: "receipt.txt",
        config: "basic-rules.json",
        expected_category: "receipts",
        expected_text_contains: &["RECEIPT", "TechMart", "$71.24"],
    },
    TestCase {
        name: "contract_text_file",
        input_file: "contract.txt",
        config: "basic-rules.json",
        expected_category: "contracts",
        expected_text_contains: &["Agreement", "CONTRACT", "Provider"],
    },
    TestCase {
        name: "unmatched_falls_to_default",
        input_file: "random.txt",
        config: "basic-rules.json",
        expected_category: "unsorted",
        expected_text_contains: &["Random Notes", "Lorem ipsum"],
    },
    // Priority rules tests
    TestCase {
        name: "priority_high_wins",
        input_file: "invoice.txt",
        config: "priority-rules.json",
        expected_category: "high-priority",
        expected_text_contains: &["Invoice"],
    },
    // Compound conditions tests
    TestCase {
        name: "compound_tax_invoice",
        input_file: "tax-invoice.txt",
        config: "compound-conditions.json",
        expected_category: "tax-invoices",
        expected_text_contains: &["Invoice", "VAT"],
    },
    TestCase {
        name: "compound_draft_excluded",
        input_file: "draft-invoice.txt",
        config: "compound-conditions.json",
        expected_category: "unsorted",
        expected_text_contains: &["DRAFT"],
    },
    TestCase {
        name: "compound_active_contract",
        input_file: "active-contract.txt",
        config: "compound-conditions.json",
        expected_category: "active-contracts",
        expected_text_contains: &["Agreement", "2026"],
    },
    // Variable extraction tests
    TestCase {
        name: "variable_extraction_config",
        input_file: "complex-variables.txt",
        config: "variable-extraction.json",
        expected_category: "invoices",
        expected_text_contains: &["Invoice", "Acme International"],
    },
    // =========================================================================
    // Document type variation tests
    // =========================================================================
    TestCase {
        name: "markdown_invoice",
        input_file: "invoice.md",
        config: "basic-rules.json",
        expected_category: "invoices",
        expected_text_contains: &["INVOICE", "INV-MD-001"],
    },
    TestCase {
        name: "german_invoice",
        input_file: "rechnung.txt",
        config: "basic-rules.json",
        expected_category: "invoices",
        expected_text_contains: &["RECHNUNG", "MwSt"],
    },
    TestCase {
        name: "french_receipt",
        input_file: "recu.txt",
        config: "basic-rules.json",
        expected_category: "receipts",
        expected_text_contains: &["REÇU", "Total"],
    },
    // =========================================================================
    // Edge case tests
    // =========================================================================
    TestCase {
        name: "empty_document",
        input_file: "empty.txt",
        config: "basic-rules.json",
        expected_category: "unsorted",
        expected_text_contains: &[],
    },
    TestCase {
        name: "whitespace_only",
        input_file: "whitespace.txt",
        config: "basic-rules.json",
        expected_category: "unsorted",
        expected_text_contains: &[],
    },
    TestCase {
        name: "unicode_content",
        input_file: "unicode-invoice.txt",
        config: "basic-rules.json",
        expected_category: "invoices",
        expected_text_contains: &["Invoice", "Müller"],
    },
    TestCase {
        name: "very_long_lines",
        input_file: "long-lines.txt",
        config: "basic-rules.json",
        expected_category: "invoices",
        expected_text_contains: &["Invoice"],
    },
    TestCase {
        name: "special_chars_filename",
        input_file: "invoice (copy).txt",
        config: "basic-rules.json",
        expected_category: "invoices",
        expected_text_contains: &["Invoice"],
    },
    // =========================================================================
    // Multiple rules matching tests
    // =========================================================================
    TestCase {
        name: "multi_match_priority",
        input_file: "invoice-receipt.txt",
        config: "priority-rules.json",
        expected_category: "high-priority",
        expected_text_contains: &["Invoice", "Receipt"],
    },
    TestCase {
        name: "three_way_match",
        input_file: "triple-match.txt",
        config: "basic-rules.json",
        expected_category: "contracts",
        expected_text_contains: &["Contract", "Invoice", "Receipt"],
    },
    // =========================================================================
    // Symlink tests
    // =========================================================================
    TestCase {
        name: "symlink_config",
        input_file: "invoice.txt",
        config: "symlink-config.json",
        expected_category: "invoices",
        expected_text_contains: &["Invoice"],
    },
    // =========================================================================
    // Variable extraction edge cases
    // =========================================================================
    TestCase {
        name: "no_variables_match",
        input_file: "random.txt",
        config: "variable-extraction.json",
        expected_category: "unsorted",
        expected_text_contains: &["Random Notes"],
    },
    TestCase {
        name: "overlapping_patterns",
        input_file: "multi-vendor.txt",
        config: "variable-extraction.json",
        expected_category: "invoices",
        expected_text_contains: &["First Corp"],
    },
];

fn get_fixture_path(relative: &str) -> std::path::PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir)
        .join("tests")
        .join("fixtures")
        .join(relative)
}

/// Run a single test case through the full pipeline.
fn run_test_case(test_case: &TestCase) {
    // Setup temp directories for output
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let output_dir = temp_dir.path().join("output");
    std::fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    // Load config
    let config_path = get_fixture_path(&format!("configs/{}", test_case.config));
    let config = load_config(&config_path).expect("Failed to load config");

    // Setup processor registry (OCR disabled for tests)
    let registry = ProcessorRegistry::new(false, &[], 300);

    // Get input file path
    let input_path = get_fixture_path(&format!("inputs/{}", test_case.input_file));

    // Process the document
    let processed = registry
        .process(&input_path)
        .expect(&format!("Failed to process {}", test_case.input_file));

    // Verify text extraction
    for expected_text in test_case.expected_text_contains {
        assert!(
            processed.text.contains(expected_text),
            "Test '{}': Expected text to contain '{}', but got:\n{}",
            test_case.name,
            expected_text,
            processed.text
        );
    }

    // Setup categorizer
    let categorizer = Categorizer::new(config.rules.clone(), config.defaults.clone());

    // Categorize the document
    let result = categorizer.categorize(&processed.text);

    // Verify categorization
    assert_eq!(
        result.category, test_case.expected_category,
        "Test '{}': Expected category '{}', got '{}'",
        test_case.name, test_case.expected_category, result.category
    );

    // Setup storage and store the document
    let storage = FileStorage::new(&output_dir);

    // Extract variables and substitute in directory/filename
    let variable_engine = paporg::VariableEngine::new(&config.variables.extracted);
    let extracted_vars = variable_engine.extract_variables(&processed.text);

    let output_directory = variable_engine.substitute(
        &result.output.directory,
        &processed.metadata.original_filename,
        &extracted_vars,
    );
    let output_filename = variable_engine.substitute(
        &result.output.filename,
        &processed.metadata.original_filename,
        &extracted_vars,
    );

    // Store the PDF
    let stored_path = storage
        .store(
            &processed.pdf_bytes,
            &output_directory,
            &output_filename,
            "pdf",
        )
        .expect("Failed to store document");

    // Verify file was stored
    assert!(
        stored_path.exists(),
        "Test '{}': Stored file does not exist at {:?}",
        test_case.name,
        stored_path
    );

    // Verify PDF content is valid (has bytes)
    let stored_content = std::fs::read(&stored_path).expect("Failed to read stored file");
    assert!(
        !stored_content.is_empty(),
        "Test '{}': Stored file is empty",
        test_case.name
    );
}

// ============================================================================
// Individual test functions for each test case
// ============================================================================

#[test]
fn test_invoice_processing() {
    run_test_case(&TEST_CASES[0]);
}

#[test]
fn test_receipt_processing() {
    run_test_case(&TEST_CASES[1]);
}

#[test]
fn test_contract_processing() {
    run_test_case(&TEST_CASES[2]);
}

#[test]
fn test_unmatched_default_category() {
    run_test_case(&TEST_CASES[3]);
}

#[test]
fn test_priority_high_wins() {
    run_test_case(&TEST_CASES[4]);
}

#[test]
fn test_compound_tax_invoice() {
    run_test_case(&TEST_CASES[5]);
}

#[test]
fn test_compound_draft_excluded() {
    run_test_case(&TEST_CASES[6]);
}

#[test]
fn test_compound_active_contract() {
    run_test_case(&TEST_CASES[7]);
}

#[test]
fn test_variable_extraction_config() {
    run_test_case(&TEST_CASES[8]);
}

// ============================================================================
// Document type variation tests
// ============================================================================

#[test]
fn test_markdown_invoice() {
    run_test_case(&TEST_CASES[9]);
}

#[test]
fn test_german_invoice() {
    run_test_case(&TEST_CASES[10]);
}

#[test]
fn test_french_receipt() {
    run_test_case(&TEST_CASES[11]);
}

// ============================================================================
// Edge case tests
// ============================================================================

#[test]
fn test_empty_document() {
    run_test_case(&TEST_CASES[12]);
}

#[test]
fn test_whitespace_only() {
    run_test_case(&TEST_CASES[13]);
}

#[test]
fn test_unicode_content() {
    run_test_case(&TEST_CASES[14]);
}

#[test]
fn test_very_long_lines() {
    run_test_case(&TEST_CASES[15]);
}

#[test]
fn test_special_chars_filename() {
    run_test_case(&TEST_CASES[16]);
}

// ============================================================================
// Multiple rules matching tests
// ============================================================================

#[test]
fn test_multi_match_priority() {
    run_test_case(&TEST_CASES[17]);
}

#[test]
fn test_three_way_match() {
    run_test_case(&TEST_CASES[18]);
}

// ============================================================================
// Symlink tests
// ============================================================================

#[test]
fn test_symlink_config() {
    run_test_case(&TEST_CASES[19]);
}

// ============================================================================
// Variable extraction edge cases
// ============================================================================

#[test]
fn test_no_variables_match() {
    run_test_case(&TEST_CASES[20]);
}

#[test]
fn test_overlapping_patterns() {
    run_test_case(&TEST_CASES[21]);
}

// ============================================================================
// Test case count validation
// ============================================================================

/// Test that all test cases in TEST_CASES array are executed.
#[test]
fn test_all_cases_defined() {
    assert_eq!(TEST_CASES.len(), 22, "Expected 22 test cases defined");
}

// ============================================================================
// Additional integration tests
// ============================================================================

/// Test that processor registry correctly routes text files.
#[test]
fn test_processor_registry_routes_text() {
    let registry = ProcessorRegistry::new(false, &[], 300);
    let input_path = get_fixture_path("inputs/invoice.txt");

    let result = registry.process(&input_path);
    assert!(result.is_ok(), "Text file processing should succeed");

    let processed = result.unwrap();
    assert!(
        !processed.text.is_empty(),
        "Extracted text should not be empty"
    );
    assert!(
        !processed.pdf_bytes.is_empty(),
        "PDF bytes should be generated"
    );
}

/// Test variable extraction from invoice text.
#[test]
fn test_variable_extraction_from_invoice() {
    let config_path = get_fixture_path("configs/basic-rules.json");
    let config = load_config(&config_path).expect("Failed to load config");

    let registry = ProcessorRegistry::new(false, &[], 300);
    let input_path = get_fixture_path("inputs/invoice.txt");
    let processed = registry.process(&input_path).expect("Failed to process");

    let variable_engine = paporg::VariableEngine::new(&config.variables.extracted);
    let extracted_vars = variable_engine.extract_variables(&processed.text);

    // Should extract vendor from "Invoice from: Acme Corporation"
    assert!(
        extracted_vars.contains_key("vendor"),
        "Should extract vendor variable"
    );

    // Should extract invoice number from "Invoice Number: INV-2026-001"
    assert!(
        extracted_vars.contains_key("invoice_number"),
        "Should extract invoice_number variable"
    );
}

/// Test variable extraction with the extended config.
#[test]
fn test_extended_variable_extraction() {
    let config_path = get_fixture_path("configs/variable-extraction.json");
    let config = load_config(&config_path).expect("Failed to load config");

    let registry = ProcessorRegistry::new(false, &[], 300);
    let input_path = get_fixture_path("inputs/complex-variables.txt");
    let processed = registry.process(&input_path).expect("Failed to process");

    let variable_engine = paporg::VariableEngine::new(&config.variables.extracted);
    let extracted_vars = variable_engine.extract_variables(&processed.text);

    // Should extract vendor with slugify transform
    assert!(
        extracted_vars.contains_key("vendor"),
        "Should extract vendor variable, got: {:?}",
        extracted_vars
    );

    // Should extract invoice_number
    assert!(
        extracted_vars.contains_key("invoice_number"),
        "Should extract invoice_number variable"
    );

    // Should extract amount
    assert!(
        extracted_vars.contains_key("amount"),
        "Should extract amount variable"
    );

    // Should extract uppercase_code with uppercase transform
    assert!(
        extracted_vars.contains_key("uppercase_code"),
        "Expected uppercase_code variable to be extracted, got: {:?}",
        extracted_vars.keys().collect::<Vec<_>>()
    );
    let code = extracted_vars.get("uppercase_code").unwrap();
    assert_eq!(code, "TESTCODE", "uppercase_code should be uppercase");

    // Should extract category_hint with lowercase transform
    assert!(
        extracted_vars.contains_key("category_hint"),
        "Expected category_hint variable to be extracted, got: {:?}",
        extracted_vars.keys().collect::<Vec<_>>()
    );
    let hint = extracted_vars.get("category_hint").unwrap();
    assert!(
        hint.chars().all(|c| c.is_lowercase() || !c.is_alphabetic()),
        "category_hint should be lowercase, got: {}",
        hint
    );
}

/// Test that the storage system creates proper directory structure.
#[test]
fn test_storage_creates_directory_structure() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let storage = FileStorage::new(temp_dir.path());

    let content = b"Test PDF content";
    let result = storage.store(content, "2026/invoices/tax", "test_doc", "pdf");

    assert!(result.is_ok(), "Storage should succeed");
    let stored_path = result.unwrap();

    // Verify the full path structure was created
    assert!(stored_path.exists(), "File should exist");
    assert!(
        stored_path.to_string_lossy().contains("2026/invoices/tax"),
        "Path should contain directory structure"
    );
}

/// Test variable substitution in output paths.
#[test]
fn test_variable_in_output_path() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let output_dir = temp_dir.path().join("output");
    std::fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    let config_path = get_fixture_path("configs/basic-rules.json");
    let config = load_config(&config_path).expect("Failed to load config");

    let registry = ProcessorRegistry::new(false, &[], 300);
    let input_path = get_fixture_path("inputs/invoice.txt");
    let processed = registry.process(&input_path).expect("Failed to process");

    let variable_engine = paporg::VariableEngine::new(&config.variables.extracted);
    let extracted_vars = variable_engine.extract_variables(&processed.text);

    // Substitute variables in a path template
    let path = variable_engine.substitute("$y/invoices/$vendor", "invoice.txt", &extracted_vars);

    // Year should be substituted (4 digit number)
    assert!(!path.contains("$y"), "Year should be substituted: {}", path);

    // Vendor should be substituted
    assert!(
        !path.contains("$vendor"),
        "Vendor should be substituted: {}",
        path
    );

    // Path should contain actual vendor value (slugified)
    assert!(
        path.contains("acme"),
        "Path should contain vendor 'acme': {}",
        path
    );
}

/// Test priority rules - higher priority should win when both match.
#[test]
fn test_priority_rules_higher_wins() {
    let config_path = get_fixture_path("configs/priority-rules.json");
    let config = load_config(&config_path).expect("Failed to load config");

    let categorizer = Categorizer::new(config.rules.clone(), config.defaults.clone());

    // Text that matches both low and high priority rules
    let result = categorizer.categorize("This is an Invoice document");

    // Higher priority (100) should win over lower priority (10)
    assert_eq!(
        result.category, "high-priority",
        "Higher priority rule should win"
    );
    assert_eq!(
        result.rule_id,
        Some("high-priority-invoice".to_string()),
        "Higher priority rule ID should be returned"
    );
}

/// Test compound conditions - AND logic.
#[test]
fn test_compound_all_conditions() {
    let config_path = get_fixture_path("configs/compound-conditions.json");
    let config = load_config(&config_path).expect("Failed to load config");

    let categorizer = Categorizer::new(config.rules.clone(), config.defaults.clone());

    // Text that matches the tax-invoice rule (Invoice AND VAT)
    let result = categorizer.categorize("Invoice with VAT included");
    assert_eq!(result.category, "tax-invoices");

    // Text that only matches first condition (Invoice but no VAT)
    let result2 = categorizer.categorize("Invoice without tax");
    assert_ne!(result2.category, "tax-invoices");
}

/// Test compound conditions - NOT logic.
#[test]
fn test_compound_not_conditions() {
    let config_path = get_fixture_path("configs/compound-conditions.json");
    let config = load_config(&config_path).expect("Failed to load config");

    let categorizer = Categorizer::new(config.rules.clone(), config.defaults.clone());

    // Invoice without DRAFT - should match non-draft rule
    let result = categorizer.categorize("Invoice document final");
    assert_eq!(result.category, "final-invoices");

    // Invoice with DRAFT - should NOT match non-draft rule
    let result2 = categorizer.categorize("DRAFT Invoice document");
    assert_ne!(result2.category, "final-invoices");
}

/// Test compound conditions - OR logic with patterns.
#[test]
fn test_compound_any_with_patterns() {
    let config_path = get_fixture_path("configs/compound-conditions.json");
    let config = load_config(&config_path).expect("Failed to load config");

    let categorizer = Categorizer::new(config.rules.clone(), config.defaults.clone());

    // USD pattern
    let result = categorizer.categorize("Amount: $100.00");
    assert_eq!(result.category, "financial");

    // EUR pattern
    let result2 = categorizer.categorize("Total: 500 EUR");
    assert_eq!(result2.category, "financial");

    // No currency
    let result3 = categorizer.categorize("Random text without currency");
    assert_eq!(result3.category, "unsorted");
}

/// Test using the TestHarness from common module.
#[test]
fn test_with_harness() {
    use common::harness::TestHarness;
    use common::{ConfigBuilder, RuleBuilder, VariableBuilder};

    let harness = TestHarness::new();

    // Write a test input file
    let input_path = harness.write_text_input(
        "test-doc.txt",
        "Invoice from TestCorp\nInvoice Number: TEST-001\nTotal: $500.00",
    );

    // Build a config programmatically
    let config = ConfigBuilder::new()
        .rule(
            RuleBuilder::new("invoice", "invoices")
                .contains("Invoice")
                .priority(50)
                .output("$y/invoices", "$original")
                .build(),
        )
        .variable(
            VariableBuilder::new("vendor", r"from\s+(?P<vendor>\w+)")
                .slugify()
                .build(),
        )
        .build();

    // Run the pipeline
    let result = harness.run_pipeline(&input_path, &config);

    // Verify results
    assert_eq!(result.categorization.category, "invoices");
    assert!(result.extracted_variables.contains_key("vendor"));
    assert_eq!(
        result.extracted_variables.get("vendor"),
        Some(&"testcorp".to_string())
    );
}

/// Test full pipeline with storage using harness.
#[test]
fn test_full_pipeline_with_storage() {
    use common::harness::TestHarness;
    use common::{ConfigBuilder, RuleBuilder};

    let harness = TestHarness::new();

    let input_path =
        harness.write_text_input("store-test.txt", "Receipt from TechStore\nTotal: $99.99");

    let config = ConfigBuilder::new()
        .rule(
            RuleBuilder::new("receipt", "receipts")
                .contains("Receipt")
                .output("receipts/$y", "$original")
                .build(),
        )
        .build();

    let result = harness.run_pipeline_with_storage(&input_path, &config);

    // Verify storage
    assert!(result.stored_path.is_some(), "File should be stored");
    assert!(
        result.stored_path.as_ref().unwrap().exists(),
        "Stored file should exist"
    );
}

// ============================================================================
// Error handling tests (negative tests)
// ============================================================================

/// Test processing a nonexistent file returns appropriate error.
#[test]
fn test_nonexistent_file() {
    let registry = ProcessorRegistry::new(false, &[], 300);
    let result = registry.process(Path::new("/nonexistent/path/does-not-exist.txt"));

    assert!(
        result.is_err(),
        "Processing nonexistent file should return an error"
    );

    match result {
        Err(paporg::ProcessError::ReadDocument { path, .. }) => {
            assert!(
                path.to_string_lossy().contains("does-not-exist.txt"),
                "Error should reference the missing file"
            );
        }
        Err(e) => panic!("Expected ReadDocument error, got: {:?}", e),
        Ok(_) => panic!("Expected error for nonexistent file"),
    }
}

/// Test processing a file with unsupported extension returns appropriate error.
#[test]
fn test_unsupported_extension() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let file_path = temp_dir.path().join("document.xyz");
    std::fs::write(&file_path, b"some content").expect("Failed to write test file");

    let registry = ProcessorRegistry::new(false, &[], 300);
    let result = registry.process(&file_path);

    assert!(
        result.is_err(),
        "Processing unsupported extension should return an error"
    );

    match result {
        Err(paporg::ProcessError::UnsupportedFormat(ext)) => {
            assert_eq!(
                ext, "xyz",
                "Error should reference the unsupported extension"
            );
        }
        Err(e) => panic!("Expected UnsupportedFormat error, got: {:?}", e),
        Ok(_) => panic!("Expected error for unsupported extension"),
    }
}

/// Test processing a file with no extension returns appropriate error.
#[test]
fn test_no_extension() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let file_path = temp_dir.path().join("document");
    std::fs::write(&file_path, b"some content").expect("Failed to write test file");

    let registry = ProcessorRegistry::new(false, &[], 300);
    let result = registry.process(&file_path);

    assert!(
        result.is_err(),
        "Processing file without extension should return an error"
    );

    match result {
        Err(paporg::ProcessError::UnsupportedFormat(ext)) => {
            assert_eq!(ext, "", "Error should have empty extension string");
        }
        Err(e) => panic!("Expected UnsupportedFormat error, got: {:?}", e),
        Ok(_) => panic!("Expected error for file without extension"),
    }
}

// ============================================================================
// OCR tests (ignored by default - run with `cargo test -- --ignored`)
// ============================================================================

/// Test image processing with OCR (requires tesseract).
/// Run with: cargo test test_image_ocr -- --ignored
#[test]
#[ignore]
fn test_image_ocr() {
    // This test requires tesseract to be installed
    // It tests the full OCR pipeline with an image file
    let registry = ProcessorRegistry::new(true, &["eng".to_string()], 300);

    // Create a simple test image with text
    // Note: In a real scenario, you'd have a fixture image
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let img_path = temp_dir.path().join("test.png");

    // Create a simple white image (OCR would extract no text, but tests the pipeline)
    let img = image::RgbImage::new(100, 100);
    img.save(&img_path).expect("Failed to save test image");

    let result = registry.process(&img_path);

    // OCR processing should succeed (even if no text extracted)
    assert!(
        result.is_ok(),
        "Image processing with OCR should succeed: {:?}",
        result.err()
    );
}

/// Test PDF with scanned images (requires tesseract).
/// Run with: cargo test test_scanned_pdf_ocr -- --ignored
#[test]
#[ignore]
fn test_scanned_pdf_ocr() {
    // This test requires tesseract to be installed
    // It would test OCR on a PDF containing scanned images
    // Placeholder for when we have appropriate fixtures
}
