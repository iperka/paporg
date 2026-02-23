//! Cross-resource validation for GitOps configuration.

use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

// Pre-compiled regex for extracting variable names from templates
static RE_VARIABLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$([a-zA-Z_][a-zA-Z0-9_]*)").unwrap());

/// Names reserved for built-in variables. Extracted variables must not use these names.
/// These must exactly match the keys registered in `VariableEngine::get_builtin_variables()`.
const BUILTIN_VARIABLE_NAMES: &[&str] = &[
    "y",
    "l",
    "m",
    "d",
    "h",
    "i",
    "s",
    "original",
    "timestamp",
    "uuid",
];

use super::error::{GitOpsError, Result};
use super::loader::LoadedConfig;
use super::resource::{
    EmailAuthType, EmailSourceConfig, ImportSourceResource, MatchCondition, RuleResource,
    SettingsResource, VariableResource,
};

/// Validator for GitOps configuration.
pub struct ConfigValidator {
    /// Collected validation errors.
    errors: Vec<String>,
}

impl ConfigValidator {
    /// Creates a new validator.
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Validates the entire loaded configuration.
    pub fn validate(&mut self, config: &LoadedConfig) -> Result<()> {
        self.errors.clear();

        // Validate settings
        self.validate_settings(&config.settings.resource);

        // Validate variables
        for var in &config.variables {
            self.validate_variable(&var.resource);
        }

        // Validate rules
        for rule in &config.rules {
            self.validate_rule(&rule.resource);
        }

        // Validate import sources
        for source in &config.import_sources {
            self.validate_import_source(&source.resource);
        }

        // Cross-resource validation
        self.validate_variable_references(config);
        self.validate_unique_names(config);
        self.validate_directory_separation(config);
        self.validate_path_security(config);

        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(GitOpsError::Validation(self.errors.join("; ")))
        }
    }

    /// Validates the settings resource.
    fn validate_settings(&mut self, settings: &SettingsResource) {
        if settings.spec.input_directory.is_empty() {
            self.errors
                .push("Settings: inputDirectory is required".to_string());
        }

        if settings.spec.output_directory.is_empty() {
            self.errors
                .push("Settings: outputDirectory is required".to_string());
        }

        if settings.spec.worker_count == 0 {
            self.errors
                .push("Settings: workerCount must be greater than 0".to_string());
        }

        if settings.spec.ocr.dpi == 0 {
            self.errors
                .push("Settings: ocr.dpi must be greater than 0".to_string());
        }

        if settings.spec.defaults.output.directory.is_empty() {
            self.errors
                .push("Settings: defaults.output.directory is required".to_string());
        }

        if settings.spec.defaults.output.filename.is_empty() {
            self.errors
                .push("Settings: defaults.output.filename is required".to_string());
        }

        // Validate git settings if enabled
        if settings.spec.git.enabled && settings.spec.git.repository.is_empty() {
            self.errors
                .push("Settings: git.repository is required when git is enabled".to_string());
        }
    }

    /// Validates a variable resource.
    fn validate_variable(&mut self, variable: &VariableResource) {
        let name = &variable.metadata.name;

        if name.is_empty() {
            self.errors.push("Variable: name is required".to_string());
            return;
        }

        if !is_valid_identifier(name) {
            self.errors.push(format!(
                "Variable '{}': name must be a valid identifier (letters, numbers, underscores, hyphens)",
                name
            ));
        }

        // Check for collision with built-in variable names
        if BUILTIN_VARIABLE_NAMES.contains(&name.as_str()) {
            self.errors.push(format!(
                "Variable '{}': metadata.name '{}' conflicts with built-in variable '${}'; choose a different name",
                name, name, name
            ));
        }

        if variable.spec.pattern.is_empty() {
            self.errors
                .push(format!("Variable '{}': pattern is required", name));
            return;
        }

        // Validate regex pattern
        if let Err(e) = Regex::new(&variable.spec.pattern) {
            self.errors
                .push(format!("Variable '{}': invalid regex pattern: {}", name, e));
        }
    }

    /// Validates a rule resource.
    fn validate_rule(&mut self, rule: &RuleResource) {
        let name = &rule.metadata.name;

        if name.is_empty() {
            self.errors.push("Rule: name is required".to_string());
            return;
        }

        if !is_valid_identifier(name) {
            self.errors
                .push(format!("Rule '{}': name must be a valid identifier", name));
        }

        if rule.spec.category.is_empty() {
            self.errors
                .push(format!("Rule '{}': category is required", name));
        }

        if rule.spec.output.directory.is_empty() {
            self.errors
                .push(format!("Rule '{}': output.directory is required", name));
        }

        if rule.spec.output.filename.is_empty() {
            self.errors
                .push(format!("Rule '{}': output.filename is required", name));
        }

        // Validate match condition
        self.validate_match_condition(&rule.spec.match_condition, name);

        // Validate symlinks
        for (i, symlink) in rule.spec.symlinks.iter().enumerate() {
            if symlink.target.is_empty() {
                self.errors.push(format!(
                    "Rule '{}': symlink[{}].target is required",
                    name, i
                ));
            }
        }
    }

    /// Validates a match condition.
    fn validate_match_condition(&mut self, condition: &MatchCondition, rule_name: &str) {
        match condition {
            MatchCondition::Simple(simple) => {
                // At least one condition type should be set
                let has_condition = simple.contains.is_some()
                    || simple.contains_any.is_some()
                    || simple.contains_all.is_some()
                    || simple.pattern.is_some();

                if !has_condition {
                    self.errors.push(format!(
                        "Rule '{}': match condition must specify at least one of: contains, containsAny, containsAll, pattern",
                        rule_name
                    ));
                }

                // Validate regex pattern if specified
                if let Some(pattern) = &simple.pattern {
                    if let Err(e) = Regex::new(pattern) {
                        self.errors.push(format!(
                            "Rule '{}': invalid match pattern: {}",
                            rule_name, e
                        ));
                    }
                }

                // Validate containsAny is not empty
                if let Some(list) = &simple.contains_any {
                    if list.is_empty() {
                        self.errors.push(format!(
                            "Rule '{}': containsAny must have at least one value",
                            rule_name
                        ));
                    }
                }

                // Validate containsAll is not empty
                if let Some(list) = &simple.contains_all {
                    if list.is_empty() {
                        self.errors.push(format!(
                            "Rule '{}': containsAll must have at least one value",
                            rule_name
                        ));
                    }
                }
            }
            MatchCondition::Compound(compound) => {
                let has_condition =
                    compound.all.is_some() || compound.any.is_some() || compound.not.is_some();

                if !has_condition {
                    self.errors.push(format!(
                        "Rule '{}': compound match must specify at least one of: all, any, not",
                        rule_name
                    ));
                }

                // Recursively validate nested conditions
                if let Some(all) = &compound.all {
                    if all.is_empty() {
                        self.errors.push(format!(
                            "Rule '{}': 'all' must have at least one condition",
                            rule_name
                        ));
                    }
                    for cond in all {
                        self.validate_match_condition(cond, rule_name);
                    }
                }

                if let Some(any) = &compound.any {
                    if any.is_empty() {
                        self.errors.push(format!(
                            "Rule '{}': 'any' must have at least one condition",
                            rule_name
                        ));
                    }
                    for cond in any {
                        self.validate_match_condition(cond, rule_name);
                    }
                }

                if let Some(not) = &compound.not {
                    self.validate_match_condition(not, rule_name);
                }
            }
        }
    }

    /// Validates that variable references in rules exist.
    fn validate_variable_references(&mut self, config: &LoadedConfig) {
        let variable_names: HashSet<&str> = config
            .variables
            .iter()
            .map(|v| v.resource.metadata.name.as_str())
            .collect();

        // Built-in variables
        let builtin_vars: HashSet<&str> = BUILTIN_VARIABLE_NAMES.iter().copied().collect();

        for rule in &config.rules {
            let rule_name = &rule.resource.metadata.name;

            // Check directory template
            let dir_vars = extract_variable_names(&rule.resource.spec.output.directory);
            for var in &dir_vars {
                if !builtin_vars.contains(var.as_str()) && !variable_names.contains(var.as_str()) {
                    self.errors.push(format!(
                        "Rule '{}': output.directory references undefined variable '${}'. Define it in variables/ or use a built-in variable.",
                        rule_name, var
                    ));
                }
            }

            // Check filename template
            let file_vars = extract_variable_names(&rule.resource.spec.output.filename);
            for var in &file_vars {
                if !builtin_vars.contains(var.as_str()) && !variable_names.contains(var.as_str()) {
                    self.errors.push(format!(
                        "Rule '{}': output.filename references undefined variable '${}'. Define it in variables/ or use a built-in variable.",
                        rule_name, var
                    ));
                }
            }

            // Check symlink targets
            for (i, symlink) in rule.resource.spec.symlinks.iter().enumerate() {
                let link_vars = extract_variable_names(&symlink.target);
                for var in &link_vars {
                    if !builtin_vars.contains(var.as_str())
                        && !variable_names.contains(var.as_str())
                    {
                        self.errors.push(format!(
                            "Rule '{}': symlinks[{}].target references undefined variable '${}'. Define it in variables/ or use a built-in variable.",
                            rule_name, i, var
                        ));
                    }
                }
            }
        }
    }

    /// Validates that resource names are unique within their kind.
    fn validate_unique_names(&mut self, config: &LoadedConfig) {
        let mut variable_names: HashSet<&str> = HashSet::new();
        for var in &config.variables {
            let name = var.resource.metadata.name.as_str();
            if !variable_names.insert(name) {
                self.errors
                    .push(format!("Duplicate variable name: '{}'", name));
            }
        }

        let mut rule_names: HashSet<&str> = HashSet::new();
        for rule in &config.rules {
            let name = rule.resource.metadata.name.as_str();
            if !rule_names.insert(name) {
                self.errors.push(format!("Duplicate rule name: '{}'", name));
            }
        }

        let mut import_source_names: HashSet<&str> = HashSet::new();
        for source in &config.import_sources {
            let name = source.resource.metadata.name.as_str();
            if !import_source_names.insert(name) {
                self.errors
                    .push(format!("Duplicate import source name: '{}'", name));
            }
        }
    }

    /// Validates an ImportSource resource.
    fn validate_import_source(&mut self, source: &ImportSourceResource) {
        use super::resource::ImportSourceType;

        let name = &source.metadata.name;

        // Validate name
        if name.is_empty() {
            self.errors
                .push("ImportSource: name is required".to_string());
            return;
        }

        if !is_valid_identifier(name) {
            self.errors.push(format!(
                "ImportSource '{}': name must be a valid identifier (letters, numbers, underscores, hyphens)",
                name
            ));
        }

        // Check that local config is present when source type is local
        if source.spec.source_type == ImportSourceType::Local && source.spec.local.is_none() {
            self.errors.push(format!(
                "ImportSource '{}': local config is required when source type is 'local'",
                name
            ));
        }

        // Check that email config is present when source type is email
        if source.spec.source_type == ImportSourceType::Email && source.spec.email.is_none() {
            self.errors.push(format!(
                "ImportSource '{}': email config is required when source type is 'email'",
                name
            ));
        }

        // Validate local source config if present
        if let Some(local) = &source.spec.local {
            // Path must not be empty
            if local.path.is_empty() {
                self.errors
                    .push(format!("ImportSource '{}': local.path is required", name));
            }

            // Validate glob patterns in include filters
            for pattern in &local.filters.include {
                if let Err(e) = glob::Pattern::new(pattern) {
                    self.errors.push(format!(
                        "ImportSource '{}': invalid include glob pattern '{}': {}",
                        name, pattern, e
                    ));
                }
            }

            // Validate glob patterns in exclude filters
            for pattern in &local.filters.exclude {
                if let Err(e) = glob::Pattern::new(pattern) {
                    self.errors.push(format!(
                        "ImportSource '{}': invalid exclude glob pattern '{}': {}",
                        name, pattern, e
                    ));
                }
            }
        }

        // Validate email source config if present
        if let Some(email) = &source.spec.email {
            self.validate_email_source(name, email);
        }
    }

    /// Validates email source configuration.
    fn validate_email_source(&mut self, name: &str, email: &EmailSourceConfig) {
        // Host is required
        if email.host.is_empty() {
            self.errors
                .push(format!("ImportSource '{}': email.host is required", name));
        }

        // Username is required
        if email.username.is_empty() {
            self.errors.push(format!(
                "ImportSource '{}': email.username is required",
                name
            ));
        }

        // TLS is required for security
        if !email.use_tls {
            self.errors.push(format!(
                "ImportSource '{}': email.useTls must be true (TLS is required for security)",
                name
            ));
        }

        // Validate authentication settings
        match email.auth.auth_type {
            EmailAuthType::Password => {
                // Accept any of: passwordInsecure (direct), passwordFile, or passwordEnvVar
                let has_password = crate::secrets::has_secret_source(
                    email.auth.password_insecure.as_deref(),
                    email.auth.password_file.as_deref(),
                    email.auth.password_env_var.as_deref(),
                );

                if !has_password {
                    self.errors.push(format!(
                        "ImportSource '{}': password authentication requires one of: passwordInsecure, passwordFile, or passwordEnvVar",
                        name
                    ));
                }
            }
            EmailAuthType::OAuth2 => {
                if email.auth.oauth2.is_none() {
                    self.errors.push(format!(
                        "ImportSource '{}': email.auth.oauth2 is required for OAuth2 authentication",
                        name
                    ));
                } else if let Some(oauth2) = &email.auth.oauth2 {
                    // Accept any of: direct value, file, or env var for client credentials
                    let has_client_id = crate::secrets::has_secret_source(
                        oauth2.client_id_insecure.as_deref(),
                        oauth2.client_id_file.as_deref(),
                        oauth2.client_id_env_var.as_deref(),
                    );

                    let has_client_secret = crate::secrets::has_secret_source(
                        oauth2.client_secret_insecure.as_deref(),
                        oauth2.client_secret_file.as_deref(),
                        oauth2.client_secret_env_var.as_deref(),
                    );

                    if !has_client_id {
                        self.errors.push(format!(
                            "ImportSource '{}': OAuth2 requires one of: clientId, clientIdFile, or clientIdEnvVar",
                            name
                        ));
                    }
                    if !has_client_secret {
                        self.errors.push(format!(
                            "ImportSource '{}': OAuth2 requires one of: clientSecret, clientSecretFile, or clientSecretEnvVar",
                            name
                        ));
                    }
                    // refresh_token is optional (can use device flow instead)
                    // token_url is only required for custom provider
                    if oauth2.provider == crate::gitops::resource::OAuth2Provider::Custom
                        && oauth2.token_url.as_ref().is_none_or(|url| url.is_empty())
                    {
                        self.errors.push(format!(
                            "ImportSource '{}': email.auth.oauth2.tokenUrl is required for custom provider",
                            name
                        ));
                    }
                }
            }
        }

        // Validate MIME type patterns
        for pattern in &email.mime_filters.include {
            if !is_valid_mime_pattern(pattern) {
                self.errors.push(format!(
                    "ImportSource '{}': invalid MIME pattern '{}' (expected format: type/subtype or type/*)",
                    name, pattern
                ));
            }
        }
        for pattern in &email.mime_filters.exclude {
            if !is_valid_mime_pattern(pattern) {
                self.errors.push(format!(
                    "ImportSource '{}': invalid MIME pattern '{}' (expected format: type/subtype or type/*)",
                    name, pattern
                ));
            }
        }

        // Validate filename glob patterns
        for pattern in &email.mime_filters.filename_include {
            if let Err(e) = glob::Pattern::new(pattern) {
                self.errors.push(format!(
                    "ImportSource '{}': invalid filenameInclude pattern '{}': {}",
                    name, pattern, e
                ));
            }
        }
        for pattern in &email.mime_filters.filename_exclude {
            if let Err(e) = glob::Pattern::new(pattern) {
                self.errors.push(format!(
                    "ImportSource '{}': invalid filenameExclude pattern '{}': {}",
                    name, pattern, e
                ));
            }
        }

        // Validate size constraints
        if email.min_attachment_size > email.max_attachment_size {
            self.errors.push(format!(
                "ImportSource '{}': minAttachmentSize ({}) cannot be greater than maxAttachmentSize ({})",
                name, email.min_attachment_size, email.max_attachment_size
            ));
        }

        // Validate batch size
        if email.batch_size == 0 {
            self.errors.push(format!(
                "ImportSource '{}': batchSize must be greater than 0",
                name
            ));
        }
    }

    /// Validates that input and output directories don't overlap.
    fn validate_directory_separation(&mut self, config: &LoadedConfig) {
        use std::path::PathBuf;

        let settings = &config.settings.resource.spec;

        // Expand and try to canonicalize all paths
        let input_expanded = expand_tilde(&settings.input_directory);
        let output_expanded = expand_tilde(&settings.output_directory);

        let input = std::fs::canonicalize(&input_expanded)
            .unwrap_or_else(|_| PathBuf::from(&input_expanded));
        let output = std::fs::canonicalize(&output_expanded)
            .unwrap_or_else(|_| PathBuf::from(&output_expanded));

        // Check if paths are the same
        if input == output {
            self.errors.push(
                "Directory overlap: input and output directories cannot be the same".to_string(),
            );
        }

        // Check if output is inside input
        if output.starts_with(&input) && output != input {
            self.errors.push(format!(
                "Directory overlap: output directory '{}' is inside input directory '{}'",
                settings.output_directory, settings.input_directory
            ));
        }

        // Check if input is inside output
        if input.starts_with(&output) && input != output {
            self.errors.push(format!(
                "Directory overlap: input directory '{}' is inside output directory '{}'",
                settings.input_directory, settings.output_directory
            ));
        }

        // Check ImportSource paths don't overlap with output
        for source in &config.import_sources {
            if let Some(local) = &source.resource.spec.local {
                let source_path_expanded = expand_tilde(&local.path);
                let source_path = std::fs::canonicalize(&source_path_expanded)
                    .unwrap_or_else(|_| PathBuf::from(&source_path_expanded));

                // Check if source path is the same as or inside output directory
                if source_path == output {
                    self.errors.push(format!(
                        "Directory overlap: ImportSource '{}' path '{}' is the same as output directory",
                        source.resource.metadata.name, local.path
                    ));
                } else if source_path.starts_with(&output) {
                    self.errors.push(format!(
                        "Directory overlap: ImportSource '{}' path '{}' is inside output directory",
                        source.resource.metadata.name, local.path
                    ));
                } else if output.starts_with(&source_path) {
                    // Check if output directory is inside ImportSource path
                    self.errors.push(format!(
                        "Directory overlap: output directory is inside ImportSource '{}' path '{}'",
                        source.resource.metadata.name, local.path
                    ));
                }
            }
        }
    }

    /// Validates path security (no traversal, no absolute paths in templates).
    fn validate_path_security(&mut self, config: &LoadedConfig) {
        // Validate default output settings
        let defaults = &config.settings.resource.spec.defaults;
        self.check_path_security(
            &defaults.output.directory,
            "Settings",
            "defaults.output.directory",
        );
        self.check_path_security(
            &defaults.output.filename,
            "Settings",
            "defaults.output.filename",
        );

        // Validate rule output paths and symlinks
        for rule in &config.rules {
            let name = &rule.resource.metadata.name;
            self.check_path_security(
                &rule.resource.spec.output.directory,
                &format!("Rule '{}'", name),
                "output.directory",
            );
            self.check_path_security(
                &rule.resource.spec.output.filename,
                &format!("Rule '{}'", name),
                "output.filename",
            );

            // Validate symlink targets
            for (i, symlink) in rule.resource.spec.symlinks.iter().enumerate() {
                self.check_path_security(
                    &symlink.target,
                    &format!("Rule '{}'", name),
                    &format!("symlinks[{}].target", i),
                );
            }
        }
    }

    /// Checks a path template for security issues.
    fn check_path_security(&mut self, path_template: &str, resource: &str, field: &str) {
        // Check for path traversal (.. sequences)
        if contains_path_traversal(path_template) {
            self.errors.push(format!(
                "Path traversal detected in {}.{}: '{}'",
                resource, field, path_template
            ));
        }

        // Check for absolute paths (must be relative to output_directory)
        // Uses is_absolute() to handle both Unix (/) and Windows (C:\, \\server\share) paths
        if std::path::Path::new(path_template).is_absolute() {
            self.errors.push(format!(
                "Absolute path not allowed in {}.{}: paths must be relative to output directory",
                resource, field
            ));
        }
    }

    /// Returns the collected errors.
    pub fn errors(&self) -> &[String] {
        &self.errors
    }
}

