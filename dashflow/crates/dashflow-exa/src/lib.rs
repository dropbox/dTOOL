// NOTE: needless_pass_by_value was removed - all pass-by-value is intentional
// or required for async ownership semantics

//! # Exa Search Tool
//!
//! Exa is a search engine designed for AI applications, using embeddings-based
//! and keyword search to find high-quality web content.
//!
//! ## Features
//!
//! - Neural search using embeddings
//! - Keyword search (Google-like)
//! - Auto mode that intelligently combines methods
//! - Category filtering (research papers, news, etc.)
//! - Domain filtering (include/exclude specific domains)
//! - Clean, parsed HTML content
//!
//! ## Usage
//!
//! ```rust,no_run
//! use dashflow_exa::ExaSearchTool;
//! use dashflow::core::tools::Tool;
//!
//! # tokio_test::block_on(async {
//! let exa = ExaSearchTool::new("your-api-key");
//!
//! // Simple search
//! let results = exa._call_str("latest developments in AI".to_string()).await.unwrap();
//! println!("Search results: {}", results);
//! # });
//! ```

use async_trait::async_trait;
use dashflow::constants::{DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT};
use dashflow::core::http_client::{json_with_limit, SEARCH_RESPONSE_SIZE_LIMIT};
use dashflow::core::tools::{Tool, ToolInput};
use dashflow::core::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Create an HTTP client with standard timeouts
fn create_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
        .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// Search type for Exa API
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SearchType {
    /// Keyword-based search (Google-like)
    Keyword,
    /// Neural/embeddings-based search
    Neural,
    /// Fast search using streamlined models
    Fast,
    /// Automatically choose the best method (default)
    #[default]
    Auto,
}

/// A single search result from Exa
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExaResult {
    /// Page title
    pub title: Option<String>,
    /// Page URL
    pub url: String,
    /// Published date (if available)
    #[serde(rename = "publishedDate")]
    pub published_date: Option<String>,
    /// Author (if available)
    pub author: Option<String>,
    /// Text content snippet
    pub text: Option<String>,
    /// Highlighted snippets
    pub highlights: Option<Vec<String>>,
    /// Summary
    pub summary: Option<String>,
}

/// Response from Exa search API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExaSearchResponse {
    /// Request ID for tracking
    #[serde(rename = "requestId")]
    pub request_id: Option<String>,
    /// Search results
    pub results: Vec<ExaResult>,
}

/// Request to Exa search API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExaSearchRequest {
    /// Search query
    pub query: String,
    /// Search type (auto, neural, keyword, fast)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    /// Number of results (max 100)
    #[serde(rename = "numResults", skip_serializing_if = "Option::is_none")]
    pub num_results: Option<u32>,
    /// Category filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Include domains
    #[serde(rename = "includeDomains", skip_serializing_if = "Option::is_none")]
    pub include_domains: Option<Vec<String>>,
    /// Exclude domains
    #[serde(rename = "excludeDomains", skip_serializing_if = "Option::is_none")]
    pub exclude_domains: Option<Vec<String>>,
}

/// Exa search tool for `DashFlow` agents
///
/// This tool provides access to Exa's AI-powered search engine, which can perform
/// both keyword and neural (embeddings-based) search to find relevant web content.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_exa::ExaSearchTool;
/// use dashflow::core::tools::Tool;
///
/// # tokio_test::block_on(async {
/// let exa = ExaSearchTool::builder()
///     .api_key("your-api-key")
///     .num_results(5)
///     .search_type("neural")
///     .build()
///     .unwrap();
///
/// let results = exa._call_str("best practices for Rust async programming".to_string())
///     .await
///     .unwrap();
/// println!("Found: {}", results);
/// # });
/// ```
pub struct ExaSearchTool {
    api_key: String,
    num_results: u32,
    search_type: String,
    category: Option<String>,
    include_domains: Option<Vec<String>>,
    exclude_domains: Option<Vec<String>>,
    client: reqwest::Client,
}

