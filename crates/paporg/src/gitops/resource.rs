//! K8s-style resource types for GitOps configuration.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The API version for all paporg resources.
pub const API_VERSION: &str = "paporg.io/v1";

/// The kind of resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceKind {
    Settings,
    Variable,
    Rule,
    ImportSource,
}

impl ResourceKind {
    /// Returns the directory name for storing resources of this kind.
    pub fn directory(&self) -> Option<&'static str> {
        match self {
            ResourceKind::Settings => None, // settings.yaml at root
            ResourceKind::Variable => Some("variables"),
            ResourceKind::Rule => Some("rules"),
            ResourceKind::ImportSource => Some("sources"),
        }
    }

    /// Returns all resource kinds.
    pub fn all() -> &'static [ResourceKind] {
        &[
            ResourceKind::Settings,
            ResourceKind::Variable,
            ResourceKind::Rule,
            ResourceKind::ImportSource,
        ]
    }
}

impl std::fmt::Display for ResourceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceKind::Settings => write!(f, "Settings"),
            ResourceKind::Variable => write!(f, "Variable"),
            ResourceKind::Rule => write!(f, "Rule"),
            ResourceKind::ImportSource => write!(f, "ImportSource"),
        }
    }
}

impl std::str::FromStr for ResourceKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "settings" => Ok(ResourceKind::Settings),
            "variable" => Ok(ResourceKind::Variable),
            "rule" => Ok(ResourceKind::Rule),
            "importsource" => Ok(ResourceKind::ImportSource),
            _ => Err(format!("Unknown resource kind: {}", s)),
        }
    }
}

/// Metadata for a resource, following K8s conventions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObjectMeta {
    /// The unique name of the resource within its kind.
    pub name: String,

    /// Key-value labels for organizing and selecting resources.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub labels: HashMap<String, String>,

    /// Key-value annotations for storing additional metadata.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub annotations: HashMap<String, String>,
}

impl ObjectMeta {
    /// Creates a new ObjectMeta with just a name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            labels: HashMap::new(),
            annotations: HashMap::new(),
        }
    }

    /// Adds a label to the metadata.
    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }
}

/// A generic K8s-style resource wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resource<T> {
    /// API version, should always be `paporg.io/v1`.
    pub api_version: String,

    /// The kind of resource.
    pub kind: ResourceKind,

    /// Resource metadata.
    pub metadata: ObjectMeta,

    /// The resource specification.
    pub spec: T,
}

impl<T> Resource<T> {
    /// Creates a new resource with the given kind and spec.
    pub fn new(kind: ResourceKind, name: impl Into<String>, spec: T) -> Self {
        Self {
            api_version: API_VERSION.to_string(),
            kind,
            metadata: ObjectMeta::new(name),
            spec,
        }
    }

    /// Returns the name of the resource.
    pub fn name(&self) -> &str {
        &self.metadata.name
    }
}

// ============================================================================
// Settings Resource
// ============================================================================

/// Settings specification - global configuration for paporg.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSpec {
    /// Directory to watch for incoming documents.
    pub input_directory: String,

    /// Base directory for organized documents.
    pub output_directory: String,

    /// Number of worker threads.
    #[serde(default = "default_worker_count")]
    pub worker_count: usize,

    /// OCR configuration.
    #[serde(default)]
    pub ocr: OcrSettings,

    /// Default output settings.
    #[serde(default)]
    pub defaults: DefaultOutputSettings,

    /// Git synchronization settings.
    #[serde(default)]
    pub git: GitSettings,

    /// AI settings for rule suggestions.
    #[serde(default)]
    pub ai: AiSettings,
}

fn default_worker_count() -> usize {
    num_cpus::get()
}

/// OCR settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrSettings {
    /// Whether OCR is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Languages for OCR (e.g., "eng", "deu").
    #[serde(default = "default_languages")]
    pub languages: Vec<String>,

    /// DPI for image processing.
    #[serde(default = "default_dpi")]
    pub dpi: u32,
}

fn default_true() -> bool {
    true
}

