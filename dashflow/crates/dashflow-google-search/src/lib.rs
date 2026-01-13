//! # Google Custom Search Integration for DashFlow
//!
//! This crate provides Google Custom Search API integration for `DashFlow` Rust,
//! allowing agents to search the web using Google's search engine.
//!
//! ## Features
//!
//! - Web search using Google Custom Search JSON API
//! - Configurable result count and search options
//! - Image search support
//! - Site restriction for targeted searches
//! - Retriever implementation for RAG chains
//!
//! ## Setup
//!
//! 1. Create a Google Cloud project at <https://console.cloud.google.com>
//! 2. Enable the Custom Search JSON API
//! 3. Create API credentials and get your API key
//! 4. Create a Programmable Search Engine at <https://programmablesearchengine.google.com>
//! 5. Get your Search Engine ID (cx parameter)
//!
//! ## Usage
//!
//! ### Web Search Tool
//!
//! ```rust,no_run
//! use dashflow_google_search::GoogleSearchTool;
//! use dashflow::core::tools::Tool;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Set environment variables
//! std::env::set_var("GOOGLE_API_KEY", "your-api-key");
//! std::env::set_var("GOOGLE_CSE_ID", "your-search-engine-id");
//!
//! let google = GoogleSearchTool::new()?;
//!
//! let results = google._call_str("rust programming language".to_string()).await?;
//! println!("{}", results);
//! # Ok(())
//! # }
//! ```
//!
//! ### Retriever for RAG
//!
//! ```rust,no_run
//! use dashflow_google_search::GoogleSearchRetriever;
//! use dashflow::core::retrievers::Retriever;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! std::env::set_var("GOOGLE_API_KEY", "your-api-key");
//! std::env::set_var("GOOGLE_CSE_ID", "your-search-engine-id");
//!
//! let retriever = GoogleSearchRetriever::new()?;
//!
//! let docs = retriever._get_relevant_documents("machine learning tutorials", None).await?;
//!
//! for doc in docs {
//!     println!("Title: {}", doc.metadata.get("title").unwrap());
//!     println!("URL: {}", doc.metadata.get("link").unwrap());
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Environment Variables
//!
//! - `GOOGLE_API_KEY`: Required. Your Google Cloud API key
//! - `GOOGLE_CSE_ID`: Required. Your Programmable Search Engine ID
//!
//! ## Python Baseline
//!
//! This crate implements functionality from:
//! - `dashflow_community.utilities.google_search` - Google Search utility
//! - `dashflow_community.tools.google_search` - Google Search tool
//!
//! # See Also
//!
//! - [`Tool`] - The trait this tool implements
//! - [`Retriever`] - The retriever trait
//! - [`dashflow-tavily`](https://docs.rs/dashflow-tavily) - Alternative: Tavily AI-optimized search
//! - [`dashflow-webscrape`](https://docs.rs/dashflow-webscrape) - Web content extraction tool
//! - [Google Custom Search API](https://developers.google.com/custom-search/v1/overview) - Official docs

