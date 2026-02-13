use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub version: String,
    pub input_directory: String,
    pub output_directory: String,
    #[serde(default = "default_worker_count")]
    pub worker_count: usize,
    #[serde(default)]
    pub ocr: OcrConfig,
    #[serde(default)]
    pub variables: VariablesConfig,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub defaults: DefaultsConfig,
    #[serde(default)]
    pub ai: AiConfig,
}

fn default_worker_count() -> usize {
    num_cpus::get()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_languages")]
    pub languages: Vec<String>,
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

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            languages: default_languages(),
            dpi: 300,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VariablesConfig {
    #[serde(default)]
    pub extracted: Vec<ExtractedVariable>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedVariable {
    pub name: String,
    pub pattern: String,
    #[serde(default)]
    pub transform: Option<VariableTransform>,
    #[serde(default)]
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VariableTransform {
    Slugify,
    Uppercase,
    Lowercase,
    Trim,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(rename = "match")]
    pub match_condition: MatchCondition,
    pub category: String,
    pub output: OutputConfig,
    #[serde(default)]
    pub symlinks: Vec<SymlinkConfig>,
}

/// Custom deserialization for MatchCondition to properly handle untagged enum.
/// The issue is that both SimpleMatch and CompoundMatch have all optional fields,
/// so we need a smarter way to distinguish them.
#[derive(Debug, Clone, Serialize)]
pub enum MatchCondition {
    Simple(SimpleMatch),
    Compound(CompoundMatch),
}

impl<'de> serde::Deserialize<'de> for MatchCondition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;
        use serde_json::Value;

        let value = Value::deserialize(deserializer)?;

        if let Value::Object(map) = &value {
            // Check for compound match keys first
            if map.contains_key("all") || map.contains_key("any") || map.contains_key("not") {
                let compound: CompoundMatch = serde_json::from_value(value)
                    .map_err(|e| D::Error::custom(format!("Invalid compound match: {}", e)))?;
                return Ok(MatchCondition::Compound(compound));
            }
            // Otherwise it's a simple match
            let simple: SimpleMatch = serde_json::from_value(value)
                .map_err(|e| D::Error::custom(format!("Invalid simple match: {}", e)))?;
            return Ok(MatchCondition::Simple(simple));
        }

        Err(D::Error::custom("MatchCondition must be an object"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompoundMatch {
    #[serde(default)]
    pub all: Option<Vec<MatchCondition>>,
    #[serde(default)]
    pub any: Option<Vec<MatchCondition>>,
    #[serde(default)]
    pub not: Option<Box<MatchCondition>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleMatch {
    #[serde(default)]
    pub contains: Option<String>,
    #[serde(rename = "containsAny", default)]
    pub contains_any: Option<Vec<String>>,
    #[serde(rename = "containsAll", default)]
    pub contains_all: Option<Vec<String>>,
    #[serde(default)]
    pub pattern: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub directory: String,
    pub filename: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymlinkConfig {
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultsConfig {
    pub output: OutputConfig,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            output: OutputConfig {
                directory: "$y/unsorted".to_string(),
                filename: "$original_$timestamp".to_string(),
            },
        }
    }
}

/// AI configuration for rule suggestions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    /// Enable AI-powered rule suggestions.
    #[serde(default)]
    pub enabled: bool,
    /// Directory to cache downloaded models.
    #[serde(default = "default_model_cache")]
    pub model_cache_dir: String,
    /// Model repository on Hugging Face.
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
                // Windows fallback using LOCALAPPDATA
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

impl Default for AiConfig {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocumentFormat {
    Pdf,
    Docx,
    Text,
    Image,
}

impl DocumentFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "pdf" => Some(Self::Pdf),
            "docx" => Some(Self::Docx),
            "txt" | "text" | "md" => Some(Self::Text),
            "png" | "jpg" | "jpeg" | "tiff" | "tif" | "bmp" | "gif" | "webp" => Some(Self::Image),
            _ => None,
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Docx => "docx",
            Self::Text => "txt",
            Self::Image => "png",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DocumentMetadata {
    pub original_filename: String,
    pub format: DocumentFormat,
    pub extracted_variables: HashMap<String, String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl DocumentMetadata {
    pub fn new(original_filename: String, format: DocumentFormat) -> Self {
        Self {
            original_filename,
            format,
            extracted_variables: HashMap::new(),
            created_at: chrono::Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_extension_pdf() {
        assert_eq!(
            DocumentFormat::from_extension("pdf"),
            Some(DocumentFormat::Pdf)
        );
        assert_eq!(
            DocumentFormat::from_extension("PDF"),
            Some(DocumentFormat::Pdf)
        );
    }

    #[test]
    fn test_from_extension_docx() {
        assert_eq!(
            DocumentFormat::from_extension("docx"),
            Some(DocumentFormat::Docx)
        );
        assert_eq!(
            DocumentFormat::from_extension("DOCX"),
            Some(DocumentFormat::Docx)
        );
    }

    #[test]
    fn test_from_extension_text_variants() {
        assert_eq!(
            DocumentFormat::from_extension("txt"),
            Some(DocumentFormat::Text)
        );
        assert_eq!(
            DocumentFormat::from_extension("text"),
            Some(DocumentFormat::Text)
        );
        assert_eq!(
            DocumentFormat::from_extension("md"),
            Some(DocumentFormat::Text)
        );
        assert_eq!(
            DocumentFormat::from_extension("TXT"),
            Some(DocumentFormat::Text)
        );
        assert_eq!(
            DocumentFormat::from_extension("MD"),
            Some(DocumentFormat::Text)
        );
    }

    #[test]
    fn test_from_extension_image_variants() {
        assert_eq!(
            DocumentFormat::from_extension("png"),
            Some(DocumentFormat::Image)
        );
        assert_eq!(
            DocumentFormat::from_extension("jpg"),
            Some(DocumentFormat::Image)
        );
        assert_eq!(
            DocumentFormat::from_extension("jpeg"),
            Some(DocumentFormat::Image)
        );
        assert_eq!(
            DocumentFormat::from_extension("tiff"),
            Some(DocumentFormat::Image)
        );
        assert_eq!(
            DocumentFormat::from_extension("tif"),
            Some(DocumentFormat::Image)
        );
        assert_eq!(
            DocumentFormat::from_extension("bmp"),
            Some(DocumentFormat::Image)
        );
        assert_eq!(
            DocumentFormat::from_extension("gif"),
            Some(DocumentFormat::Image)
        );
        assert_eq!(
            DocumentFormat::from_extension("webp"),
            Some(DocumentFormat::Image)
        );
    }

    #[test]
    fn test_from_extension_case_insensitive() {
        assert_eq!(
            DocumentFormat::from_extension("PDF"),
            Some(DocumentFormat::Pdf)
        );
        assert_eq!(
            DocumentFormat::from_extension("Pdf"),
            Some(DocumentFormat::Pdf)
        );
        assert_eq!(
            DocumentFormat::from_extension("pDf"),
            Some(DocumentFormat::Pdf)
        );
        assert_eq!(
            DocumentFormat::from_extension("PNG"),
            Some(DocumentFormat::Image)
        );
        assert_eq!(
            DocumentFormat::from_extension("Png"),
            Some(DocumentFormat::Image)
        );
    }

    #[test]
    fn test_from_extension_unknown() {
        assert_eq!(DocumentFormat::from_extension("xyz"), None);
        assert_eq!(DocumentFormat::from_extension("doc"), None); // Not docx
        assert_eq!(DocumentFormat::from_extension("html"), None);
        assert_eq!(DocumentFormat::from_extension("csv"), None);
        assert_eq!(DocumentFormat::from_extension(""), None);
    }

    #[test]
    fn test_extension_accessor() {
        assert_eq!(DocumentFormat::Pdf.extension(), "pdf");
        assert_eq!(DocumentFormat::Docx.extension(), "docx");
        assert_eq!(DocumentFormat::Text.extension(), "txt");
        assert_eq!(DocumentFormat::Image.extension(), "png");
    }

    #[test]
    fn test_document_metadata_creation() {
        let metadata = DocumentMetadata::new("test.pdf".to_string(), DocumentFormat::Pdf);

        assert_eq!(metadata.original_filename, "test.pdf");
        assert_eq!(metadata.format, DocumentFormat::Pdf);
        assert!(metadata.extracted_variables.is_empty());
    }

    #[test]
    fn test_document_format_equality() {
        assert_eq!(DocumentFormat::Pdf, DocumentFormat::Pdf);
        assert_ne!(DocumentFormat::Pdf, DocumentFormat::Docx);
        assert_ne!(DocumentFormat::Text, DocumentFormat::Image);
    }

    #[test]
    fn test_ocr_config_default() {
        let config = OcrConfig::default();

        assert!(config.enabled);
        assert_eq!(config.languages, vec!["eng".to_string()]);
        assert_eq!(config.dpi, 300);
    }

    #[test]
    fn test_defaults_config_default() {
        let defaults = DefaultsConfig::default();

        assert_eq!(defaults.output.directory, "$y/unsorted");
        assert_eq!(defaults.output.filename, "$original_$timestamp");
    }
}