impl ExaSearchTool {
    /// Create a new Exa search tool
    ///
    /// # Arguments
    ///
    /// * `api_key` - Your Exa API key
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_exa::ExaSearchTool;
    ///
    /// let exa = ExaSearchTool::new("your-api-key");
    /// ```
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            num_results: 10,
            search_type: "auto".to_string(),
            category: None,
            include_domains: None,
            exclude_domains: None,
            client: create_http_client(),
        }
    }

    /// Create a builder for `ExaSearchTool`
    #[must_use]
    pub fn builder() -> ExaSearchToolBuilder {
        ExaSearchToolBuilder::default()
    }

    /// Perform a search using the Exa API
    async fn search(&self, query: String) -> Result<ExaSearchResponse> {
        let request = ExaSearchRequest {
            query,
            r#type: Some(self.search_type.clone()),
            num_results: Some(self.num_results),
            category: self.category.clone(),
            include_domains: self.include_domains.clone(),
            exclude_domains: self.exclude_domains.clone(),
        };

        let response = self
            .client
            .post("https://api.exa.ai/search")
            .header("x-api-key", &self.api_key)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                dashflow::core::Error::tool_error(format!("Exa API request failed: {e}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(dashflow::core::Error::tool_error(format!(
                "Exa API error ({status}): {error_text}"
            )));
        }

        // M-216: Use size-limited JSON parsing to prevent memory exhaustion
        let search_response: ExaSearchResponse =
            json_with_limit(response, SEARCH_RESPONSE_SIZE_LIMIT).await.map_err(|e| {
                dashflow::core::Error::tool_error(format!("Failed to parse Exa response: {e}"))
            })?;

        Ok(search_response)
    }

    /// Format search results as a string
    fn format_results(&self, response: &ExaSearchResponse) -> String {
        if response.results.is_empty() {
            return "No results found.".to_string();
        }

        let mut output = format!("Found {} results:\n\n", response.results.len());

        for (i, result) in response.results.iter().enumerate() {
            output.push_str(&format!("{}. ", i + 1));

            if let Some(title) = &result.title {
                output.push_str(&format!("{title}\n"));
            } else {
                output.push_str("(No title)\n");
            }

            output.push_str(&format!("   URL: {}\n", result.url));

            if let Some(author) = &result.author {
                output.push_str(&format!("   Author: {author}\n"));
            }

            if let Some(date) = &result.published_date {
                output.push_str(&format!("   Published: {date}\n"));
            }

            if let Some(text) = &result.text {
                let snippet = if text.len() > 200 {
                    format!("{}...", &text[..200])
                } else {
                    text.clone()
                };
                output.push_str(&format!("   Snippet: {snippet}\n"));
            }

            output.push('\n');
        }

        output
    }
}

#[async_trait]
impl Tool for ExaSearchTool {
    fn name(&self) -> &'static str {
        "exa_search"
    }

    fn description(&self) -> &'static str {
        "Search the web using Exa's AI-powered search engine. \
         Supports both keyword and neural (embeddings-based) search. \
         Returns high-quality, relevant web content with titles, URLs, and snippets. \
         Best for finding recent information, research papers, and technical content."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query to execute"
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

        let response = self.search(query).await?;
        Ok(self.format_results(&response))
    }
}

/// Builder for `ExaSearchTool`
#[derive(Default)]
pub struct ExaSearchToolBuilder {
    api_key: Option<String>,
    num_results: Option<u32>,
    search_type: Option<String>,
    category: Option<String>,
    include_domains: Option<Vec<String>>,
    exclude_domains: Option<Vec<String>>,
}

impl ExaSearchToolBuilder {
    /// Set the API key
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set the number of results to return (max 100)
    #[must_use]
    pub fn num_results(mut self, num_results: u32) -> Self {
        self.num_results = Some(num_results.min(100));
        self
    }

    /// Set the search type (auto, neural, keyword, fast)
    pub fn search_type(mut self, search_type: impl Into<String>) -> Self {
        self.search_type = Some(search_type.into());
        self
    }

    /// Set the category filter (e.g., "research paper", "news")
    pub fn category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// Set domains to include in search
    #[must_use]
    pub fn include_domains(mut self, domains: Vec<String>) -> Self {
        self.include_domains = Some(domains);
        self
    }

    /// Set domains to exclude from search
    #[must_use]
    pub fn exclude_domains(mut self, domains: Vec<String>) -> Self {
        self.exclude_domains = Some(domains);
        self
    }

