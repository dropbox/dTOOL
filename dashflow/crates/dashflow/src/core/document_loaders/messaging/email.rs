//! Email and messaging format loaders.
//!
//! This module provides loaders for email and calendar/contact formats:
//! - Email (RFC 822 email messages)
//! - EML (email message format)
//! - EMLX (Apple Mail format)
//! - MBOX (mailbox format)
//! - MHTML (MIME HTML format)
//! - ICS (iCalendar format)
//! - VCF (vCard contact format)
//!
//! © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::Result;

/// `EMLLoader` loads EML (email message) files and extracts their content.
///
/// EML is a file format for storing email messages, based on the RFC 822 standard.
/// It preserves the complete email including headers, body, and attachments.
/// Most email clients can export and import messages in this format.
///
/// Supports extensions: .eml
///
/// By default, only common headers (From, To, Cc, Subject, Date) are included.
/// Use `with_all_headers(true)` to include all email headers.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::EMLLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = EMLLoader::new("message.eml").with_all_headers(true);
/// let docs = loader.load().await?;
/// println!("Loaded email: {}", docs[0].page_content);
/// # Ok(())
/// # }
/// ```
pub struct EMLLoader {
    /// Path to the EML file
    pub file_path: PathBuf,
    /// Include full headers (default: false, only includes common headers)
    pub include_all_headers: bool,
}

impl EMLLoader {
    /// Create a new `EMLLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            include_all_headers: false,
        }
    }

    /// Include all email headers, not just common ones.
    #[must_use]
    pub fn with_all_headers(mut self, include: bool) -> Self {
        self.include_all_headers = include;
        self
    }
}

#[async_trait]
impl DocumentLoader for EMLLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        // Simple EML parser - headers until blank line, then body
        let parts: Vec<&str> = content.splitn(2, "\n\n").collect();
        let (headers_text, body) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            (content.as_str(), "")
        };

        // Parse common headers
        let common_headers = ["From", "To", "Cc", "Bcc", "Subject", "Date", "Reply-To"];
        let mut header_content = String::new();
        let mut doc = Document::new("")
            .with_metadata("source", self.file_path.display().to_string())
            .with_metadata("format", "eml");

        // Track current header for continuation lines
        let mut current_key: Option<String> = None;
        let mut current_value = String::new();

        for line in headers_text.lines() {
            if line.trim().is_empty() {
                // Flush current header on blank line
                if let Some(key) = current_key.take() {
                    let value = current_value.trim();
                    let is_common = common_headers.iter().any(|h| h.eq_ignore_ascii_case(&key));

                    if self.include_all_headers || is_common {
                        header_content.push_str(&key);
                        header_content.push_str(": ");
                        header_content.push_str(value);
                        header_content.push('\n');

                        if is_common {
                            doc = doc.with_metadata(
                                key.to_lowercase().replace('-', "_"),
                                value.to_string(),
                            );
                        }
                    }
                    current_value.clear();
                }
                continue;
            }

            // Headers can span multiple lines (continuation lines start with whitespace)
            if line.starts_with(' ') || line.starts_with('\t') {
                // Continuation line - append to current header value
                if current_key.is_some() {
                    current_value.push(' ');
                    current_value.push_str(line.trim());
                }
            } else if let Some(colon_pos) = line.find(':') {
                // New header - flush previous if any
                if let Some(key) = current_key.take() {
                    let value = current_value.trim();
                    let is_common = common_headers.iter().any(|h| h.eq_ignore_ascii_case(&key));

                    if self.include_all_headers || is_common {
                        header_content.push_str(&key);
                        header_content.push_str(": ");
                        header_content.push_str(value);
                        header_content.push('\n');

                        if is_common {
                            doc = doc.with_metadata(
                                key.to_lowercase().replace('-', "_"),
                                value.to_string(),
                            );
                        }
                    }
                    current_value.clear();
                }

                // Start new header
                let (key, value) = line.split_at(colon_pos);
                current_key = Some(key.trim().to_string());
                current_value = value[1..].trim().to_string(); // Skip the colon
            }
        }

        // Flush final header
        if let Some(key) = current_key {
            let value = current_value.trim();
            let is_common = common_headers.iter().any(|h| h.eq_ignore_ascii_case(&key));

            if self.include_all_headers || is_common {
                header_content.push_str(&key);
                header_content.push_str(": ");
                header_content.push_str(value);
                header_content.push('\n');

                if is_common {
                    doc =
                        doc.with_metadata(key.to_lowercase().replace('-', "_"), value.to_string());
                }
            }
        }

        // Combine headers and body
        let mut page_content = header_content;
        page_content.push('\n');
        page_content.push_str(body);

        doc.page_content = page_content;

        Ok(vec![doc])
    }
}

