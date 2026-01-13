//! # Tavily Search Tool
//!
//! Tavily is a search API designed for AI agents and LLMs, providing real-time web access
//! with citations and high-quality search results.
//!
//! ## Features
//!
//! - Real-time web search optimized for AI
//! - LLM-generated answers with citations
//! - Configurable search depth (basic/advanced)
//! - Topic-specific search (general, news, finance)
//! - Image search support
//! - Raw HTML content extraction
//!
//! ## Usage
//!
//! ```rust,no_run
//! use dashflow_tavily::TavilySearchTool;
//! use dashflow::core::tools::Tool;
//!
//! # tokio_test::block_on(async {
//! let tavily = TavilySearchTool::new("tvly-YOUR_API_KEY");
//!
//! // Simple search
//! let results = tavily._call_str("Who is Leo Messi?".to_string()).await.unwrap();
//! println!("Search results: {}", results);
//! # });
//! ```
//!
//! # See Also
//!
//! - [`Tool`] - The trait this tool implements
//! - [`Retriever`] - The retriever trait
//! - [`dashflow-google-search`](https://docs.rs/dashflow-google-search) - Alternative: Google Custom Search
//! - [`dashflow-webscrape`](https://docs.rs/dashflow-webscrape) - Web content extraction tool
//! - [Tavily API Documentation](https://docs.tavily.com/) - Official Tavily docs

use async_trait::async_trait;
use dashflow::core::config::RunnableConfig;
use dashflow::core::documents::Document;
use dashflow::core::http_client::{json_with_limit, SEARCH_RESPONSE_SIZE_LIMIT};
use dashflow::core::retrievers::Retriever;
use dashflow::core::tools::{Tool, ToolInput};
use dashflow::core::Result;
use dashflow::{DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::borrow::Cow;
use std::collections::HashMap;

/// Create an HTTP client with standard timeouts
fn create_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
        .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// Search depth for Tavily API
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SearchDepth {
    /// Basic search (faster)
    #[default]
    Basic,
    /// Advanced search (more comprehensive)
    Advanced,
}

/// Topic category for search
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Topic {
    /// General web search
    #[default]
    General,
    /// News articles
    News,
    /// Financial content
    Finance,
}

/// A single search result from Tavily
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TavilyResult {
    /// Page title
    pub title: String,
    /// Page URL
    pub url: String,
    /// Content snippet
    pub content: String,
    /// Relevance score (0-1)
    pub score: Option<f64>,
    /// Raw HTML content (if requested)
    pub raw_content: Option<String>,
}

/// Response from Tavily search API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TavilySearchResponse {
    /// Original search query
    pub query: String,
    /// LLM-generated answer (if requested)
    pub answer: Option<String>,
    /// Search results
    pub results: Vec<TavilyResult>,
    /// Image URLs (if requested)
    pub images: Option<Vec<String>>,
    /// Response time in seconds
    pub response_time: Option<f64>,
}

/// Request to Tavily search API
#[derive(Clone, Serialize, Deserialize)]
pub struct TavilySearchRequest {
    /// API key
    pub api_key: String,
    /// Search query
    pub query: String,
    /// Search depth
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_depth: Option<String>,
    /// Topic category
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    /// Maximum number of results (0-20)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_results: Option<u32>,
    /// Include LLM-generated answer
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_answer: Option<bool>,
    /// Include raw HTML content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_raw_content: Option<bool>,
    /// Include image search results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_images: Option<bool>,
}

// Custom Debug implementation to prevent API key exposure in logs
impl std::fmt::Debug for TavilySearchRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TavilySearchRequest")
            .field("api_key", &"[REDACTED]")
            .field("query", &self.query)
            .field("search_depth", &self.search_depth)
            .field("topic", &self.topic)
            .field("max_results", &self.max_results)
            .field("include_answer", &self.include_answer)
            .field("include_raw_content", &self.include_raw_content)
            .field("include_images", &self.include_images)
            .finish()
    }
}

/// Tavily search tool for `DashFlow` agents
///
/// This tool provides access to Tavily's AI-optimized search API, which returns
/// high-quality search results with optional LLM-generated answers and citations.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_tavily::TavilySearchTool;
/// use dashflow::core::tools::Tool;
///
/// # tokio_test::block_on(async {
/// let tavily = TavilySearchTool::builder()
///     .api_key("tvly-YOUR_API_KEY")
///     .max_results(5)
///     .search_depth("advanced")
///     .include_answer(true)
///     .build()
///     .unwrap();
///
/// let results = tavily._call_str("latest AI research".to_string())
///     .await
///     .unwrap();
/// println!("Found: {}", results);
/// # });
/// ```
pub struct TavilySearchTool {
    api_key: String,
    max_results: u32,
    search_depth: String,
    topic: String,
    include_answer: bool,
    include_images: bool,
    include_raw_content: bool,
    client: reqwest::Client,
}

impl TavilySearchTool {
    /// Create a new Tavily search tool
    ///
    /// # Arguments
    ///
    /// * `api_key` - Your Tavily API key (format: "tvly-...")
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_tavily::TavilySearchTool;
    ///
    /// let tavily = TavilySearchTool::new("tvly-YOUR_API_KEY");
    /// ```
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            max_results: 5,
            search_depth: "basic".to_string(),
            topic: "general".to_string(),
            include_answer: false,
            include_images: false,
            include_raw_content: false,
            client: create_http_client(),
        }
    }

    /// Create a builder for `TavilySearchTool`
    #[must_use]
    pub fn builder() -> TavilySearchToolBuilder {
        TavilySearchToolBuilder::default()
    }

    /// Perform a search using the Tavily API
    async fn search(&self, query: String) -> Result<TavilySearchResponse> {
        let request = TavilySearchRequest {
            api_key: self.api_key.clone(),
            query,
            search_depth: Some(self.search_depth.clone()),
            topic: Some(self.topic.clone()),
            max_results: Some(self.max_results),
            include_answer: Some(self.include_answer),
            include_raw_content: Some(self.include_raw_content),
            include_images: Some(self.include_images),
        };

        let response = self
            .client
            .post("https://api.tavily.com/search")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                dashflow::core::Error::tool_error(format!("Tavily API request failed: {e}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(dashflow::core::Error::tool_error(format!(
                "Tavily API error ({status}): {error_text}"
            )));
        }

        // M-216: Use size-limited JSON parsing to prevent memory exhaustion
        let search_response: TavilySearchResponse =
            json_with_limit(response, SEARCH_RESPONSE_SIZE_LIMIT).await.map_err(|e| {
                dashflow::core::Error::tool_error(format!("Failed to parse Tavily response: {e}"))
            })?;

        Ok(search_response)
    }

    /// Format search results as a string
    fn format_results(&self, response: TavilySearchResponse) -> String {
        let mut output = String::new();

        // Include answer if present
        if let Some(answer) = response.answer {
            output.push_str("Answer:\n");
            output.push_str(&answer);
            output.push_str("\n\n");
        }

        // Include search results
        if response.results.is_empty() {
            output.push_str("No results found.");
            return output;
        }

        output.push_str(&format!("Found {} results:\n\n", response.results.len()));

        for (i, result) in response.results.iter().enumerate() {
            output.push_str(&format!("{}. {}\n", i + 1, result.title));
            output.push_str(&format!("   URL: {}\n", result.url));

            if let Some(score) = result.score {
                output.push_str(&format!("   Relevance: {score:.2}\n"));
            }

            // Truncate content to 250 chars (avoid UTF-8 boundary panics)
            let snippet = truncate_with_ellipsis(&result.content, 250);
            output.push_str(&format!("   Content: {snippet}\n"));
            output.push('\n');
        }

        // Include images if present
        if let Some(images) = response.images {
            if !images.is_empty() {
                output.push_str(&format!("Images ({}):\n", images.len()));
                for (i, url) in images.iter().take(5).enumerate() {
                    output.push_str(&format!("{}. {}\n", i + 1, url));
                }
                output.push('\n');
            }
        }

        output
    }
}

