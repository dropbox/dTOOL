// Allow clippy warnings for structured text loaders
// - needless_pass_by_value: json_value consumed by serialization in create_document
#![allow(clippy::needless_pass_by_value)]

//! Structured data format document loaders (CSV, TSV, JSON).
//!
//! This module provides loaders for structured text files with well-defined formats:
//! - CSV (Comma-Separated Values) with configurable delimiters
//! - TSV (Tab-Separated Values) with row-level or combined loading
//! - JSON with array/object support and JSON pointer extraction

use async_trait::async_trait;
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::Result;

/// Loads CSV files as documents.
///
/// The `CSVLoader` reads CSV files and creates one document per row. Each row's data
/// can be used as the full content (JSON format) or a specific column can be designated
/// as the main content. Column names and values are added to document metadata.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::CSVLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Load CSV with all columns as JSON content
/// let loader = CSVLoader::new("data.csv");
/// let documents = loader.load().await?;
///
/// // Load CSV with specific column as content
/// let loader = CSVLoader::new("data.csv")
///     .with_content_column("text");
/// let documents = loader.load().await?;
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

/// Loads TSV (Tab-Separated Values) files as documents.
///
/// The `TSVLoader` reads TSV files and can create either a single document containing
/// all rows or separate documents per row. Headers can be parsed and used to create
/// structured key-value pairs.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::TSVLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Load TSV as single document
/// let loader = TSVLoader::new("data.tsv");
/// let documents = loader.load().await?;
///
/// // Load TSV with separate documents per row
/// let loader = TSVLoader::new("data.tsv")
///     .with_separate_rows(true);
/// let documents = loader.load().await?;
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
    use crate::test_prelude::*;
    use tempfile::TempDir;

    // CSV Loader Tests
    #[tokio::test]
    async fn test_csv_loader_with_headers() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.csv");

        let csv_content = "name,age,city\nAlice,30,NYC\nBob,25,SF";
        std::fs::write(&file_path, csv_content).unwrap();

        let loader = CSVLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2, "Should load 2 rows from CSV");

        // First row should contain Alice's data
        let alice_doc = &docs[0];
        assert!(alice_doc.page_content.contains("Alice"));
        assert!(alice_doc.page_content.contains("30"));
        assert!(alice_doc.page_content.contains("NYC"));
        assert_eq!(
            alice_doc.get_metadata("name").and_then(|v| v.as_str()),
            Some("Alice")
        );
        assert_eq!(
            alice_doc.get_metadata("age").and_then(|v| v.as_str()),
            Some("30")
        );
        assert_eq!(
            alice_doc.get_metadata("city").and_then(|v| v.as_str()),
            Some("NYC")
        );
        assert_eq!(
            alice_doc.get_metadata("row").and_then(|v| v.as_u64()),
            Some(0),
            "First data row should have row index 0"
        );
        // Validate source path metadata
        assert!(
            alice_doc.get_metadata("source").is_some(),
            "Source metadata should be present"
        );

        // Second row should contain Bob's data
        let bob_doc = &docs[1];
        assert!(bob_doc.page_content.contains("Bob"));
        assert_eq!(
            bob_doc.get_metadata("name").and_then(|v| v.as_str()),
            Some("Bob")
        );
        assert_eq!(
            bob_doc.get_metadata("row").and_then(|v| v.as_u64()),
            Some(1),
            "Second data row should have row index 1"
        );
    }

    #[tokio::test]
    async fn test_csv_loader_with_content_column() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.csv");

        let csv_content = "id,text,category\n1,Hello world,greeting\n2,Goodbye,farewell";
        std::fs::write(&file_path, csv_content).unwrap();

        let loader = CSVLoader::new(&file_path).with_content_column("text");
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2, "Should load 2 rows from CSV");
        assert_eq!(
            docs[0].page_content, "Hello world",
            "First doc page_content should match content column value"
        );
        assert_eq!(
            docs[1].page_content, "Goodbye",
            "Second doc page_content should match content column value"
        );
        // Validate other columns are in metadata
        assert_eq!(
            docs[0].get_metadata("category").and_then(|v| v.as_str()),
            Some("greeting"),
            "Non-content columns should be in metadata"
        );
        assert_eq!(
            docs[0].get_metadata("id").and_then(|v| v.as_str()),
            Some("1"),
            "ID column should be in metadata"
        );
        assert_eq!(
            docs[1].get_metadata("category").and_then(|v| v.as_str()),
            Some("farewell")
        );
        // Validate source metadata
        assert!(
            docs[0].get_metadata("source").is_some(),
            "Source metadata should be present"
        );
    }

    #[tokio::test]
    async fn test_csv_loader_without_headers() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.csv");

        let csv_content = "Alice,30,NYC\nBob,25,SF";
        std::fs::write(&file_path, csv_content).unwrap();

        let loader = CSVLoader::new(&file_path).with_headers(false);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2, "Should load 2 rows without headers");
        assert!(
            docs[0].page_content.contains("Alice"),
            "Page content should contain first field value"
        );
        assert!(
            docs[0].page_content.contains("column_0"),
            "Auto-generated column names should be present in page_content"
        );
        assert!(
            docs[0].page_content.contains("column_1"),
            "All columns should have generated names in page_content"
        );
        assert!(
            docs[0].page_content.contains("column_2"),
            "Third column should have generated name in page_content"
        );
        // Validate structure - without headers, columns are only in page_content as JSON, not in metadata
        assert!(
            docs[0].page_content.contains("\"column_0\":\"Alice\"")
                || docs[0].page_content.contains("\"column_0\": \"Alice\""),
            "Page content should be JSON with column_0"
        );
        // Validate basic metadata
        assert_eq!(
            docs[0].get_metadata("row").and_then(|v| v.as_u64()),
            Some(0),
            "Row index should be in metadata"
        );
        assert!(
            docs[0].get_metadata("source").is_some(),
            "Source should be in metadata"
        );
    }

    #[tokio::test]
    async fn test_csv_loader_with_tsv() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.tsv");

        let tsv_content = "name\tage\nAlice\t30\nBob\t25";
        std::fs::write(&file_path, tsv_content).unwrap();

        let loader = CSVLoader::new(&file_path).with_delimiter(b'\t');
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2, "Should load 2 rows from TSV");
        assert!(
            docs[0].page_content.contains("Alice"),
            "Page content should contain field values"
        );
        assert_eq!(
            docs[0].get_metadata("name").and_then(|v| v.as_str()),
            Some("Alice"),
            "Tab-delimited fields should be parsed correctly"
        );
        assert_eq!(
            docs[0].get_metadata("age").and_then(|v| v.as_str()),
            Some("30"),
            "Second tab-delimited field should be parsed"
        );
        assert_eq!(
            docs[1].get_metadata("name").and_then(|v| v.as_str()),
            Some("Bob"),
            "Second row should be parsed correctly"
        );
        // Validate source metadata
        assert!(
            docs[0].get_metadata("source").is_some(),
            "Source metadata should be present"
        );
    }

    #[tokio::test]
    async fn test_csv_loader_empty_csv() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.csv");
        std::fs::write(&file_path, "").unwrap();

        let loader = CSVLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 0, "Empty CSV should return zero documents");
    }

    #[tokio::test]
    async fn test_csv_loader_quote_escaping() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("quoted.csv");

        // CSV with quoted fields containing commas
        let csv_content = r#"name,address,notes
