// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! Document format loaders (PDF, Word, EPUB, etc.)
//!
//! This module contains loaders for common document formats that preserve
//! text and structure from various file types.

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::Result;

/// Loads PDF files as documents.
///
/// The `PDFLoader` reads PDF files and extracts text content.
/// By default, it splits the content into one document per page,
/// but this can be configured.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::PDFLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = PDFLoader::new("document.pdf");
/// let documents = loader.load().await?;
/// println!("Loaded {} documents (pages)", documents.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct PDFLoader {
    /// Path to the PDF file
    pub file_path: PathBuf,
    /// Whether to split into one document per page (default: true)
    pub split_pages: bool,
}

impl PDFLoader {
    /// Create a new `PDFLoader` for the given file path.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::PDFLoader;
    ///
    /// let loader = PDFLoader::new("document.pdf");
    /// ```
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            split_pages: true,
        }
    }

    /// Set whether to split into one document per page.
    ///
    /// If false, the entire PDF becomes a single document.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::PDFLoader;
    ///
    /// let loader = PDFLoader::new("document.pdf")
    ///     .with_split_pages(false);
    /// ```
    #[must_use]
    pub fn with_split_pages(mut self, split_pages: bool) -> Self {
        self.split_pages = split_pages;
        self
    }
}

#[async_trait]
impl DocumentLoader for PDFLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let file_path = self.file_path.clone();
        let split_pages = self.split_pages;

        // Use spawn_blocking to avoid blocking the async runtime with std::fs I/O
        tokio::task::spawn_blocking(move || {
            // Extract text from PDF
            let bytes = std::fs::read(&file_path).map_err(crate::core::error::Error::Io)?;
            let text = pdf_extract::extract_text_from_mem(&bytes).map_err(|e| {
                crate::core::error::Error::InvalidInput(format!(
                    "Failed to extract text from PDF: {e}"
                ))
            })?;

            if split_pages {
                // Split by page breaks (pdf-extract separates pages with form feeds)
                let pages: Vec<&str> = text.split('\x0C').collect();
                let documents = pages
                    .into_iter()
                    .enumerate()
                    .filter(|(_, page_text)| !page_text.trim().is_empty())
                    .map(|(page_num, page_text)| {
                        Document::new(page_text.trim().to_string())
                            .with_metadata("source", file_path.display().to_string())
                            .with_metadata("page", page_num)
                    })
                    .collect();
                Ok(documents)
            } else {
                // Single document for entire PDF
                let doc =
                    Document::new(text).with_metadata("source", file_path.display().to_string());
                Ok(vec![doc])
            }
        })
        .await
        .map_err(|e| crate::core::error::Error::Other(format!("Task join error: {e}")))?
    }
}

/// Loads EPUB (electronic publication) files as documents.
///
/// EPUB files are ZIP archives containing HTML/XHTML content.
/// This loader extracts and parses the HTML content files.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::EpubLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = EpubLoader::new("book.epub");
/// let documents = loader.load().await?;
/// println!("Loaded {} documents", documents.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct EpubLoader {
    file_path: PathBuf,
}

impl EpubLoader {
    /// Create a new `EPub` loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for EpubLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        // EPub is a ZIP archive, so use ZipFileLoader to extract
        use crate::core::document_loaders::ZipFileLoader;

        let zip_loader = ZipFileLoader::new(&self.file_path).with_extract(true);
        let mut docs = zip_loader.load().await?;

        // Filter to only HTML/XHTML content files
        docs.retain(|doc| {
            doc.get_metadata("source")
                .and_then(|v| v.as_str())
                .is_some_and(|s| {
                    s.ends_with(".html") || s.ends_with(".xhtml") || s.ends_with(".htm")
                })
        });

        // Parse HTML content from each file
        let mut result = Vec::new();
        let html_tag_regex = regex::Regex::new(r"<[^>]+>").expect("static regex pattern is valid");
        for doc in docs {
            // Use simple HTML tag stripping for text extraction
            let text = doc
                .page_content
                .replace("<br>", "\n")
                .replace("<br/>", "\n")
                .replace("<br />", "\n")
                .replace("<p>", "\n")
                .replace("</p>", "\n");

            // Remove all HTML tags
            let text = html_tag_regex.replace_all(&text, "").to_string();

            // Decode HTML entities
            let text = html_escape::decode_html_entities(&text).to_string();

            result.push(
                Document::new(text)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("format", "epub")
                    .with_metadata("type", "ebook"),
            );
        }

        Ok(result)
    }
}

/// Loads Microsoft Word (.docx) files as documents.
///
/// Extracts text content from DOCX files by parsing the internal
/// XML structure (word/document.xml).
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::WordDocumentLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = WordDocumentLoader::new("document.docx");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct WordDocumentLoader {
    file_path: PathBuf,
}

impl WordDocumentLoader {
    /// Create a new Word document loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for WordDocumentLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        // Read the docx file (which is a ZIP archive)
        let blob = Blob::from_path(&self.file_path);
        let data = blob.as_bytes()?;

        let reader = std::io::Cursor::new(data);
        let mut archive = zip::ZipArchive::new(reader)?;

        // Extract document.xml which contains the text content
        let mut doc_xml = String::new();
        if let Ok(mut file) = archive.by_name("word/document.xml") {
            std::io::Read::read_to_string(&mut file, &mut doc_xml)?;
        } else {
            return Err(crate::core::error::Error::InvalidInput(
                "Not a valid DOCX file: missing word/document.xml".to_string(),
            ));
        }