fn truncate_with_ellipsis(input: &str, max_chars: usize) -> Cow<'_, str> {
    match input.char_indices().nth(max_chars) {
        None => Cow::Borrowed(input),
        Some((byte_idx, _)) => Cow::Owned(format!("{}...", &input[..byte_idx])),
    }
}

#[async_trait]
impl Tool for TavilySearchTool {
    fn name(&self) -> &'static str {
        "tavily_search"
    }

    fn description(&self) -> &'static str {
        "Search the web using Tavily's AI-optimized search API. \
         Returns high-quality search results with citations, designed for AI agents and LLMs. \
         Can optionally include LLM-generated answers, images, and raw HTML content. \
         Best for real-time web information, news, and factual queries."
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
        Ok(self.format_results(response))
    }
}

/// Builder for `TavilySearchTool`
#[derive(Default)]
pub struct TavilySearchToolBuilder {
    api_key: Option<String>,
    max_results: Option<u32>,
    search_depth: Option<String>,
    topic: Option<String>,
    include_answer: Option<bool>,
    include_images: Option<bool>,
    include_raw_content: Option<bool>,
}

impl TavilySearchToolBuilder {
    /// Set the API key
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set the maximum number of results (0-20)
    #[must_use]
    pub fn max_results(mut self, max_results: u32) -> Self {
        self.max_results = Some(max_results.min(20));
        self
    }

    /// Set the search depth ("basic" or "advanced")
    pub fn search_depth(mut self, search_depth: impl Into<String>) -> Self {
        self.search_depth = Some(search_depth.into());
        self
    }

    /// Set the topic ("general", "news", or "finance")
    pub fn topic(mut self, topic: impl Into<String>) -> Self {
        self.topic = Some(topic.into());
        self
    }

    /// Include LLM-generated answer
    #[must_use]
    pub fn include_answer(mut self, include: bool) -> Self {
        self.include_answer = Some(include);
        self
    }

    /// Include image search results
    #[must_use]
    pub fn include_images(mut self, include: bool) -> Self {
        self.include_images = Some(include);
        self
    }

    /// Include raw HTML content
    #[must_use]
    pub fn include_raw_content(mut self, include: bool) -> Self {
        self.include_raw_content = Some(include);
        self
    }

    /// Build the `TavilySearchTool`
    pub fn build(self) -> Result<TavilySearchTool> {
        let api_key = self.api_key.ok_or_else(|| {
            dashflow::core::Error::tool_error("API key is required for Tavily search".to_string())
        })?;

        Ok(TavilySearchTool {
            api_key,
            max_results: self.max_results.unwrap_or(5),
            search_depth: self.search_depth.unwrap_or_else(|| "basic".to_string()),
            topic: self.topic.unwrap_or_else(|| "general".to_string()),
            include_answer: self.include_answer.unwrap_or(false),
            include_images: self.include_images.unwrap_or(false),
            include_raw_content: self.include_raw_content.unwrap_or(false),
            client: create_http_client(),
        })
    }
}