/// Loads BibTeX bibliography (.bib) files.
///
/// The `BibTeXLoader` reads .bib files and extracts citation entries.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::BibTeXLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = BibTeXLoader::new("references.bib");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]

/// Loads iCalendar (ICS) files and extracts calendar events.
///
/// Parses .ics files which contain calendar events, todos, and other scheduling information.
/// Extracts event summaries, descriptions, dates, and other properties into a readable format.
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::ICSLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = ICSLoader::new("calendar.ics");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct ICSLoader {
    file_path: PathBuf,
}

impl ICSLoader {
    /// Create a new ICS loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for ICSLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        // Parse ICS format (simplified)
        // Extract events, todos, journals, etc.
        let mut events = Vec::new();
        let mut current_event: Vec<String> = Vec::new();
        let mut in_event = false;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("BEGIN:VEVENT")
                || trimmed.starts_with("BEGIN:VTODO")
                || trimmed.starts_with("BEGIN:VJOURNAL")
            {
                in_event = true;
                current_event.clear();
                current_event.push(trimmed.to_string());
            } else if trimmed.starts_with("END:VEVENT")
                || trimmed.starts_with("END:VTODO")
                || trimmed.starts_with("END:VJOURNAL")
            {
                if in_event {
                    current_event.push(trimmed.to_string());
                    events.push(current_event.clone());
                    current_event.clear();
                }
                in_event = false;
            } else if in_event {
                current_event.push(trimmed.to_string());
            }
        }

        // Convert events to documents
        let mut documents = Vec::new();
        for event_lines in events {
            let mut summary = String::new();
            let mut description = String::new();
            let mut dtstart = String::new();
            let mut dtend = String::new();
            let mut location = String::new();

            for line in &event_lines {
                if line.starts_with("SUMMARY:") {
                    summary = line.strip_prefix("SUMMARY:").unwrap_or("").to_string();
                } else if line.starts_with("DESCRIPTION:") {
                    description = line.strip_prefix("DESCRIPTION:").unwrap_or("").to_string();
                } else if line.starts_with("DTSTART") {
                    dtstart = line.split(':').nth(1).unwrap_or("").to_string();
                } else if line.starts_with("DTEND") {
                    dtend = line.split(':').nth(1).unwrap_or("").to_string();
                } else if line.starts_with("LOCATION:") {
                    location = line.strip_prefix("LOCATION:").unwrap_or("").to_string();
                }
            }

            // Build readable content
            let mut content_parts = Vec::new();
            if !summary.is_empty() {
                content_parts.push(format!("Summary: {summary}"));
            }
            if !dtstart.is_empty() {
                content_parts.push(format!("Start: {dtstart}"));
            }
            if !dtend.is_empty() {
                content_parts.push(format!("End: {dtend}"));
            }
            if !location.is_empty() {
                content_parts.push(format!("Location: {location}"));
            }
            if !description.is_empty() {
                content_parts.push(format!("Description: {description}"));
            }

            let doc_content = content_parts.join("\n");
            if !doc_content.is_empty() {
                documents.push(
                    Document::new(doc_content)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "ics")
                        .with_metadata("type", "calendar"),
                );
            }
        }

        // If no events found, return the entire content
        if documents.is_empty() {
            documents.push(
                Document::new(content)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("format", "ics")
                    .with_metadata("type", "calendar"),
            );
        }

        Ok(documents)
    }
}