        // Parse XML and extract text from <w:t> tags
        let mut text = String::new();
        let re =
            regex::Regex::new(r"<w:t[^>]*>([^<]*)</w:t>").expect("static regex pattern is valid");

        for cap in re.captures_iter(&doc_xml) {
            if let Some(content) = cap.get(1) {
                // Decode XML entities
                let decoded = html_escape::decode_html_entities(content.as_str());
                text.push_str(&decoded);
            }
        }

        let doc = Document::new(text)
            .with_metadata("source", self.file_path.display().to_string())
            .with_metadata("format", "docx");

        Ok(vec![doc])
    }
}

/// Loads `PowerPoint` (.pptx) presentation files as documents.
///
/// The `PowerPointLoader` reads .pptx files and extracts text content from slides.
/// Can optionally create separate documents for each slide.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::PowerPointLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = PowerPointLoader::new("presentation.pptx");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct PowerPointLoader {
    file_path: PathBuf,
    slides_as_documents: bool,
}

impl PowerPointLoader {
    /// Create a new `PowerPoint` loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            slides_as_documents: false,
        }
    }

    /// Set whether to create separate documents for each slide.
    /// Default is false (all slides in one document).
    #[must_use]
    pub fn with_slides_as_documents(mut self, enabled: bool) -> Self {
        self.slides_as_documents = enabled;
        self
    }
}

#[async_trait]
impl DocumentLoader for PowerPointLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let data = blob.as_bytes()?;

        let reader = std::io::Cursor::new(data);
        let mut archive = zip::ZipArchive::new(reader)?;

        let mut slides = Vec::new();

        // Compile regex outside the loop
        let text_tag_regex =
            regex::Regex::new(r"<a:t>([^<]*)</a:t>").expect("static regex pattern is valid");

        // Find all slide XML files
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();

            if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
                let mut content = String::new();
                std::io::Read::read_to_string(&mut file, &mut content)?;

                // Extract text from <a:t> tags
                let mut slide_text = String::new();

                for cap in text_tag_regex.captures_iter(&content) {
                    if let Some(text) = cap.get(1) {
                        let decoded = html_escape::decode_html_entities(text.as_str());
                        if !slide_text.is_empty() {
                            slide_text.push(' ');
                        }
                        slide_text.push_str(&decoded);
                    }
                }

                if !slide_text.is_empty() {
                    slides.push(slide_text);
                }
            }
        }

        if slides.is_empty() {
            return Err(crate::core::error::Error::InvalidInput(
                "No slides found in PPTX file".to_string(),
            ));
        }

        if self.slides_as_documents {
            // Create one document per slide
            Ok(slides
                .into_iter()
                .enumerate()
                .map(|(idx, text)| {
                    Document::new(text)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "pptx")
                        .with_metadata("slide", (idx + 1) as i64)
                })
                .collect())
        } else {
            // Combine all slides into one document
            let combined = slides.join("\n\n");
            let doc = Document::new(combined)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "pptx")
                .with_metadata("slide_count", slides.len() as i64);
            Ok(vec![doc])
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;
    use tempfile::TempDir;
    use zip::write::FileOptions;

    #[tokio::test]
    async fn test_powerpoint_loader() {
        let temp_dir = TempDir::new().unwrap();
        let pptx_path = temp_dir.path().join("test.pptx");

        // Create a minimal PPTX file (ZIP with slide XML)
        let file = std::fs::File::create(&pptx_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);

        let options =
            FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);

        zip.start_file("ppt/slides/slide1.xml", options).unwrap();
        let slide_content = r#"<?xml version="1.0"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:sp>
        <p:txBody>
          <a:p><a:r><a:t>Slide 1 Title</a:t></a:r></a:p>
          <a:p><a:r><a:t>Slide 1 Content</a:t></a:r></a:p>
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>"#;
        std::io::Write::write_all(&mut zip, slide_content.as_bytes()).unwrap();
        zip.finish().unwrap();

        let loader = PowerPointLoader::new(&pptx_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Slide 1 Title"));
        assert!(docs[0].page_content.contains("Slide 1 Content"));
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("pptx")
        );
    }

    #[tokio::test]
    async fn test_powerpoint_loader_slides_as_documents() {
        let temp_dir = TempDir::new().unwrap();
        let pptx_path = temp_dir.path().join("test.pptx");

        // Create a PPTX with 2 slides
        let file = std::fs::File::create(&pptx_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);

        let options =
            FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);

        // Slide 1
        zip.start_file("ppt/slides/slide1.xml", options).unwrap();
        std::io::Write::write_all(&mut zip, r#"<a:t>Slide 1</a:t>"#.as_bytes()).unwrap();

        // Slide 2
        zip.start_file("ppt/slides/slide2.xml", options).unwrap();
        std::io::Write::write_all(&mut zip, r#"<a:t>Slide 2</a:t>"#.as_bytes()).unwrap();

        zip.finish().unwrap();

        let loader = PowerPointLoader::new(&pptx_path).with_slides_as_documents(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].page_content, "Slide 1");
        assert_eq!(docs[1].page_content, "Slide 2");
        assert_eq!(
            docs[0].get_metadata("slide").and_then(|v| v.as_i64()),
            Some(1)
        );
        assert_eq!(
            docs[1].get_metadata("slide").and_then(|v| v.as_i64()),
            Some(2)
        );
    }
}