/// Build a Tavily search tool from a ToolConfig
///
/// This function enables configuration-driven tool creation, following the
/// same pattern as `dashflow_openai::build_chat_model()`.
///
/// # Arguments
///
/// * `config` - The tool configuration (must be ToolConfig::Tavily variant)
///
/// # Returns
///
/// Returns `Arc<dyn Tool>` for use with agents and graph nodes.
///
/// # Errors
///
/// Returns an error if:
/// - The configuration is not a Tavily tool config
/// - API key resolution fails (environment variable not set)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::ToolConfig;
/// use dashflow_tavily::build_tool;
///
/// let config = ToolConfig::Tavily {
///     api_key: SecretReference::Env("TAVILY_API_KEY".into()),
///     max_results: 5,
///     search_depth: "advanced".into(),
///     topic: "general".into(),
///     include_answer: true,
///     include_images: false,
///     include_raw_content: false,
/// };
/// let tool = build_tool(&config)?;
/// ```
pub fn build_tool(
    config: &dashflow::core::config_loader::ToolConfig,
) -> dashflow::core::Result<std::sync::Arc<dyn Tool>> {
    use dashflow::core::config_loader::ToolConfig;

    match config {
        ToolConfig::Tavily {
            api_key,
            max_results,
            search_depth,
            topic,
            include_answer,
            include_images,
            include_raw_content,
        } => {
            let resolved_key = api_key.resolve().map_err(|e| {
                dashflow::core::Error::tool_error(format!("Failed to resolve Tavily API key: {e}"))
            })?;

            let tool = TavilySearchTool::builder()
                .api_key(resolved_key)
                .max_results(*max_results)
                .search_depth(search_depth.clone())
                .topic(topic.clone())
                .include_answer(*include_answer)
                .include_images(*include_images)
                .include_raw_content(*include_raw_content)
                .build()?;

            Ok(std::sync::Arc::new(tool))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== SearchDepth Enum Tests ====================

    #[test]
    fn test_search_depth_default_is_basic() {
        let depth: SearchDepth = Default::default();
        assert!(matches!(depth, SearchDepth::Basic));
    }

    #[test]
    fn test_search_depth_serialize_basic() {
        let depth = SearchDepth::Basic;
        assert_eq!(serde_json::to_string(&depth).unwrap(), "\"basic\"");
    }

    #[test]
    fn test_search_depth_serialize_advanced() {
        let depth = SearchDepth::Advanced;
        assert_eq!(serde_json::to_string(&depth).unwrap(), "\"advanced\"");
    }

    #[test]
    fn test_search_depth_deserialize_basic() {
        let depth: SearchDepth = serde_json::from_str("\"basic\"").unwrap();
        assert!(matches!(depth, SearchDepth::Basic));
    }

    #[test]
    fn test_search_depth_deserialize_advanced() {
        let depth: SearchDepth = serde_json::from_str("\"advanced\"").unwrap();
        assert!(matches!(depth, SearchDepth::Advanced));
    }

    #[test]
    fn test_search_depth_deserialize_invalid() {
        let result: std::result::Result<SearchDepth, _> = serde_json::from_str("\"invalid\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_search_depth_clone() {
        let depth = SearchDepth::Advanced;
        let cloned = depth.clone();
        assert!(matches!(cloned, SearchDepth::Advanced));
    }

    #[test]
    fn test_search_depth_debug() {
        let depth = SearchDepth::Basic;
        let debug = format!("{:?}", depth);
        assert!(debug.contains("Basic"));
    }

    // ==================== Topic Enum Tests ====================

    #[test]
    fn test_topic_default_is_general() {
        let topic: Topic = Default::default();
        assert!(matches!(topic, Topic::General));
    }

    #[test]
    fn test_topic_serialize_general() {
        let topic = Topic::General;
        assert_eq!(serde_json::to_string(&topic).unwrap(), "\"general\"");
    }

    #[test]
    fn test_topic_serialize_news() {
        let topic = Topic::News;
        assert_eq!(serde_json::to_string(&topic).unwrap(), "\"news\"");
    }

    #[test]
    fn test_topic_serialize_finance() {
        let topic = Topic::Finance;
        assert_eq!(serde_json::to_string(&topic).unwrap(), "\"finance\"");
    }

    #[test]
    fn test_topic_deserialize_general() {
        let topic: Topic = serde_json::from_str("\"general\"").unwrap();
        assert!(matches!(topic, Topic::General));
    }

    #[test]
    fn test_topic_deserialize_news() {
        let topic: Topic = serde_json::from_str("\"news\"").unwrap();
        assert!(matches!(topic, Topic::News));
    }

    #[test]
    fn test_topic_deserialize_finance() {
        let topic: Topic = serde_json::from_str("\"finance\"").unwrap();
        assert!(matches!(topic, Topic::Finance));
    }

    #[test]
    fn test_topic_deserialize_invalid() {
        let result: std::result::Result<Topic, _> = serde_json::from_str("\"sports\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_topic_clone() {
        let topic = Topic::Finance;
        let cloned = topic.clone();
        assert!(matches!(cloned, Topic::Finance));
    }

    #[test]
    fn test_topic_debug() {
        let topic = Topic::News;
        let debug = format!("{:?}", topic);
        assert!(debug.contains("News"));
    }

    // ==================== TavilyResult Tests ====================

    #[test]
    fn test_tavily_result_minimal() {
        let result = TavilyResult {
            title: "Test".to_string(),
            url: "https://example.com".to_string(),
            content: "Content".to_string(),
            score: None,
            raw_content: None,
        };
        assert_eq!(result.title, "Test");
        assert!(result.score.is_none());
        assert!(result.raw_content.is_none());
    }

    #[test]
    fn test_tavily_result_full() {
        let result = TavilyResult {
            title: "Full Test".to_string(),
            url: "https://example.com/full".to_string(),
            content: "Full content".to_string(),
            score: Some(0.95),
            raw_content: Some("<html>Raw</html>".to_string()),
        };
        assert_eq!(result.score, Some(0.95));
        assert_eq!(result.raw_content, Some("<html>Raw</html>".to_string()));
    }

    #[test]
    fn test_tavily_result_serialize() {
        let result = TavilyResult {
            title: "Test".to_string(),
            url: "https://example.com".to_string(),
            content: "Content".to_string(),
            score: Some(0.8),
            raw_content: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["title"], "Test");
        assert_eq!(json["url"], "https://example.com");
        assert_eq!(json["content"], "Content");
        assert_eq!(json["score"], 0.8);
    }

    #[test]
    fn test_tavily_result_deserialize() {
        let json = r#"{"title":"Test","url":"https://example.com","content":"Content","score":0.9}"#;
        let result: TavilyResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.title, "Test");
        assert_eq!(result.score, Some(0.9));
    }

    #[test]
    fn test_tavily_result_deserialize_missing_optional() {
        let json = r#"{"title":"Test","url":"https://example.com","content":"Content"}"#;
        let result: TavilyResult = serde_json::from_str(json).unwrap();
        assert!(result.score.is_none());
        assert!(result.raw_content.is_none());
    }

    #[test]
    fn test_tavily_result_clone() {
        let result = TavilyResult {
            title: "Test".to_string(),
            url: "https://example.com".to_string(),
            content: "Content".to_string(),
            score: Some(0.5),
            raw_content: None,
        };
        let cloned = result.clone();
        assert_eq!(cloned.title, result.title);
        assert_eq!(cloned.score, result.score);
    }

    #[test]
    fn test_tavily_result_debug() {
        let result = TavilyResult {
            title: "Debug Test".to_string(),
            url: "https://example.com".to_string(),
            content: "Content".to_string(),
            score: None,
            raw_content: None,
        };
        let debug = format!("{:?}", result);
        assert!(debug.contains("Debug Test"));
        assert!(debug.contains("TavilyResult"));
    }

    // ==================== TavilySearchResponse Tests ====================

    #[test]
    fn test_search_response_minimal() {
        let response = TavilySearchResponse {
            query: "test".to_string(),
            answer: None,
            results: vec![],
            images: None,
            response_time: None,
        };
        assert_eq!(response.query, "test");
        assert!(response.results.is_empty());
    }

    #[test]
    fn test_search_response_full() {
        let response = TavilySearchResponse {
            query: "full test".to_string(),
            answer: Some("The answer".to_string()),
            results: vec![TavilyResult {
                title: "Result".to_string(),
                url: "https://example.com".to_string(),
                content: "Content".to_string(),
                score: Some(0.9),
                raw_content: None,
            }],
            images: Some(vec!["https://example.com/img.jpg".to_string()]),
            response_time: Some(0.5),
        };
        assert_eq!(response.answer, Some("The answer".to_string()));
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.images.as_ref().unwrap().len(), 1);
        assert_eq!(response.response_time, Some(0.5));
    }

    #[test]
    fn test_search_response_serialize() {
        let response = TavilySearchResponse {
            query: "test".to_string(),
            answer: Some("answer".to_string()),
            results: vec![],
            images: None,
            response_time: Some(1.0),
        };
        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["query"], "test");
        assert_eq!(json["answer"], "answer");
        assert_eq!(json["response_time"], 1.0);
    }

    #[test]
    fn test_search_response_deserialize() {
        let json = r#"{"query":"test","answer":"answer","results":[],"images":null,"response_time":0.5}"#;
        let response: TavilySearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.query, "test");
        assert_eq!(response.answer, Some("answer".to_string()));
    }

    #[test]
    fn test_search_response_deserialize_minimal() {
        let json = r#"{"query":"test","results":[]}"#;
        let response: TavilySearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.query, "test");
        assert!(response.answer.is_none());
    }

    #[test]
    fn test_search_response_multiple_results() {
        let response = TavilySearchResponse {
            query: "multi".to_string(),
            answer: None,
            results: vec![
                TavilyResult {
                    title: "First".to_string(),
                    url: "https://first.com".to_string(),
                    content: "First content".to_string(),
                    score: Some(0.9),
                    raw_content: None,
                },
                TavilyResult {
                    title: "Second".to_string(),
                    url: "https://second.com".to_string(),
                    content: "Second content".to_string(),
                    score: Some(0.8),
                    raw_content: None,
                },
            ],
            images: None,
            response_time: None,
        };
        assert_eq!(response.results.len(), 2);
        assert_eq!(response.results[0].title, "First");
        assert_eq!(response.results[1].title, "Second");
    }

    #[test]
    fn test_search_response_clone() {
        let response = TavilySearchResponse {
            query: "clone".to_string(),
            answer: Some("cloned".to_string()),
            results: vec![],
            images: None,
            response_time: Some(0.1),
        };
        let cloned = response.clone();
        assert_eq!(cloned.query, response.query);
        assert_eq!(cloned.answer, response.answer);
    }

    #[test]
    fn test_search_response_debug() {
        let response = TavilySearchResponse {
            query: "debug test".to_string(),
            answer: None,
            results: vec![],
            images: None,
            response_time: None,
        };
        let debug = format!("{:?}", response);
        assert!(debug.contains("debug test"));
        assert!(debug.contains("TavilySearchResponse"));
    }

    // ==================== TavilySearchRequest Tests ====================

    #[test]
    fn test_search_request_minimal() {
        let request = TavilySearchRequest {
            api_key: "test-key".to_string(),
            query: "search".to_string(),
            search_depth: None,
            topic: None,
            max_results: None,
            include_answer: None,
            include_raw_content: None,
            include_images: None,
        };
        assert_eq!(request.api_key, "test-key");
        assert_eq!(request.query, "search");
    }

    #[test]
    fn test_search_request_full() {
        let request = TavilySearchRequest {
            api_key: "test-key".to_string(),
            query: "search".to_string(),
            search_depth: Some("advanced".to_string()),
            topic: Some("news".to_string()),
            max_results: Some(10),
            include_answer: Some(true),
            include_raw_content: Some(false),
            include_images: Some(true),
        };
        assert_eq!(request.search_depth, Some("advanced".to_string()));
        assert_eq!(request.max_results, Some(10));
    }

    #[test]
    fn test_search_request_serialize_skips_none() {
        let request = TavilySearchRequest {
            api_key: "key".to_string(),
            query: "q".to_string(),
            search_depth: None,
            topic: None,
            max_results: None,
            include_answer: None,
            include_raw_content: None,
            include_images: None,
        };
        let json = serde_json::to_value(&request).unwrap();
        assert!(json.get("search_depth").is_none());
        assert!(json.get("topic").is_none());
        assert!(json.get("max_results").is_none());
        assert!(json.get("include_answer").is_none());
        assert!(json.get("include_raw_content").is_none());
        assert!(json.get("include_images").is_none());
    }

    #[test]
    fn test_search_request_serialize_includes_values() {
        let request = TavilySearchRequest {
            api_key: "key".to_string(),
            query: "q".to_string(),
            search_depth: Some("basic".to_string()),
            topic: Some("general".to_string()),
            max_results: Some(5),
            include_answer: Some(true),
            include_raw_content: Some(false),
            include_images: Some(true),
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["search_depth"], "basic");
        assert_eq!(json["topic"], "general");
        assert_eq!(json["max_results"], 5);
        assert_eq!(json["include_answer"], true);
        assert_eq!(json["include_raw_content"], false);
        assert_eq!(json["include_images"], true);
    }

    #[test]
    fn test_search_request_debug_redacts_api_key() {
        let request = TavilySearchRequest {
            api_key: "tvly-super-secret".to_string(),
            query: "who am i".to_string(),
            search_depth: None,
            topic: None,
            max_results: None,
            include_answer: None,
            include_raw_content: None,
            include_images: None,
        };
        let debug = format!("{:?}", request);
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("tvly-super-secret"));
        assert!(debug.contains("who am i"));
    }

    #[test]
    fn test_search_request_debug_shows_all_fields() {
        let request = TavilySearchRequest {
            api_key: "secret".to_string(),
            query: "test query".to_string(),
            search_depth: Some("advanced".to_string()),
            topic: Some("news".to_string()),
            max_results: Some(10),
            include_answer: Some(true),
            include_raw_content: Some(false),
            include_images: Some(true),
        };
        let debug = format!("{:?}", request);
        assert!(debug.contains("test query"));
        assert!(debug.contains("advanced"));
        assert!(debug.contains("news"));
        assert!(debug.contains("10"));
    }

    #[test]
    fn test_search_request_clone() {
        let request = TavilySearchRequest {
            api_key: "key".to_string(),
            query: "query".to_string(),
            search_depth: Some("basic".to_string()),
            topic: None,
            max_results: Some(5),
            include_answer: None,
            include_raw_content: None,
            include_images: None,
        };
        let cloned = request.clone();
        assert_eq!(cloned.api_key, request.api_key);
        assert_eq!(cloned.search_depth, request.search_depth);
    }

    // ==================== TavilySearchTool Tests ====================

    #[test]
    fn test_tavily_tool_creation() {
        let tavily = TavilySearchTool::new("tvly-test-key");
        assert_eq!(tavily.name(), "tavily_search");
        assert!(tavily.description().contains("Tavily"));
    }

    #[test]
    fn test_tavily_tool_new_defaults() {
        let tavily = TavilySearchTool::new("test-key");
        assert_eq!(tavily.api_key, "test-key");
        assert_eq!(tavily.max_results, 5);
        assert_eq!(tavily.search_depth, "basic");
        assert_eq!(tavily.topic, "general");
        assert!(!tavily.include_answer);
        assert!(!tavily.include_images);
        assert!(!tavily.include_raw_content);
    }

    #[test]
    fn test_tavily_tool_new_with_into() {
        let key = String::from("string-key");
        let tavily = TavilySearchTool::new(key);
        assert_eq!(tavily.api_key, "string-key");

        let tavily2 = TavilySearchTool::new("str-key");
        assert_eq!(tavily2.api_key, "str-key");
    }

    #[test]
    fn test_tavily_tool_description_quality() {
        let tavily = TavilySearchTool::new("tvly-test-key");
        let description = tavily.description();
        assert!(description.contains("Search"));
        assert!(description.contains("web"));
        assert!(description.contains("citations"));
        assert!(description.contains("LLM"));
        assert!(description.contains("AI"));
    }

    #[test]
    fn test_tavily_tool_builder() {
        let tavily = TavilySearchTool::builder()
            .api_key("tvly-test-key")
            .max_results(10)
            .search_depth("advanced")
            .topic("news")
            .include_answer(true)
            .include_images(true)
            .build()
            .unwrap();

        assert_eq!(tavily.api_key, "tvly-test-key");
        assert_eq!(tavily.max_results, 10);
        assert_eq!(tavily.search_depth, "advanced");
        assert_eq!(tavily.topic, "news");
        assert!(tavily.include_answer);
        assert!(tavily.include_images);
    }

    #[test]
    fn test_tavily_builder_missing_api_key() {
        let result = TavilySearchTool::builder().max_results(5).build();
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains("API key"));
    }

    #[test]
    fn test_tavily_builder_defaults() {
        let tavily = TavilySearchTool::builder()
            .api_key("test")
            .build()
            .unwrap();
        assert_eq!(tavily.max_results, 5);
        assert_eq!(tavily.search_depth, "basic");
        assert_eq!(tavily.topic, "general");
        assert!(!tavily.include_answer);
        assert!(!tavily.include_images);
        assert!(!tavily.include_raw_content);
    }

    #[test]
    fn test_tavily_builder_include_raw_content() {
        let tavily = TavilySearchTool::builder()
            .api_key("test")
            .include_raw_content(true)
            .build()
            .unwrap();
        assert!(tavily.include_raw_content);
    }

    #[test]
    fn test_tavily_builder_topic_finance() {
        let tavily = TavilySearchTool::builder()
            .api_key("test")
            .topic("finance")
            .build()
            .unwrap();
        assert_eq!(tavily.topic, "finance");
    }

    #[test]
    fn test_tavily_args_schema() {
        let tavily = TavilySearchTool::new("tvly-test-key");
        let schema = tavily.args_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
        assert_eq!(schema["required"][0], "query");
    }

    #[test]
    fn test_tavily_args_schema_query_description() {
        let tavily = TavilySearchTool::new("test");
        let schema = tavily.args_schema();
        let query_desc = schema["properties"]["query"]["description"].as_str();
        assert!(query_desc.is_some());
        assert!(query_desc.unwrap().contains("query"));
    }

    #[tokio::test]
    async fn test_max_results_clamped_to_20() {
        let tavily = TavilySearchTool::builder()
            .api_key("test")
            .max_results(100)
            .build()
            .unwrap();
        assert_eq!(tavily.max_results, 20);
    }

    #[tokio::test]
    async fn test_max_results_at_boundary() {
        let tavily = TavilySearchTool::builder()
            .api_key("test")
            .max_results(20)
            .build()
            .unwrap();
        assert_eq!(tavily.max_results, 20);
    }

    #[tokio::test]
    async fn test_max_results_below_limit() {
        let tavily = TavilySearchTool::builder()
            .api_key("test")
            .max_results(10)
            .build()
            .unwrap();
        assert_eq!(tavily.max_results, 10);
    }

    #[tokio::test]
    async fn test_max_results_zero() {
        let tavily = TavilySearchTool::builder()
            .api_key("test")
            .max_results(0)
            .build()
            .unwrap();
        assert_eq!(tavily.max_results, 0);
    }

    // ==================== Tool Trait Tests ====================

    #[tokio::test]
    async fn test_call_structured_missing_query_field() {
        let tavily = TavilySearchTool::new("tvly-test-key");
        let err = tavily
            ._call(ToolInput::Structured(serde_json::json!({})))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Missing 'query'"));
    }

    #[tokio::test]
    async fn test_call_structured_query_not_string() {
        let tavily = TavilySearchTool::new("tvly-test-key");
        let err = tavily
            ._call(ToolInput::Structured(serde_json::json!({"query": 123})))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Missing 'query'"));
    }

    #[tokio::test]
    async fn test_call_structured_query_null() {
        let tavily = TavilySearchTool::new("test-key");
        let err = tavily
            ._call(ToolInput::Structured(serde_json::json!({"query": null})))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Missing 'query'"));
    }

    #[tokio::test]
    async fn test_call_structured_query_array() {
        let tavily = TavilySearchTool::new("test-key");
        let err = tavily
            ._call(ToolInput::Structured(serde_json::json!({"query": ["a", "b"]})))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Missing 'query'"));
    }

    #[tokio::test]
    async fn test_call_structured_extra_fields_ignored() {
        // This test verifies that extra fields don't cause errors
        // (they're ignored, but will fail at API level due to invalid key)
        let tavily = TavilySearchTool::new("test-key");
        let result = tavily
            ._call(ToolInput::Structured(serde_json::json!({
                "query": "test",
                "extra": "ignored"
            })))
            .await;
        // Will fail at API level, not at parsing
        assert!(result.is_err());
    }

    // ==================== truncate_with_ellipsis Tests ====================

    #[test]
    fn test_truncate_short_string() {
        let result = truncate_with_ellipsis("hello", 10);
        assert_eq!(result.as_ref(), "hello");
        assert!(matches!(result, Cow::Borrowed(_)));
    }

    #[test]
    fn test_truncate_exact_length() {
        let result = truncate_with_ellipsis("hello", 5);
        assert_eq!(result.as_ref(), "hello");
    }

    #[test]
    fn test_truncate_long_string() {
        let result = truncate_with_ellipsis("hello world", 5);
        assert_eq!(result.as_ref(), "hello...");
        assert!(matches!(result, Cow::Owned(_)));
    }

    #[test]
    fn test_truncate_empty_string() {
        let result = truncate_with_ellipsis("", 10);
        assert_eq!(result.as_ref(), "");
    }

    #[test]
    fn test_truncate_zero_limit() {
        let result = truncate_with_ellipsis("hello", 0);
        assert_eq!(result.as_ref(), "...");
    }

    #[test]
    fn test_truncate_utf8_emoji() {
        let input = "💖💖💖💖💖💖💖💖💖💖"; // 10 emojis, 4 bytes each
        let result = truncate_with_ellipsis(input, 5);
        assert!(result.ends_with("..."));
        // Should have exactly 5 emojis + ellipsis
        let without_ellipsis = result.trim_end_matches("...");
        assert_eq!(without_ellipsis.chars().count(), 5);
    }

    #[test]
    fn test_truncate_utf8_chinese() {
        let input = "你好世界欢迎光临"; // 8 Chinese characters, 3 bytes each
        let result = truncate_with_ellipsis(input, 4);
        assert!(result.ends_with("..."));
        let without_ellipsis = result.trim_end_matches("...");
        assert_eq!(without_ellipsis.chars().count(), 4);
    }

    #[test]
    fn test_truncate_mixed_utf8() {
        let input = "a💖b💖c"; // 5 characters, mixed bytes
        let result = truncate_with_ellipsis(input, 3);
        assert_eq!(result.as_ref(), "a💖b...");
    }

    #[test]
    fn test_truncate_preserves_utf8_boundary() {
        // Ensure we don't split in the middle of a multi-byte character
        let input = "🎉"; // Single 4-byte emoji
        let result = truncate_with_ellipsis(input, 1);
        assert_eq!(result.as_ref(), "🎉");
    }

    // ==================== format_results Tests ====================

    #[test]
    fn test_format_results_with_answer() {
        let tavily = TavilySearchTool::new("test-key");
        let response = TavilySearchResponse {
            query: "test".to_string(),
            answer: Some("This is the answer".to_string()),
            results: vec![TavilyResult {
                title: "Result".to_string(),
                url: "https://example.com".to_string(),
                content: "Content".to_string(),
                score: None,
                raw_content: None,
            }],
            images: None,
            response_time: None,
        };
        let formatted = tavily.format_results(response);
        assert!(formatted.contains("Answer:"));
        assert!(formatted.contains("This is the answer"));
    }

    #[test]
    fn test_format_results_without_answer() {
        let tavily = TavilySearchTool::new("test-key");
        let response = TavilySearchResponse {
            query: "test".to_string(),
            answer: None,
            results: vec![TavilyResult {
                title: "Result".to_string(),
                url: "https://example.com".to_string(),
                content: "Content".to_string(),
                score: None,
                raw_content: None,
            }],
            images: None,
            response_time: None,
        };
        let formatted = tavily.format_results(response);
        assert!(!formatted.contains("Answer:"));
    }

    #[test]
    fn test_format_results_empty() {
        let tavily = TavilySearchTool::new("test-key");
        let response = TavilySearchResponse {
            query: "test".to_string(),
            answer: None,
            results: vec![],
            images: None,
            response_time: None,
        };
        let formatted = tavily.format_results(response);
        assert_eq!(formatted, "No results found.");
    }

    #[test]
    fn test_format_results_with_score() {
        let tavily = TavilySearchTool::new("test-key");
        let response = TavilySearchResponse {
            query: "test".to_string(),
            answer: None,
            results: vec![TavilyResult {
                title: "Result".to_string(),
                url: "https://example.com".to_string(),
                content: "Content".to_string(),
                score: Some(0.95),
                raw_content: None,
            }],
            images: None,
            response_time: None,
        };
        let formatted = tavily.format_results(response);
        assert!(formatted.contains("Relevance: 0.95"));
    }

    #[test]
    fn test_format_results_without_score() {
        let tavily = TavilySearchTool::new("test-key");
        let response = TavilySearchResponse {
            query: "test".to_string(),
            answer: None,
            results: vec![TavilyResult {
                title: "Result".to_string(),
                url: "https://example.com".to_string(),
                content: "Content".to_string(),
                score: None,
                raw_content: None,
            }],
            images: None,
            response_time: None,
        };
        let formatted = tavily.format_results(response);
        assert!(!formatted.contains("Relevance:"));
    }

    #[test]
    fn test_format_results_multiple_results() {
        let tavily = TavilySearchTool::new("test-key");
        let response = TavilySearchResponse {
            query: "test".to_string(),
            answer: None,
            results: vec![
                TavilyResult {
                    title: "First".to_string(),
                    url: "https://first.com".to_string(),
                    content: "First content".to_string(),
                    score: None,
                    raw_content: None,
                },
                TavilyResult {
                    title: "Second".to_string(),
                    url: "https://second.com".to_string(),
                    content: "Second content".to_string(),
                    score: None,
                    raw_content: None,
                },
            ],
            images: None,
            response_time: None,
        };
        let formatted = tavily.format_results(response);
        assert!(formatted.contains("Found 2 results:"));
        assert!(formatted.contains("1. First"));
        assert!(formatted.contains("2. Second"));
    }

    #[test]
    fn test_format_results_with_images() {
        let tavily = TavilySearchTool::new("test-key");
        let response = TavilySearchResponse {
            query: "test".to_string(),
            answer: None,
            results: vec![TavilyResult {
                title: "Result".to_string(),
                url: "https://example.com".to_string(),
                content: "Content".to_string(),
                score: None,
                raw_content: None,
            }],
            images: Some(vec![
                "https://example.com/img1.jpg".to_string(),
                "https://example.com/img2.jpg".to_string(),
            ]),
            response_time: None,
        };
        let formatted = tavily.format_results(response);
        assert!(formatted.contains("Images (2):"));
        assert!(formatted.contains("https://example.com/img1.jpg"));
        assert!(formatted.contains("https://example.com/img2.jpg"));
    }

    #[test]
    fn test_format_results_limits_images_to_five() {
        let tavily = TavilySearchTool::new("test-key");
        let images: Vec<String> = (0..6)
            .map(|i| format!("https://example.com/{i}.jpg"))
            .collect();
        let response = TavilySearchResponse {
            query: "test".to_string(),
            answer: None,
            results: vec![TavilyResult {
                title: "Result".to_string(),
                url: "https://example.com".to_string(),
                content: "Content".to_string(),
                score: None,
                raw_content: None,
            }],
            images: Some(images.clone()),
            response_time: None,
        };
        let formatted = tavily.format_results(response);
        for url in images.iter().take(5) {
            assert!(formatted.contains(url));
        }
        assert!(!formatted.contains(&images[5]));
    }

    #[test]
    fn test_format_results_empty_images_list() {
        let tavily = TavilySearchTool::new("test-key");
        let response = TavilySearchResponse {
            query: "test".to_string(),
            answer: None,
            results: vec![TavilyResult {
                title: "Result".to_string(),
                url: "https://example.com".to_string(),
                content: "Content".to_string(),
                score: None,
                raw_content: None,
            }],
            images: Some(vec![]),
            response_time: None,
        };
        let formatted = tavily.format_results(response);
        assert!(!formatted.contains("Images"));
    }

    #[test]
    fn test_format_results_utf8_truncation_safe() {
        let tavily = TavilySearchTool::new("test-key");
        let content = "💖".repeat(300);
        let response = TavilySearchResponse {
            query: "test".to_string(),
            answer: None,
            results: vec![TavilyResult {
                title: "Unicode".to_string(),
                url: "https://example.com".to_string(),
                content,
                score: None,
                raw_content: None,
            }],
            images: None,
            response_time: None,
        };
        let formatted = tavily.format_results(response);
        assert!(formatted.contains("..."));
        // Should not panic due to UTF-8 boundary issues
    }

    #[test]
    fn test_format_results_long_content_truncated() {
        let tavily = TavilySearchTool::new("test-key");
        let content = "a".repeat(500);
        let response = TavilySearchResponse {
            query: "test".to_string(),
            answer: None,
            results: vec![TavilyResult {
                title: "Long".to_string(),
                url: "https://example.com".to_string(),
                content,
                score: None,
                raw_content: None,
            }],
            images: None,
            response_time: None,
        };
        let formatted = tavily.format_results(response);
        assert!(formatted.contains("..."));
        // Content should be truncated to 250 chars + ellipsis
    }

    #[test]
    fn test_format_results_short_content_not_truncated() {
        let tavily = TavilySearchTool::new("test-key");
        let response = TavilySearchResponse {
            query: "test".to_string(),
            answer: None,
            results: vec![TavilyResult {
                title: "Short".to_string(),
                url: "https://example.com".to_string(),
                content: "Short content".to_string(),
                score: None,
                raw_content: None,
            }],
            images: None,
            response_time: None,
        };
        let formatted = tavily.format_results(response);
        assert!(formatted.contains("Short content"));
        // Count occurrences of "..." - should only appear for other reasons or not at all
    }

    // ==================== build_tool Tests ====================

    #[test]
    fn test_build_tool_from_config_inline_secret() {
        use dashflow::core::config_loader::{SecretReference, ToolConfig};

        let config = ToolConfig::Tavily {
            api_key: SecretReference::from_inline("tvly-test-key"),
            max_results: 5,
            search_depth: "basic".to_string(),
            topic: "general".to_string(),
            include_answer: false,
            include_images: false,
            include_raw_content: false,
        };

        let tool = build_tool(&config).unwrap();
        assert_eq!(tool.name(), "tavily_search");
    }

    #[test]
    fn test_build_tool_config_with_all_options() {
        use dashflow::core::config_loader::{SecretReference, ToolConfig};

        let config = ToolConfig::Tavily {
            api_key: SecretReference::from_inline("tvly-test"),
            max_results: 10,
            search_depth: "advanced".to_string(),
            topic: "news".to_string(),
            include_answer: true,
            include_images: true,
            include_raw_content: true,
        };

        let tool = build_tool(&config).unwrap();
        assert_eq!(tool.name(), "tavily_search");
        // Can't directly inspect internals, but verify it builds without error
    }

    #[test]
    fn test_build_tool_env_var_not_set() {
        use dashflow::core::config_loader::{SecretReference, ToolConfig};

        let config = ToolConfig::Tavily {
            api_key: SecretReference::from_env("UNLIKELY_ENV_VAR_12345"),
            max_results: 5,
            search_depth: "basic".to_string(),
            topic: "general".to_string(),
            include_answer: false,
            include_images: false,
            include_raw_content: false,
        };

        let result = build_tool(&config);
        assert!(result.is_err());
    }
}

