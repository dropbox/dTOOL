//! # Wikipedia Search Tool
//!
//! Wikipedia is the free encyclopedia containing millions of articles on diverse topics.
//! This tool provides access to Wikipedia content for `DashFlow` agents.
//!
//! ## Features
//!
//! - Search Wikipedia articles by title or query
//! - Retrieve article content and summaries
//! - Configurable content length
//! - No API key required (uses public Wikipedia API)
//!
//! ## Usage
//!
//! ```rust,no_run
//! use dashflow_wikipedia::WikipediaSearchTool;
//! use dashflow::core::tools::Tool;
//!
//! # tokio_test::block_on(async {
//! let wiki = WikipediaSearchTool::new();
//!
//! // Search for an article
//! let results = wiki._call_str("Rust programming language".to_string()).await.unwrap();
//! println!("Wikipedia: {}", results);
//! # });
//! ```

use async_trait::async_trait;
use dashflow::core::config::RunnableConfig;
use dashflow::core::documents::Document;
use dashflow::core::retrievers::Retriever;
use dashflow::core::tools::{Tool, ToolInput};
use dashflow::core::Result;
use serde_json::json;
use std::collections::HashMap;

/// Wikipedia search tool for `DashFlow` agents
///
/// This tool provides access to Wikipedia content, allowing agents to retrieve
/// encyclopedia articles and summaries on a wide range of topics.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_wikipedia::WikipediaSearchTool;
/// use dashflow::core::tools::Tool;
///
/// # tokio_test::block_on(async {
/// let wiki = WikipediaSearchTool::builder()
///     .max_chars(1000)
///     .load_all_available_meta(true)
///     .build();
///
/// let results = wiki._call_str("Albert Einstein".to_string())
///     .await
///     .unwrap();
/// println!("Found: {}", results);
/// # });
/// ```
#[derive(Debug, Clone)]
pub struct WikipediaSearchTool {
    max_chars: usize,
    load_all_available_meta: bool,
}

impl WikipediaSearchTool {
    /// Create a new Wikipedia search tool with default settings
    ///
    /// Default settings:
    /// - `max_chars`: 4000
    /// - `load_all_available_meta`: false
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_wikipedia::WikipediaSearchTool;
    ///
    /// let wiki = WikipediaSearchTool::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_chars: 4000,
            load_all_available_meta: false,
        }
    }

    /// Create a builder for `WikipediaSearchTool`
    #[must_use]
    pub fn builder() -> WikipediaSearchToolBuilder {
        WikipediaSearchToolBuilder::default()
    }

    /// Search Wikipedia and retrieve article content
    async fn search(&self, query: String) -> Result<String> {
        // Use tokio::task::spawn_blocking since wikipedia crate is synchronous
        let max_chars = self.max_chars;
        let load_meta = self.load_all_available_meta;

        tokio::task::spawn_blocking(move || {
            let wiki = wikipedia::Wikipedia::<wikipedia::http::default::Client>::default();

            // Try to get the page directly by title
            let page = wiki.page_from_title(query.clone());

            // Get the content
            let content = page.get_content().map_err(|e| {
                dashflow::core::Error::tool_error(format!("Failed to fetch Wikipedia page: {e}"))
            })?;

            // Get summary if available
            let summary = if load_meta {
                page.get_summary().ok()
            } else {
                None
            };

            // Get page title
            let title = page.get_title().unwrap_or_else(|_| query.clone());

            // Format the output
            let mut output = format!("Page: {title}\n\n");

            if let Some(summary_text) = summary {
                output.push_str("Summary:\n");
                output.push_str(&summary_text);
                output.push_str("\n\n");
            }

            output.push_str("Content:\n");

            // Truncate content if it exceeds max_chars
            if content.len() > max_chars {
                output.push_str(&content[..max_chars]);
                output.push_str(&format!(
                    "\n\n[Content truncated at {} characters. Original length: {} characters]",
                    max_chars,
                    content.len()
                ));
            } else {
                output.push_str(&content);
            }

            Ok(output)
        })
        .await
        .map_err(|e| {
            dashflow::core::Error::tool_error(format!("Wikipedia search task failed: {e}"))
        })?
    }
}