fn default_languages() -> Vec<String> {
    vec!["eng".to_string()]
}

fn default_dpi() -> u32 {
    300
}

impl Default for OcrSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            languages: default_languages(),
            dpi: 300,
        }
    }
}

/// Default output settings for documents that don't match any rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultOutputSettings {
    /// Output path configuration.
    pub output: OutputSettings,
}

impl Default for DefaultOutputSettings {
    fn default() -> Self {
        Self {
            output: OutputSettings {
                directory: "$y/unsorted".to_string(),
                filename: "$original_$timestamp".to_string(),
            },
        }
    }
}

/// Git synchronization settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitSettings {
    /// Whether git sync is enabled.
    #[serde(default)]
    pub enabled: bool,

    /// Git repository URL.
    #[serde(default)]
    pub repository: String,

    /// Branch to sync with.
    #[serde(default = "default_branch")]
    pub branch: String,

    /// Sync interval in seconds.
    #[serde(default = "default_sync_interval")]
    pub sync_interval: u64,

    /// Authentication settings.
    #[serde(default)]
    pub auth: GitAuthSettings,

    /// Git user name for commits.
    #[serde(default = "default_user_name")]
    pub user_name: String,

    /// Git user email for commits.
    #[serde(default = "default_user_email")]
    pub user_email: String,
}

fn default_branch() -> String {
    "main".to_string()
}

fn default_sync_interval() -> u64 {
    300 // 5 minutes
}

fn default_user_name() -> String {
    "Paporg".to_string()
}

fn default_user_email() -> String {
    "paporg@localhost".to_string()
}

/// Git authentication settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitAuthSettings {
    /// Authentication type: none, token, or ssh-key.
    #[serde(default, rename = "type")]
    pub auth_type: GitAuthType,

    /// Environment variable containing the token.
    #[serde(default)]
    pub token_env_var: String,

    /// Direct token value (for local development).
    /// WARNING: This stores the token in plaintext in the config file.
    /// Prefer token_env_var or token_file for better security.
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "token")]
    pub token_insecure: Option<String>,

    /// Path to file containing the token (for Docker secrets).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_file: Option<String>,

    /// Path to SSH key file.
    #[serde(default)]
    pub ssh_key_path: String,
}

/// Git authentication type.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GitAuthType {
    #[default]
    None,
    Token,
    SshKey,
}

/// AI settings for rule suggestions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSettings {
    /// Whether AI suggestions are enabled.
    #[serde(default)]
    pub enabled: bool,

    /// Directory to cache downloaded models.
    #[serde(default = "default_model_cache")]
    pub model_cache_dir: String,

    /// Hugging Face model repository.
    #[serde(default = "default_model_repo")]
    pub model_repo: String,

    /// Model filename (GGUF format).
    #[serde(default = "default_model_file")]
    pub model_file: String,

    /// Inference timeout in seconds.
    #[serde(default = "default_ai_timeout")]
    pub timeout_secs: u64,
}

fn default_model_cache() -> String {
    // Use platform-specific cache directory from dirs crate
    dirs::cache_dir()
        .map(|p| {
            p.join("paporg")
                .join("models")
                .to_string_lossy()
                .to_string()
        })
        .unwrap_or_else(|| {
            // Fallback for platforms where cache_dir() returns None
            if cfg!(target_os = "windows") {
                std::env::var("LOCALAPPDATA")
                    .map(|p| format!("{}\\paporg\\models", p))
                    .unwrap_or_else(|_| "C:\\ProgramData\\paporg\\models".to_string())
            } else {
                // Unix-like fallback to user's home directory cache
                dirs::home_dir()
                    .map(|p| {
                        p.join(".cache")
                            .join("paporg")
                            .join("models")
                            .to_string_lossy()
                            .to_string()
                    })
                    .unwrap_or_else(|| "/tmp/paporg/models".to_string())
            }
        })
}

fn default_model_repo() -> String {
    "Qwen/Qwen2.5-1.5B-Instruct-GGUF".to_string()
}

fn default_model_file() -> String {
    "qwen2.5-1.5b-instruct-q4_k_m.gguf".to_string()
}

