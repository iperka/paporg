//! Email parsing and attachment extraction.

use glob::Pattern;
use log::debug;
use mail_parser::{Message, MessageParser, MimeHeaders, PartType};

use crate::gitops::resource::AttachmentFilters;

use super::error::{EmailError, Result};

/// Information about the email from which an attachment was extracted.
#[derive(Debug, Clone)]
pub struct EmailInfo {
    /// The email's Message-ID header.
    pub message_id: Option<String>,
    /// The email's Subject header.
    pub subject: Option<String>,
    /// The email's From header.
    pub from: Option<String>,
    /// The email's To header.
    pub to: Option<String>,
    /// The email's Date header.
    pub date: Option<String>,
}

/// An attachment extracted from an email.
#[derive(Debug)]
pub struct ExtractedAttachment {
    /// UID of the email this attachment came from.
    pub uid: u32,
    /// The attachment's filename (sanitized).
    pub filename: String,
    /// The attachment's MIME type.
    pub mime_type: String,
    /// The attachment's content.
    pub content: Vec<u8>,
    /// Information about the source email.
    pub email_info: EmailInfo,
}

/// Parser for extracting attachments from email messages.
pub struct EmailParser {
    filters: AttachmentFilters,
    min_size: u64,
    max_size: u64,
    filename_include_patterns: Vec<Pattern>,
    filename_exclude_patterns: Vec<Pattern>,
}

impl EmailParser {
    /// Creates a new email parser with the given filters and size constraints.
    pub fn new(filters: AttachmentFilters, min_size: u64, max_size: u64) -> Self {
        // Pre-compile glob patterns for filenames
        let filename_include_patterns: Vec<Pattern> = filters
            .filename_include
            .iter()
            .filter_map(|p| Pattern::new(p).ok())
            .collect();

        let filename_exclude_patterns: Vec<Pattern> = filters
            .filename_exclude
            .iter()
            .filter_map(|p| Pattern::new(p).ok())
            .collect();

        Self {
            filename_include_patterns,
            filename_exclude_patterns,
            filters,
            min_size,
            max_size,
        }
    }

    /// Extracts attachments from a raw email message.
    pub fn extract_attachments(
        &self,
        raw_email: &[u8],
        uid: u32,
    ) -> Result<Vec<ExtractedAttachment>> {
        let message = MessageParser::default()
            .parse(raw_email)
            .ok_or_else(|| EmailError::ParseError("Failed to parse email message".to_string()))?;

        let email_info = self.extract_email_info(&message);
        let mut attachments = Vec::new();

        debug!(
            "Parsing email UID={} subject={:?}",
            uid,
            email_info.subject.as_deref().unwrap_or("(no subject)")
        );

        // Iterate through all parts of the message
        for part in message.parts.iter() {
            // Check if this part is an attachment
            if !self.is_attachment(part) {
                continue;
            }

            // Get content
            let content = match &part.body {
                PartType::Binary(data) | PartType::InlineBinary(data) => data.to_vec(),
                PartType::Text(text) => text.as_bytes().to_vec(),
                PartType::Html(html) => html.as_bytes().to_vec(),
                _ => continue,
            };

            // Get MIME type
            let mime_type = part
                .content_type()
                .map(|ct| {
                    if let Some(subtype) = ct.subtype() {
                        format!("{}/{}", ct.ctype(), subtype)
                    } else {
                        ct.ctype().to_string()
                    }
                })
                .unwrap_or_else(|| "application/octet-stream".to_string());

            // Get filename
            let filename = self.get_attachment_filename(part, &mime_type);

            // Apply filters
            if !self.passes_filters(&filename, &mime_type, content.len()) {
                debug!("Attachment '{}' ({}) filtered out", filename, mime_type);
                continue;
            }

            debug!(
                "Found attachment: {} ({}, {} bytes)",
                filename,
                mime_type,
                content.len()
            );

            attachments.push(ExtractedAttachment {
                uid,
                filename,
                mime_type,
                content,
                email_info: email_info.clone(),
            });
        }

        debug!(
            "Extracted {} attachments from email UID={}",
            attachments.len(),
            uid
        );
        Ok(attachments)
    }

    /// Extracts metadata from the email message.
    fn extract_email_info(&self, message: &Message) -> EmailInfo {
        EmailInfo {
            message_id: message.message_id().map(|s| s.to_string()),
            subject: message.subject().map(|s| s.to_string()),
            from: message
                .from()
                .and_then(|addr| addr.first().map(format_address)),
            to: message
                .to()
                .and_then(|addr| addr.first().map(format_address)),
            date: message.date().map(|d| d.to_rfc3339()),
        }
    }