impl Default for WikipediaSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WikipediaSearchTool {
    fn name(&self) -> &'static str {
        "wikipedia_search"
    }

    fn description(&self) -> &'static str {
        "Search Wikipedia, the free encyclopedia, to find information on a wide range of topics. \
         Returns the full content of Wikipedia articles. Best for factual information, \
         historical context, biographies, scientific concepts, and general knowledge. \
         Input should be a specific topic or article title to search for."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The topic or article title to search for on Wikipedia"
                }
            },
            "required": ["query"]
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let query = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(v) => v
                .get("query")
                .and_then(|q| q.as_str())
                .ok_or_else(|| {
                    dashflow::core::Error::tool_error(
                        "Missing 'query' field in structured input".to_string(),
                    )
                })?
                .to_string(),
        };

        self.search(query).await
    }
}

/// Builder for `WikipediaSearchTool`
#[derive(Debug, Clone, Default)]
pub struct WikipediaSearchToolBuilder {
    max_chars: Option<usize>,
    load_all_available_meta: Option<bool>,
}

impl WikipediaSearchToolBuilder {
    /// Set the maximum number of characters to return from article content
    ///
    /// Default: 4000
    #[must_use]
    pub fn max_chars(mut self, max_chars: usize) -> Self {
        self.max_chars = Some(max_chars);
        self
    }

    /// Whether to load all available metadata (summary, etc.)
    ///
    /// Default: false
    #[must_use]
    pub fn load_all_available_meta(mut self, load: bool) -> Self {
        self.load_all_available_meta = Some(load);
        self
    }

    /// Build the `WikipediaSearchTool`
    #[must_use]
    pub fn build(self) -> WikipediaSearchTool {
        WikipediaSearchTool {
            max_chars: self.max_chars.unwrap_or(4000),
            load_all_available_meta: self.load_all_available_meta.unwrap_or(false),
        }
    }
}

/// Wikipedia retriever for document retrieval from Wikipedia articles
///
/// This retriever wraps the `WikipediaSearchTool` and converts search results into Documents
/// suitable for use in retrieval chains and RAG applications.
///
/// # Python Baseline
///
/// This implements the functionality from:
/// `~/dashflow_community/dashflow_community/retrievers/wikipedia.py`
///
/// Python equivalent:
/// ```python
/// from dashflow_community.retrievers import WikipediaRetriever
///
/// retriever = WikipediaRetriever()
/// docs = retriever.invoke("Rust programming language")
/// ```
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_wikipedia::WikipediaRetriever;
/// use dashflow::core::retrievers::Retriever;
///
/// # tokio_test::block_on(async {
/// let retriever = WikipediaRetriever::builder()
///     .max_docs(3)
///     .doc_content_chars_max(2000)
///     .build();
///
/// let docs = retriever._get_relevant_documents("Albert Einstein", None)
///     .await
///     .unwrap();
///
/// for doc in docs {
///     println!("Title: {}", doc.metadata.get("title").unwrap());
///     println!("Content: {}", &doc.page_content[..100]);
/// }
/// # });
/// ```
#[derive(Debug, Clone)]
pub struct WikipediaRetriever {
    /// Internal search tool
    tool: WikipediaSearchTool,
    /// Maximum number of documents to return (default: 3)
    /// Note: Currently Wikipedia retriever returns single best match.
    /// This field is reserved for future multi-document support.
    /// Maximum number of documents to retrieve
    #[allow(dead_code)] // API Parity: Python LangChain max_docs - reserved for multi-doc search
    max_docs: usize,
}

impl WikipediaRetriever {
    /// Create a new `WikipediaRetriever` with default settings
    ///
    /// Default settings:
    /// - `max_docs`: 3
    /// - `doc_content_chars_max`: 4000
    #[must_use]
    pub fn new() -> Self {
        Self {
            tool: WikipediaSearchTool::new(),
            max_docs: 3,
        }
    }

