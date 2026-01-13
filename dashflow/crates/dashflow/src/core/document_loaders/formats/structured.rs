// Allow clippy warnings for structured data loaders
// - needless_pass_by_value: json_value consumed by serialization in create_document
#![allow(clippy::needless_pass_by_value)]

//! Structured data format document loaders.
//!
//! This module provides loaders for structured data file formats including:
//! - CSV (.csv) - Comma-separated values
//! - TSV (.tsv) - Tab-separated values
//! - JSON (.json) - JavaScript Object Notation
//! - XML (.xml) - Extensible Markup Language
//! - YAML (.yaml, .yml) - YAML Ain't Markup Language
//! - TOML (.toml) - Tom's Obvious Minimal Language
//! - INI (.ini) - INI configuration files
//!
//! All loaders support backward compatibility through re-exports at the top level.

use async_trait::async_trait;
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::Result;

/// Loads CSV files as documents.
///
/// The `CSVLoader` reads CSV files and creates one document per row.
/// It supports custom delimiters, header configuration, and column selection.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::CSVLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = CSVLoader::new("data.csv");
/// let documents = loader.load().await?;
/// println!("Loaded {} documents", documents.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct CSVLoader {
    /// Path to the CSV file
    pub file_path: PathBuf,
    /// Column to use as the main content (default: None means all columns as JSON)
    pub content_column: Option<String>,
    /// Whether to include headers in the first row (default: true)
    pub has_headers: bool,
    /// CSV delimiter character (default: ',')
    pub delimiter: u8,
}

impl CSVLoader {
    /// Create a new `CSVLoader` for the given file path.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::CSVLoader;
    ///
    /// let loader = CSVLoader::new("data.csv");
    /// ```
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            content_column: None,
            has_headers: true,
            delimiter: b',',
        }
    }

    /// Set the column to use as the main content.
    ///
    /// If not set, all columns are included as JSON.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::CSVLoader;
    ///
    /// let loader = CSVLoader::new("data.csv")
    ///     .with_content_column("text");
    /// ```
    #[must_use]
    pub fn with_content_column(mut self, column: impl Into<String>) -> Self {
        self.content_column = Some(column.into());
        self
    }

    /// Set whether the CSV has headers.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::CSVLoader;
    ///
    /// let loader = CSVLoader::new("data.csv")
    ///     .with_headers(false);
    /// ```
    #[must_use]
    pub fn with_headers(mut self, has_headers: bool) -> Self {
        self.has_headers = has_headers;
        self
    }

    /// Set the CSV delimiter.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::CSVLoader;
    ///
    /// let loader = CSVLoader::new("data.tsv")
    ///     .with_delimiter(b'\t');
    /// ```
    #[must_use]
    pub fn with_delimiter(mut self, delimiter: u8) -> Self {
        self.delimiter = delimiter;
        self
    }
}

#[async_trait]
impl DocumentLoader for CSVLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let mut documents = Vec::new();

        // Read the CSV file
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(self.has_headers)
            .delimiter(self.delimiter)
            .from_path(&self.file_path)
            .map_err(|e| {
                crate::core::error::Error::InvalidInput(format!("Failed to read CSV file: {e}"))
            })?;

        // Get headers if present
        let headers: Option<Vec<String>> = if self.has_headers {
            Some(
                reader
                    .headers()
                    .map_err(|e| {
                        crate::core::error::Error::InvalidInput(format!(
                            "Failed to read CSV headers: {e}"
                        ))
                    })?
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect(),
            )
        } else {
            None
        };

        // Process each row
        for (row_num, result) in reader.records().enumerate() {
            let record = result.map_err(|e| {
                crate::core::error::Error::InvalidInput(format!("Failed to read CSV record: {e}"))
            })?;

            // Determine the content based on configuration
            let content = if let Some(ref col_name) = self.content_column {
                // Use specific column as content
                if let Some(ref headers) = headers {
                    if let Some(col_idx) = headers.iter().position(|h| h == col_name) {
                        record.get(col_idx).unwrap_or("").to_string()
                    } else {
                        return Err(crate::core::error::Error::InvalidInput(format!(
                            "Column '{col_name}' not found in CSV headers"
                        )));
                    }
                } else {
                    return Err(crate::core::error::Error::InvalidInput(
                        "Cannot use content_column without headers".to_string(),
                    ));
                }
            } else {
                // Use all columns as JSON
                let mut row_data = serde_json::Map::new();
                if let Some(ref headers) = headers {
                    for (idx, value) in record.iter().enumerate() {
                        if let Some(header) = headers.get(idx) {
                            row_data.insert(
                                header.clone(),
                                serde_json::Value::String(value.to_string()),
                            );
                        }
                    }
                } else {
                    // No headers, use indices
                    for (idx, value) in record.iter().enumerate() {
                        row_data.insert(
                            format!("column_{idx}"),
                            serde_json::Value::String(value.to_string()),
                        );
                    }
                }
                serde_json::to_string(&row_data)
                    .map_err(crate::core::error::Error::Serialization)?
            };

            // Create document with metadata
            let mut doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("row", row_num);

            // Add individual columns to metadata if headers are present
            if let Some(ref headers) = headers {
                for (idx, value) in record.iter().enumerate() {
                    if let Some(header) = headers.get(idx) {
                        doc = doc.with_metadata(header.clone(), value.to_string());
                    }
                }
            }

            documents.push(doc);
        }

        Ok(documents)
    }
}