impl Default for ConfigValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Checks if a MIME pattern is valid.
/// Valid patterns are: "type/subtype", "type/*", or "*/*".
fn is_valid_mime_pattern(pattern: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }

    let parts: Vec<&str> = pattern.split('/').collect();
    if parts.len() != 2 {
        return false;
    }

    let type_part = parts[0];
    let subtype_part = parts[1];

    // Type must be non-empty and contain only valid characters
    if type_part.is_empty() {
        return false;
    }

    // Allow wildcard for type
    if type_part != "*" && !is_valid_mime_token(type_part) {
        return false;
    }

    // Subtype must be non-empty and contain only valid characters
    if subtype_part.is_empty() {
        return false;
    }

    // Allow wildcard for subtype
    if subtype_part != "*" && !is_valid_mime_token(subtype_part) {
        return false;
    }

    true
}

/// Checks if a string is a valid MIME token (type or subtype).
fn is_valid_mime_token(s: &str) -> bool {
    // MIME tokens can contain alphanumeric characters, hyphens, dots, and plus signs
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '+')
}

/// Checks if a string is a valid identifier.
fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let first = s.chars().next().unwrap();
    if !first.is_alphabetic() && first != '_' {
        return false;
    }

    s.chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

/// Extracts variable names from a template string.
///
/// Variable names start with `$` followed by an identifier. When variables
/// are concatenated with underscores (e.g., `$day_$original`), we need to
/// properly split them. The underscore before a `$` belongs to the separator,
/// not the variable name.
fn extract_variable_names(template: &str) -> Vec<String> {
    RE_VARIABLE
        .captures_iter(template)
        .map(|cap| {
            let mut name = cap[1].to_string();
            // If the match ends with underscore and is followed by $ in the original,
            // strip the trailing underscore (it's a separator, not part of the variable name)
            let match_end = cap.get(0).unwrap().end();
            if name.ends_with('_') && template[match_end..].starts_with('$') {
                name.pop();
            }
            name
        })
        .collect()
}

