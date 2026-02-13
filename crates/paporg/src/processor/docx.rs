use std::io::Read;
use std::path::Path;

use lopdf::{dictionary, Document, Object, Stream};
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::config::schema::{DocumentFormat, DocumentMetadata};
use crate::error::ProcessError;
use crate::processor::{DocumentProcessor, ProcessedContent};

pub struct DocxProcessor;

impl DocxProcessor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DocxProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentProcessor for DocxProcessor {
    fn process(&self, path: &Path) -> Result<ProcessedContent, ProcessError> {
        let file = std::fs::File::open(path).map_err(|e| ProcessError::ReadDocument {
            path: path.to_path_buf(),
            source: e,
        })?;

        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| ProcessError::DocxProcessing(format!("Failed to open DOCX: {}", e)))?;

        // Extract text from document.xml
        let text = extract_docx_text(&mut archive)?;

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document.docx")
            .to_string();

        let metadata = DocumentMetadata::new(filename, DocumentFormat::Docx);

        // Create PDF from extracted text
        let pdf_bytes = create_docx_pdf(&text)?;

        Ok(ProcessedContent {
            text,
            pdf_bytes,
            metadata,
        })
    }

    fn supports(&self, format: DocumentFormat) -> bool {
        matches!(format, DocumentFormat::Docx)
    }
}

fn extract_docx_text<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<String, ProcessError> {
    let mut document_xml = archive
        .by_name("word/document.xml")
        .map_err(|e| ProcessError::DocxProcessing(format!("Failed to find document.xml: {}", e)))?;

    let mut xml_content = String::new();
    document_xml
        .read_to_string(&mut xml_content)
        .map_err(|e| ProcessError::DocxProcessing(format!("Failed to read document.xml: {}", e)))?;

    parse_docx_xml(&xml_content)
}

fn parse_docx_xml(xml: &str) -> Result<String, ProcessError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut text = String::new();
    let mut in_text_element = false;
    let mut in_paragraph = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let local_name = e.local_name();
                match local_name.as_ref() {
                    b"t" => in_text_element = true,
                    b"p" => in_paragraph = true,
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let local_name = e.local_name();
                match local_name.as_ref() {
                    b"t" => in_text_element = false,
                    b"p" => {
                        if in_paragraph {
                            text.push('\n');
                            in_paragraph = false;
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                if in_text_element {
                    let decoded = e.unescape().unwrap_or_default();
                    text.push_str(&decoded);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(ProcessError::DocxProcessing(format!(
                    "XML parsing error: {}",
                    e
                )));
            }
            _ => {}
        }
    }

    Ok(text)
}

fn create_docx_pdf(text: &str) -> Result<Vec<u8>, ProcessError> {
    let mut doc = Document::with_version("1.5");

    let pages_id = doc.new_object_id();
    let font_id = doc.new_object_id();
    let resources_id = doc.new_object_id();

    // Font
    doc.objects.insert(
        font_id,
        Object::Dictionary(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
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

    // Split text into pages (roughly 50 lines per page)
    let lines: Vec<&str> = text.lines().collect();
    let lines_per_page = 50;
    let page_count = lines.len().div_ceil(lines_per_page);
    let page_count = page_count.max(1);

    let mut page_ids = Vec::new();

    for page_num in 0..page_count {
        let start_line = page_num * lines_per_page;
        let end_line = ((page_num + 1) * lines_per_page).min(lines.len());
        let page_lines = if start_line < lines.len() {
            &lines[start_line..end_line]
        } else {
            &[]
        };

        let content_id = doc.new_object_id();
        let page_id = doc.new_object_id();

        // Content stream
        let content = format_text_for_pdf(page_lines);
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

        page_ids.push(page_id);
    }

    // Pages
    let kids: Vec<Object> = page_ids.iter().map(|id| (*id).into()).collect();
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => page_ids.len() as i64,
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

fn format_text_for_pdf(lines: &[&str]) -> String {
    let mut content = String::new();
    content.push_str("BT\n");
    content.push_str("/F1 11 Tf\n");
    content.push_str("50 742 Td\n");
    content.push_str("14 TL\n");

    for line in lines {
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
            _ => " ".to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supports_docx_format() {
        let processor = DocxProcessor::new();
        assert!(processor.supports(DocumentFormat::Docx));
        assert!(!processor.supports(DocumentFormat::Pdf));
        assert!(!processor.supports(DocumentFormat::Text));
        assert!(!processor.supports(DocumentFormat::Image));
    }

    #[test]
    fn test_parse_simple_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:body>
                <w:p>
                    <w:r>
                        <w:t>Hello World</w:t>
                    </w:r>
                </w:p>
            </w:body>
        </w:document>"#;

        let text = parse_docx_xml(xml).unwrap();
        assert!(text.contains("Hello World"));
    }
}
