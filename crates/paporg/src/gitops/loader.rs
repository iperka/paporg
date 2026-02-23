//! Configuration loader for multi-file YAML configurations.

use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::config::schema::{
    CompoundMatch as LegacyCompoundMatch, Config as LegacyConfig, DefaultsConfig,
    ExtractedVariable, MatchCondition as LegacyMatchCondition, OcrConfig, OutputConfig,
    Rule as LegacyRule, SimpleMatch as LegacySimpleMatch, SymlinkConfig,
    VariableTransform as LegacyTransform, VariablesConfig,
};

use super::error::{GitOpsError, Result};
use super::resource::{
    AnyResource, ImportSourceResource, MatchCondition, ResourceHeader, ResourceKind,
    ResourceWithPath, RuleResource, SettingsResource, VariableResource, VariableTransform,
    API_VERSION,
};

/// Loaded configuration from the config directory.
#[derive(Debug, Clone)]
pub struct LoadedConfig {
    /// The settings resource (required).
    pub settings: ResourceWithPath<SettingsResource>,
    /// All variable resources.
    pub variables: Vec<ResourceWithPath<VariableResource>>,
    /// All rule resources.
    pub rules: Vec<ResourceWithPath<RuleResource>>,
    /// All import source resources.
    pub import_sources: Vec<ResourceWithPath<ImportSourceResource>>,
}

impl LoadedConfig {
    /// Returns all resources as a flat list.
    pub fn all_resources(&self) -> Vec<(&ResourceKind, &str, &Path)> {
        let mut resources = Vec::new();
        resources.push((
            &ResourceKind::Settings,
            self.settings.resource.metadata.name.as_str(),
            self.settings.path.as_path(),
        ));
        for var in &self.variables {
            resources.push((
                &ResourceKind::Variable,
                var.resource.metadata.name.as_str(),
                var.path.as_path(),
            ));
        }
        for rule in &self.rules {
            resources.push((
                &ResourceKind::Rule,
                rule.resource.metadata.name.as_str(),
                rule.path.as_path(),
            ));
        }
        for source in &self.import_sources {
            resources.push((
                &ResourceKind::ImportSource,
                source.resource.metadata.name.as_str(),
                source.path.as_path(),
            ));
        }
        resources
    }

    /// Converts the loaded config to the legacy Config format.
    pub fn to_legacy_config(&self) -> LegacyConfig {
        let settings = &self.settings.resource.spec;

        // Convert variables
        let extracted: Vec<ExtractedVariable> = self
            .variables
            .iter()
            .map(|v| ExtractedVariable {
                name: v.resource.metadata.name.clone(),
                pattern: v.resource.spec.pattern.clone(),
                transform: v.resource.spec.transform.map(|t| match t {
                    VariableTransform::Slugify => LegacyTransform::Slugify,
                    VariableTransform::Uppercase => LegacyTransform::Uppercase,
                    VariableTransform::Lowercase => LegacyTransform::Lowercase,
                    VariableTransform::Trim => LegacyTransform::Trim,
                }),
                default: v.resource.spec.default.clone(),
            })
            .collect();

        // Convert rules
        let rules: Vec<LegacyRule> = self
            .rules
            .iter()
            .map(|r| LegacyRule {
                id: r.resource.metadata.name.clone(),
                name: r.resource.metadata.name.clone(),
                priority: r.resource.spec.priority,
                match_condition: convert_match_condition(&r.resource.spec.match_condition),
                category: r.resource.spec.category.clone(),
                output: OutputConfig {
                    directory: r.resource.spec.output.directory.clone(),
                    filename: r.resource.spec.output.filename.clone(),
                },
                symlinks: r
                    .resource
                    .spec
                    .symlinks
                    .iter()
                    .map(|s| SymlinkConfig {
                        target: s.target.clone(),
                    })
                    .collect(),
            })
            .collect();

        LegacyConfig {
            version: "1.0".to_string(),
            input_directory: settings.input_directory.clone(),
            output_directory: settings.output_directory.clone(),
            worker_count: settings.worker_count,
            ocr: OcrConfig {
                enabled: settings.ocr.enabled,
                languages: settings.ocr.languages.clone(),
                dpi: settings.ocr.dpi,
            },
            variables: VariablesConfig { extracted },
            rules,
            defaults: DefaultsConfig {
                output: OutputConfig {
                    directory: settings.defaults.output.directory.clone(),
                    filename: settings.defaults.output.filename.clone(),
                },
            },
            ai: crate::config::schema::AiConfig {
                enabled: settings.ai.enabled,
                model_cache_dir: settings.ai.model_cache_dir.clone(),
                model_repo: settings.ai.model_repo.clone(),
                model_file: settings.ai.model_file.clone(),
                timeout_secs: settings.ai.timeout_secs,
            },
        }
    }
}