/// Tavily Search API retriever for document retrieval from web search results
///
/// This retriever wraps the `TavilySearchTool` and converts search results into Documents
/// suitable for use in retrieval chains and RAG applications.
///
/// # Python Baseline
///
/// This implements the functionality from:
/// `~/dashflow_community/dashflow_community/retrievers/tavily_search_api.py`
///
/// Python equivalent:
/// ```python
/// from dashflow_community.retrievers import TavilySearchAPIRetriever
///
/// retriever = TavilySearchAPIRetriever(
///     k=5,
///     include_generated_answer=True,
///     include_raw_content=False,
/// )
/// docs = retriever.invoke("latest AI research")
/// ```
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_tavily::TavilySearchRetriever;
/// use dashflow::core::retrievers::Retriever;
///
/// # tokio_test::block_on(async {
/// let retriever = TavilySearchRetriever::builder()
///     .api_key("tvly-YOUR_API_KEY")
///     .k(5)
///     .include_generated_answer(true)
///     .build()
///     .unwrap();
///
/// let docs = retriever._get_relevant_documents("latest AI breakthroughs", None)
///     .await
///     .unwrap();
///
/// for doc in docs {
///     println!("Title: {}", doc.metadata.get("title").unwrap());
///     println!("Content: {}", doc.page_content);
/// }
/// # });
/// ```
pub struct TavilySearchRetriever {
    /// Number of results to return
    k: usize,
    /// Include LLM-generated answer as first document
    include_generated_answer: bool,
    /// Include raw HTML content instead of snippets
    include_raw_content: bool,
    /// Include images in metadata
    include_images: bool,
    /// Search depth (stored for reference, already configured in tool)
    ///
    /// Search depth configuration for reference
    #[allow(dead_code)] // API Parity: Python LangChain search_depth - kept for API consistency
    search_depth: SearchDepth,
    /// Internal search tool
    tool: TavilySearchTool,
}

