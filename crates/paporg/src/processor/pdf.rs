use std::path::Path;
use std::process::Command;

use crate::config::schema::{DocumentFormat, DocumentMetadata};
use crate::error::ProcessError;
use crate::processor::ocr::OcrProcessor;
use crate::processor::{DocumentProcessor, ProcessedContent};

pub struct PdfProcessor {
    ocr: Option<OcrProcessor>,
}

impl PdfProcessor {
    pub fn new(ocr: Option<OcrProcessor>) -> Self {
        Self { ocr }
    }
}

impl DocumentProcessor for PdfProcessor {
    fn process(&self, path: &Path) -> Result<ProcessedContent, ProcessError> {
        let _span = tracing::info_span!("processor.pdf").entered();

        let pdf_bytes = std::fs::read(path).map_err(|e| ProcessError::ReadDocument {
            path: path.to_path_buf(),
            source: e,
        })?;

        let text = match lopdf::Document::load_mem(&pdf_bytes) {
            Ok(doc) => {
                // Extract text from PDF
                let mut text = extract_text_from_pdf(&doc)?;

                // If no usable text was extracted and OCR is available, try OCR
                if should_use_ocr(&text) {
                    if let Some(ref ocr) = self.ocr {
                        let _ocr_span =
                            tracing::info_span!("processor.ocr_fallback", reason = "text_quality")
                                .entered();
                        text = self.ocr_pdf(&pdf_bytes, &doc, ocr)?;
                    }
                }
                text
            }
            Err(e) => {
                // lopdf can't parse this PDF (e.g. invalid cross-reference table).
                // Fall back to OCR via pdftoppm/poppler which handles more PDF variants.
                tracing::warn!(
                    "lopdf failed to parse {}: {}. Falling back to OCR.",
                    path.display(),
                    e
                );
                if let Some(ref ocr) = self.ocr {
                    let _ocr_span = tracing::info_span!(
                        "processor.ocr_fallback",
                        reason = "lopdf_parse_failed"
                    )
                    .entered();
                    self.ocr_pdf_without_doc(&pdf_bytes, ocr)?
                } else {
                    return Err(ProcessError::PdfProcessing(format!(
                        "Failed to load PDF: {}. OCR fallback unavailable.",
                        e
                    )));
                }
            }
        };

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document.pdf")
            .to_string();

        let metadata = DocumentMetadata::new(filename, DocumentFormat::Pdf);

        Ok(ProcessedContent {
            text,
            pdf_bytes,
            metadata,
        })
    }

    fn supports(&self, format: DocumentFormat) -> bool {
        matches!(format, DocumentFormat::Pdf)
    }
}

impl PdfProcessor {
    fn ocr_pdf(
        &self,
        pdf_bytes: &[u8],
        doc: &lopdf::Document,
        ocr: &OcrProcessor,
    ) -> Result<String, ProcessError> {
        let page_count = doc.get_pages().len();
        self.ocr_pages(pdf_bytes, page_count, ocr)
    }

    /// OCR a PDF when lopdf can't parse it. Uses pdftoppm to discover page count.
    fn ocr_pdf_without_doc(
        &self,
        pdf_bytes: &[u8],
        ocr: &OcrProcessor,
    ) -> Result<String, ProcessError> {
        let page_count = count_pdf_pages(pdf_bytes)?;
        self.ocr_pages(pdf_bytes, page_count, ocr)
    }

    fn ocr_pages(
        &self,
        pdf_bytes: &[u8],
        page_count: usize,
        ocr: &OcrProcessor,
    ) -> Result<String, ProcessError> {
        let mut all_text = String::new();

        for page_num in 1..=page_count {
            if let Ok(image_data) = render_pdf_page_to_image(pdf_bytes, page_num as u32, ocr.dpi())
            {
                if let Ok(page_text) = ocr.process_image_bytes(&image_data) {
                    all_text.push_str(&page_text);
                    all_text.push('\n');
                }
            }
        }

        Ok(all_text)
    }
}

fn extract_text_from_pdf(doc: &lopdf::Document) -> Result<String, ProcessError> {
    let mut text = String::new();

    for (page_num, _) in doc.get_pages() {
        if let Ok(page_text) = doc.extract_text(&[page_num]) {
            text.push_str(&page_text);
            text.push('\n');
        }
    }

    Ok(text)
}

/// Pattern for Identity-H Unimplemented errors (common with CID fonts).
const IDENTITY_H_PATTERN: &str = "?Identity-H Unimplemented?";

/// Minimum number of characters required before applying alphanumeric ratio check.
/// Text shorter than this is considered valid regardless of character composition.
const MIN_TOTAL_CHARS: usize = 50;

/// Minimum percentage of alphanumeric characters required for text to be considered valid.
/// If alphanumeric ratio is below this threshold, OCR will be used instead.
const MIN_ALPHANUMERIC_PERCENT: usize = 10;

