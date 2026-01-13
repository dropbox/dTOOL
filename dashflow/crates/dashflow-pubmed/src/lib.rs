//! PubMed/PMC Search Tool for `DashFlow` Rust
//!
//! This crate provides a tool for searching medical and scientific literature through
//! the PubMed/PMC database using the NCBI E-utilities API.
//!
//! # Overview
//!
//! `PubMed` is a free search engine accessing primarily the MEDLINE database of references
//! and abstracts on life sciences and biomedical topics. The tool uses two E-utilities:
//! - **`ESearch`**: Searches and retrieves a list of article UIDs matching a query
//! - **`EFetch`**: Fetches full article details (title, authors, abstract, etc.)
//!
//! # Features
//!
//! - Free API access (no authentication required)
//! - Search 35+ million citations from biomedical literature
//! - Configurable result count and sorting
//! - JSON response format
//! - Rate limiting guidelines (3 requests/sec without API key)
//!
//! # Example
//!
//! ```ignore
//! use dashflow_pubmed::PubMedSearch;
//! use dashflow::core::tools::Tool;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let tool = PubMedSearch::new()
//!     .max_results(5)
//!     .build();
//!
//! let results = tool.run("CRISPR gene editing", None).await?;
//! println!("Search results:\n{}", results);
//! # Ok(())
//! # }
//! ```
//!
//! # API Endpoints
//!
//! - **`ESearch`**: `https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi`
//! - **`EFetch`**: `https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi`
//!
//! # Rate Limits
//!
//! - Without API key: 3 requests/second
//! - With API key: 10 requests/second
//! - Bulk/systematic retrieval prohibited
//!
//! # References
//!
//! - [NCBI E-utilities Documentation](https://www.ncbi.nlm.nih.gov/books/NBK25501/)
//! - [PubMed](https://pubmed.ncbi.nlm.nih.gov/)
//! - [PMC (PubMed Central)](https://pmc.ncbi.nlm.nih.gov/)

use async_trait::async_trait;
use dashflow::core::config::RunnableConfig;
use dashflow::core::documents::Document;
use dashflow::core::error::{Error, Result};
use dashflow::core::retrievers::Retriever;
use dashflow::core::tools::{Tool, ToolInput};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

const ESEARCH_BASE_URL: &str = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi";
const EFETCH_BASE_URL: &str = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi";

/// PubMed/PMC search tool for medical and scientific literature
///
/// # Example
///
/// ```ignore
/// use dashflow_pubmed::PubMedSearch;
/// use dashflow::core::tools::Tool;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let tool = PubMedSearch::builder()
///     .max_results(10)
///     .sort_by("relevance")
///     .build();
///
/// let results = tool.run("immunotherapy cancer treatment", None).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct PubMedSearch {
    /// HTTP client
    client: Client,
    /// Maximum number of results to return (default: 5, max: 10,000)
    max_results: usize,
    /// Database to search (default: "pubmed")
    database: String,
    /// Sort order: "relevance", "`pub_date`", "author", "journal"
    sort_by: Option<String>,
    /// API key for higher rate limits (optional)
    api_key: Option<String>,
}

impl PubMedSearch {
    /// Create a new `PubMedSearch` tool with default settings
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_pubmed::PubMedSearch;
    ///
    /// let tool = PubMedSearch::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            max_results: 3,
            database: "pubmed".to_string(),
            sort_by: None,
            api_key: None,
        }
    }

    /// Create a builder for `PubMedSearch`
    #[must_use]
    pub fn builder() -> Self {
        Self::new()
    }

    /// Set the maximum number of results (default: 5, max: 10,000)
    #[must_use]
    pub fn max_results(mut self, max: usize) -> Self {
        self.max_results = max.min(10000);
        self
    }

    /// Set the database to search (default: "pubmed")
    ///
    /// Other databases: "pmc", "pubmedhealth", "mesh", etc.
    pub fn database(mut self, db: impl Into<String>) -> Self {
        self.database = db.into();
        self
    }

    /// Set the sort order
    ///
    /// Options: "relevance", "`pub_date`", "author", "journal"
    pub fn sort_by(mut self, sort: impl Into<String>) -> Self {
        self.sort_by = Some(sort.into());
        self
    }

    /// Set API key for higher rate limits (10 req/sec vs 3 req/sec)
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Build the tool (for consistency with builder pattern)
    #[must_use]
    pub fn build(self) -> Self {
        self
    }

    /// Execute a search query
    async fn search(&self, query: &str) -> Result<Vec<PubMedArticle>> {
        // Step 1: ESearch to get article IDs
        let max_results_str = self.max_results.to_string();
        let mut params = vec![
            ("db", self.database.as_str()),
            ("term", query),
            ("retmax", max_results_str.as_str()),
            ("retmode", "json"),
        ];

        if let Some(sort) = &self.sort_by {
            params.push(("sort", sort.as_str()));
        }

        if let Some(api_key) = &self.api_key {
            params.push(("api_key", api_key.as_str()));
        }

        let esearch_response = self
            .client
            .get(ESEARCH_BASE_URL)
            .query(&params)
            .send()
            .await
            .map_err(|e| Error::tool_error(format!("ESearch request failed: {e}")))?;

        let esearch_json: ESearchResponse = esearch_response
            .json()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to parse ESearch response: {e}")))?;

        let id_list = esearch_json
            .esearchresult
            .idlist
            .iter()
            .map(std::string::String::as_str)
            .collect::<Vec<_>>();

        if id_list.is_empty() {
            return Ok(vec![]);
        }

        // Step 2: EFetch to get article details
        let ids = id_list.join(",");
        let mut efetch_params = vec![
            ("db", self.database.as_str()),
            ("id", ids.as_str()),
            ("retmode", "xml"),
            ("rettype", "abstract"),
        ];

        if let Some(api_key) = &self.api_key {
            efetch_params.push(("api_key", api_key.as_str()));
        }

        let efetch_response = self
            .client
            .get(EFETCH_BASE_URL)
            .query(&efetch_params)
            .send()
            .await
            .map_err(|e| Error::tool_error(format!("EFetch request failed: {e}")))?;

        let xml_text = efetch_response
            .text()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to get EFetch response text: {e}")))?;

        // Parse XML to extract article information
        let articles = parse_pubmed_xml(&xml_text, &id_list)?;

        Ok(articles)
    }

    /// Format search results as a string
    fn format_results(&self, articles: &[PubMedArticle]) -> String {
        if articles.is_empty() {
            return "No results found.".to_string();
        }

        let mut output = format!("Found {} article(s):\n\n", articles.len());

        for (i, article) in articles.iter().enumerate() {
            output.push_str(&format!("{}. {}\n", i + 1, article.title));
            output.push_str(&format!("   PMID: {}\n", article.pmid));

            if !article.authors.is_empty() {
                let authors = if article.authors.len() > 3 {
                    format!("{} et al.", article.authors[0])
                } else {
                    article.authors.join(", ")
                };
                output.push_str(&format!("   Authors: {authors}\n"));
            }

            if let Some(journal) = &article.journal {
                output.push_str(&format!("   Journal: {journal}\n"));
            }

            if let Some(pub_date) = &article.pub_date {
                output.push_str(&format!("   Published: {pub_date}\n"));
            }

            if let Some(abstract_text) = &article.abstract_text {
                let truncated = if abstract_text.len() > 300 {
                    format!("{}...", &abstract_text[..300])
                } else {
                    abstract_text.clone()
                };
                output.push_str(&format!("   Abstract: {truncated}\n"));
            }

            output.push_str(&format!(
                "   Link: https://pubmed.ncbi.nlm.nih.gov/{}/\n\n",
                article.pmid
            ));
        }

        output
    }

    /// Load documents from `PubMed` search
    ///
    /// This method converts `PubMed` articles into Document objects suitable for retrieval.
    /// Each document contains the abstract as `page_content` and metadata with article details.
    ///
    /// # Python Baseline
    ///
    /// Equivalent to `PubMedAPIWrapper.load_docs()` from:
    /// `~/dashflow_community/dashflow_community/utilities/pubmed.py`
    pub async fn load_docs(&self, query: &str) -> Result<Vec<Document>> {
        let articles = self.search(query).await?;

        Ok(articles
            .iter()
            .map(|article| {
                let mut metadata = HashMap::new();

                metadata.insert(
                    "uid".to_string(),
                    serde_json::Value::String(article.pmid.clone()),
                );
                metadata.insert(
                    "Title".to_string(),
                    serde_json::Value::String(article.title.clone()),
                );

                if let Some(pub_date) = &article.pub_date {
                    metadata.insert(
                        "Published".to_string(),
                        serde_json::Value::String(pub_date.clone()),
                    );
                }

                if let Some(journal) = &article.journal {
                    metadata.insert(
                        "Journal".to_string(),
                        serde_json::Value::String(journal.clone()),
                    );
                }

                if !article.authors.is_empty() {
                    metadata.insert(
                        "Authors".to_string(),
                        serde_json::Value::String(article.authors.join(", ")),
                    );
                }

                // Python uses "Summary" as page_content
                let page_content = article
                    .abstract_text
                    .clone()
                    .unwrap_or_else(|| "No abstract available".to_string());

                Document {
                    page_content,
                    metadata,
                    id: Some(article.pmid.clone()),
                }
            })
            .collect())
    }
}

