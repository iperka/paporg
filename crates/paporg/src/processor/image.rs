use std::path::Path;

use image::GenericImageView;
use lopdf::{dictionary, Document, Object, Stream};

use crate::config::schema::{DocumentFormat, DocumentMetadata};
use crate::error::ProcessError;
use crate::processor::ocr::OcrProcessor;
use crate::processor::{DocumentProcessor, ProcessedContent};

pub struct ImageProcessor {
    ocr: Option<OcrProcessor>,
}

impl ImageProcessor {
    pub fn new(ocr: OcrProcessor) -> Self {
        Self { ocr: Some(ocr) }
    }

    pub fn new_without_ocr() -> Self {
        Self { ocr: None }
    }
}

impl DocumentProcessor for ImageProcessor {
    fn process(&self, path: &Path) -> Result<ProcessedContent, ProcessError> {
        let _span = tracing::info_span!("processor.image").entered();

        let image_data = std::fs::read(path).map_err(|e| ProcessError::ReadDocument {
            path: path.to_path_buf(),
            source: e,
        })?;

        // Perform OCR if available
        let text = if let Some(ref ocr) = self.ocr {
            ocr.process_image_bytes(&image_data)?
        } else {
            String::new()
        };

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("image")
            .to_string();

        let metadata = DocumentMetadata::new(filename, DocumentFormat::Image);

        // Create PDF with embedded image
        let pdf_bytes = create_image_pdf(&image_data, path)?;

        Ok(ProcessedContent {
            text,
            pdf_bytes,
            metadata,
        })
    }

    fn supports(&self, format: DocumentFormat) -> bool {
        matches!(format, DocumentFormat::Image)
    }
}

fn create_image_pdf(image_data: &[u8], path: &Path) -> Result<Vec<u8>, ProcessError> {
    let img = image::load_from_memory(image_data)
        .map_err(|e| ProcessError::ImageProcessing(format!("Failed to load image: {}", e)))?;

    let (width, height) = img.dimensions();
    let rgb = img.to_rgb8();

    let mut doc = Document::with_version("1.5");

    let pages_id = doc.new_object_id();
    let resources_id = doc.new_object_id();
    let content_id = doc.new_object_id();
    let page_id = doc.new_object_id();
    let image_id = doc.new_object_id();

    // Determine image format
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();

    // Create image XObject
    let image_stream = if extension == "jpg" || extension == "jpeg" {
        // Use JPEG data directly
        Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => width as i64,
                "Height" => height as i64,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
                "Filter" => "DCTDecode",
            },
            image_data.to_vec(),
        )
    } else {
        // Convert to raw RGB for other formats
        Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => width as i64,
                "Height" => height as i64,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
            },
            rgb.into_raw(),
        )
    };

    doc.objects.insert(image_id, Object::Stream(image_stream));

    // Resources with image
    doc.objects.insert(
        resources_id,
        Object::Dictionary(dictionary! {
            "XObject" => dictionary! {
                "Im1" => image_id,
            },
        }),
    );

    // Scale image to fit page (US Letter: 612x792 points)
    let page_width = 612.0_f64;
    let page_height = 792.0_f64;
    let margin = 36.0_f64; // 0.5 inch margin

    let available_width = page_width - 2.0 * margin;
    let available_height = page_height - 2.0 * margin;

    let scale_x = available_width / width as f64;
    let scale_y = available_height / height as f64;
    let scale = scale_x.min(scale_y);

    let img_width = (width as f64 * scale) as i64;
    let img_height = (height as f64 * scale) as i64;
    let x = ((page_width - img_width as f64) / 2.0) as i64;
    let y = ((page_height - img_height as f64) / 2.0) as i64;

    // Content stream to draw image
    let content = format!(
        "q\n{} 0 0 {} {} {} cm\n/Im1 Do\nQ\n",
        img_width, img_height, x, y
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supports_image_format() {
        let processor = ImageProcessor::new_without_ocr();
        assert!(processor.supports(DocumentFormat::Image));
        assert!(!processor.supports(DocumentFormat::Pdf));
        assert!(!processor.supports(DocumentFormat::Text));
        assert!(!processor.supports(DocumentFormat::Docx));
    }
}