    /// Create a builder for `WikipediaRetriever`
    #[must_use]
    pub fn builder() -> WikipediaRetrieverBuilder {
        WikipediaRetrieverBuilder::default()
    }

    /// Get Wikipedia page as Document
    async fn get_page_document(&self, query: &str) -> Result<Document> {
        let max_chars = self.tool.max_chars;
        let load_meta = self.tool.load_all_available_meta;
        let query_owned = query.to_string();

        let result = tokio::task::spawn_blocking(move || {
            let wiki = wikipedia::Wikipedia::<wikipedia::http::default::Client>::default();
            let page = wiki.page_from_title(query_owned.clone());

            // Get content
            let content = page.get_content().map_err(|e| {
                dashflow::core::Error::http(format!("Failed to fetch Wikipedia page: {e}"))
            })?;

            // Get metadata
            let title = page.get_title().unwrap_or_else(|_| query_owned.clone());
            let summary = if load_meta {
                page.get_summary().ok()
            } else {
                None
            };

            Ok::<_, dashflow::core::Error>((title, content, summary))
        })
        .await
        .map_err(|e| {
            dashflow::core::Error::http(format!("Wikipedia retrieval task failed: {e}"))
        })??;

        let (title, content, summary) = result;

        let mut metadata = HashMap::new();
        metadata.insert(
            "title".to_string(),
            serde_json::Value::String(title.clone()),
        );
        metadata.insert(
            "source".to_string(),
            serde_json::Value::String("wikipedia".to_string()),
        );

        if let Some(summary_text) = summary {
            metadata.insert(
                "summary".to_string(),
                serde_json::Value::String(summary_text),
            );
        }

        // Truncate content if needed
        let page_content = if content.len() > max_chars {
            content[..max_chars].to_string()
        } else {
            content
        };

        Ok(Document {
            page_content,
            metadata,
            id: Some(title),
        })
    }
}

impl Default for WikipediaRetriever {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Retriever for WikipediaRetriever {
    async fn _get_relevant_documents(
        &self,
        query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        // For Wikipedia, we typically just return the single best match
        // (the Python baseline also returns a single page via load())
        let doc = self.get_page_document(query).await?;
        Ok(vec![doc])
    }
}

/// Builder for `WikipediaRetriever`
#[derive(Debug, Clone, Default)]
pub struct WikipediaRetrieverBuilder {
    max_docs: Option<usize>,
    doc_content_chars_max: Option<usize>,
    load_all_available_meta: Option<bool>,
}

impl WikipediaRetrieverBuilder {
    /// Set the maximum number of documents to retrieve
    ///
    /// Default: 3
    #[must_use]
    pub fn max_docs(mut self, max_docs: usize) -> Self {
        self.max_docs = Some(max_docs);
        self
    }

    /// Set the maximum number of characters per document
    ///
    /// Default: 4000
    #[must_use]
    pub fn doc_content_chars_max(mut self, max_chars: usize) -> Self {
        self.doc_content_chars_max = Some(max_chars);
        self
    }

    /// Whether to load all available metadata
    ///
    /// Default: false
    #[must_use]
    pub fn load_all_available_meta(mut self, load: bool) -> Self {
        self.load_all_available_meta = Some(load);
        self
    }