impl Default for PubMedSearch {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for PubMedSearch {
    fn name(&self) -> &'static str {
        "pubmed_search"
    }

    fn description(&self) -> &'static str {
        "Search PubMed/PMC for medical and scientific literature. \
         Input should be a search query string. Returns article titles, \
         authors, abstracts, and links to full articles. Covers 35+ million \
         citations from biomedical literature."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query for PubMed (e.g., 'CRISPR gene editing', 'cancer immunotherapy')"
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
                .ok_or_else(|| Error::tool_error("Missing 'query' field in structured input"))?
                .to_string(),
        };

        if query.trim().is_empty() {
            return Err(Error::tool_error("Search query cannot be empty"));
        }

        let articles = self.search(&query).await?;
        Ok(self.format_results(&articles))
    }
}

/// Article information from `PubMed`
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PubMedArticle {
    pmid: String,
    title: String,
    authors: Vec<String>,
    journal: Option<String>,
    pub_date: Option<String>,
    abstract_text: Option<String>,
}

/// `ESearch` API response structure
#[derive(Debug, Deserialize)]
struct ESearchResponse {
    esearchresult: ESearchResult,
}

#[derive(Debug, Deserialize)]
struct ESearchResult {
    idlist: Vec<String>,
}

/// Parse `PubMed` XML response
fn parse_pubmed_xml(xml: &str, id_list: &[&str]) -> Result<Vec<PubMedArticle>> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut articles = Vec::new();
    let mut current_article: Option<PubMedArticle> = None;
    let mut current_element = String::new();
    let mut in_abstract = false;
    let mut abstract_parts = Vec::new();
    let mut in_author_list = false;
    let mut current_author_parts: Vec<String> = Vec::new();

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                current_element = name.clone();

                if name == "PubmedArticle" {
                    current_article = Some(PubMedArticle {
                        pmid: String::new(),
                        title: String::new(),
                        authors: Vec::new(),
                        journal: None,
                        pub_date: None,
                        abstract_text: None,
                    });
                } else if name == "Abstract" {
                    in_abstract = true;
                    abstract_parts.clear();
                } else if name == "AuthorList" {
                    in_author_list = true;
                } else if name == "Author" && in_author_list {
                    current_author_parts.clear();
                }
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if name == "PubmedArticle" {
                    if let Some(article) = current_article.take() {
                        articles.push(article);
                    }
                } else if name == "Abstract" {
                    in_abstract = false;
                    if let Some(article) = current_article.as_mut() {
                        article.abstract_text = Some(abstract_parts.join(" "));
                    }
                } else if name == "AuthorList" {
                    in_author_list = false;
                } else if name == "Author" && in_author_list && !current_author_parts.is_empty() {
                    if let Some(article) = current_article.as_mut() {
                        article.authors.push(current_author_parts.join(" "));
                    }
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default().trim().to_string();

                if text.is_empty() {
                    continue;
                }

                if let Some(article) = current_article.as_mut() {
                    match current_element.as_str() {
                        "PMID" => {
                            if article.pmid.is_empty() {
                                article.pmid = text;
                            }
                        }
                        "ArticleTitle" => {
                            article.title = text;
                        }
                        "Title" if article.journal.is_none() => {
                            article.journal = Some(text);
                        }
                        "Year" | "PubDate" => {
                            if article.pub_date.is_none() {
                                article.pub_date = Some(text);
                            }
                        }
                        "AbstractText" if in_abstract => {
                            abstract_parts.push(text);
                        }
                        "LastName" if in_author_list => {
                            current_author_parts.insert(0, text);
                        }
                        "ForeName" | "Initials" if in_author_list => {
                            current_author_parts.push(text);
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(Error::tool_error(format!(
                    "XML parsing error at position {}: {}",
                    reader.buffer_position(),
                    e
                )));
            }
            _ => {}
        }

        buf.clear();
    }

    // Ensure articles match the requested ID order
    if articles.len() != id_list.len() {
        // Some articles may not have been parsed; create placeholder entries
        let mut ordered_articles = Vec::new();
        for &id in id_list {
            if let Some(article) = articles.iter().find(|a| a.pmid == id) {
                ordered_articles.push(article.clone());
            } else {
                // Fallback: create minimal article with just PMID
                ordered_articles.push(PubMedArticle {
                    pmid: id.to_string(),
                    title: "[Article details not available]".to_string(),
                    authors: Vec::new(),
                    journal: None,
                    pub_date: None,
                    abstract_text: None,
                });
            }
        }
        return Ok(ordered_articles);
    }

    Ok(articles)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // ==========================================================================
    // PubMedSearch Creation and Builder Tests
    // ==========================================================================

    #[test]
    fn test_pubmed_search_creation() {
        let tool = PubMedSearch::new();
        assert_eq!(tool.max_results, 3);
        assert_eq!(tool.database, "pubmed");
        assert!(tool.api_key.is_none());
    }

    #[test]
    fn test_pubmed_search_builder() {
        let tool = PubMedSearch::builder()
            .max_results(10)
            .database("pmc")
            .sort_by("pub_date")
            .api_key("test_key")
            .build();

        assert_eq!(tool.max_results, 10);
        assert_eq!(tool.database, "pmc");
        assert_eq!(tool.sort_by, Some("pub_date".to_string()));
        assert_eq!(tool.api_key, Some("test_key".to_string()));
    }

    #[test]
    fn test_max_results_clamping() {
        let tool = PubMedSearch::new().max_results(20000);
        assert_eq!(tool.max_results, 10000); // Should clamp to max
    }

    #[test]
    fn test_max_results_zero() {
        let tool = PubMedSearch::new().max_results(0);
        assert_eq!(tool.max_results, 0);
    }

    #[test]
    fn test_max_results_boundary() {
        let tool = PubMedSearch::new().max_results(10000);
        assert_eq!(tool.max_results, 10000); // Exactly at max
    }

    #[test]
    fn test_max_results_just_over_boundary() {
        let tool = PubMedSearch::new().max_results(10001);
        assert_eq!(tool.max_results, 10000); // Should clamp
    }

    #[test]
    fn test_builder_chaining() {
        // Test that builder methods can be chained in any order
        let tool1 = PubMedSearch::builder()
            .max_results(5)
            .database("pmc")
            .sort_by("relevance")
            .api_key("key1")
            .build();

        let tool2 = PubMedSearch::builder()
            .api_key("key1")
            .sort_by("relevance")
            .max_results(5)
            .database("pmc")
            .build();

        assert_eq!(tool1.max_results, tool2.max_results);
        assert_eq!(tool1.database, tool2.database);
        assert_eq!(tool1.sort_by, tool2.sort_by);
        assert_eq!(tool1.api_key, tool2.api_key);
    }

    #[test]
    fn test_default_trait() {
        let tool = PubMedSearch::default();
        assert_eq!(tool.max_results, 3);
        assert_eq!(tool.database, "pubmed");
        assert!(tool.sort_by.is_none());
        assert!(tool.api_key.is_none());
    }

    #[test]
    fn test_clone_trait() {
        let tool = PubMedSearch::builder()
            .max_results(5)
            .database("pmc")
            .sort_by("pub_date")
            .api_key("secret")
            .build();

        let cloned = tool.clone();
        assert_eq!(cloned.max_results, 5);
        assert_eq!(cloned.database, "pmc");
        assert_eq!(cloned.sort_by, Some("pub_date".to_string()));
        assert_eq!(cloned.api_key, Some("secret".to_string()));
    }

    #[test]
    fn test_debug_trait() {
        let tool = PubMedSearch::new();
        let debug_str = format!("{:?}", tool);
        assert!(debug_str.contains("PubMedSearch"));
        assert!(debug_str.contains("max_results"));
        assert!(debug_str.contains("database"));
    }

    // ==========================================================================
    // Tool Trait Implementation Tests
    // ==========================================================================

    #[test]
    fn test_tool_name() {
        let tool = PubMedSearch::new();
        assert_eq!(tool.name(), "pubmed_search");
    }

    #[test]
    fn test_tool_description() {
        let tool = PubMedSearch::new();
        let desc = tool.description();
        assert!(desc.contains("PubMed"));
        assert!(desc.contains("medical"));
    }

    #[test]
    fn test_tool_description_content() {
        let tool = PubMedSearch::new();
        let desc = tool.description();
        // Verify key information is in the description
        assert!(desc.contains("scientific"));
        assert!(desc.contains("literature"));
        assert!(desc.contains("35+ million"));
    }

    #[test]
    fn test_tool_args_schema() {
        let tool = PubMedSearch::new();
        let schema = tool.args_schema();
        assert!(schema.is_object());
        assert!(schema["properties"]["query"].is_object());
        assert_eq!(schema["required"][0], "query");
    }

    #[test]
    fn test_tool_args_schema_query_description() {
        let tool = PubMedSearch::new();
        let schema = tool.args_schema();
        let query_desc = schema["properties"]["query"]["description"]
            .as_str()
            .unwrap();
        assert!(query_desc.contains("Search query"));
        assert!(query_desc.contains("PubMed"));
    }

    // ==========================================================================
    // Input Handling Tests
    // ==========================================================================

    #[tokio::test]
    async fn test_empty_query() {
        let tool = PubMedSearch::new();
        let result = tool._call(ToolInput::String("".to_string())).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[tokio::test]
    async fn test_whitespace_only_query() {
        let tool = PubMedSearch::new();
        let result = tool._call(ToolInput::String("   ".to_string())).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[tokio::test]
    async fn test_tabs_and_newlines_query() {
        let tool = PubMedSearch::new();
        let result = tool._call(ToolInput::String("\t\n\r".to_string())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_structured_input_missing_query() {
        let tool = PubMedSearch::new();
        let input = json!({ "other_field": "value" });
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("query"));
    }

    #[tokio::test]
    async fn test_structured_input_null_query() {
        let tool = PubMedSearch::new();
        let input = json!({ "query": null });
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_structured_input_numeric_query() {
        let tool = PubMedSearch::new();
        let input = json!({ "query": 12345 });
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_structured_input_array_query() {
        let tool = PubMedSearch::new();
        let input = json!({ "query": ["CRISPR", "cancer"] });
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    // ==========================================================================
    // XML Parsing Tests
    // ==========================================================================

    #[test]
    fn test_parse_pubmed_xml_basic() {
        let xml = r#"<?xml version="1.0"?>
<PubmedArticleSet>
  <PubmedArticle>
    <MedlineCitation>
      <PMID>12345</PMID>
      <Article>
        <ArticleTitle>Test Article Title</ArticleTitle>
        <Abstract>
          <AbstractText>This is the abstract text.</AbstractText>
        </Abstract>
        <AuthorList>
          <Author>
            <LastName>Smith</LastName>
            <ForeName>John</ForeName>
          </Author>
        </AuthorList>
        <Journal>
          <Title>Test Journal</Title>
        </Journal>
      </Article>
    </MedlineCitation>
    <PubmedData>
      <History>
        <PubMedPubDate PubStatus="pubmed">
          <Year>2024</Year>
        </PubMedPubDate>
      </History>
    </PubmedData>
  </PubmedArticle>
</PubmedArticleSet>"#;

        let id_list = vec!["12345"];
        let articles = parse_pubmed_xml(xml, &id_list).unwrap();
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].pmid, "12345");
        assert_eq!(articles[0].title, "Test Article Title");
        assert_eq!(articles[0].authors, vec!["Smith John"]);
        assert_eq!(articles[0].journal, Some("Test Journal".to_string()));
        assert_eq!(
            articles[0].abstract_text,
            Some("This is the abstract text.".to_string())
        );
    }

    #[test]
    fn test_parse_pubmed_xml_multiple_articles() {
        let xml = r#"<?xml version="1.0"?>
<PubmedArticleSet>
  <PubmedArticle>
    <MedlineCitation>
      <PMID>111</PMID>
      <Article>
        <ArticleTitle>Article One</ArticleTitle>
      </Article>
    </MedlineCitation>
  </PubmedArticle>
  <PubmedArticle>
    <MedlineCitation>
      <PMID>222</PMID>
      <Article>
        <ArticleTitle>Article Two</ArticleTitle>
      </Article>
    </MedlineCitation>
  </PubmedArticle>
  <PubmedArticle>
    <MedlineCitation>
      <PMID>333</PMID>
      <Article>
        <ArticleTitle>Article Three</ArticleTitle>
      </Article>
    </MedlineCitation>
  </PubmedArticle>
</PubmedArticleSet>"#;

        let id_list = vec!["111", "222", "333"];
        let articles = parse_pubmed_xml(xml, &id_list).unwrap();
        assert_eq!(articles.len(), 3);
        assert_eq!(articles[0].title, "Article One");
        assert_eq!(articles[1].title, "Article Two");
        assert_eq!(articles[2].title, "Article Three");
    }

    #[test]
    fn test_parse_pubmed_xml_multiple_authors() {
        let xml = r#"<?xml version="1.0"?>
<PubmedArticleSet>
  <PubmedArticle>
    <MedlineCitation>
      <PMID>12345</PMID>
      <Article>
        <ArticleTitle>Multi-Author Study</ArticleTitle>
        <AuthorList>
          <Author>
            <LastName>Smith</LastName>
            <ForeName>John</ForeName>
          </Author>
          <Author>
            <LastName>Doe</LastName>
            <ForeName>Jane</ForeName>
          </Author>
          <Author>
            <LastName>Johnson</LastName>
            <Initials>R</Initials>
          </Author>
        </AuthorList>
      </Article>
    </MedlineCitation>
  </PubmedArticle>
</PubmedArticleSet>"#;

        let id_list = vec!["12345"];
        let articles = parse_pubmed_xml(xml, &id_list).unwrap();
        assert_eq!(articles[0].authors.len(), 3);
        assert_eq!(articles[0].authors[0], "Smith John");
        assert_eq!(articles[0].authors[1], "Doe Jane");
        assert_eq!(articles[0].authors[2], "Johnson R");
    }

    #[test]
    fn test_parse_pubmed_xml_multi_part_abstract() {
        let xml = r#"<?xml version="1.0"?>
<PubmedArticleSet>
  <PubmedArticle>
    <MedlineCitation>
      <PMID>12345</PMID>
      <Article>
        <ArticleTitle>Structured Abstract Article</ArticleTitle>
        <Abstract>
          <AbstractText Label="BACKGROUND">Background text.</AbstractText>
          <AbstractText Label="METHODS">Methods text.</AbstractText>
          <AbstractText Label="RESULTS">Results text.</AbstractText>
          <AbstractText Label="CONCLUSION">Conclusion text.</AbstractText>
        </Abstract>
      </Article>
    </MedlineCitation>
  </PubmedArticle>
</PubmedArticleSet>"#;

        let id_list = vec!["12345"];
        let articles = parse_pubmed_xml(xml, &id_list).unwrap();
        let abstract_text = articles[0].abstract_text.as_ref().unwrap();
        assert!(abstract_text.contains("Background text."));
        assert!(abstract_text.contains("Methods text."));
        assert!(abstract_text.contains("Results text."));
        assert!(abstract_text.contains("Conclusion text."));
    }

    #[test]
    fn test_parse_pubmed_xml_empty() {
        let xml = r#"<?xml version="1.0"?><PubmedArticleSet></PubmedArticleSet>"#;
        let id_list: Vec<&str> = vec![];
        let articles = parse_pubmed_xml(xml, &id_list).unwrap();
        assert!(articles.is_empty());
    }

    #[test]
    fn test_parse_pubmed_xml_missing_fields() {
        // Article with minimal fields - only PMID and title
        let xml = r#"<?xml version="1.0"?>
<PubmedArticleSet>
  <PubmedArticle>
    <MedlineCitation>
      <PMID>99999</PMID>
      <Article>
        <ArticleTitle>Minimal Article</ArticleTitle>
      </Article>
    </MedlineCitation>
  </PubmedArticle>
</PubmedArticleSet>"#;

        let id_list = vec!["99999"];
        let articles = parse_pubmed_xml(xml, &id_list).unwrap();
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].pmid, "99999");
        assert_eq!(articles[0].title, "Minimal Article");
        assert!(articles[0].authors.is_empty());
        assert!(articles[0].journal.is_none());
        assert!(articles[0].abstract_text.is_none());
        assert!(articles[0].pub_date.is_none());
    }

    #[test]
    fn test_parse_pubmed_xml_preserves_xml_order() {
        // When all articles are present, they are returned in XML document order
        // (reordering only happens when there's a count mismatch to create placeholders)
        let xml = r#"<?xml version="1.0"?>
<PubmedArticleSet>
  <PubmedArticle>
    <MedlineCitation>
      <PMID>333</PMID>
      <Article><ArticleTitle>Third in XML</ArticleTitle></Article>
    </MedlineCitation>
  </PubmedArticle>
  <PubmedArticle>
    <MedlineCitation>
      <PMID>111</PMID>
      <Article><ArticleTitle>First in XML</ArticleTitle></Article>
    </MedlineCitation>
  </PubmedArticle>
  <PubmedArticle>
    <MedlineCitation>
      <PMID>222</PMID>
      <Article><ArticleTitle>Second in XML</ArticleTitle></Article>
    </MedlineCitation>
  </PubmedArticle>
</PubmedArticleSet>"#;

        // Note: id_list order doesn't affect output when count matches
        let id_list = vec!["111", "222", "333"];
        let articles = parse_pubmed_xml(xml, &id_list).unwrap();
        assert_eq!(articles.len(), 3);
        // Articles are in XML document order
        assert_eq!(articles[0].pmid, "333");
        assert_eq!(articles[1].pmid, "111");
        assert_eq!(articles[2].pmid, "222");
    }

    #[test]
    fn test_parse_pubmed_xml_missing_article_placeholder() {
        // If an ID doesn't have a corresponding article in XML, create placeholder
        let xml = r#"<?xml version="1.0"?>
<PubmedArticleSet>
  <PubmedArticle>
    <MedlineCitation>
      <PMID>111</PMID>
      <Article><ArticleTitle>Found Article</ArticleTitle></Article>
    </MedlineCitation>
  </PubmedArticle>
</PubmedArticleSet>"#;

        let id_list = vec!["111", "999"]; // 999 not in XML
        let articles = parse_pubmed_xml(xml, &id_list).unwrap();
        assert_eq!(articles.len(), 2);
        assert_eq!(articles[0].pmid, "111");
        assert_eq!(articles[1].pmid, "999");
        assert!(articles[1].title.contains("not available"));
    }

    #[test]
    fn test_parse_pubmed_xml_special_characters() {
        let xml = r#"<?xml version="1.0"?>
<PubmedArticleSet>
  <PubmedArticle>
    <MedlineCitation>
      <PMID>12345</PMID>
      <Article>
        <ArticleTitle>Testing &amp; Special &lt;Characters&gt;</ArticleTitle>
        <Abstract>
          <AbstractText>Contains "quotes" and 'apostrophes'.</AbstractText>
        </Abstract>
      </Article>
    </MedlineCitation>
  </PubmedArticle>
</PubmedArticleSet>"#;

        let id_list = vec!["12345"];
        let articles = parse_pubmed_xml(xml, &id_list).unwrap();
        assert!(articles[0].title.contains("&"));
        assert!(articles[0].title.contains("<"));
        assert!(articles[0].title.contains(">"));
    }

    #[test]
    fn test_parse_pubmed_xml_unicode() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PubmedArticleSet>
  <PubmedArticle>
    <MedlineCitation>
      <PMID>12345</PMID>
      <Article>
        <ArticleTitle>Study on α-synuclein and β-amyloid</ArticleTitle>
        <AuthorList>
          <Author>
            <LastName>Müller</LastName>
            <ForeName>François</ForeName>
          </Author>
        </AuthorList>
      </Article>
    </MedlineCitation>
  </PubmedArticle>
</PubmedArticleSet>"#;

        let id_list = vec!["12345"];
        let articles = parse_pubmed_xml(xml, &id_list).unwrap();
        assert!(articles[0].title.contains("α-synuclein"));
        assert!(articles[0].title.contains("β-amyloid"));
        assert!(articles[0].authors[0].contains("Müller"));
    }

    #[test]
    fn test_parse_pubmed_xml_invalid_xml() {
        let xml = r#"<not valid xml"#;
        let id_list = vec!["12345"];
        let result = parse_pubmed_xml(xml, &id_list);
        // Should either return error or empty results - don't crash
        if result.is_ok() {
            let articles = result.unwrap();
            // If parsing succeeds, should have placeholder
            assert!(articles.iter().any(|a| a.pmid == "12345"));
        }
    }

    // ==========================================================================
    // Format Results Tests
    // ==========================================================================

    #[test]
    fn test_format_results_empty() {
        let tool = PubMedSearch::new();
        let articles: Vec<PubMedArticle> = vec![];
        let output = tool.format_results(&articles);
        assert_eq!(output, "No results found.");
    }

    #[test]
    fn test_format_results_single_article() {
        let tool = PubMedSearch::new();
        let articles = vec![PubMedArticle {
            pmid: "12345".to_string(),
            title: "Test Article".to_string(),
            authors: vec!["Smith John".to_string()],
            journal: Some("Test Journal".to_string()),
            pub_date: Some("2024".to_string()),
            abstract_text: Some("Test abstract".to_string()),
        }];
        let output = tool.format_results(&articles);
        assert!(output.contains("Found 1 article"));
        assert!(output.contains("Test Article"));
        assert!(output.contains("PMID: 12345"));
        assert!(output.contains("Smith John"));
        assert!(output.contains("Test Journal"));
        assert!(output.contains("2024"));
        assert!(output.contains("Test abstract"));
        assert!(output.contains("pubmed.ncbi.nlm.nih.gov/12345/"));
    }

    #[test]
    fn test_format_results_multiple_articles() {
        let tool = PubMedSearch::new();
        let articles = vec![
            PubMedArticle {
                pmid: "111".to_string(),
                title: "First Article".to_string(),
                authors: vec![],
                journal: None,
                pub_date: None,
                abstract_text: None,
            },
            PubMedArticle {
                pmid: "222".to_string(),
                title: "Second Article".to_string(),
                authors: vec![],
                journal: None,
                pub_date: None,
                abstract_text: None,
            },
        ];
        let output = tool.format_results(&articles);
        assert!(output.contains("Found 2 article(s)"));
        assert!(output.contains("1. First Article"));
        assert!(output.contains("2. Second Article"));
    }

    #[test]
    fn test_format_results_author_truncation() {
        let tool = PubMedSearch::new();
        // More than 3 authors should be truncated to "et al."
        let articles = vec![PubMedArticle {
            pmid: "12345".to_string(),
            title: "Multi-Author Study".to_string(),
            authors: vec![
                "Smith John".to_string(),
                "Doe Jane".to_string(),
                "Johnson Bob".to_string(),
                "Williams Alice".to_string(),
            ],
            journal: None,
            pub_date: None,
            abstract_text: None,
        }];
        let output = tool.format_results(&articles);
        assert!(output.contains("Smith John et al."));
        assert!(!output.contains("Doe Jane"));
    }

    #[test]
    fn test_format_results_three_authors_no_truncation() {
        let tool = PubMedSearch::new();
        let articles = vec![PubMedArticle {
            pmid: "12345".to_string(),
            title: "Three-Author Study".to_string(),
            authors: vec![
                "Smith John".to_string(),
                "Doe Jane".to_string(),
                "Johnson Bob".to_string(),
            ],
            journal: None,
            pub_date: None,
            abstract_text: None,
        }];
        let output = tool.format_results(&articles);
        assert!(output.contains("Smith John"));
        assert!(output.contains("Doe Jane"));
        assert!(output.contains("Johnson Bob"));
        assert!(!output.contains("et al."));
    }

    #[test]
    fn test_format_results_abstract_truncation() {
        let tool = PubMedSearch::new();
        // Abstract longer than 300 characters should be truncated
        let long_abstract = "A".repeat(500);
        let articles = vec![PubMedArticle {
            pmid: "12345".to_string(),
            title: "Long Abstract Study".to_string(),
            authors: vec![],
            journal: None,
            pub_date: None,
            abstract_text: Some(long_abstract),
        }];
        let output = tool.format_results(&articles);
        // Should end with "..."
        assert!(output.contains("..."));
        // Should not contain all 500 A's
        assert!(output.matches('A').count() < 500);
    }

    #[test]
    fn test_format_results_short_abstract_no_truncation() {
        let tool = PubMedSearch::new();
        let short_abstract = "A".repeat(100);
        let articles = vec![PubMedArticle {
            pmid: "12345".to_string(),
            title: "Short Abstract Study".to_string(),
            authors: vec![],
            journal: None,
            pub_date: None,
            abstract_text: Some(short_abstract.clone()),
        }];
        let output = tool.format_results(&articles);
        // Should contain exact abstract without "..."
        assert!(output.contains(&short_abstract));
        // The abstract portion shouldn't end with "..." (but note the output has other "..." potentially)
        assert!(output.contains(&format!("Abstract: {}", short_abstract)));
    }

    // ==========================================================================
    // Database Configuration Tests
    // ==========================================================================

    #[test]
    fn test_database_pmc() {
        let tool = PubMedSearch::new().database("pmc").build();
        assert_eq!(tool.database, "pmc");
    }

    #[test]
    fn test_database_mesh() {
        let tool = PubMedSearch::new().database("mesh").build();
        assert_eq!(tool.database, "mesh");
    }

    #[test]
    fn test_database_empty_string() {
        let tool = PubMedSearch::new().database("").build();
        assert_eq!(tool.database, "");
    }

    #[test]
    fn test_sort_options() {
        let relevance = PubMedSearch::new().sort_by("relevance").build();
        let pub_date = PubMedSearch::new().sort_by("pub_date").build();
        let author = PubMedSearch::new().sort_by("author").build();
        let journal = PubMedSearch::new().sort_by("journal").build();

        assert_eq!(relevance.sort_by, Some("relevance".to_string()));
        assert_eq!(pub_date.sort_by, Some("pub_date".to_string()));
        assert_eq!(author.sort_by, Some("author".to_string()));
        assert_eq!(journal.sort_by, Some("journal".to_string()));
    }

    // ==========================================================================
    // PubMedArticle Serialization Tests
    // ==========================================================================

    #[test]
    fn test_pubmed_article_serialize() {
        let article = PubMedArticle {
            pmid: "12345".to_string(),
            title: "Test Article".to_string(),
            authors: vec!["Smith John".to_string()],
            journal: Some("Test Journal".to_string()),
            pub_date: Some("2024".to_string()),
            abstract_text: Some("Test abstract".to_string()),
        };
        let json = serde_json::to_string(&article).unwrap();
        assert!(json.contains("12345"));
        assert!(json.contains("Test Article"));
    }

    #[test]
    fn test_pubmed_article_deserialize() {
        let json = r#"{
            "pmid": "12345",
            "title": "Test Article",
            "authors": ["Smith John"],
            "journal": "Test Journal",
            "pub_date": "2024",
            "abstract_text": "Test abstract"
        }"#;
        let article: PubMedArticle = serde_json::from_str(json).unwrap();
        assert_eq!(article.pmid, "12345");
        assert_eq!(article.title, "Test Article");
        assert_eq!(article.authors, vec!["Smith John"]);
    }

    #[test]
    fn test_pubmed_article_serialize_roundtrip() {
        let article = PubMedArticle {
            pmid: "99999".to_string(),
            title: "Roundtrip Test".to_string(),
            authors: vec!["Author One".to_string(), "Author Two".to_string()],
            journal: Some("Science".to_string()),
            pub_date: Some("2025".to_string()),
            abstract_text: Some("Important findings.".to_string()),
        };
        let json = serde_json::to_string(&article).unwrap();
        let parsed: PubMedArticle = serde_json::from_str(&json).unwrap();
        assert_eq!(article.pmid, parsed.pmid);
        assert_eq!(article.title, parsed.title);
        assert_eq!(article.authors, parsed.authors);
        assert_eq!(article.journal, parsed.journal);
        assert_eq!(article.pub_date, parsed.pub_date);
        assert_eq!(article.abstract_text, parsed.abstract_text);
    }

    #[test]
    fn test_pubmed_article_clone() {
        let article = PubMedArticle {
            pmid: "12345".to_string(),
            title: "Clone Test".to_string(),
            authors: vec!["Author".to_string()],
            journal: Some("Journal".to_string()),
            pub_date: Some("2024".to_string()),
            abstract_text: Some("Abstract".to_string()),
        };
        let cloned = article.clone();
        assert_eq!(article.pmid, cloned.pmid);
        assert_eq!(article.title, cloned.title);
    }

    #[test]
    fn test_pubmed_article_debug() {
        let article = PubMedArticle {
            pmid: "12345".to_string(),
            title: "Debug Test".to_string(),
            authors: vec![],
            journal: None,
            pub_date: None,
            abstract_text: None,
        };
        let debug_str = format!("{:?}", article);
        assert!(debug_str.contains("PubMedArticle"));
        assert!(debug_str.contains("12345"));
    }

    // ==========================================================================
    // Integration Tests (require network)
    // ==========================================================================

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_pubmed_search_integration() {
        let tool = PubMedSearch::new().max_results(2).build();
        let output = tool
            ._call(ToolInput::String("CRISPR".to_string()))
            .await
            .expect("PubMed call failed");
        assert!(output.contains("PMID"));
        assert!(output.contains("pubmed.ncbi.nlm.nih.gov"));
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_no_results() {
        let tool = PubMedSearch::new();
        let output = tool
            ._call(ToolInput::String("zxqkjhweriuoasdf123456789".to_string()))
            .await
            .expect("PubMed call failed");
        assert!(output.contains("No results found"));
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_structured_input_with_query() {
        let tool = PubMedSearch::new().max_results(1).build();
        let input = json!({ "query": "cancer immunotherapy" });
        let output = tool
            ._call(ToolInput::Structured(input))
            .await
            .expect("PubMed call failed");
        assert!(output.contains("PMID"));
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_load_docs() {
        let tool = PubMedSearch::new().max_results(2).build();
        let docs = tool.load_docs("CRISPR").await.expect("load_docs failed");
        assert!(!docs.is_empty());
        assert!(docs.len() <= 2);

        // Check document structure
        let doc = &docs[0];
        assert!(doc.id.is_some());
        assert!(doc.metadata.contains_key("Title"));
        assert!(doc.metadata.contains_key("uid"));
    }
}

/// `PubMed` retriever for document retrieval from medical and scientific literature
///
/// This retriever wraps `PubMedSearch` and converts search results into Documents
/// suitable for use in retrieval chains and RAG applications.
///
/// # Python Baseline
///
/// This implements the functionality from:
/// `~/dashflow_community/dashflow_community/retrievers/pubmed.py`
///
/// Python equivalent:
/// ```python
/// from dashflow_community.retrievers import PubMedRetriever
///
/// retriever = PubMedRetriever(top_k_results=5)
/// docs = retriever.invoke("CRISPR gene editing")
/// ```
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_pubmed::PubMedRetriever;
/// use dashflow::core::retrievers::Retriever;
///
/// # tokio_test::block_on(async {
/// let retriever = PubMedRetriever::builder()
///     .max_results(5)
///     .build();
///
/// let docs = retriever._get_relevant_documents("cancer immunotherapy", None)
///     .await
///     .unwrap();
///
/// for doc in docs {
///     println!("Title: {}", doc.metadata.get("Title").unwrap());
///     println!("Abstract: {}", doc.page_content);
/// }
/// # });
/// ```
pub struct PubMedRetriever {
    /// Internal `PubMedSearch` tool
    search: PubMedSearch,
}

impl PubMedRetriever {
    /// Create a new `PubMedRetriever` with default settings
    ///
    /// Default settings:
    /// - `max_results`: 3
    /// - database: "pubmed"
    #[must_use]
    pub fn new() -> Self {
        Self {
            search: PubMedSearch::new(),
        }
    }

    /// Create a builder for `PubMedRetriever`
    #[must_use]
    pub fn builder() -> PubMedRetrieverBuilder {
        PubMedRetrieverBuilder::default()
    }
}

impl Default for PubMedRetriever {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Retriever for PubMedRetriever {
    async fn _get_relevant_documents(
        &self,
        query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        self.search.load_docs(query).await
    }
}

/// Builder for `PubMedRetriever`
#[derive(Default)]
pub struct PubMedRetrieverBuilder {
    max_results: Option<usize>,
    database: Option<String>,
    sort_by: Option<String>,
    api_key: Option<String>,
}

impl PubMedRetrieverBuilder {
    /// Set the maximum number of results (default: 3, max: 10,000)
    #[must_use]
    pub fn max_results(mut self, max: usize) -> Self {
        self.max_results = Some(max);
        self
    }

    /// Set the database to search (default: "pubmed")
    pub fn database(mut self, db: impl Into<String>) -> Self {
        self.database = Some(db.into());
        self
    }

    /// Set the sort order
    pub fn sort_by(mut self, sort: impl Into<String>) -> Self {
        self.sort_by = Some(sort.into());
        self
    }

    /// Set API key for higher rate limits
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Build the `PubMedRetriever`
    #[must_use]
    pub fn build(self) -> PubMedRetriever {
        let mut search = PubMedSearch::new();

        if let Some(max) = self.max_results {
            search = search.max_results(max);
        }
        if let Some(db) = self.database {
            search = search.database(db);
        }
        if let Some(sort) = self.sort_by {
            search = search.sort_by(sort);
        }
        if let Some(key) = self.api_key {
            search = search.api_key(key);
        }

        PubMedRetriever {
            search: search.build(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod retriever_tests {
    use super::*;

    // ==========================================================================
    // PubMedRetriever Creation Tests
    // ==========================================================================

    #[test]
    fn test_pubmed_retriever_creation() {
        let retriever = PubMedRetriever::new();
        assert_eq!(retriever.search.max_results, 3);
        assert_eq!(retriever.search.database, "pubmed");
    }

    #[test]
    fn test_pubmed_retriever_builder() {
        let retriever = PubMedRetriever::builder()
            .max_results(10)
            .database("pmc")
            .sort_by("pub_date")
            .build();

        assert_eq!(retriever.search.max_results, 10);
        assert_eq!(retriever.search.database, "pmc");
    }

    #[test]
    fn test_pubmed_retriever_default() {
        let retriever = PubMedRetriever::default();
        assert_eq!(retriever.search.max_results, 3);
    }

    #[test]
    fn test_pubmed_retriever_with_api_key() {
        let retriever = PubMedRetriever::builder()
            .max_results(1)
            .api_key("test_key")
            .build();
        assert_eq!(retriever.search.api_key, Some("test_key".to_string()));
    }

    // ==========================================================================
    // PubMedRetrieverBuilder Tests
    // ==========================================================================

    #[test]
    fn test_builder_default_state() {
        let builder = PubMedRetrieverBuilder::default();
        assert!(builder.max_results.is_none());
        assert!(builder.database.is_none());
        assert!(builder.sort_by.is_none());
        assert!(builder.api_key.is_none());
    }

    #[test]
    fn test_builder_all_options() {
        let retriever = PubMedRetriever::builder()
            .max_results(50)
            .database("pmc")
            .sort_by("relevance")
            .api_key("my_api_key")
            .build();

        assert_eq!(retriever.search.max_results, 50);
        assert_eq!(retriever.search.database, "pmc");
        assert_eq!(retriever.search.sort_by, Some("relevance".to_string()));
        assert_eq!(retriever.search.api_key, Some("my_api_key".to_string()));
    }

    #[test]
    fn test_builder_partial_options() {
        // Only set some options
        let retriever = PubMedRetriever::builder()
            .max_results(20)
            .build();

        assert_eq!(retriever.search.max_results, 20);
        assert_eq!(retriever.search.database, "pubmed"); // Default
        assert!(retriever.search.sort_by.is_none());
        assert!(retriever.search.api_key.is_none());
    }

    #[test]
    fn test_builder_database_only() {
        let retriever = PubMedRetriever::builder()
            .database("mesh")
            .build();

        assert_eq!(retriever.search.database, "mesh");
        assert_eq!(retriever.search.max_results, 3); // Default
    }

    #[test]
    fn test_builder_sort_only() {
        let retriever = PubMedRetriever::builder()
            .sort_by("pub_date")
            .build();

        assert_eq!(retriever.search.sort_by, Some("pub_date".to_string()));
    }

    #[test]
    fn test_builder_api_key_only() {
        let retriever = PubMedRetriever::builder()
            .api_key("secret_key")
            .build();

        assert_eq!(retriever.search.api_key, Some("secret_key".to_string()));
    }

    #[test]
    fn test_builder_chaining_order() {
        // Test that order of chaining doesn't affect result
        let r1 = PubMedRetriever::builder()
            .max_results(5)
            .database("pmc")
            .sort_by("author")
            .api_key("key")
            .build();

        let r2 = PubMedRetriever::builder()
            .api_key("key")
            .sort_by("author")
            .max_results(5)
            .database("pmc")
            .build();

        assert_eq!(r1.search.max_results, r2.search.max_results);
        assert_eq!(r1.search.database, r2.search.database);
        assert_eq!(r1.search.sort_by, r2.search.sort_by);
        assert_eq!(r1.search.api_key, r2.search.api_key);
    }

    #[test]
    fn test_builder_max_results_clamping() {
        let retriever = PubMedRetriever::builder()
            .max_results(100000) // Way over limit
            .build();

        assert_eq!(retriever.search.max_results, 10000); // Clamped
    }

    #[test]
    fn test_builder_empty_strings() {
        let retriever = PubMedRetriever::builder()
            .database("")
            .sort_by("")
            .api_key("")
            .build();

        assert_eq!(retriever.search.database, "");
        assert_eq!(retriever.search.sort_by, Some("".to_string()));
        assert_eq!(retriever.search.api_key, Some("".to_string()));
    }

    #[test]
    fn test_builder_string_ownership() {
        // Test that Into<String> works with various types
        let retriever = PubMedRetriever::builder()
            .database("pmc")                          // &str
            .sort_by(String::from("pub_date"))       // String
            .api_key("api_key".to_owned())           // String
            .build();

        assert_eq!(retriever.search.database, "pmc");
        assert_eq!(retriever.search.sort_by, Some("pub_date".to_string()));
    }

    // ==========================================================================
    // Integration Tests (require network)
    // ==========================================================================

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_pubmed_retriever_get_documents() {
        let retriever = PubMedRetriever::builder().max_results(2).build();

        let docs = retriever
            ._get_relevant_documents("CRISPR", None)
            .await
            .expect("PubMed retriever failed");
        assert!(!docs.is_empty());
        assert!(docs.len() <= 2);

        // Check document structure
        let first_doc = &docs[0];
        assert!(first_doc.metadata.contains_key("Title"));
        assert!(first_doc.metadata.contains_key("uid"));
        assert!(!first_doc.page_content.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_pubmed_retriever_with_config() {
        let retriever = PubMedRetriever::new();
        let config = RunnableConfig::default();

        // Config should be accepted even if not used
        let docs = retriever
            ._get_relevant_documents("cancer", Some(&config))
            .await
            .expect("PubMed retriever failed");

        assert!(!docs.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_pubmed_retriever_document_metadata() {
        let retriever = PubMedRetriever::builder().max_results(1).build();

        let docs = retriever
            ._get_relevant_documents("gene therapy", None)
            .await
            .expect("PubMed retriever failed");

        assert!(!docs.is_empty());
        let doc = &docs[0];

        // Verify all expected metadata fields
        assert!(doc.metadata.contains_key("uid"));
        assert!(doc.metadata.contains_key("Title"));
        // These may or may not be present depending on article
        // doc.metadata.get("Published")
        // doc.metadata.get("Journal")
        // doc.metadata.get("Authors")

        // Verify document has an ID
        assert!(doc.id.is_some());
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_pubmed_retriever_no_results() {
        let retriever = PubMedRetriever::new();

        let docs = retriever
            ._get_relevant_documents("zxqkjhweriuoasdf123456789", None)
            .await
            .expect("PubMed retriever should handle no results");

        assert!(docs.is_empty());
    }
}
