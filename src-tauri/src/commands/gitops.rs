//! GitOps resource management commands.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use paporg::gitops::{AnyResource, ConfigLoader, ConfigValidator, FileTreeNode, ResourceKind};
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::RwLock;

use super::ApiResponse;
use crate::state::TauriAppState;

/// Maximum allowed length for regex patterns to prevent ReDoS attacks.
const MAX_REGEX_PATTERN_LENGTH: usize = 1000;

// ============================================================================
// Response Types
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceListResponse {
    pub kind: String,
    pub items: Vec<ResourceSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSummary {
    pub name: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceDetail {
    pub name: String,
    pub path: String,
    pub yaml: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulateRuleResponse {
    pub matches: bool,
    pub resolved_directory: String,
    pub resolved_filename: String,
    pub resolved_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_details: Option<String>,
}

// ============================================================================
// Request Types
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulateRuleRequest {
    pub match_type: String,
    pub match_value: serde_json::Value,
    pub category: String,
    pub output_directory: String,
    pub output_filename: String,
    pub ocr_text: String,
    pub filename: String,
}

// ============================================================================
// Commands
// ============================================================================

/// Get file tree.
#[tauri::command]
pub async fn get_file_tree(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
) -> Result<ApiResponse<FileTreeNode>, String> {
    let state = state.read().await;

    let config_dir = match &state.config_dir {
        Some(dir) => dir.clone(),
        None => return Ok(ApiResponse::err("No config directory set")),
    };

    let loader = ConfigLoader::new(&config_dir);
    match loader.get_file_tree() {
        Ok(tree) => Ok(ApiResponse::ok(tree)),
        Err(e) => Ok(ApiResponse::err(e.to_string())),
    }
}

/// List resources of a given kind.
#[tauri::command]
pub async fn list_gitops_resources(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    kind: String,
) -> Result<ApiResponse<ResourceListResponse>, String> {
    let state = state.read().await;

    let resource_kind = match kind.parse::<ResourceKind>() {
        Ok(k) => k,
        Err(_) => return Ok(ApiResponse::err(format!("Unknown resource kind: {}", kind))),
    };

    let config = match state.config() {
        Some(c) => c,
        None => {
            return Ok(ApiResponse::ok(ResourceListResponse {
                kind,
                items: vec![],
            }))
        }
    };

    let items = match resource_kind {
        ResourceKind::Settings => {
            vec![ResourceSummary {
                name: config.settings.resource.metadata.name.clone(),
                path: config.settings.path.to_string_lossy().to_string(),
                labels: if config.settings.resource.metadata.labels.is_empty() {
                    None
                } else {
                    Some(config.settings.resource.metadata.labels.clone())
                },
            }]
        }
        ResourceKind::Variable => config
            .variables
            .iter()
            .map(|v| ResourceSummary {
                name: v.resource.metadata.name.clone(),
                path: v.path.to_string_lossy().to_string(),
                labels: if v.resource.metadata.labels.is_empty() {
                    None
                } else {
                    Some(v.resource.metadata.labels.clone())
                },
            })
            .collect(),
        ResourceKind::Rule => config
            .rules
            .iter()
            .map(|r| ResourceSummary {
                name: r.resource.metadata.name.clone(),
                path: r.path.to_string_lossy().to_string(),
                labels: if r.resource.metadata.labels.is_empty() {
                    None
                } else {
                    Some(r.resource.metadata.labels.clone())
                },
            })
            .collect(),
        ResourceKind::ImportSource => config
            .import_sources
            .iter()
            .map(|s| ResourceSummary {
                name: s.resource.metadata.name.clone(),
                path: s.path.to_string_lossy().to_string(),
                labels: if s.resource.metadata.labels.is_empty() {
                    None
                } else {
                    Some(s.resource.metadata.labels.clone())
                },
            })
            .collect(),
    };

    Ok(ApiResponse::ok(ResourceListResponse { kind, items }))
}

/// Get a single resource.
#[tauri::command]
pub async fn get_gitops_resource(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    kind: String,
    name: String,
) -> Result<ApiResponse<ResourceDetail>, String> {
    let state = state.read().await;

    let resource_kind = match kind.parse::<ResourceKind>() {
        Ok(k) => k,
        Err(_) => return Ok(ApiResponse::err(format!("Unknown resource kind: {}", kind))),
    };

    let config = match state.config() {
        Some(c) => c,
        None => return Ok(ApiResponse::err("Configuration not loaded")),
    };

    let (yaml_result, path): (Option<Result<String, _>>, PathBuf) = match resource_kind {
        ResourceKind::Settings => {
            if config.settings.resource.metadata.name == name {
                (
                    Some(serde_yaml::to_string(&config.settings.resource)),
                    config.settings.path.clone(),
                )
            } else {
                (None, PathBuf::new())
            }
        }
        ResourceKind::Variable => {
            match config
                .variables
                .iter()
                .find(|v| v.resource.metadata.name == name)
            {
                Some(v) => (Some(serde_yaml::to_string(&v.resource)), v.path.clone()),
                None => (None, PathBuf::new()),
            }
        }
        ResourceKind::Rule => {
            match config
                .rules
                .iter()
                .find(|r| r.resource.metadata.name == name)
            {
                Some(r) => (Some(serde_yaml::to_string(&r.resource)), r.path.clone()),
                None => (None, PathBuf::new()),
            }
        }
        ResourceKind::ImportSource => {
            match config
                .import_sources
                .iter()
                .find(|s| s.resource.metadata.name == name)
            {
                Some(s) => (Some(serde_yaml::to_string(&s.resource)), s.path.clone()),
                None => (None, PathBuf::new()),
            }
        }
    };

    match yaml_result {
        Some(Ok(yaml)) => Ok(ApiResponse::ok(ResourceDetail {
            name,
            path: path.to_string_lossy().to_string(),
            yaml,
        })),
        Some(Err(e)) => Ok(ApiResponse::err(format!(
            "YAML serialization failed: {}",
            e
        ))),
        None => Ok(ApiResponse::err(format!(
            "Resource not found: {}/{}",
            kind, name
        ))),
    }
}

/// Create a new resource.
#[tauri::command]
pub async fn create_gitops_resource(
    app: tauri::AppHandle,
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    kind: String,
    yaml: String,
    path: Option<String>,
) -> Result<ApiResponse<ResourceDetail>, String> {
    let state_guard = state.read().await;

    let resource_kind = match kind.parse::<ResourceKind>() {
        Ok(k) => k,
        Err(_) => return Ok(ApiResponse::err(format!("Unknown resource kind: {}", kind))),
    };

    let config_dir = match &state_guard.config_dir {
        Some(dir) => dir.clone(),
        None => return Ok(ApiResponse::err("No config directory set")),
    };

    let loader = ConfigLoader::new(&config_dir);

    // Parse the YAML
    let resource: AnyResource = match loader.parse_resource(&yaml, std::path::Path::new("new.yaml"))
    {
        Ok(r) => r,
        Err(e) => return Ok(ApiResponse::err(format!("Invalid YAML: {}", e))),
    };

    // Verify the kind matches
    if resource.kind() != resource_kind {
        return Ok(ApiResponse::err(format!(
            "Resource kind mismatch: expected {}, got {}",
            resource_kind,
            resource.kind()
        )));
    }

    // Check if resource already exists
    if let Some(config) = state_guard.config() {
        let exists = match resource_kind {
            ResourceKind::Settings => true,
            ResourceKind::Variable => config
                .variables
                .iter()
                .any(|v| v.resource.metadata.name == resource.name()),
            ResourceKind::Rule => config
                .rules
                .iter()
                .any(|r| r.resource.metadata.name == resource.name()),
            ResourceKind::ImportSource => config
                .import_sources
                .iter()
                .any(|s| s.resource.metadata.name == resource.name()),
        };

        if exists {
            return Ok(ApiResponse::err(format!(
                "Resource already exists: {}/{}",
                kind,
                resource.name()
            )));
        }
    }

    // Determine the path
    let file_path = path
        .map(PathBuf::from)
        .unwrap_or_else(|| loader.default_path_for_resource(resource_kind, resource.name()));

    // Write the resource
    if let Err(e) = loader.write_resource(&resource, &file_path) {
        return Ok(ApiResponse::err(format!("Failed to write resource: {}", e)));
    }

    // Need to reload - release read lock and acquire write lock
    drop(state_guard);
    let mut state_write = state.write().await;
    if let Err(e) = state_write.reload() {
        log::error!("Failed to reload config after resource change: {}", e);
    }
    drop(state_write);

    crate::events::emit_config_changed(&app);

    Ok(ApiResponse::ok(ResourceDetail {
        name: resource.name().to_string(),
        path: file_path.to_string_lossy().to_string(),
        yaml,
    }))
}

/// Update an existing resource.
#[tauri::command]
pub async fn update_gitops_resource(
    app: tauri::AppHandle,
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    kind: String,
    name: String,
    yaml: String,
) -> Result<ApiResponse<ResourceDetail>, String> {
    let state_guard = state.read().await;

    let resource_kind = match kind.parse::<ResourceKind>() {
        Ok(k) => k,
        Err(_) => return Ok(ApiResponse::err(format!("Unknown resource kind: {}", kind))),
    };

    let config_dir = match &state_guard.config_dir {
        Some(dir) => dir.clone(),
        None => return Ok(ApiResponse::err("No config directory set")),
    };

    let loader = ConfigLoader::new(&config_dir);

    // Parse the YAML
    let resource: AnyResource =
        match loader.parse_resource(&yaml, std::path::Path::new("update.yaml")) {
            Ok(r) => r,
            Err(e) => return Ok(ApiResponse::err(format!("Invalid YAML: {}", e))),
        };

    // Verify the name matches
    if resource.name() != name {
        return Ok(ApiResponse::err(format!(
            "Resource name mismatch: expected {}, got {}",
            name,
            resource.name()
        )));
    }

    // Get the current path
    let config = match state_guard.config() {
        Some(c) => c,
        None => return Ok(ApiResponse::err("Configuration not loaded")),
    };

    let file_path = match resource_kind {
        ResourceKind::Settings => config.settings.path.clone(),
        ResourceKind::Variable => {
            match config
                .variables
                .iter()
                .find(|v| v.resource.metadata.name == name)
            {
                Some(v) => v.path.clone(),
                None => {
                    return Ok(ApiResponse::err(format!(
                        "Resource not found: {}/{}",
                        kind, name
                    )))
                }
            }
        }
        ResourceKind::Rule => {
            match config
                .rules
                .iter()
                .find(|r| r.resource.metadata.name == name)
            {
                Some(r) => r.path.clone(),
                None => {
                    return Ok(ApiResponse::err(format!(
                        "Resource not found: {}/{}",
                        kind, name
                    )))
                }
            }
        }
        ResourceKind::ImportSource => {
            match config
                .import_sources
                .iter()
                .find(|s| s.resource.metadata.name == name)
            {
                Some(s) => s.path.clone(),
                None => {
                    return Ok(ApiResponse::err(format!(
                        "Resource not found: {}/{}",
                        kind, name
                    )))
                }
            }
        }
    };

    // Write the updated resource
    if let Err(e) = loader.write_resource(&resource, &file_path) {
        return Ok(ApiResponse::err(format!("Failed to write resource: {}", e)));
    }

    // Reload config
    drop(state_guard);
    let mut state_write = state.write().await;
    if let Err(e) = state_write.reload() {
        log::error!("Failed to reload config after resource change: {}", e);
    }
    drop(state_write);

    crate::events::emit_config_changed(&app);

    Ok(ApiResponse::ok(ResourceDetail {
        name,
        path: file_path.to_string_lossy().to_string(),
        yaml,
    }))
}

/// Delete a resource.
#[tauri::command]
pub async fn delete_gitops_resource(
    app: tauri::AppHandle,
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    kind: String,
    name: String,
) -> Result<ApiResponse<()>, String> {
    let state_guard = state.read().await;

    let resource_kind = match kind.parse::<ResourceKind>() {
        Ok(k) => k,
        Err(_) => return Ok(ApiResponse::err(format!("Unknown resource kind: {}", kind))),
    };

    // Cannot delete settings
    if resource_kind == ResourceKind::Settings {
        return Ok(ApiResponse::err("Cannot delete Settings resource"));
    }

    let config_dir = match &state_guard.config_dir {
        Some(dir) => dir.clone(),
        None => return Ok(ApiResponse::err("No config directory set")),
    };

    let config = match state_guard.config() {
        Some(c) => c,
        None => return Ok(ApiResponse::err("Configuration not loaded")),
    };

    // Get the path
    let file_path = match resource_kind {
        ResourceKind::Variable => {
            match config
                .variables
                .iter()
                .find(|v| v.resource.metadata.name == name)
            {
                Some(v) => v.path.clone(),
                None => {
                    return Ok(ApiResponse::err(format!(
                        "Resource not found: {}/{}",
                        kind, name
                    )))
                }
            }
        }
        ResourceKind::Rule => {
            match config
                .rules
                .iter()
                .find(|r| r.resource.metadata.name == name)
            {
                Some(r) => r.path.clone(),
                None => {
                    return Ok(ApiResponse::err(format!(
                        "Resource not found: {}/{}",
                        kind, name
                    )))
                }
            }
        }
        ResourceKind::ImportSource => {
            match config
                .import_sources
                .iter()
                .find(|s| s.resource.metadata.name == name)
            {
                Some(s) => s.path.clone(),
                None => {
                    return Ok(ApiResponse::err(format!(
                        "Resource not found: {}/{}",
                        kind, name
                    )))
                }
            }
        }
        ResourceKind::Settings => unreachable!(),
    };

    let loader = ConfigLoader::new(&config_dir);
    if let Err(e) = loader.delete_resource(&file_path) {
        return Ok(ApiResponse::err(format!(
            "Failed to delete resource: {}",
            e
        )));
    }

    // Reload config
    drop(state_guard);
    let mut state_write = state.write().await;
    if let Err(e) = state_write.reload() {
        log::error!("Failed to reload config after resource change: {}", e);
    }
    drop(state_write);

    crate::events::emit_config_changed(&app);

    Ok(ApiResponse::ok(()))
}

/// Simulate a rule match.
#[tauri::command]
pub async fn simulate_rule(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    request: SimulateRuleRequest,
) -> Result<ApiResponse<SimulateRuleResponse>, String> {
    use paporg::config::variables::VariableEngine;

    let text = &request.ocr_text;

    // Validate match_type
    let valid_match_types = ["contains", "containsAny", "containsAll", "pattern"];
    if !valid_match_types.contains(&request.match_type.as_str()) {
        return Ok(ApiResponse::err(format!(
            "Invalid match_type: '{}'. Must be one of: {}",
            request.match_type,
            valid_match_types.join(", ")
        )));
    }

    // Helper to parse match values for array-based match types
    let parse_values = |value: &serde_json::Value| -> Vec<String> {
        match value {
            serde_json::Value::Array(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            serde_json::Value::String(s) => s.split(',').map(|s| s.trim().to_string()).collect(),
            _ => vec![],
        }
    };

    // Build match condition from request
    let matches = match request.match_type.as_str() {
        "contains" => {
            let value = request.match_value.as_str().unwrap_or_default();
            if value.is_empty() {
                return Ok(ApiResponse::err("Match value cannot be empty"));
            }
            text.contains(value)
        }
        "containsAny" => {
            let values = parse_values(&request.match_value);
            if values.is_empty() {
                return Ok(ApiResponse::err("Match values cannot be empty"));
            }
            values.iter().any(|v| text.contains(v))
        }
        "containsAll" => {
            let values = parse_values(&request.match_value);
            if values.is_empty() {
                return Ok(ApiResponse::err("Match values cannot be empty"));
            }
            values.iter().all(|v| text.contains(v))
        }
        "pattern" => {
            let pattern = request.match_value.as_str().unwrap_or_default();

            if pattern.len() > MAX_REGEX_PATTERN_LENGTH {
                return Ok(ApiResponse::err(format!(
                    "Regex pattern too long: {} characters (max {})",
                    pattern.len(),
                    MAX_REGEX_PATTERN_LENGTH
                )));
            }

            match regex::RegexBuilder::new(pattern)
                .size_limit(1 << 20)
                .dfa_size_limit(1 << 20)
                .build()
            {
                Ok(re) => re.is_match(text),
                Err(e) => {
                    return Ok(ApiResponse::err(format!("Invalid regex pattern: {}", e)));
                }
            }
        }
        _ => unreachable!(),
    };

    // Generate match details
    let match_details = if matches {
        match request.match_type.as_str() {
            "contains" => {
                let value = request.match_value.as_str().unwrap_or_default();
                Some(format!("Found \"{}\" in document", value))
            }
            "containsAny" => {
                let values = parse_values(&request.match_value);
                let found: Vec<&String> = values
                    .iter()
                    .filter(|v| text.contains(v.as_str()))
                    .collect();
                Some(format!(
                    "Found: {}",
                    found
                        .iter()
                        .map(|s| format!("\"{}\"", s))
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            }
            "containsAll" => Some("All required terms found".to_string()),
            "pattern" => Some("Pattern matched".to_string()),
            _ => None,
        }
    } else {
        match request.match_type.as_str() {
            "contains" => {
                let value = request.match_value.as_str().unwrap_or_default();
                Some(format!("\"{}\" not found in document", value))
            }
            "containsAny" => Some("None of the terms found".to_string()),
            "containsAll" => {
                let values = parse_values(&request.match_value);
                let missing: Vec<&String> = values
                    .iter()
                    .filter(|v| !text.contains(v.as_str()))
                    .collect();
                Some(format!(
                    "Missing: {}",
                    missing
                        .iter()
                        .map(|s| format!("\"{}\"", s))
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            }
            "pattern" => Some("Pattern did not match".to_string()),
            _ => None,
        }
    };

    // Get variable definitions from config
    let state = state.read().await;
    let extracted_vars = state
        .config()
        .map(|c| c.to_legacy_config().variables.extracted)
        .unwrap_or_default();

    // Substitute variables in output paths
    let engine = VariableEngine::new(&extracted_vars);
    let mut extracted = engine.extract_variables(text);
    extracted.insert("category".to_string(), request.category.clone());

    let resolved_directory =
        engine.substitute(&request.output_directory, &request.filename, &extracted);
    let resolved_filename =
        engine.substitute(&request.output_filename, &request.filename, &extracted);
    let extension = request.filename.rsplit('.').next().unwrap_or("pdf");
    let resolved_path = format!("{}/{}.{}", resolved_directory, resolved_filename, extension);

    Ok(ApiResponse::ok(SimulateRuleResponse {
        matches,
        resolved_directory,
        resolved_filename,
        resolved_path,
        match_details,
    }))
}

/// Validate configuration.
#[tauri::command]
pub async fn validate_config(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
) -> Result<ApiResponse<Vec<String>>, String> {
    let state = state.read().await;

    let config = match state.config() {
        Some(c) => c,
        None => return Ok(ApiResponse::err("Configuration not loaded")),
    };

    let mut validator = ConfigValidator::new();
    match validator.validate(config) {
        Ok(()) => Ok(ApiResponse::ok(vec![])),
        Err(_) => Ok(ApiResponse::ok(validator.errors().to_vec())),
    }
}