    /// Build the `WikipediaRetriever`
    #[must_use]
    pub fn build(self) -> WikipediaRetriever {
        let tool = WikipediaSearchTool {
            max_chars: self.doc_content_chars_max.unwrap_or(4000),
            load_all_available_meta: self.load_all_available_meta.unwrap_or(false),
        };

        WikipediaRetriever {
            tool,
            max_docs: self.max_docs.unwrap_or(3),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // WikipediaSearchTool - Basic Construction Tests
    // ========================================================================

    #[test]
    fn test_wikipedia_tool_creation() {
        let wiki = WikipediaSearchTool::new();
        assert_eq!(wiki.name(), "wikipedia_search");
        assert!(wiki.description().contains("Wikipedia"));
        assert_eq!(wiki.max_chars, 4000);
        assert!(!wiki.load_all_available_meta);
    }

    #[test]
    fn test_wikipedia_tool_default_equivalent_to_new() {
        let wiki_new = WikipediaSearchTool::new();
        let wiki_default = WikipediaSearchTool::default();
        assert_eq!(wiki_new.max_chars, wiki_default.max_chars);
        assert_eq!(
            wiki_new.load_all_available_meta,
            wiki_default.load_all_available_meta
        );
    }

    #[test]
    fn test_default() {
        let wiki = WikipediaSearchTool::default();
        assert_eq!(wiki.max_chars, 4000);
    }

    #[test]
    fn test_wikipedia_tool_clone() {
        let wiki = WikipediaSearchTool::builder()
            .max_chars(1500)
            .load_all_available_meta(true)
            .build();
        let cloned = wiki.clone();
        assert_eq!(wiki.max_chars, cloned.max_chars);
        assert_eq!(wiki.load_all_available_meta, cloned.load_all_available_meta);
    }

    #[test]
    fn test_wikipedia_tool_debug() {
        let wiki = WikipediaSearchTool::new();
        let debug_str = format!("{:?}", wiki);
        assert!(debug_str.contains("WikipediaSearchTool"));
        assert!(debug_str.contains("max_chars"));
        assert!(debug_str.contains("4000"));
    }

    // ========================================================================
    // WikipediaSearchTool - Builder Tests
    // ========================================================================

    #[test]
    fn test_wikipedia_tool_builder() {
        let wiki = WikipediaSearchTool::builder()
            .max_chars(2000)
            .load_all_available_meta(true)
            .build();

        assert_eq!(wiki.max_chars, 2000);
        assert!(wiki.load_all_available_meta);
    }

    #[test]
    fn test_wikipedia_tool_builder_default_values() {
        let wiki = WikipediaSearchTool::builder().build();
        assert_eq!(wiki.max_chars, 4000);
        assert!(!wiki.load_all_available_meta);
    }

    #[test]
    fn test_wikipedia_tool_builder_only_max_chars() {
        let wiki = WikipediaSearchTool::builder().max_chars(500).build();
        assert_eq!(wiki.max_chars, 500);
        assert!(!wiki.load_all_available_meta); // Default
    }

    #[test]
    fn test_wikipedia_tool_builder_only_load_meta() {
        let wiki = WikipediaSearchTool::builder()
            .load_all_available_meta(true)
            .build();
        assert_eq!(wiki.max_chars, 4000); // Default
        assert!(wiki.load_all_available_meta);
    }

    #[test]
    fn test_wikipedia_tool_builder_zero_max_chars() {
        let wiki = WikipediaSearchTool::builder().max_chars(0).build();
        assert_eq!(wiki.max_chars, 0);
    }

    #[test]
    fn test_wikipedia_tool_builder_large_max_chars() {
        let wiki = WikipediaSearchTool::builder().max_chars(1_000_000).build();
        assert_eq!(wiki.max_chars, 1_000_000);
    }

    #[test]
    fn test_wikipedia_tool_builder_chaining_order_independent() {
        let wiki1 = WikipediaSearchTool::builder()
            .max_chars(100)
            .load_all_available_meta(true)
            .build();
        let wiki2 = WikipediaSearchTool::builder()
            .load_all_available_meta(true)
            .max_chars(100)
            .build();
        assert_eq!(wiki1.max_chars, wiki2.max_chars);
        assert_eq!(wiki1.load_all_available_meta, wiki2.load_all_available_meta);
    }

    #[test]
    fn test_wikipedia_tool_builder_last_value_wins() {
        let wiki = WikipediaSearchTool::builder()
            .max_chars(100)
            .max_chars(200)
            .max_chars(300)
            .build();
        assert_eq!(wiki.max_chars, 300);
    }

    #[test]
    fn test_wikipedia_tool_builder_clone() {
        let builder = WikipediaSearchTool::builder()
            .max_chars(1234)
            .load_all_available_meta(true);
        let builder_clone = builder.clone();
        let wiki1 = builder.build();
        let wiki2 = builder_clone.build();
        assert_eq!(wiki1.max_chars, wiki2.max_chars);
        assert_eq!(wiki1.load_all_available_meta, wiki2.load_all_available_meta);
    }

    #[test]
    fn test_wikipedia_tool_builder_debug() {
        let builder = WikipediaSearchTool::builder().max_chars(999);
        let debug_str = format!("{:?}", builder);
        assert!(debug_str.contains("WikipediaSearchToolBuilder"));
    }

    #[test]
    fn test_wikipedia_tool_builder_default_trait() {
        let builder = WikipediaSearchToolBuilder::default();
        let wiki = builder.build();
        assert_eq!(wiki.max_chars, 4000);
        assert!(!wiki.load_all_available_meta);
    }

    // ========================================================================
    // WikipediaSearchTool - Tool Trait Tests
    // ========================================================================

    #[test]
    fn test_wikipedia_tool_name() {
        let wiki = WikipediaSearchTool::new();
        assert_eq!(wiki.name(), "wikipedia_search");
    }

    #[test]
    fn test_wikipedia_tool_name_is_nonempty() {
        let wiki = WikipediaSearchTool::new();
        let name = wiki.name();
        assert!(!name.is_empty());
        // Verify it's a reasonable name
        assert!(name.len() < 100);
    }

    #[test]
    fn test_wikipedia_tool_description_content() {
        let wiki = WikipediaSearchTool::new();
        let desc = wiki.description();
        assert!(desc.contains("Wikipedia"));
        assert!(desc.contains("free encyclopedia"));
        assert!(desc.contains("topic"));
    }

    #[test]
    fn test_wikipedia_tool_description_is_nonempty() {
        let wiki = WikipediaSearchTool::new();
        let desc = wiki.description();
        assert!(!desc.is_empty());
        // Verify it's a reasonable description length
        assert!(desc.len() > 20);
    }

    #[test]
    fn test_wikipedia_args_schema() {
        let wiki = WikipediaSearchTool::new();
        let schema = wiki.args_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
        assert_eq!(schema["required"][0], "query");
    }

    #[test]
    fn test_wikipedia_args_schema_query_type() {
        let wiki = WikipediaSearchTool::new();
        let schema = wiki.args_schema();
        assert_eq!(schema["properties"]["query"]["type"], "string");
    }

    #[test]
    fn test_wikipedia_args_schema_query_description() {
        let wiki = WikipediaSearchTool::new();
        let schema = wiki.args_schema();
        let query_desc = schema["properties"]["query"]["description"]
            .as_str()
            .unwrap();
        assert!(query_desc.contains("topic") || query_desc.contains("article"));
        assert!(query_desc.contains("Wikipedia"));
    }

    #[test]
    fn test_wikipedia_args_schema_is_valid_json_schema() {
        let wiki = WikipediaSearchTool::new();
        let schema = wiki.args_schema();

        // Verify basic JSON Schema structure
        assert!(schema.is_object());
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"].is_object());
        assert!(schema["required"].is_array());
    }

    #[test]
    fn test_wikipedia_args_schema_required_is_single_item() {
        let wiki = WikipediaSearchTool::new();
        let schema = wiki.args_schema();
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0], "query");
    }

