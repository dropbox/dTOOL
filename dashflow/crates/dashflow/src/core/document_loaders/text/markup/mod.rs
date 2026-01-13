//! Markup and configuration file loaders.
//!
//! This module provides document loaders for various markup and configuration file formats:
//! - HTML documents
//! - Markdown files
//! - XML files
//! - YAML configuration files
//! - TOML configuration files
//! - INI configuration files

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::Result;

/// Loads HTML files as documents.
///
/// The `HTMLLoader` reads HTML files and converts them to plain text using the html2text library.
/// It supports configurable text wrapping width.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::HTMLLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = HTMLLoader::new("document.html");
/// let documents = loader.load().await?;
/// println!("Loaded {} documents", documents.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct HTMLLoader {
    /// Path to the HTML file
    pub file_path: PathBuf,
    /// Width for text wrapping (default: 80)
    pub width: usize,
}

impl HTMLLoader {
    /// Create a new `HTMLLoader` for the given file path.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::HTMLLoader;
    ///
    /// let loader = HTMLLoader::new("document.html");
    /// ```
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            width: 80,
        }
    }

    /// Set the width for text wrapping.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::HTMLLoader;
    ///
    /// let loader = HTMLLoader::new("document.html")
    ///     .with_width(100);
    /// ```
    #[must_use]
    pub fn with_width(mut self, width: usize) -> Self {
        self.width = width;
        self
    }
}

#[async_trait]
impl DocumentLoader for HTMLLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let file_path = self.file_path.clone();
        let width = self.width;

        // Use spawn_blocking to avoid blocking the async runtime with std::fs I/O
        tokio::task::spawn_blocking(move || {
            // Read the HTML file
            let html_content = std::fs::read(&file_path).map_err(crate::core::error::Error::Io)?;

            // Convert HTML to plain text
            let text = html2text::from_read(&html_content[..], width);

            // Create document with metadata
            let doc = Document::new(text)
                .with_metadata("source", file_path.display().to_string())
                .with_metadata("format", "html");

            Ok(vec![doc])
        })
        .await
        .map_err(|e| crate::core::error::Error::Other(format!("Task join error: {e}")))?
    }
}

/// Loads Markdown files as documents.
///
/// The `MarkdownLoader` reads Markdown files and can either preserve the Markdown
/// formatting or convert to plain text.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::MarkdownLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = MarkdownLoader::new("document.md");
/// let documents = loader.load().await?;
/// println!("Loaded {} documents", documents.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct MarkdownLoader {
    /// Path to the Markdown file
    pub file_path: PathBuf,
    /// Whether to convert to plain text (default: false, keeps markdown)
    pub to_plain_text: bool,
}

impl MarkdownLoader {
    /// Create a new `MarkdownLoader` for the given file path.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::MarkdownLoader;
    ///
    /// let loader = MarkdownLoader::new("document.md");
    /// ```
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            to_plain_text: false,
        }
    }

    /// Set whether to convert to plain text.
    ///
    /// If true, Markdown formatting is removed and only plain text is returned.
    /// If false (default), Markdown formatting is preserved.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::MarkdownLoader;
    ///
    /// let loader = MarkdownLoader::new("document.md")
    ///     .with_plain_text(true);
    /// ```
    #[must_use]
    pub fn with_plain_text(mut self, to_plain_text: bool) -> Self {
        self.to_plain_text = to_plain_text;
        self
    }
}

#[async_trait]
impl DocumentLoader for MarkdownLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let file_path = self.file_path.clone();
        let to_plain_text = self.to_plain_text;

        // Use spawn_blocking to avoid blocking the async runtime with std::fs I/O
        tokio::task::spawn_blocking(move || {
            // Read the Markdown file
            let markdown_content =
                std::fs::read_to_string(&file_path).map_err(crate::core::error::Error::Io)?;

            let content = if to_plain_text {
                // Convert Markdown to plain text
                use pulldown_cmark::{Event, Parser, Tag, TagEnd};

                let parser = Parser::new(&markdown_content);
                let mut plain_text = String::new();

                for event in parser {
                    match event {
                        Event::Text(text) | Event::Code(text) => {
                            plain_text.push_str(&text);
                        }
                        Event::Start(Tag::Paragraph | Tag::Heading { .. }) => {
                            // Add newlines before blocks
                            if !plain_text.is_empty() && !plain_text.ends_with('\n') {
                                plain_text.push('\n');
                            }
                        }
                        Event::End(TagEnd::Paragraph | TagEnd::Heading(_)) => {
                            plain_text.push('\n');
                        }
                        Event::SoftBreak | Event::HardBreak => {
                            plain_text.push('\n');
                        }
                        _ => {}
                    }
                }

                plain_text.trim().to_string()
            } else {
                // Keep original Markdown
                markdown_content
            };

            // Create document with metadata
            let doc = Document::new(content)
                .with_metadata("source", file_path.display().to_string())
                .with_metadata("format", "markdown");

            Ok(vec![doc])
        })
        .await
        .map_err(|e| crate::core::error::Error::Other(format!("Task join error: {e}")))?
    }
}

