use std::path::Path;

use crate::config::schema::{DocumentFormat, DocumentMetadata};
use crate::error::ProcessError;
use crate::processor::{DocumentProcessor, ProcessedContent};

pub struct TextProcessor;

impl TextProcessor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TextProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentProcessor for TextProcessor {
    fn process(&self, path: &Path) -> Result<ProcessedContent, ProcessError> {
        let text = std::fs::read_to_string(path).map_err(|e| ProcessError::ReadDocument {
            path: path.to_path_buf(),
            source: e,
        })?;

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document.txt")
            .to_string();

        let metadata = DocumentMetadata::new(filename, DocumentFormat::Text);

        // Create a simple PDF from text
        let pdf_bytes = create_text_pdf(&text)?;

        Ok(ProcessedContent {
            text,
            pdf_bytes,
            metadata,
        })
    }

    fn supports(&self, format: DocumentFormat) -> bool {
        matches!(format, DocumentFormat::Text)
    }
}

fn create_text_pdf(text: &str) -> Result<Vec<u8>, ProcessError> {
    use lopdf::{dictionary, Document, Object, Stream};

    let mut doc = Document::with_version("1.5");

    let pages_id = doc.new_object_id();
    let font_id = doc.new_object_id();
    let resources_id = doc.new_object_id();
    let content_id = doc.new_object_id();
    let page_id = doc.new_object_id();

    // Font
    doc.objects.insert(
        font_id,
        Object::Dictionary(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Courier",
        }),
    );

    // Resources
    doc.objects.insert(
        resources_id,
        Object::Dictionary(dictionary! {
            "Font" => dictionary! {
                "F1" => font_id,
            },
        }),
    );

    // Content stream - simple text placement
    let content = format_text_for_pdf(text);
    let content_stream = Stream::new(dictionary! {}, content.into_bytes());
    doc.objects
        .insert(content_id, Object::Stream(content_stream));

    // Page
    doc.objects.insert(
        page_id,
        Object::Dictionary(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Resources" => resources_id,
            "Contents" => content_id,
        }),
    );

    // Pages
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        }),
    );

    // Catalog
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buffer = Vec::new();
    doc.save_to(&mut buffer)
        .map_err(|e| ProcessError::PdfProcessing(e.to_string()))?;

    Ok(buffer)
}

fn format_text_for_pdf(text: &str) -> String {
    let mut content = String::new();
    content.push_str("BT\n");
    content.push_str("/F1 10 Tf\n");
    content.push_str("50 742 Td\n");
    content.push_str("12 TL\n");

    for line in text.lines().take(60) {
        // Limit lines per page for simplicity
        let escaped = escape_pdf_string(line);
        content.push_str(&format!("({}) Tj T*\n", escaped));
    }

    content.push_str("ET\n");
    content
}

fn escape_pdf_string(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '(' => "\\(".to_string(),
            ')' => "\\)".to_string(),
            '\\' => "\\\\".to_string(),
            c if c.is_ascii() && !c.is_control() => c.to_string(),
            _ => " ".to_string(), // Replace non-ASCII with space for simplicity
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_process_text_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Hello, World!").unwrap();
        writeln!(temp_file, "This is a test document.").unwrap();

        let processor = TextProcessor::new();
        let result = processor.process(temp_file.path()).unwrap();

        assert!(result.text.contains("Hello, World!"));
        assert!(result.text.contains("This is a test document."));
        assert!(!result.pdf_bytes.is_empty());
    }

    #[test]
    fn test_supports_text_format() {
        let processor = TextProcessor::new();
        assert!(processor.supports(DocumentFormat::Text));
        assert!(!processor.supports(DocumentFormat::Pdf));
        assert!(!processor.supports(DocumentFormat::Docx));
        assert!(!processor.supports(DocumentFormat::Image));
    }
}