fn default_ai_timeout() -> u64 {
    60
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            model_cache_dir: default_model_cache(),
            model_repo: default_model_repo(),
            model_file: default_model_file(),
            timeout_secs: default_ai_timeout(),
        }
    }
}

/// Type alias for Settings resource.
pub type SettingsResource = Resource<SettingsSpec>;

// ============================================================================
// Variable Resource
// ============================================================================

/// Variable specification - defines how to extract variables from document text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableSpec {
    /// Regex pattern to extract the variable value.
    /// Use named capture groups like `(?P<value>...)`.
    pub pattern: String,

    /// Optional transformation to apply to the extracted value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<VariableTransform>,

    /// Default value if the pattern doesn't match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

/// Transformation to apply to extracted variable values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VariableTransform {
    /// Convert to URL-friendly slug.
    Slugify,
    /// Convert to uppercase.
    Uppercase,
    /// Convert to lowercase.
    Lowercase,
    /// Trim whitespace.
    Trim,
}

/// Type alias for Variable resource.
pub type VariableResource = Resource<VariableSpec>;

// ============================================================================
// Rule Resource
// ============================================================================

/// Rule specification - defines how to categorize and organize documents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSpec {
    /// Priority of this rule (higher = more important).
    #[serde(default)]
    pub priority: i32,

    /// Category name for matched documents.
    pub category: String,

    /// Match conditions for this rule.
    #[serde(rename = "match")]
    pub match_condition: MatchCondition,

    /// Output path configuration.
    pub output: OutputSettings,

    /// Additional symlinks to create.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symlinks: Vec<SymlinkSettings>,
}

/// Match condition for rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MatchCondition {
    /// Simple match condition.
    Simple(SimpleMatch),
    /// Compound match condition (all, any, not).
    Compound(CompoundMatch),
}

/// A simple match condition.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleMatch {
    /// Match if text contains this string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contains: Option<String>,

    /// Match if text contains any of these strings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contains_any: Option<Vec<String>>,

    /// Match if text contains all of these strings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contains_all: Option<Vec<String>>,

    /// Match if text matches this regex pattern.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

/// A compound match condition.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompoundMatch {
    /// All conditions must match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all: Option<Vec<MatchCondition>>,

    /// Any condition must match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub any: Option<Vec<MatchCondition>>,

    /// Condition must not match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not: Option<Box<MatchCondition>>,
}

/// Output path settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputSettings {
    /// Directory path template.
    pub directory: String,

    /// Filename template.
    pub filename: String,
}

/// Symlink configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymlinkSettings {
    /// Target directory template for the symlink.
    pub target: String,
}

/// Type alias for Rule resource.
pub type RuleResource = Resource<RuleSpec>;

// ============================================================================
// ImportSource Resource
// ============================================================================

/// Type of import source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportSourceType {
    Local,
    Email,
}

/// ImportSource specification - defines where to import documents from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSourceSpec {
    /// The type of import source.
    #[serde(rename = "type")]
    pub source_type: ImportSourceType,

    /// Whether this source is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Configuration for local directory source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<LocalSourceConfig>,

    /// Configuration for email attachment source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<EmailSourceConfig>,
}

/// Configuration for a local directory import source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSourceConfig {
    /// Path to the local directory to watch.
    pub path: String,

    /// Whether to watch subdirectories recursively.
    #[serde(default)]
    pub recursive: bool,

    /// File filters for inclusion/exclusion.
    #[serde(default)]
    pub filters: FileFilters,

    /// Poll interval in seconds.
    #[serde(default = "default_poll_interval")]
    pub poll_interval: u64,
}

fn default_poll_interval() -> u64 {
    60
}