// ============================================================================
// VCF (vCard) Loader
// ============================================================================

/// Loads vCard (VCF) files and extracts contact information.
///
/// Parses .vcf files which contain contact information including names, emails,
/// phone numbers, addresses, and other personal details.
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::VCFLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = VCFLoader::new("contacts.vcf");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct VCFLoader {
    file_path: PathBuf,
}

impl VCFLoader {
    /// Create a new VCF loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for VCFLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        // Parse VCF format
        let mut contacts = Vec::new();
        let mut current_contact: Vec<String> = Vec::new();
        let mut in_vcard = false;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("BEGIN:VCARD") {
                in_vcard = true;
                current_contact.clear();
                current_contact.push(trimmed.to_string());
            } else if trimmed.starts_with("END:VCARD") {
                if in_vcard {
                    current_contact.push(trimmed.to_string());
                    contacts.push(current_contact.clone());
                    current_contact.clear();
                }
                in_vcard = false;
            } else if in_vcard {
                current_contact.push(trimmed.to_string());
            }
        }

        // Convert contacts to documents
        let mut documents = Vec::new();
        for contact_lines in contacts {
            let mut name = String::new();
            let mut emails = Vec::new();
            let mut phones = Vec::new();
            let mut org = String::new();
            let mut title = String::new();
            let mut note = String::new();

            for line in &contact_lines {
                if line.starts_with("FN:") {
                    name = line.strip_prefix("FN:").unwrap_or("").to_string();
                } else if line.starts_with("EMAIL") {
                    if let Some(email) = line.split(':').nth(1) {
                        emails.push(email.to_string());
                    }
                } else if line.starts_with("TEL") {
                    if let Some(phone) = line.split(':').nth(1) {
                        phones.push(phone.to_string());
                    }
                } else if line.starts_with("ORG:") {
                    org = line.strip_prefix("ORG:").unwrap_or("").to_string();
                } else if line.starts_with("TITLE:") {
                    title = line.strip_prefix("TITLE:").unwrap_or("").to_string();
                } else if line.starts_with("NOTE:") {
                    note = line.strip_prefix("NOTE:").unwrap_or("").to_string();
                }
            }

            // Build readable content
            let mut content_parts = Vec::new();
            if !name.is_empty() {
                content_parts.push(format!("Name: {name}"));
            }
            if !org.is_empty() {
                content_parts.push(format!("Organization: {org}"));
            }
            if !title.is_empty() {
                content_parts.push(format!("Title: {title}"));
            }
            for email in &emails {
                content_parts.push(format!("Email: {email}"));
            }
            for phone in &phones {
                content_parts.push(format!("Phone: {phone}"));
            }
            if !note.is_empty() {
                content_parts.push(format!("Note: {note}"));
            }

            let doc_content = content_parts.join("\n");
            if !doc_content.is_empty() {
                documents.push(
                    Document::new(doc_content)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "vcf")
                        .with_metadata("type", "contact"),
                );
            }
        }

        // If no contacts found, return the entire content
        if documents.is_empty() {
            documents.push(
                Document::new(content)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("format", "vcf")
                    .with_metadata("type", "contact"),
            );
        }

        Ok(documents)
    }
}

// ============================================================================
// MBOX (Email Mailbox) Loader
// ============================================================================

/// Loads MBOX email mailbox files.
///
/// Parses .mbox files which contain email messages in Unix mailbox format.
/// Each message is extracted as a separate document with headers and body.
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::MBOXLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = MBOXLoader::new("mailbox.mbox");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct MBOXLoader {
    file_path: PathBuf,
}

