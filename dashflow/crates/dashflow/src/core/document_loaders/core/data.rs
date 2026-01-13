// Document loader clippy exceptions:
// - clone_on_ref_ptr: Arc::clone() for sharing loader state across documents
// - needless_pass_by_value: API ergonomics - PathBuf/String parameters are cheap
// - redundant_clone: Clone before async operations for ownership clarity
#![allow(clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! Core data structure loaders.
//!
//! This module provides loaders for structured data formats:
//! - **`ExcelLoader`**: Load Excel spreadsheets (.xlsx, .xls, .xlsm)
//! - **`DataFrameLoader`**: Load in-memory data structures (JSON array of objects)
//!
//! © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::core::documents::{Document, DocumentLoader};
use crate::core::error::Result;
use serde_json::Value;

/// Loads Excel spreadsheet files (.xlsx, .xls, .xlsm).
///
/// The `ExcelLoader` reads Excel files using the calamine crate and converts
/// each sheet into a separate document. Rows are represented as CSV-formatted text.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::ExcelLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = ExcelLoader::new("data.xlsx");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct ExcelLoader {
    file_path: PathBuf,
    sheet_names: Option<Vec<String>>,
}

impl ExcelLoader {
    /// Create a new Excel loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            sheet_names: None,
        }
    }

    /// Load only specific sheets by name.
    #[must_use]
    pub fn with_sheets(mut self, sheets: Vec<String>) -> Self {
        self.sheet_names = Some(sheets);
        self
    }
}

#[async_trait]
impl DocumentLoader for ExcelLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        // Clone data for spawn_blocking (avoid blocking async runtime with std::fs)
        let file_path = self.file_path.clone();
        let sheet_names_filter = self.sheet_names.clone();

        // Perform all filesystem I/O and Excel parsing in spawn_blocking
        tokio::task::spawn_blocking(move || {
            use calamine::Reader;
            use std::fs::File;
            use std::io::BufReader;

            let file = File::open(&file_path)?;
            let reader = BufReader::new(file);

            // Use calamine crate to read Excel files
            let mut workbook: calamine::Xlsx<_> =
                calamine::open_workbook_from_rs(reader).map_err(|e| {
                    crate::core::error::Error::InvalidInput(format!(
                        "Failed to open Excel file: {e}"
                    ))
                })?;

            let sheet_names = if let Some(ref names) = sheet_names_filter {
                names.clone()
            } else {
                workbook.sheet_names().clone()
            };

            let mut documents = Vec::new();

            for sheet_name in sheet_names {
                if let Ok(range) = workbook.worksheet_range(&sheet_name) {
                    let mut content = String::new();

                    for row in range.rows() {
                        let row_text: Vec<String> =
                            row.iter().map(std::string::ToString::to_string).collect();
                        content.push_str(&row_text.join(","));
                        content.push('\n');
                    }

                    documents.push(
                        Document::new(content)
                            .with_metadata("source", file_path.display().to_string())
                            .with_metadata("sheet", sheet_name.clone())
                            .with_metadata("format", "excel"),
                    );
                }
            }

            Ok::<Vec<Document>, crate::core::error::Error>(documents)
        })
        .await
        .map_err(|e| crate::core::error::Error::other(format!("Task join failed: {e}")))?
    }
}

/// Loads data from an in-memory data structure (similar to pandas `DataFrame`).
///
/// This loader takes structured data as a vector of rows and converts it to documents.
/// Each row can become a separate document, or all rows can be combined.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::DataFrameLoader;
/// use dashflow::core::documents::DocumentLoader;
/// use serde_json::json;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let data = vec![
///     json!({"name": "Alice", "age": 30}),
///     json!({"name": "Bob", "age": 25}),
/// ];
/// let loader = DataFrameLoader::new(data);
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct DataFrameLoader {
    data: Vec<Value>,
    page_content_column: Option<String>,
    combine_rows: bool,
}

impl DataFrameLoader {
    /// Create a new `DataFrame` loader with the given data.
    #[must_use]
    pub fn new(data: Vec<Value>) -> Self {
        Self {
            data,
            page_content_column: None,
            combine_rows: false,
        }
    }

    /// Specify which column should be used as the page content.
    /// If not set, all columns are serialized to JSON.
    #[must_use]
    pub fn with_page_content_column(mut self, column: impl Into<String>) -> Self {
        self.page_content_column = Some(column.into());
        self
    }

    /// Combine all rows into a single document instead of one document per row.
    #[must_use]
    pub fn with_combine_rows(mut self, combine: bool) -> Self {
        self.combine_rows = combine;
        self
    }
}