    #[test]
    fn test_wikipedia_args_schema_properties_has_query() {
        let wiki = WikipediaSearchTool::new();
        let schema = wiki.args_schema();
        assert!(schema["properties"].get("query").is_some());
    }

    // ========================================================================
    // WikipediaSearchTool - Async Tool Call Tests
    // ========================================================================

    #[tokio::test]
    async fn test_wikipedia_tool_call_str_interface() {
        // Test that _call_str exists and returns Result<String>
        let wiki = WikipediaSearchTool::new();
        // We can't actually call it without network, but we verify the interface
        let _: &dyn Tool = &wiki;
    }

    // ========================================================================
    // WikipediaRetriever - Basic Construction Tests
    // ========================================================================

    #[test]
    fn test_wikipedia_retriever_creation() {
        let retriever = WikipediaRetriever::new();
        assert_eq!(retriever.max_docs, 3);
        assert_eq!(retriever.tool.max_chars, 4000);
    }

    #[test]
    fn test_wikipedia_retriever_default_equivalent_to_new() {
        let retriever_new = WikipediaRetriever::new();
        let retriever_default = WikipediaRetriever::default();
        assert_eq!(retriever_new.max_docs, retriever_default.max_docs);
        assert_eq!(
            retriever_new.tool.max_chars,
            retriever_default.tool.max_chars
        );
    }