    /// Checks if a message part is an attachment.
    fn is_attachment(&self, part: &mail_parser::MessagePart) -> bool {
        // Check Content-Disposition
        if let Some(disposition) = part.content_disposition() {
            if disposition.ctype() == "attachment" {
                return true;
            }
        }

        // Check if it has a filename (inline attachments)
        if part.attachment_name().is_some() {
            return true;
        }

        // Check Content-Type for common attachment types
        if let Some(content_type) = part.content_type() {
            let ctype = content_type.ctype();
            // Exclude text/plain and text/html which are typically body parts
            if ctype != "text" && ctype != "multipart" {
                // Has a subtype and is not a message container
                if content_type.subtype().is_some() && ctype != "message" {
                    return true;
                }
            }
        }

        false
    }

    /// Gets a sanitized filename for an attachment.
    fn get_attachment_filename(&self, part: &mail_parser::MessagePart, mime_type: &str) -> String {
        // Try to get the filename from various sources
        let raw_filename = part
            .attachment_name()
            .or_else(|| part.content_type().and_then(|ct| ct.attribute("name")))
            .map(|s| s.to_string());

        let filename = match raw_filename {
            Some(name) if !name.is_empty() => name,
            _ => {
                // Generate a filename based on MIME type
                let extension = mime_to_extension(mime_type);
                format!("attachment.{}", extension)
            }
        };

        // Sanitize the filename
        sanitize_filename(&filename)
    }

    /// Checks if an attachment passes all filters.
    fn passes_filters(&self, filename: &str, mime_type: &str, size: usize) -> bool {
        let size = size as u64;

        // Check size constraints
        if size < self.min_size {
            debug!(
                "Attachment '{}' too small: {} < {}",
                filename, size, self.min_size
            );
            return false;
        }

        if size > self.max_size {
            debug!(
                "Attachment '{}' too large: {} > {}",
                filename, size, self.max_size
            );
            return false;
        }

        // Check MIME type filters
        if !self.passes_mime_filter(mime_type) {
            return false;
        }

        // Check filename filters
        if !self.passes_filename_filter(filename) {
            return false;
        }

        true
    }

    /// Checks if a MIME type passes the include/exclude filters.
    fn passes_mime_filter(&self, mime_type: &str) -> bool {
        // Check exclude first
        for pattern in &self.filters.exclude {
            if mime_matches(mime_type, pattern) {
                debug!(
                    "MIME type '{}' excluded by pattern '{}'",
                    mime_type, pattern
                );
                return false;
            }
        }

        // If no include patterns, allow all
        if self.filters.include.is_empty() {
            return true;
        }

        // Check if it matches any include pattern
        for pattern in &self.filters.include {
            if mime_matches(mime_type, pattern) {
                return true;
            }
        }

        debug!(
            "MIME type '{}' not in include list: {:?}",
            mime_type, self.filters.include
        );
        false
    }

    /// Checks if a filename passes the include/exclude filters.
    fn passes_filename_filter(&self, filename: &str) -> bool {
        // Check exclude first
        for pattern in &self.filename_exclude_patterns {
            if pattern.matches(filename) {
                debug!("Filename '{}' excluded by pattern '{}'", filename, pattern);
                return false;
            }
        }

        // If no include patterns, allow all
        if self.filename_include_patterns.is_empty() {
            return true;
        }

        // Check if it matches any include pattern
        for pattern in &self.filename_include_patterns {
            if pattern.matches(filename) {
                return true;
            }
        }

        debug!("Filename '{}' not in include list", filename);
        false
    }
}

/// Formats an email address for display.
/// If the address has a display name, formats as "Name <email@example.com>".
/// Otherwise, returns just the email address.
fn format_address(addr: &mail_parser::Addr) -> String {
    if let Some(name) = addr.name() {
        format!("{} <{}>", name, addr.address().unwrap_or_default())
    } else {
        addr.address().unwrap_or_default().to_string()
    }
}

/// Checks if a MIME type matches a pattern.
/// Patterns can be exact matches or use wildcards (e.g., "image/*").
fn mime_matches(mime_type: &str, pattern: &str) -> bool {
    if pattern == "*/*" {
        return true;
    }

    let mime_parts: Vec<&str> = mime_type.split('/').collect();
    let pattern_parts: Vec<&str> = pattern.split('/').collect();

    if mime_parts.len() != 2 || pattern_parts.len() != 2 {
        return false;
    }

    // Check type
    if pattern_parts[0] != "*" && pattern_parts[0] != mime_parts[0] {
        return false;
    }

    // Check subtype
    if pattern_parts[1] != "*" && pattern_parts[1] != mime_parts[1] {
        return false;
    }

    true
}

/// Sanitizes a filename to remove potentially dangerous characters.
fn sanitize_filename(filename: &str) -> String {
    let filename = filename
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();

    // Remove leading/trailing dots and spaces
    let filename = filename.trim_matches(|c| c == '.' || c == ' ');

    // Limit length
    if filename.len() > 255 {
        let ext_start = filename.rfind('.').unwrap_or(filename.len());
        let ext = &filename[ext_start..];
        let base = &filename[..255 - ext.len().min(50)];
        format!("{}{}", base, ext)
    } else if filename.is_empty() {
        "attachment".to_string()
    } else {
        filename.to_string()
    }
}

