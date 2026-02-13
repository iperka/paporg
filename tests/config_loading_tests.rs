//! Table-driven tests for configuration loading and validation.
//!
//! Tests cover both JSON config files and GitOps YAML configurations.

mod common;

use std::path::PathBuf;

use paporg::config::load_config_from_str;
use paporg::gitops::ConfigLoader;

/// Represents a single config loading test case.
struct ConfigTestCase {
    /// Test case name for identification.
    name: &'static str,
    /// The config JSON content to test.
    config_json: &'static str,
    /// Whether loading should succeed.
    should_succeed: bool,
    /// Expected error substring (if should_succeed is false).
    expected_error: Option<&'static str>,
}

/// All JSON config loading test cases.
const JSON_CONFIG_TESTS: &[ConfigTestCase] = &[
    ConfigTestCase {
        name: "valid_minimal",
        config_json: r#"{
            "version": "1.0",
            "input_directory": "/input",
            "output_directory": "/output",
            "rules": [],
            "defaults": {
                "output": { "directory": "$y/unsorted", "filename": "$original" }
            }
        }"#,
        should_succeed: true,
        expected_error: None,
    },
    ConfigTestCase {
        name: "valid_full",
        config_json: r#"{
            "version": "1.0",
            "input_directory": "/input",
            "output_directory": "/output",
            "worker_count": 8,
            "ocr": {
                "enabled": true,
                "languages": ["eng", "deu"],
                "dpi": 400
            },
            "variables": {
                "extracted": [
                    {
                        "name": "vendor",
                        "pattern": "from[:\\s]+(?P<vendor>[A-Za-z]+)",
                        "transform": "slugify",
                        "default": "unknown"
                    }
                ]
            },
            "rules": [
                {
                    "id": "invoice",
                    "name": "Invoices",
                    "priority": 100,
                    "match": { "contains": "Invoice" },
                    "category": "invoices",
                    "output": { "directory": "$y/invoices", "filename": "$vendor" },
                    "symlinks": [{ "target": "latest/$vendor" }]
                }
            ],
            "defaults": {
                "output": { "directory": "$y/unsorted", "filename": "$original_$timestamp" }
            }
        }"#,
        should_succeed: true,
        expected_error: None,
    },
    ConfigTestCase {
        name: "valid_with_contains_any",
        config_json: r#"{
            "version": "1.0",
            "input_directory": "/input",
            "output_directory": "/output",
            "rules": [
                {
                    "id": "bills",
                    "name": "Bills",
                    "match": { "containsAny": ["Invoice", "Rechnung", "Bill"] },
                    "category": "bills",
                    "output": { "directory": "bills", "filename": "$original" }
                }
            ],
            "defaults": {
                "output": { "directory": "unsorted", "filename": "$original" }
            }
        }"#,
        should_succeed: true,
        expected_error: None,
    },
    ConfigTestCase {
        name: "valid_with_contains_all",
        config_json: r#"{
            "version": "1.0",
            "input_directory": "/input",
            "output_directory": "/output",
            "rules": [
                {
                    "id": "tax-invoice",
                    "name": "Tax Invoices",
                    "match": { "containsAll": ["Invoice", "VAT"] },
                    "category": "tax",
                    "output": { "directory": "tax", "filename": "$original" }
                }
            ],
            "defaults": {
                "output": { "directory": "unsorted", "filename": "$original" }
            }
        }"#,
        should_succeed: true,
        expected_error: None,
    },
    ConfigTestCase {
        name: "valid_with_pattern",
        config_json: r#"{
            "version": "1.0",
            "input_directory": "/input",
            "output_directory": "/output",
            "rules": [
                {
                    "id": "numbered",
                    "name": "Numbered Invoice",
                    "match": { "pattern": "INV-\\d{4,}" },
                    "category": "invoices",
                    "output": { "directory": "invoices", "filename": "$original" }
                }
            ],
            "defaults": {
                "output": { "directory": "unsorted", "filename": "$original" }
            }
        }"#,
        should_succeed: true,
        expected_error: None,
    },
    ConfigTestCase {
        name: "valid_compound_all",
        config_json: r#"{
            "version": "1.0",
            "input_directory": "/input",
            "output_directory": "/output",
            "rules": [
                {
                    "id": "tax-invoice",
                    "name": "Tax Invoice",
                    "match": {
                        "all": [
                            { "containsAny": ["Invoice", "Rechnung"] },
                            { "containsAny": ["VAT", "MwSt"] }
                        ]
                    },
                    "category": "tax",
                    "output": { "directory": "tax", "filename": "$original" }
                }
            ],
            "defaults": {
                "output": { "directory": "unsorted", "filename": "$original" }
            }
        }"#,
        should_succeed: true,
        expected_error: None,
    },
    ConfigTestCase {
        name: "valid_compound_not",
        config_json: r#"{
            "version": "1.0",
            "input_directory": "/input",
            "output_directory": "/output",
            "rules": [
                {
                    "id": "non-draft",
                    "name": "Non-Draft",
                    "match": {
                        "all": [
                            { "contains": "invoice" },
                            { "not": { "contains": "DRAFT" } }
                        ]
                    },
                    "category": "final",
                    "output": { "directory": "final", "filename": "$original" }
                }
            ],
            "defaults": {
                "output": { "directory": "unsorted", "filename": "$original" }
            }
        }"#,
        should_succeed: true,
        expected_error: None,
    },
    ConfigTestCase {
        name: "invalid_version",
        config_json: r#"{
            "version": "2.0",
            "input_directory": "/input",
            "output_directory": "/output",
            "rules": [],
            "defaults": {
                "output": { "directory": "$y/unsorted", "filename": "$original" }
            }
        }"#,
        should_succeed: false,
        expected_error: Some("Unsupported config version"),
    },
    ConfigTestCase {
        name: "invalid_regex_in_rule",
        config_json: r#"{
            "version": "1.0",
            "input_directory": "/input",
            "output_directory": "/output",
            "rules": [
                {
                    "id": "bad-regex",
                    "name": "Bad Regex",
                    "match": { "pattern": "[invalid" },
                    "category": "test",
                    "output": { "directory": "test", "filename": "test" }
                }
            ],
            "defaults": {
                "output": { "directory": "unsorted", "filename": "$original" }
            }
        }"#,
        should_succeed: false,
        expected_error: Some("Invalid regex pattern"),
    },
    ConfigTestCase {
        name: "invalid_regex_in_variable",
        config_json: r#"{
            "version": "1.0",
            "input_directory": "/input",
            "output_directory": "/output",
            "variables": {
                "extracted": [
                    {
                        "name": "test",
                        "pattern": "(?P<test>[invalid"
                    }
                ]
            },
            "rules": [],
            "defaults": {
                "output": { "directory": "unsorted", "filename": "$original" }
            }
        }"#,
        should_succeed: false,
        expected_error: Some("Invalid variable pattern"),
    },
    ConfigTestCase {
        name: "missing_capture_group",
        config_json: r#"{
            "version": "1.0",
            "input_directory": "/input",
            "output_directory": "/output",
            "variables": {
                "extracted": [
                    {
                        "name": "vendor",
                        "pattern": "from[:\\s]+([A-Za-z]+)"
                    }
                ]
            },
            "rules": [],
            "defaults": {
                "output": { "directory": "unsorted", "filename": "$original" }
            }
        }"#,
        should_succeed: false,
        expected_error: Some("named capture group"),
    },
    ConfigTestCase {
        name: "duplicate_rule_ids",
        config_json: r#"{
            "version": "1.0",
            "input_directory": "/input",
            "output_directory": "/output",
            "rules": [
                {
                    "id": "duplicate",
                    "name": "Rule 1",
                    "match": { "contains": "test1" },
                    "category": "cat1",
                    "output": { "directory": "dir1", "filename": "file1" }
                },
                {
                    "id": "duplicate",
                    "name": "Rule 2",
                    "match": { "contains": "test2" },
                    "category": "cat2",
                    "output": { "directory": "dir2", "filename": "file2" }
                }
            ],
            "defaults": {
                "output": { "directory": "unsorted", "filename": "$original" }
            }
        }"#,
        should_succeed: false,
        expected_error: Some("Duplicate rule ID"),
    },
    ConfigTestCase {
        name: "missing_required_field",
        config_json: r#"{
            "version": "1.0",
            "output_directory": "/output",
            "rules": [],
            "defaults": {
                "output": { "directory": "unsorted", "filename": "$original" }
            }
        }"#,
        should_succeed: false,
        expected_error: Some("input_directory"),
    },
    ConfigTestCase {
        name: "invalid_json",
        config_json: r#"{ invalid json }"#,
        should_succeed: false,
        expected_error: Some("Failed to parse config JSON"),
    },
    ConfigTestCase {
        name: "empty_rules_array",
        config_json: r#"{
            "version": "1.0",
            "input_directory": "/input",
            "output_directory": "/output",
            "rules": [],
            "defaults": {
                "output": { "directory": "unsorted", "filename": "$original" }
            }
        }"#,
        should_succeed: true,
        expected_error: None,
    },
    ConfigTestCase {
        name: "valid_variable_with_default",
        config_json: r#"{
            "version": "1.0",
            "input_directory": "/input",
            "output_directory": "/output",
            "variables": {
                "extracted": [
                    {
                        "name": "vendor",
                        "pattern": "from[:\\s]+(?P<vendor>[A-Za-z]+)",
                        "default": "unknown"
                    }
                ]
            },
            "rules": [],
            "defaults": {
                "output": { "directory": "unsorted", "filename": "$original" }
            }
        }"#,
        should_succeed: true,
        expected_error: None,
    },
    ConfigTestCase {
        name: "valid_all_transforms",
        config_json: r#"{
            "version": "1.0",
            "input_directory": "/input",
            "output_directory": "/output",
            "variables": {
                "extracted": [
                    { "name": "slug", "pattern": "(?P<slug>\\w+)", "transform": "slugify" },
                    { "name": "upper", "pattern": "(?P<upper>\\w+)", "transform": "uppercase" },
                    { "name": "lower", "pattern": "(?P<lower>\\w+)", "transform": "lowercase" },
                    { "name": "trimmed", "pattern": "(?P<trimmed>\\w+)", "transform": "trim" }
                ]
            },
            "rules": [],
            "defaults": {
                "output": { "directory": "unsorted", "filename": "$original" }
            }
        }"#,
        should_succeed: true,
        expected_error: None,
    },
];