/// Determines if OCR should be used instead of extracted text.
/// Returns true if:
/// - Text is empty or whitespace only
/// - Text contains only font encoding error markers (Identity-H Unimplemented)
/// - Text contains very high ratio of non-printable/garbled characters
fn should_use_ocr(text: &str) -> bool {
    let trimmed = text.trim();

    // Empty text - definitely use OCR
    if trimmed.is_empty() {
        return true;
    }

    // Check for Identity-H Unimplemented errors (common with CID fonts)
    // If most of the text is this error pattern, use OCR
    let cleaned = trimmed
        .replace(IDENTITY_H_PATTERN, "")
        .replace(['\n', ' '], "");

    if cleaned.is_empty() {
        return true;
    }

    // Count ratio of printable alphanumeric characters
    // Use chars().count() instead of len() to correctly handle Unicode/non-ASCII text
    let total_chars = trimmed.chars().count();
    let alphanumeric_chars = trimmed.chars().filter(|c| c.is_alphanumeric()).count();

    // If less than MIN_ALPHANUMERIC_PERCENT of text is alphanumeric, it's likely garbled
    if total_chars > MIN_TOTAL_CHARS
        && alphanumeric_chars * 100 < total_chars * MIN_ALPHANUMERIC_PERCENT
    {
        return true;
    }

    false
}

