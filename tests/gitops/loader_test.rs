//! Integration tests for the GitOps config loader.

use std::path::PathBuf;
use paporg::gitops::{ConfigLoader, ConfigValidator, ResourceKind};

fn fixtures_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gitops/valid")
}

#[test]
fn test_load_valid_config() {
    let loader = ConfigLoader::new(fixtures_path());
    let config = loader.load().expect("Should load valid config");

    // Check settings
    assert_eq!(config.settings.resource.metadata.name, "default");
    assert_eq!(config.settings.resource.spec.input_directory, "/data/inbox");
    assert_eq!(config.settings.resource.spec.worker_count, 4);
    assert!(config.settings.resource.spec.ocr.enabled);

    // Check variables
    assert_eq!(config.variables.len(), 2);
    let vendor = config.variables.iter().find(|v| v.resource.metadata.name == "vendor");
    assert!(vendor.is_some());

    // Check rules (should be sorted by priority descending)
    assert_eq!(config.rules.len(), 3);
    assert_eq!(config.rules[0].resource.spec.priority, 100); // tax-invoices
    assert_eq!(config.rules[1].resource.spec.priority, 50);  // general-invoices
    assert_eq!(config.rules[2].resource.spec.priority, 30);  // retail-receipts
}

#[test]
fn test_validate_valid_config() {
    let loader = ConfigLoader::new(fixtures_path());
    let config = loader.load().expect("Should load valid config");

    let mut validator = ConfigValidator::new();
    let result = validator.validate(&config);

    assert!(result.is_ok(), "Validation errors: {:?}", validator.errors());
}

#[test]
fn test_to_legacy_config() {
    let loader = ConfigLoader::new(fixtures_path());
    let config = loader.load().expect("Should load valid config");
    let legacy = config.to_legacy_config();

    assert_eq!(legacy.input_directory, "/data/inbox");
    assert_eq!(legacy.output_directory, "/data/documents");
    assert_eq!(legacy.worker_count, 4);
    assert_eq!(legacy.variables.extracted.len(), 2);
    assert_eq!(legacy.rules.len(), 3);
}

#[test]
fn test_get_file_tree() {
    let loader = ConfigLoader::new(fixtures_path());
    let tree = loader.get_file_tree().expect("Should get file tree");

    assert!(tree.is_directory);

    // Should have settings.yaml at root
    let settings = tree.children.iter().find(|c| c.name == "settings.yaml");
    assert!(settings.is_some());
    let settings = settings.unwrap();
    assert!(!settings.is_directory);
    assert!(settings.resource.is_some());
    assert_eq!(settings.resource.as_ref().unwrap().kind, ResourceKind::Settings);

    // Should have variables directory
    let variables = tree.children.iter().find(|c| c.name == "variables");
    assert!(variables.is_some());
    assert!(variables.unwrap().is_directory);

    // Should have rules directory
    let rules = tree.children.iter().find(|c| c.name == "rules");
    assert!(rules.is_some());
    assert!(rules.unwrap().is_directory);
}

#[test]
fn test_default_path_for_resource() {
    let loader = ConfigLoader::new(".");

    let settings_path = loader.default_path_for_resource(ResourceKind::Settings, "default");
    assert_eq!(settings_path, PathBuf::from("settings.yaml"));

    let var_path = loader.default_path_for_resource(ResourceKind::Variable, "vendor");
    assert_eq!(var_path, PathBuf::from("variables/vendor.yaml"));

    let rule_path = loader.default_path_for_resource(ResourceKind::Rule, "my-rule");
    assert_eq!(rule_path, PathBuf::from("rules/my-rule.yaml"));
}

#[test]
fn test_parse_invalid_api_version() {
    let loader = ConfigLoader::new(".");
    let yaml = r#"
apiVersion: wrong/v1
kind: Rule
metadata:
  name: test
spec:
  priority: 0
  category: Test
  match:
    contains: test
  output:
    directory: Test
    filename: test
"#;
    let result = loader.parse_resource(yaml, std::path::Path::new("test.yaml"));
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(err.to_string().contains("Invalid API version"));
}

#[test]
fn test_all_resources_method() {
    let loader = ConfigLoader::new(fixtures_path());
    let config = loader.load().expect("Should load valid config");

    let all = config.all_resources();

    // Should have 1 settings + 2 variables + 3 rules = 6 resources
    assert_eq!(all.len(), 6);

    // First should be settings
    assert_eq!(all[0].0, &ResourceKind::Settings);
    assert_eq!(all[0].1, "default");
}
