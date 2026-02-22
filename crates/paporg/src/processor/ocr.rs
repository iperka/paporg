use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use crate::error::ProcessError;

#[derive(Clone)]
pub struct OcrProcessor {
    inner: Arc<OcrProcessorInner>,
}

struct OcrProcessorInner {
    languages: String,
    dpi: u32,
}

impl OcrProcessor {
    pub fn new(languages: &[String], dpi: u32) -> Self {
        let lang_str = if languages.is_empty() {
            "eng".to_string()
        } else {
            languages.join("+")
        };

        Self {
            inner: Arc::new(OcrProcessorInner {
                languages: lang_str,
                dpi,
            }),
        }
    }

    pub fn dpi(&self) -> u32 {
        self.inner.dpi
    }

    pub fn process_image(&self, image_path: &Path) -> Result<String, ProcessError> {
        self.process_image_bytes(&std::fs::read(image_path).map_err(|e| {
            ProcessError::ReadDocument {
                path: image_path.to_path_buf(),
                source: e,
            }
        })?)
    }

    pub fn process_image_bytes(&self, image_data: &[u8]) -> Result<String, ProcessError> {
        let _span = tracing::info_span!("processor.ocr").entered();

        // Load image
        let img = image::load_from_memory(image_data)
            .map_err(|e| ProcessError::OcrFailed(format!("Failed to load image: {}", e)))?;

        // Convert to PNG in memory for leptess
        let mut png_data = Vec::new();
        let mut cursor = Cursor::new(&mut png_data);
        img.write_to(&mut cursor, image::ImageFormat::Png)
            .map_err(|e| ProcessError::OcrFailed(format!("Failed to convert image: {}", e)))?;

        // Create Tesseract instance
        let mut lt = leptess::LepTess::new(None, &self.inner.languages).map_err(|e| {
            ProcessError::OcrFailed(format!("Failed to initialize Tesseract: {}", e))
        })?;

        // Set image from PNG bytes
        lt.set_image_from_mem(&png_data)
            .map_err(|e| ProcessError::OcrFailed(format!("Failed to set image for OCR: {}", e)))?;

        // Get text
        let text = lt
            .get_utf8_text()
            .map_err(|e| ProcessError::OcrFailed(format!("OCR failed: {}", e)))?;

        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ocr_processor_creation() {
        let processor = OcrProcessor::new(&["eng".to_string(), "deu".to_string()], 300);
        assert_eq!(processor.inner.languages, "eng+deu");
        assert_eq!(processor.dpi(), 300);
    }

    #[test]
    fn test_ocr_processor_default_language() {
        let processor = OcrProcessor::new(&[], 300);
        assert_eq!(processor.inner.languages, "eng");
    }

    #[test]
    fn test_ocr_processor_single_language() {
        let processor = OcrProcessor::new(&["fra".to_string()], 300);
        assert_eq!(processor.inner.languages, "fra");
    }

    #[test]
    fn test_ocr_processor_custom_dpi() {
        let processor = OcrProcessor::new(&["eng".to_string()], 150);
        assert_eq!(processor.dpi(), 150);
    }

    #[test]
    fn test_invalid_image_data_error() {
        let processor = OcrProcessor::new(&["eng".to_string()], 300);
        let result = processor.process_image_bytes(b"not valid image data");

        assert!(result.is_err());
        match result {
            Err(ProcessError::OcrFailed(msg)) => {
                assert!(msg.contains("Failed to load image"));
            }
            _ => panic!("Expected OcrFailed error for invalid image data"),
        }
    }

    #[test]
    fn test_empty_image_data_error() {
        let processor = OcrProcessor::new(&["eng".to_string()], 300);
        let result = processor.process_image_bytes(&[]);

        assert!(result.is_err());
        match result {
            Err(ProcessError::OcrFailed(msg)) => {
                assert!(msg.contains("Failed to load image"));
            }
            _ => panic!("Expected OcrFailed error for empty image data"),
        }
    }

    #[test]
    fn test_nonexistent_file_error() {
        let processor = OcrProcessor::new(&["eng".to_string()], 300);
        let result = processor.process_image(Path::new("/nonexistent/image.png"));

        assert!(result.is_err());
        match result {
            Err(ProcessError::ReadDocument { path, .. }) => {
                assert_eq!(path.to_str().unwrap(), "/nonexistent/image.png");
            }
            _ => panic!("Expected ReadDocument error for nonexistent file"),
        }
    }

    #[test]
    fn test_ocr_processor_clone() {
        let processor = OcrProcessor::new(&["eng".to_string(), "deu".to_string()], 300);
        let cloned = processor.clone();

        // Both should have same settings
        assert_eq!(processor.dpi(), cloned.dpi());
        assert_eq!(processor.inner.languages, cloned.inner.languages);
    }
}