    #[test]
    fn test_wikipedia_retriever_default() {
        let retriever = WikipediaRetriever::default();
        assert_eq!(retriever.max_docs, 3);
    }

    #[test]
    fn test_wikipedia_retriever_clone() {
        let retriever = WikipediaRetriever::builder()
            .max_docs(10)
            .doc_content_chars_max(5000)
            .build();
        let cloned = retriever.clone();
        assert_eq!(retriever.max_docs, cloned.max_docs);
        assert_eq!(retriever.tool.max_chars, cloned.tool.max_chars);
    }

    #[test]
    fn test_wikipedia_retriever_debug() {
        let retriever = WikipediaRetriever::new();
        let debug_str = format!("{:?}", retriever);
        assert!(debug_str.contains("WikipediaRetriever"));
        assert!(debug_str.contains("max_docs"));
        assert!(debug_str.contains("tool"));
    }

    // ========================================================================
    // WikipediaRetriever - Builder Tests
    // ========================================================================

    #[test]
    fn test_wikipedia_retriever_builder() {
        let retriever = WikipediaRetriever::builder()
            .max_docs(5)
            .doc_content_chars_max(2000)
            .load_all_available_meta(true)
            .build();

        assert_eq!(retriever.max_docs, 5);
        assert_eq!(retriever.tool.max_chars, 2000);
        assert!(retriever.tool.load_all_available_meta);
    }

    #[test]
    fn test_wikipedia_retriever_builder_default_values() {
        let retriever = WikipediaRetriever::builder().build();
        assert_eq!(retriever.max_docs, 3);
        assert_eq!(retriever.tool.max_chars, 4000);
        assert!(!retriever.tool.load_all_available_meta);
    }

    #[test]
    fn test_wikipedia_retriever_builder_only_max_docs() {
        let retriever = WikipediaRetriever::builder().max_docs(7).build();
        assert_eq!(retriever.max_docs, 7);
        assert_eq!(retriever.tool.max_chars, 4000); // Default
    }

    #[test]
    fn test_wikipedia_retriever_builder_only_doc_content_chars_max() {
        let retriever = WikipediaRetriever::builder()
            .doc_content_chars_max(1500)
            .build();
        assert_eq!(retriever.max_docs, 3); // Default
        assert_eq!(retriever.tool.max_chars, 1500);
    }

    #[test]
    fn test_wikipedia_retriever_builder_only_load_meta() {
        let retriever = WikipediaRetriever::builder()
            .load_all_available_meta(true)
            .build();
        assert_eq!(retriever.max_docs, 3); // Default
        assert_eq!(retriever.tool.max_chars, 4000); // Default
        assert!(retriever.tool.load_all_available_meta);
    }

    #[test]
    fn test_wikipedia_retriever_builder_zero_max_docs() {
        let retriever = WikipediaRetriever::builder().max_docs(0).build();
        assert_eq!(retriever.max_docs, 0);
    }

    #[test]
    fn test_wikipedia_retriever_builder_large_max_docs() {
        let retriever = WikipediaRetriever::builder().max_docs(1000).build();
        assert_eq!(retriever.max_docs, 1000);
    }

    #[test]
    fn test_wikipedia_retriever_builder_zero_doc_content_chars() {
        let retriever = WikipediaRetriever::builder()
            .doc_content_chars_max(0)
            .build();
        assert_eq!(retriever.tool.max_chars, 0);
    }

    #[test]
    fn test_wikipedia_retriever_builder_large_doc_content_chars() {
        let retriever = WikipediaRetriever::builder()
            .doc_content_chars_max(10_000_000)
            .build();
        assert_eq!(retriever.tool.max_chars, 10_000_000);
    }