impl MBOXLoader {
    /// Create a new MBOX loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for MBOXLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        // Parse MBOX format - messages separated by "From " lines
        let mut documents = Vec::new();
        let mut current_message = Vec::new();

        for line in content.lines() {
            // MBOX format: messages start with "From " (note the space)
            if line.starts_with("From ") && !current_message.is_empty() {
                // Process previous message
                let message_text = current_message.join("\n");
                if !message_text.trim().is_empty() {
                    documents.push(
                        Document::new(message_text)
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "mbox")
                            .with_metadata("type", "email"),
                    );
                }
                current_message.clear();
            }
            current_message.push(line.to_string());
        }

        // Process last message
        if !current_message.is_empty() {
            let message_text = current_message.join("\n");
            if !message_text.trim().is_empty() {
                documents.push(
                    Document::new(message_text)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "mbox")
                        .with_metadata("type", "email"),
                );
            }
        }

        // If no messages found, return entire content
        if documents.is_empty() {
            documents.push(
                Document::new(content)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("format", "mbox")
                    .with_metadata("type", "email"),
            );
        }

        Ok(documents)
    }
}

// ============================================================================
// MHTML (MIME HTML Web Archive) Loader
// ============================================================================

/// Loads MHTML web archive files.
///
/// Parses .mhtml/.mht files which contain complete web pages saved as MIME multipart
/// documents. Extracts the HTML content and other text parts.
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::MHTMLLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = MHTMLLoader::new("webpage.mhtml");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct MHTMLLoader {
    file_path: PathBuf,
}

impl MHTMLLoader {
    /// Create a new MHTML loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for MHTMLLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        // Parse MHTML MIME multipart format
        // Look for text/html and text/plain parts
        let mut text_content = Vec::new();
        let mut in_text_part = false;
        let mut current_part = Vec::new();

        for line in content.lines() {
            // Check for content-type headers
            if line.to_lowercase().starts_with("content-type:") {
                let content_type = line.to_lowercase();
                in_text_part =
                    content_type.contains("text/html") || content_type.contains("text/plain");

                // Save previous part if it was text
                if !current_part.is_empty() {
                    let part_text = current_part.join("\n");
                    if !part_text.trim().is_empty() {
                        text_content.push(part_text);
                    }
                    current_part.clear();
                }
                continue;
            }

            // Check for MIME boundaries (lines starting with --)
            if line.starts_with("--") && line.len() > 10 {
                in_text_part = false;
                if !current_part.is_empty() {
                    let part_text = current_part.join("\n");
                    if !part_text.trim().is_empty() {
                        text_content.push(part_text);
                    }
                    current_part.clear();
                }
                continue;
            }

            // Skip headers until we find the content
            if in_text_part && !line.is_empty() && !line.contains(':') {
                current_part.push(line.to_string());
            }
        }

        // Add final part
        if !current_part.is_empty() {
            let part_text = current_part.join("\n");
            if !part_text.trim().is_empty() {
                text_content.push(part_text);
            }
        }

        let combined_text = if text_content.is_empty() {
            content
        } else {
            text_content.join("\n\n")
        };

        Ok(vec![Document::new(combined_text)
            .with_metadata("source", self.file_path.display().to_string())
            .with_metadata("format", "mhtml")
            .with_metadata("type", "web_archive")])
    }
}

// ============================================================================
// Email Loader - RFC 822 Email Format
// ============================================================================

/// Loader for RFC 822 email message format (.eml, .email, .msg files).
///
/// Parses email headers (From, To, Subject, Date) and body content, creating
/// a single document with the full email message.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::{DocumentLoader, EmailLoader};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = EmailLoader::new("message.eml");
/// let docs = loader.load().await?;
/// println!("Loaded {} email message", docs.len());
/// # Ok(())
/// # }
/// ```
/// Loader for RFC 822 email message format (.eml, .email, .msg files).
///
/// Parses email headers (From, To, Subject, Date) and body content, creating
/// a single document with the full email message.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::{DocumentLoader, EmailLoader};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = EmailLoader::new("message.eml");
/// let docs = loader.load().await?;
/// println!("Loaded {} email message", docs.len());
/// # Ok(())
/// # }
/// ```
pub struct EmailLoader {
    file_path: PathBuf,
}

