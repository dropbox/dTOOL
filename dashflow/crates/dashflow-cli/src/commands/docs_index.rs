// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! High-performance documentation index using Tantivy.
//!
//! This module provides indexed documentation search for DashFlow crates,
//! designed to be fast and useful for AI code assistants.
//!
//! # Architecture
//!
//! ```text
//! .dashflow/docs_index/
//! ├── tantivy/          # Full-text search index (BM25)
//! └── metadata.json     # Index metadata & timestamps
//! ```
//!
//! # Usage
//!
//! ```bash
//! # Build/rebuild the index
//! dashflow introspect docs index build
//!
//! # Check index status
//! dashflow introspect docs index status
//!
//! # Search (uses index if available, falls back to grep)
//! dashflow introspect docs search StateGraph
//! ```

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::SystemTime;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{doc, Index, IndexWriter, ReloadPolicy};
use walkdir::WalkDir;

/// Index storage location relative to workspace root
const INDEX_DIR: &str = ".dashflow/docs_index";

/// Documentation item extracted from source code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocItem {
    /// Unique identifier (crate::module::name)
    pub qualified_name: String,
    /// Short name (e.g., "StateGraph")
    pub name: String,
    /// Item type (fn, struct, enum, trait, type, const, static, mod)
    pub item_type: String,
    /// Crate name (e.g., "dashflow-openai")
    pub crate_name: String,
    /// Module path within crate (e.g., "assistant")
    pub module_path: String,
    /// Full documentation text
    pub documentation: String,
    /// First line/summary of documentation
    pub summary: String,
    /// Source file path
    pub file_path: String,
    /// Line number in source file
    pub line_number: u32,
    /// Function/type signature if available
    pub signature: Option<String>,
    /// Visibility (pub, pub(crate), etc.)
    pub visibility: String,
}

/// Index metadata for tracking freshness
#[derive(Debug, Serialize, Deserialize)]
pub struct IndexMetadata {
    /// When the index was last built
    pub built_at: String,
    /// Number of items indexed
    pub item_count: usize,
    /// Crates included in index
    pub crates: Vec<String>,
    /// Source files indexed
    pub file_count: usize,
    /// Total documentation bytes
    pub doc_bytes: usize,
    /// Index schema version
    pub schema_version: u32,
    /// File modification timestamps at index time
    pub file_timestamps: HashMap<String, u64>,
}

const SCHEMA_VERSION: u32 = 1;

/// Field references for the documentation index schema.
/// Created once during schema building to avoid runtime field lookups.
#[derive(Clone, Copy)]
struct DocIndexFields {
    qualified_name: Field,
    name: Field,
    item_type: Field,
    crate_name: Field,
    module_path: Field,
    documentation: Field,
    summary: Field,
    file_path: Field,
    line_number: Field,
    signature: Field,
    visibility: Field,
}

impl DocIndexFields {
    /// Get field references from an existing schema (e.g., when opening an index).
    /// Returns an error if any expected field is missing.
    fn from_schema(schema: &Schema) -> Result<Self> {
        Ok(Self {
            qualified_name: schema.get_field("qualified_name")?,
            name: schema.get_field("name")?,
            item_type: schema.get_field("item_type")?,
            crate_name: schema.get_field("crate_name")?,
            module_path: schema.get_field("module_path")?,
            documentation: schema.get_field("documentation")?,
            summary: schema.get_field("summary")?,
            file_path: schema.get_field("file_path")?,
            line_number: schema.get_field("line_number")?,
            signature: schema.get_field("signature")?,
            visibility: schema.get_field("visibility")?,
        })
    }
}

/// Build the Tantivy schema for documentation along with field references.
fn build_schema_with_fields() -> (Schema, DocIndexFields) {
    let mut schema_builder = Schema::builder();

    // Stored and indexed fields - capture references as we build
    let qualified_name = schema_builder.add_text_field("qualified_name", STRING | STORED);
    let name = schema_builder.add_text_field("name", TEXT | STORED);
    let item_type = schema_builder.add_text_field("item_type", STRING | STORED);
    let crate_name = schema_builder.add_text_field("crate_name", STRING | STORED);
    let module_path = schema_builder.add_text_field("module_path", TEXT | STORED);
    let documentation = schema_builder.add_text_field("documentation", TEXT | STORED);
    let summary = schema_builder.add_text_field("summary", TEXT | STORED);
    let file_path = schema_builder.add_text_field("file_path", STRING | STORED);
    let line_number = schema_builder.add_u64_field("line_number", INDEXED | STORED);
    let signature = schema_builder.add_text_field("signature", TEXT | STORED);
    let visibility = schema_builder.add_text_field("visibility", STRING | STORED);

    let fields = DocIndexFields {
        qualified_name,
        name,
        item_type,
        crate_name,
        module_path,
        documentation,
        summary,
        file_path,
        line_number,
        signature,
        visibility,
    };

    (schema_builder.build(), fields)
}