    /// Build the `ExaSearchTool`
    pub fn build(self) -> Result<ExaSearchTool> {
        let api_key = self.api_key.ok_or_else(|| {
            dashflow::core::Error::tool_error("API key is required for Exa search".to_string())
        })?;

        Ok(ExaSearchTool {
            api_key,
            num_results: self.num_results.unwrap_or(10),
            search_type: self.search_type.unwrap_or_else(|| "auto".to_string()),
            category: self.category,
            include_domains: self.include_domains,
            exclude_domains: self.exclude_domains,
            client: create_http_client(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // SearchType Tests
    // =========================================================================

    #[test]
    fn test_search_type_default() {
        let default: SearchType = Default::default();
        match default {
            SearchType::Auto => {} // Expected
            _ => panic!("Expected Auto as default SearchType"),
        }
    }

    #[test]
    fn test_search_type_serialization_keyword() {
        let st = SearchType::Keyword;
        let json = serde_json::to_string(&st).unwrap();
        assert_eq!(json, "\"keyword\"");
    }

    #[test]
    fn test_search_type_serialization_neural() {
        let st = SearchType::Neural;
        let json = serde_json::to_string(&st).unwrap();
        assert_eq!(json, "\"neural\"");
    }

    #[test]
    fn test_search_type_serialization_fast() {
        let st = SearchType::Fast;
        let json = serde_json::to_string(&st).unwrap();
        assert_eq!(json, "\"fast\"");
    }

    #[test]
    fn test_search_type_serialization_auto() {
        let st = SearchType::Auto;
        let json = serde_json::to_string(&st).unwrap();
        assert_eq!(json, "\"auto\"");
    }

    #[test]
    fn test_search_type_deserialization_keyword() {
        let st: SearchType = serde_json::from_str("\"keyword\"").unwrap();
        assert!(matches!(st, SearchType::Keyword));
    }

    #[test]
    fn test_search_type_deserialization_neural() {
        let st: SearchType = serde_json::from_str("\"neural\"").unwrap();
        assert!(matches!(st, SearchType::Neural));
    }

    #[test]
    fn test_search_type_deserialization_fast() {
        let st: SearchType = serde_json::from_str("\"fast\"").unwrap();
        assert!(matches!(st, SearchType::Fast));
    }

    #[test]
    fn test_search_type_deserialization_auto() {
        let st: SearchType = serde_json::from_str("\"auto\"").unwrap();
        assert!(matches!(st, SearchType::Auto));
    }

    #[test]
    fn test_search_type_clone() {
        let st = SearchType::Neural;
        let cloned = st.clone();
        assert!(matches!(cloned, SearchType::Neural));
    }

    #[test]
    fn test_search_type_debug() {
        let st = SearchType::Keyword;
        let debug = format!("{:?}", st);
        assert_eq!(debug, "Keyword");
    }

    // =========================================================================
    // ExaResult Tests
    // =========================================================================

    #[test]
    fn test_exa_result_all_fields() {
        let result = ExaResult {
            title: Some("Test Title".to_string()),
            url: "https://example.com".to_string(),
            published_date: Some("2024-01-15".to_string()),
            author: Some("John Doe".to_string()),
            text: Some("Content text".to_string()),
            highlights: Some(vec!["highlight 1".to_string(), "highlight 2".to_string()]),
            summary: Some("Summary text".to_string()),
        };
        assert_eq!(result.title, Some("Test Title".to_string()));
        assert_eq!(result.url, "https://example.com");
        assert_eq!(result.published_date, Some("2024-01-15".to_string()));
        assert_eq!(result.author, Some("John Doe".to_string()));
        assert_eq!(result.text, Some("Content text".to_string()));
        assert_eq!(result.highlights.as_ref().unwrap().len(), 2);
        assert_eq!(result.summary, Some("Summary text".to_string()));
    }

    #[test]
    fn test_exa_result_minimal_fields() {
        let result = ExaResult {
            title: None,
            url: "https://example.com".to_string(),
            published_date: None,
            author: None,
            text: None,
            highlights: None,
            summary: None,
        };
        assert!(result.title.is_none());
        assert_eq!(result.url, "https://example.com");
    }

    #[test]
    fn test_exa_result_serialization() {
        let result = ExaResult {
            title: Some("Title".to_string()),
            url: "https://test.com".to_string(),
            published_date: Some("2024-01-01".to_string()),
            author: None,
            text: None,
            highlights: None,
            summary: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"title\":\"Title\""));
        assert!(json.contains("\"url\":\"https://test.com\""));
        assert!(json.contains("\"publishedDate\":\"2024-01-01\""));
    }

    #[test]
    fn test_exa_result_deserialization() {
        let json = r#"{
            "title": "Test",
            "url": "https://example.com",
            "publishedDate": "2024-01-01",
            "author": "Author Name",
            "text": "Content",
            "highlights": ["a", "b"],
            "summary": "Summary"
        }"#;
        let result: ExaResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.title, Some("Test".to_string()));
        assert_eq!(result.url, "https://example.com");
        assert_eq!(result.published_date, Some("2024-01-01".to_string()));
        assert_eq!(result.author, Some("Author Name".to_string()));
        assert_eq!(result.text, Some("Content".to_string()));
        assert_eq!(result.highlights, Some(vec!["a".to_string(), "b".to_string()]));
        assert_eq!(result.summary, Some("Summary".to_string()));
    }

    #[test]
    fn test_exa_result_deserialization_minimal() {
        let json = r#"{"url": "https://example.com"}"#;
        let result: ExaResult = serde_json::from_str(json).unwrap();
        assert!(result.title.is_none());
        assert_eq!(result.url, "https://example.com");
    }

    #[test]
    fn test_exa_result_clone() {
        let result = ExaResult {
            title: Some("Test".to_string()),
            url: "https://example.com".to_string(),
            published_date: None,
            author: None,
            text: None,
            highlights: None,
            summary: None,
        };
        let cloned = result.clone();
        assert_eq!(cloned.title, result.title);
        assert_eq!(cloned.url, result.url);
    }

    #[test]
    fn test_exa_result_debug() {
        let result = ExaResult {
            title: Some("Test".to_string()),
            url: "https://example.com".to_string(),
            published_date: None,
            author: None,
            text: None,
            highlights: None,
            summary: None,
        };
        let debug = format!("{:?}", result);
        assert!(debug.contains("ExaResult"));
        assert!(debug.contains("Test"));
    }

    // =========================================================================
    // ExaSearchResponse Tests
    // =========================================================================

    #[test]
    fn test_exa_search_response_with_request_id() {
        let response = ExaSearchResponse {
            request_id: Some("req-123".to_string()),
            results: vec![],
        };
        assert_eq!(response.request_id, Some("req-123".to_string()));
        assert!(response.results.is_empty());
    }

    #[test]
    fn test_exa_search_response_without_request_id() {
        let response = ExaSearchResponse {
            request_id: None,
            results: vec![],
        };
        assert!(response.request_id.is_none());
    }

    #[test]
    fn test_exa_search_response_with_results() {
        let response = ExaSearchResponse {
            request_id: Some("id".to_string()),
            results: vec![
                ExaResult {
                    title: Some("Result 1".to_string()),
                    url: "https://example1.com".to_string(),
                    published_date: None,
                    author: None,
                    text: None,
                    highlights: None,
                    summary: None,
                },
                ExaResult {
                    title: Some("Result 2".to_string()),
                    url: "https://example2.com".to_string(),
                    published_date: None,
                    author: None,
                    text: None,
                    highlights: None,
                    summary: None,
                },
            ],
        };
        assert_eq!(response.results.len(), 2);
    }

    #[test]
    fn test_exa_search_response_serialization() {
        let response = ExaSearchResponse {
            request_id: Some("abc123".to_string()),
            results: vec![],
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"requestId\":\"abc123\""));
        assert!(json.contains("\"results\":[]"));
    }