#[test]
fn test_json_config_loading() {
    for test_case in JSON_CONFIG_TESTS {
        let result = load_config_from_str(test_case.config_json);

        if test_case.should_succeed {
            assert!(
                result.is_ok(),
                "Test '{}': Expected success but got error: {:?}",
                test_case.name,
                result.err()
            );
        } else {
            assert!(
                result.is_err(),
                "Test '{}': Expected error but got success",
                test_case.name
            );

            if let Some(expected_error) = test_case.expected_error {
                let error_msg = result.err().unwrap().to_string();
                assert!(
                    error_msg.contains(expected_error),
                    "Test '{}': Expected error containing '{}', got '{}'",
                    test_case.name,
                    expected_error,
                    error_msg
                );
            }
        }
    }
}

// Removed brittle test_json_config_test_count - tests are validated by test_json_config_loading

/// GitOps config test cases.
struct GitOpsTestCase {
    name: &'static str,
    fixture_path: &'static str,
    should_succeed: bool,
    expected_error: Option<&'static str>,
}

const GITOPS_TESTS: &[GitOpsTestCase] = &[
    GitOpsTestCase {
        name: "gitops_valid_full",
        fixture_path: "valid",
        should_succeed: true,
        expected_error: None,
    },
    GitOpsTestCase {
        name: "gitops_invalid_bad_yaml",
        fixture_path: "invalid-yaml",
        should_succeed: false,
        expected_error: Some("yaml"),
    },
    GitOpsTestCase {
        name: "gitops_invalid_missing_version",
        fixture_path: "missing-version",
        should_succeed: false,
        expected_error: Some("version"),
    },
    GitOpsTestCase {
        name: "gitops_invalid_bad_rule",
        fixture_path: "bad-rule",
        should_succeed: false,
        expected_error: Some("match"),
    },
];