/// Extract documentation items from all Rust source files
pub fn extract_doc_items(crates_dir: &Path) -> Result<Vec<DocItem>> {
    let mut items = Vec::new();

    // Regex patterns for extracting documented items
    // Simplified pattern that's more permissive
    let doc_item_pattern = Regex::new(
        r"(?ms)((?:///[^\n]*\n)+)\s*(?:\#\[[^\]]*\]\s*)*pub(?:\([^)]*\))?\s+(?:async\s+)?(?:unsafe\s+)?(?:const\s+)?(fn|struct|enum|trait|type|const|static|mod|impl)\s+(\w+)",
    )?;

    // Also capture module-level docs (//!)
    let mod_doc_pattern = Regex::new(r"(?m)^[ \t]*//![^\n]*")?;

    for entry in WalkDir::new(crates_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
    {
        let path = entry.path();
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Extract crate name from path
        let crate_name = path
            .strip_prefix(crates_dir)
            .ok()
            .and_then(|p| p.components().next())
            .and_then(|c| c.as_os_str().to_str())
            .unwrap_or("unknown")
            .to_string();

        // Extract module path from file path
        let module_path = path
            .strip_prefix(crates_dir)
            .ok()
            .map(|p| {
                p.components()
                    .skip(1) // Skip crate name
                    .filter_map(|c| c.as_os_str().to_str())
                    .collect::<Vec<_>>()
                    .join("::")
                    .trim_end_matches(".rs")
                    .replace("/", "::")
                    .replace("src::", "")
                    .replace("::mod", "")
                    .replace("::lib", "")
            })
            .unwrap_or_default();

        // Extract documented items
        for cap in doc_item_pattern.captures_iter(&content) {
            let doc_text = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let item_type = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let name = cap.get(3).map(|m| m.as_str()).unwrap_or("");
            let visibility = "pub"; // Simplified - we only match pub items

            if name.is_empty() {
                continue;
            }

            // Calculate line number
            let match_start = cap.get(0).map(|m| m.start()).unwrap_or(0);
            let line_number = content[..match_start].matches('\n').count() as u32 + 1;

            // Clean up documentation
            let documentation = doc_text
                .lines()
                .map(|l| l.trim().trim_start_matches("///").trim())
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string();

            // Extract summary (first non-empty line)
            let summary = documentation
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("")
                .to_string();

            // Build signature (simplified - just type and name)
            let signature = if matches!(
                item_type,
                "fn" | "struct" | "enum" | "trait" | "type" | "impl"
            ) {
                Some(format!("{} {} {}", visibility, item_type, name))
            } else {
                None
            };

            // Build qualified name
            let qualified_name = if module_path.is_empty() {
                format!("{}::{}", crate_name, name)
            } else {
                format!("{}::{}::{}", crate_name, module_path, name)
            };

            items.push(DocItem {
                qualified_name,
                name: name.to_string(),
                item_type: item_type.to_string(),
                crate_name: crate_name.clone(),
                module_path: module_path.clone(),
                documentation,
                summary,
                file_path: path.to_string_lossy().to_string(),
                line_number,
                signature,
                visibility: visibility.to_string(),
            });
        }

        // Also extract module-level documentation for lib.rs/mod.rs
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_name == "lib.rs" || file_name == "mod.rs" {
            let mod_docs: Vec<&str> = mod_doc_pattern
                .find_iter(&content)
                .map(|m| m.as_str().trim().trim_start_matches("//!").trim())
                .collect();

            if !mod_docs.is_empty() {
                let documentation = mod_docs.join("\n");
                let summary = mod_docs.first().copied().unwrap_or("").to_string();

                items.push(DocItem {
                    qualified_name: if module_path.is_empty() {
                        crate_name.clone()
                    } else {
                        format!("{}::{}", crate_name, module_path)
                    },
                    name: if module_path.is_empty() {
                        crate_name.clone()
                    } else {
                        module_path
                            .split("::")
                            .last()
                            .unwrap_or(&module_path)
                            .to_string()
                    },
                    item_type: "mod".to_string(),
                    crate_name: crate_name.clone(),
                    module_path: module_path.clone(),
                    documentation,
                    summary,
                    file_path: path.to_string_lossy().to_string(),
                    line_number: 1,
                    signature: None,
                    visibility: "pub".to_string(),
                });
            }
        }
    }

    Ok(items)
}

/// Build the documentation index
pub fn build_index(workspace_root: &Path) -> Result<IndexMetadata> {
    let crates_dir = workspace_root.join("crates");
    let index_path = workspace_root.join(INDEX_DIR).join("tantivy");

    // Create index directory
    fs::create_dir_all(&index_path)?;

    // Extract all documentation items
    let items = extract_doc_items(&crates_dir)?;

    // Build schema and create index with field references
    let (schema, fields) = build_schema_with_fields();
    let index = Index::create_in_dir(&index_path, schema)?;

    // Index all items
    let mut writer: IndexWriter = index.writer(50_000_000)?; // 50MB buffer
    let mut doc_bytes = 0usize;
    let mut file_timestamps: HashMap<String, u64> = HashMap::new();

    for item in &items {
        doc_bytes += item.documentation.len();

        writer.add_document(doc!(
            fields.qualified_name => item.qualified_name.clone(),
            fields.name => item.name.clone(),
            fields.item_type => item.item_type.clone(),
            fields.crate_name => item.crate_name.clone(),
            fields.module_path => item.module_path.clone(),
            fields.documentation => item.documentation.clone(),
            fields.summary => item.summary.clone(),
            fields.file_path => item.file_path.clone(),
            fields.line_number => item.line_number as u64,
            fields.signature => item.signature.clone().unwrap_or_default(),
            fields.visibility => item.visibility.clone(),
        ))?;

        // Track file timestamps
        if !file_timestamps.contains_key(&item.file_path) {
            if let Ok(metadata) = fs::metadata(&item.file_path) {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(duration) = modified.duration_since(SystemTime::UNIX_EPOCH) {
                        file_timestamps.insert(item.file_path.clone(), duration.as_secs());
                    }
                }
            }
        }
    }

    writer.commit()?;

    // Collect unique crates
    let mut crates: Vec<String> = items.iter().map(|i| i.crate_name.clone()).collect();
    crates.sort();
    crates.dedup();

    // Build metadata
    let metadata = IndexMetadata {
        built_at: chrono::Utc::now().to_rfc3339(),
        item_count: items.len(),
        crates,
        file_count: file_timestamps.len(),
        doc_bytes,
        schema_version: SCHEMA_VERSION,
        file_timestamps,
    };

    // Save metadata
    let metadata_path = workspace_root.join(INDEX_DIR).join("metadata.json");
    fs::write(&metadata_path, serde_json::to_string_pretty(&metadata)?)?;

    Ok(metadata)
}