/// Loads XML files as documents.
///
/// The `XMLLoader` reads XML files and creates a Document with the parsed XML content.
/// It can either preserve the raw XML text or parse it into a structured format.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::XMLLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = XMLLoader::new("example.xml");
/// let documents = loader.load().await?;
/// println!("Loaded {} documents", documents.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct XMLLoader {
    /// Path to the XML file
    pub file_path: PathBuf,
    /// Whether to parse XML structure (true) or keep as raw text (false)
    pub parse_structure: bool,
}

impl XMLLoader {
    /// Create a new `XMLLoader` for the given file path.
    ///
    /// By default, the loader preserves the raw XML text as the document content.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::XMLLoader;
    ///
    /// let loader = XMLLoader::new("example.xml");
    /// ```
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            parse_structure: false,
        }
    }

    /// Configure the loader to parse XML structure into a nested representation.
    ///
    /// When enabled, the XML is parsed and converted to a JSON-like structure
    /// in the document content. When disabled (default), the raw XML text is preserved.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::XMLLoader;
    ///
    /// let loader = XMLLoader::new("example.xml")
    ///     .with_parse_structure(true);
    /// ```
    #[must_use]
    pub fn with_parse_structure(mut self, parse: bool) -> Self {
        self.parse_structure = parse;
        self
    }

    /// Parse XML into a nested `HashMap` structure (similar to `XMLOutputParser` logic).
    fn parse_xml_to_structure(&self, xml_text: &str) -> Result<String> {
        use quick_xml::events::Event;
        use quick_xml::Reader;

        let mut reader = Reader::from_str(xml_text);
        reader.config_mut().trim_text(true);

        let mut result = Vec::new();
        let mut depth = 0;

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    result.push(format!("{}<{}>", "  ".repeat(depth), tag_name));
                    depth += 1;
                }
                Ok(Event::End(ref e)) => {
                    depth = depth.saturating_sub(1);
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    result.push(format!("{}</{}>", "  ".repeat(depth), tag_name));
                }
                Ok(Event::Text(ref e)) => {
                    let text = e
                        .unescape()
                        .map_err(|e| crate::core::error::Error::InvalidInput(e.to_string()))?;
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        result.push(format!("{}{}", "  ".repeat(depth), trimmed));
                    }
                }
                Ok(Event::CData(ref e)) => {
                    // CDATA sections contain character data that should not be parsed
                    // Extract and include the content
                    let text = String::from_utf8_lossy(e.as_ref());
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        result.push(format!("{}{}", "  ".repeat(depth), trimmed));
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(crate::core::error::Error::InvalidInput(format!(
                        "XML parsing error: {e}"
                    )))
                }
                _ => {} // Ignore other events (Comments, Decl, PI, DocType, etc.)
            }
        }

        Ok(result.join("\n"))
    }
}

#[async_trait]
impl DocumentLoader for XMLLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        // Read the XML file
        let blob = Blob::from_path(&self.file_path);
        let xml_content = blob.as_string()?;

        // Choose content format based on parse_structure setting
        let content = if self.parse_structure {
            self.parse_xml_to_structure(&xml_content)?
        } else {
            xml_content
        };

        // Create document with metadata
        let doc = Document::new(content)
            .with_metadata("source", self.file_path.display().to_string())
            .with_metadata("format", "xml");

        Ok(vec![doc])
    }
}

/// Loads a YAML configuration file as a document.
///
/// The `YAMLLoader` reads YAML files and creates a Document with either:
/// - Formatted YAML structure (default)
/// - Raw YAML text
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::YAMLLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = YAMLLoader::new("config.yaml");
/// let documents = loader.load().await?;
/// println!("Loaded {} documents", documents.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct YAMLLoader {
    /// Path to the YAML file
    pub file_path: PathBuf,
    /// Whether to parse and format YAML (true) or keep as raw text (false)
    pub format_yaml: bool,
}

impl YAMLLoader {
    /// Create a new `YAMLLoader` for the given file path.
    ///
    /// By default, the loader formats the YAML content.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::YAMLLoader;
    ///
    /// let loader = YAMLLoader::new("config.yaml");
    /// ```
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            format_yaml: true,
        }
    }

    /// Configure whether to format YAML structure or preserve raw text.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::YAMLLoader;
    ///
    /// let loader = YAMLLoader::new("config.yaml")
    ///     .with_format(false); // Keep raw YAML text
    /// ```
    #[must_use]
    pub fn with_format(mut self, format: bool) -> Self {
        self.format_yaml = format;
        self
    }
}