impl EmailLoader {
    /// Create a new email loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for EmailLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        // Parse RFC 822 email format
        // Headers followed by blank line, then body
        let mut headers = Vec::new();
        let mut body = Vec::new();
        let mut in_body = false;

        for line in content.lines() {
            if in_body {
                body.push(line.to_string());
            } else if line.trim().is_empty() {
                // Blank line separates headers from body
                in_body = true;
            } else {
                // Header line (may span multiple lines with continuation)
                headers.push(line.to_string());
            }
        }

        // Build formatted email content
        let mut email_text = Vec::new();

        // Extract key headers
        let mut from = String::new();
        let mut to = String::new();
        let mut subject = String::new();
        let mut date = String::new();

        for header in &headers {
            let header_lower = header.to_lowercase();
            if header_lower.starts_with("from:") {
                from = header[5..].trim().to_string();
            } else if header_lower.starts_with("to:") {
                to = header[3..].trim().to_string();
            } else if header_lower.starts_with("subject:") {
                subject = header[8..].trim().to_string();
            } else if header_lower.starts_with("date:") {
                date = header[5..].trim().to_string();
            }
        }

        // Format email content
        if !from.is_empty() {
            email_text.push(format!("From: {from}"));
        }
        if !to.is_empty() {
            email_text.push(format!("To: {to}"));
        }
        if !date.is_empty() {
            email_text.push(format!("Date: {date}"));
        }
        if !subject.is_empty() {
            email_text.push(format!("Subject: {subject}"));
        }
        email_text.push(String::new()); // Blank line
        email_text.push(body.join("\n"));

        let email_content = email_text.join("\n");

        Ok(vec![Document::new(email_content)
            .with_metadata("source", self.file_path.display().to_string())
            .with_metadata("format", "email")
            .with_metadata("type", "rfc822")
            .with_metadata("from", from)
            .with_metadata("to", to)
            .with_metadata("subject", subject)
            .with_metadata("date", date)])
    }
}

// ============================================================================
// WARC Loader - Web ARChive Format
// ============================================================================

/// Loader for WARC (Web `ARChive`) format files.
///
/// WARC is the standard format for web crawl archives, used by the Internet
/// Archive and Common Crawl. Parses WARC records and extracts content.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::{DocumentLoader, WARCLoader};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = WARCLoader::new("archive.warc");
/// let docs = loader.load().await?;
/// println!("Loaded {} WARC records", docs.len());
/// # Ok(())
/// # }
/// ```
/// Loader for EMLX (Apple Mail) email message format.
///
/// EMLX is the email storage format used by Apple Mail. Similar to EML but
/// with additional metadata. Parses email headers and body content.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::{DocumentLoader, EMLXLoader};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = EMLXLoader::new("message.emlx");
/// let docs = loader.load().await?;
/// println!("Loaded Apple Mail message");
/// # Ok(())
/// # }
/// ```
pub struct EMLXLoader {
    file_path: PathBuf,
}