/// Configuration for an email attachment import source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailSourceConfig {
    /// IMAP server hostname (e.g., "imap.gmail.com").
    pub host: String,

    /// IMAP server port (default: 993 for IMAPS).
    #[serde(default = "default_imap_port")]
    pub port: u16,

    /// Whether to use TLS (required for security).
    #[serde(default = "default_true")]
    pub use_tls: bool,

    /// Email username (typically the email address).
    pub username: String,

    /// Authentication settings.
    pub auth: EmailAuthSettings,

    /// IMAP folder to scan (default: "INBOX").
    #[serde(default = "default_inbox")]
    pub folder: String,

    /// Only process emails received after this date (ISO 8601 format).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since_date: Option<String>,

    /// MIME type filters for attachments.
    #[serde(default)]
    pub mime_filters: AttachmentFilters,

    /// Minimum attachment size in bytes (default: 0).
    #[serde(default)]
    pub min_attachment_size: u64,

    /// Maximum attachment size in bytes (default: 50MB).
    #[serde(default = "default_max_attachment_size")]
    pub max_attachment_size: u64,

    /// Poll interval in seconds (default: 300 = 5 minutes).
    #[serde(default = "default_email_poll_interval")]
    pub poll_interval: u64,

    /// Maximum number of emails to process per batch (default: 50).
    #[serde(default = "default_batch_size")]
    pub batch_size: u32,
}

fn default_imap_port() -> u16 {
    993
}

fn default_inbox() -> String {
    "INBOX".to_string()
}

fn default_max_attachment_size() -> u64 {
    52_428_800 // 50 MB
}

fn default_email_poll_interval() -> u64 {
    300 // 5 minutes
}

fn default_batch_size() -> u32 {
    50
}

/// Email authentication settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailAuthSettings {
    /// Authentication type.
    #[serde(rename = "type")]
    pub auth_type: EmailAuthType,

    /// Environment variable containing the password (for password auth).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password_env_var: Option<String>,

    /// Direct password value (for local development).
    /// WARNING: Storing passwords directly in config files is insecure.
    /// Prefer using password_env_var or password_file instead.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "passwordInsecure",
        alias = "password"
    )]
    pub password_insecure: Option<String>,

    /// Path to file containing the password (for Docker secrets).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password_file: Option<String>,

    /// OAuth2 settings (for OAuth2 auth).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth2: Option<OAuth2Settings>,
}

/// Email authentication type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmailAuthType {
    #[default]
    Password,
    OAuth2,
}

/// OAuth2 provider preset for known providers.
///
/// Note: Defaults to `Custom` to require users to explicitly specify their
/// OAuth2 provider. Use `gmail` or `outlook` for well-known providers with
/// pre-configured endpoints, or `custom` for other providers (requires
/// explicit `deviceAuthUrl` and `tokenUrl` configuration).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OAuth2Provider {
    Gmail,
    Outlook,
    #[default]
    Custom,
}

/// OAuth2 authentication settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuth2Settings {
    /// OAuth2 provider preset (gmail, outlook, custom).
    /// If not specified, defaults to custom (requires explicit URLs).
    /// Specify "gmail" or "outlook" to use well-known provider endpoints.
    #[serde(default)]
    pub provider: OAuth2Provider,

    /// Environment variable containing the OAuth2 client ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id_env_var: Option<String>,

    /// Environment variable containing the OAuth2 client secret.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret_env_var: Option<String>,

    /// Environment variable containing the OAuth2 refresh token.
    /// Not needed when using Device Flow - tokens are stored in the database.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token_env_var: Option<String>,

    /// Direct OAuth2 client ID (for local development).
    /// WARNING: This stores the client ID in plaintext in the config file.
    /// Prefer client_id_env_var or client_id_file for better security.
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "client_id")]
    pub client_id_insecure: Option<String>,

    /// Direct OAuth2 client secret (for local development).
    /// WARNING: This stores the client secret in plaintext in the config file.
    /// Prefer client_secret_env_var or client_secret_file for better security.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "client_secret"
    )]
    pub client_secret_insecure: Option<String>,

    /// Direct OAuth2 refresh token (for local development).
    /// WARNING: This stores the refresh token in plaintext in the config file.
    /// Prefer refresh_token_env_var or refresh_token_file for better security.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "refresh_token"
    )]
    pub refresh_token_insecure: Option<String>,

    /// Path to file containing OAuth2 client ID (for Docker secrets).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id_file: Option<String>,

    /// Path to file containing OAuth2 client secret (for Docker secrets).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret_file: Option<String>,

    /// Path to file containing OAuth2 refresh token (for Docker secrets).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token_file: Option<String>,

    /// OAuth2 token endpoint URL.
    /// Required for custom provider, optional for known providers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_url: Option<String>,
}