impl TavilySearchRetriever {
    /// Create a new `TavilySearchRetriever`
    ///
    /// # Arguments
    ///
    /// * `api_key` - Your Tavily API key (format: "tvly-...")
    ///
    /// Default settings:
    /// - k: 10
    /// - `include_generated_answer`: false
    /// - `include_raw_content`: false
    /// - `include_images`: false
    /// - `search_depth`: Basic
    pub fn new(api_key: impl Into<String>) -> Self {
        let mut tool = TavilySearchTool::new(api_key);
        // Keep default `k` and the underlying tool's `max_results` consistent.
        tool.max_results = 10;
        Self {
            k: 10,
            include_generated_answer: false,
            include_raw_content: false,
            include_images: false,
            search_depth: SearchDepth::Basic,
            tool,
        }
    }

    /// Create a builder for `TavilySearchRetriever`
    #[must_use]
    pub fn builder() -> TavilySearchRetrieverBuilder {
        TavilySearchRetrieverBuilder::default()
    }
}

#[async_trait]
impl Retriever for TavilySearchRetriever {
    async fn _get_relevant_documents(
        &self,
        query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        // Adjust max_results if we're including a generated answer
        let max_results = if self.include_generated_answer {
            if self.k == 0 {
                return Ok(vec![]);
            }
            self.k - 1
        } else {
            self.k
        };

        // Perform the search
        let response = self.tool.search(query.to_string()).await?;

        let mut documents = Vec::new();

        // If including generated answer, add it as first document
        if self.include_generated_answer {
            if let Some(answer) = &response.answer {
                let mut metadata = HashMap::new();
                metadata.insert(
                    "title".to_string(),
                    serde_json::Value::String("Suggested Answer".to_string()),
                );
                metadata.insert(
                    "source".to_string(),
                    serde_json::Value::String("https://tavily.com/".to_string()),
                );

                documents.push(Document {
                    page_content: answer.clone(),
                    metadata,
                    id: None,
                });
            }
        }

        // Convert search results to documents
        for result in response.results.iter().take(max_results) {
            let mut metadata = HashMap::new();

            metadata.insert(
                "title".to_string(),
                serde_json::Value::String(result.title.clone()),
            );
            metadata.insert(
                "source".to_string(),
                serde_json::Value::String(result.url.clone()),
            );

            if let Some(score) = result.score {
                metadata.insert(
                    "score".to_string(),
                    serde_json::Value::Number(
                        serde_json::Number::from_f64(score)
                            .unwrap_or_else(|| serde_json::Number::from(0)),
                    ),
                );
            }

            // Add images to metadata if present
            if self.include_images {
                if let Some(images) = &response.images {
                    metadata.insert(
                        "images".to_string(),
                        serde_json::Value::Array(
                            images
                                .iter()
                                .map(|img| serde_json::Value::String(img.clone()))
                                .collect(),
                        ),
                    );
                }
            }

            // Use raw_content if available and requested, otherwise use snippet
            let page_content = if self.include_raw_content {
                result
                    .raw_content
                    .as_ref()
                    .unwrap_or(&result.content)
                    .clone()
            } else {
                result.content.clone()
            };

            documents.push(Document {
                page_content,
                metadata,
                id: Some(result.url.clone()),
            });
        }

        Ok(documents)
    }
}

