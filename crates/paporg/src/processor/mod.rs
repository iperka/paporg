pub mod docx;
pub mod image;
pub mod ocr;
pub mod pdf;
pub mod text;

use std::path::Path;

use crate::config::schema::{DocumentFormat, DocumentMetadata};
use crate::error::ProcessError;

pub struct ProcessedContent {
    pub text: String,
    pub pdf_bytes: Vec<u8>,
    pub metadata: DocumentMetadata,
}

pub trait DocumentProcessor: Send + Sync {
    fn process(&self, path: &Path) -> Result<ProcessedContent, ProcessError>;
    fn supports(&self, format: DocumentFormat) -> bool;
}

pub struct ProcessorRegistry {
    processors: Vec<Box<dyn DocumentProcessor>>,
}

impl ProcessorRegistry {
    pub fn new(ocr_enabled: bool, ocr_languages: &[String], ocr_dpi: u32) -> Self {
        let mut processors: Vec<Box<dyn DocumentProcessor>> =
            vec![Box::new(text::TextProcessor::new())];

        if ocr_enabled {
            let ocr = ocr::OcrProcessor::new(ocr_languages, ocr_dpi);
            processors.push(Box::new(image::ImageProcessor::new(ocr.clone())));
            processors.push(Box::new(pdf::PdfProcessor::new(Some(ocr.clone()))));
            processors.push(Box::new(docx::DocxProcessor::new()));
        } else {
            processors.push(Box::new(image::ImageProcessor::new_without_ocr()));
            processors.push(Box::new(pdf::PdfProcessor::new(None)));
            processors.push(Box::new(docx::DocxProcessor::new()));
        }

        Self { processors }
    }

    pub fn process(&self, path: &Path) -> Result<ProcessedContent, ProcessError> {
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let format = DocumentFormat::from_extension(extension)
            .ok_or_else(|| ProcessError::UnsupportedFormat(extension.to_string()))?;

        for processor in &self.processors {
            if processor.supports(format) {
                return processor.process(path);
            }
        }

        Err(ProcessError::UnsupportedFormat(extension.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_registry_routes_text_format() {
        let registry = ProcessorRegistry::new(false, &[], 300);

        let mut temp_file = NamedTempFile::with_suffix(".txt").unwrap();
        writeln!(temp_file, "Test content").unwrap();

        let result = registry.process(temp_file.path());
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert!(processed.text.contains("Test content"));
        assert_eq!(processed.metadata.format, DocumentFormat::Text);
    }

    #[test]
    fn test_registry_routes_md_format() {
        let registry = ProcessorRegistry::new(false, &[], 300);

        let mut temp_file = NamedTempFile::with_suffix(".md").unwrap();
        writeln!(temp_file, "# Markdown Heading").unwrap();

        let result = registry.process(temp_file.path());
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert!(processed.text.contains("# Markdown Heading"));
        assert_eq!(processed.metadata.format, DocumentFormat::Text);
    }

    #[test]
    fn test_unsupported_format_error() {
        let registry = ProcessorRegistry::new(false, &[], 300);

        let temp_file = NamedTempFile::with_suffix(".xyz").unwrap();
        std::fs::write(temp_file.path(), b"some content").unwrap();

        let result = registry.process(temp_file.path());
        assert!(result.is_err());

        match result {
            Err(ProcessError::UnsupportedFormat(ext)) => {
                assert_eq!(ext, "xyz");
            }
            _ => panic!("Expected UnsupportedFormat error"),
        }
    }

    #[test]
    fn test_no_extension_error() {
        let registry = ProcessorRegistry::new(false, &[], 300);

        // Create a temp file without extension
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("noextension");
        std::fs::write(&file_path, b"some content").unwrap();

        let result = registry.process(&file_path);
        assert!(result.is_err());

        match result {
            Err(ProcessError::UnsupportedFormat(ext)) => {
                assert_eq!(ext, "");
            }
            _ => panic!("Expected UnsupportedFormat error for empty extension"),
        }
    }

    #[test]
    fn test_registry_with_ocr_disabled() {
        // OCR disabled - should still process text files
        let registry = ProcessorRegistry::new(false, &[], 300);

        let mut temp_file = NamedTempFile::with_suffix(".txt").unwrap();
        writeln!(temp_file, "OCR disabled test").unwrap();

        let result = registry.process(temp_file.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_registry_with_ocr_enabled() {
        // OCR enabled - processor should be created but text files still work
        let registry = ProcessorRegistry::new(true, &["eng".to_string()], 300);

        let mut temp_file = NamedTempFile::with_suffix(".txt").unwrap();
        writeln!(temp_file, "OCR enabled test").unwrap();

        let result = registry.process(temp_file.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_file_not_found_error() {
        let registry = ProcessorRegistry::new(false, &[], 300);

        let result = registry.process(Path::new("/nonexistent/path/file.txt"));
        assert!(result.is_err());
    }
}