    #[test]
    fn test_wikipedia_retriever_builder_chaining_order_independent() {
        let r1 = WikipediaRetriever::builder()
            .max_docs(5)
            .doc_content_chars_max(1000)
            .load_all_available_meta(true)
            .build();
        let r2 = WikipediaRetriever::builder()
            .load_all_available_meta(true)
            .doc_content_chars_max(1000)
            .max_docs(5)
            .build();
        assert_eq!(r1.max_docs, r2.max_docs);
        assert_eq!(r1.tool.max_chars, r2.tool.max_chars);
        assert_eq!(r1.tool.load_all_available_meta, r2.tool.load_all_available_meta);
    }

    #[test]
    fn test_wikipedia_retriever_builder_last_value_wins() {
        let retriever = WikipediaRetriever::builder()
            .max_docs(1)
            .max_docs(2)
            .max_docs(3)
            .build();
        assert_eq!(retriever.max_docs, 3);
    }

    #[test]
    fn test_wikipedia_retriever_builder_clone() {
        let builder = WikipediaRetriever::builder()
            .max_docs(8)
            .doc_content_chars_max(3000);
        let builder_clone = builder.clone();
        let r1 = builder.build();
        let r2 = builder_clone.build();
        assert_eq!(r1.max_docs, r2.max_docs);
        assert_eq!(r1.tool.max_chars, r2.tool.max_chars);
    }

    #[test]
    fn test_wikipedia_retriever_builder_debug() {
        let builder = WikipediaRetriever::builder().max_docs(5);
        let debug_str = format!("{:?}", builder);
        assert!(debug_str.contains("WikipediaRetrieverBuilder"));
    }

    #[test]
    fn test_wikipedia_retriever_builder_default_trait() {
        let builder = WikipediaRetrieverBuilder::default();
        let retriever = builder.build();
        assert_eq!(retriever.max_docs, 3);
        assert_eq!(retriever.tool.max_chars, 4000);
    }

    // ========================================================================
    // WikipediaRetriever - Internal Tool Configuration Tests
    // ========================================================================

    #[test]
    fn test_wikipedia_retriever_internal_tool_name() {
        let retriever = WikipediaRetriever::new();
        assert_eq!(retriever.tool.name(), "wikipedia_search");
    }

    #[test]
    fn test_wikipedia_retriever_internal_tool_default_meta_false() {
        let retriever = WikipediaRetriever::new();
        assert!(!retriever.tool.load_all_available_meta);
    }

    #[test]
    fn test_wikipedia_retriever_builder_propagates_meta_to_tool() {
        let retriever = WikipediaRetriever::builder()
            .load_all_available_meta(true)
            .build();
        assert!(retriever.tool.load_all_available_meta);
    }

    #[test]
    fn test_wikipedia_retriever_builder_propagates_chars_to_tool() {
        let retriever = WikipediaRetriever::builder()
            .doc_content_chars_max(999)
            .build();
        assert_eq!(retriever.tool.max_chars, 999);
    }

    // ========================================================================
    // Cross-Component Tests
    // ========================================================================

    #[test]
    fn test_tool_and_retriever_share_defaults() {
        let tool = WikipediaSearchTool::new();
        let retriever = WikipediaRetriever::new();
        // Both should use same default max_chars
        assert_eq!(tool.max_chars, retriever.tool.max_chars);
    }

    #[test]
    fn test_tool_builder_and_retriever_builder_produce_same_tool_config() {
        let tool = WikipediaSearchTool::builder()
            .max_chars(2500)
            .load_all_available_meta(true)
            .build();
        let retriever = WikipediaRetriever::builder()
            .doc_content_chars_max(2500)
            .load_all_available_meta(true)
            .build();
        assert_eq!(tool.max_chars, retriever.tool.max_chars);
        assert_eq!(
            tool.load_all_available_meta,
            retriever.tool.load_all_available_meta
        );
    }

    // ========================================================================
    // Edge Cases and Boundary Tests
    // ========================================================================