/// Check if index exists and is up to date
pub fn check_index_status(workspace_root: &Path) -> Result<Option<IndexMetadata>> {
    let metadata_path = workspace_root.join(INDEX_DIR).join("metadata.json");

    if !metadata_path.exists() {
        return Ok(None);
    }

    let metadata: IndexMetadata = serde_json::from_str(&fs::read_to_string(&metadata_path)?)?;

    Ok(Some(metadata))
}

/// Check if index needs rebuilding (files have changed)
pub fn index_needs_rebuild(workspace_root: &Path) -> Result<bool> {
    let metadata = match check_index_status(workspace_root)? {
        Some(m) => m,
        None => return Ok(true),
    };

    // Check if any indexed file has changed
    for (file_path, indexed_timestamp) in &metadata.file_timestamps {
        if let Ok(file_metadata) = fs::metadata(file_path) {
            if let Ok(modified) = file_metadata.modified() {
                if let Ok(duration) = modified.duration_since(SystemTime::UNIX_EPOCH) {
                    if duration.as_secs() > *indexed_timestamp {
                        return Ok(true);
                    }
                }
            }
        } else {
            // File was deleted
            return Ok(true);
        }
    }

    // Check for new files
    let crates_dir = workspace_root.join("crates");
    for entry in WalkDir::new(&crates_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
    {
        let path_str = entry.path().to_string_lossy().to_string();
        if !metadata.file_timestamps.contains_key(&path_str) {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Search result from the index
#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub qualified_name: String,
    pub name: String,
    pub item_type: String,
    pub crate_name: String,
    pub summary: String,
    pub file_path: String,
    pub line_number: u32,
    pub score: f32,
}

/// Sanitize query for Tantivy query parser
/// Removes special characters that would cause syntax errors
fn sanitize_query(query: &str) -> String {
    // Keep only alphanumeric, spaces, and underscores
    // This provides a simple, robust search experience
    query
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c.is_whitespace() {
                c
            } else {
                ' ' // Replace special chars with space
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Search the documentation index
pub fn search_index(workspace_root: &Path, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
    let index_path = workspace_root.join(INDEX_DIR).join("tantivy");

    if !index_path.exists() {
        anyhow::bail!("Index not found. Run: dashflow introspect docs index build");
    }

    let index = Index::open_in_dir(&index_path)?;
    let schema = index.schema();
    let fields = DocIndexFields::from_schema(&schema)?;

    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()?;
    let searcher = reader.searcher();

    // Sanitize query (remove special characters)
    let sanitized_query = sanitize_query(query);
    let query_parser = QueryParser::for_index(
        &index,
        vec![
            fields.name,
            fields.documentation,
            fields.summary,
            fields.module_path,
        ],
    );
    let parsed_query = query_parser.parse_query(&sanitized_query)?;

    let top_docs = searcher.search(&parsed_query, &TopDocs::with_limit(limit))?;

    let mut results = Vec::new();
    for (score, doc_address) in top_docs {
        let retrieved_doc: tantivy::TantivyDocument = searcher.doc(doc_address)?;

        // Helper to extract text field value
        let get_text = |field: Field| -> String {
            retrieved_doc
                .get_first(field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };

        results.push(SearchResult {
            qualified_name: get_text(fields.qualified_name),
            name: get_text(fields.name),
            item_type: get_text(fields.item_type),
            crate_name: get_text(fields.crate_name),
            summary: get_text(fields.summary),
            file_path: get_text(fields.file_path),
            line_number: retrieved_doc
                .get_first(fields.line_number)
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            score,
        });
    }

    Ok(results)
}

/// Get full documentation for a specific item
#[allow(dead_code)] // Architectural: Reserved for detailed doc item lookup by qualified name
pub fn get_doc_item(workspace_root: &Path, qualified_name: &str) -> Result<Option<DocItem>> {
    let index_path = workspace_root.join(INDEX_DIR).join("tantivy");

    if !index_path.exists() {
        anyhow::bail!("Index not found. Run: dashflow introspect docs index build");
    }

    let index = Index::open_in_dir(&index_path)?;
    let schema = index.schema();
    let fields = DocIndexFields::from_schema(&schema)?;

    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()?;
    let searcher = reader.searcher();

    // Exact match on qualified_name
    let query_parser = QueryParser::for_index(&index, vec![fields.qualified_name]);

    // Use quotes for exact phrase match
    let parsed_query = query_parser.parse_query(&format!("\"{}\"", qualified_name))?;

    let top_docs = searcher.search(&parsed_query, &TopDocs::with_limit(1))?;

    if let Some((_score, doc_address)) = top_docs.first() {
        let retrieved_doc: tantivy::TantivyDocument = searcher.doc(*doc_address)?;

        // Helper to extract text field value
        let get_text = |field: Field| -> String {
            retrieved_doc
                .get_first(field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };

        let signature = get_text(fields.signature);

        return Ok(Some(DocItem {
            qualified_name: get_text(fields.qualified_name),
            name: get_text(fields.name),
            item_type: get_text(fields.item_type),
            crate_name: get_text(fields.crate_name),
            module_path: get_text(fields.module_path),
            documentation: get_text(fields.documentation),
            summary: get_text(fields.summary),
            file_path: get_text(fields.file_path),
            line_number: retrieved_doc
                .get_first(fields.line_number)
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            signature: if signature.is_empty() {
                None
            } else {
                Some(signature)
            },
            visibility: get_text(fields.visibility),
        }));
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_schema_builds() {
        let (schema, fields) = build_schema_with_fields();
        // Fields struct captures all fields at build time - verify schema still has them
        assert!(schema.get_field("name").is_ok());
        assert!(schema.get_field("documentation").is_ok());
        assert!(schema.get_field("qualified_name").is_ok());
        // Verify fields struct is populated
        assert_eq!(
            schema.get_field("name").unwrap(),
            fields.name,
            "Fields struct should match schema"
        );
    }

    #[test]
    fn test_extract_doc_items() {
        let temp_dir = TempDir::new().unwrap();
        let crate_dir = temp_dir.path().join("test-crate").join("src");
        fs::create_dir_all(&crate_dir).unwrap();

        // Create a test file with documented items
        let test_file = crate_dir.join("lib.rs");
        fs::write(
            &test_file,
            r#"
/// A test struct for documentation.
///
/// This is a longer description.
pub struct TestStruct {
    field: i32,
}

/// A test function.
pub fn test_fn() {}
"#,
        )
        .unwrap();

        let items = extract_doc_items(temp_dir.path()).unwrap();
        assert!(!items.is_empty());

        let struct_item = items.iter().find(|i| i.name == "TestStruct");
        assert!(struct_item.is_some());
        let item = struct_item.unwrap();
        assert_eq!(item.item_type, "struct");
        assert!(item.documentation.contains("test struct"));
    }
}