fn fixtures_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gitops")
}

#[test]
fn test_gitops_config_loading() {
    for test_case in GITOPS_TESTS {
        let loader = ConfigLoader::new(fixtures_path().join(test_case.fixture_path));
        let result = loader.load();

        if test_case.should_succeed {
            assert!(
                result.is_ok(),
                "GitOps test '{}': Expected success but got error: {:?}",
                test_case.name,
                result.err()
            );

            // Verify the config can be converted to legacy format
            let config = result.unwrap();
            let legacy = config.to_legacy_config();
            assert_eq!(legacy.version, "1.0");
        } else {
            assert!(
                result.is_err(),
                "GitOps test '{}': Expected error but got success",
                test_case.name
            );

            if let Some(expected_error) = test_case.expected_error {
                let error_msg = result.err().unwrap().to_string();
                assert!(
                    error_msg.contains(expected_error),
                    "GitOps test '{}': Expected error containing '{}', got '{}'",
                    test_case.name,
                    expected_error,
                    error_msg
                );
            }
        }
    }
}

/// Test that a valid config has correct field values.
#[test]
fn test_valid_config_field_values() {
    let config_json = r#"{
        "version": "1.0",
        "input_directory": "/data/inbox",
        "output_directory": "/data/documents",
        "worker_count": 4,
        "ocr": {
            "enabled": true,
            "languages": ["eng", "deu", "fra"],
            "dpi": 400
        },
        "rules": [],
        "defaults": {
            "output": { "directory": "unsorted", "filename": "$original" }
        }
    }"#;

    let config = load_config_from_str(config_json).expect("Should load valid config");

    assert_eq!(config.version, "1.0");
    assert_eq!(config.input_directory, "/data/inbox");
    assert_eq!(config.output_directory, "/data/documents");
    assert_eq!(config.worker_count, 4);
    assert!(config.ocr.enabled);
    assert_eq!(config.ocr.languages, vec!["eng", "deu", "fra"]);
    assert_eq!(config.ocr.dpi, 400);
}