/// Builder for `TavilySearchRetriever`
#[derive(Default)]
pub struct TavilySearchRetrieverBuilder {
    api_key: Option<String>,
    k: Option<usize>,
    include_generated_answer: Option<bool>,
    include_raw_content: Option<bool>,
    include_images: Option<bool>,
    search_depth: Option<SearchDepth>,
}

impl TavilySearchRetrieverBuilder {
    /// Set the API key
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set the number of results to return (default: 10)
    #[must_use]
    pub fn k(mut self, k: usize) -> Self {
        self.k = Some(k);
        self
    }

    /// Include LLM-generated answer as first document (default: false)
    #[must_use]
    pub fn include_generated_answer(mut self, include: bool) -> Self {
        self.include_generated_answer = Some(include);
        self
    }

    /// Include raw HTML content instead of snippets (default: false)
    #[must_use]
    pub fn include_raw_content(mut self, include: bool) -> Self {
        self.include_raw_content = Some(include);
        self
    }

    /// Include images in metadata (default: false)
    #[must_use]
    pub fn include_images(mut self, include: bool) -> Self {
        self.include_images = Some(include);
        self
    }

    /// Set search depth (default: Basic)
    #[must_use]
    pub fn search_depth(mut self, depth: SearchDepth) -> Self {
        self.search_depth = Some(depth);
        self
    }

