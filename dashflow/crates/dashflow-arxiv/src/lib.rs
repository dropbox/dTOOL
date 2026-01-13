//! # Arxiv Research Paper Search Tool
//!
//! Arxiv is a free distribution service and an open-access archive for scholarly articles
//! in the fields of physics, mathematics, computer science, quantitative biology,
//! quantitative finance, statistics, electrical engineering and systems science, and economics.
//!
//! This tool provides access to arXiv research papers for `DashFlow` agents.
//!
//! ## Features
//!
//! - Search arXiv papers by query (title, author, abstract)
//! - Retrieve paper metadata (title, authors, abstract, publication date)
//! - Download paper PDFs
//! - Configurable maximum results
//! - No API key required (uses public arXiv API)
//!
//! ## Usage
//!
//! ```rust,no_run
//! use dashflow_arxiv::ArxivSearchTool;
//! use dashflow::core::tools::Tool;
//!
//! # tokio_test::block_on(async {
//! let arxiv = ArxivSearchTool::new();
//!
//! // Search for papers
//! let results = arxiv._call_str("quantum computing".to_string()).await.unwrap();
//! println!("Arxiv: {}", results);
//! # });
//! ```

use async_trait::async_trait;
use dashflow::core::config::RunnableConfig;
use dashflow::core::documents::Document;
use dashflow::core::retrievers::Retriever;
use dashflow::core::tools::{Tool, ToolInput};
use dashflow::core::Result;
use dashflow::{DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

/// Arxiv research paper search tool for `DashFlow` agents
///
/// This tool provides access to arXiv research papers, allowing agents to search
/// for and retrieve academic papers across multiple scientific disciplines.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_arxiv::ArxivSearchTool;
/// use dashflow::core::tools::Tool;
///
/// # tokio_test::block_on(async {
/// let arxiv = ArxivSearchTool::builder()
///     .max_results(5)
///     .build();
///
/// let results = arxiv._call_str("machine learning".to_string())
///     .await
///     .unwrap();
/// println!("Found: {}", results);
/// # });
/// ```
#[derive(Debug, Clone)]
pub struct ArxivSearchTool {
    max_results: usize,
    sort_by: SortBy,
    sort_order: SortOrder,
}

/// Sort criteria for arXiv search results
#[derive(Debug, Clone, Copy, Default)]
pub enum SortBy {
    /// Sort by relevance (default)
    #[default]
    Relevance,
    /// Sort by last updated date
    LastUpdatedDate,
    /// Sort by submitted date
    SubmittedDate,
}

impl SortBy {
    fn as_str(&self) -> &str {
        match self {
            SortBy::Relevance => "relevance",
            SortBy::LastUpdatedDate => "lastUpdatedDate",
            SortBy::SubmittedDate => "submittedDate",
        }
    }
}

/// Sort order for arXiv search results
#[derive(Debug, Clone, Copy, Default)]
pub enum SortOrder {
    /// Ascending order
    Ascending,
    /// Descending order (default)
    #[default]
    Descending,
}

impl SortOrder {
    fn as_str(&self) -> &str {
        match self {
            SortOrder::Ascending => "ascending",
            SortOrder::Descending => "descending",
        }
    }
}

/// Represents an arXiv paper entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArxivPaper {
    /// Paper ID
    pub id: String,
    /// Paper title
    pub title: String,
    /// Authors
    pub authors: Vec<String>,
    /// Abstract/summary
    pub summary: String,
    /// Publication date
    pub published: String,
    /// PDF URL
    pub pdf_url: String,
    /// Categories
    pub categories: Vec<String>,
}

impl ArxivSearchTool {
    /// Create a new Arxiv search tool with default settings
    ///
    /// Default settings:
    /// - `max_results`: 3
    /// - `sort_by`: Relevance
    /// - `sort_order`: Descending
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_arxiv::ArxivSearchTool;
    ///
    /// let arxiv = ArxivSearchTool::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_results: 3,
            sort_by: SortBy::default(),
            sort_order: SortOrder::default(),
        }
    }

    /// Create a builder for `ArxivSearchTool`
    #[must_use]
    pub fn builder() -> ArxivSearchToolBuilder {
        ArxivSearchToolBuilder::default()
    }

    /// Search arXiv and retrieve paper information
    async fn search(&self, query: String) -> Result<String> {
        let client = reqwest::Client::builder()
            .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
            .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
            .build()
            .map_err(|e| {
                dashflow::core::Error::tool_error(format!("Failed to create HTTP client: {e}"))
            })?;

        // Build the query URL
        let url = format!(
            "https://export.arxiv.org/api/query?search_query=all:{}&start=0&max_results={}&sortBy={}&sortOrder={}",
            urlencoding::encode(&query),
            self.max_results,
            self.sort_by.as_str(),
            self.sort_order.as_str()
        );

        // Make the request
        let response = client.get(&url).send().await.map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to query arXiv API: {e}"))
        })?;

        let xml_text = response.text().await.map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to read arXiv response: {e}"))
        })?;

        // Parse the Atom XML response
        let papers = Self::parse_arxiv_response(&xml_text)?;

        if papers.is_empty() {
            return Ok(format!("No papers found for query: {query}"));
        }

        // Format the output
        let mut output = format!("Found {} papers for query: {}\n\n", papers.len(), query);

        for (i, paper) in papers.iter().enumerate() {
            output.push_str(&format!("Paper {}:\n", i + 1));
            output.push_str(&format!("Title: {}\n", paper.title.trim()));
            output.push_str(&format!("Authors: {}\n", paper.authors.join(", ")));
            output.push_str(&format!("Published: {}\n", paper.published));
            output.push_str(&format!("Categories: {}\n", paper.categories.join(", ")));
            output.push_str(&format!("PDF: {}\n", paper.pdf_url));
            output.push_str(&format!("Summary: {}\n", paper.summary.trim()));
            output.push('\n');
        }

        Ok(output)
    }

    /// Parse arXiv Atom XML response
    fn parse_arxiv_response(xml: &str) -> Result<Vec<ArxivPaper>> {
        use quick_xml::events::Event;
        use quick_xml::Reader;

        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut papers = Vec::new();
        let mut current_paper: Option<ArxivPaper> = None;
        let mut in_entry = false;
        let mut in_author = false;
        let mut text_buffer = String::new();

        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    if name == "entry" {
                        in_entry = true;
                        current_paper = Some(ArxivPaper {
                            id: String::new(),
                            title: String::new(),
                            authors: Vec::new(),
                            summary: String::new(),
                            published: String::new(),
                            pdf_url: String::new(),
                            categories: Vec::new(),
                        });
                    } else if name == "author" && in_entry {
                        in_author = true;
                    } else if name == "link" && in_entry {
                        // Check for PDF link (handles both self-closing and regular tags)
                        let mut is_pdf_link = false;
                        let mut href_value = String::new();
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref());
                            let value = String::from_utf8_lossy(&attr.value);
                            if key == "title" && value == "pdf" {
                                is_pdf_link = true;
                            }
                            if key == "href" {
                                href_value = value.to_string();
                            }
                        }
                        if is_pdf_link && !href_value.is_empty() {
                            if let Some(paper) = current_paper.as_mut() {
                                paper.pdf_url = href_value;
                            }
                        }
                    } else if name == "category" && in_entry {
                        // Get term attribute for categories (handles self-closing tags)
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref());
                            if key == "term" {
                                if let Some(paper) = current_paper.as_mut() {
                                    paper
                                        .categories
                                        .push(String::from_utf8_lossy(&attr.value).to_string());
                                }
                            }
                        }
                    }
                }
                Ok(Event::Text(e)) => {
                    text_buffer = e.unescape().unwrap_or_default().to_string();
                }
                Ok(Event::End(e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    if name == "entry" {
                        if let Some(paper) = current_paper.take() {
                            papers.push(paper);
                        }
                        in_entry = false;
                    } else if name == "author" {
                        in_author = false;
                    } else if in_entry {
                        if let Some(paper) = current_paper.as_mut() {
                            match name.as_str() {
                                "id" => paper.id = text_buffer.clone(),
                                "title" => paper.title = text_buffer.clone(),
                                "summary" => paper.summary = text_buffer.clone(),
                                "published" => paper.published = text_buffer.clone(),
                                "name" if in_author => {
                                    paper.authors.push(text_buffer.clone());
                                }
                                _ => {}
                            }
                        }
                    }
                    text_buffer.clear();
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(dashflow::core::Error::tool_error(format!(
                        "XML parsing error: {e}"
                    )))
                }
                _ => {}
            }
        }

        Ok(papers)
    }
}

