use std::path::Path;

use crate::config::schema::Config;
use crate::error::ConfigError;

const SCHEMA_JSON: &str = include_str!("../../../../schema/config-v1.json");

pub fn load_config<P: AsRef<Path>>(path: P) -> Result<Config, ConfigError> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path).map_err(|e| ConfigError::ReadFile {
        path: path.to_path_buf(),
        source: e,
    })?;

    load_config_from_str(&content)
}

pub fn load_config_from_str(content: &str) -> Result<Config, ConfigError> {
    let json_value: serde_json::Value = serde_json::from_str(content)?;

    validate_schema(&json_value)?;

    let config: Config = serde_json::from_value(json_value)?;

    validate_config(&config)?;

    Ok(config)
}

fn validate_schema(json_value: &serde_json::Value) -> Result<(), ConfigError> {
    let schema: serde_json::Value =
        serde_json::from_str(SCHEMA_JSON).map_err(|e| ConfigError::Validation {
            message: format!("Invalid embedded schema JSON: {}", e),
        })?;

    let compiled =
        jsonschema::JSONSchema::compile(&schema).map_err(|e| ConfigError::Validation {
            message: format!("Failed to compile JSON schema: {}", e),
        })?;

    let result = compiled.validate(json_value);
    if let Err(errors) = result {
        let error_messages: Vec<String> = errors
            .map(|e| format!("{} at {}", e, e.instance_path))
            .collect();
        return Err(ConfigError::SchemaValidation {
            errors: error_messages.join("; "),
        });
    }

    Ok(())
}

fn validate_config(config: &Config) -> Result<(), ConfigError> {
    // Validate version
    if config.version != "1.0" {
        return Err(ConfigError::Validation {
            message: format!("Unsupported config version: {}", config.version),
        });
    }

    // Validate extracted variable patterns
    for var in &config.variables.extracted {
        if let Err(e) = regex::Regex::new(&var.pattern) {
            return Err(ConfigError::InvalidPattern {
                name: var.name.clone(),
                reason: e.to_string(),
            });
        }

        // Check that pattern contains named capture group matching variable name
        if !var.pattern.contains(&format!("?P<{}>", var.name))
            && !var.pattern.contains(&format!("?<{}>", var.name))
        {
            return Err(ConfigError::InvalidPattern {
                name: var.name.clone(),
                reason: format!(
                    "Pattern must contain named capture group '?P<{}>' or '?<{}>'",
                    var.name, var.name
                ),
            });
        }
    }

    // Validate rules
    let mut rule_ids = std::collections::HashSet::new();
    for rule in &config.rules {
        if !rule_ids.insert(&rule.id) {
            return Err(ConfigError::InvalidRule {
                id: rule.id.clone(),
                reason: "Duplicate rule ID".to_string(),
            });
        }

        validate_match_condition(&rule.match_condition, &rule.id)?;
    }

    Ok(())
}

fn validate_match_condition(
    condition: &crate::config::schema::MatchCondition,
    rule_id: &str,
) -> Result<(), ConfigError> {
    use crate::config::schema::MatchCondition;

    match condition {
        MatchCondition::Compound(compound) => {
            if let Some(all) = &compound.all {
                for cond in all {
                    validate_match_condition(cond, rule_id)?;
                }
            }
            if let Some(any) = &compound.any {
                for cond in any {
                    validate_match_condition(cond, rule_id)?;
                }
            }
            if let Some(not) = &compound.not {
                validate_match_condition(not, rule_id)?;
            }
        }
        MatchCondition::Simple(simple) => {
            if let Some(pattern) = &simple.pattern {
                if let Err(e) = regex::Regex::new(pattern) {
                    return Err(ConfigError::InvalidRule {
                        id: rule_id.to_string(),
                        reason: format!("Invalid regex pattern: {}", e),
                    });
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_valid_config() {
        let config_json = r#"
        {
            "version": "1.0",
            "input_directory": "/input",
            "output_directory": "/output",
            "worker_count": 4,
            "rules": [],
            "defaults": {
                "output": {
                    "directory": "$y/unsorted",
                    "filename": "$original_$timestamp"
                }
            }
        }
        "#;

        let config = load_config_from_str(config_json).unwrap();
        assert_eq!(config.version, "1.0");
        assert_eq!(config.input_directory, "/input");
        assert_eq!(config.output_directory, "/output");
        assert_eq!(config.worker_count, 4);
    }

    #[test]
    fn test_load_config_with_rules() {
        let config_json = r#"
        {
            "version": "1.0",
            "input_directory": "/input",
            "output_directory": "/output",
            "rules": [
                {
                    "id": "test-rule",
                    "name": "Test Rule",
                    "priority": 100,
                    "match": {
                        "containsAny": ["invoice", "bill"]
                    },
                    "category": "invoices",
                    "output": {
                        "directory": "$y/invoices",
                        "filename": "$original"
                    }
                }
            ],
            "defaults": {
                "output": {
                    "directory": "$y/unsorted",
                    "filename": "$original"
                }
            }
        }
        "#;

        let config = load_config_from_str(config_json).unwrap();
        assert_eq!(config.rules.len(), 1);
        assert_eq!(config.rules[0].id, "test-rule");
        assert_eq!(config.rules[0].priority, 100);
    }

    #[test]
    fn test_load_config_with_extracted_variables() {
        let config_json = r#"
        {
            "version": "1.0",
            "input_directory": "/input",
            "output_directory": "/output",
            "variables": {
                "extracted": [
                    {
                        "name": "vendor",
                        "pattern": "(?i)from[:\\s]+(?P<vendor>[A-Za-z]+)",
                        "transform": "slugify",
                        "default": "unknown"
                    }
                ]
            },
            "rules": [],
            "defaults": {
                "output": {
                    "directory": "$y/unsorted",
                    "filename": "$original"
                }
            }
        }
        "#;

        let config = load_config_from_str(config_json).unwrap();
        assert_eq!(config.variables.extracted.len(), 1);
        assert_eq!(config.variables.extracted[0].name, "vendor");
    }

    #[test]
    fn test_invalid_version() {
        let config_json = r#"
        {
            "version": "2.0",
            "input_directory": "/input",
            "output_directory": "/output",
            "rules": [],
            "defaults": {
                "output": {
                    "directory": "$y/unsorted",
                    "filename": "$original"
                }
            }
        }
        "#;

        let result = load_config_from_str(config_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_regex_pattern() {
        let config_json = r#"
        {
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
                "output": {
                    "directory": "$y/unsorted",
                    "filename": "$original"
                }
            }
        }
        "#;

        let result = load_config_from_str(config_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_capture_group() {
        let config_json = r#"
        {
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
                "output": {
                    "directory": "$y/unsorted",
                    "filename": "$original"
                }
            }
        }
        "#;

        let result = load_config_from_str(config_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_rule_ids() {
        let config_json = r#"
        {
            "version": "1.0",
            "input_directory": "/input",
            "output_directory": "/output",
            "rules": [
                {
                    "id": "test-rule",
                    "name": "Test Rule 1",
                    "match": { "contains": "test" },
                    "category": "test",
                    "output": { "directory": "test", "filename": "test" }
                },
                {
                    "id": "test-rule",
                    "name": "Test Rule 2",
                    "match": { "contains": "test2" },
                    "category": "test2",
                    "output": { "directory": "test2", "filename": "test2" }
                }
            ],
            "defaults": {
                "output": {
                    "directory": "$y/unsorted",
                    "filename": "$original"
                }
            }
        }
        "#;

        let result = load_config_from_str(config_json);
        assert!(result.is_err());
    }
}