"Smith, John","123 Main St, Apt 4B","Has a comma, see?"
"Doe, Jane","456 Oak Ave","Simple note"#;
        std::fs::write(&file_path, csv_content).unwrap();

        let loader = CSVLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2, "Should parse 2 rows with quoted fields");

        // First row: verify commas inside quotes are preserved
        assert_eq!(
            docs[0].get_metadata("name").and_then(|v| v.as_str()),
            Some("Smith, John"),
            "Comma inside quotes should be preserved"
        );
        assert_eq!(
            docs[0].get_metadata("address").and_then(|v| v.as_str()),
            Some("123 Main St, Apt 4B"),
            "Multiple commas in quoted field should be preserved"
        );
        assert_eq!(
            docs[0].get_metadata("notes").and_then(|v| v.as_str()),
            Some("Has a comma, see?"),
            "Commas in notes field should be preserved"
        );

        // Second row: simple case
        assert_eq!(
            docs[1].get_metadata("name").and_then(|v| v.as_str()),
            Some("Doe, Jane")
        );
    }

    #[tokio::test]
    async fn test_csv_loader_multiline_fields() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("multiline.csv");

        // CSV with fields containing embedded newlines
        let csv_content = "name,bio\nAlice,\"Line 1\nLine 2\nLine 3\"\nBob,\"Single line\"";
        std::fs::write(&file_path, csv_content).unwrap();

        let loader = CSVLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2, "Should parse rows with multiline fields");

        // First row: verify newlines in bio field are preserved
        let alice_bio = docs[0].get_metadata("bio").and_then(|v| v.as_str());
        assert_eq!(
            alice_bio,
            Some("Line 1\nLine 2\nLine 3"),
            "Newlines inside quoted field should be preserved"
        );
        assert!(
            alice_bio.unwrap().contains('\n'),
            "Bio should contain actual newline characters"
        );

        // Second row: simple case
        assert_eq!(
            docs[1].get_metadata("bio").and_then(|v| v.as_str()),
            Some("Single line")
        );
    }

    #[tokio::test]
    async fn test_csv_loader_unicode_content() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("unicode.csv");

        let csv_content = "name,message\nAlice,Hello 世界 🌍\nBob,مرحبا בעולם\nCarol,Привет ∑∫√";
        std::fs::write(&file_path, csv_content).unwrap();

        let loader = CSVLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 3, "Should load 3 rows with Unicode");

        // Verify Unicode is preserved correctly
        assert_eq!(
            docs[0].get_metadata("message").and_then(|v| v.as_str()),
            Some("Hello 世界 🌍"),
            "Chinese characters and emoji should be preserved"
        );
        assert_eq!(
            docs[1].get_metadata("message").and_then(|v| v.as_str()),
            Some("مرحبا בעולם"),
            "Arabic and Hebrew should be preserved"
        );
        assert_eq!(
            docs[2].get_metadata("message").and_then(|v| v.as_str()),
            Some("Привет ∑∫√"),
            "Cyrillic and math symbols should be preserved"
        );
    }

    #[tokio::test]
    async fn test_csv_loader_large_csv() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large.csv");

        // Create a CSV with 10,000 rows
        let mut csv_content = String::from("id,value\n");
        for i in 0..10000 {
            csv_content.push_str(&format!("{},value_{}\n", i, i));
        }
        std::fs::write(&file_path, csv_content).unwrap();

        let loader = CSVLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 10000, "Should load 10K rows");

        // Spot check first and last rows
        assert_eq!(
            docs[0].get_metadata("id").and_then(|v| v.as_str()),
            Some("0"),
            "First row should have id=0"
        );
        assert_eq!(
            docs[0].get_metadata("value").and_then(|v| v.as_str()),
            Some("value_0")
        );
        assert_eq!(
            docs[9999].get_metadata("id").and_then(|v| v.as_str()),
            Some("9999"),
            "Last row should have id=9999"
        );
        assert_eq!(
            docs[9999].get_metadata("value").and_then(|v| v.as_str()),
            Some("value_9999")
        );
    }

    #[tokio::test]
    async fn test_csv_loader_no_newline_at_end() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("no_newline.csv");

        // CSV without trailing newline
        let csv_content = "name,age\nAlice,30\nBob,25";
        std::fs::write(&file_path, csv_content).unwrap();

        let loader = CSVLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2, "Should parse CSV without trailing newline");
        assert_eq!(
            docs[0].get_metadata("name").and_then(|v| v.as_str()),
            Some("Alice")
        );
        assert_eq!(
            docs[1].get_metadata("name").and_then(|v| v.as_str()),
            Some("Bob"),
            "Last row should be parsed even without trailing newline"
        );
    }

    #[tokio::test]
    async fn test_csv_loader_only_headers() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("headers_only.csv");

        // CSV with only header row
        let csv_content = "name,age,city\n";
        std::fs::write(&file_path, csv_content).unwrap();

        let loader = CSVLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(
            docs.len(),
            0,
            "CSV with only headers should return zero documents"
        );
    }

    #[tokio::test]
    async fn test_csv_loader_empty_fields() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty_fields.csv");

        // CSV with empty fields
        let csv_content = "name,middle,last\nAlice,,Smith\n,Bob,Jones\nCarol,Maria,";
        std::fs::write(&file_path, csv_content).unwrap();

        let loader = CSVLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 3, "Should parse rows with empty fields");

        // First row: empty middle name
        assert_eq!(
            docs[0].get_metadata("name").and_then(|v| v.as_str()),
            Some("Alice")
        );
        assert_eq!(
            docs[0].get_metadata("middle").and_then(|v| v.as_str()),
            Some(""),
            "Empty field should be empty string"
        );
        assert_eq!(
            docs[0].get_metadata("last").and_then(|v| v.as_str()),
            Some("Smith")
        );

        // Second row: empty first name
        assert_eq!(
            docs[1].get_metadata("name").and_then(|v| v.as_str()),
            Some(""),
            "Empty first field should be empty string"
        );
        assert_eq!(
            docs[1].get_metadata("middle").and_then(|v| v.as_str()),
            Some("Bob")
        );

        // Third row: empty last name
        assert_eq!(
            docs[2].get_metadata("last").and_then(|v| v.as_str()),
            Some(""),
            "Empty trailing field should be empty string"
        );
    }

    // JSON Loader Tests
    #[tokio::test]
    async fn test_json_loader_array() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.json");

        let json_content = r#"[
            {"text": "First document", "category": "A"},
            {"text": "Second document", "category": "B"}
        ]"#;
        std::fs::write(&file_path, json_content).unwrap();

        let loader = JSONLoader::new(&file_path).with_content_key("text");
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2, "Should load 2 documents from array");

        // Validate first document
        assert_eq!(docs[0].page_content, "First document");
        assert_eq!(
            docs[0].get_metadata("category").and_then(|v| v.as_str()),
            Some("A"),
            "First doc should have category A"
        );
        assert_eq!(
            docs[0].get_metadata("index").and_then(|v| v.as_u64()),
            Some(0),
            "First doc should have index 0"
        );
        assert!(
            docs[0].get_metadata("source").is_some(),
            "First doc should have source metadata"
        );

        // Validate second document
        assert_eq!(docs[1].page_content, "Second document");
        assert_eq!(
            docs[1].get_metadata("category").and_then(|v| v.as_str()),
            Some("B"),
            "Second doc should have category B"
        );
        assert_eq!(
            docs[1].get_metadata("index").and_then(|v| v.as_u64()),
            Some(1),
            "Second doc should have index 1"
        );
    }

    #[tokio::test]
    async fn test_json_loader_object() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.json");

        let json_content = r#"{"title": "Test Document", "content": "This is content"}"#;
        std::fs::write(&file_path, json_content).unwrap();

        let loader = JSONLoader::new(&file_path).with_content_key("content");
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Single object should create one document");
        assert_eq!(docs[0].page_content, "This is content");
        assert_eq!(
            docs[0].get_metadata("title").and_then(|v| v.as_str()),
            Some("Test Document"),
            "Object fields should be in metadata"
        );
        assert_eq!(
            docs[0].get_metadata("index").and_then(|v| v.as_u64()),
            Some(0),
            "Document should have index 0"
        );
        assert!(
            docs[0].get_metadata("source").is_some(),
            "Document should have source metadata"
        );
        assert!(
            docs[0].get_metadata("content").is_none(),
            "Content key should not be duplicated in metadata"
        );
    }

    #[tokio::test]
    async fn test_json_loader_with_pointer() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.json");

        let json_content = r#"{"data": {"items": [
            {"text": "Item 1"},
            {"text": "Item 2"}
        ]}}"#;
        std::fs::write(&file_path, json_content).unwrap();

        let loader = JSONLoader::new(&file_path)
            .with_json_pointer("/data/items")
            .with_content_key("text");
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2, "JSON pointer should extract nested array");
        assert_eq!(docs[0].page_content, "Item 1");
        assert_eq!(docs[1].page_content, "Item 2");
        assert_eq!(
            docs[0].get_metadata("index").and_then(|v| v.as_u64()),
            Some(0),
            "First item should have index 0"
        );
        assert_eq!(
            docs[1].get_metadata("index").and_then(|v| v.as_u64()),
            Some(1),
            "Second item should have index 1"
        );
        assert!(
            docs[0].get_metadata("source").is_some(),
            "Documents should have source metadata"
        );
    }

    #[tokio::test]
    async fn test_json_loader_without_content_key() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.json");

        let json_content = r#"[{"id": 1, "value": "test"}]"#;
        std::fs::write(&file_path, json_content).unwrap();

        let loader = JSONLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(
            docs.len(),
            1,
            "Array with one element should create one document"
        );
        // Content should be the entire JSON object as pretty-printed string
        assert!(
            docs[0].page_content.contains("\"id\""),
            "Content should include id field"
        );
        assert!(
            docs[0].page_content.contains("\"value\""),
            "Content should include value field"
        );
        assert!(
            docs[0].page_content.contains("1") && docs[0].page_content.contains("test"),
            "Content should include field values"
        );
        assert_eq!(
            docs[0].get_metadata("id"),
            Some(&serde_json::Value::String("1".to_string())),
            "id should be in metadata as string"
        );
        assert_eq!(
            docs[0].get_metadata("value").and_then(|v| v.as_str()),
            Some("test"),
            "value should be in metadata"
        );
    }

    #[tokio::test]
    async fn test_json_loader_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.json");

        // Empty file should error (not valid JSON)
        std::fs::write(&file_path, "").unwrap();

        let loader = JSONLoader::new(&file_path);
        let result = loader.load().await;

        assert!(result.is_err(), "Empty file should error (not valid JSON)");
    }

    #[tokio::test]
    async fn test_json_loader_malformed_json() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("malformed.json");

        // Test various malformed JSON syntax errors
        let malformed_cases = vec![
            (r#"{"unclosed": "brace""#, "unclosed brace"),
            (r#"{"missing": "comma" "next": "field"}"#, "missing comma"),
            (r#"{"trailing": "comma",}"#, "trailing comma"),
            (r#"[1, 2, 3,]"#, "trailing comma in array"),
        ];

        for (content, description) in malformed_cases {
            std::fs::write(&file_path, content).unwrap();
            let loader = JSONLoader::new(&file_path);
            let result = loader.load().await;
            assert!(
                result.is_err(),
                "Malformed JSON ({}) should error",
                description
            );
        }
    }

    #[tokio::test]
    async fn test_json_loader_deeply_nested() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("nested.json");

        // Create deeply nested JSON (10 levels) - properly balanced braces
        let json = r#"{
            "level1": {
                "level2": {
                    "level3": {
                        "level4": {
                            "level5": {
                                "level6": {
                                    "level7": {
                                        "level8": {
                                            "level9": {
                                                "level10": {
                                                    "text": "Deep content"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }"#;
        std::fs::write(&file_path, json).unwrap();

        let loader = JSONLoader::new(&file_path)
            .with_json_pointer(
                "/level1/level2/level3/level4/level5/level6/level7/level8/level9/level10",
            )
            .with_content_key("text");
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should extract deeply nested object");
        assert_eq!(docs[0].page_content, "Deep content");
    }

    #[tokio::test]
    async fn test_json_loader_large_array() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large.json");

        // Create large JSON array (10K elements)
        let mut json = String::from("[");
        for i in 0..10000 {
            if i > 0 {
                json.push(',');
            }
            json.push_str(&format!(
                r#"{{"id": {}, "text": "Item {}", "value": {}}}"#,
                i,
                i,
                i * 2
            ));
        }
        json.push(']');
        std::fs::write(&file_path, json).unwrap();

        let loader = JSONLoader::new(&file_path).with_content_key("text");
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 10000, "Should load all 10K documents");

        // Spot-check first and last elements
        assert_eq!(docs[0].page_content, "Item 0");
        assert_eq!(
            docs[0].get_metadata("id"),
            Some(&serde_json::Value::String("0".to_string()))
        );
        assert_eq!(docs[9999].page_content, "Item 9999");
        assert_eq!(
            docs[9999].get_metadata("id"),
            Some(&serde_json::Value::String("9999".to_string()))
        );
    }

    #[tokio::test]
    async fn test_json_loader_mixed_types() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("mixed.json");

        let json_content = r#"[
            {"text": "String value", "type": "string"},
            {"text": "Number value", "number": 42},
            {"text": "Bool value", "flag": true},
            {"text": "Null value", "empty": null},
            {"text": "Nested value", "nested": {"key": "value"}}
        ]"#;
        std::fs::write(&file_path, json_content).unwrap();

        let loader = JSONLoader::new(&file_path).with_content_key("text");
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 5, "Should handle array with mixed value types");
        assert_eq!(docs[0].page_content, "String value");
        assert_eq!(docs[1].page_content, "Number value");
        assert_eq!(docs[2].page_content, "Bool value");
        assert_eq!(docs[3].page_content, "Null value");
        assert_eq!(docs[4].page_content, "Nested value");

        // Validate metadata preserves type information
        assert_eq!(
            docs[0].get_metadata("type").and_then(|v| v.as_str()),
            Some("string")
        );
        assert_eq!(
            docs[1].get_metadata("number"),
            Some(&serde_json::Value::String("42".to_string())),
            "Number should be serialized to metadata"
        );
        assert_eq!(
            docs[2].get_metadata("flag"),
            Some(&serde_json::Value::String("true".to_string())),
            "Bool should be serialized to metadata"
        );
        assert_eq!(
            docs[3].get_metadata("empty"),
            Some(&serde_json::Value::String("null".to_string())),
            "Null should be serialized to metadata"
        );
    }

    #[tokio::test]
    async fn test_json_loader_unicode_content() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("unicode.json");

        let json_content = r#"[
            {"text": "Hello 世界 🌍", "lang": "mixed"},
            {"text": "مرحبا בעולם", "lang": "rtl"},
            {"text": "Привет ∑∫√", "lang": "math"}
        ]"#;
        std::fs::write(&file_path, json_content).unwrap();

        let loader = JSONLoader::new(&file_path).with_content_key("text");
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 3, "Should handle Unicode content");
        assert_eq!(docs[0].page_content, "Hello 世界 🌍");
        assert_eq!(docs[1].page_content, "مرحبا בעולם");
        assert_eq!(docs[2].page_content, "Привет ∑∫√");
    }

    #[tokio::test]
    async fn test_json_loader_empty_array() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty_array.json");

        std::fs::write(&file_path, "[]").unwrap();

        let loader = JSONLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 0, "Empty array should produce no documents");
    }

    #[tokio::test]
    async fn test_json_loader_empty_object() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty_object.json");

        std::fs::write(&file_path, "{}").unwrap();

        let loader = JSONLoader::new(&file_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Empty object should create one document");
        assert_eq!(docs[0].page_content, "{}");
        assert_eq!(
            docs[0].get_metadata("index").and_then(|v| v.as_u64()),
            Some(0)
        );
    }

    #[tokio::test]
    async fn test_json_loader_null_values() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("nulls.json");

        let json_content = r#"[
            {"text": "With null", "nullable": null, "other": "value"}
        ]"#;
        std::fs::write(&file_path, json_content).unwrap();

        let loader = JSONLoader::new(&file_path).with_content_key("text");
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Should handle null values in fields");
        assert_eq!(docs[0].page_content, "With null");
        assert_eq!(
            docs[0].get_metadata("nullable"),
            Some(&serde_json::Value::String("null".to_string())),
            "Null value should be in metadata"
        );
        assert_eq!(
            docs[0].get_metadata("other").and_then(|v| v.as_str()),
            Some("value")
        );
    }

    #[tokio::test]
    async fn test_json_loader_escaped_characters() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("escaped.json");

        let json_content = r#"[
            {"text": "Line 1\nLine 2\nLine 3", "type": "newlines"},
            {"text": "Tab\there\tand\there", "type": "tabs"},
            {"text": "Quote: \"Hello\"", "type": "quotes"},
            {"text": "Backslash: \\path\\to\\file", "type": "backslash"}
        ]"#;
        std::fs::write(&file_path, json_content).unwrap();

        let loader = JSONLoader::new(&file_path).with_content_key("text");
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 4, "Should handle escaped characters");

        // JSON parser should convert escape sequences to actual characters
        assert_eq!(docs[0].page_content, "Line 1\nLine 2\nLine 3");
        assert!(
            docs[0].page_content.contains('\n'),
            "Newlines should be actual newline chars"
        );

        assert_eq!(docs[1].page_content, "Tab\there\tand\there");
        assert!(
            docs[1].page_content.contains('\t'),
            "Tabs should be actual tab chars"
        );

        assert_eq!(docs[2].page_content, "Quote: \"Hello\"");
        assert!(
            docs[2].page_content.contains('"'),
            "Quotes should be actual quote chars"
        );

        assert_eq!(docs[3].page_content, "Backslash: \\path\\to\\file");
        assert!(
            docs[3].page_content.contains('\\'),
            "Backslashes should be actual backslash chars"
        );
    }

    #[tokio::test]
    async fn test_json_loader_invalid_pointer() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.json");

        let json_content = r#"{"data": {"items": [1, 2, 3]}}"#;
        std::fs::write(&file_path, json_content).unwrap();

        // Test pointer to non-existent path
        let loader = JSONLoader::new(&file_path).with_json_pointer("/nonexistent/path");
        let result = loader.load().await;

        assert!(result.is_err(), "Invalid JSON pointer should error");

        // Verify error message mentions pointer
        let err = result.unwrap_err();
        let err_msg = format!("{}", err);
        assert!(
            err_msg.contains("pointer") || err_msg.contains("not found"),
            "Error message should mention pointer issue: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_json_loader_missing_content_key() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.json");

        let json_content = r#"[{"id": 1, "value": "test"}]"#;
        std::fs::write(&file_path, json_content).unwrap();

        // Request content_key that doesn't exist
        let loader = JSONLoader::new(&file_path).with_content_key("nonexistent");
        let result = loader.load().await;

        assert!(result.is_err(), "Missing content key should error");

        let err = result.unwrap_err();
        let err_msg = format!("{}", err);
        assert!(
            err_msg.contains("nonexistent") || err_msg.contains("not found"),
            "Error message should mention missing key: {}",
            err_msg
        );
    }

    // TSV Loader Tests
    #[tokio::test]
    async fn test_tsv_loader() {
        let temp_dir = TempDir::new().unwrap();
        let tsv_path = temp_dir.path().join("test.tsv");

        let tsv_content = "Name\tAge\tCity\nAlice\t30\tNew York\nBob\t25\tSan Francisco\n";

        std::fs::write(&tsv_path, tsv_content).unwrap();

        let loader = TSVLoader::new(&tsv_path);
        let docs = loader.load().await.unwrap();

        // Validate document count (single document with all rows in default mode)
        assert_eq!(
            docs.len(),
            1,
            "TSVLoader should create exactly 1 document in default mode (all rows combined)"
        );

        // Validate content includes header row (tab-separated)
        assert!(
            docs[0].page_content.contains("Name\tAge\tCity"),
            "Content should include header row with tab separators"
        );

        // Validate content includes both data rows (tab-separated)
        assert!(
            docs[0].page_content.contains("Alice\t30\tNew York"),
            "Content should include first data row with tab separators"
        );
        assert!(
            docs[0].page_content.contains("Bob\t25\tSan Francisco"),
            "Content should include second data row with tab separators"
        );

        // Validate format metadata
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("tsv"),
            "Document should have format='tsv' metadata"
        );

        // Validate row_count metadata (2 data rows, header not counted)
        assert_eq!(
            docs[0].get_metadata("row_count").and_then(|v| v.as_i64()),
            Some(2),
            "Document should have row_count=2 metadata (2 data rows, header not counted)"
        );

        // Validate source metadata contains file path
        let source = docs[0]
            .get_metadata("source")
            .and_then(|v| v.as_str())
            .expect("Document should have source metadata");
        assert!(
            source.contains("test.tsv"),
            "Source metadata should contain filename 'test.tsv', got: {}",
            source
        );
    }

    #[tokio::test]
    async fn test_tsv_loader_separate_rows() {
        let temp_dir = TempDir::new().unwrap();
        let tsv_path = temp_dir.path().join("test.tsv");

        let tsv_content = "Product\tPrice\nLaptop\t999\nMouse\t29\n";

        std::fs::write(&tsv_path, tsv_content).unwrap();

        let loader = TSVLoader::new(&tsv_path).with_separate_rows(true);
        let docs = loader.load().await.unwrap();

        // Validate document count (separate mode creates one doc per data row)
        assert_eq!(
            docs.len(),
            2,
            "TSVLoader with separate_rows=true should create 2 documents for 2 data rows"
        );

        // Validate first document content (key-value pairs from header and values)
        assert!(
            docs[0].page_content.contains("Product: Laptop"),
            "First document should contain 'Product: Laptop' key-value pair"
        );
        assert!(
            docs[0].page_content.contains("Price: 999"),
            "First document should contain 'Price: 999' key-value pair"
        );

        // Validate second document content (key-value pairs)
        assert!(
            docs[1].page_content.contains("Product: Mouse"),
            "Second document should contain 'Product: Mouse' key-value pair"
        );
        assert!(
            docs[1].page_content.contains("Price: 29"),
            "Second document should contain 'Price: 29' key-value pair"
        );

        // Validate row_index metadata for first document (0-based)
        assert_eq!(
            docs[0].get_metadata("row_index").and_then(|v| v.as_i64()),
            Some(0),
            "First document should have row_index=0 metadata"
        );

        // Validate row_index metadata for second document
        assert_eq!(
            docs[1].get_metadata("row_index").and_then(|v| v.as_i64()),
            Some(1),
            "Second document should have row_index=1 metadata (0-based indexing)"
        );

        // Validate format metadata for both documents
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("tsv"),
            "First document should have format='tsv' metadata"
        );
        assert_eq!(
            docs[1].get_metadata("format").and_then(|v| v.as_str()),
            Some("tsv"),
            "Second document should have format='tsv' metadata"
        );

        // Validate source metadata contains file path for both documents
        for (idx, doc) in docs.iter().enumerate() {
            let source = doc
                .get_metadata("source")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            assert!(
                source.contains("test.tsv"),
                "Document {} should have source metadata containing 'test.tsv', got: {}",
                idx,
                source
            );
        }

        // Validate NO row_count metadata in separate mode (only in combined mode)
        assert!(
            docs[0].get_metadata("row_count").is_none(),
            "Separate row mode should NOT have 'row_count' metadata (only combined mode has it)"
        );
    }

    #[tokio::test]
    async fn test_tsv_loader_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let tsv_path = temp_dir.path().join("empty.tsv");

        // Empty file (no rows, no headers)
        std::fs::write(&tsv_path, "").unwrap();

        let loader = TSVLoader::new(&tsv_path);
        let docs = loader.load().await.unwrap();

        // Empty file returns empty document list (no documents created)
        assert_eq!(
            docs.len(),
            0,
            "Empty TSV file should return empty document list (no documents created)"
        );
    }

    #[tokio::test]
    async fn test_tsv_loader_header_only() {
        let temp_dir = TempDir::new().unwrap();
        let tsv_path = temp_dir.path().join("header_only.tsv");

        // File with header row but no data rows
        let tsv_content = "Column1\tColumn2\tColumn3\n";

        std::fs::write(&tsv_path, tsv_content).unwrap();

        let loader = TSVLoader::new(&tsv_path);
        let docs = loader.load().await.unwrap();

        // Header-only file creates single document with header and no data rows
        assert_eq!(
            docs.len(),
            1,
            "Header-only TSV file should create 1 document"
        );
        assert_eq!(
            docs[0].get_metadata("row_count").and_then(|v| v.as_i64()),
            Some(0),
            "Header-only file should have row_count=0 (no data rows)"
        );
        assert!(
            docs[0].page_content.contains("Column1\tColumn2\tColumn3"),
            "Content should include header row"
        );
    }

    #[tokio::test]
    async fn test_tsv_loader_single_column() {
        let temp_dir = TempDir::new().unwrap();
        let tsv_path = temp_dir.path().join("single_column.tsv");

        // TSV with single column (no tabs)
        let tsv_content = "Item\nApple\nBanana\nCherry\n";

        std::fs::write(&tsv_path, tsv_content).unwrap();

        let loader = TSVLoader::new(&tsv_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "Single-column TSV should create 1 document");
        assert_eq!(
            docs[0].get_metadata("row_count").and_then(|v| v.as_i64()),
            Some(3),
            "Single-column TSV with 3 data rows should have row_count=3"
        );
        assert!(
            docs[0].page_content.contains("Apple"),
            "Content should contain first data item 'Apple'"
        );
        assert!(
            docs[0].page_content.contains("Cherry"),
            "Content should contain last data item 'Cherry'"
        );
    }

    #[tokio::test]
    async fn test_tsv_loader_many_columns() {
        let temp_dir = TempDir::new().unwrap();
        let tsv_path = temp_dir.path().join("many_columns.tsv");

        // TSV with 20 columns
        let mut header = Vec::new();
        let mut row = Vec::new();
        for i in 1..=20 {
            header.push(format!("Col{}", i));
            row.push(format!("Val{}", i));
        }
        let tsv_content = format!("{}\n{}\n", header.join("\t"), row.join("\t"));

        std::fs::write(&tsv_path, tsv_content).unwrap();

        let loader = TSVLoader::new(&tsv_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(
            docs.len(),
            1,
            "TSV with 20 columns should create 1 document"
        );
        assert_eq!(
            docs[0].get_metadata("row_count").and_then(|v| v.as_i64()),
            Some(1),
            "TSV with 1 data row should have row_count=1"
        );

        // Validate first and last column headers and values present
        assert!(
            docs[0].page_content.contains("Col1"),
            "Content should contain first column header 'Col1'"
        );
        assert!(
            docs[0].page_content.contains("Col20"),
            "Content should contain last column header 'Col20'"
        );
        assert!(
            docs[0].page_content.contains("Val1"),
            "Content should contain first column value 'Val1'"
        );
        assert!(
            docs[0].page_content.contains("Val20"),
            "Content should contain last column value 'Val20'"
        );
    }

    #[tokio::test]
    async fn test_tsv_loader_many_rows() {
        let temp_dir = TempDir::new().unwrap();
        let tsv_path = temp_dir.path().join("many_rows.tsv");

        // Generate TSV with 1000 rows
        let mut tsv_content = String::from("ID\tValue\n");
        for i in 0..1000 {
            tsv_content.push_str(&format!("{}\tRow{}\n", i, i));
        }

        std::fs::write(&tsv_path, tsv_content).unwrap();

        let loader = TSVLoader::new(&tsv_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1, "TSV with 1000 rows should create 1 document");
        assert_eq!(
            docs[0].get_metadata("row_count").and_then(|v| v.as_i64()),
            Some(1000),
            "TSV with 1000 data rows should have row_count=1000"
        );

        // Validate first and last rows present
        assert!(
            docs[0].page_content.contains("0\tRow0"),
            "Content should contain first row '0\tRow0'"
        );
        assert!(
            docs[0].page_content.contains("999\tRow999"),
            "Content should contain last row '999\tRow999'"
        );
    }

    #[tokio::test]
    async fn test_tsv_loader_empty_fields() {
        let temp_dir = TempDir::new().unwrap();
        let tsv_path = temp_dir.path().join("empty_fields.tsv");

        // TSV with empty fields (consecutive tabs)
        let tsv_content = "Name\tAge\tCity\nAlice\t\tBoston\n\t25\t\nBob\t30\tSeattle\n";

        std::fs::write(&tsv_path, tsv_content).unwrap();

        let loader = TSVLoader::new(&tsv_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(
            docs.len(),
            1,
            "TSV with empty fields should create 1 document"
        );
        assert_eq!(
            docs[0].get_metadata("row_count").and_then(|v| v.as_i64()),
            Some(3),
            "TSV with 3 data rows should have row_count=3"
        );

        // Validate that rows with empty fields are still parsed
        // Row 1: "Alice\t\tBoston" (empty Age field)
        assert!(
            docs[0].page_content.contains("Alice\t\tBoston"),
            "Content should contain row with empty Age field: 'Alice\\t\\tBoston'"
        );

        // Row 2: "\t25\t" (empty Name and City fields)
        // Row 3: "Bob\t30\tSeattle" (all fields present)
        assert!(
            docs[0].page_content.contains("Bob\t30\tSeattle"),
            "Content should contain complete row: 'Bob\\t30\\tSeattle'"
        );
    }

    #[tokio::test]
    async fn test_tsv_loader_no_headers() {
        let temp_dir = TempDir::new().unwrap();
        let tsv_path = temp_dir.path().join("no_headers.tsv");

        // TSV without header row (only data)
        let tsv_content = "100\tItem1\n200\tItem2\n300\tItem3\n";

        std::fs::write(&tsv_path, tsv_content).unwrap();

        let loader = TSVLoader::new(&tsv_path).with_headers(false);
        let docs = loader.load().await.unwrap();

        assert_eq!(
            docs.len(),
            1,
            "TSV without headers should create 1 document"
        );
        assert_eq!(
            docs[0].get_metadata("row_count").and_then(|v| v.as_i64()),
            Some(3),
            "TSV with 3 rows (no header) should have row_count=3"
        );

        // All three rows should be treated as data (no header row consumed)
        assert!(
            docs[0].page_content.contains("100\tItem1"),
            "Content should contain first row as data: '100\\tItem1'"
        );
        assert!(
            docs[0].page_content.contains("200\tItem2"),
            "Content should contain second row as data: '200\\tItem2'"
        );
        assert!(
            docs[0].page_content.contains("300\tItem3"),
            "Content should contain third row as data: '300\\tItem3'"
        );
    }

    #[tokio::test]
    async fn test_tsv_loader_whitespace_trimming() {
        let temp_dir = TempDir::new().unwrap();
        let tsv_path = temp_dir.path().join("whitespace.tsv");

        // TSV with leading/trailing whitespace in fields
        let tsv_content = "Name\t Age \t  City  \n  Alice  \t30\tNew York\nBob\t  25  \t  LA  \n";

        std::fs::write(&tsv_path, tsv_content).unwrap();

        let loader = TSVLoader::new(&tsv_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(
            docs.len(),
            1,
            "TSV with whitespace should create 1 document"
        );

        // Parser trims whitespace from fields (lines 3058, 3069: s.trim())
        // However, content reconstruction preserves original lines for data rows
        // Headers are trimmed and reconstructed with tabs (line 3096)
        // Data rows use original lines (line 3101: all_content.push_str(line))

        // For separate_rows mode, values are trimmed (line 3069)
        // Let's test separate mode to verify trimming
        let loader_sep = TSVLoader::new(&tsv_path).with_separate_rows(true);
        let docs_sep = loader_sep.load().await.unwrap();

        assert_eq!(docs_sep.len(), 2, "Separate mode should create 2 documents");

        // In separate mode, headers and values are trimmed
        assert!(
            docs_sep[0].page_content.contains("Name: Alice"),
            "Trimmed header 'Name' and trimmed value 'Alice' should be present"
        );
        assert!(
            docs_sep[0].page_content.contains("Age: 30"),
            "Trimmed header 'Age' (no spaces) and value '30' should be present"
        );
        assert!(
            docs_sep[0].page_content.contains("City: New York"),
            "Trimmed header 'City' and value 'New York' should be present"
        );

        assert!(
            docs_sep[1].page_content.contains("Age: 25"),
            "Second document should have trimmed Age value '25' (not '  25  ')"
        );
        assert!(
            docs_sep[1].page_content.contains("City: LA"),
            "Second document should have trimmed City value 'LA' (not '  LA  ')"
        );
    }

    #[tokio::test]
    async fn test_tsv_loader_very_long_row() {
        let temp_dir = TempDir::new().unwrap();
        let tsv_path = temp_dir.path().join("long_row.tsv");

        // TSV with very long field value (5000 characters)
        let long_text = "A".repeat(5000);
        let tsv_content = format!("ID\tText\n1\t{}\n2\tShort\n", long_text);

        std::fs::write(&tsv_path, tsv_content).unwrap();

        let loader = TSVLoader::new(&tsv_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(
            docs.len(),
            1,
            "TSV with very long row should create 1 document"
        );
        assert_eq!(
            docs[0].get_metadata("row_count").and_then(|v| v.as_i64()),
            Some(2),
            "TSV with 2 data rows should have row_count=2"
        );

        // Validate long text is present
        assert!(
            docs[0].page_content.contains(&long_text),
            "Content should contain very long field value (5000 'A' characters)"
        );
        assert!(
            docs[0].page_content.contains("2\tShort"),
            "Content should also contain second row with short value"
        );
    }

    #[tokio::test]
    async fn test_tsv_loader_file_not_found() {
        let tsv_path = Path::new("/nonexistent/path/file.tsv");

        let loader = TSVLoader::new(tsv_path);
        let result = loader.load().await;

        assert!(
            result.is_err(),
            "Loading nonexistent TSV file should return error"
        );
    }
}