/// Converts a MIME type to a file extension.
fn mime_to_extension(mime_type: &str) -> &'static str {
    match mime_type.to_lowercase().as_str() {
        "application/pdf" => "pdf",
        "application/msword" => "doc",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "docx",
        "application/vnd.ms-excel" => "xls",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "xlsx",
        "application/vnd.ms-powerpoint" => "ppt",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => "pptx",
        "application/zip" => "zip",
        "application/x-gzip" | "application/gzip" => "gz",
        "application/x-tar" => "tar",
        "application/json" => "json",
        "application/xml" | "text/xml" => "xml",
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "image/tiff" => "tiff",
        "image/bmp" => "bmp",
        "text/plain" => "txt",
        "text/html" => "html",
        "text/csv" => "csv",
        _ => "bin",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mime_matches() {
        // Exact matches
        assert!(mime_matches("application/pdf", "application/pdf"));
        assert!(mime_matches("image/jpeg", "image/jpeg"));

        // Wildcard subtype
        assert!(mime_matches("image/jpeg", "image/*"));
        assert!(mime_matches("image/png", "image/*"));
        assert!(!mime_matches("application/pdf", "image/*"));

        // Wildcard both
        assert!(mime_matches("application/pdf", "*/*"));
        assert!(mime_matches("image/jpeg", "*/*"));

        // No match
        assert!(!mime_matches("application/pdf", "application/json"));
        assert!(!mime_matches("image/jpeg", "application/*"));
    }

    #[test]
    fn test_sanitize_filename() {
        // Normal filename
        assert_eq!(sanitize_filename("document.pdf"), "document.pdf");

        // Filename with special characters
        assert_eq!(sanitize_filename("doc<>ument.pdf"), "doc__ument.pdf");

        // Filename with path traversal attempt
        assert_eq!(
            sanitize_filename("../../../etc/passwd"),
            "_.._.._etc_passwd"
        );

        // Empty filename
        assert_eq!(sanitize_filename(""), "attachment");

        // Filename with only dots
        assert_eq!(sanitize_filename("..."), "attachment");

        // Filename with spaces
        assert_eq!(sanitize_filename("my document.pdf"), "my document.pdf");
    }

    #[test]
    fn test_mime_to_extension() {
        assert_eq!(mime_to_extension("application/pdf"), "pdf");
        assert_eq!(mime_to_extension("image/jpeg"), "jpg");
        assert_eq!(mime_to_extension("APPLICATION/PDF"), "pdf");
        assert_eq!(mime_to_extension("unknown/type"), "bin");
    }

    #[test]
    fn test_parser_size_filtering() {
        let filters = AttachmentFilters {
            include: vec!["application/pdf".to_string()],
            exclude: vec![],
            filename_include: vec![],
            filename_exclude: vec![],
        };

        let parser = EmailParser::new(filters, 100, 1000);

        // Too small
        assert!(!parser.passes_filters("doc.pdf", "application/pdf", 50));

        // Too large
        assert!(!parser.passes_filters("doc.pdf", "application/pdf", 2000));

        // Just right
        assert!(parser.passes_filters("doc.pdf", "application/pdf", 500));
    }

    #[test]
    fn test_parser_mime_filtering() {
        let filters = AttachmentFilters {
            include: vec!["application/pdf".to_string(), "image/*".to_string()],
            exclude: vec!["image/gif".to_string()],
            filename_include: vec![],
            filename_exclude: vec![],
        };

        let parser = EmailParser::new(filters, 0, u64::MAX);

        // Included
        assert!(parser.passes_filters("doc.pdf", "application/pdf", 100));
        assert!(parser.passes_filters("photo.jpg", "image/jpeg", 100));

        // Excluded
        assert!(!parser.passes_filters("anim.gif", "image/gif", 100));

        // Not in include list
        assert!(!parser.passes_filters(
            "doc.docx",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            100
        ));
    }

    #[test]
    fn test_parser_filename_filtering() {
        let filters = AttachmentFilters {
            include: vec![],
            exclude: vec![],
            filename_include: vec!["*.pdf".to_string()],
            filename_exclude: vec!["signature*".to_string()],
        };

        let parser = EmailParser::new(filters, 0, u64::MAX);

        // Included
        assert!(parser.passes_filters("invoice.pdf", "application/pdf", 100));

        // Excluded by pattern
        assert!(!parser.passes_filters("signature.pdf", "application/pdf", 100));

        // Not matching include pattern
        assert!(!parser.passes_filters("invoice.docx", "application/pdf", 100));
    }
}