/// Get the page count of a PDF using pdfinfo (poppler-utils).
/// Used as fallback when lopdf can't parse the PDF structure.
fn count_pdf_pages(pdf_bytes: &[u8]) -> Result<usize, ProcessError> {
    let temp_dir = std::env::temp_dir();
    let pdf_path = temp_dir.join(format!("paporg_pagecount_{}.pdf", uuid::Uuid::new_v4()));

    std::fs::write(&pdf_path, pdf_bytes)
        .map_err(|e| ProcessError::PdfProcessing(format!("Failed to write temp PDF: {}", e)))?;

    let output = Command::new("pdfinfo")
        .arg(&pdf_path)
        .output()
        .map_err(|e| {
            let _ = std::fs::remove_file(&pdf_path);
            ProcessError::PdfProcessing(format!(
                "Failed to run pdfinfo: {}. Make sure poppler-utils is installed.",
                e
            ))
        })?;

    let _ = std::fs::remove_file(&pdf_path);

    if !output.status.success() {
        return Err(ProcessError::PdfProcessing(format!(
            "pdfinfo failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(count_str) = line.strip_prefix("Pages:") {
            if let Ok(count) = count_str.trim().parse::<usize>() {
                return Ok(count);
            }
        }
    }

    // Default to 1 page if we can't determine the count
    Ok(1)
}

fn render_pdf_page_to_image(
    pdf_bytes: &[u8],
    page_num: u32,
    dpi: u32,
) -> Result<Vec<u8>, ProcessError> {
    // Use poppler/pdfimages via command line for rendering
    // This is a fallback approach - in production, you might use pdfium or similar

    // Write PDF to temp file
    let temp_dir = std::env::temp_dir();
    let pdf_path = temp_dir.join(format!("paporg_temp_{}.pdf", uuid::Uuid::new_v4()));
    let output_prefix = temp_dir.join(format!("paporg_page_{}", uuid::Uuid::new_v4()));

    std::fs::write(&pdf_path, pdf_bytes)
        .map_err(|e| ProcessError::PdfProcessing(format!("Failed to write temp PDF: {}", e)))?;

    // Use pdftoppm to render page
    let output = Command::new("pdftoppm")
        .args([
            "-png",
            "-r",
            &dpi.to_string(),
            "-f",
            &page_num.to_string(),
            "-l",
            &page_num.to_string(),
            pdf_path.to_str().unwrap(),
            output_prefix.to_str().unwrap(),
        ])
        .output()
        .map_err(|e| {
            ProcessError::PdfProcessing(format!(
                "Failed to run pdftoppm: {}. Make sure poppler-utils is installed.",
                e
            ))
        })?;

    // Clean up temp PDF
    let _ = std::fs::remove_file(&pdf_path);

    if !output.status.success() {
        return Err(ProcessError::PdfProcessing(format!(
            "pdftoppm failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    // Find the output file (pdftoppm adds page number suffix)
    let output_path = format!("{}-{}.png", output_prefix.display(), page_num);
    let output_path_alt = format!("{}-{:02}.png", output_prefix.display(), page_num);
    let output_path_alt2 = format!("{}-{:03}.png", output_prefix.display(), page_num);

    let paths = [output_path, output_path_alt, output_path_alt2];
    let image_path = paths
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .ok_or_else(|| {
            ProcessError::PdfProcessing("Failed to find rendered page image".to_string())
        })?;

    let image_data = std::fs::read(image_path).map_err(|e| {
        ProcessError::PdfProcessing(format!("Failed to read rendered image: {}", e))
    })?;

    // Clean up temp image
    let _ = std::fs::remove_file(image_path);

    Ok(image_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_supports_pdf_format() {
        let processor = PdfProcessor::new(None);
        assert!(processor.supports(DocumentFormat::Pdf));
        assert!(!processor.supports(DocumentFormat::Image));
        assert!(!processor.supports(DocumentFormat::Text));
        assert!(!processor.supports(DocumentFormat::Docx));
    }

    #[test]
    fn test_process_pdf_with_embedded_text() {
        // Create a minimal valid PDF with embedded text
        use lopdf::{dictionary, Document, Object, Stream};

        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = doc.new_object_id();
        let resources_id = doc.new_object_id();
        let content_id = doc.new_object_id();
        let page_id = doc.new_object_id();

        doc.objects.insert(
            font_id,
            Object::Dictionary(dictionary! {
                "Type" => "Font",
                "Subtype" => "Type1",
                "BaseFont" => "Courier",
            }),
        );

        doc.objects.insert(
            resources_id,
            Object::Dictionary(dictionary! {
                "Font" => dictionary! {
                    "F1" => font_id,
                },
            }),
        );

        let content = "BT /F1 12 Tf 50 700 Td (Test PDF Content) Tj ET";
        let content_stream = Stream::new(dictionary! {}, content.as_bytes().to_vec());
        doc.objects
            .insert(content_id, Object::Stream(content_stream));

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

        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page_id.into()],
                "Count" => 1,
            }),
        );

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let mut pdf_bytes = Vec::new();
        doc.save_to(&mut pdf_bytes).unwrap();

        // Write to temp file
        let mut temp_file = NamedTempFile::with_suffix(".pdf").unwrap();
        std::io::Write::write_all(&mut temp_file, &pdf_bytes).unwrap();

        let processor = PdfProcessor::new(None);
        let result = processor.process(temp_file.path());

        assert!(result.is_ok());
        let processed = result.unwrap();
        assert!(!processed.pdf_bytes.is_empty());
        assert_eq!(processed.metadata.format, DocumentFormat::Pdf);
    }

    #[test]
    fn test_corrupted_pdf_error() {
        let temp_file = NamedTempFile::with_suffix(".pdf").unwrap();
        std::fs::write(temp_file.path(), b"not a valid pdf content").unwrap();

        // Without OCR, lopdf parse failure should error with "OCR fallback unavailable"
        let processor = PdfProcessor::new(None);
        let result = processor.process(temp_file.path());

        assert!(result.is_err());
        match result {
            Err(ProcessError::PdfProcessing(msg)) => {
                assert!(
                    msg.contains("Failed to load PDF"),
                    "Expected 'Failed to load PDF' in: {}",
                    msg
                );
            }
            _ => panic!("Expected PdfProcessing error"),
        }
    }

    #[test]
    fn test_pdf_file_not_found_error() {
        let processor = PdfProcessor::new(None);
        let result = processor.process(Path::new("/nonexistent/file.pdf"));

        assert!(result.is_err());
        match result {
            Err(ProcessError::ReadDocument { path, .. }) => {
                assert_eq!(path.to_str().unwrap(), "/nonexistent/file.pdf");
            }
            _ => panic!("Expected ReadDocument error"),
        }
    }

    #[test]
    fn test_empty_pdf_minimal() {
        // Create a minimal empty PDF
        use lopdf::{dictionary, Document, Object};

        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let page_id = doc.new_object_id();

        doc.objects.insert(
            page_id,
            Object::Dictionary(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            }),
        );

        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page_id.into()],
                "Count" => 1,
            }),
        );

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let mut pdf_bytes = Vec::new();
        doc.save_to(&mut pdf_bytes).unwrap();

        let temp_file = NamedTempFile::with_suffix(".pdf").unwrap();
        std::fs::write(temp_file.path(), &pdf_bytes).unwrap();

        let processor = PdfProcessor::new(None);
        let result = processor.process(temp_file.path());

        // Empty PDF should process but have empty text
        assert!(result.is_ok());
        let processed = result.unwrap();
        assert!(processed.text.trim().is_empty());
    }

    // ============================================
    // should_use_ocr tests
    // ============================================

    #[test]
    fn test_should_use_ocr_empty_text() {
        assert!(should_use_ocr(""));
        assert!(should_use_ocr("   "));
        assert!(should_use_ocr("\n\n\n"));
        assert!(should_use_ocr("  \t  \n  "));
    }

    #[test]
    fn test_should_use_ocr_identity_h_only() {
        // Text that is only Identity-H errors should use OCR
        let text = "?Identity-H Unimplemented? ?Identity-H Unimplemented?";
        assert!(should_use_ocr(text));

        // Mixed with spaces/newlines
        let text = "?Identity-H Unimplemented?\n\n?Identity-H Unimplemented?";
        assert!(should_use_ocr(text));
    }

    #[test]
    fn test_should_use_ocr_valid_text() {
        // Normal text with good alphanumeric ratio
        assert!(!should_use_ocr("This is a normal document with text"));
        assert!(!should_use_ocr("Invoice #12345 for John Doe"));

        // Short text is always valid (below MIN_TOTAL_CHARS threshold)
        assert!(!should_use_ocr("Hi"));
        assert!(!should_use_ocr("!@#$%")); // Short, so not checked for ratio
    }

    #[test]
    fn test_should_use_ocr_garbled_text() {
        // Text with very low alphanumeric ratio (below MIN_ALPHANUMERIC_PERCENT)
        // This simulates garbled output from font encoding issues
        // Need > MIN_TOTAL_CHARS (50) characters total
        let garbled = "!@#$%^&*(){}[]|\\:\";<>?,./~`!@#$%^&*(){}[]|\\:\";<>?,./~`!!";
        // Use chars().count() to match production code's Unicode-aware counting
        assert!(garbled.chars().count() > MIN_TOTAL_CHARS);
        assert!(should_use_ocr(garbled));
    }

    #[test]
    fn test_should_use_ocr_threshold_boundary() {
        // Create text above MIN_TOTAL_CHARS (50) to trigger the ratio check

        // Text with ~12% alphanumeric (above 10% threshold) - should NOT use OCR
        // 6 alphanumeric out of 51 chars = 11.7%
        let mut text = String::from("abcdef");
        text.push_str(&"!".repeat(45)); // 6 + 45 = 51 chars
        assert!(text.chars().count() > MIN_TOTAL_CHARS);
        let alnum = text.chars().filter(|c| c.is_alphanumeric()).count();
        let total = text.chars().count();
        assert!(
            alnum * 100 >= total * MIN_ALPHANUMERIC_PERCENT,
            "alnum={}, total={}, ratio={}%",
            alnum,
            total,
            alnum * 100 / total
        );
        assert!(!should_use_ocr(&text));

        // Text with ~7.8% alphanumeric (below 10% threshold) - should use OCR
        // 4 alphanumeric out of 51 chars = 7.8%
        let mut text = String::from("abcd");
        text.push_str(&"!".repeat(47)); // 4 + 47 = 51 chars
        assert!(text.chars().count() > MIN_TOTAL_CHARS);
        let alnum = text.chars().filter(|c| c.is_alphanumeric()).count();
        let total = text.chars().count();
        assert!(
            alnum * 100 < total * MIN_ALPHANUMERIC_PERCENT,
            "alnum={}, total={}, ratio={}%",
            alnum,
            total,
            alnum * 100 / total
        );
        assert!(should_use_ocr(&text));
    }

    #[test]
    fn test_should_use_ocr_unicode_text() {
        // Unicode text should work correctly
        assert!(!should_use_ocr("日本語のテキスト")); // Japanese text
        assert!(!should_use_ocr("Émoji test: 🔐 works great"));
        assert!(!should_use_ocr("Ünïcödé chàrâctérs wörk"));
    }

    #[test]
    fn test_should_use_ocr_mixed_identity_h_with_content() {
        // Text mixing real content with Identity-H tokens should NOT use OCR
        // because there's still meaningful extracted text
        let text = "Invoice #123 ?Identity-H Unimplemented? Total: $500";
        assert!(!should_use_ocr(text));

        // Multiple Identity-H markers but still significant real content
        let text = "Document Title ?Identity-H Unimplemented? Section 1 ?Identity-H Unimplemented? Content here";
        assert!(!should_use_ocr(text));
    }

    #[test]
    fn test_should_use_ocr_exactly_min_chars() {
        // Test with exactly MIN_TOTAL_CHARS characters (boundary test)
        // All alphanumeric - should NOT use OCR (ratio check doesn't apply at boundary)
        let text = "a".repeat(MIN_TOTAL_CHARS);
        assert_eq!(text.chars().count(), MIN_TOTAL_CHARS);
        assert!(!should_use_ocr(&text));

        // All non-alphanumeric at exactly threshold - ratio check should NOT trigger
        // because condition is total_chars > MIN_TOTAL_CHARS (strict greater than)
        let text = "!".repeat(MIN_TOTAL_CHARS);
        assert_eq!(text.chars().count(), MIN_TOTAL_CHARS);
        assert!(!should_use_ocr(&text)); // Not triggered because not > threshold

        // One char over threshold with all non-alphanumeric - should use OCR
        let text = "!".repeat(MIN_TOTAL_CHARS + 1);
        assert!(text.chars().count() > MIN_TOTAL_CHARS);
        assert!(should_use_ocr(&text));
    }
}