/// Attachment filters based on MIME types and filenames.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentFilters {
    /// MIME type patterns to include (e.g., "application/pdf", "image/*").
    #[serde(default)]
    pub include: Vec<String>,

    /// MIME type patterns to exclude.
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Filename glob patterns to include.
    #[serde(default)]
    pub filename_include: Vec<String>,

    /// Filename glob patterns to exclude.
    #[serde(default)]
    pub filename_exclude: Vec<String>,
}

/// File filters for import sources.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileFilters {
    /// Glob patterns to include (default: ["*"]).
    #[serde(default = "default_include_patterns")]
    pub include: Vec<String>,

    /// Glob patterns to exclude.
    #[serde(default)]
    pub exclude: Vec<String>,
}

fn default_include_patterns() -> Vec<String> {
    vec!["*".to_string()]
}

/// Type alias for ImportSource resource.
pub type ImportSourceResource = Resource<ImportSourceSpec>;

// ============================================================================
// Any Resource (for generic handling)
// ============================================================================

/// A resource that can be any of the supported types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum AnyResource {
    Settings(SettingsResource),
    Variable(VariableResource),
    Rule(RuleResource),
    ImportSource(ImportSourceResource),
}

impl AnyResource {
    /// Returns the kind of this resource.
    pub fn kind(&self) -> ResourceKind {
        match self {
            AnyResource::Settings(_) => ResourceKind::Settings,
            AnyResource::Variable(_) => ResourceKind::Variable,
            AnyResource::Rule(_) => ResourceKind::Rule,
            AnyResource::ImportSource(_) => ResourceKind::ImportSource,
        }
    }

    /// Returns the name of this resource.
    pub fn name(&self) -> &str {
        match self {
            AnyResource::Settings(r) => &r.metadata.name,
            AnyResource::Variable(r) => &r.metadata.name,
            AnyResource::Rule(r) => &r.metadata.name,
            AnyResource::ImportSource(r) => &r.metadata.name,
        }
    }

    /// Returns the API version of this resource.
    pub fn api_version(&self) -> &str {
        match self {
            AnyResource::Settings(r) => &r.api_version,
            AnyResource::Variable(r) => &r.api_version,
            AnyResource::Rule(r) => &r.api_version,
            AnyResource::ImportSource(r) => &r.api_version,
        }
    }

    /// Returns the metadata of this resource.
    pub fn metadata(&self) -> &ObjectMeta {
        match self {
            AnyResource::Settings(r) => &r.metadata,
            AnyResource::Variable(r) => &r.metadata,
            AnyResource::Rule(r) => &r.metadata,
            AnyResource::ImportSource(r) => &r.metadata,
        }
    }
}

/// Intermediate struct for parsing resources before determining their type.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceHeader {
    pub api_version: String,
    pub kind: ResourceKind,
    pub metadata: ObjectMeta,
}

// ============================================================================
// Resource with path information
// ============================================================================

/// A resource along with its file path.
#[derive(Debug, Clone)]
pub struct ResourceWithPath<T> {
    /// The resource.
    pub resource: T,
    /// The file path relative to the config directory.
    pub path: std::path::PathBuf,
}