impl Default for ArxivSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ArxivSearchTool {
    fn name(&self) -> &'static str {
        "arxiv_search"
    }

    fn description(&self) -> &'static str {
        "Search arXiv for research papers in physics, mathematics, computer science, \
         quantitative biology, quantitative finance, statistics, and other scientific fields. \
         Returns paper titles, authors, abstracts, publication dates, and PDF links. \
         Best for finding academic research papers and scientific literature. \
         Input should be a search query (e.g., 'quantum computing', 'neural networks')."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query for finding research papers on arXiv"
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

/// Builder for `ArxivSearchTool`
#[derive(Debug, Clone, Default)]
pub struct ArxivSearchToolBuilder {
    max_results: Option<usize>,
    sort_by: Option<SortBy>,
    sort_order: Option<SortOrder>,
}

impl ArxivSearchToolBuilder {
    /// Set the maximum number of results to return
    ///
    /// Default: 3
    #[must_use]
    pub fn max_results(mut self, max_results: usize) -> Self {
        self.max_results = Some(max_results);
        self
    }

    /// Set the sort criteria
    ///
    /// Default: Relevance
    #[must_use]
    pub fn sort_by(mut self, sort_by: SortBy) -> Self {
        self.sort_by = Some(sort_by);
        self
    }

    /// Set the sort order
    ///
    /// Default: Descending
    #[must_use]
    pub fn sort_order(mut self, sort_order: SortOrder) -> Self {
        self.sort_order = Some(sort_order);
        self
    }

    /// Build the `ArxivSearchTool`
    #[must_use]
    pub fn build(self) -> ArxivSearchTool {
        ArxivSearchTool {
            max_results: self.max_results.unwrap_or(3),
            sort_by: self.sort_by.unwrap_or_default(),
            sort_order: self.sort_order.unwrap_or_default(),
        }
    }
}

/// Arxiv retriever for document retrieval from arXiv research papers
///
/// This retriever wraps the `ArxivSearchTool` and converts search results into Documents
/// suitable for use in retrieval chains and RAG applications.
///
/// # Python Baseline
///
/// This implements the functionality from:
/// `~/dashflow_community/dashflow_community/retrievers/arxiv.py`
///
/// Python equivalent:
/// ```python
/// from dashflow_community.retrievers import ArxivRetriever
///
/// retriever = ArxivRetriever(
///     load_max_docs=5,
///     get_full_documents=True,
/// )
/// docs = retriever.invoke("quantum computing")
/// ```
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_arxiv::ArxivRetriever;
/// use dashflow::core::retrievers::Retriever;
///
/// # tokio_test::block_on(async {
/// let retriever = ArxivRetriever::builder()
///     .max_results(5)
///     .get_full_documents(true)
///     .build();
///
/// let docs = retriever._get_relevant_documents("quantum computing", None)
///     .await
///     .unwrap();
///
/// for doc in docs {
///     println!("Title: {}", doc.metadata.get("Title").unwrap());
///     println!("Content: {}", doc.page_content);
/// }
/// # });
/// ```
#[derive(Debug, Clone)]
pub struct ArxivRetriever {
    /// Internal search tool
    tool: ArxivSearchTool,
    /// Whether to return full document text or just summaries (default: false)
    get_full_documents: bool,
}

impl ArxivRetriever {
    /// Create a new `ArxivRetriever` with default settings
    ///
    /// Default settings:
    /// - `max_results`: 3
    /// - `get_full_documents`: false (summaries only)
    /// - `sort_by`: Relevance
    /// - `sort_order`: Descending
    #[must_use]
    pub fn new() -> Self {
        Self {
            tool: ArxivSearchTool::new(),
            get_full_documents: false,
        }
    }

    /// Create a builder for `ArxivRetriever`
    #[must_use]
    pub fn builder() -> ArxivRetrieverBuilder {
        ArxivRetrieverBuilder::default()
    }