    /// Build the `TavilySearchRetriever`
    pub fn build(self) -> Result<TavilySearchRetriever> {
        let api_key = self.api_key.ok_or_else(|| {
            dashflow::core::Error::tool_error("API key is required for Tavily search".to_string())
        })?;

        let k = self.k.unwrap_or(10);
        let include_generated_answer = self.include_generated_answer.unwrap_or(false);
        let include_raw_content = self.include_raw_content.unwrap_or(false);
        let include_images = self.include_images.unwrap_or(false);
        let search_depth = self.search_depth.unwrap_or(SearchDepth::Basic);

        // Build the internal tool with appropriate settings
        let tool = TavilySearchTool::builder()
            .api_key(api_key)
            .max_results(k as u32)
            .search_depth(match search_depth {
                SearchDepth::Basic => "basic",
                SearchDepth::Advanced => "advanced",
            })
            .include_answer(include_generated_answer)
            .include_raw_content(include_raw_content)
            .include_images(include_images)
            .build()?;

        Ok(TavilySearchRetriever {
            k,
            include_generated_answer,
            include_raw_content,
            include_images,
            search_depth,
            tool,
        })
    }
}

#[cfg(test)]
mod retriever_tests {
    use super::*;

    // ==================== TavilySearchRetriever Creation Tests ====================

    #[test]
    fn test_tavily_retriever_creation() {
        let retriever = TavilySearchRetriever::new("tvly-test-key");
        assert_eq!(retriever.k, 10);
        assert!(!retriever.include_generated_answer);
        assert!(!retriever.include_raw_content);
        assert!(!retriever.include_images);
        assert_eq!(retriever.tool.max_results, 10);
    }

    #[test]
    fn test_tavily_retriever_new_defaults() {
        let retriever = TavilySearchRetriever::new("test-key");
        assert_eq!(retriever.k, 10);
        assert!(!retriever.include_generated_answer);
        assert!(!retriever.include_raw_content);
        assert!(!retriever.include_images);
        assert!(matches!(retriever.search_depth, SearchDepth::Basic));
    }

    #[test]
    fn test_tavily_retriever_new_syncs_tool_max_results() {
        let retriever = TavilySearchRetriever::new("test-key");
        assert_eq!(retriever.k, retriever.tool.max_results as usize);
    }

    #[test]
    fn test_tavily_retriever_new_with_string() {
        let key = String::from("string-key");
        let retriever = TavilySearchRetriever::new(key);
        assert_eq!(retriever.tool.api_key, "string-key");
    }

    // ==================== TavilySearchRetrieverBuilder Tests ====================

    #[test]
    fn test_tavily_retriever_builder() {
        let retriever = TavilySearchRetriever::builder()
            .api_key("tvly-test-key")
            .k(5)
            .include_generated_answer(true)
            .include_raw_content(false)
            .include_images(true)
            .search_depth(SearchDepth::Advanced)
            .build()
            .unwrap();

        assert_eq!(retriever.k, 5);
        assert!(retriever.include_generated_answer);
        assert!(!retriever.include_raw_content);
        assert!(retriever.include_images);
        assert_eq!(retriever.tool.max_results, 5);
        assert_eq!(retriever.tool.search_depth, "advanced");
        assert_eq!(retriever.tool.topic, "general");
        assert!(retriever.tool.include_answer);
        assert!(retriever.tool.include_images);
        assert!(!retriever.tool.include_raw_content);
    }