impl<T> ResourceWithPath<T> {
    pub fn new(resource: T, path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            resource,
            path: path.into(),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_kind_display() {
        assert_eq!(ResourceKind::Settings.to_string(), "Settings");
        assert_eq!(ResourceKind::Variable.to_string(), "Variable");
        assert_eq!(ResourceKind::Rule.to_string(), "Rule");
        assert_eq!(ResourceKind::ImportSource.to_string(), "ImportSource");
    }

    #[test]
    fn test_resource_kind_from_str() {
        assert_eq!(
            "settings".parse::<ResourceKind>().unwrap(),
            ResourceKind::Settings
        );
        assert_eq!(
            "Settings".parse::<ResourceKind>().unwrap(),
            ResourceKind::Settings
        );
        assert_eq!(
            "variable".parse::<ResourceKind>().unwrap(),
            ResourceKind::Variable
        );
        assert_eq!("rule".parse::<ResourceKind>().unwrap(), ResourceKind::Rule);
        assert_eq!(
            "importsource".parse::<ResourceKind>().unwrap(),
            ResourceKind::ImportSource
        );
        assert!("unknown".parse::<ResourceKind>().is_err());
    }

    #[test]
    fn test_resource_kind_directory() {
        assert_eq!(ResourceKind::Settings.directory(), None);
        assert_eq!(ResourceKind::Variable.directory(), Some("variables"));
        assert_eq!(ResourceKind::Rule.directory(), Some("rules"));
        assert_eq!(ResourceKind::ImportSource.directory(), Some("sources"));
    }

    #[test]
    fn test_object_meta_new() {
        let meta = ObjectMeta::new("test-resource");
        assert_eq!(meta.name, "test-resource");
        assert!(meta.labels.is_empty());
        assert!(meta.annotations.is_empty());
    }

    #[test]
    fn test_object_meta_with_label() {
        let meta = ObjectMeta::new("test")
            .with_label("category", "finance")
            .with_label("team", "backend");
        assert_eq!(meta.labels.get("category"), Some(&"finance".to_string()));
        assert_eq!(meta.labels.get("team"), Some(&"backend".to_string()));
    }

    #[test]
    fn test_settings_resource_creation() {
        let spec = SettingsSpec {
            input_directory: "/data/inbox".to_string(),
            output_directory: "/data/documents".to_string(),
            worker_count: 4,
            ocr: OcrSettings::default(),
            defaults: DefaultOutputSettings::default(),
            git: GitSettings::default(),
            ai: AiSettings::default(),
        };
        let resource: SettingsResource = Resource::new(ResourceKind::Settings, "default", spec);

        assert_eq!(resource.api_version, API_VERSION);
        assert_eq!(resource.kind, ResourceKind::Settings);
        assert_eq!(resource.name(), "default");
        assert_eq!(resource.spec.input_directory, "/data/inbox");
    }

    #[test]
    fn test_variable_resource_creation() {
        let spec = VariableSpec {
            pattern: r"(?i)from[\s:]+(?P<vendor>.+)".to_string(),
            transform: Some(VariableTransform::Slugify),
            default: Some("unknown".to_string()),
        };
        let resource: VariableResource = Resource::new(ResourceKind::Variable, "vendor", spec);

        assert_eq!(resource.kind, ResourceKind::Variable);
        assert_eq!(resource.name(), "vendor");
        assert_eq!(resource.spec.transform, Some(VariableTransform::Slugify));
    }

    #[test]
    fn test_rule_resource_creation() {
        let spec = RuleSpec {
            priority: 100,
            category: "Tax".to_string(),
            match_condition: MatchCondition::Compound(CompoundMatch {
                all: Some(vec![
                    MatchCondition::Simple(SimpleMatch {
                        contains_any: Some(vec!["Invoice".to_string(), "Rechnung".to_string()]),
                        ..Default::default()
                    }),
                    MatchCondition::Simple(SimpleMatch {
                        contains_any: Some(vec!["VAT".to_string(), "MwSt".to_string()]),
                        ..Default::default()
                    }),
                ]),
                ..Default::default()
            }),
            output: OutputSettings {
                directory: "Tax/$year/Invoices".to_string(),
                filename: "$original_$timestamp".to_string(),
            },
            symlinks: vec![SymlinkSettings {
                target: "ByVendor/$vendor".to_string(),
            }],
        };
        let resource: RuleResource = Resource::new(ResourceKind::Rule, "tax-invoices", spec);

        assert_eq!(resource.kind, ResourceKind::Rule);
        assert_eq!(resource.name(), "tax-invoices");
        assert_eq!(resource.spec.priority, 100);
        assert_eq!(resource.spec.category, "Tax");
    }

    #[test]
    fn test_serialize_settings() {
        let spec = SettingsSpec {
            input_directory: "/data/inbox".to_string(),
            output_directory: "/data/documents".to_string(),
            worker_count: 4,
            ocr: OcrSettings::default(),
            defaults: DefaultOutputSettings::default(),
            git: GitSettings::default(),
            ai: AiSettings::default(),
        };
        let resource: SettingsResource = Resource::new(ResourceKind::Settings, "default", spec);

        let yaml = serde_yaml::to_string(&resource).unwrap();
        assert!(yaml.contains("apiVersion: paporg.io/v1"));
        assert!(yaml.contains("kind: Settings"));
        assert!(yaml.contains("name: default"));
    }

    #[test]
    fn test_deserialize_settings() {
        let yaml = r#"
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
"#;
        let resource: SettingsResource = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(resource.api_version, API_VERSION);
        assert_eq!(resource.kind, ResourceKind::Settings);
        assert_eq!(resource.metadata.name, "default");
        assert_eq!(resource.spec.input_directory, "/data/inbox");
        assert_eq!(resource.spec.ocr.languages, vec!["eng", "deu"]);
    }

    #[test]
    fn test_deserialize_variable() {
        let yaml = r#"
apiVersion: paporg.io/v1
kind: Variable
metadata:
  name: vendor
spec:
  pattern: "(?i)from[:\\s]+(?P<vendor>[A-Za-z0-9\\s]+)"
  transform: slugify
  default: unknown
"#;
        let resource: VariableResource = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(resource.kind, ResourceKind::Variable);
        assert_eq!(resource.metadata.name, "vendor");
        assert_eq!(resource.spec.transform, Some(VariableTransform::Slugify));
    }

    #[test]
    fn test_deserialize_rule() {
        let yaml = r#"
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
"#;
        let resource: RuleResource = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(resource.kind, ResourceKind::Rule);
        assert_eq!(resource.metadata.name, "tax-invoices");
        assert_eq!(
            resource.metadata.labels.get("category"),
            Some(&"finance".to_string())
        );
        assert_eq!(resource.spec.priority, 100);
    }

    #[test]
    fn test_any_resource() {
        let yaml = r#"
apiVersion: paporg.io/v1
kind: Variable
metadata:
  name: test
spec:
  pattern: "test"
"#;
        let resource: AnyResource = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(resource.kind(), ResourceKind::Variable);
        assert_eq!(resource.name(), "test");
    }

    #[test]
    fn test_git_auth_type_default() {
        let settings = GitAuthSettings::default();
        assert_eq!(settings.auth_type, GitAuthType::None);
    }

    #[test]
    fn test_ocr_settings_default() {
        let ocr = OcrSettings::default();
        assert!(ocr.enabled);
        assert_eq!(ocr.languages, vec!["eng"]);
        assert_eq!(ocr.dpi, 300);
    }

    #[test]
    fn test_deserialize_email_import_source_password_auth() {
        let yaml = r#"
apiVersion: paporg.io/v1
kind: ImportSource
metadata:
  name: email-invoices
spec:
  type: email
  enabled: true
  email:
    host: imap.gmail.com
    port: 993
    useTls: true
    username: documents@example.com
    auth:
      type: password
      passwordEnvVar: GMAIL_APP_PASSWORD
    folder: INBOX
    sinceDate: "2024-01-01T00:00:00Z"
    mimeFilters:
      include:
        - application/pdf
        - image/*
      filenameExclude:
        - "signature*"
        - "logo*"
    minAttachmentSize: 1024
    maxAttachmentSize: 52428800
    pollInterval: 300
    batchSize: 20
"#;
        let resource: ImportSourceResource = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(resource.kind, ResourceKind::ImportSource);
        assert_eq!(resource.metadata.name, "email-invoices");
        assert_eq!(resource.spec.source_type, ImportSourceType::Email);
        assert!(resource.spec.enabled);

        let email = resource.spec.email.expect("email config should be present");
        assert_eq!(email.host, "imap.gmail.com");
        assert_eq!(email.port, 993);
        assert!(email.use_tls);
        assert_eq!(email.username, "documents@example.com");
        assert_eq!(email.auth.auth_type, EmailAuthType::Password);
        assert_eq!(
            email.auth.password_env_var,
            Some("GMAIL_APP_PASSWORD".to_string())
        );
        assert_eq!(email.folder, "INBOX");
        assert_eq!(email.since_date, Some("2024-01-01T00:00:00Z".to_string()));
        assert_eq!(
            email.mime_filters.include,
            vec!["application/pdf", "image/*"]
        );
        assert_eq!(
            email.mime_filters.filename_exclude,
            vec!["signature*", "logo*"]
        );
        assert_eq!(email.min_attachment_size, 1024);
        assert_eq!(email.max_attachment_size, 52_428_800);
        assert_eq!(email.poll_interval, 300);
        assert_eq!(email.batch_size, 20);
    }

    #[test]
    fn test_deserialize_email_import_source_oauth2_auth() {
        let yaml = r#"
apiVersion: paporg.io/v1
kind: ImportSource
metadata:
  name: gmail-oauth
spec:
  type: email
  enabled: true
  email:
    host: imap.gmail.com
    port: 993
    useTls: true
    username: documents@example.com
    auth:
      type: oauth2
      oauth2:
        clientIdEnvVar: GMAIL_CLIENT_ID
        clientSecretEnvVar: GMAIL_CLIENT_SECRET
        refreshTokenEnvVar: GMAIL_REFRESH_TOKEN
        tokenUrl: https://oauth2.googleapis.com/token
    folder: INBOX
    pollInterval: 300
"#;
        let resource: ImportSourceResource = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(resource.spec.source_type, ImportSourceType::Email);

        let email = resource.spec.email.expect("email config should be present");
        assert_eq!(email.auth.auth_type, EmailAuthType::OAuth2);

        let oauth2 = email.auth.oauth2.expect("oauth2 config should be present");
        assert_eq!(
            oauth2.client_id_env_var,
            Some("GMAIL_CLIENT_ID".to_string())
        );
        assert_eq!(
            oauth2.client_secret_env_var,
            Some("GMAIL_CLIENT_SECRET".to_string())
        );
        assert_eq!(
            oauth2.refresh_token_env_var,
            Some("GMAIL_REFRESH_TOKEN".to_string())
        );
        assert_eq!(
            oauth2.token_url,
            Some("https://oauth2.googleapis.com/token".to_string())
        );
    }

    #[test]
    fn test_email_source_config_defaults() {
        let yaml = r#"
apiVersion: paporg.io/v1
kind: ImportSource
metadata:
  name: minimal-email
spec:
  type: email
  email:
    host: imap.example.com
    username: user@example.com
    auth:
      type: password
      passwordEnvVar: EMAIL_PASSWORD
"#;
        let resource: ImportSourceResource = serde_yaml::from_str(yaml).unwrap();
        let email = resource.spec.email.expect("email config should be present");

        // Check defaults
        assert_eq!(email.port, 993);
        assert!(email.use_tls);
        assert_eq!(email.folder, "INBOX");
        assert!(email.since_date.is_none());
        assert!(email.mime_filters.include.is_empty());
        assert!(email.mime_filters.exclude.is_empty());
        assert_eq!(email.min_attachment_size, 0);
        assert_eq!(email.max_attachment_size, 52_428_800);
        assert_eq!(email.poll_interval, 300);
        assert_eq!(email.batch_size, 50);
    }

    #[test]
    fn test_attachment_filters_default() {
        let filters = AttachmentFilters::default();
        assert!(filters.include.is_empty());
        assert!(filters.exclude.is_empty());
        assert!(filters.filename_include.is_empty());
        assert!(filters.filename_exclude.is_empty());
    }

    #[test]
    fn test_email_auth_type_default() {
        let auth_type = EmailAuthType::default();
        assert_eq!(auth_type, EmailAuthType::Password);
    }
}