    /// Convert `ArxivPaper` to Document
    fn paper_to_document(paper: &ArxivPaper, get_full_documents: bool) -> Document {
        let mut metadata = HashMap::new();
        metadata.insert(
            "Entry ID".to_string(),
            serde_json::Value::String(paper.id.clone()),
        );
        metadata.insert(
            "Title".to_string(),
            serde_json::Value::String(paper.title.clone()),
        );
        metadata.insert(
            "Authors".to_string(),
            serde_json::Value::String(paper.authors.join(", ")),
        );
        metadata.insert(
            "Published".to_string(),
            serde_json::Value::String(paper.published.clone()),
        );
        metadata.insert(
            "PDF URL".to_string(),
            serde_json::Value::String(paper.pdf_url.clone()),
        );
        metadata.insert(
            "Categories".to_string(),
            serde_json::Value::String(paper.categories.join(", ")),
        );

        let page_content = if get_full_documents {
            // Full document includes title, authors, and summary
            format!(
                "Title: {}\nAuthors: {}\nPublished: {}\n\n{}",
                paper.title.trim(),
                paper.authors.join(", "),
                paper.published,
                paper.summary.trim()
            )
        } else {
            // Just the summary
            paper.summary.trim().to_string()
        };

        Document {
            page_content,
            metadata,
            id: Some(paper.id.clone()),
        }
    }

    /// Get papers from arXiv API
    async fn get_papers(&self, query: &str) -> Result<Vec<ArxivPaper>> {
        let client = reqwest::Client::builder()
            .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
            .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
            .build()
            .map_err(|e| {
                dashflow::core::Error::http(format!("Failed to create HTTP client: {e}"))
            })?;

        let url = format!(
            "https://export.arxiv.org/api/query?search_query=all:{}&start=0&max_results={}&sortBy={}&sortOrder={}",
            urlencoding::encode(query),
            self.tool.max_results,
            self.tool.sort_by.as_str(),
            self.tool.sort_order.as_str()
        );

        let response =
            client.get(&url).send().await.map_err(|e| {
                dashflow::core::Error::http(format!("Failed to query arXiv API: {e}"))
            })?;

        let xml_text = response.text().await.map_err(|e| {
            dashflow::core::Error::http(format!("Failed to read arXiv response: {e}"))
        })?;

        ArxivSearchTool::parse_arxiv_response(&xml_text)
    }
}

impl Default for ArxivRetriever {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Retriever for ArxivRetriever {
    async fn _get_relevant_documents(
        &self,
        query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        let papers = self.get_papers(query).await?;

        Ok(papers
            .iter()
            .map(|paper| Self::paper_to_document(paper, self.get_full_documents))
            .collect())
    }
}

/// Builder for `ArxivRetriever`
#[derive(Debug, Clone, Default)]
pub struct ArxivRetrieverBuilder {
    max_results: Option<usize>,
    get_full_documents: Option<bool>,
    sort_by: Option<SortBy>,
    sort_order: Option<SortOrder>,
}

impl ArxivRetrieverBuilder {
    /// Set the maximum number of documents to retrieve
    ///
    /// Default: 3
    #[must_use]
    pub fn max_results(mut self, max_results: usize) -> Self {
        self.max_results = Some(max_results);
        self
    }

    /// Set whether to return full documents or just summaries
    ///
    /// Default: false (summaries only)
    #[must_use]
    pub fn get_full_documents(mut self, get_full_documents: bool) -> Self {
        self.get_full_documents = Some(get_full_documents);
        self
    }

    /// Set the sort criteria
    ///
    /// Default: Relevance
    #[must_use]
    pub fn sort_by(mut self, sort_by: SortBy) -> Self {
        self.sort_by = Some(sort_by);
        self
    }

    /// Set the sort order
    ///
    /// Default: Descending
    #[must_use]
    pub fn sort_order(mut self, sort_order: SortOrder) -> Self {
        self.sort_order = Some(sort_order);
        self
    }