use async_trait::async_trait;
use dashflow::core::config::RunnableConfig;
use dashflow::core::documents::Document;
use dashflow::core::http_client::{json_with_limit, SEARCH_RESPONSE_SIZE_LIMIT};
use dashflow::core::config_loader::env_vars::{env_string, GOOGLE_API_KEY, GOOGLE_CSE_ID};
use dashflow::core::retrievers::Retriever;
use dashflow::core::tools::{Tool, ToolInput};
use dashflow::core::Result;
use dashflow::{DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

/// Create an HTTP client with standard timeouts
fn create_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
        .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

const GOOGLE_SEARCH_API_BASE: &str = "https://www.googleapis.com/customsearch/v1";

/// A single search result from Google
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleSearchResult {
    /// Result title
    pub title: String,
    /// Result URL
    pub link: String,
    /// Snippet/description
    pub snippet: String,
    /// Display link
    pub display_link: String,
    /// HTML snippet (if available)
    pub html_snippet: Option<String>,
    /// Formatted URL
    pub formatted_url: Option<String>,
}

/// Search type for Google Custom Search
#[derive(Debug, Clone, Copy, Default)]
pub enum SearchType {
    /// Web search (default)
    #[default]
    Web,
    /// Image search
    Image,
}

impl SearchType {
    fn as_str(&self) -> &str {
        match self {
            SearchType::Web => "web",
            SearchType::Image => "image",
        }
    }
}

/// Safe search level
#[derive(Debug, Clone, Copy, Default)]
pub enum SafeSearchLevel {
    /// No filtering
    Off,
    /// Moderate filtering (default)
    #[default]
    Medium,
    /// Strict filtering
    High,
}

impl SafeSearchLevel {
    fn as_str(&self) -> &str {
        match self {
            SafeSearchLevel::Off => "off",
            SafeSearchLevel::Medium => "medium",
            SafeSearchLevel::High => "high",
        }
    }
}

/// Google Custom Search tool for DashFlow agents
///
/// Searches the web using Google's Custom Search JSON API.
///
/// # Environment Variables
///
/// - `GOOGLE_API_KEY`: Required. Your Google Cloud API key
/// - `GOOGLE_CSE_ID`: Required. Your Programmable Search Engine ID
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_google_search::GoogleSearchTool;
/// use dashflow::core::tools::Tool;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// std::env::set_var("GOOGLE_API_KEY", "your-api-key");
/// std::env::set_var("GOOGLE_CSE_ID", "your-search-engine-id");
///
/// let google = GoogleSearchTool::builder()
///     .num_results(5)
///     .safe_search(dashflow_google_search::SafeSearchLevel::High)
///     .build()?;
///
/// let results = google._call_str("rust programming".to_string()).await?;
/// println!("{}", results);
/// # Ok(())
/// # }
/// ```
pub struct GoogleSearchTool {
    /// Google API key
    api_key: String,
    /// Custom Search Engine ID
    cse_id: String,
    /// Number of results to return
    num_results: usize,
    /// Search type (web or image)
    search_type: SearchType,
    /// Safe search level
    safe_search: SafeSearchLevel,
    /// Restrict to specific site (optional)
    site_restrict: Option<String>,
    /// Language restriction (e.g., "lang_en")
    language: Option<String>,
    /// Country restriction (e.g., "countryUS")
    country: Option<String>,
    /// HTTP client
    client: reqwest::Client,
}

impl GoogleSearchTool {
    /// Create a new Google Search tool
    ///
    /// Reads API key and CSE ID from environment variables.
    ///
    /// # Errors
    ///
    /// Returns an error if required environment variables are not set.
    pub fn new() -> Result<Self> {
        let api_key = env_string(GOOGLE_API_KEY).ok_or_else(|| {
            dashflow::core::Error::tool_error(format!(
                "{GOOGLE_API_KEY} environment variable not set. \
                Get your API key from https://console.cloud.google.com/",
            ))
        })?;

        let cse_id = env_string(GOOGLE_CSE_ID).ok_or_else(|| {
            dashflow::core::Error::tool_error(format!(
                "{GOOGLE_CSE_ID} environment variable not set. \
                Create a search engine at https://programmablesearchengine.google.com/",
            ))
        })?;

        Ok(Self {
            api_key,
            cse_id,
            num_results: 5,
            search_type: SearchType::default(),
            safe_search: SafeSearchLevel::default(),
            site_restrict: None,
            language: None,
            country: None,
            client: create_http_client(),
        })
    }

    /// Create a new Google Search tool with explicit credentials
    #[must_use]
    pub fn with_credentials(api_key: String, cse_id: String) -> Self {
        Self {
            api_key,
            cse_id,
            num_results: 5,
            search_type: SearchType::default(),
            safe_search: SafeSearchLevel::default(),
            site_restrict: None,
            language: None,
            country: None,
            client: create_http_client(),
        }
    }

    /// Create a builder for `GoogleSearchTool`
    pub fn builder() -> GoogleSearchToolBuilder {
        GoogleSearchToolBuilder::default()
    }

    /// Set the number of results to return
    #[must_use]
    pub fn with_num_results(mut self, num_results: usize) -> Self {
        self.num_results = num_results.min(10); // Google API max is 10 per request
        self
    }

    /// Set the search type
    #[must_use]
    pub fn with_search_type(mut self, search_type: SearchType) -> Self {
        self.search_type = search_type;
        self
    }

    /// Set the safe search level
    #[must_use]
    pub fn with_safe_search(mut self, level: SafeSearchLevel) -> Self {
        self.safe_search = level;
        self
    }

    /// Restrict search to a specific site
    #[must_use]
    pub fn with_site_restrict(mut self, site: String) -> Self {
        self.site_restrict = Some(site);
        self
    }

    /// Search Google and return results
    pub async fn search(&self, query: &str) -> Result<Vec<GoogleSearchResult>> {
        let mut url = format!(
            "{}?key={}&cx={}&q={}&num={}&safe={}",
            GOOGLE_SEARCH_API_BASE,
            self.api_key,
            self.cse_id,
            urlencoding::encode(query),
            self.num_results,
            self.safe_search.as_str()
        );

        // Add search type for image search
        if matches!(self.search_type, SearchType::Image) {
            url.push_str(&format!("&searchType={}", self.search_type.as_str()));
        }

        // Add site restriction if set
        if let Some(site) = &self.site_restrict {
            url.push_str(&format!("&siteSearch={}", urlencoding::encode(site)));
        }

        // Add language restriction if set
        if let Some(lang) = &self.language {
            url.push_str(&format!("&lr={}", urlencoding::encode(lang)));
        }

        // Add country restriction if set
        if let Some(country) = &self.country {
            url.push_str(&format!("&cr={}", urlencoding::encode(country)));
        }

        let response =
            self.client.get(&url).send().await.map_err(|e| {
                dashflow::core::Error::http(format!("Google API request failed: {e}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(dashflow::core::Error::tool_error(format!(
                "Google API error ({}): {}",
                status, body
            )));
        }

        // M-216: Use size-limited JSON parsing to prevent memory exhaustion
        let data: serde_json::Value = json_with_limit(response, SEARCH_RESPONSE_SIZE_LIMIT)
            .await
            .map_err(|e| {
                dashflow::core::Error::tool_error(format!(
                    "Failed to parse Google API response: {e}"
                ))
            })?;

        // Check for API errors in the response
        if let Some(error) = data.get("error") {
            let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");
            return Err(dashflow::core::Error::tool_error(format!(
                "Google API error: {message}"
            )));
        }

        let items = data.get("items").and_then(|i| i.as_array());

        let results = items
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        let title = item.get("title")?.as_str()?.to_string();
                        let link = item.get("link")?.as_str()?.to_string();
                        let snippet = item
                            .get("snippet")
                            .and_then(|s| s.as_str())
                            .unwrap_or("")
                            .to_string();
                        let display_link = item
                            .get("displayLink")
                            .and_then(|d| d.as_str())
                            .unwrap_or("")
                            .to_string();
                        let html_snippet = item
                            .get("htmlSnippet")
                            .and_then(|h| h.as_str())
                            .map(|s| s.to_string());
                        let formatted_url = item
                            .get("formattedUrl")
                            .and_then(|f| f.as_str())
                            .map(|s| s.to_string());

                        Some(GoogleSearchResult {
                            title,
                            link,
                            snippet,
                            display_link,
                            html_snippet,
                            formatted_url,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(results)
    }

    /// Format search results as a human-readable string
    fn format_results(&self, results: &[GoogleSearchResult], query: &str) -> String {
        if results.is_empty() {
            return format!("No results found for query: {query}");
        }

        let mut output = format!("Found {} results for query: {}\n\n", results.len(), query);

        for (i, result) in results.iter().enumerate() {
            output.push_str(&format!("Result {}:\n", i + 1));
            output.push_str(&format!("Title: {}\n", result.title));
            output.push_str(&format!("URL: {}\n", result.link));
            output.push_str(&format!("Snippet: {}\n", result.snippet));
            output.push('\n');
        }

        output
    }
}

#[async_trait]
impl Tool for GoogleSearchTool {
    fn name(&self) -> &'static str {
        "google_search"
    }

    fn description(&self) -> &'static str {
        "Search the web using Google. Returns titles, URLs, and snippets from search results. \
         Best for finding information on any topic, getting current information, \
         researching products, finding documentation, and general knowledge queries. \
         Input should be a search query (e.g., 'rust async programming', 'best pizza near me')."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
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

        let results = self.search(&query).await?;
        Ok(self.format_results(&results, &query))
    }
}

/// Builder for `GoogleSearchTool`
#[derive(Default)]
pub struct GoogleSearchToolBuilder {
    api_key: Option<String>,
    cse_id: Option<String>,
    num_results: Option<usize>,
    search_type: Option<SearchType>,
    safe_search: Option<SafeSearchLevel>,
    site_restrict: Option<String>,
    language: Option<String>,
    country: Option<String>,
}

impl GoogleSearchToolBuilder {
    /// Set the API key
    #[must_use]
    pub fn api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    /// Set the Custom Search Engine ID
    #[must_use]
    pub fn cse_id(mut self, cse_id: String) -> Self {
        self.cse_id = Some(cse_id);
        self
    }

    /// Set the number of results to return (max 10)
    #[must_use]
    pub fn num_results(mut self, num_results: usize) -> Self {
        self.num_results = Some(num_results.min(10));
        self
    }

    /// Set the search type
    #[must_use]
    pub fn search_type(mut self, search_type: SearchType) -> Self {
        self.search_type = Some(search_type);
        self
    }

    /// Set the safe search level
    #[must_use]
    pub fn safe_search(mut self, level: SafeSearchLevel) -> Self {
        self.safe_search = Some(level);
        self
    }

    /// Restrict search to a specific site
    #[must_use]
    pub fn site_restrict(mut self, site: String) -> Self {
        self.site_restrict = Some(site);
        self
    }

    /// Set language restriction (e.g., "lang_en")
    #[must_use]
    pub fn language(mut self, language: String) -> Self {
        self.language = Some(language);
        self
    }

    /// Set country restriction (e.g., "countryUS")
    #[must_use]
    pub fn country(mut self, country: String) -> Self {
        self.country = Some(country);
        self
    }

    /// Build the `GoogleSearchTool`
    ///
    /// # Errors
    ///
    /// Returns an error if API key or CSE ID are not provided and not in environment.
    pub fn build(self) -> Result<GoogleSearchTool> {
        let api_key = self
            .api_key
            .or_else(|| env_string(GOOGLE_API_KEY))
            .ok_or_else(|| {
                dashflow::core::Error::tool_error(format!(
                    "{GOOGLE_API_KEY} not provided. Set it via builder or environment variable."
                ))
            })?;

        let cse_id = self
            .cse_id
            .or_else(|| env_string(GOOGLE_CSE_ID))
            .ok_or_else(|| {
                dashflow::core::Error::tool_error(format!(
                    "{GOOGLE_CSE_ID} not provided. Set it via builder or environment variable."
                ))
            })?;

        Ok(GoogleSearchTool {
            api_key,
            cse_id,
            num_results: self.num_results.unwrap_or(5).min(10),
            search_type: self.search_type.unwrap_or_default(),
            safe_search: self.safe_search.unwrap_or_default(),
            site_restrict: self.site_restrict,
            language: self.language,
            country: self.country,
            client: create_http_client(),
        })
    }
}

/// Google Search retriever for document retrieval
///
/// Wraps `GoogleSearchTool` and converts search results into Documents
/// suitable for use in retrieval chains and RAG applications.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_google_search::GoogleSearchRetriever;
/// use dashflow::core::retrievers::Retriever;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// std::env::set_var("GOOGLE_API_KEY", "your-api-key");
/// std::env::set_var("GOOGLE_CSE_ID", "your-search-engine-id");
///
/// let retriever = GoogleSearchRetriever::builder()
///     .num_results(5)
///     .build()?;
///
/// let docs = retriever._get_relevant_documents("rust programming", None).await?;
/// # Ok(())
/// # }
/// ```
pub struct GoogleSearchRetriever {
    /// Internal search tool
    tool: GoogleSearchTool,
}

impl GoogleSearchRetriever {
    /// Create a new `GoogleSearchRetriever`
    ///
    /// Reads credentials from environment variables.
    pub fn new() -> Result<Self> {
        Ok(Self {
            tool: GoogleSearchTool::new()?,
        })
    }

    /// Create a new `GoogleSearchRetriever` with explicit credentials
    #[must_use]
    pub fn with_credentials(api_key: String, cse_id: String) -> Self {
        Self {
            tool: GoogleSearchTool::with_credentials(api_key, cse_id),
        }
    }

    /// Create a builder for `GoogleSearchRetriever`
    pub fn builder() -> GoogleSearchRetrieverBuilder {
        GoogleSearchRetrieverBuilder::default()
    }

    /// Convert `GoogleSearchResult` to Document
    fn result_to_document(result: &GoogleSearchResult) -> Document {
        let mut metadata = HashMap::new();

        metadata.insert(
            "source".to_string(),
            serde_json::Value::String(result.link.clone()),
        );
        metadata.insert(
            "title".to_string(),
            serde_json::Value::String(result.title.clone()),
        );
        metadata.insert(
            "link".to_string(),
            serde_json::Value::String(result.link.clone()),
        );
        metadata.insert(
            "display_link".to_string(),
            serde_json::Value::String(result.display_link.clone()),
        );

        if let Some(formatted_url) = &result.formatted_url {
            metadata.insert(
                "formatted_url".to_string(),
                serde_json::Value::String(formatted_url.clone()),
            );
        }

        // Page content is the snippet
        let page_content = format!("{}\n\n{}", result.title, result.snippet);

        Document {
            page_content,
            metadata,
            id: Some(result.link.clone()),
        }
    }
}

#[async_trait]
impl Retriever for GoogleSearchRetriever {
    async fn _get_relevant_documents(
        &self,
        query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        let results = self.tool.search(query).await?;

        Ok(results.iter().map(Self::result_to_document).collect())
    }
}

/// Builder for `GoogleSearchRetriever`
#[derive(Default)]
pub struct GoogleSearchRetrieverBuilder {
    api_key: Option<String>,
    cse_id: Option<String>,
    num_results: Option<usize>,
    search_type: Option<SearchType>,
    safe_search: Option<SafeSearchLevel>,
    site_restrict: Option<String>,
}

impl GoogleSearchRetrieverBuilder {
    /// Set the API key
    #[must_use]
    pub fn api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    /// Set the Custom Search Engine ID
    #[must_use]
    pub fn cse_id(mut self, cse_id: String) -> Self {
        self.cse_id = Some(cse_id);
        self
    }

    /// Set the number of results
    #[must_use]
    pub fn num_results(mut self, num_results: usize) -> Self {
        self.num_results = Some(num_results);
        self
    }

    /// Set the search type
    #[must_use]
    pub fn search_type(mut self, search_type: SearchType) -> Self {
        self.search_type = Some(search_type);
        self
    }

    /// Set the safe search level
    #[must_use]
    pub fn safe_search(mut self, level: SafeSearchLevel) -> Self {
        self.safe_search = Some(level);
        self
    }

    /// Restrict search to a specific site
    #[must_use]
    pub fn site_restrict(mut self, site: String) -> Self {
        self.site_restrict = Some(site);
        self
    }

    /// Build the `GoogleSearchRetriever`
    pub fn build(self) -> Result<GoogleSearchRetriever> {
        let mut builder = GoogleSearchToolBuilder::default();

        if let Some(api_key) = self.api_key {
            builder = builder.api_key(api_key);
        }
        if let Some(cse_id) = self.cse_id {
            builder = builder.cse_id(cse_id);
        }
        if let Some(num_results) = self.num_results {
            builder = builder.num_results(num_results);
        }
        if let Some(search_type) = self.search_type {
            builder = builder.search_type(search_type);
        }
        if let Some(safe_search) = self.safe_search {
            builder = builder.safe_search(safe_search);
        }
        if let Some(site) = self.site_restrict {
            builder = builder.site_restrict(site);
        }

        Ok(GoogleSearchRetriever {
            tool: builder.build()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    // ==================== Credential and Construction Tests ====================

    #[test]
    fn test_tool_creation_fails_without_credentials() {
        env::remove_var("GOOGLE_API_KEY");
        env::remove_var("GOOGLE_CSE_ID");

        let result = GoogleSearchTool::new();
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_with_credentials() {
        let tool =
            GoogleSearchTool::with_credentials("test-key".to_string(), "test-cse".to_string());
        assert_eq!(tool.name(), "google_search");
        assert!(tool.description().contains("Google"));
        assert_eq!(tool.num_results, 5);
    }

    #[test]
    fn test_builder_with_credentials() {
        let result = GoogleSearchTool::builder()
            .api_key("test-key".to_string())
            .cse_id("test-cse".to_string())
            .num_results(10)
            .search_type(SearchType::Web)
            .safe_search(SafeSearchLevel::High)
            .build();

        assert!(result.is_ok());
        let tool = result.unwrap();
        assert_eq!(tool.num_results, 10);
    }

    #[test]
    fn test_builder_fails_without_credentials() {
        env::remove_var("GOOGLE_API_KEY");
        env::remove_var("GOOGLE_CSE_ID");

        let result = GoogleSearchTool::builder().num_results(10).build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_fails_with_only_api_key() {
        env::remove_var("GOOGLE_API_KEY");
        env::remove_var("GOOGLE_CSE_ID");

        let result = GoogleSearchTool::builder()
            .api_key("test-key".to_string())
            .build();
        assert!(result.is_err());
        let err = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("Expected error"),
        };
        assert!(err.contains("GOOGLE_CSE_ID"));
    }

    #[test]
    fn test_builder_fails_with_only_cse_id() {
        env::remove_var("GOOGLE_API_KEY");
        env::remove_var("GOOGLE_CSE_ID");

        let result = GoogleSearchTool::builder()
            .cse_id("test-cse".to_string())
            .build();
        assert!(result.is_err());
        let err = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("Expected error"),
        };
        assert!(err.contains("GOOGLE_API_KEY"));
    }

    #[test]
    fn test_error_message_contains_helpful_url() {
        env::remove_var("GOOGLE_API_KEY");
        env::remove_var("GOOGLE_CSE_ID");

        let result = GoogleSearchTool::new();
        let err = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("Expected error"),
        };
        assert!(err.contains("console.cloud.google.com"));
    }

    // ==================== Num Results Capping Tests ====================

    #[test]
    fn test_num_results_capped_at_10() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string())
            .with_num_results(20);
        assert_eq!(tool.num_results, 10);
    }

    #[test]
    fn test_num_results_zero() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string())
            .with_num_results(0);
        assert_eq!(tool.num_results, 0);
    }

    #[test]
    fn test_num_results_one() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string())
            .with_num_results(1);
        assert_eq!(tool.num_results, 1);
    }

    #[test]
    fn test_num_results_exactly_10() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string())
            .with_num_results(10);
        assert_eq!(tool.num_results, 10);
    }

    #[test]
    fn test_num_results_11_capped() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string())
            .with_num_results(11);
        assert_eq!(tool.num_results, 10);
    }

    #[test]
    fn test_num_results_large_value_capped() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string())
            .with_num_results(1000);
        assert_eq!(tool.num_results, 10);
    }

    #[test]
    fn test_builder_num_results_capped() {
        let tool = GoogleSearchTool::builder()
            .api_key("key".to_string())
            .cse_id("cse".to_string())
            .num_results(50)
            .build()
            .unwrap();
        assert_eq!(tool.num_results, 10);
    }

    // ==================== SearchType Enum Tests ====================

    #[test]
    fn test_search_type_as_str() {
        assert_eq!(SearchType::Web.as_str(), "web");
        assert_eq!(SearchType::Image.as_str(), "image");
    }

    #[test]
    fn test_search_type_default() {
        let default_type = SearchType::default();
        assert_eq!(default_type.as_str(), "web");
    }

    #[test]
    fn test_search_type_copy() {
        let st = SearchType::Image;
        let st_copy = st;
        assert_eq!(st.as_str(), st_copy.as_str());
    }

    #[test]
    fn test_search_type_clone() {
        let st = SearchType::Web;
        let st_clone = st.clone();
        assert_eq!(st.as_str(), st_clone.as_str());
    }

    #[test]
    fn test_search_type_debug() {
        let st = SearchType::Image;
        let debug_str = format!("{:?}", st);
        assert!(debug_str.contains("Image"));
    }

    // ==================== SafeSearchLevel Enum Tests ====================

    #[test]
    fn test_safe_search_as_str() {
        assert_eq!(SafeSearchLevel::Off.as_str(), "off");
        assert_eq!(SafeSearchLevel::Medium.as_str(), "medium");
        assert_eq!(SafeSearchLevel::High.as_str(), "high");
    }

    #[test]
    fn test_safe_search_default() {
        let default_level = SafeSearchLevel::default();
        assert_eq!(default_level.as_str(), "medium");
    }

    #[test]
    fn test_safe_search_copy() {
        let ssl = SafeSearchLevel::High;
        let ssl_copy = ssl;
        assert_eq!(ssl.as_str(), ssl_copy.as_str());
    }

    #[test]
    fn test_safe_search_clone() {
        let ssl = SafeSearchLevel::Off;
        let ssl_clone = ssl.clone();
        assert_eq!(ssl.as_str(), ssl_clone.as_str());
    }

    #[test]
    fn test_safe_search_debug() {
        let ssl = SafeSearchLevel::Medium;
        let debug_str = format!("{:?}", ssl);
        assert!(debug_str.contains("Medium"));
    }

    // ==================== GoogleSearchResult Tests ====================

    #[test]
    fn test_google_search_result_clone() {
        let result = GoogleSearchResult {
            title: "Test".to_string(),
            link: "https://example.com".to_string(),
            snippet: "Snippet".to_string(),
            display_link: "example.com".to_string(),
            html_snippet: Some("<b>Snippet</b>".to_string()),
            formatted_url: Some("https://example.com/".to_string()),
        };
        let cloned = result.clone();
        assert_eq!(result.title, cloned.title);
        assert_eq!(result.link, cloned.link);
        assert_eq!(result.html_snippet, cloned.html_snippet);
    }

    #[test]
    fn test_google_search_result_debug() {
        let result = GoogleSearchResult {
            title: "Test".to_string(),
            link: "https://example.com".to_string(),
            snippet: "Snippet".to_string(),
            display_link: "example.com".to_string(),
            html_snippet: None,
            formatted_url: None,
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("Test"));
        assert!(debug_str.contains("example.com"));
    }

    #[test]
    fn test_google_search_result_serialize() {
        let result = GoogleSearchResult {
            title: "Test Title".to_string(),
            link: "https://example.com".to_string(),
            snippet: "A snippet".to_string(),
            display_link: "example.com".to_string(),
            html_snippet: None,
            formatted_url: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("Test Title"));
        assert!(json.contains("https://example.com"));
    }

    #[test]
    fn test_google_search_result_deserialize() {
        let json = r#"{
            "title": "Deserialized Title",
            "link": "https://test.com",
            "snippet": "Some snippet",
            "display_link": "test.com",
            "html_snippet": "<b>Some</b>",
            "formatted_url": "https://test.com/"
        }"#;
        let result: GoogleSearchResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.title, "Deserialized Title");
        assert_eq!(result.link, "https://test.com");
        assert_eq!(result.html_snippet, Some("<b>Some</b>".to_string()));
    }

    #[test]
    fn test_google_search_result_deserialize_minimal() {
        let json = r#"{
            "title": "Minimal",
            "link": "https://min.com",
            "snippet": "",
            "display_link": "min.com"
        }"#;
        let result: GoogleSearchResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.title, "Minimal");
        assert!(result.html_snippet.is_none());
        assert!(result.formatted_url.is_none());
    }

    #[test]
    fn test_google_search_result_with_special_chars() {
        let result = GoogleSearchResult {
            title: "Title with <script> & \"quotes\"".to_string(),
            link: "https://example.com/path?q=test&foo=bar".to_string(),
            snippet: "Snippet with\nnewlines\tand tabs".to_string(),
            display_link: "example.com".to_string(),
            html_snippet: Some("<b>Bold</b> &amp; <i>italic</i>".to_string()),
            formatted_url: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: GoogleSearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.title, deserialized.title);
        assert_eq!(result.link, deserialized.link);
    }

    // ==================== Tool Trait Tests ====================

    #[test]
    fn test_args_schema() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string());
        let schema = tool.args_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
        assert_eq!(schema["required"][0], "query");
    }

    #[test]
    fn test_args_schema_query_description() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string());
        let schema = tool.args_schema();
        let query_desc = schema["properties"]["query"]["description"]
            .as_str()
            .unwrap();
        assert!(query_desc.contains("query"));
    }

    #[test]
    fn test_tool_name_is_google_search() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string());
        assert_eq!(tool.name(), "google_search");
    }

    #[test]
    fn test_tool_description_mentions_web() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string());
        let desc = tool.description();
        assert!(desc.contains("web") || desc.contains("Web"));
    }

    #[test]
    fn test_tool_description_mentions_google() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string());
        let desc = tool.description();
        assert!(desc.contains("Google"));
    }

    // ==================== Format Results Tests ====================

    #[test]
    fn test_format_results_empty() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string());
        let result = tool.format_results(&[], "test query");
        assert!(result.contains("No results found"));
        assert!(result.contains("test query"));
    }

    #[test]
    fn test_format_results_with_data() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string());
        let results = vec![GoogleSearchResult {
            title: "Test Title".to_string(),
            link: "https://example.com".to_string(),
            snippet: "Test snippet".to_string(),
            display_link: "example.com".to_string(),
            html_snippet: None,
            formatted_url: None,
        }];

        let result = tool.format_results(&results, "test query");

        assert!(result.contains("Found 1 results"));
        assert!(result.contains("Test Title"));
        assert!(result.contains("https://example.com"));
    }

    #[test]
    fn test_format_results_multiple() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string());
        let results = vec![
            GoogleSearchResult {
                title: "First".to_string(),
                link: "https://first.com".to_string(),
                snippet: "First snippet".to_string(),
                display_link: "first.com".to_string(),
                html_snippet: None,
                formatted_url: None,
            },
            GoogleSearchResult {
                title: "Second".to_string(),
                link: "https://second.com".to_string(),
                snippet: "Second snippet".to_string(),
                display_link: "second.com".to_string(),
                html_snippet: None,
                formatted_url: None,
            },
            GoogleSearchResult {
                title: "Third".to_string(),
                link: "https://third.com".to_string(),
                snippet: "Third snippet".to_string(),
                display_link: "third.com".to_string(),
                html_snippet: None,
                formatted_url: None,
            },
        ];

        let result = tool.format_results(&results, "test");

        assert!(result.contains("Found 3 results"));
        assert!(result.contains("Result 1:"));
        assert!(result.contains("Result 2:"));
        assert!(result.contains("Result 3:"));
        assert!(result.contains("First"));
        assert!(result.contains("Second"));
        assert!(result.contains("Third"));
    }

    #[test]
    fn test_format_results_preserves_special_characters() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string());
        let results = vec![GoogleSearchResult {
            title: "Title with <html> & \"special\" chars".to_string(),
            link: "https://example.com/path?a=1&b=2".to_string(),
            snippet: "Snippet with\nnewline".to_string(),
            display_link: "example.com".to_string(),
            html_snippet: None,
            formatted_url: None,
        }];

        let result = tool.format_results(&results, "special");

        assert!(result.contains("<html>"));
        assert!(result.contains("&"));
        assert!(result.contains("\"special\""));
    }

    // ==================== Document Conversion Tests ====================

    #[test]
    fn test_result_to_document() {
        let result = GoogleSearchResult {
            title: "Test Title".to_string(),
            link: "https://example.com".to_string(),
            snippet: "Test snippet content".to_string(),
            display_link: "example.com".to_string(),
            html_snippet: None,
            formatted_url: Some("https://example.com/page".to_string()),
        };

        let doc = GoogleSearchRetriever::result_to_document(&result);

        assert!(doc.page_content.contains("Test Title"));
        assert!(doc.page_content.contains("Test snippet"));
        assert_eq!(
            doc.metadata.get("link"),
            Some(&serde_json::Value::String(
                "https://example.com".to_string()
            ))
        );
        assert_eq!(doc.id, Some("https://example.com".to_string()));
    }

    #[test]
    fn test_result_to_document_metadata_fields() {
        let result = GoogleSearchResult {
            title: "Title".to_string(),
            link: "https://example.com".to_string(),
            snippet: "Snippet".to_string(),
            display_link: "example.com".to_string(),
            html_snippet: None,
            formatted_url: Some("https://example.com/formatted".to_string()),
        };

        let doc = GoogleSearchRetriever::result_to_document(&result);

        assert!(doc.metadata.contains_key("source"));
        assert!(doc.metadata.contains_key("title"));
        assert!(doc.metadata.contains_key("link"));
        assert!(doc.metadata.contains_key("display_link"));
        assert!(doc.metadata.contains_key("formatted_url"));
    }

    #[test]
    fn test_result_to_document_without_formatted_url() {
        let result = GoogleSearchResult {
            title: "Title".to_string(),
            link: "https://example.com".to_string(),
            snippet: "Snippet".to_string(),
            display_link: "example.com".to_string(),
            html_snippet: None,
            formatted_url: None,
        };

        let doc = GoogleSearchRetriever::result_to_document(&result);

        assert!(!doc.metadata.contains_key("formatted_url"));
    }

    #[test]
    fn test_result_to_document_page_content_format() {
        let result = GoogleSearchResult {
            title: "My Title".to_string(),
            link: "https://example.com".to_string(),
            snippet: "My snippet text".to_string(),
            display_link: "example.com".to_string(),
            html_snippet: None,
            formatted_url: None,
        };

        let doc = GoogleSearchRetriever::result_to_document(&result);

        // Format is "{title}\n\n{snippet}"
        assert!(doc.page_content.starts_with("My Title"));
        assert!(doc.page_content.contains("\n\n"));
        assert!(doc.page_content.ends_with("My snippet text"));
    }

    #[test]
    fn test_result_to_document_source_equals_link() {
        let result = GoogleSearchResult {
            title: "Title".to_string(),
            link: "https://specific-link.com/page".to_string(),
            snippet: "Snippet".to_string(),
            display_link: "specific-link.com".to_string(),
            html_snippet: None,
            formatted_url: None,
        };

        let doc = GoogleSearchRetriever::result_to_document(&result);

        assert_eq!(
            doc.metadata.get("source"),
            Some(&serde_json::Value::String(
                "https://specific-link.com/page".to_string()
            ))
        );
    }

    // ==================== Retriever Tests ====================

    #[test]
    fn test_retriever_creation_fails_without_credentials() {
        env::remove_var("GOOGLE_API_KEY");
        env::remove_var("GOOGLE_CSE_ID");

        let result = GoogleSearchRetriever::new();
        assert!(result.is_err());
    }

    #[test]
    fn test_retriever_with_credentials() {
        let retriever =
            GoogleSearchRetriever::with_credentials("key".to_string(), "cse".to_string());
        assert_eq!(retriever.tool.num_results, 5);
    }

    #[test]
    fn test_retriever_builder() {
        let result = GoogleSearchRetriever::builder()
            .api_key("key".to_string())
            .cse_id("cse".to_string())
            .num_results(3)
            .build();

        assert!(result.is_ok());
        let retriever = result.unwrap();
        assert_eq!(retriever.tool.num_results, 3);
    }

    #[test]
    fn test_retriever_builder_all_options() {
        let result = GoogleSearchRetriever::builder()
            .api_key("key".to_string())
            .cse_id("cse".to_string())
            .num_results(7)
            .search_type(SearchType::Image)
            .safe_search(SafeSearchLevel::High)
            .site_restrict("example.com".to_string())
            .build();

        assert!(result.is_ok());
        let retriever = result.unwrap();
        assert_eq!(retriever.tool.num_results, 7);
    }

    #[test]
    fn test_retriever_builder_fails_without_credentials() {
        env::remove_var("GOOGLE_API_KEY");
        env::remove_var("GOOGLE_CSE_ID");

        let result = GoogleSearchRetriever::builder().num_results(5).build();
        assert!(result.is_err());
    }

    // ==================== Builder Chain Tests ====================

    #[test]
    fn test_tool_builder_chain_all_methods() {
        let tool = GoogleSearchTool::builder()
            .api_key("key".to_string())
            .cse_id("cse".to_string())
            .num_results(5)
            .search_type(SearchType::Web)
            .safe_search(SafeSearchLevel::Medium)
            .site_restrict("rust-lang.org".to_string())
            .language("lang_en".to_string())
            .country("countryUS".to_string())
            .build()
            .unwrap();

        assert_eq!(tool.num_results, 5);
        assert!(tool.site_restrict.is_some());
        assert!(tool.language.is_some());
        assert!(tool.country.is_some());
    }

    #[test]
    fn test_tool_with_methods_chain() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string())
            .with_num_results(7)
            .with_search_type(SearchType::Image)
            .with_safe_search(SafeSearchLevel::Off)
            .with_site_restrict("docs.rs".to_string());

        assert_eq!(tool.num_results, 7);
        assert!(tool.site_restrict.is_some());
        assert_eq!(tool.site_restrict.unwrap(), "docs.rs");
    }

    // ==================== Type and Trait Bound Tests ====================

    #[test]
    fn test_google_search_tool_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<GoogleSearchTool>();
    }

    #[test]
    fn test_google_search_tool_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<GoogleSearchTool>();
    }

    #[test]
    fn test_google_search_retriever_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<GoogleSearchRetriever>();
    }

    #[test]
    fn test_google_search_retriever_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<GoogleSearchRetriever>();
    }

    #[test]
    fn test_google_search_result_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<GoogleSearchResult>();
    }

    #[test]
    fn test_google_search_result_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<GoogleSearchResult>();
    }

    #[test]
    fn test_builder_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<GoogleSearchToolBuilder>();
    }

    #[test]
    fn test_retriever_builder_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<GoogleSearchRetrieverBuilder>();
    }

    // ==================== Default Values Tests ====================

    #[test]
    fn test_tool_default_num_results() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string());
        assert_eq!(tool.num_results, 5);
    }

    #[test]
    fn test_tool_default_search_type() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string());
        assert_eq!(tool.search_type.as_str(), "web");
    }

    #[test]
    fn test_tool_default_safe_search() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string());
        assert_eq!(tool.safe_search.as_str(), "medium");
    }

    #[test]
    fn test_tool_default_no_site_restrict() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string());
        assert!(tool.site_restrict.is_none());
    }

    #[test]
    fn test_tool_default_no_language() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string());
        assert!(tool.language.is_none());
    }

    #[test]
    fn test_tool_default_no_country() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string());
        assert!(tool.country.is_none());
    }

    #[test]
    fn test_builder_default() {
        let builder = GoogleSearchToolBuilder::default();
        // Should have all None values
        assert!(builder.api_key.is_none());
        assert!(builder.cse_id.is_none());
        assert!(builder.num_results.is_none());
    }

    #[test]
    fn test_retriever_builder_default() {
        let builder = GoogleSearchRetrieverBuilder::default();
        assert!(builder.api_key.is_none());
        assert!(builder.cse_id.is_none());
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_empty_api_key_string() {
        let tool = GoogleSearchTool::with_credentials("".to_string(), "cse".to_string());
        // Empty string is allowed at construction time, would fail at API call
        assert_eq!(tool.api_key, "");
    }

    #[test]
    fn test_empty_cse_id_string() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "".to_string());
        assert_eq!(tool.cse_id, "");
    }

    #[test]
    fn test_unicode_credentials() {
        let tool = GoogleSearchTool::with_credentials(
            "键值-with-中文".to_string(),
            "引擎ID-αβγ".to_string(),
        );
        assert_eq!(tool.api_key, "键值-with-中文");
        assert_eq!(tool.cse_id, "引擎ID-αβγ");
    }

    #[test]
    fn test_site_restrict_with_unicode() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string())
            .with_site_restrict("例え.jp".to_string());
        assert_eq!(tool.site_restrict, Some("例え.jp".to_string()));
    }

    #[test]
    fn test_format_results_empty_query() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string());
        let result = tool.format_results(&[], "");
        assert!(result.contains("No results found"));
    }

    #[test]
    fn test_format_results_unicode_query() {
        let tool = GoogleSearchTool::with_credentials("key".to_string(), "cse".to_string());
        let result = tool.format_results(&[], "日本語クエリ 🔍");
        assert!(result.contains("日本語クエリ"));
        assert!(result.contains("🔍"));
    }

    // ==================== API Constant Tests ====================

    #[test]
    fn test_api_base_url_is_https() {
        assert!(GOOGLE_SEARCH_API_BASE.starts_with("https://"));
    }

    #[test]
    fn test_api_base_url_contains_googleapis() {
        assert!(GOOGLE_SEARCH_API_BASE.contains("googleapis.com"));
    }

    // ==================== Integration test (requires API key) ====================

    #[tokio::test]
    #[ignore = "requires GOOGLE_API_KEY and GOOGLE_CSE_ID environment variables"]
    async fn test_search_integration() {
        let tool = GoogleSearchTool::new().unwrap();
        let results = tool
            .search("rust programming language")
            .await
            .expect("Google search failed");

        assert!(!results.is_empty());
        assert!(!results[0].title.is_empty());
        assert!(!results[0].link.is_empty());
    }
}