/// Test config defaults are applied correctly.
#[test]
fn test_config_defaults_applied() {
    let config_json = r#"{
        "version": "1.0",
        "input_directory": "/input",
        "output_directory": "/output",
        "rules": [],
        "defaults": {
            "output": { "directory": "unsorted", "filename": "$original" }
        }
    }"#;

    let config = load_config_from_str(config_json).expect("Should load config");

    // worker_count should default to num_cpus
    assert!(config.worker_count >= 1);

    // OCR should have defaults
    assert!(config.ocr.enabled); // default is true
    assert_eq!(config.ocr.dpi, 300); // default DPI
}

/// Test rule priority ordering.
#[test]
fn test_rules_have_priority() {
    let config_json = r#"{
        "version": "1.0",
        "input_directory": "/input",
        "output_directory": "/output",
        "rules": [
            {
                "id": "low",
                "name": "Low Priority",
                "priority": 10,
                "match": { "contains": "test" },
                "category": "low",
                "output": { "directory": "low", "filename": "low" }
            },
            {
                "id": "high",
                "name": "High Priority",
                "priority": 100,
                "match": { "contains": "test" },
                "category": "high",
                "output": { "directory": "high", "filename": "high" }
            }
        ],
        "defaults": {
            "output": { "directory": "unsorted", "filename": "$original" }
        }
    }"#;

    let config = load_config_from_str(config_json).expect("Should load config");

    assert_eq!(config.rules.len(), 2);
    // Rules should be loaded with their priorities intact
    assert!(config.rules.iter().any(|r| r.priority == 10));
    assert!(config.rules.iter().any(|r| r.priority == 100));
}

/// Test symlinks configuration.
#[test]
fn test_symlinks_parsed() {
    let config_json = r#"{
        "version": "1.0",
        "input_directory": "/input",
        "output_directory": "/output",
        "rules": [
            {
                "id": "with-symlinks",
                "name": "With Symlinks",
                "match": { "contains": "test" },
                "category": "test",
                "output": { "directory": "test", "filename": "test" },
                "symlinks": [
                    { "target": "latest/test" },
                    { "target": "by-vendor/$vendor" }
                ]
            }
        ],
        "defaults": {
            "output": { "directory": "unsorted", "filename": "$original" }
        }
    }"#;

    let config = load_config_from_str(config_json).expect("Should load config");

    assert_eq!(config.rules[0].symlinks.len(), 2);
    assert_eq!(config.rules[0].symlinks[0].target, "latest/test");
    assert_eq!(config.rules[0].symlinks[1].target, "by-vendor/$vendor");
}

/// Test nested compound conditions.
#[test]
fn test_deeply_nested_compound_conditions() {
    let config_json = r#"{
        "version": "1.0",
        "input_directory": "/input",
        "output_directory": "/output",
        "rules": [
            {
                "id": "nested",
                "name": "Nested Conditions",
                "match": {
                    "all": [
                        {
                            "any": [
                                { "contains": "invoice" },
                                { "contains": "bill" }
                            ]
                        },
                        {
                            "not": {
                                "any": [
                                    { "contains": "draft" },
                                    { "contains": "void" }
                                ]
                            }
                        }
                    ]
                },
                "category": "final",
                "output": { "directory": "final", "filename": "final" }
            }
        ],
        "defaults": {
            "output": { "directory": "unsorted", "filename": "$original" }
        }
    }"#;

    let result = load_config_from_str(config_json);
    assert!(
        result.is_ok(),
        "Should parse deeply nested conditions: {:?}",
        result.err()
    );
}