    /// Build the `ArxivRetriever`
    #[must_use]
    pub fn build(self) -> ArxivRetriever {
        let tool = ArxivSearchTool {
            max_results: self.max_results.unwrap_or(3),
            sort_by: self.sort_by.unwrap_or_default(),
            sort_order: self.sort_order.unwrap_or_default(),
        };

        ArxivRetriever {
            tool,
            get_full_documents: self.get_full_documents.unwrap_or(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =======================================================================
    // ArxivSearchTool Creation Tests
    // =======================================================================

    #[test]
    fn test_arxiv_tool_creation() {
        let arxiv = ArxivSearchTool::new();
        assert_eq!(arxiv.name(), "arxiv_search");
        assert!(arxiv.description().contains("arXiv"));
        assert_eq!(arxiv.max_results, 3);
    }

    #[test]
    fn test_arxiv_tool_default_sort_by() {
        let arxiv = ArxivSearchTool::new();
        assert!(matches!(arxiv.sort_by, SortBy::Relevance));
    }

    #[test]
    fn test_arxiv_tool_default_sort_order() {
        let arxiv = ArxivSearchTool::new();
        assert!(matches!(arxiv.sort_order, SortOrder::Descending));
    }

    #[test]
    fn test_arxiv_tool_description_contains_fields() {
        let arxiv = ArxivSearchTool::new();
        let desc = arxiv.description();
        assert!(desc.contains("physics"));
        assert!(desc.contains("mathematics"));
        assert!(desc.contains("computer science"));
        assert!(desc.contains("PDF"));
    }

    #[test]
    fn test_arxiv_tool_name_is_static() {
        let arxiv1 = ArxivSearchTool::new();
        let arxiv2 = ArxivSearchTool::builder().max_results(10).build();
        assert_eq!(arxiv1.name(), arxiv2.name());
    }

    // =======================================================================
    // ArxivSearchTool Builder Tests
    // =======================================================================

    #[test]
    fn test_arxiv_tool_builder() {
        let arxiv = ArxivSearchTool::builder()
            .max_results(5)
            .sort_by(SortBy::SubmittedDate)
            .sort_order(SortOrder::Ascending)
            .build();

        assert_eq!(arxiv.max_results, 5);
    }

    #[test]
    fn test_arxiv_tool_builder_default() {
        let arxiv = ArxivSearchToolBuilder::default().build();
        assert_eq!(arxiv.max_results, 3);
        assert!(matches!(arxiv.sort_by, SortBy::Relevance));
        assert!(matches!(arxiv.sort_order, SortOrder::Descending));
    }

    #[test]
    fn test_arxiv_tool_builder_max_results_only() {
        let arxiv = ArxivSearchTool::builder().max_results(10).build();
        assert_eq!(arxiv.max_results, 10);
        assert!(matches!(arxiv.sort_by, SortBy::Relevance));
    }

    #[test]
    fn test_arxiv_tool_builder_sort_by_only() {
        let arxiv = ArxivSearchTool::builder()
            .sort_by(SortBy::LastUpdatedDate)
            .build();
        assert_eq!(arxiv.max_results, 3);
        assert!(matches!(arxiv.sort_by, SortBy::LastUpdatedDate));
    }

    #[test]
    fn test_arxiv_tool_builder_sort_order_only() {
        let arxiv = ArxivSearchTool::builder()
            .sort_order(SortOrder::Ascending)
            .build();
        assert!(matches!(arxiv.sort_order, SortOrder::Ascending));
    }

    #[test]
    fn test_arxiv_tool_builder_chaining() {
        let arxiv = ArxivSearchTool::builder()
            .max_results(1)
            .sort_by(SortBy::SubmittedDate)
            .sort_order(SortOrder::Ascending)
            .max_results(7) // Override
            .build();
        assert_eq!(arxiv.max_results, 7);
    }

    #[test]
    fn test_arxiv_tool_builder_zero_max_results() {
        let arxiv = ArxivSearchTool::builder().max_results(0).build();
        assert_eq!(arxiv.max_results, 0);
    }

    #[test]
    fn test_arxiv_tool_builder_large_max_results() {
        let arxiv = ArxivSearchTool::builder().max_results(1000).build();
        assert_eq!(arxiv.max_results, 1000);
    }

    // =======================================================================
    // Schema Tests
    // =======================================================================

    #[test]
    fn test_arxiv_args_schema() {
        let arxiv = ArxivSearchTool::new();
        let schema = arxiv.args_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
        assert_eq!(schema["required"][0], "query");
    }

    #[test]
    fn test_arxiv_args_schema_query_type() {
        let arxiv = ArxivSearchTool::new();
        let schema = arxiv.args_schema();
        assert_eq!(schema["properties"]["query"]["type"], "string");
    }

    #[test]
    fn test_arxiv_args_schema_query_description() {
        let arxiv = ArxivSearchTool::new();
        let schema = arxiv.args_schema();
        assert!(schema["properties"]["query"]["description"]
            .as_str()
            .unwrap()
            .contains("search query"));
    }

    #[test]
    fn test_arxiv_args_schema_is_object() {
        let arxiv = ArxivSearchTool::new();
        let schema = arxiv.args_schema();
        assert!(schema.is_object());
    }

    // =======================================================================
    // Default Trait Tests
    // =======================================================================

    #[test]
    fn test_default() {
        let arxiv = ArxivSearchTool::default();
        assert_eq!(arxiv.max_results, 3);
    }

    #[test]
    fn test_default_equals_new() {
        let default = ArxivSearchTool::default();
        let new = ArxivSearchTool::new();
        assert_eq!(default.max_results, new.max_results);
    }

    // =======================================================================
    // SortBy Enum Tests
    // =======================================================================

    #[test]
    fn test_sort_by_string() {
        assert_eq!(SortBy::Relevance.as_str(), "relevance");
        assert_eq!(SortBy::LastUpdatedDate.as_str(), "lastUpdatedDate");
        assert_eq!(SortBy::SubmittedDate.as_str(), "submittedDate");
    }

    #[test]
    fn test_sort_by_default() {
        let sort_by = SortBy::default();
        assert!(matches!(sort_by, SortBy::Relevance));
    }

    #[test]
    fn test_sort_by_clone() {
        let sort_by = SortBy::LastUpdatedDate;
        let cloned = sort_by;
        assert_eq!(sort_by.as_str(), cloned.as_str());
    }

    #[test]
    fn test_sort_by_debug() {
        let sort_by = SortBy::SubmittedDate;
        let debug_str = format!("{:?}", sort_by);
        assert!(debug_str.contains("SubmittedDate"));
    }

    // =======================================================================
    // SortOrder Enum Tests
    // =======================================================================

    #[test]
    fn test_sort_order_string() {
        assert_eq!(SortOrder::Ascending.as_str(), "ascending");
        assert_eq!(SortOrder::Descending.as_str(), "descending");
    }

    #[test]
    fn test_sort_order_default() {
        let sort_order = SortOrder::default();
        assert!(matches!(sort_order, SortOrder::Descending));
    }

    #[test]
    fn test_sort_order_clone() {
        let sort_order = SortOrder::Ascending;
        let cloned = sort_order;
        assert_eq!(sort_order.as_str(), cloned.as_str());
    }

    #[test]
    fn test_sort_order_debug() {
        let sort_order = SortOrder::Ascending;
        let debug_str = format!("{:?}", sort_order);
        assert!(debug_str.contains("Ascending"));
    }

    // =======================================================================
    // ArxivPaper Struct Tests
    // =======================================================================

    #[test]
    fn test_arxiv_paper_creation() {
        let paper = ArxivPaper {
            id: "2301.00001".to_string(),
            title: "Test Paper".to_string(),
            authors: vec!["Author One".to_string(), "Author Two".to_string()],
            summary: "This is a test summary.".to_string(),
            published: "2023-01-01".to_string(),
            pdf_url: "https://arxiv.org/pdf/2301.00001.pdf".to_string(),
            categories: vec!["cs.AI".to_string(), "cs.LG".to_string()],
        };
        assert_eq!(paper.id, "2301.00001");
        assert_eq!(paper.title, "Test Paper");
        assert_eq!(paper.authors.len(), 2);
    }

    #[test]
    fn test_arxiv_paper_serialization() {
        let paper = ArxivPaper {
            id: "2301.00001".to_string(),
            title: "Test Paper".to_string(),
            authors: vec!["Author".to_string()],
            summary: "Summary".to_string(),
            published: "2023-01-01".to_string(),
            pdf_url: "https://arxiv.org/pdf/2301.00001.pdf".to_string(),
            categories: vec!["cs.AI".to_string()],
        };
        let json = serde_json::to_string(&paper).unwrap();
        assert!(json.contains("2301.00001"));
        assert!(json.contains("Test Paper"));
    }

    #[test]
    fn test_arxiv_paper_deserialization() {
        let json = r#"{
            "id": "2301.00001",
            "title": "Test Paper",
            "authors": ["Author One"],
            "summary": "Summary text",
            "published": "2023-01-01",
            "pdf_url": "https://arxiv.org/pdf/2301.00001.pdf",
            "categories": ["cs.AI"]
        }"#;
        let paper: ArxivPaper = serde_json::from_str(json).unwrap();
        assert_eq!(paper.id, "2301.00001");
        assert_eq!(paper.title, "Test Paper");
    }

    #[test]
    fn test_arxiv_paper_empty_authors() {
        let paper = ArxivPaper {
            id: "id".to_string(),
            title: "title".to_string(),
            authors: vec![],
            summary: "summary".to_string(),
            published: "date".to_string(),
            pdf_url: "url".to_string(),
            categories: vec![],
        };
        assert!(paper.authors.is_empty());
    }

    #[test]
    fn test_arxiv_paper_clone() {
        let paper = ArxivPaper {
            id: "id".to_string(),
            title: "title".to_string(),
            authors: vec!["author".to_string()],
            summary: "summary".to_string(),
            published: "date".to_string(),
            pdf_url: "url".to_string(),
            categories: vec!["cat".to_string()],
        };
        let cloned = paper.clone();
        assert_eq!(paper.id, cloned.id);
        assert_eq!(paper.title, cloned.title);
    }

    #[test]
    fn test_arxiv_paper_debug() {
        let paper = ArxivPaper {
            id: "id".to_string(),
            title: "title".to_string(),
            authors: vec![],
            summary: "summary".to_string(),
            published: "date".to_string(),
            pdf_url: "url".to_string(),
            categories: vec![],
        };
        let debug_str = format!("{:?}", paper);
        assert!(debug_str.contains("ArxivPaper"));
    }

    // =======================================================================
    // XML Parsing Tests
    // =======================================================================

    #[test]
    fn test_parse_arxiv_response_single_entry() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
            <entry>
                <id>http://arxiv.org/abs/2301.00001v1</id>
                <title>Test Paper Title</title>
                <summary>This is the abstract of the paper.</summary>
                <published>2023-01-01T00:00:00Z</published>
                <author><name>John Doe</name></author>
                <link title="pdf" href="http://arxiv.org/pdf/2301.00001v1"/>
                <category term="cs.AI"/>
            </entry>
        </feed>"#;

        let papers = ArxivSearchTool::parse_arxiv_response(xml).unwrap();
        assert_eq!(papers.len(), 1);
        assert!(papers[0].id.contains("2301.00001"));
        assert_eq!(papers[0].title, "Test Paper Title");
    }

    #[test]
    fn test_parse_arxiv_response_multiple_entries() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
            <entry>
                <id>http://arxiv.org/abs/2301.00001v1</id>
                <title>First Paper</title>
                <summary>First summary.</summary>
                <published>2023-01-01T00:00:00Z</published>
                <author><name>Author One</name></author>
            </entry>
            <entry>
                <id>http://arxiv.org/abs/2301.00002v1</id>
                <title>Second Paper</title>
                <summary>Second summary.</summary>
                <published>2023-01-02T00:00:00Z</published>
                <author><name>Author Two</name></author>
            </entry>
        </feed>"#;

        let papers = ArxivSearchTool::parse_arxiv_response(xml).unwrap();
        assert_eq!(papers.len(), 2);
        assert_eq!(papers[0].title, "First Paper");
        assert_eq!(papers[1].title, "Second Paper");
    }