fn convert_match_condition(cond: &MatchCondition) -> LegacyMatchCondition {
    match cond {
        MatchCondition::Simple(s) => LegacyMatchCondition::Simple(LegacySimpleMatch {
            contains: s.contains.clone(),
            contains_any: s.contains_any.clone(),
            contains_all: s.contains_all.clone(),
            pattern: s.pattern.clone(),
            case_sensitive: s.case_sensitive,
        }),
        MatchCondition::Compound(c) => LegacyMatchCondition::Compound(LegacyCompoundMatch {
            all: c
                .all
                .as_ref()
                .map(|v| v.iter().map(convert_match_condition).collect()),
            any: c
                .any
                .as_ref()
                .map(|v| v.iter().map(convert_match_condition).collect()),
            not: c.not.as_ref().map(|n| Box::new(convert_match_condition(n))),
            case_sensitive: c.case_sensitive,
        }),
    }
}

/// Configuration loader for the GitOps system.
pub struct ConfigLoader {
    /// Root directory for configuration files.
    config_dir: PathBuf,
}

impl ConfigLoader {
    /// Creates a new config loader for the given directory.
    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_dir: config_dir.into(),
        }
    }

    /// Returns the config directory path.
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Loads all configuration from the config directory.
    pub fn load(&self) -> Result<LoadedConfig> {
        if !self.config_dir.exists() {
            return Err(GitOpsError::ConfigDirNotFound(self.config_dir.clone()));
        }

        let mut settings: Option<ResourceWithPath<SettingsResource>> = None;
        let mut variables: Vec<ResourceWithPath<VariableResource>> = Vec::new();
        let mut rules: Vec<ResourceWithPath<RuleResource>> = Vec::new();
        let mut import_sources: Vec<ResourceWithPath<ImportSourceResource>> = Vec::new();

        // Walk the config directory
        for entry in WalkDir::new(&self.config_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Only process YAML files
            if !path.is_file() {
                continue;
            }

            // Skip files in hidden directories or hidden files themselves
            // Check the relative path for any component starting with '.'
            if let Ok(relative) = path.strip_prefix(&self.config_dir) {
                let has_hidden_component = relative.components().any(|c| {
                    c.as_os_str()
                        .to_str()
                        .map(|s| s.starts_with('.'))
                        .unwrap_or(false)
                });
                if has_hidden_component {
                    continue;
                }
            }

            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "yaml" && ext != "yml" {
                continue;
            }

            // Parse the resource
            match self.load_file(path) {
                Ok(resource) => {
                    let relative_path = path
                        .strip_prefix(&self.config_dir)
                        .unwrap_or(path)
                        .to_path_buf();

                    match resource {
                        AnyResource::Settings(r) => {
                            if settings.is_some() {
                                return Err(GitOpsError::DuplicateName {
                                    kind: "Settings".to_string(),
                                    name: r.metadata.name.clone(),
                                });
                            }
                            settings = Some(ResourceWithPath::new(r, relative_path));
                        }
                        AnyResource::Variable(r) => {
                            // Check for duplicate names
                            if variables
                                .iter()
                                .any(|v| v.resource.metadata.name == r.metadata.name)
                            {
                                return Err(GitOpsError::DuplicateName {
                                    kind: "Variable".to_string(),
                                    name: r.metadata.name.clone(),
                                });
                            }
                            variables.push(ResourceWithPath::new(r, relative_path));
                        }
                        AnyResource::Rule(r) => {
                            // Check for duplicate names
                            if rules
                                .iter()
                                .any(|rule| rule.resource.metadata.name == r.metadata.name)
                            {
                                return Err(GitOpsError::DuplicateName {
                                    kind: "Rule".to_string(),
                                    name: r.metadata.name.clone(),
                                });
                            }
                            rules.push(ResourceWithPath::new(r, relative_path));
                        }
                        AnyResource::ImportSource(r) => {
                            // Check for duplicate names
                            if import_sources
                                .iter()
                                .any(|s| s.resource.metadata.name == r.metadata.name)
                            {
                                return Err(GitOpsError::DuplicateName {
                                    kind: "ImportSource".to_string(),
                                    name: r.metadata.name.clone(),
                                });
                            }
                            import_sources.push(ResourceWithPath::new(r, relative_path));
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to load {}: {}", path.display(), e);
                    return Err(e);
                }
            }
        }

        // Ensure settings exists
        let settings = settings.ok_or(GitOpsError::MissingSettings)?;

        // Sort rules by priority (descending)
        rules.sort_by(|a, b| b.resource.spec.priority.cmp(&a.resource.spec.priority));

        // Sort import sources by name
        import_sources.sort_by(|a, b| a.resource.metadata.name.cmp(&b.resource.metadata.name));

        Ok(LoadedConfig {
            settings,
            variables,
            rules,
            import_sources,
        })
    }

    /// Loads a single resource file.
    pub fn load_file(&self, path: &Path) -> Result<AnyResource> {
        let content = fs::read_to_string(path).map_err(|e| GitOpsError::ReadFile {
            path: path.to_path_buf(),
            source: e,
        })?;

        self.parse_resource(&content, path)
    }

    /// Parses a resource from YAML content.
    pub fn parse_resource(&self, content: &str, path: &Path) -> Result<AnyResource> {
        // First, parse the header to determine the kind
        let header: ResourceHeader =
            serde_yaml::from_str(content).map_err(|e| GitOpsError::ParseYaml {
                path: path.to_path_buf(),
                message: e.to_string(),
            })?;

        // Validate API version
        if header.api_version != API_VERSION {
            return Err(GitOpsError::InvalidApiVersion {
                version: header.api_version,
                expected: API_VERSION.to_string(),
            });
        }

        // Parse based on kind
        match header.kind {
            ResourceKind::Settings => {
                let resource: SettingsResource =
                    serde_yaml::from_str(content).map_err(|e| GitOpsError::ParseYaml {
                        path: path.to_path_buf(),
                        message: e.to_string(),
                    })?;
                Ok(AnyResource::Settings(resource))
            }
            ResourceKind::Variable => {
                let resource: VariableResource =
                    serde_yaml::from_str(content).map_err(|e| GitOpsError::ParseYaml {
                        path: path.to_path_buf(),
                        message: e.to_string(),
                    })?;
                Ok(AnyResource::Variable(resource))
            }
            ResourceKind::Rule => {
                let resource: RuleResource =
                    serde_yaml::from_str(content).map_err(|e| GitOpsError::ParseYaml {
                        path: path.to_path_buf(),
                        message: e.to_string(),
                    })?;
                Ok(AnyResource::Rule(resource))
            }
            ResourceKind::ImportSource => {
                let resource: ImportSourceResource =
                    serde_yaml::from_str(content).map_err(|e| GitOpsError::ParseYaml {
                        path: path.to_path_buf(),
                        message: e.to_string(),
                    })?;
                Ok(AnyResource::ImportSource(resource))
            }
        }
    }

    /// Writes a resource to a file.
    pub fn write_resource(&self, resource: &AnyResource, path: &Path) -> Result<()> {
        let full_path = self.config_dir.join(path);

        // Ensure parent directory exists
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).map_err(|e| GitOpsError::WriteFile {
                path: full_path.clone(),
                source: e,
            })?;
        }

        // Add schema comment based on resource kind
        let schema_comment = Self::get_schema_comment(resource.kind());

        let yaml_content = match resource {
            AnyResource::Settings(r) => serde_yaml::to_string(r),
            AnyResource::Variable(r) => serde_yaml::to_string(r),
            AnyResource::Rule(r) => serde_yaml::to_string(r),
            AnyResource::ImportSource(r) => serde_yaml::to_string(r),
        }
        .map_err(|e| GitOpsError::SerializeYaml(e.to_string()))?;

        let content = format!("{}{}", schema_comment, yaml_content);

        fs::write(&full_path, content).map_err(|e| GitOpsError::WriteFile {
            path: full_path,
            source: e,
        })?;

        Ok(())
    }

    /// Returns the schema comment for a given resource kind.
    fn get_schema_comment(kind: ResourceKind) -> String {
        let schema_name = match kind {
            ResourceKind::Settings => "settings",
            ResourceKind::Variable => "variable",
            ResourceKind::Rule => "rule",
            ResourceKind::ImportSource => "import-source",
        };
        format!(
            "# yaml-language-server: $schema=https://paporg.io/schemas/{}.json\n",
            schema_name
        )
    }

    /// Deletes a resource file.
    pub fn delete_resource(&self, path: &Path) -> Result<()> {
        let full_path = self.config_dir.join(path);

        if !full_path.exists() {
            return Err(GitOpsError::ResourceNotFound {
                kind: "unknown".to_string(),
                name: path.display().to_string(),
            });
        }

        fs::remove_file(&full_path).map_err(|e| GitOpsError::WriteFile {
            path: full_path,
            source: e,
        })?;

        Ok(())
    }

    /// Gets the default path for a resource.
    pub fn default_path_for_resource(&self, kind: ResourceKind, name: &str) -> PathBuf {
        match kind {
            ResourceKind::Settings => PathBuf::from("settings.yaml"),
            ResourceKind::Variable => PathBuf::from(format!("variables/{}.yaml", name)),
            ResourceKind::Rule => PathBuf::from(format!("rules/{}.yaml", name)),
            ResourceKind::ImportSource => PathBuf::from(format!("sources/{}.yaml", name)),
        }
    }

    /// Returns the file tree structure of the config directory.
    pub fn get_file_tree(&self) -> Result<FileTreeNode> {
        self.build_file_tree(&self.config_dir, "")
    }

    fn build_file_tree(&self, path: &Path, relative: &str) -> Result<FileTreeNode> {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let resource_info = if ext == "yaml" || ext == "yml" {
                self.load_file(path).ok().map(|r| ResourceInfo {
                    kind: r.kind(),
                    name: r.name().to_string(),
                })
            } else {
                None
            };

            Ok(FileTreeNode {
                name,
                path: relative.to_string(),
                is_directory: false,
                children: Vec::new(),
                resource: resource_info,
            })
        } else {
            let mut children = Vec::new();

            let mut entries: Vec<_> = fs::read_dir(path)
                .map_err(|e| GitOpsError::ReadDirectory {
                    path: path.to_path_buf(),
                    source: e,
                })?
                .filter_map(|e| e.ok())
                .collect();

            // Sort: directories first, then by name
            entries.sort_by(|a, b| {
                let a_is_dir = a.path().is_dir();
                let b_is_dir = b.path().is_dir();
                match (a_is_dir, b_is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.file_name().cmp(&b.file_name()),
                }
            });

            for entry in entries {
                let entry_path = entry.path();
                let entry_name = entry.file_name().to_string_lossy().to_string();

                // Skip hidden files and non-yaml files
                if entry_name.starts_with('.') {
                    continue;
                }

                let child_relative = if relative.is_empty() {
                    entry_name.clone()
                } else {
                    format!("{}/{}", relative, entry_name)
                };

                if entry_path.is_file() {
                    let ext = entry_path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    if ext != "yaml" && ext != "yml" {
                        continue;
                    }
                    children.push(self.build_file_tree(&entry_path, &child_relative)?);
                } else if entry_path.is_dir() {
                    // Include empty directories that are within resource type folders
                    let is_resource_subfolder = child_relative.starts_with("sources/")
                        || child_relative.starts_with("variables/")
                        || child_relative.starts_with("rules/");

                    let child_node = self.build_file_tree(&entry_path, &child_relative)?;

                    // Keep folder if it has children OR it's a subfolder of a resource type directory
                    if !child_node.children.is_empty() || is_resource_subfolder {
                        children.push(child_node);
                    }
                }
            }

            Ok(FileTreeNode {
                name,
                path: relative.to_string(),
                is_directory: true,
                children,
                resource: None,
            })
        }
    }
}