/// Loads JSON files as documents.
///
/// The `JSONLoader` reads JSON files and creates documents from JSON data.
/// It supports loading JSON arrays (one document per array element) or
/// extracting data using JSON pointers.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::JSONLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Load entire JSON file as one document
/// let loader = JSONLoader::new("data.json");
/// let documents = loader.load().await?;
/// println!("Loaded {} documents", documents.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct JSONLoader {
    /// Path to the JSON file
    pub file_path: PathBuf,
    /// JSON pointer to extract specific data (e.g., "/data/items")
    pub json_pointer: Option<String>,
    /// Field to use as the main content
    pub content_key: Option<String>,
}

impl JSONLoader {
    /// Create a new `JSONLoader` for the given file path.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::JSONLoader;
    ///
    /// let loader = JSONLoader::new("data.json");
    /// ```
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            json_pointer: None,
            content_key: None,
        }
    }

    /// Set a JSON pointer to extract specific data from the file.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::JSONLoader;
    ///
    /// let loader = JSONLoader::new("data.json")
    ///     .with_json_pointer("/items");
    /// ```
    #[must_use]
    pub fn with_json_pointer(mut self, pointer: impl Into<String>) -> Self {
        self.json_pointer = Some(pointer.into());
        self
    }

    /// Set the field to use as the main content.
    ///
    /// If not set, the entire JSON object is serialized as the content.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow::core::document_loaders::JSONLoader;
    ///
    /// let loader = JSONLoader::new("data.json")
    ///     .with_content_key("text");
    /// ```
    #[must_use]
    pub fn with_content_key(mut self, key: impl Into<String>) -> Self {
        self.content_key = Some(key.into());
        self
    }

    fn create_document(&self, json_value: Value, index: usize) -> Result<Document> {
        let content = if let Some(ref content_key) = self.content_key {
            // Extract specific field as content
            json_value
                .get(content_key)
                .ok_or_else(|| {
                    crate::core::error::Error::InvalidInput(format!(
                        "Content key '{content_key}' not found"
                    ))
                })?
                .as_str()
                .ok_or_else(|| {
                    crate::core::error::Error::InvalidInput(format!(
                        "Content key '{content_key}' is not a string"
                    ))
                })?
                .to_string()
        } else {
            // Use entire JSON as content
            serde_json::to_string_pretty(&json_value)?
        };

        // Create document with metadata
        let mut doc = Document::new(content)
            .with_metadata("source", self.file_path.display().to_string())
            .with_metadata("index", index);

        // Add all JSON fields to metadata if it's an object
        if let Value::Object(ref map) = json_value {
            for (key, value) in map {
                if self.content_key.as_ref() != Some(key) {
                    let metadata_value = match value {
                        Value::String(s) => s.clone(),
                        other => serde_json::to_string(other).unwrap_or_default(),
                    };
                    doc = doc.with_metadata(key.clone(), metadata_value);
                }
            }
        }

        Ok(doc)
    }
}