    #[test]
    fn test_parse_arxiv_response_multiple_authors() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
            <entry>
                <id>http://arxiv.org/abs/2301.00001v1</id>
                <title>Multi-Author Paper</title>
                <summary>Summary.</summary>
                <published>2023-01-01T00:00:00Z</published>
                <author><name>Alice</name></author>
                <author><name>Bob</name></author>
                <author><name>Charlie</name></author>
            </entry>
        </feed>"#;

        let papers = ArxivSearchTool::parse_arxiv_response(xml).unwrap();
        assert_eq!(papers.len(), 1);
        assert_eq!(papers[0].authors.len(), 3);
        assert!(papers[0].authors.contains(&"Alice".to_string()));
        assert!(papers[0].authors.contains(&"Bob".to_string()));
        assert!(papers[0].authors.contains(&"Charlie".to_string()));
    }

    #[test]
    fn test_parse_arxiv_response_multiple_categories() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
            <entry>
                <id>http://arxiv.org/abs/2301.00001v1</id>
                <title>Multi-Category Paper</title>
                <summary>Summary.</summary>
                <published>2023-01-01T00:00:00Z</published>
                <author><name>Author</name></author>
                <category term="cs.AI"/>
                <category term="cs.LG"/>
                <category term="stat.ML"/>
            </entry>
        </feed>"#;