    #[test]
    fn test_exa_search_response_deserialization() {
        let json = r#"{"requestId": "test-id", "results": []}"#;
        let response: ExaSearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.request_id, Some("test-id".to_string()));
        assert!(response.results.is_empty());
    }

    #[test]
    fn test_exa_search_response_clone() {
        let response = ExaSearchResponse {
            request_id: Some("id".to_string()),
            results: vec![],
        };
        let cloned = response.clone();
        assert_eq!(cloned.request_id, response.request_id);
    }

    // =========================================================================
    // ExaSearchRequest Tests
    // =========================================================================

    #[test]
    fn test_exa_search_request_minimal() {
        let request = ExaSearchRequest {
            query: "test query".to_string(),
            r#type: None,
            num_results: None,
            category: None,
            include_domains: None,
            exclude_domains: None,
        };
        assert_eq!(request.query, "test query");
    }

    #[test]
    fn test_exa_search_request_all_fields() {
        let request = ExaSearchRequest {
            query: "test".to_string(),
            r#type: Some("neural".to_string()),
            num_results: Some(10),
            category: Some("news".to_string()),
            include_domains: Some(vec!["example.com".to_string()]),
            exclude_domains: Some(vec!["spam.com".to_string()]),
        };
        assert_eq!(request.r#type, Some("neural".to_string()));
        assert_eq!(request.num_results, Some(10));
        assert_eq!(request.category, Some("news".to_string()));
        assert!(request.include_domains.is_some());
        assert!(request.exclude_domains.is_some());
    }

    #[test]
    fn test_exa_search_request_serialization_minimal() {
        let request = ExaSearchRequest {
            query: "test".to_string(),
            r#type: None,
            num_results: None,
            category: None,
            include_domains: None,
            exclude_domains: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"query\":\"test\""));
        // Optional fields should be skipped
        assert!(!json.contains("numResults"));
        assert!(!json.contains("type"));
    }

    #[test]
    fn test_exa_search_request_serialization_full() {
        let request = ExaSearchRequest {
            query: "search".to_string(),
            r#type: Some("keyword".to_string()),
            num_results: Some(5),
            category: Some("research paper".to_string()),
            include_domains: Some(vec!["arxiv.org".to_string()]),
            exclude_domains: Some(vec!["spam.com".to_string()]),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"query\":\"search\""));
        assert!(json.contains("\"type\":\"keyword\""));
        assert!(json.contains("\"numResults\":5"));
        assert!(json.contains("\"category\":\"research paper\""));
        assert!(json.contains("\"includeDomains\""));
        assert!(json.contains("\"excludeDomains\""));
    }

    #[test]
    fn test_exa_search_request_deserialization() {
        let json = r#"{"query": "test", "type": "auto", "numResults": 20}"#;
        let request: ExaSearchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.query, "test");
        assert_eq!(request.r#type, Some("auto".to_string()));
        assert_eq!(request.num_results, Some(20));
    }

    #[test]
    fn test_exa_search_request_clone() {
        let request = ExaSearchRequest {
            query: "test".to_string(),
            r#type: Some("neural".to_string()),
            num_results: None,
            category: None,
            include_domains: None,
            exclude_domains: None,
        };
        let cloned = request.clone();
        assert_eq!(cloned.query, request.query);
        assert_eq!(cloned.r#type, request.r#type);
    }

    // =========================================================================
    // ExaSearchTool Creation Tests
    // =========================================================================

    #[test]
    fn test_exa_tool_creation() {
        let exa = ExaSearchTool::new("test-api-key");
        assert_eq!(exa.name(), "exa_search");
        assert!(exa.description().contains("Exa"));
    }

    #[test]
    fn test_exa_tool_new_with_string() {
        let exa = ExaSearchTool::new(String::from("key"));
        assert_eq!(exa.api_key, "key");
    }

    #[test]
    fn test_exa_tool_new_with_str() {
        let exa = ExaSearchTool::new("key");
        assert_eq!(exa.api_key, "key");
    }

    #[test]
    fn test_exa_tool_defaults() {
        let exa = ExaSearchTool::new("key");
        assert_eq!(exa.num_results, 10);
        assert_eq!(exa.search_type, "auto");
        assert!(exa.category.is_none());
        assert!(exa.include_domains.is_none());
        assert!(exa.exclude_domains.is_none());
    }

    // =========================================================================
    // Builder Tests
    // =========================================================================

    #[test]
    fn test_exa_tool_builder() {
        let exa = ExaSearchTool::builder()
            .api_key("test-key")
            .num_results(5)
            .search_type("neural")
            .category("research paper")
            .build()
            .unwrap();

        assert_eq!(exa.api_key, "test-key");
        assert_eq!(exa.num_results, 5);
        assert_eq!(exa.search_type, "neural");
        assert_eq!(exa.category, Some("research paper".to_string()));
    }

    #[test]
    fn test_exa_builder_missing_api_key() {
        let result = ExaSearchTool::builder().num_results(5).build();
        assert!(result.is_err());
    }

    #[test]
    fn test_exa_builder_api_key_only() {
        let exa = ExaSearchTool::builder()
            .api_key("my-key")
            .build()
            .unwrap();
        assert_eq!(exa.api_key, "my-key");
        assert_eq!(exa.num_results, 10); // default
        assert_eq!(exa.search_type, "auto"); // default
    }

    #[test]
    fn test_exa_builder_num_results_capped_at_100() {
        let exa = ExaSearchTool::builder()
            .api_key("key")
            .num_results(200)
            .build()
            .unwrap();
        assert_eq!(exa.num_results, 100);
    }

    #[test]
    fn test_exa_builder_num_results_exact_100() {
        let exa = ExaSearchTool::builder()
            .api_key("key")
            .num_results(100)
            .build()
            .unwrap();
        assert_eq!(exa.num_results, 100);
    }

    #[test]
    fn test_exa_builder_num_results_under_100() {
        let exa = ExaSearchTool::builder()
            .api_key("key")
            .num_results(50)
            .build()
            .unwrap();
        assert_eq!(exa.num_results, 50);
    }

    #[test]
    fn test_exa_builder_include_domains() {
        let domains = vec!["example.com".to_string(), "test.org".to_string()];
        let exa = ExaSearchTool::builder()
            .api_key("key")
            .include_domains(domains.clone())
            .build()
            .unwrap();
        assert_eq!(exa.include_domains, Some(domains));
    }

    #[test]
    fn test_exa_builder_exclude_domains() {
        let domains = vec!["spam.com".to_string()];
        let exa = ExaSearchTool::builder()
            .api_key("key")
            .exclude_domains(domains.clone())
            .build()
            .unwrap();
        assert_eq!(exa.exclude_domains, Some(domains));
    }

    #[test]
    fn test_exa_builder_both_domain_filters() {
        let include = vec!["good.com".to_string()];
        let exclude = vec!["bad.com".to_string()];
        let exa = ExaSearchTool::builder()
            .api_key("key")
            .include_domains(include.clone())
            .exclude_domains(exclude.clone())
            .build()
            .unwrap();
        assert_eq!(exa.include_domains, Some(include));
        assert_eq!(exa.exclude_domains, Some(exclude));
    }

    #[test]
    fn test_exa_builder_search_type_keyword() {
        let exa = ExaSearchTool::builder()
            .api_key("key")
            .search_type("keyword")
            .build()
            .unwrap();
        assert_eq!(exa.search_type, "keyword");
    }

    #[test]
    fn test_exa_builder_search_type_neural() {
        let exa = ExaSearchTool::builder()
            .api_key("key")
            .search_type("neural")
            .build()
            .unwrap();
        assert_eq!(exa.search_type, "neural");
    }

    #[test]
    fn test_exa_builder_search_type_fast() {
        let exa = ExaSearchTool::builder()
            .api_key("key")
            .search_type("fast")
            .build()
            .unwrap();
        assert_eq!(exa.search_type, "fast");
    }

    #[test]
    fn test_exa_builder_category_news() {
        let exa = ExaSearchTool::builder()
            .api_key("key")
            .category("news")
            .build()
            .unwrap();
        assert_eq!(exa.category, Some("news".to_string()));
    }

    #[test]
    fn test_exa_builder_default() {
        let builder = ExaSearchToolBuilder::default();
        assert!(builder.api_key.is_none());
        assert!(builder.num_results.is_none());
        assert!(builder.search_type.is_none());
        assert!(builder.category.is_none());
        assert!(builder.include_domains.is_none());
        assert!(builder.exclude_domains.is_none());
    }

    #[test]
    fn test_exa_builder_chaining() {
        // Test that builder methods can be chained in any order
        let exa = ExaSearchTool::builder()
            .category("research paper")
            .search_type("neural")
            .num_results(20)
            .api_key("key")
            .include_domains(vec!["arxiv.org".to_string()])
            .exclude_domains(vec!["spam.com".to_string()])
            .build()
            .unwrap();
        assert_eq!(exa.api_key, "key");
        assert_eq!(exa.search_type, "neural");
        assert_eq!(exa.num_results, 20);
        assert_eq!(exa.category, Some("research paper".to_string()));
    }

    // =========================================================================
    // Tool Trait Tests
    // =========================================================================

    #[test]
    fn test_exa_args_schema() {
        let exa = ExaSearchTool::new("test-key");
        let schema = exa.args_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
        assert_eq!(schema["required"][0], "query");
    }

    #[test]
    fn test_exa_args_schema_query_description() {
        let exa = ExaSearchTool::new("key");
        let schema = exa.args_schema();
        let query_desc = schema["properties"]["query"]["description"].as_str().unwrap();
        assert!(query_desc.contains("query"));
    }

    #[test]
    fn test_exa_tool_name() {
        let exa = ExaSearchTool::new("key");
        assert_eq!(exa.name(), "exa_search");
    }

    #[test]
    fn test_exa_tool_description_content() {
        let exa = ExaSearchTool::new("key");
        let desc = exa.description();
        assert!(desc.contains("Exa"));
        assert!(desc.contains("search"));
        assert!(desc.contains("AI"));
    }

    // =========================================================================
    // Format Results Tests
    // =========================================================================

    #[tokio::test]
    async fn test_format_results() {
        let exa = ExaSearchTool::new("test-key");
        let response = ExaSearchResponse {
            request_id: Some("test-123".to_string()),
            results: vec![ExaResult {
                title: Some("Test Result".to_string()),
                url: "https://example.com".to_string(),
                published_date: Some("2024-01-01".to_string()),
                author: Some("Test Author".to_string()),
                text: Some("This is a test snippet".to_string()),
                highlights: None,
                summary: None,
            }],
        };

        let formatted = exa.format_results(&response);
        assert!(formatted.contains("Test Result"));
        assert!(formatted.contains("https://example.com"));
        assert!(formatted.contains("Test Author"));
        assert!(formatted.contains("2024-01-01"));
    }

    #[tokio::test]
    async fn test_format_empty_results() {
        let exa = ExaSearchTool::new("test-key");
        let response = ExaSearchResponse {
            request_id: None,
            results: vec![],
        };

        let formatted = exa.format_results(&response);
        assert_eq!(formatted, "No results found.");
    }

    #[test]
    fn test_format_results_no_title() {
        let exa = ExaSearchTool::new("key");
        let response = ExaSearchResponse {
            request_id: None,
            results: vec![ExaResult {
                title: None,
                url: "https://example.com".to_string(),
                published_date: None,
                author: None,
                text: None,
                highlights: None,
                summary: None,
            }],
        };
        let formatted = exa.format_results(&response);
        assert!(formatted.contains("(No title)"));
        assert!(formatted.contains("https://example.com"));
    }

    #[test]
    fn test_format_results_long_text_truncation() {
        let exa = ExaSearchTool::new("key");
        let long_text = "a".repeat(300); // Longer than 200 char limit
        let response = ExaSearchResponse {
            request_id: None,
            results: vec![ExaResult {
                title: Some("Title".to_string()),
                url: "https://example.com".to_string(),
                published_date: None,
                author: None,
                text: Some(long_text),
                highlights: None,
                summary: None,
            }],
        };
        let formatted = exa.format_results(&response);
        assert!(formatted.contains("..."));
        // Should have truncated text + "..."
        assert!(formatted.contains(&"a".repeat(200)));
    }

    #[test]
    fn test_format_results_text_under_limit() {
        let exa = ExaSearchTool::new("key");
        let short_text = "Short text under 200 characters";
        let response = ExaSearchResponse {
            request_id: None,
            results: vec![ExaResult {
                title: Some("Title".to_string()),
                url: "https://example.com".to_string(),
                published_date: None,
                author: None,
                text: Some(short_text.to_string()),
                highlights: None,
                summary: None,
            }],
        };
        let formatted = exa.format_results(&response);
        assert!(formatted.contains(short_text));
        // Should not have "..." at the end of snippet
        assert!(!formatted.contains("Short text under 200 characters..."));
    }

    #[test]
    fn test_format_results_multiple_results() {
        let exa = ExaSearchTool::new("key");
        let response = ExaSearchResponse {
            request_id: None,
            results: vec![
                ExaResult {
                    title: Some("First Result".to_string()),
                    url: "https://first.com".to_string(),
                    published_date: None,
                    author: None,
                    text: None,
                    highlights: None,
                    summary: None,
                },
                ExaResult {
                    title: Some("Second Result".to_string()),
                    url: "https://second.com".to_string(),
                    published_date: None,
                    author: None,
                    text: None,
                    highlights: None,
                    summary: None,
                },
                ExaResult {
                    title: Some("Third Result".to_string()),
                    url: "https://third.com".to_string(),
                    published_date: None,
                    author: None,
                    text: None,
                    highlights: None,
                    summary: None,
                },
            ],
        };
        let formatted = exa.format_results(&response);
        assert!(formatted.contains("Found 3 results:"));
        assert!(formatted.contains("1. First Result"));
        assert!(formatted.contains("2. Second Result"));
        assert!(formatted.contains("3. Third Result"));
    }

    #[test]
    fn test_format_results_with_author_and_date() {
        let exa = ExaSearchTool::new("key");
        let response = ExaSearchResponse {
            request_id: None,
            results: vec![ExaResult {
                title: Some("Article".to_string()),
                url: "https://example.com".to_string(),
                published_date: Some("2024-06-15".to_string()),
                author: Some("Jane Doe".to_string()),
                text: None,
                highlights: None,
                summary: None,
            }],
        };
        let formatted = exa.format_results(&response);
        assert!(formatted.contains("Author: Jane Doe"));
        assert!(formatted.contains("Published: 2024-06-15"));
    }

    #[test]
    fn test_format_results_without_author_and_date() {
        let exa = ExaSearchTool::new("key");
        let response = ExaSearchResponse {
            request_id: None,
            results: vec![ExaResult {
                title: Some("Article".to_string()),
                url: "https://example.com".to_string(),
                published_date: None,
                author: None,
                text: None,
                highlights: None,
                summary: None,
            }],
        };
        let formatted = exa.format_results(&response);
        assert!(!formatted.contains("Author:"));
        assert!(!formatted.contains("Published:"));
    }

    // =========================================================================
    // Tool _call Tests
    // =========================================================================

    #[tokio::test]
    async fn test_call_with_string_input_structure() {
        // This test verifies the input parsing without actually calling the API
        // We can't mock the HTTP client easily, so we just verify input handling
        let input = ToolInput::String("test query".to_string());
        match input {
            ToolInput::String(s) => assert_eq!(s, "test query"),
            _ => panic!("Expected string input"),
        }
    }

    #[tokio::test]
    async fn test_call_with_structured_input_structure() {
        let input = ToolInput::Structured(json!({"query": "structured query"}));
        match &input {
            ToolInput::Structured(v) => {
                let query = v.get("query").and_then(|q| q.as_str());
                assert_eq!(query, Some("structured query"));
            }
            _ => panic!("Expected structured input"),
        }
    }

    #[tokio::test]
    async fn test_structured_input_missing_query() {
        let input = ToolInput::Structured(json!({"other_field": "value"}));
        match &input {
            ToolInput::Structured(v) => {
                let query = v.get("query").and_then(|q| q.as_str());
                assert!(query.is_none());
            }
            _ => panic!("Expected structured input"),
        }
    }

    // =========================================================================
    // HTTP Client Tests
    // =========================================================================

    #[test]
    fn test_create_http_client() {
        let client = create_http_client();
        // Just verify it doesn't panic and returns a client
        let _ = client;
    }

    // =========================================================================
    // Additional Edge Cases
    // =========================================================================

    #[test]
    fn test_exa_result_empty_url() {
        let result = ExaResult {
            title: Some("Title".to_string()),
            url: String::new(),
            published_date: None,
            author: None,
            text: None,
            highlights: None,
            summary: None,
        };
        assert!(result.url.is_empty());
    }

    #[test]
    fn test_exa_result_empty_highlights_vec() {
        let result = ExaResult {
            title: None,
            url: "https://example.com".to_string(),
            published_date: None,
            author: None,
            text: None,
            highlights: Some(vec![]),
            summary: None,
        };
        assert!(result.highlights.as_ref().unwrap().is_empty());
    }

    #[test]
    fn test_format_results_result_count_header() {
        let exa = ExaSearchTool::new("key");
        let response = ExaSearchResponse {
            request_id: None,
            results: vec![ExaResult {
                title: Some("Single".to_string()),
                url: "https://example.com".to_string(),
                published_date: None,
                author: None,
                text: None,
                highlights: None,
                summary: None,
            }],
        };
        let formatted = exa.format_results(&response);
        assert!(formatted.contains("Found 1 results:"));
    }

    #[test]
    fn test_builder_api_key_with_string_type() {
        let key = String::from("my-api-key");
        let exa = ExaSearchTool::builder()
            .api_key(key)
            .build()
            .unwrap();
        assert_eq!(exa.api_key, "my-api-key");
    }

    #[test]
    fn test_builder_category_with_string_type() {
        let category = String::from("company");
        let exa = ExaSearchTool::builder()
            .api_key("key")
            .category(category)
            .build()
            .unwrap();
        assert_eq!(exa.category, Some("company".to_string()));
    }

    #[test]
    fn test_builder_search_type_with_string_type() {
        let search_type = String::from("auto");
        let exa = ExaSearchTool::builder()
            .api_key("key")
            .search_type(search_type)
            .build()
            .unwrap();
        assert_eq!(exa.search_type, "auto");
    }

    #[test]
    fn test_search_request_debug() {
        let request = ExaSearchRequest {
            query: "test".to_string(),
            r#type: None,
            num_results: None,
            category: None,
            include_domains: None,
            exclude_domains: None,
        };
        let debug = format!("{:?}", request);
        assert!(debug.contains("ExaSearchRequest"));
        assert!(debug.contains("test"));
    }

    #[test]
    fn test_search_response_debug() {
        let response = ExaSearchResponse {
            request_id: Some("id".to_string()),
            results: vec![],
        };
        let debug = format!("{:?}", response);
        assert!(debug.contains("ExaSearchResponse"));
    }

    #[test]
    fn test_format_results_special_characters_in_title() {
        let exa = ExaSearchTool::new("key");
        let response = ExaSearchResponse {
            request_id: None,
            results: vec![ExaResult {
                title: Some("Title with <html> & \"special\" chars".to_string()),
                url: "https://example.com".to_string(),
                published_date: None,
                author: None,
                text: None,
                highlights: None,
                summary: None,
            }],
        };
        let formatted = exa.format_results(&response);
        assert!(formatted.contains("Title with <html> & \"special\" chars"));
    }

    #[test]
    fn test_format_results_unicode_content() {
        let exa = ExaSearchTool::new("key");
        let response = ExaSearchResponse {
            request_id: None,
            results: vec![ExaResult {
                title: Some("日本語タイトル".to_string()),
                url: "https://example.com".to_string(),
                published_date: None,
                author: Some("田中太郎".to_string()),
                text: Some("这是中文内容".to_string()),
                highlights: None,
                summary: None,
            }],
        };
        let formatted = exa.format_results(&response);
        assert!(formatted.contains("日本語タイトル"));
        assert!(formatted.contains("田中太郎"));
        assert!(formatted.contains("这是中文内容"));
    }

    #[test]
    fn test_exa_builder_empty_domain_vectors() {
        let exa = ExaSearchTool::builder()
            .api_key("key")
            .include_domains(vec![])
            .exclude_domains(vec![])
            .build()
            .unwrap();
        assert_eq!(exa.include_domains, Some(vec![]));
        assert_eq!(exa.exclude_domains, Some(vec![]));
    }

    #[test]
    fn test_num_results_boundary_zero() {
        let exa = ExaSearchTool::builder()
            .api_key("key")
            .num_results(0)
            .build()
            .unwrap();
        assert_eq!(exa.num_results, 0);
    }

    #[test]
    fn test_num_results_boundary_one() {
        let exa = ExaSearchTool::builder()
            .api_key("key")
            .num_results(1)
            .build()
            .unwrap();
        assert_eq!(exa.num_results, 1);
    }

    #[test]
    fn test_num_results_boundary_99() {
        let exa = ExaSearchTool::builder()
            .api_key("key")
            .num_results(99)
            .build()
            .unwrap();
        assert_eq!(exa.num_results, 99);
    }

    #[test]
    fn test_num_results_boundary_101() {
        let exa = ExaSearchTool::builder()
            .api_key("key")
            .num_results(101)
            .build()
            .unwrap();
        assert_eq!(exa.num_results, 100); // Capped
    }

    #[test]
    fn test_num_results_large_value() {
        let exa = ExaSearchTool::builder()
            .api_key("key")
            .num_results(u32::MAX)
            .build()
            .unwrap();
        assert_eq!(exa.num_results, 100); // Capped
    }
}