impl EMLXLoader {
    /// Create a new EMLX loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for EMLXLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        // EMLX format: first line is byte count, rest is standard email format
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return Ok(vec![]);
        }

        // Skip first line (byte count), parse rest as email
        let email_content: Vec<&str> = lines.iter().skip(1).copied().collect();
        let email_text = email_content.join("\n");

        // Parse email headers and body (similar to EmailLoader)
        let mut headers = Vec::new();
        let mut body = Vec::new();
        let mut in_body = false;

        for line in email_text.lines() {
            if in_body {
                body.push(line.to_string());
            } else if line.trim().is_empty() {
                in_body = true;
            } else {
                headers.push(line.to_string());
            }
        }

        // Extract key headers
        let mut from = String::new();
        let mut to = String::new();
        let mut subject = String::new();
        let mut date = String::new();

        for header in &headers {
            let header_lower = header.to_lowercase();
            if header_lower.starts_with("from:") {
                from = header[5..].trim().to_string();
            } else if header_lower.starts_with("to:") {
                to = header[3..].trim().to_string();
            } else if header_lower.starts_with("subject:") {
                subject = header[8..].trim().to_string();
            } else if header_lower.starts_with("date:") {
                date = header[5..].trim().to_string();
            }
        }

        // Format email content
        let mut formatted_text = Vec::new();
        if !from.is_empty() {
            formatted_text.push(format!("From: {from}"));
        }
        if !to.is_empty() {
            formatted_text.push(format!("To: {to}"));
        }
        if !date.is_empty() {
            formatted_text.push(format!("Date: {date}"));
        }
        if !subject.is_empty() {
            formatted_text.push(format!("Subject: {subject}"));
        }
        formatted_text.push(String::new());
        formatted_text.push(body.join("\n"));

        Ok(vec![Document::new(formatted_text.join("\n"))
            .with_metadata("source", self.file_path.display().to_string())
            .with_metadata("format", "emlx")
            .with_metadata("type", "apple_mail")
            .with_metadata("from", from)
            .with_metadata("to", to)
            .with_metadata("subject", subject)
            .with_metadata("date", date)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // =====================
    // EMLLoader Tests
    // =====================

    #[tokio::test]
    async fn test_eml_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "From: sender@example.com").unwrap();
        writeln!(file, "To: recipient@example.com").unwrap();
        writeln!(file, "Subject: Test Email").unwrap();
        writeln!(file, "Date: Mon, 1 Jan 2024 12:00:00 +0000").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "This is the email body.").unwrap();

        let loader = EMLLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Test Email"));
        assert!(docs[0].page_content.contains("email body"));
        assert_eq!(
            docs[0].metadata.get("format").unwrap().as_str().unwrap(),
            "eml"
        );
    }

    #[tokio::test]
    async fn test_eml_loader_all_headers() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "From: sender@example.com").unwrap();
        writeln!(file, "X-Custom-Header: custom-value").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "Body").unwrap();

        let loader = EMLLoader::new(file.path()).with_all_headers(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("X-Custom-Header"));
    }

    #[tokio::test]
    async fn test_eml_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "From: sender@example.com").unwrap();
        writeln!(file, "Subject: Test").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "Body").unwrap();

        let loader = EMLLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert!(docs[0].metadata.contains_key("from"));
        assert!(docs[0].metadata.contains_key("subject"));
    }

    #[test]
    fn test_eml_loader_builder_chain() {
        let loader = EMLLoader::new("test.eml").with_all_headers(true);

        assert!(loader.include_all_headers);
    }

    // =====================
    // ICSLoader Tests
    // =====================

    #[tokio::test]
    async fn test_ics_loader_basic_event() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "BEGIN:VCALENDAR").unwrap();
        writeln!(file, "BEGIN:VEVENT").unwrap();
        writeln!(file, "SUMMARY:Team Meeting").unwrap();
        writeln!(file, "DTSTART:20240101T100000Z").unwrap();
        writeln!(file, "DTEND:20240101T110000Z").unwrap();
        writeln!(file, "LOCATION:Conference Room A").unwrap();
        writeln!(file, "DESCRIPTION:Weekly sync").unwrap();
        writeln!(file, "END:VEVENT").unwrap();
        writeln!(file, "END:VCALENDAR").unwrap();

        let loader = ICSLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Team Meeting"));
        assert!(docs[0].page_content.contains("Conference Room A"));
        assert_eq!(
            docs[0].metadata.get("format").unwrap().as_str().unwrap(),
            "ics"
        );
    }

    #[tokio::test]
    async fn test_ics_loader_multiple_events() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "BEGIN:VCALENDAR").unwrap();
        writeln!(file, "BEGIN:VEVENT").unwrap();
        writeln!(file, "SUMMARY:Event 1").unwrap();
        writeln!(file, "END:VEVENT").unwrap();
        writeln!(file, "BEGIN:VEVENT").unwrap();
        writeln!(file, "SUMMARY:Event 2").unwrap();
        writeln!(file, "END:VEVENT").unwrap();
        writeln!(file, "END:VCALENDAR").unwrap();

        let loader = ICSLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
    }

    #[tokio::test]
    async fn test_ics_loader_vtodo() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "BEGIN:VCALENDAR").unwrap();
        writeln!(file, "BEGIN:VTODO").unwrap();
        writeln!(file, "SUMMARY:Complete report").unwrap();
        writeln!(file, "END:VTODO").unwrap();
        writeln!(file, "END:VCALENDAR").unwrap();

        let loader = ICSLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Complete report"));
    }

    #[tokio::test]
    async fn test_ics_loader_empty_calendar() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "BEGIN:VCALENDAR").unwrap();
        writeln!(file, "VERSION:2.0").unwrap();
        writeln!(file, "END:VCALENDAR").unwrap();

        let loader = ICSLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        // Returns full content if no events found
        assert_eq!(docs.len(), 1);
    }

    // =====================
    // VCFLoader Tests
    // =====================

    #[tokio::test]
    async fn test_vcf_loader_basic_contact() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "BEGIN:VCARD").unwrap();
        writeln!(file, "VERSION:3.0").unwrap();
        writeln!(file, "FN:John Doe").unwrap();
        writeln!(file, "EMAIL:john@example.com").unwrap();
        writeln!(file, "TEL:+1234567890").unwrap();
        writeln!(file, "ORG:Acme Inc").unwrap();
        writeln!(file, "TITLE:Developer").unwrap();
        writeln!(file, "END:VCARD").unwrap();

        let loader = VCFLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("John Doe"));
        assert!(docs[0].page_content.contains("john@example.com"));
        assert!(docs[0].page_content.contains("Acme Inc"));
        assert_eq!(
            docs[0].metadata.get("format").unwrap().as_str().unwrap(),
            "vcf"
        );
    }

    #[tokio::test]
    async fn test_vcf_loader_multiple_contacts() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "BEGIN:VCARD").unwrap();
        writeln!(file, "FN:Person One").unwrap();
        writeln!(file, "END:VCARD").unwrap();
        writeln!(file, "BEGIN:VCARD").unwrap();
        writeln!(file, "FN:Person Two").unwrap();
        writeln!(file, "END:VCARD").unwrap();

        let loader = VCFLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
    }

    #[tokio::test]
    async fn test_vcf_loader_multiple_emails() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "BEGIN:VCARD").unwrap();
        writeln!(file, "FN:Multi Email").unwrap();
        writeln!(file, "EMAIL;TYPE=WORK:work@example.com").unwrap();
        writeln!(file, "EMAIL;TYPE=HOME:home@example.com").unwrap();
        writeln!(file, "END:VCARD").unwrap();

        let loader = VCFLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("work@example.com"));
        assert!(docs[0].page_content.contains("home@example.com"));
    }

    // =====================
    // MBOXLoader Tests
    // =====================

    #[tokio::test]
    async fn test_mbox_loader_single_message() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "From sender@example.com Mon Jan 1 12:00:00 2024").unwrap();
        writeln!(file, "From: sender@example.com").unwrap();
        writeln!(file, "Subject: Test").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "Message body").unwrap();

        let loader = MBOXLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Message body"));
        assert_eq!(
            docs[0].metadata.get("format").unwrap().as_str().unwrap(),
            "mbox"
        );
    }

    #[tokio::test]
    async fn test_mbox_loader_multiple_messages() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "From sender1@example.com Mon Jan 1 12:00:00 2024").unwrap();
        writeln!(file, "Subject: Message 1").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "Body 1").unwrap();
        writeln!(file, "From sender2@example.com Mon Jan 2 12:00:00 2024").unwrap();
        writeln!(file, "Subject: Message 2").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "Body 2").unwrap();

        let loader = MBOXLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
    }

    // =====================
    // MHTMLLoader Tests
    // =====================

    #[tokio::test]
    async fn test_mhtml_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "MIME-Version: 1.0").unwrap();
        writeln!(file, "Content-Type: text/html; charset=utf-8").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "<html><body>Hello World</body></html>").unwrap();

        let loader = MHTMLLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(
            docs[0].metadata.get("format").unwrap().as_str().unwrap(),
            "mhtml"
        );
    }

    #[tokio::test]
    async fn test_mhtml_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Content-Type: text/plain").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "Plain text content").unwrap();

        let loader = MHTMLLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(
            docs[0].metadata.get("type").unwrap().as_str().unwrap(),
            "web_archive"
        );
    }

    // =====================
    // EmailLoader Tests
    // =====================

    #[tokio::test]
    async fn test_email_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "From: sender@example.com").unwrap();
        writeln!(file, "To: recipient@example.com").unwrap();
        writeln!(file, "Subject: Hello").unwrap();
        writeln!(file, "Date: Mon, 1 Jan 2024 12:00:00 +0000").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "Email body content").unwrap();

        let loader = EmailLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("sender@example.com"));
        assert!(docs[0].page_content.contains("Hello"));
        assert!(docs[0].page_content.contains("Email body content"));
        assert_eq!(
            docs[0].metadata.get("format").unwrap().as_str().unwrap(),
            "email"
        );
    }

    #[tokio::test]
    async fn test_email_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "From: sender@example.com").unwrap();
        writeln!(file, "To: recipient@example.com").unwrap();
        writeln!(file, "Subject: Test Subject").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "Body").unwrap();

        let loader = EmailLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(
            docs[0].metadata.get("from").unwrap().as_str().unwrap(),
            "sender@example.com"
        );
        assert_eq!(
            docs[0].metadata.get("to").unwrap().as_str().unwrap(),
            "recipient@example.com"
        );
        assert_eq!(
            docs[0].metadata.get("subject").unwrap().as_str().unwrap(),
            "Test Subject"
        );
    }

    // =====================
    // EMLXLoader Tests
    // =====================

    #[tokio::test]
    async fn test_emlx_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        // EMLX format: first line is byte count
        writeln!(file, "1234").unwrap();
        writeln!(file, "From: sender@example.com").unwrap();
        writeln!(file, "To: recipient@example.com").unwrap();
        writeln!(file, "Subject: Apple Mail Test").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "Email body").unwrap();

        let loader = EMLXLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Apple Mail Test"));
        assert!(docs[0].page_content.contains("Email body"));
        assert_eq!(
            docs[0].metadata.get("format").unwrap().as_str().unwrap(),
            "emlx"
        );
    }

    #[tokio::test]
    async fn test_emlx_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "500").unwrap();
        writeln!(file, "From: sender@example.com").unwrap();
        writeln!(file, "Subject: Test").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "Body").unwrap();

        let loader = EMLXLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(
            docs[0].metadata.get("type").unwrap().as_str().unwrap(),
            "apple_mail"
        );
        assert!(docs[0].metadata.contains_key("from"));
        assert!(docs[0].metadata.contains_key("subject"));
    }

    #[tokio::test]
    async fn test_emlx_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = EMLXLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 0);
    }

    // =====================
    // Empty File Tests
    // =====================

    #[tokio::test]
    async fn test_eml_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = EMLLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
    }

    #[tokio::test]
    async fn test_ics_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = ICSLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
    }

    #[tokio::test]
    async fn test_vcf_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = VCFLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
    }

    #[tokio::test]
    async fn test_mbox_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = MBOXLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
    }

    #[tokio::test]
    async fn test_mhtml_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = MHTMLLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
    }

    #[tokio::test]
    async fn test_email_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = EmailLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
    }
}