        let papers = ArxivSearchTool::parse_arxiv_response(xml).unwrap();
        assert_eq!(papers.len(), 1);
        assert_eq!(papers[0].categories.len(), 3);
        assert!(papers[0].categories.contains(&"cs.AI".to_string()));
        assert!(papers[0].categories.contains(&"cs.LG".to_string()));
        assert!(papers[0].categories.contains(&"stat.ML".to_string()));
    }

    #[test]
    fn test_parse_arxiv_response_empty_feed() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
        </feed>"#;

        let papers = ArxivSearchTool::parse_arxiv_response(xml).unwrap();
        assert!(papers.is_empty());
    }

    #[test]
    fn test_parse_arxiv_response_with_pdf_link() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
            <entry>
                <id>http://arxiv.org/abs/2301.00001v1</id>
                <title>Paper with PDF</title>
                <summary>Summary.</summary>
                <published>2023-01-01T00:00:00Z</published>
                <author><name>Author</name></author>
                <link title="pdf" href="http://arxiv.org/pdf/2301.00001v1"/>
            </entry>
        </feed>"#;

        let papers = ArxivSearchTool::parse_arxiv_response(xml).unwrap();
        assert_eq!(papers.len(), 1);
        assert_eq!(papers[0].pdf_url, "http://arxiv.org/pdf/2301.00001v1");
    }

    #[test]
    fn test_parse_arxiv_response_whitespace_handling() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
            <entry>
                <id>http://arxiv.org/abs/2301.00001v1</id>
                <title>   Paper Title with Whitespace   </title>
                <summary>
                    Summary with
                    multiple lines
                    and whitespace.
                </summary>
                <published>2023-01-01T00:00:00Z</published>
                <author><name>Author</name></author>
            </entry>
        </feed>"#;

        let papers = ArxivSearchTool::parse_arxiv_response(xml).unwrap();
        assert_eq!(papers.len(), 1);
        // Note: XML parser trims text, so whitespace is handled
    }

    #[test]
    fn test_parse_arxiv_response_special_characters() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
            <entry>
                <id>http://arxiv.org/abs/2301.00001v1</id>
                <title>Paper with &amp; special &lt;characters&gt;</title>
                <summary>Summary with "quotes" and 'apostrophes'.</summary>
                <published>2023-01-01T00:00:00Z</published>
                <author><name>O'Brien</name></author>
            </entry>
        </feed>"#;

        let papers = ArxivSearchTool::parse_arxiv_response(xml).unwrap();
        assert_eq!(papers.len(), 1);
        assert!(papers[0].title.contains("&"));
        assert!(papers[0].title.contains("<"));
        assert!(papers[0].title.contains(">"));
    }

    #[test]
    fn test_parse_arxiv_response_unicode() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
            <entry>
                <id>http://arxiv.org/abs/2301.00001v1</id>
                <title>论文标题 αβγ δε</title>
                <summary>中文摘要 résumé français</summary>
                <published>2023-01-01T00:00:00Z</published>
                <author><name>李明</name></author>
            </entry>
        </feed>"#;

        let papers = ArxivSearchTool::parse_arxiv_response(xml).unwrap();
        assert_eq!(papers.len(), 1);
        assert!(papers[0].title.contains("论文"));
        assert!(papers[0].authors[0].contains("李明"));
    }

    // =======================================================================
    // Paper to Document Conversion Tests
    // =======================================================================

    #[test]
    fn test_paper_to_document_summary_mode() {
        let paper = ArxivPaper {
            id: "2301.00001".to_string(),
            title: "Test Paper".to_string(),
            authors: vec!["Author One".to_string()],
            summary: "This is the paper summary.".to_string(),
            published: "2023-01-01".to_string(),
            pdf_url: "https://arxiv.org/pdf/2301.00001.pdf".to_string(),
            categories: vec!["cs.AI".to_string()],
        };

        let doc = ArxivRetriever::paper_to_document(&paper, false);
        assert_eq!(doc.page_content, "This is the paper summary.");
        assert!(!doc.page_content.contains("Title:"));
    }

    #[test]
    fn test_paper_to_document_full_mode() {
        let paper = ArxivPaper {
            id: "2301.00001".to_string(),
            title: "Test Paper".to_string(),
            authors: vec!["Author One".to_string()],
            summary: "This is the paper summary.".to_string(),
            published: "2023-01-01".to_string(),
            pdf_url: "https://arxiv.org/pdf/2301.00001.pdf".to_string(),
            categories: vec!["cs.AI".to_string()],
        };

        let doc = ArxivRetriever::paper_to_document(&paper, true);
        assert!(doc.page_content.contains("Title:"));
        assert!(doc.page_content.contains("Authors:"));
        assert!(doc.page_content.contains("Published:"));
        assert!(doc.page_content.contains("This is the paper summary."));
    }

    #[test]
    fn test_paper_to_document_metadata() {
        let paper = ArxivPaper {
            id: "2301.00001".to_string(),
            title: "Test Paper".to_string(),
            authors: vec!["Alice".to_string(), "Bob".to_string()],
            summary: "Summary".to_string(),
            published: "2023-01-01".to_string(),
            pdf_url: "https://arxiv.org/pdf/2301.00001.pdf".to_string(),
            categories: vec!["cs.AI".to_string(), "cs.LG".to_string()],
        };

        let doc = ArxivRetriever::paper_to_document(&paper, false);
        assert!(doc.metadata.contains_key("Entry ID"));
        assert!(doc.metadata.contains_key("Title"));
        assert!(doc.metadata.contains_key("Authors"));
        assert!(doc.metadata.contains_key("Published"));
        assert!(doc.metadata.contains_key("PDF URL"));
        assert!(doc.metadata.contains_key("Categories"));
    }

    #[test]
    fn test_paper_to_document_id() {
        let paper = ArxivPaper {
            id: "2301.00001".to_string(),
            title: "Test".to_string(),
            authors: vec![],
            summary: "Summary".to_string(),
            published: "2023-01-01".to_string(),
            pdf_url: "url".to_string(),
            categories: vec![],
        };

        let doc = ArxivRetriever::paper_to_document(&paper, false);
        assert_eq!(doc.id, Some("2301.00001".to_string()));
    }

    #[test]
    fn test_paper_to_document_authors_joined() {
        let paper = ArxivPaper {
            id: "id".to_string(),
            title: "title".to_string(),
            authors: vec!["Alice".to_string(), "Bob".to_string(), "Charlie".to_string()],
            summary: "summary".to_string(),
            published: "date".to_string(),
            pdf_url: "url".to_string(),
            categories: vec![],
        };

        let doc = ArxivRetriever::paper_to_document(&paper, false);
        let authors_value = doc.metadata.get("Authors").unwrap();
        assert_eq!(authors_value.as_str().unwrap(), "Alice, Bob, Charlie");
    }

    #[test]
    fn test_paper_to_document_categories_joined() {
        let paper = ArxivPaper {
            id: "id".to_string(),
            title: "title".to_string(),
            authors: vec![],
            summary: "summary".to_string(),
            published: "date".to_string(),
            pdf_url: "url".to_string(),
            categories: vec!["cs.AI".to_string(), "cs.LG".to_string()],
        };

        let doc = ArxivRetriever::paper_to_document(&paper, false);
        let categories_value = doc.metadata.get("Categories").unwrap();
        assert_eq!(categories_value.as_str().unwrap(), "cs.AI, cs.LG");
    }

    #[test]
    fn test_paper_to_document_trims_whitespace() {
        let paper = ArxivPaper {
            id: "id".to_string(),
            title: "  Title with spaces  ".to_string(),
            authors: vec![],
            summary: "  Summary with spaces  ".to_string(),
            published: "date".to_string(),
            pdf_url: "url".to_string(),
            categories: vec![],
        };

        let doc = ArxivRetriever::paper_to_document(&paper, false);
        assert_eq!(doc.page_content, "Summary with spaces");
    }

    // =======================================================================
    // ArxivRetriever Tests
    // =======================================================================

    #[test]
    fn test_arxiv_retriever_creation() {
        let retriever = ArxivRetriever::new();
        assert!(!retriever.get_full_documents);
        assert_eq!(retriever.tool.max_results, 3);
    }

    #[test]
    fn test_arxiv_retriever_builder() {
        let retriever = ArxivRetriever::builder()
            .max_results(5)
            .get_full_documents(true)
            .sort_by(SortBy::SubmittedDate)
            .build();

        assert!(retriever.get_full_documents);
        assert_eq!(retriever.tool.max_results, 5);
    }

    #[test]
    fn test_arxiv_retriever_default() {
        let retriever = ArxivRetriever::default();
        assert!(!retriever.get_full_documents);
        assert_eq!(retriever.tool.max_results, 3);
    }

    #[test]
    fn test_arxiv_retriever_builder_default() {
        let retriever = ArxivRetrieverBuilder::default().build();
        assert!(!retriever.get_full_documents);
        assert_eq!(retriever.tool.max_results, 3);
    }

    #[test]
    fn test_arxiv_retriever_builder_all_options() {
        let retriever = ArxivRetriever::builder()
            .max_results(10)
            .get_full_documents(true)
            .sort_by(SortBy::LastUpdatedDate)
            .sort_order(SortOrder::Ascending)
            .build();

        assert!(retriever.get_full_documents);
        assert_eq!(retriever.tool.max_results, 10);
        assert!(matches!(retriever.tool.sort_by, SortBy::LastUpdatedDate));
        assert!(matches!(retriever.tool.sort_order, SortOrder::Ascending));
    }

    #[test]
    fn test_arxiv_retriever_builder_chaining() {
        let retriever = ArxivRetriever::builder()
            .max_results(1)
            .get_full_documents(false)
            .max_results(5) // Override
            .get_full_documents(true) // Override
            .build();

        assert!(retriever.get_full_documents);
        assert_eq!(retriever.tool.max_results, 5);
    }

    #[test]
    fn test_arxiv_retriever_clone() {
        let retriever = ArxivRetriever::builder()
            .max_results(5)
            .get_full_documents(true)
            .build();

        let cloned = retriever.clone();
        assert_eq!(retriever.tool.max_results, cloned.tool.max_results);
        assert_eq!(retriever.get_full_documents, cloned.get_full_documents);
    }

    #[test]
    fn test_arxiv_retriever_debug() {
        let retriever = ArxivRetriever::new();
        let debug_str = format!("{:?}", retriever);
        assert!(debug_str.contains("ArxivRetriever"));
    }

    // =======================================================================
    // Tool Clone and Debug Tests
    // =======================================================================

    #[test]
    fn test_arxiv_tool_clone() {
        let arxiv = ArxivSearchTool::builder()
            .max_results(7)
            .sort_by(SortBy::SubmittedDate)
            .build();

        let cloned = arxiv.clone();
        assert_eq!(arxiv.max_results, cloned.max_results);
    }

    #[test]
    fn test_arxiv_tool_debug() {
        let arxiv = ArxivSearchTool::new();
        let debug_str = format!("{:?}", arxiv);
        assert!(debug_str.contains("ArxivSearchTool"));
    }

    // =======================================================================
    // Builder Clone and Debug Tests
    // =======================================================================

    #[test]
    fn test_arxiv_tool_builder_clone() {
        let builder = ArxivSearchTool::builder().max_results(5);
        let cloned = builder.clone();
        let tool1 = builder.build();
        let tool2 = cloned.build();
        assert_eq!(tool1.max_results, tool2.max_results);
    }

    #[test]
    fn test_arxiv_tool_builder_debug() {
        let builder = ArxivSearchTool::builder().max_results(5);
        let debug_str = format!("{:?}", builder);
        assert!(debug_str.contains("ArxivSearchToolBuilder"));
    }

    #[test]
    fn test_arxiv_retriever_builder_clone() {
        let builder = ArxivRetriever::builder()
            .max_results(5)
            .get_full_documents(true);
        let cloned = builder.clone();
        let ret1 = builder.build();
        let ret2 = cloned.build();
        assert_eq!(ret1.tool.max_results, ret2.tool.max_results);
        assert_eq!(ret1.get_full_documents, ret2.get_full_documents);
    }

    #[test]
    fn test_arxiv_retriever_builder_debug() {
        let builder = ArxivRetriever::builder().max_results(5);
        let debug_str = format!("{:?}", builder);
        assert!(debug_str.contains("ArxivRetrieverBuilder"));
    }

    // =======================================================================
    // ToolInput Handling Tests
    // =======================================================================

    #[test]
    fn test_tool_input_structured_query_extraction() {
        // Test that structured input with query field can be parsed
        let structured = serde_json::json!({
            "query": "test query"
        });
        let query = structured
            .get("query")
            .and_then(|q| q.as_str())
            .map(|s| s.to_string());
        assert_eq!(query, Some("test query".to_string()));
    }

    #[test]
    fn test_tool_input_structured_missing_query() {
        // Test that structured input without query field returns None
        let structured = serde_json::json!({
            "other_field": "value"
        });
        let query = structured.get("query").and_then(|q| q.as_str());
        assert!(query.is_none());
    }

    #[test]
    fn test_tool_input_structured_query_not_string() {
        // Test that structured input with non-string query returns None from as_str
        let structured = serde_json::json!({
            "query": 12345
        });
        let query = structured.get("query").and_then(|q| q.as_str());
        assert!(query.is_none());
    }

    #[tokio::test]
    async fn test_tool_call_structured_missing_query_returns_error() {
        let arxiv = ArxivSearchTool::new();
        let structured = serde_json::json!({
            "other_field": "value"
        });
        let input = ToolInput::Structured(structured);
        let result = arxiv._call(input).await;

        // Should fail with missing query error
        assert!(result.is_err());
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(err_msg.contains("Missing 'query' field"));
    }

    #[tokio::test]
    async fn test_tool_call_structured_query_not_string_returns_error() {
        let arxiv = ArxivSearchTool::new();
        let structured = serde_json::json!({
            "query": 12345
        });
        let input = ToolInput::Structured(structured);
        let result = arxiv._call(input).await;

        // Should fail because query is not a string
        assert!(result.is_err());
    }

    // =======================================================================
    // Edge Case Tests
    // =======================================================================

    #[test]
    fn test_parse_empty_xml() {
        let xml = "";
        let result = ArxivSearchTool::parse_arxiv_response(xml);
        // Empty XML should parse to empty results
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_parse_minimal_valid_xml() {
        let xml = r#"<?xml version="1.0"?><feed></feed>"#;
        let result = ArxivSearchTool::parse_arxiv_response(xml);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_parse_entry_with_minimal_fields() {
        let xml = r#"<?xml version="1.0"?>
        <feed>
            <entry>
                <id>id</id>
                <title>title</title>
                <summary>summary</summary>
                <published>date</published>
            </entry>
        </feed>"#;

        let papers = ArxivSearchTool::parse_arxiv_response(xml).unwrap();
        assert_eq!(papers.len(), 1);
        assert_eq!(papers[0].id, "id");
        assert_eq!(papers[0].title, "title");
        assert_eq!(papers[0].summary, "summary");
        assert!(papers[0].authors.is_empty());
        assert!(papers[0].categories.is_empty());
        assert!(papers[0].pdf_url.is_empty());
    }

    #[test]
    fn test_arxiv_paper_with_long_summary() {
        let long_summary = "A".repeat(10000);
        let paper = ArxivPaper {
            id: "id".to_string(),
            title: "title".to_string(),
            authors: vec![],
            summary: long_summary.clone(),
            published: "date".to_string(),
            pdf_url: "url".to_string(),
            categories: vec![],
        };

        let doc = ArxivRetriever::paper_to_document(&paper, false);
        assert_eq!(doc.page_content.len(), 10000);
    }

    #[test]
    fn test_arxiv_paper_with_many_authors() {
        let authors: Vec<String> = (0..100).map(|i| format!("Author {}", i)).collect();
        let paper = ArxivPaper {
            id: "id".to_string(),
            title: "title".to_string(),
            authors: authors.clone(),
            summary: "summary".to_string(),
            published: "date".to_string(),
            pdf_url: "url".to_string(),
            categories: vec![],
        };

        let doc = ArxivRetriever::paper_to_document(&paper, false);
        let authors_value = doc.metadata.get("Authors").unwrap().as_str().unwrap();
        assert!(authors_value.contains("Author 0"));
        assert!(authors_value.contains("Author 99"));
    }

    // =======================================================================
    // Integration tests (require network access)
    // =======================================================================

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_arxiv_search_quantum_computing() {
        let arxiv = ArxivSearchTool::new();
        let content = arxiv
            ._call_str("quantum computing".to_string())
            .await
            .expect("arXiv search failed");
        assert!(
            content.contains("Found"),
            "Expected 'Found' in response, got: {}",
            &content[..content.len().min(200)]
        );
        assert!(content.contains("Paper"));
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_arxiv_search_with_builder() {
        let arxiv = ArxivSearchTool::builder()
            .max_results(2)
            .sort_by(SortBy::SubmittedDate)
            .build();

        let content = arxiv
            ._call_str("machine learning".to_string())
            .await
            .expect("arXiv search failed");
        assert!(content.contains("machine learning"));
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_arxiv_no_results() {
        let arxiv = ArxivSearchTool::new();
        let content = arxiv
            ._call_str("xyzabc123impossiblequery999".to_string())
            .await
            .expect("arXiv search failed");
        assert!(content.contains("No papers found"));
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_arxiv_retriever_get_documents() {
        let retriever = ArxivRetriever::builder()
            .max_results(2)
            .get_full_documents(false)
            .build();

        let docs = retriever
            ._get_relevant_documents("quantum computing", None)
            .await
            .expect("arXiv retriever failed");

        assert!(
            !docs.is_empty(),
            "Expected at least one document (possible rate limiting)"
        );
        assert!(docs.len() <= 2);

        // Check document structure
        let first_doc = &docs[0];
        assert!(first_doc.metadata.contains_key("Title"));
        assert!(first_doc.metadata.contains_key("Authors"));
        assert!(first_doc.metadata.contains_key("Published"));
        assert!(first_doc.metadata.contains_key("PDF URL"));
        assert!(first_doc.metadata.contains_key("Entry ID"));
        assert!(first_doc.metadata.contains_key("Categories"));
        assert!(!first_doc.page_content.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_arxiv_retriever_full_documents() {
        let retriever = ArxivRetriever::builder()
            .max_results(1)
            .get_full_documents(true)
            .build();

        let docs = retriever
            ._get_relevant_documents("machine learning", None)
            .await
            .expect("arXiv retriever failed");

        assert!(
            !docs.is_empty(),
            "Expected at least one document (possible rate limiting)"
        );
        assert_eq!(docs.len(), 1);

        // Full documents should have Title, Authors, Published in content
        let content = &docs[0].page_content;
        assert!(content.contains("Title:"));
        assert!(content.contains("Authors:"));
        assert!(content.contains("Published:"));
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_arxiv_retriever_summary_only() {
        let retriever = ArxivRetriever::builder()
            .max_results(1)
            .get_full_documents(false)
            .build();

        let docs = retriever
            ._get_relevant_documents("neural networks", None)
            .await
            .expect("arXiv retriever failed");

        assert!(
            !docs.is_empty(),
            "Expected at least one document (possible rate limiting)"
        );
        assert_eq!(docs.len(), 1);

        // Summary only should NOT have "Title:" prefix
        let content = &docs[0].page_content;
        assert!(!content.contains("Title:"));
        assert!(!content.contains("Authors:"));
    }
}