/// Information about a resource.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResourceInfo {
    pub kind: ResourceKind,
    pub name: String,
}

/// A node in the file tree.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTreeNode {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub children: Vec<FileTreeNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<ResourceInfo>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_settings() -> String {
        r#"
apiVersion: paporg.io/v1
kind: Settings
metadata:
  name: default
spec:
  inputDirectory: /data/inbox
  outputDirectory: /data/documents
  workerCount: 4
  ocr:
    enabled: true
    languages: [eng, deu]
    dpi: 300
  defaults:
    output:
      directory: "$y/unsorted"
      filename: "$original_$timestamp"
  git:
    enabled: false
"#
        .to_string()
    }

    fn create_test_variable() -> String {
        r#"
apiVersion: paporg.io/v1
kind: Variable
metadata:
  name: vendor
spec:
  pattern: "(?i)from[:\\s]+(?P<vendor>[A-Za-z0-9\\s]+)"
  transform: slugify
  default: unknown
"#
        .to_string()
    }

    fn create_test_rule() -> String {
        r#"
apiVersion: paporg.io/v1
kind: Rule
metadata:
  name: tax-invoices
  labels:
    category: finance
spec:
  priority: 100
  category: Tax
  match:
    all:
      - containsAny: [Invoice, Rechnung]
      - containsAny: [VAT, MwSt]
  output:
    directory: "Tax/$year/Invoices"
    filename: "$original_$timestamp"
  symlinks:
    - target: "ByVendor/$vendor"
"#
        .to_string()
    }

    fn setup_test_config_dir() -> TempDir {
        let dir = TempDir::new().unwrap();

        // Create settings.yaml
        fs::write(dir.path().join("settings.yaml"), create_test_settings()).unwrap();

        // Create variables directory
        fs::create_dir_all(dir.path().join("variables")).unwrap();
        fs::write(
            dir.path().join("variables/vendor.yaml"),
            create_test_variable(),
        )
        .unwrap();

        // Create rules directory with subdirectory
        fs::create_dir_all(dir.path().join("rules/invoices")).unwrap();
        fs::write(
            dir.path().join("rules/invoices/tax-invoices.yaml"),
            create_test_rule(),
        )
        .unwrap();

        dir
    }

    #[test]
    fn test_load_config() {
        let dir = setup_test_config_dir();
        let loader = ConfigLoader::new(dir.path());

        let config = loader.load().unwrap();

        assert_eq!(config.settings.resource.metadata.name, "default");
        assert_eq!(config.settings.resource.spec.input_directory, "/data/inbox");
        assert_eq!(config.variables.len(), 1);
        assert_eq!(config.variables[0].resource.metadata.name, "vendor");
        assert_eq!(config.rules.len(), 1);
        assert_eq!(config.rules[0].resource.metadata.name, "tax-invoices");
    }

    #[test]
    fn test_load_missing_settings() {
        let dir = TempDir::new().unwrap();
        let loader = ConfigLoader::new(dir.path());

        let result = loader.load();
        assert!(matches!(result, Err(GitOpsError::MissingSettings)));
    }

    #[test]
    fn test_load_nonexistent_directory() {
        let loader = ConfigLoader::new("/nonexistent/path");
        let result = loader.load();
        assert!(matches!(result, Err(GitOpsError::ConfigDirNotFound(_))));
    }

    #[test]
    fn test_parse_resource_invalid_api_version() {
        let yaml = r#"
apiVersion: wrong/v1
kind: Settings
metadata:
  name: default
spec:
  inputDirectory: /data/inbox
  outputDirectory: /data/documents
"#;
        let loader = ConfigLoader::new(".");
        let result = loader.parse_resource(yaml, Path::new("test.yaml"));
        assert!(matches!(result, Err(GitOpsError::InvalidApiVersion { .. })));
    }

    #[test]
    fn test_to_legacy_config() {
        let dir = setup_test_config_dir();
        let loader = ConfigLoader::new(dir.path());
        let config = loader.load().unwrap();

        let legacy = config.to_legacy_config();

        assert_eq!(legacy.input_directory, "/data/inbox");
        assert_eq!(legacy.output_directory, "/data/documents");
        assert_eq!(legacy.worker_count, 4);
        assert_eq!(legacy.variables.extracted.len(), 1);
        assert_eq!(legacy.variables.extracted[0].name, "vendor");
        assert_eq!(legacy.rules.len(), 1);
        assert_eq!(legacy.rules[0].id, "tax-invoices");
    }

    #[test]
    fn test_get_file_tree() {
        let dir = setup_test_config_dir();
        let loader = ConfigLoader::new(dir.path());

        let tree = loader.get_file_tree().unwrap();

        assert!(tree.is_directory);
        assert!(!tree.children.is_empty());

        // Find settings.yaml
        let settings = tree.children.iter().find(|c| c.name == "settings.yaml");
        assert!(settings.is_some());
        let settings = settings.unwrap();
        assert!(!settings.is_directory);
        assert!(settings.resource.is_some());
        assert_eq!(
            settings.resource.as_ref().unwrap().kind,
            ResourceKind::Settings
        );
    }

    #[test]
    fn test_default_path_for_resource() {
        let loader = ConfigLoader::new(".");

        assert_eq!(
            loader.default_path_for_resource(ResourceKind::Settings, "default"),
            PathBuf::from("settings.yaml")
        );
        assert_eq!(
            loader.default_path_for_resource(ResourceKind::Variable, "vendor"),
            PathBuf::from("variables/vendor.yaml")
        );
        assert_eq!(
            loader.default_path_for_resource(ResourceKind::Rule, "tax-invoices"),
            PathBuf::from("rules/tax-invoices.yaml")
        );
        assert_eq!(
            loader.default_path_for_resource(ResourceKind::ImportSource, "local-documents"),
            PathBuf::from("sources/local-documents.yaml")
        );
    }

    #[test]
    fn test_write_and_delete_resource() {
        let dir = TempDir::new().unwrap();
        let loader = ConfigLoader::new(dir.path());

        // Create a settings resource
        let settings = SettingsResource {
            api_version: API_VERSION.to_string(),
            kind: ResourceKind::Settings,
            metadata: super::super::resource::ObjectMeta::new("default"),
            spec: super::super::resource::SettingsSpec {
                input_directory: "/inbox".to_string(),
                output_directory: "/output".to_string(),
                worker_count: 4,
                ocr: super::super::resource::OcrSettings::default(),
                defaults: super::super::resource::DefaultOutputSettings::default(),
                git: super::super::resource::GitSettings::default(),
                ai: super::super::resource::AiSettings::default(),
            },
        };

        let path = PathBuf::from("settings.yaml");
        loader
            .write_resource(&AnyResource::Settings(settings), &path)
            .unwrap();

        // Verify file exists
        assert!(dir.path().join("settings.yaml").exists());

        // Load and verify
        let loaded = loader.load_file(&dir.path().join("settings.yaml")).unwrap();
        assert_eq!(loaded.kind(), ResourceKind::Settings);
        assert_eq!(loaded.name(), "default");

        // Delete
        loader.delete_resource(&path).unwrap();
        assert!(!dir.path().join("settings.yaml").exists());
    }

    #[test]
    fn test_duplicate_resource_name() {
        let dir = TempDir::new().unwrap();

        // Create settings
        fs::write(dir.path().join("settings.yaml"), create_test_settings()).unwrap();

        // Create two variables with the same name
        fs::create_dir_all(dir.path().join("variables")).unwrap();
        fs::write(
            dir.path().join("variables/vendor.yaml"),
            create_test_variable(),
        )
        .unwrap();
        fs::write(
            dir.path().join("variables/vendor2.yaml"),
            create_test_variable(),
        )
        .unwrap();

        let loader = ConfigLoader::new(dir.path());
        let result = loader.load();
        assert!(matches!(result, Err(GitOpsError::DuplicateName { .. })));
    }

    #[test]
    fn test_hidden_files_and_directories_are_skipped() {
        let dir = TempDir::new().unwrap();

        // Create valid settings
        fs::write(dir.path().join("settings.yaml"), create_test_settings()).unwrap();

        // Create a hidden YAML file that would fail to parse (e.g., .pre-commit-config.yaml)
        let invalid_yaml = r#"
repos:
  - repo: https://github.com/pre-commit/pre-commit-hooks
    rev: v4.4.0
    hooks:
      - id: trailing-whitespace
"#;
        fs::write(dir.path().join(".pre-commit-config.yaml"), invalid_yaml).unwrap();

        // Create a hidden directory with YAML files (e.g., .github/workflows)
        let github_workflow = r#"
name: CI
on: push
jobs:
  build:
    runs-on: ubuntu-latest
"#;
        fs::create_dir_all(dir.path().join(".github/workflows")).unwrap();
        fs::write(dir.path().join(".github/workflows/ci.yml"), github_workflow).unwrap();

        let loader = ConfigLoader::new(dir.path());
        // This should succeed because hidden files and directories are skipped
        let config = loader.load().unwrap();
        assert_eq!(config.settings.resource.metadata.name, "default");
    }

    #[test]
    fn test_get_schema_comment() {
        assert_eq!(
            ConfigLoader::get_schema_comment(ResourceKind::Settings),
            "# yaml-language-server: $schema=https://paporg.io/schemas/settings.json\n"
        );
        assert_eq!(
            ConfigLoader::get_schema_comment(ResourceKind::Variable),
            "# yaml-language-server: $schema=https://paporg.io/schemas/variable.json\n"
        );
        assert_eq!(
            ConfigLoader::get_schema_comment(ResourceKind::Rule),
            "# yaml-language-server: $schema=https://paporg.io/schemas/rule.json\n"
        );
        assert_eq!(
            ConfigLoader::get_schema_comment(ResourceKind::ImportSource),
            "# yaml-language-server: $schema=https://paporg.io/schemas/import-source.json\n"
        );
    }

    #[test]
    fn test_write_resource_includes_schema_comment() {
        let dir = TempDir::new().unwrap();
        let loader = ConfigLoader::new(dir.path());

        // Create a settings resource
        let settings = SettingsResource {
            api_version: API_VERSION.to_string(),
            kind: ResourceKind::Settings,
            metadata: super::super::resource::ObjectMeta::new("default"),
            spec: super::super::resource::SettingsSpec {
                input_directory: "/inbox".to_string(),
                output_directory: "/output".to_string(),
                worker_count: 4,
                ocr: super::super::resource::OcrSettings::default(),
                defaults: super::super::resource::DefaultOutputSettings::default(),
                git: super::super::resource::GitSettings::default(),
                ai: super::super::resource::AiSettings::default(),
            },
        };

        let path = PathBuf::from("settings.yaml");
        loader
            .write_resource(&AnyResource::Settings(settings), &path)
            .unwrap();

        // Read the raw file content and verify schema comment
        let content = fs::read_to_string(dir.path().join("settings.yaml")).unwrap();
        assert!(content.starts_with(
            "# yaml-language-server: $schema=https://paporg.io/schemas/settings.json\n"
        ));
    }
}