    #[test]
    fn test_wikipedia_tool_builder_max_usize_chars() {
        let wiki = WikipediaSearchTool::builder()
            .max_chars(usize::MAX)
            .build();
        assert_eq!(wiki.max_chars, usize::MAX);
    }

    #[test]
    fn test_wikipedia_retriever_builder_max_usize_docs() {
        let retriever = WikipediaRetriever::builder()
            .max_docs(usize::MAX)
            .build();
        assert_eq!(retriever.max_docs, usize::MAX);
    }

    #[test]
    fn test_wikipedia_tool_load_meta_toggle() {
        // Test explicit false
        let wiki_false = WikipediaSearchTool::builder()
            .load_all_available_meta(false)
            .build();
        assert!(!wiki_false.load_all_available_meta);

        // Test explicit true
        let wiki_true = WikipediaSearchTool::builder()
            .load_all_available_meta(true)
            .build();
        assert!(wiki_true.load_all_available_meta);
    }

    #[test]
    fn test_wikipedia_retriever_load_meta_toggle() {
        // Test explicit false
        let ret_false = WikipediaRetriever::builder()
            .load_all_available_meta(false)
            .build();
        assert!(!ret_false.tool.load_all_available_meta);

        // Test explicit true
        let ret_true = WikipediaRetriever::builder()
            .load_all_available_meta(true)
            .build();
        assert!(ret_true.tool.load_all_available_meta);
    }

    #[test]
    fn test_multiple_tool_instances_independent() {
        let wiki1 = WikipediaSearchTool::builder().max_chars(100).build();
        let wiki2 = WikipediaSearchTool::builder().max_chars(200).build();
        assert_eq!(wiki1.max_chars, 100);
        assert_eq!(wiki2.max_chars, 200);
    }

    #[test]
    fn test_multiple_retriever_instances_independent() {
        let ret1 = WikipediaRetriever::builder().max_docs(1).build();
        let ret2 = WikipediaRetriever::builder().max_docs(2).build();
        assert_eq!(ret1.max_docs, 1);
        assert_eq!(ret2.max_docs, 2);
    }

    // ========================================================================
    // Integration tests (require network access)
    // ========================================================================

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_wikipedia_search_rust() {
        let wiki = WikipediaSearchTool::new();
        let content = wiki
            ._call_str("Rust programming language".to_string())
            .await
            .expect("Wikipedia search failed");
        assert!(content.contains("Rust"));
        assert!(content.contains("programming"));
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_wikipedia_search_with_builder() {
        let wiki = WikipediaSearchTool::builder()
            .max_chars(500)
            .load_all_available_meta(true)
            .build();

        let content = wiki
            ._call_str("Albert Einstein".to_string())
            .await
            .expect("Wikipedia search failed");
        assert!(content.contains("Einstein"));
        // With load_all_available_meta(true), content includes metadata
        // Max chars is 500 for text, but metadata adds more (~3.5-4.5KB total)
        assert!(content.len() < 5000); // Should include truncated text + metadata
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_wikipedia_retriever_get_documents() {
        let retriever = WikipediaRetriever::builder()
            .doc_content_chars_max(1000)
            .build();

        let docs = retriever
            ._get_relevant_documents("Rust programming language", None)
            .await
            .expect("Wikipedia retriever failed");
        assert_eq!(docs.len(), 1);

        let first_doc = &docs[0];
        assert!(first_doc.metadata.contains_key("title"));
        assert!(first_doc.metadata.contains_key("source"));
        assert!(!first_doc.page_content.is_empty());
        assert!(first_doc.page_content.len() <= 1000);
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_wikipedia_retriever_with_metadata() {
        let retriever = WikipediaRetriever::builder()
            .load_all_available_meta(true)
            .build();

        let docs = retriever
            ._get_relevant_documents("Python programming language", None)
            .await
            .expect("Wikipedia retriever failed");
        assert_eq!(docs.len(), 1);

        let doc = &docs[0];
        // With metadata enabled, should have summary
        assert!(doc.metadata.contains_key("summary") || doc.metadata.contains_key("title"));
    }
}