#[async_trait]
impl DocumentLoader for DataFrameLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        if self.combine_rows {
            // Combine all rows into a single document
            let mut content = String::new();
            for (i, row) in self.data.iter().enumerate() {
                if let Some(ref col) = self.page_content_column {
                    if let Some(value) = row.get(col) {
                        content.push_str(&value.to_string());
                    }
                } else {
                    content.push_str(&serde_json::to_string_pretty(row).unwrap_or_default());
                }
                if i < self.data.len() - 1 {
                    content.push('\n');
                }
            }

            Ok(vec![Document::new(content)
                .with_metadata("source", "dataframe")
                .with_metadata("format", "json")
                .with_metadata("rows", self.data.len() as i64)])
        } else {
            // Create one document per row
            let documents: Vec<Document> = self
                .data
                .iter()
                .enumerate()
                .map(|(i, row)| {
                    let content = if let Some(ref col) = self.page_content_column {
                        row.get(col)
                            .map(std::string::ToString::to_string)
                            .unwrap_or_default()
                    } else {
                        serde_json::to_string_pretty(row).unwrap_or_default()
                    };

                    Document::new(content)
                        .with_metadata("source", "dataframe")
                        .with_metadata("format", "json")
                        .with_metadata("row", i as i64)
                })
                .collect();

            Ok(documents)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    // ==========================================================================
    // ExcelLoader Tests
    // ==========================================================================

    #[test]
    fn test_excel_loader_new() {
        let loader = ExcelLoader::new("data.xlsx");
        assert_eq!(loader.file_path, PathBuf::from("data.xlsx"));
        assert!(loader.sheet_names.is_none());
    }

    #[test]
    fn test_excel_loader_with_sheets() {
        let loader =
            ExcelLoader::new("data.xlsx").with_sheets(vec!["Sheet1".to_string(), "Sheet2".to_string()]);
        assert_eq!(
            loader.sheet_names,
            Some(vec!["Sheet1".to_string(), "Sheet2".to_string()])
        );
    }

    #[test]
    fn test_excel_loader_clone() {
        let loader =
            ExcelLoader::new("data.xlsx").with_sheets(vec!["Sheet1".to_string()]);
        let cloned = loader.clone();
        assert_eq!(cloned.file_path, loader.file_path);
        assert_eq!(cloned.sheet_names, loader.sheet_names);
    }

    #[test]
    fn test_excel_loader_debug() {
        let loader = ExcelLoader::new("test.xlsx");
        let debug_str = format!("{:?}", loader);
        assert!(debug_str.contains("ExcelLoader"));
        assert!(debug_str.contains("test.xlsx"));
    }

    #[test]
    fn test_excel_loader_various_extensions() {
        let loader1 = ExcelLoader::new("file.xlsx");
        assert_eq!(loader1.file_path, PathBuf::from("file.xlsx"));

        let loader2 = ExcelLoader::new("file.xls");
        assert_eq!(loader2.file_path, PathBuf::from("file.xls"));

        let loader3 = ExcelLoader::new("file.xlsm");
        assert_eq!(loader3.file_path, PathBuf::from("file.xlsm"));
    }

    #[tokio::test]
    async fn test_excel_loader_file_not_found() {
        let loader = ExcelLoader::new("/nonexistent/path/file.xlsx");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    // ==========================================================================
    // DataFrameLoader Tests
    // ==========================================================================

    #[test]
    fn test_dataframe_loader_new() {
        let data = vec![json!({"name": "Alice"})];
        let loader = DataFrameLoader::new(data.clone());
        assert_eq!(loader.data.len(), 1);
        assert!(loader.page_content_column.is_none());
        assert!(!loader.combine_rows);
    }

    #[test]
    fn test_dataframe_loader_with_page_content_column() {
        let data = vec![json!({"name": "Alice", "bio": "A person"})];
        let loader = DataFrameLoader::new(data).with_page_content_column("bio");
        assert_eq!(loader.page_content_column, Some("bio".to_string()));
    }

    #[test]
    fn test_dataframe_loader_with_combine_rows() {
        let data = vec![json!({"name": "Alice"})];
        let loader = DataFrameLoader::new(data).with_combine_rows(true);
        assert!(loader.combine_rows);

        let loader2 = DataFrameLoader::new(vec![]).with_combine_rows(false);
        assert!(!loader2.combine_rows);
    }

    #[test]
    fn test_dataframe_loader_chained_config() {
        let data = vec![json!({"name": "Alice", "bio": "A person"})];
        let loader = DataFrameLoader::new(data)
            .with_page_content_column("bio")
            .with_combine_rows(true);

        assert_eq!(loader.page_content_column, Some("bio".to_string()));
        assert!(loader.combine_rows);
    }

    #[tokio::test]
    async fn test_dataframe_loader_single_row() {
        let data = vec![json!({"name": "Alice", "age": 30})];
        let loader = DataFrameLoader::new(data);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Alice"));
        assert!(docs[0].page_content.contains("30"));
        assert_eq!(docs[0].metadata.get("source").unwrap(), "dataframe");
        assert_eq!(docs[0].metadata.get("format").unwrap(), "json");
        assert_eq!(docs[0].metadata.get("row").unwrap(), &0);
    }

    #[tokio::test]
    async fn test_dataframe_loader_multiple_rows() {
        let data = vec![
            json!({"name": "Alice", "age": 30}),
            json!({"name": "Bob", "age": 25}),
            json!({"name": "Charlie", "age": 35}),
        ];
        let loader = DataFrameLoader::new(data);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 3);
        assert!(docs[0].page_content.contains("Alice"));
        assert!(docs[1].page_content.contains("Bob"));
        assert!(docs[2].page_content.contains("Charlie"));

        assert_eq!(docs[0].metadata.get("row").unwrap(), &0);
        assert_eq!(docs[1].metadata.get("row").unwrap(), &1);
        assert_eq!(docs[2].metadata.get("row").unwrap(), &2);
    }

    #[tokio::test]
    async fn test_dataframe_loader_with_page_content_column_specified() {
        let data = vec![
            json!({"name": "Alice", "bio": "Software engineer"}),
            json!({"name": "Bob", "bio": "Data scientist"}),
        ];
        let loader = DataFrameLoader::new(data).with_page_content_column("bio");
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert!(docs[0].page_content.contains("Software engineer"));
        assert!(docs[1].page_content.contains("Data scientist"));
        // Should NOT contain the name field since we specified page_content_column
        assert!(!docs[0].page_content.contains("Alice"));
    }

    #[tokio::test]
    async fn test_dataframe_loader_combine_rows() {
        let data = vec![
            json!({"name": "Alice"}),
            json!({"name": "Bob"}),
        ];
        let loader = DataFrameLoader::new(data).with_combine_rows(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Alice"));
        assert!(docs[0].page_content.contains("Bob"));
        assert_eq!(docs[0].metadata.get("rows").unwrap(), &2);
    }

    #[tokio::test]
    async fn test_dataframe_loader_combine_rows_with_page_content_column() {
        let data = vec![
            json!({"name": "Alice", "bio": "Engineer"}),
            json!({"name": "Bob", "bio": "Scientist"}),
        ];
        let loader = DataFrameLoader::new(data)
            .with_page_content_column("bio")
            .with_combine_rows(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Engineer"));
        assert!(docs[0].page_content.contains("Scientist"));
    }

    #[tokio::test]
    async fn test_dataframe_loader_empty_data() {
        let data: Vec<Value> = vec![];
        let loader = DataFrameLoader::new(data);
        let docs = loader.load().await.unwrap();

        assert!(docs.is_empty());
    }

    #[tokio::test]
    async fn test_dataframe_loader_empty_data_combine_rows() {
        let data: Vec<Value> = vec![];
        let loader = DataFrameLoader::new(data).with_combine_rows(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
        assert_eq!(docs[0].metadata.get("rows").unwrap(), &0);
    }

    #[tokio::test]
    async fn test_dataframe_loader_missing_column() {
        let data = vec![
            json!({"name": "Alice"}),
            json!({"name": "Bob"}),
        ];
        // Specify a column that doesn't exist
        let loader = DataFrameLoader::new(data).with_page_content_column("nonexistent");
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        // Should have empty content since column doesn't exist
        assert!(docs[0].page_content.is_empty());
        assert!(docs[1].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_dataframe_loader_nested_json() {
        let data = vec![json!({
            "name": "Alice",
            "address": {
                "city": "New York",
                "country": "USA"
            }
        })];
        let loader = DataFrameLoader::new(data);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Alice"));
        assert!(docs[0].page_content.contains("New York"));
        assert!(docs[0].page_content.contains("USA"));
    }

    #[tokio::test]
    async fn test_dataframe_loader_array_values() {
        let data = vec![json!({
            "name": "Alice",
            "skills": ["Rust", "Python", "SQL"]
        })];
        let loader = DataFrameLoader::new(data);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Rust"));
        assert!(docs[0].page_content.contains("Python"));
    }

    #[tokio::test]
    async fn test_dataframe_loader_null_values() {
        let data = vec![json!({
            "name": "Alice",
            "bio": null
        })];
        let loader = DataFrameLoader::new(data);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("null"));
    }

    #[tokio::test]
    async fn test_dataframe_loader_numeric_values() {
        let data = vec![json!({
            "count": 42,
            "price": 19.99,
            "is_active": true
        })];
        let loader = DataFrameLoader::new(data);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("42"));
        assert!(docs[0].page_content.contains("19.99"));
        assert!(docs[0].page_content.contains("true"));
    }

    // ==========================================================================
    // DocumentLoader Trait Tests
    // ==========================================================================

    #[test]
    fn test_loaders_implement_document_loader() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<ExcelLoader>();
        _assert_document_loader::<DataFrameLoader>();
    }
}