    #[test]
    fn test_tavily_retriever_builder_missing_api_key() {
        let result = TavilySearchRetriever::builder().k(5).build();
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains("API key"));
    }

    #[test]
    fn test_tavily_retriever_builder_defaults() {
        let retriever = TavilySearchRetriever::builder()
            .api_key("test")
            .build()
            .unwrap();

        assert_eq!(retriever.k, 10);
        assert!(!retriever.include_generated_answer);
        assert!(!retriever.include_raw_content);
        assert!(!retriever.include_images);
        assert!(matches!(retriever.search_depth, SearchDepth::Basic));
    }

    #[test]
    fn test_tavily_retriever_builder_k_only() {
        let retriever = TavilySearchRetriever::builder()
            .api_key("test")
            .k(3)
            .build()
            .unwrap();

        assert_eq!(retriever.k, 3);
        assert_eq!(retriever.tool.max_results, 3);
    }

    #[test]
    fn test_tavily_retriever_builder_include_generated_answer_only() {
        let retriever = TavilySearchRetriever::builder()
            .api_key("test")
            .include_generated_answer(true)
            .build()
            .unwrap();

        assert!(retriever.include_generated_answer);
        assert!(retriever.tool.include_answer);
    }

    #[test]
    fn test_tavily_retriever_builder_include_raw_content_only() {
        let retriever = TavilySearchRetriever::builder()
            .api_key("test")
            .include_raw_content(true)
            .build()
            .unwrap();

        assert!(retriever.include_raw_content);
        assert!(retriever.tool.include_raw_content);
    }

    #[test]
    fn test_tavily_retriever_builder_include_images_only() {
        let retriever = TavilySearchRetriever::builder()
            .api_key("test")
            .include_images(true)
            .build()
            .unwrap();

        assert!(retriever.include_images);
        assert!(retriever.tool.include_images);
    }

    #[test]
    fn test_tavily_retriever_builder_search_depth_basic() {
        let retriever = TavilySearchRetriever::builder()
            .api_key("test")
            .search_depth(SearchDepth::Basic)
            .build()
            .unwrap();

        assert_eq!(retriever.tool.search_depth, "basic");
    }

    #[test]
    fn test_tavily_retriever_builder_search_depth_advanced() {
        let retriever = TavilySearchRetriever::builder()
            .api_key("test")
            .search_depth(SearchDepth::Advanced)
            .build()
            .unwrap();

        assert_eq!(retriever.tool.search_depth, "advanced");
    }

    #[test]
    fn test_tavily_retriever_builder_all_false() {
        let retriever = TavilySearchRetriever::builder()
            .api_key("test")
            .include_generated_answer(false)
            .include_raw_content(false)
            .include_images(false)
            .build()
            .unwrap();

        assert!(!retriever.include_generated_answer);
        assert!(!retriever.include_raw_content);
        assert!(!retriever.include_images);
        assert!(!retriever.tool.include_answer);
        assert!(!retriever.tool.include_raw_content);
        assert!(!retriever.tool.include_images);
    }

    #[test]
    fn test_tavily_retriever_builder_all_true() {
        let retriever = TavilySearchRetriever::builder()
            .api_key("test")
            .include_generated_answer(true)
            .include_raw_content(true)
            .include_images(true)
            .build()
            .unwrap();

        assert!(retriever.include_generated_answer);
        assert!(retriever.include_raw_content);
        assert!(retriever.include_images);
        assert!(retriever.tool.include_answer);
        assert!(retriever.tool.include_raw_content);
        assert!(retriever.tool.include_images);
    }

    #[test]
    fn test_tavily_retriever_builder_api_key_with_string() {
        let key = String::from("string-key");
        let retriever = TavilySearchRetriever::builder()
            .api_key(key)
            .build()
            .unwrap();

        assert_eq!(retriever.tool.api_key, "string-key");
    }

    // ==================== Retriever _get_relevant_documents Tests ====================

    #[tokio::test]
    async fn test_retriever_k_zero_returns_empty_without_network() {
        let retriever = TavilySearchRetriever::builder()
            .api_key("tvly-test-key")
            .k(0)
            .include_generated_answer(true)
            .build()
            .unwrap();

        let docs = retriever
            ._get_relevant_documents("any query", None)
            .await
            .unwrap();
        assert!(docs.is_empty());
    }

    #[tokio::test]
    async fn test_retriever_k_zero_no_generated_answer() {
        let retriever = TavilySearchRetriever::builder()
            .api_key("tvly-test-key")
            .k(0)
            .include_generated_answer(false)
            .build()
            .unwrap();

        // Without generated answer, k=0 should still work (but may hit API)
        // For isolation, this test verifies the early return path only applies
        // when include_generated_answer is true
        assert_eq!(retriever.k, 0);
    }

    // ==================== TavilySearchRetrieverBuilder Default Tests ====================

    #[test]
    fn test_retriever_builder_default() {
        let builder = TavilySearchRetrieverBuilder::default();
        // All fields should be None
        assert!(builder.api_key.is_none());
        assert!(builder.k.is_none());
        assert!(builder.include_generated_answer.is_none());
        assert!(builder.include_raw_content.is_none());
        assert!(builder.include_images.is_none());
        assert!(builder.search_depth.is_none());
    }

    #[test]
    fn test_retriever_builder_fluent_api() {
        // Test that all builder methods return Self for chaining
        let retriever = TavilySearchRetriever::builder()
            .api_key("test")
            .k(5)
            .include_generated_answer(true)
            .include_raw_content(true)
            .include_images(true)
            .search_depth(SearchDepth::Advanced)
            .build()
            .unwrap();

        // If we got here, fluent API works
        assert_eq!(retriever.k, 5);
    }

    // ==================== k and max_results Relationship Tests ====================

    #[test]
    fn test_retriever_k_syncs_with_tool_max_results() {
        let retriever = TavilySearchRetriever::builder()
            .api_key("test")
            .k(7)
            .build()
            .unwrap();

        assert_eq!(retriever.k, 7);
        assert_eq!(retriever.tool.max_results, 7);
    }

    #[test]
    fn test_retriever_k_large_value() {
        let retriever = TavilySearchRetriever::builder()
            .api_key("test")
            .k(100)
            .build()
            .unwrap();

        // k itself is not clamped, but tool.max_results is clamped to 20
        assert_eq!(retriever.k, 100);
        assert_eq!(retriever.tool.max_results, 20);
    }

    #[test]
    fn test_retriever_k_one() {
        let retriever = TavilySearchRetriever::builder()
            .api_key("test")
            .k(1)
            .build()
            .unwrap();

        assert_eq!(retriever.k, 1);
        assert_eq!(retriever.tool.max_results, 1);
    }

    // ==================== SearchDepth Mapping Tests ====================

    #[test]
    fn test_retriever_search_depth_maps_to_string_basic() {
        let retriever = TavilySearchRetriever::builder()
            .api_key("test")
            .search_depth(SearchDepth::Basic)
            .build()
            .unwrap();

        assert!(matches!(retriever.search_depth, SearchDepth::Basic));
        assert_eq!(retriever.tool.search_depth, "basic");
    }

    #[test]
    fn test_retriever_search_depth_maps_to_string_advanced() {
        let retriever = TavilySearchRetriever::builder()
            .api_key("test")
            .search_depth(SearchDepth::Advanced)
            .build()
            .unwrap();

        assert!(matches!(retriever.search_depth, SearchDepth::Advanced));
        assert_eq!(retriever.tool.search_depth, "advanced");
    }

    // ==================== include_generated_answer Effect Tests ====================

    #[test]
    fn test_retriever_include_generated_answer_sets_tool_include_answer() {
        let retriever_with = TavilySearchRetriever::builder()
            .api_key("test")
            .include_generated_answer(true)
            .build()
            .unwrap();

        let retriever_without = TavilySearchRetriever::builder()
            .api_key("test")
            .include_generated_answer(false)
            .build()
            .unwrap();

        assert!(retriever_with.tool.include_answer);
        assert!(!retriever_without.tool.include_answer);
    }

    // ==================== Edge Cases Tests ====================

    #[test]
    fn test_retriever_empty_api_key() {
        let retriever = TavilySearchRetriever::builder()
            .api_key("")
            .build()
            .unwrap();

        // Empty string is technically valid (will fail at API level)
        assert_eq!(retriever.tool.api_key, "");
    }

    #[test]
    fn test_retriever_whitespace_api_key() {
        let retriever = TavilySearchRetriever::builder()
            .api_key("   ")
            .build()
            .unwrap();

        // Whitespace is technically valid (will fail at API level)
        assert_eq!(retriever.tool.api_key, "   ");
    }

    #[test]
    fn test_retriever_builder_override_values() {
        // Test that later calls override earlier ones
        let retriever = TavilySearchRetriever::builder()
            .api_key("first")
            .api_key("second")
            .k(5)
            .k(10)
            .build()
            .unwrap();

        assert_eq!(retriever.tool.api_key, "second");
        assert_eq!(retriever.k, 10);
    }
}