#[async_trait]
impl DocumentLoader for JSONLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let loader = self.clone();

        // Use spawn_blocking to avoid blocking the async runtime with std::fs I/O
        tokio::task::spawn_blocking(move || {
            // Read the JSON file
            let content = std::fs::read_to_string(&loader.file_path).map_err(|e| {
                crate::core::error::Error::InvalidInput(format!(
                    "Failed to read JSON file {}: {}",
                    loader.file_path.display(),
                    e
                ))
            })?;

            // Parse JSON
            let mut json_data: Value = serde_json::from_str(&content).map_err(|e| {
                crate::core::error::Error::InvalidInput(format!(
                    "Failed to parse JSON from {}: {}",
                    loader.file_path.display(),
                    e
                ))
            })?;

            // Apply JSON pointer if specified
            if let Some(ref pointer) = loader.json_pointer {
                json_data = json_data
                    .pointer(pointer)
                    .ok_or_else(|| {
                        crate::core::error::Error::InvalidInput(format!(
                            "JSON pointer '{pointer}' not found"
                        ))
                    })?
                    .clone();
            }

            // Convert JSON to documents
            let documents = match json_data {
                Value::Array(items) => {
                    // Each array item becomes a document
                    items
                        .into_iter()
                        .enumerate()
                        .map(|(idx, item)| loader.create_document(item, idx))
                        .collect::<Result<Vec<_>>>()?
                }
                Value::Object(_) => {
                    // Single object becomes one document
                    vec![loader.create_document(json_data, 0)?]
                }
                _ => {
                    // Primitive value becomes one document
                    vec![loader.create_document(json_data, 0)?]
                }
            };

            Ok(documents)
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

/// Loads TSV (Tab-Separated Values) files as documents.
///
/// The `TSVLoader` reads TSV files and creates documents from tabular data.
/// It can create a single document for the entire file or separate documents per row.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::TSVLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = TSVLoader::new("data.tsv");
/// let documents = loader.load().await?;
/// println!("Loaded {} documents", documents.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct TSVLoader {
    /// Path to the TSV file
    pub file_path: PathBuf,
    /// Whether the first row contains column headers (default: true)
    pub has_headers: bool,
    /// Create separate documents per row (default: false)
    pub separate_rows: bool,
}

impl TSVLoader {
    /// Create a new `TSVLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            has_headers: true,
            separate_rows: false,
        }
    }

    /// Set whether the first row contains headers.
    #[must_use]
    pub fn with_headers(mut self, has_headers: bool) -> Self {
        self.has_headers = has_headers;
        self
    }

    /// Create separate documents per row.
    #[must_use]
    pub fn with_separate_rows(mut self, separate: bool) -> Self {
        self.separate_rows = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for TSVLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        let mut documents = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        if lines.is_empty() {
            return Ok(documents);
        }

        let headers: Option<Vec<String>> = if self.has_headers && !lines.is_empty() {
            Some(lines[0].split('\t').map(|s| s.trim().to_string()).collect())
        } else {
            None
        };

        let data_start = usize::from(self.has_headers);
        let data_lines = &lines[data_start..];

        if self.separate_rows {
            // Create separate document per row
            for (idx, line) in data_lines.iter().enumerate() {
                let values: Vec<&str> = line.split('\t').map(str::trim).collect();

                let content = if let Some(ref header_names) = headers {
                    // Create key-value pairs
                    header_names
                        .iter()
                        .zip(values.iter())
                        .map(|(k, v)| format!("{k}: {v}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    // Just join values with newlines
                    values.join("\n")
                };

                let doc = Document::new(content)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("row_index", idx as i64)
                    .with_metadata("format", "tsv");

                documents.push(doc);
            }
        } else {
            // Create single document with all rows
            let mut all_content = String::new();

            if let Some(ref header_names) = headers {
                all_content.push_str(&header_names.join("\t"));
                all_content.push('\n');
            }

            for line in data_lines {
                all_content.push_str(line);
                all_content.push('\n');
            }

            let doc = Document::new(all_content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "tsv")
                .with_metadata("row_count", data_lines.len() as i64);

            documents.push(doc);
        }

        Ok(documents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // =====================
    // CSVLoader Tests
    // =====================

    #[tokio::test]
    async fn test_csv_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "name,age,city").unwrap();
        writeln!(file, "Alice,30,NYC").unwrap();
        writeln!(file, "Bob,25,LA").unwrap();

        let loader = CSVLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert!(docs[0].page_content.contains("Alice"));
        assert!(docs[1].page_content.contains("Bob"));
    }

    #[tokio::test]
    async fn test_csv_loader_with_content_column() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "id,text,category").unwrap();
        writeln!(file, "1,Hello world,greeting").unwrap();
        writeln!(file, "2,Goodbye world,farewell").unwrap();

        let loader = CSVLoader::new(file.path()).with_content_column("text");
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].page_content, "Hello world");
        assert_eq!(docs[1].page_content, "Goodbye world");
    }

    #[tokio::test]
    async fn test_csv_loader_no_headers() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Alice,30,NYC").unwrap();
        writeln!(file, "Bob,25,LA").unwrap();

        let loader = CSVLoader::new(file.path()).with_headers(false);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        // Without headers, columns are named column_0, column_1, etc.
        assert!(docs[0].page_content.contains("column_0"));
    }

    #[tokio::test]
    async fn test_csv_loader_custom_delimiter() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "name;age;city").unwrap();
        writeln!(file, "Alice;30;NYC").unwrap();

        let loader = CSVLoader::new(file.path()).with_delimiter(b';');
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Alice"));
    }

    #[tokio::test]
    async fn test_csv_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "name,age").unwrap();
        writeln!(file, "Alice,30").unwrap();

        let loader = CSVLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].metadata.contains_key("source"));
        assert!(docs[0].metadata.contains_key("row"));
        assert!(docs[0].metadata.contains_key("name"));
        assert!(docs[0].metadata.contains_key("age"));
    }

    #[tokio::test]
    async fn test_csv_loader_missing_column_error() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "name,age").unwrap();
        writeln!(file, "Alice,30").unwrap();

        let loader = CSVLoader::new(file.path()).with_content_column("nonexistent");
        let result = loader.load().await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_csv_loader_content_column_without_headers_error() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Alice,30").unwrap();

        let loader = CSVLoader::new(file.path())
            .with_headers(false)
            .with_content_column("name");
        let result = loader.load().await;

        assert!(result.is_err());
    }

    // =====================
    // JSONLoader Tests
    // =====================

    #[tokio::test]
    async fn test_json_loader_single_object() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"name": "Alice", "age": 30}}"#).unwrap();

        let loader = JSONLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Alice"));
    }

    #[tokio::test]
    async fn test_json_loader_array() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"[{{"name": "Alice"}}, {{"name": "Bob"}}, {{"name": "Charlie"}}]"#
        )
        .unwrap();

        let loader = JSONLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 3);
    }

    #[tokio::test]
    async fn test_json_loader_with_content_key() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"text": "Hello world", "id": 1}}"#).unwrap();

        let loader = JSONLoader::new(file.path()).with_content_key("text");
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "Hello world");
    }

    #[tokio::test]
    async fn test_json_loader_with_json_pointer() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"{{"data": {{"items": [{{"text": "one"}}, {{"text": "two"}}]}}}}"#
        )
        .unwrap();

        let loader = JSONLoader::new(file.path()).with_json_pointer("/data/items");
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
    }

    #[tokio::test]
    async fn test_json_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"name": "Alice", "age": 30}}"#).unwrap();

        let loader = JSONLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert!(docs[0].metadata.contains_key("index"));
        assert!(docs[0].metadata.contains_key("name"));
        assert!(docs[0].metadata.contains_key("age"));
    }

    #[tokio::test]
    async fn test_json_loader_invalid_pointer_error() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"name": "Alice"}}"#).unwrap();

        let loader = JSONLoader::new(file.path()).with_json_pointer("/nonexistent/path");
        let result = loader.load().await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_json_loader_missing_content_key_error() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"name": "Alice"}}"#).unwrap();

        let loader = JSONLoader::new(file.path()).with_content_key("nonexistent");
        let result = loader.load().await;

        assert!(result.is_err());
    }

    // =====================
    // XMLLoader Tests
    // =====================

    #[tokio::test]
    async fn test_xml_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"<root><item>Hello</item></root>"#).unwrap();

        let loader = XMLLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("<root>"));
        assert!(docs[0].page_content.contains("<item>"));
    }

    #[tokio::test]
    async fn test_xml_loader_with_parse_structure() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"<root><item>Hello</item></root>"#).unwrap();

        let loader = XMLLoader::new(file.path()).with_parse_structure(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        // Parsed structure has indentation
        assert!(docs[0].page_content.contains("Hello"));
    }

    #[tokio::test]
    async fn test_xml_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"<root><item>Hello</item></root>"#).unwrap();

        let loader = XMLLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(
            docs[0].metadata.get("format").unwrap().as_str().unwrap(),
            "xml"
        );
    }

    #[tokio::test]
    async fn test_xml_loader_nested_structure() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"<root><level1><level2>Deep</level2></level1></root>"#
        )
        .unwrap();

        let loader = XMLLoader::new(file.path()).with_parse_structure(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Deep"));
    }

    // =====================
    // YAMLLoader Tests
    // =====================

    #[tokio::test]
    async fn test_yaml_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "name: Alice").unwrap();
        writeln!(file, "age: 30").unwrap();

        let loader = YAMLLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("name"));
        assert!(docs[0].page_content.contains("Alice"));
    }

    #[tokio::test]
    async fn test_yaml_loader_with_format_false() {
        let mut file = NamedTempFile::new().unwrap();
        let raw_content = "name: Alice\nage: 30\n";
        write!(file, "{}", raw_content).unwrap();

        let loader = YAMLLoader::new(file.path()).with_format(false);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        // Raw content preserved
        assert!(docs[0].page_content.contains("name: Alice"));
    }

    #[tokio::test]
    async fn test_yaml_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "key: value").unwrap();

        let loader = YAMLLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(
            docs[0].metadata.get("format").unwrap().as_str().unwrap(),
            "yaml"
        );
    }

    #[tokio::test]
    async fn test_yaml_loader_nested() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "root:").unwrap();
        writeln!(file, "  child: value").unwrap();

        let loader = YAMLLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("root"));
        assert!(docs[0].page_content.contains("child"));
    }

    // =====================
    // TOMLLoader Tests
    // =====================

    #[tokio::test]
    async fn test_toml_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "name = \"Alice\"").unwrap();
        writeln!(file, "age = 30").unwrap();

        let loader = TOMLLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("name"));
        assert!(docs[0].page_content.contains("Alice"));
    }

    #[tokio::test]
    async fn test_toml_loader_with_format_false() {
        let mut file = NamedTempFile::new().unwrap();
        let raw_content = "name = \"Alice\"\nage = 30\n";
        write!(file, "{}", raw_content).unwrap();

        let loader = TOMLLoader::new(file.path()).with_format(false);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        // Raw content preserved
        assert!(docs[0].page_content.contains("name = \"Alice\""));
    }

    #[tokio::test]
    async fn test_toml_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "key = \"value\"").unwrap();

        let loader = TOMLLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(
            docs[0].metadata.get("format").unwrap().as_str().unwrap(),
            "toml"
        );
    }

    #[tokio::test]
    async fn test_toml_loader_sections() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "[section]").unwrap();
        writeln!(file, "key = \"value\"").unwrap();

        let loader = TOMLLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("section"));
    }

    // =====================
    // IniLoader Tests
    // =====================

    #[tokio::test]
    async fn test_ini_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "[section]").unwrap();
        writeln!(file, "key = value").unwrap();

        let loader = IniLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("[section]"));
        assert!(docs[0].page_content.contains("key"));
    }

    #[tokio::test]
    async fn test_ini_loader_with_format_false() {
        let mut file = NamedTempFile::new().unwrap();
        let raw_content = "[section]\nkey = value\n";
        write!(file, "{}", raw_content).unwrap();

        let loader = IniLoader::new(file.path()).with_format(false);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content.trim(), raw_content.trim());
    }

    #[tokio::test]
    async fn test_ini_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "[section]").unwrap();
        writeln!(file, "key = value").unwrap();

        let loader = IniLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(
            docs[0].metadata.get("format").unwrap().as_str().unwrap(),
            "ini"
        );
    }

    #[tokio::test]
    async fn test_ini_loader_multiple_sections() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "[database]").unwrap();
        writeln!(file, "host = localhost").unwrap();
        writeln!(file, "[server]").unwrap();
        writeln!(file, "port = 8080").unwrap();

        let loader = IniLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("database"));
        assert!(docs[0].page_content.contains("server"));
    }

    // =====================
    // TSVLoader Tests
    // =====================

    #[tokio::test]
    async fn test_tsv_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "name\tage\tcity").unwrap();
        writeln!(file, "Alice\t30\tNYC").unwrap();
        writeln!(file, "Bob\t25\tLA").unwrap();

        let loader = TSVLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Alice"));
        assert!(docs[0].page_content.contains("Bob"));
    }

    #[tokio::test]
    async fn test_tsv_loader_separate_rows() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "name\tage").unwrap();
        writeln!(file, "Alice\t30").unwrap();
        writeln!(file, "Bob\t25").unwrap();

        let loader = TSVLoader::new(file.path()).with_separate_rows(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert!(docs[0].page_content.contains("Alice"));
        assert!(docs[1].page_content.contains("Bob"));
    }

    #[tokio::test]
    async fn test_tsv_loader_no_headers() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Alice\t30\tNYC").unwrap();
        writeln!(file, "Bob\t25\tLA").unwrap();

        let loader = TSVLoader::new(file.path()).with_headers(false);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Alice"));
    }

    #[tokio::test]
    async fn test_tsv_loader_separate_rows_with_headers() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "name\tage").unwrap();
        writeln!(file, "Alice\t30").unwrap();

        let loader = TSVLoader::new(file.path()).with_separate_rows(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        // Should have key-value format
        assert!(docs[0].page_content.contains("name: Alice"));
        assert!(docs[0].page_content.contains("age: 30"));
    }

    #[tokio::test]
    async fn test_tsv_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "name\tage").unwrap();
        writeln!(file, "Alice\t30").unwrap();

        let loader = TSVLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(
            docs[0].metadata.get("format").unwrap().as_str().unwrap(),
            "tsv"
        );
        assert!(docs[0].metadata.contains_key("row_count"));
    }

    #[tokio::test]
    async fn test_tsv_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = TSVLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 0);
    }

    // =====================
    // Builder Pattern Tests
    // =====================

    #[test]
    fn test_csv_loader_builder_chain() {
        let loader = CSVLoader::new("test.csv")
            .with_content_column("text")
            .with_headers(true)
            .with_delimiter(b';');

        assert_eq!(loader.content_column, Some("text".to_string()));
        assert!(loader.has_headers);
        assert_eq!(loader.delimiter, b';');
    }

    #[test]
    fn test_json_loader_builder_chain() {
        let loader = JSONLoader::new("test.json")
            .with_json_pointer("/data")
            .with_content_key("text");

        assert_eq!(loader.json_pointer, Some("/data".to_string()));
        assert_eq!(loader.content_key, Some("text".to_string()));
    }

    #[test]
    fn test_xml_loader_builder_chain() {
        let loader = XMLLoader::new("test.xml").with_parse_structure(true);

        assert!(loader.parse_structure);
    }

    #[test]
    fn test_yaml_loader_builder_chain() {
        let loader = YAMLLoader::new("test.yaml").with_format(false);

        assert!(!loader.format_yaml);
    }

    #[test]
    fn test_toml_loader_builder_chain() {
        let loader = TOMLLoader::new("test.toml").with_format(false);

        assert!(!loader.format_toml);
    }

    #[test]
    fn test_ini_loader_builder_chain() {
        let loader = IniLoader::new("test.ini").with_format(false);

        assert!(!loader.format_ini);
    }

    #[test]
    fn test_tsv_loader_builder_chain() {
        let loader = TSVLoader::new("test.tsv")
            .with_headers(false)
            .with_separate_rows(true);

        assert!(!loader.has_headers);
        assert!(loader.separate_rows);
    }
}