#[async_trait]
impl DocumentLoader for YAMLLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        // Read the YAML file
        let blob = Blob::from_path(&self.file_path);
        let yaml_content = blob.as_string()?;

        // Parse or keep as-is based on format_yaml setting
        let content = if self.format_yaml {
            // Parse and re-serialize to validate and format
            let value: serde_yml::Value = serde_yml::from_str(&yaml_content).map_err(|e| {
                crate::core::error::Error::InvalidInput(format!("YAML parse error: {e}"))
            })?;
            serde_yml::to_string(&value).map_err(|e| {
                crate::core::error::Error::InvalidInput(format!("YAML format error: {e}"))
            })?
        } else {
            yaml_content
        };

        // Create document with metadata
        let doc = Document::new(content)
            .with_metadata("source", self.file_path.display().to_string())
            .with_metadata("format", "yaml");

        Ok(vec![doc])
    }
}

/// Loads a TOML configuration file as a document.
///
/// The `TOMLLoader` reads TOML files and creates a Document with either:
/// - Formatted TOML structure (default)
/// - Raw TOML text
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::TOMLLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = TOMLLoader::new("Cargo.toml");
/// let documents = loader.load().await?;
/// println!("Loaded {} documents", documents.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct TOMLLoader {
    /// Path to the TOML file
    pub file_path: PathBuf,
    /// Whether to parse and format TOML (true) or keep as raw text (false)
    pub format_toml: bool,
}

impl TOMLLoader {
    /// Create a new `TOMLLoader` for the given file path.
    ///
    /// By default, the loader formats the TOML content.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::TOMLLoader;
    ///
    /// let loader = TOMLLoader::new("config.toml");
    /// ```
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            format_toml: true,
        }
    }

    /// Configure whether to format TOML structure or preserve raw text.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::TOMLLoader;
    ///
    /// let loader = TOMLLoader::new("config.toml")
    ///     .with_format(false); // Keep raw TOML text
    /// ```
    #[must_use]
    pub fn with_format(mut self, format: bool) -> Self {
        self.format_toml = format;
        self
    }
}

#[async_trait]
impl DocumentLoader for TOMLLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        // Read the TOML file
        let blob = Blob::from_path(&self.file_path);
        let toml_content = blob.as_string()?;

        // Parse or keep as-is based on format_toml setting
        let content = if self.format_toml {
            // Parse and re-serialize to validate and format
            let value: toml::Value = toml::from_str(&toml_content).map_err(|e| {
                crate::core::error::Error::InvalidInput(format!("TOML parse error: {e}"))
            })?;
            toml::to_string(&value).map_err(|e| {
                crate::core::error::Error::InvalidInput(format!("TOML format error: {e}"))
            })?
        } else {
            toml_content
        };

        // Create document with metadata
        let doc = Document::new(content)
            .with_metadata("source", self.file_path.display().to_string())
            .with_metadata("format", "toml");

        Ok(vec![doc])
    }
}

/// Loads an INI configuration file as a document.
///
/// The `IniLoader` reads INI files and creates a Document with the parsed
/// INI content formatted as text with sections and key-value pairs.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::IniLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = IniLoader::new("config.ini");
/// let documents = loader.load().await?;
/// println!("Loaded {} documents", documents.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct IniLoader {
    /// Path to the INI file
    pub file_path: PathBuf,
    /// Whether to parse and format INI (true) or keep as raw text (false)
    pub format_ini: bool,
}

impl IniLoader {
    /// Create a new `IniLoader` for the given file path.
    ///
    /// By default, the loader formats the INI content.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::IniLoader;
    ///
    /// let loader = IniLoader::new("config.ini");
    /// ```
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            format_ini: true,
        }
    }

    /// Configure whether to format INI structure or preserve raw text.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::IniLoader;
    ///
    /// let loader = IniLoader::new("config.ini")
    ///     .with_format(false); // Keep raw INI text
    /// ```
    #[must_use]
    pub fn with_format(mut self, format: bool) -> Self {
        self.format_ini = format;
        self
    }
}

#[async_trait]
impl DocumentLoader for IniLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        // Read the INI file
        let blob = Blob::from_path(&self.file_path);
        let ini_content = blob.as_string()?;

        // Parse or keep as-is based on format_ini setting
        let content = if self.format_ini {
            // Parse INI using the ini crate's simple API
            let parsed = ini::macro_safe_read(&ini_content).map_err(|e| {
                crate::core::error::Error::InvalidInput(format!("INI parse error: {e}"))
            })?;

            let mut formatted = String::new();

            // Format all sections
            for (section_name, properties) in &parsed {
                formatted.push_str(&format!("[{section_name}]\n"));

                for (key, value_opt) in properties {
                    let value = value_opt.as_deref().unwrap_or("");
                    formatted.push_str(&format!("{key} = {value}\n"));
                }

                formatted.push('\n');
            }

            formatted
        } else {
            ini_content
        };

        // Create document with metadata
        let doc = Document::new(content)
            .with_metadata("source", self.file_path.display().to_string())
            .with_metadata("format", "ini");

        Ok(vec![doc])
    }
}

#[cfg(test)]
mod tests;