/// Checks if a path template could escape the base directory via path traversal.
fn contains_path_traversal(path_template: &str) -> bool {
    use std::path::Component;

    // Check using Path components for proper ".." detection
    if std::path::Path::new(path_template)
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return true;
    }

    // Also check raw segments split by / and \ for templates
    // (Path::components may not catch all template cases)
    for segment in path_template.split(&['/', '\\'][..]) {
        if segment == ".." {
            return true;
        }
    }

    false
}

/// Expands ~ to the user's home directory (for path comparison).
/// Works cross-platform: checks HOME (Unix) then USERPROFILE (Windows).
fn expand_tilde(path: &str) -> String {
    if path.starts_with('~') {
        // Try HOME (Unix) first, then USERPROFILE (Windows)
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            return path.replacen('~', &home.to_string_lossy(), 1);
        }
    }
    path.to_string()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gitops::resource::*;

    fn create_minimal_settings() -> SettingsResource {
        Resource::new(
            ResourceKind::Settings,
            "default",
            SettingsSpec {
                input_directory: "/inbox".to_string(),
                output_directory: "/output".to_string(),
                worker_count: 4,
                ocr: OcrSettings::default(),
                defaults: DefaultOutputSettings::default(),
                git: GitSettings::default(),
                ai: AiSettings::default(),
                release_channel: ReleaseChannel::default(),
            },
        )
    }

    fn create_minimal_variable(name: &str, pattern: &str) -> VariableResource {
        Resource::new(
            ResourceKind::Variable,
            name,
            VariableSpec {
                pattern: pattern.to_string(),
                transform: None,
                default: None,
            },
        )
    }

    fn create_minimal_rule(name: &str) -> RuleResource {
        Resource::new(
            ResourceKind::Rule,
            name,
            RuleSpec {
                priority: 0,
                category: "Test".to_string(),
                match_condition: MatchCondition::Simple(SimpleMatch {
                    contains: Some("test".to_string()),
                    ..Default::default()
                }),
                output: OutputSettings {
                    directory: "Test".to_string(),
                    filename: "$original".to_string(),
                },
                symlinks: Vec::new(),
            },
        )
    }

    #[test]
    fn test_valid_config() {
        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![ResourceWithPath::new(
                create_minimal_variable("vendor", r"(?P<vendor>\w+)"),
                "variables/vendor.yaml",
            )],
            rules: vec![ResourceWithPath::new(
                create_minimal_rule("test-rule"),
                "rules/test.yaml",
            )],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_ok(), "Errors: {:?}", validator.errors());
    }

    #[test]
    fn test_missing_input_directory() {
        let mut settings = create_minimal_settings();
        settings.spec.input_directory = "".to_string();

        let config = LoadedConfig {
            settings: ResourceWithPath::new(settings, "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("inputDirectory")));
    }

    #[test]
    fn test_invalid_variable_pattern() {
        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![ResourceWithPath::new(
                create_minimal_variable("bad", "[invalid(regex"),
                "variables/bad.yaml",
            )],
            rules: vec![],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("invalid regex")));
    }

    #[test]
    fn test_undefined_variable_reference() {
        let mut rule = create_minimal_rule("test");
        rule.spec.output.directory = "Test/$undefined_var".to_string();

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![ResourceWithPath::new(rule, "rules/test.yaml")],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("undefined variable")));
    }

    #[test]
    fn test_builtin_variable_references() {
        let mut rule = create_minimal_rule("test");
        rule.spec.output.directory = "Archive/$y/$m".to_string();
        rule.spec.output.filename = "$original_$timestamp".to_string();

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![ResourceWithPath::new(rule, "rules/test.yaml")],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_ok(), "Errors: {:?}", validator.errors());
    }

    #[test]
    fn test_empty_match_condition() {
        let mut rule = create_minimal_rule("test");
        rule.spec.match_condition = MatchCondition::Simple(SimpleMatch::default());

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![ResourceWithPath::new(rule, "rules/test.yaml")],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("must specify at least one")));
    }

    #[test]
    fn test_invalid_rule_name() {
        let rule = create_minimal_rule("123-invalid");

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![ResourceWithPath::new(rule, "rules/test.yaml")],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("valid identifier")));
    }

    #[test]
    fn test_extract_variable_names() {
        let vars = extract_variable_names("$year/$month/$day_$original");
        assert_eq!(vars, vec!["year", "month", "day", "original"]);
    }

    #[test]
    fn test_is_valid_identifier() {
        assert!(is_valid_identifier("test"));
        assert!(is_valid_identifier("test_name"));
        assert!(is_valid_identifier("test-name"));
        assert!(is_valid_identifier("_private"));
        assert!(is_valid_identifier("Test123"));
        assert!(!is_valid_identifier("123test"));
        assert!(!is_valid_identifier(""));
        assert!(!is_valid_identifier("test name"));
    }

    #[test]
    fn test_git_enabled_requires_repository() {
        let mut settings = create_minimal_settings();
        settings.spec.git.enabled = true;
        settings.spec.git.repository = "".to_string();

        let config = LoadedConfig {
            settings: ResourceWithPath::new(settings, "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("git.repository")));
    }

    #[test]
    fn test_nested_compound_match() {
        let mut rule = create_minimal_rule("test");
        rule.spec.match_condition = MatchCondition::Compound(CompoundMatch {
            all: Some(vec![
                MatchCondition::Simple(SimpleMatch {
                    contains: Some("invoice".to_string()),
                    ..Default::default()
                }),
                MatchCondition::Compound(CompoundMatch {
                    any: Some(vec![MatchCondition::Simple(SimpleMatch {
                        contains: Some("VAT".to_string()),
                        ..Default::default()
                    })]),
                    ..Default::default()
                }),
            ]),
            ..Default::default()
        });

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![ResourceWithPath::new(rule, "rules/test.yaml")],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_ok(), "Errors: {:?}", validator.errors());
    }

    // ========================================================================
    // ImportSource validation tests
    // ========================================================================

    fn create_minimal_import_source(name: &str) -> ImportSourceResource {
        Resource::new(
            ResourceKind::ImportSource,
            name,
            ImportSourceSpec {
                source_type: ImportSourceType::Local,
                enabled: true,
                local: Some(LocalSourceConfig {
                    path: "/data/imports".to_string(),
                    recursive: false,
                    filters: FileFilters::default(),
                    poll_interval: 60,
                }),
                email: None,
            },
        )
    }

    #[test]
    fn test_valid_import_source() {
        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(
                create_minimal_import_source("local-docs"),
                "sources/local-docs.yaml",
            )],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_ok(), "Errors: {:?}", validator.errors());
    }

    #[test]
    fn test_import_source_empty_name() {
        let source = create_minimal_import_source("");

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(source, "sources/empty.yaml")],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("ImportSource: name is required")));
    }

    #[test]
    fn test_import_source_invalid_name() {
        let source = create_minimal_import_source("123-invalid");

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(source, "sources/invalid.yaml")],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("valid identifier")));
    }

    #[test]
    fn test_import_source_empty_path() {
        let mut source = create_minimal_import_source("test-source");
        if let Some(local) = &mut source.spec.local {
            local.path = "".to_string();
        }

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(source, "sources/test.yaml")],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("local.path is required")));
    }

    #[test]
    fn test_import_source_invalid_include_glob() {
        let mut source = create_minimal_import_source("test-source");
        if let Some(local) = &mut source.spec.local {
            local.filters.include = vec!["[invalid(glob".to_string()];
        }

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(source, "sources/test.yaml")],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("invalid include glob pattern")));
    }

    #[test]
    fn test_import_source_invalid_exclude_glob() {
        let mut source = create_minimal_import_source("test-source");
        if let Some(local) = &mut source.spec.local {
            local.filters.exclude = vec!["[invalid(glob".to_string()];
        }

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(source, "sources/test.yaml")],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("invalid exclude glob pattern")));
    }

    #[test]
    fn test_import_source_valid_glob_patterns() {
        let mut source = create_minimal_import_source("test-source");
        if let Some(local) = &mut source.spec.local {
            local.filters.include = vec!["*.pdf".to_string(), "**/*.doc".to_string()];
            local.filters.exclude = vec!["*.tmp".to_string(), "~*".to_string()];
        }

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(source, "sources/test.yaml")],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_ok(), "Errors: {:?}", validator.errors());
    }

    // ========================================================================
    // Directory overlap tests
    // ========================================================================

    #[test]
    fn test_directory_overlap_same_paths() {
        let mut settings = create_minimal_settings();
        settings.spec.input_directory = "/data".to_string();
        settings.spec.output_directory = "/data".to_string();

        let config = LoadedConfig {
            settings: ResourceWithPath::new(settings, "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("cannot be the same")));
    }

    #[test]
    fn test_directory_overlap_output_inside_input() {
        let mut settings = create_minimal_settings();
        settings.spec.input_directory = "/data".to_string();
        settings.spec.output_directory = "/data/output".to_string();

        let config = LoadedConfig {
            settings: ResourceWithPath::new(settings, "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("output directory") && e.contains("inside input directory")));
    }

    #[test]
    fn test_directory_overlap_input_inside_output() {
        let mut settings = create_minimal_settings();
        settings.spec.input_directory = "/data/input".to_string();
        settings.spec.output_directory = "/data".to_string();

        let config = LoadedConfig {
            settings: ResourceWithPath::new(settings, "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("input directory") && e.contains("inside output directory")));
    }

    #[test]
    fn test_import_source_overlaps_output() {
        let mut source = create_minimal_import_source("test-source");
        if let Some(local) = &mut source.spec.local {
            local.path = "/output".to_string(); // Same as default output directory
        }

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(source, "sources/test.yaml")],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("ImportSource") && e.contains("same as output directory")));
    }

    #[test]
    fn test_import_source_inside_output() {
        let mut source = create_minimal_import_source("test-source");
        if let Some(local) = &mut source.spec.local {
            local.path = "/output/imports".to_string(); // Inside default output directory
        }

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(source, "sources/test.yaml")],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("ImportSource") && e.contains("inside output directory")));
    }

    #[test]
    fn test_output_inside_import_source() {
        let mut settings = create_minimal_settings();
        settings.spec.output_directory = "/data/imports/processed".to_string();

        let mut source = create_minimal_import_source("test-source");
        if let Some(local) = &mut source.spec.local {
            local.path = "/data/imports".to_string(); // Output is inside this
        }

        let config = LoadedConfig {
            settings: ResourceWithPath::new(settings, "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(source, "sources/test.yaml")],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("output directory is inside ImportSource")));
    }

    // ========================================================================
    // Path security tests
    // ========================================================================

    #[test]
    fn test_contains_path_traversal() {
        assert!(contains_path_traversal("../secret"));
        assert!(contains_path_traversal("foo/../bar"));
        assert!(contains_path_traversal("foo/.."));
        assert!(contains_path_traversal(".."));
        assert!(!contains_path_traversal("foo/bar"));
        assert!(!contains_path_traversal("$year/$month"));
    }

    #[test]
    fn test_double_dots_in_filename_not_flagged_as_traversal() {
        // Filenames with double dots should NOT be flagged as path traversal
        assert!(!contains_path_traversal("report..final.pdf"));
        assert!(!contains_path_traversal("file..name"));
        assert!(!contains_path_traversal("test...file.txt"));
        assert!(!contains_path_traversal("archive..2024.tar.gz"));
    }

    #[test]
    fn test_path_traversal_in_rule_directory() {
        let mut rule = create_minimal_rule("test");
        rule.spec.output.directory = "../../../sensitive".to_string();

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![ResourceWithPath::new(rule, "rules/test.yaml")],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("Path traversal detected")));
    }

    #[test]
    fn test_path_traversal_in_rule_filename() {
        let mut rule = create_minimal_rule("test");
        rule.spec.output.filename = "../escape/$original".to_string();

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![ResourceWithPath::new(rule, "rules/test.yaml")],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("Path traversal detected")));
    }

    #[test]
    fn test_path_traversal_in_symlink() {
        let mut rule = create_minimal_rule("test");
        rule.spec.symlinks = vec![SymlinkSettings {
            target: "../../outside".to_string(),
        }];

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![ResourceWithPath::new(rule, "rules/test.yaml")],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("Path traversal detected") && e.contains("symlinks")));
    }

    #[test]
    fn test_absolute_path_in_rule_directory() {
        let mut rule = create_minimal_rule("test");
        rule.spec.output.directory = "/absolute/path".to_string();

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![ResourceWithPath::new(rule, "rules/test.yaml")],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("Absolute path not allowed")));
    }

    #[test]
    fn test_absolute_path_in_default_settings() {
        let mut settings = create_minimal_settings();
        settings.spec.defaults.output.directory = "/absolute/default".to_string();

        let config = LoadedConfig {
            settings: ResourceWithPath::new(settings, "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("Absolute path not allowed") && e.contains("defaults")));
    }

    #[test]
    fn test_duplicate_import_source_names() {
        let source1 = create_minimal_import_source("duplicate-name");
        let source2 = create_minimal_import_source("duplicate-name");

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![
                ResourceWithPath::new(source1, "sources/source1.yaml"),
                ResourceWithPath::new(source2, "sources/source2.yaml"),
            ],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("Duplicate import source name")));
    }

    #[test]
    fn test_import_source_local_type_requires_local_config() {
        // Create an ImportSource with type=local but no local config
        let source = Resource::new(
            ResourceKind::ImportSource,
            "missing-local",
            ImportSourceSpec {
                source_type: ImportSourceType::Local,
                enabled: true,
                local: None, // Missing local config!
                email: None,
            },
        );

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(source, "sources/missing.yaml")],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("local config is required when source type is 'local'")));
    }

    // ========================================================================
    // Email source validation tests
    // ========================================================================

    fn create_minimal_email_source(name: &str) -> ImportSourceResource {
        use crate::gitops::resource::{
            AttachmentFilters, EmailAuthSettings, EmailAuthType, EmailSourceConfig,
        };

        Resource::new(
            ResourceKind::ImportSource,
            name,
            ImportSourceSpec {
                source_type: ImportSourceType::Email,
                enabled: true,
                local: None,
                email: Some(EmailSourceConfig {
                    host: "imap.gmail.com".to_string(),
                    port: 993,
                    use_tls: true,
                    username: "test@example.com".to_string(),
                    auth: EmailAuthSettings {
                        auth_type: EmailAuthType::Password,
                        password_env_var: Some("EMAIL_PASSWORD".to_string()),
                        password_insecure: None,
                        password_file: None,
                        oauth2: None,
                    },
                    folder: "INBOX".to_string(),
                    since_date: None,
                    mime_filters: AttachmentFilters::default(),
                    min_attachment_size: 0,
                    max_attachment_size: 52_428_800,
                    poll_interval: 300,
                    batch_size: 50,
                }),
            },
        )
    }

    #[test]
    fn test_valid_email_source() {
        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(
                create_minimal_email_source("email-docs"),
                "sources/email-docs.yaml",
            )],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_ok(), "Errors: {:?}", validator.errors());
    }

    #[test]
    fn test_email_source_type_requires_email_config() {
        let source = Resource::new(
            ResourceKind::ImportSource,
            "missing-email",
            ImportSourceSpec {
                source_type: ImportSourceType::Email,
                enabled: true,
                local: None,
                email: None, // Missing email config!
            },
        );

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(source, "sources/missing.yaml")],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("email config is required when source type is 'email'")));
    }

    #[test]
    fn test_email_source_empty_host() {
        use crate::gitops::resource::{
            AttachmentFilters, EmailAuthSettings, EmailAuthType, EmailSourceConfig,
        };

        let source = Resource::new(
            ResourceKind::ImportSource,
            "empty-host",
            ImportSourceSpec {
                source_type: ImportSourceType::Email,
                enabled: true,
                local: None,
                email: Some(EmailSourceConfig {
                    host: "".to_string(), // Empty host!
                    port: 993,
                    use_tls: true,
                    username: "test@example.com".to_string(),
                    auth: EmailAuthSettings {
                        auth_type: EmailAuthType::Password,
                        password_env_var: Some("EMAIL_PASSWORD".to_string()),
                        password_insecure: None,
                        password_file: None,
                        oauth2: None,
                    },
                    folder: "INBOX".to_string(),
                    since_date: None,
                    mime_filters: AttachmentFilters::default(),
                    min_attachment_size: 0,
                    max_attachment_size: 52_428_800,
                    poll_interval: 300,
                    batch_size: 50,
                }),
            },
        );

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(source, "sources/empty-host.yaml")],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("email.host is required")));
    }

    #[test]
    fn test_email_source_tls_required() {
        use crate::gitops::resource::{
            AttachmentFilters, EmailAuthSettings, EmailAuthType, EmailSourceConfig,
        };

        let source = Resource::new(
            ResourceKind::ImportSource,
            "no-tls",
            ImportSourceSpec {
                source_type: ImportSourceType::Email,
                enabled: true,
                local: None,
                email: Some(EmailSourceConfig {
                    host: "imap.example.com".to_string(),
                    port: 143,
                    use_tls: false, // TLS disabled!
                    username: "test@example.com".to_string(),
                    auth: EmailAuthSettings {
                        auth_type: EmailAuthType::Password,
                        password_env_var: Some("EMAIL_PASSWORD".to_string()),
                        password_insecure: None,
                        password_file: None,
                        oauth2: None,
                    },
                    folder: "INBOX".to_string(),
                    since_date: None,
                    mime_filters: AttachmentFilters::default(),
                    min_attachment_size: 0,
                    max_attachment_size: 52_428_800,
                    poll_interval: 300,
                    batch_size: 50,
                }),
            },
        );

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(source, "sources/no-tls.yaml")],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("useTls must be true") && e.contains("TLS is required")));
    }

    #[test]
    fn test_email_source_password_auth_requires_env_var() {
        use crate::gitops::resource::{
            AttachmentFilters, EmailAuthSettings, EmailAuthType, EmailSourceConfig,
        };

        let source = Resource::new(
            ResourceKind::ImportSource,
            "no-password-env",
            ImportSourceSpec {
                source_type: ImportSourceType::Email,
                enabled: true,
                local: None,
                email: Some(EmailSourceConfig {
                    host: "imap.example.com".to_string(),
                    port: 993,
                    use_tls: true,
                    username: "test@example.com".to_string(),
                    auth: EmailAuthSettings {
                        auth_type: EmailAuthType::Password,
                        password_env_var: None, // Missing!
                        password_insecure: None,
                        password_file: None,
                        oauth2: None,
                    },
                    folder: "INBOX".to_string(),
                    since_date: None,
                    mime_filters: AttachmentFilters::default(),
                    min_attachment_size: 0,
                    max_attachment_size: 52_428_800,
                    poll_interval: 300,
                    batch_size: 50,
                }),
            },
        );

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(source, "sources/no-pw.yaml")],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("password authentication requires one of")));
    }

    #[test]
    fn test_email_source_oauth2_auth_requires_config() {
        use crate::gitops::resource::{
            AttachmentFilters, EmailAuthSettings, EmailAuthType, EmailSourceConfig,
        };

        let source = Resource::new(
            ResourceKind::ImportSource,
            "no-oauth2-config",
            ImportSourceSpec {
                source_type: ImportSourceType::Email,
                enabled: true,
                local: None,
                email: Some(EmailSourceConfig {
                    host: "imap.example.com".to_string(),
                    port: 993,
                    use_tls: true,
                    username: "test@example.com".to_string(),
                    auth: EmailAuthSettings {
                        auth_type: EmailAuthType::OAuth2,
                        password_env_var: None,
                        password_insecure: None,
                        password_file: None,
                        oauth2: None, // Missing OAuth2 config!
                    },
                    folder: "INBOX".to_string(),
                    since_date: None,
                    mime_filters: AttachmentFilters::default(),
                    min_attachment_size: 0,
                    max_attachment_size: 52_428_800,
                    poll_interval: 300,
                    batch_size: 50,
                }),
            },
        );

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(source, "sources/no-oauth2.yaml")],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("oauth2 is required for OAuth2 authentication")));
    }

    #[test]
    fn test_email_source_invalid_mime_pattern() {
        use crate::gitops::resource::{
            AttachmentFilters, EmailAuthSettings, EmailAuthType, EmailSourceConfig,
        };

        let source = Resource::new(
            ResourceKind::ImportSource,
            "bad-mime",
            ImportSourceSpec {
                source_type: ImportSourceType::Email,
                enabled: true,
                local: None,
                email: Some(EmailSourceConfig {
                    host: "imap.example.com".to_string(),
                    port: 993,
                    use_tls: true,
                    username: "test@example.com".to_string(),
                    auth: EmailAuthSettings {
                        auth_type: EmailAuthType::Password,
                        password_env_var: Some("EMAIL_PASSWORD".to_string()),
                        password_insecure: None,
                        password_file: None,
                        oauth2: None,
                    },
                    folder: "INBOX".to_string(),
                    since_date: None,
                    mime_filters: AttachmentFilters {
                        include: vec!["invalid-mime".to_string()], // Invalid format!
                        exclude: vec![],
                        filename_include: vec![],
                        filename_exclude: vec![],
                    },
                    min_attachment_size: 0,
                    max_attachment_size: 52_428_800,
                    poll_interval: 300,
                    batch_size: 50,
                }),
            },
        );

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(source, "sources/bad-mime.yaml")],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("invalid MIME pattern")));
    }

    #[test]
    fn test_email_source_valid_mime_patterns() {
        use crate::gitops::resource::{
            AttachmentFilters, EmailAuthSettings, EmailAuthType, EmailSourceConfig,
        };

        let source = Resource::new(
            ResourceKind::ImportSource,
            "valid-mime",
            ImportSourceSpec {
                source_type: ImportSourceType::Email,
                enabled: true,
                local: None,
                email: Some(EmailSourceConfig {
                    host: "imap.example.com".to_string(),
                    port: 993,
                    use_tls: true,
                    username: "test@example.com".to_string(),
                    auth: EmailAuthSettings {
                        auth_type: EmailAuthType::Password,
                        password_env_var: Some("EMAIL_PASSWORD".to_string()),
                        password_insecure: None,
                        password_file: None,
                        oauth2: None,
                    },
                    folder: "INBOX".to_string(),
                    since_date: None,
                    mime_filters: AttachmentFilters {
                        include: vec![
                            "application/pdf".to_string(),
                            "image/*".to_string(),
                            "application/vnd.ms-excel".to_string(),
                            "*/*".to_string(),
                        ],
                        exclude: vec!["text/plain".to_string()],
                        filename_include: vec![],
                        filename_exclude: vec![],
                    },
                    min_attachment_size: 0,
                    max_attachment_size: 52_428_800,
                    poll_interval: 300,
                    batch_size: 50,
                }),
            },
        );

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(source, "sources/valid-mime.yaml")],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_ok(), "Errors: {:?}", validator.errors());
    }

    #[test]
    fn test_email_source_size_constraints() {
        use crate::gitops::resource::{
            AttachmentFilters, EmailAuthSettings, EmailAuthType, EmailSourceConfig,
        };

        let source = Resource::new(
            ResourceKind::ImportSource,
            "bad-sizes",
            ImportSourceSpec {
                source_type: ImportSourceType::Email,
                enabled: true,
                local: None,
                email: Some(EmailSourceConfig {
                    host: "imap.example.com".to_string(),
                    port: 993,
                    use_tls: true,
                    username: "test@example.com".to_string(),
                    auth: EmailAuthSettings {
                        auth_type: EmailAuthType::Password,
                        password_env_var: Some("EMAIL_PASSWORD".to_string()),
                        password_insecure: None,
                        password_file: None,
                        oauth2: None,
                    },
                    folder: "INBOX".to_string(),
                    since_date: None,
                    mime_filters: AttachmentFilters::default(),
                    min_attachment_size: 1_000_000, // 1MB min
                    max_attachment_size: 500_000,   // 500KB max - invalid!
                    poll_interval: 300,
                    batch_size: 50,
                }),
            },
        );

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![],
            import_sources: vec![ResourceWithPath::new(source, "sources/bad-sizes.yaml")],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("minAttachmentSize") && e.contains("cannot be greater")));
    }

    #[test]
    fn test_is_valid_mime_pattern() {
        // Valid patterns
        assert!(is_valid_mime_pattern("application/pdf"));
        assert!(is_valid_mime_pattern("image/*"));
        assert!(is_valid_mime_pattern("*/*"));
        assert!(is_valid_mime_pattern("text/plain"));
        assert!(is_valid_mime_pattern("application/vnd.ms-excel"));
        assert!(is_valid_mime_pattern("application/x-gzip"));
        assert!(is_valid_mime_pattern("image/svg+xml"));

        // Invalid patterns
        assert!(!is_valid_mime_pattern("")); // Empty
        assert!(!is_valid_mime_pattern("application")); // Missing subtype
        assert!(!is_valid_mime_pattern("application/")); // Empty subtype
        assert!(!is_valid_mime_pattern("/pdf")); // Empty type
        assert!(!is_valid_mime_pattern("application/pdf/extra")); // Too many parts
        assert!(!is_valid_mime_pattern("application pdf")); // Space instead of slash
    }

    // ========================================================================
    // Built-in variable collision tests
    // ========================================================================

    #[test]
    fn test_variable_name_conflicts_with_builtin() {
        // "y" is a built-in — should be rejected
        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![ResourceWithPath::new(
                create_minimal_variable("y", r"(?P<y>\d{4})"),
                "variables/y.yaml",
            )],
            rules: vec![],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("conflicts with built-in variable") && e.contains("'y'")));
    }

    #[test]
    fn test_variable_name_conflicts_with_builtin_h() {
        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![ResourceWithPath::new(
                create_minimal_variable("h", r"(?P<h>\d+)"),
                "variables/h.yaml",
            )],
            rules: vec![],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("conflicts with built-in variable") && e.contains("'h'")));
    }

    #[test]
    fn test_variable_name_conflicts_with_builtin_i() {
        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![ResourceWithPath::new(
                create_minimal_variable("i", r"(?P<i>\d+)"),
                "variables/i.yaml",
            )],
            rules: vec![],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("conflicts with built-in variable") && e.contains("'i'")));
    }

    #[test]
    fn test_variable_name_conflicts_with_builtin_s() {
        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![ResourceWithPath::new(
                create_minimal_variable("s", r"(?P<s>\d+)"),
                "variables/s.yaml",
            )],
            rules: vec![],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("conflicts with built-in variable") && e.contains("'s'")));
    }

    #[test]
    fn test_variable_name_conflicts_with_builtin_timestamp() {
        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![ResourceWithPath::new(
                create_minimal_variable("timestamp", r"(?P<timestamp>\d+)"),
                "variables/timestamp.yaml",
            )],
            rules: vec![],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_err());
        assert!(validator
            .errors()
            .iter()
            .any(|e| e.contains("conflicts with built-in variable") && e.contains("timestamp")));
    }

    #[test]
    fn test_non_builtin_variable_name_accepted() {
        // "vendor" is NOT a built-in — should be accepted
        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![ResourceWithPath::new(
                create_minimal_variable("vendor", r"(?P<vendor>\w+)"),
                "variables/vendor.yaml",
            )],
            rules: vec![],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_ok(), "Errors: {:?}", validator.errors());
    }

    #[test]
    fn test_builtin_time_vars_accepted_in_templates() {
        // Rules that reference h, i, s builtins in templates should be valid
        let mut rule = create_minimal_rule("test");
        rule.spec.output.directory = "$y/$m/$d".to_string();
        rule.spec.output.filename = "$original_$h$i$s".to_string();

        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![],
            rules: vec![ResourceWithPath::new(rule, "rules/test.yaml")],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_ok(), "Errors: {:?}", validator.errors());
    }

    #[test]
    fn test_non_builtin_alias_name_accepted() {
        // "year" is NOT an implemented built-in (only "y" is) — should be accepted as a user variable
        let config = LoadedConfig {
            settings: ResourceWithPath::new(create_minimal_settings(), "settings.yaml"),
            variables: vec![ResourceWithPath::new(
                create_minimal_variable("year", r"(?P<year>\d{4})"),
                "variables/year.yaml",
            )],
            rules: vec![],
            import_sources: vec![],
        };

        let mut validator = ConfigValidator::new();
        let result = validator.validate(&config);
        assert!(result.is_ok(), "Errors: {:?}", validator.errors());
    }
}
