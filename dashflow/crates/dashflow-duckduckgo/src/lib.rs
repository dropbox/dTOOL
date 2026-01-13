//! # `DuckDuckGo` Search Tool
//!
//! `DuckDuckGo` is a privacy-focused search engine that doesn't track users.
//! This tool provides web search functionality for `DashFlow` agents.
//!
//! ## Features
//!
//! - Privacy-focused web search (no user tracking)
//! - HTML-based search results extraction
//! - Configurable result count
//! - No API key required
//!
//! ## Usage
//!
//! ```rust,no_run
//! use dashflow_duckduckgo::DuckDuckGoSearchTool;
//! use dashflow::core::tools::Tool;
//!
//! # tokio_test::block_on(async {
//! let ddg = DuckDuckGoSearchTool::new();
//!
//! // Search the web
//! let results = ddg._call_str("Rust programming language".to_string()).await.unwrap();
//! println!("Search results: {}", results);
//! # });
//! ```

use async_trait::async_trait;
use dashflow::core::tools::{Tool, ToolInput};
use dashflow::core::Result;
use dashflow::{
    DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT, DEFAULT_POOL_IDLE_TIMEOUT,
    DEFAULT_TCP_KEEPALIVE,
};
use scraper::{Html, Selector};
use serde_json::json;

/// `DuckDuckGo` search tool for `DashFlow` agents
///
/// This tool provides privacy-focused web search functionality, allowing agents
/// to search the internet without tracking.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_duckduckgo::DuckDuckGoSearchTool;
/// use dashflow::core::tools::Tool;
///
/// # tokio_test::block_on(async {
/// let ddg = DuckDuckGoSearchTool::builder()
///     .max_results(5)
///     .build();
///
/// let results = ddg._call_str("quantum computing".to_string())
///     .await
///     .unwrap();
/// println!("Found: {}", results);
/// # });
/// ```
#[derive(Debug, Clone)]
pub struct DuckDuckGoSearchTool {
    max_results: usize,
    client: reqwest::Client,
}

impl DuckDuckGoSearchTool {
    /// Create a new `DuckDuckGo` search tool with default settings
    ///
    /// Default settings:
    /// - `max_results`: 5
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_duckduckgo::DuckDuckGoSearchTool;
    ///
    /// let ddg = DuckDuckGoSearchTool::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_results: 5,
            client: reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
                .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
                .pool_max_idle_per_host(32)
                .pool_idle_timeout(DEFAULT_POOL_IDLE_TIMEOUT)
                .tcp_keepalive(DEFAULT_TCP_KEEPALIVE)
                .build()
                .unwrap_or_default(),
        }
    }

    /// Create a builder for `DuckDuckGoSearchTool`
    #[must_use]
    pub fn builder() -> DuckDuckGoSearchToolBuilder {
        DuckDuckGoSearchToolBuilder::default()
    }

    /// Search `DuckDuckGo` and retrieve search results
    async fn search(&self, query: String) -> Result<String> {
        // Build the search URL
        let encoded_query = urlencoding::encode(&query);
        let url = format!("https://html.duckduckgo.com/html/?q={encoded_query}");

        // Make the HTTP request
        let response = self.client.get(&url).send().await.map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to fetch search results: {e}"))
        })?;

        let html = response.text().await.map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to read response body: {e}"))
        })?;

        // Parse the HTML
        let document = Html::parse_document(&html);

        // Select search result elements
        // DuckDuckGo HTML uses <div class="result"> for each result
        let result_selector = Selector::parse("div.result").map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to parse result selector: {e}"))
        })?;

        let title_selector = Selector::parse("a.result__a").map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to parse title selector: {e}"))
        })?;

        let snippet_selector = Selector::parse("a.result__snippet").map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to parse snippet selector: {e}"))
        })?;

        let url_selector = Selector::parse("a.result__url").map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to parse URL selector: {e}"))
        })?;

        // Extract results
        let mut results = Vec::new();
        for result in document.select(&result_selector).take(self.max_results) {
            let title = result.select(&title_selector).next().map_or_else(
                || "No title".to_string(),
                |el| el.text().collect::<String>(),
            );

            let snippet = result.select(&snippet_selector).next().map_or_else(
                || "No snippet".to_string(),
                |el| el.text().collect::<String>(),
            );

            let url = result
                .select(&url_selector)
                .next()
                .map_or_else(|| "No URL".to_string(), |el| el.text().collect::<String>());

            results.push(format!(
                "[{}]\nURL: {}\nSnippet: {}\n",
                title.trim(),
                url.trim(),
                snippet.trim()
            ));
        }

        if results.is_empty() {
            Ok(format!(
                "No search results found for query: '{query}'\n\nTry:\n- Using different keywords\n- Making the query more specific\n- Checking for typos"
            ))
        } else {
            Ok(format!(
                "DuckDuckGo Search Results for: '{}'\n\nFound {} results:\n\n{}",
                query,
                results.len(),
                results.join("\n")
            ))
        }
    }
}

impl Default for DuckDuckGoSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for DuckDuckGoSearchTool {
    fn name(&self) -> &'static str {
        "duckduckgo_search"
    }

    fn description(&self) -> &'static str {
        "Search DuckDuckGo for web results. \
         Input should be a search query string. \
         Returns search results with titles, URLs, and snippets. \
         DuckDuckGo is privacy-focused and doesn't track users."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query to look up on DuckDuckGo"
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

/// Builder for `DuckDuckGoSearchTool`
///
/// # Example
///
/// ```rust
/// use dashflow_duckduckgo::DuckDuckGoSearchTool;
///
/// let ddg = DuckDuckGoSearchTool::builder()
///     .max_results(10)
///     .build();
/// ```
#[derive(Debug, Clone, Default)]
pub struct DuckDuckGoSearchToolBuilder {
    max_results: Option<usize>,
}

impl DuckDuckGoSearchToolBuilder {
    /// Set the maximum number of search results to return
    ///
    /// Default: 5
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_duckduckgo::DuckDuckGoSearchTool;
    ///
    /// let ddg = DuckDuckGoSearchTool::builder()
    ///     .max_results(10)
    ///     .build();
    /// ```
    #[must_use]
    pub fn max_results(mut self, max_results: usize) -> Self {
        self.max_results = Some(max_results);
        self
    }

    /// Build the `DuckDuckGoSearchTool`
    #[must_use]
    pub fn build(self) -> DuckDuckGoSearchTool {
        DuckDuckGoSearchTool {
            max_results: self.max_results.unwrap_or(5),
            client: reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
                .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
                .pool_max_idle_per_host(32)
                .pool_idle_timeout(DEFAULT_POOL_IDLE_TIMEOUT)
                .tcp_keepalive(DEFAULT_TCP_KEEPALIVE)
                .build()
                .unwrap_or_default(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use dashflow::core::tools::Tool;

    // ========================================================================
    // Constructor and Builder Tests
    // ========================================================================

    #[test]
    fn test_new() {
        let ddg = DuckDuckGoSearchTool::new();
        assert_eq!(ddg.max_results, 5);
    }

    #[test]
    fn test_default_trait() {
        let ddg = DuckDuckGoSearchTool::default();
        assert_eq!(ddg.max_results, 5);
    }

    #[test]
    fn test_default_equals_new() {
        let ddg_new = DuckDuckGoSearchTool::new();
        let ddg_default = DuckDuckGoSearchTool::default();
        assert_eq!(ddg_new.max_results, ddg_default.max_results);
    }

    #[test]
    fn test_builder_defaults() {
        let ddg = DuckDuckGoSearchTool::builder().build();
        assert_eq!(ddg.max_results, 5);
    }

    #[test]
    fn test_builder_custom() {
        let ddg = DuckDuckGoSearchTool::builder().max_results(10).build();
        assert_eq!(ddg.max_results, 10);
    }

    #[test]
    fn test_builder_max_results_zero() {
        let ddg = DuckDuckGoSearchTool::builder().max_results(0).build();
        assert_eq!(ddg.max_results, 0);
    }

    #[test]
    fn test_builder_max_results_one() {
        let ddg = DuckDuckGoSearchTool::builder().max_results(1).build();
        assert_eq!(ddg.max_results, 1);
    }

    #[test]
    fn test_builder_max_results_large() {
        let ddg = DuckDuckGoSearchTool::builder().max_results(1000).build();
        assert_eq!(ddg.max_results, 1000);
    }

    #[test]
    fn test_builder_max_results_usize_max() {
        let ddg = DuckDuckGoSearchTool::builder()
            .max_results(usize::MAX)
            .build();
        assert_eq!(ddg.max_results, usize::MAX);
    }

    #[test]
    fn test_builder_chaining_multiple_calls() {
        // Last call wins
        let ddg = DuckDuckGoSearchTool::builder()
            .max_results(5)
            .max_results(10)
            .max_results(3)
            .build();
        assert_eq!(ddg.max_results, 3);
    }

    #[test]
    fn test_builder_is_clone() {
        let builder = DuckDuckGoSearchTool::builder().max_results(7);
        let builder_clone = builder.clone();
        let ddg = builder_clone.build();
        assert_eq!(ddg.max_results, 7);
    }

    #[test]
    fn test_builder_is_debug() {
        let builder = DuckDuckGoSearchTool::builder().max_results(7);
        let debug_str = format!("{:?}", builder);
        assert!(debug_str.contains("DuckDuckGoSearchToolBuilder"));
        assert!(debug_str.contains("7"));
    }

    #[test]
    fn test_builder_default_trait() {
        let builder = DuckDuckGoSearchToolBuilder::default();
        let ddg = builder.build();
        assert_eq!(ddg.max_results, 5); // default
    }

    // ========================================================================
    // Tool Trait Implementation Tests
    // ========================================================================

    #[test]
    fn test_name() {
        let ddg = DuckDuckGoSearchTool::new();
        assert_eq!(ddg.name(), "duckduckgo_search");
    }

    #[test]
    fn test_name_not_empty() {
        let ddg = DuckDuckGoSearchTool::new();
        let name = ddg.name();
        assert!(!name.is_empty());
        assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    }

    #[test]
    fn test_description() {
        let ddg = DuckDuckGoSearchTool::new();
        let desc = ddg.description();
        assert!(desc.contains("DuckDuckGo"));
        assert!(desc.contains("privacy"));
    }

    #[test]
    fn test_description_mentions_search() {
        let ddg = DuckDuckGoSearchTool::new();
        let desc = ddg.description();
        assert!(desc.to_lowercase().contains("search"));
    }

    #[test]
    fn test_description_not_empty() {
        let ddg = DuckDuckGoSearchTool::new();
        let desc = ddg.description();
        assert!(!desc.is_empty());
        assert!(desc.len() > 10, "Description should be meaningful");
    }

    #[test]
    fn test_args_schema() {
        let ddg = DuckDuckGoSearchTool::new();
        let schema = ddg.args_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
        assert_eq!(schema["required"][0], "query");
    }

    #[test]
    fn test_args_schema_query_type() {
        let ddg = DuckDuckGoSearchTool::new();
        let schema = ddg.args_schema();
        assert_eq!(schema["properties"]["query"]["type"], "string");
    }

    #[test]
    fn test_args_schema_query_description() {
        let ddg = DuckDuckGoSearchTool::new();
        let schema = ddg.args_schema();
        let query_desc = schema["properties"]["query"]["description"]
            .as_str()
            .unwrap();
        assert!(query_desc.contains("search") || query_desc.contains("query"));
    }

    #[test]
    fn test_args_schema_required_array() {
        let ddg = DuckDuckGoSearchTool::new();
        let schema = ddg.args_schema();
        assert!(schema["required"].is_array());
        assert_eq!(schema["required"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_args_schema_is_valid_json() {
        let ddg = DuckDuckGoSearchTool::new();
        let schema = ddg.args_schema();
        // Should serialize/deserialize without error
        let serialized = serde_json::to_string(&schema).unwrap();
        let _: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    }

    // ========================================================================
    // Clone and Debug Trait Tests
    // ========================================================================

    #[test]
    fn test_tool_is_clone() {
        let ddg = DuckDuckGoSearchTool::builder().max_results(7).build();
        let ddg_clone = ddg.clone();
        assert_eq!(ddg.max_results, ddg_clone.max_results);
    }

    #[test]
    fn test_tool_is_debug() {
        let ddg = DuckDuckGoSearchTool::builder().max_results(7).build();
        let debug_str = format!("{:?}", ddg);
        assert!(debug_str.contains("DuckDuckGoSearchTool"));
        assert!(debug_str.contains("7"));
    }

    #[test]
    fn test_cloned_tool_independent() {
        let ddg1 = DuckDuckGoSearchTool::builder().max_results(5).build();
        let ddg2 = ddg1.clone();
        // Both should have same max_results (cloned)
        assert_eq!(ddg1.max_results, ddg2.max_results);
        // But be independent instances
        assert_eq!(ddg1.max_results, 5);
        assert_eq!(ddg2.max_results, 5);
    }

    // ========================================================================
    // Send + Sync Bounds Tests
    // ========================================================================

    #[test]
    fn test_tool_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DuckDuckGoSearchTool>();
    }

    #[test]
    fn test_tool_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DuckDuckGoSearchTool>();
    }

    #[test]
    fn test_builder_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DuckDuckGoSearchToolBuilder>();
    }

    #[test]
    fn test_builder_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DuckDuckGoSearchToolBuilder>();
    }

    // ========================================================================
    // Input Parsing Tests (Async)
    // ========================================================================

    #[tokio::test]
    async fn test_call_with_string_input_format() {
        // This tests input parsing, but will fail on network
        // We verify it reaches the search phase (network error, not parse error)
        let ddg = DuckDuckGoSearchTool::new();
        let result = ddg._call(ToolInput::String("test query".to_string())).await;
        // Should either succeed or fail with network error, not parse error
        if let Err(e) = result {
            let err_str = e.to_string().to_lowercase();
            assert!(
                err_str.contains("fetch")
                    || err_str.contains("network")
                    || err_str.contains("connect")
                    || err_str.contains("dns")
                    || err_str.contains("timeout"),
                "Expected network error, got: {}",
                e
            );
        }
    }

    #[tokio::test]
    async fn test_call_with_structured_input_format() {
        let ddg = DuckDuckGoSearchTool::new();
        let input = ToolInput::Structured(json!({"query": "test query"}));
        let result = ddg._call(input).await;
        // Should either succeed or fail with network error, not parse error
        if let Err(e) = result {
            let err_str = e.to_string().to_lowercase();
            assert!(
                err_str.contains("fetch")
                    || err_str.contains("network")
                    || err_str.contains("connect")
                    || err_str.contains("dns")
                    || err_str.contains("timeout"),
                "Expected network error, got: {}",
                e
            );
        }
    }

    #[tokio::test]
    async fn test_call_missing_query_field() {
        let ddg = DuckDuckGoSearchTool::new();
        let input = ToolInput::Structured(json!({"not_query": "value"}));
        let result = ddg._call(input).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("query") || err.contains("Missing"),
            "Expected missing query error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_call_empty_structured_input() {
        let ddg = DuckDuckGoSearchTool::new();
        let input = ToolInput::Structured(json!({}));
        let result = ddg._call(input).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("query") || err.contains("Missing"),
            "Expected missing query error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_call_query_null() {
        let ddg = DuckDuckGoSearchTool::new();
        let input = ToolInput::Structured(json!({"query": null}));
        let result = ddg._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_query_number() {
        let ddg = DuckDuckGoSearchTool::new();
        let input = ToolInput::Structured(json!({"query": 12345}));
        let result = ddg._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_query_boolean() {
        let ddg = DuckDuckGoSearchTool::new();
        let input = ToolInput::Structured(json!({"query": true}));
        let result = ddg._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_query_array() {
        let ddg = DuckDuckGoSearchTool::new();
        let input = ToolInput::Structured(json!({"query": ["a", "b"]}));
        let result = ddg._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_query_object() {
        let ddg = DuckDuckGoSearchTool::new();
        let input = ToolInput::Structured(json!({"query": {"nested": "value"}}));
        let result = ddg._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_extra_fields_ignored() {
        // Extra fields should be ignored, query should be extracted
        let ddg = DuckDuckGoSearchTool::new();
        let input = ToolInput::Structured(json!({
            "query": "test",
            "extra_field": "ignored",
            "another": 123
        }));
        let result = ddg._call(input).await;
        // Should attempt search (network error) not fail on parsing
        if let Err(e) = result {
            let err_str = e.to_string().to_lowercase();
            assert!(
                err_str.contains("fetch")
                    || err_str.contains("network")
                    || err_str.contains("connect")
                    || err_str.contains("dns")
                    || err_str.contains("timeout"),
                "Expected network error, got: {}",
                e
            );
        }
    }

    // ========================================================================
    // URL Encoding Tests
    // ========================================================================

    #[test]
    fn test_url_encoding_simple() {
        let encoded = urlencoding::encode("rust programming");
        assert_eq!(encoded, "rust%20programming");
    }

    #[test]
    fn test_url_encoding_special_chars() {
        let encoded = urlencoding::encode("c++ language");
        assert!(encoded.contains("%2B") || encoded.contains("+"));
    }

    #[test]
    fn test_url_encoding_unicode() {
        let encoded = urlencoding::encode("日本語");
        assert!(encoded.contains("%"));
    }

    #[test]
    fn test_url_encoding_ampersand() {
        let encoded = urlencoding::encode("rock & roll");
        assert!(encoded.contains("%26"));
    }

    #[test]
    fn test_url_encoding_question_mark() {
        let encoded = urlencoding::encode("what is rust?");
        assert!(encoded.contains("%3F"));
    }

    #[test]
    fn test_url_encoding_hash() {
        let encoded = urlencoding::encode("rust #programming");
        assert!(encoded.contains("%23"));
    }

    #[test]
    fn test_url_encoding_empty() {
        let encoded = urlencoding::encode("");
        assert_eq!(encoded, "");
    }

    #[test]
    fn test_url_encoding_only_spaces() {
        let encoded = urlencoding::encode("   ");
        assert_eq!(encoded, "%20%20%20");
    }

    // ========================================================================
    // Tool Configuration Consistency Tests
    // ========================================================================

    #[test]
    fn test_multiple_instances_independent() {
        let ddg1 = DuckDuckGoSearchTool::builder().max_results(3).build();
        let ddg2 = DuckDuckGoSearchTool::builder().max_results(10).build();
        assert_eq!(ddg1.max_results, 3);
        assert_eq!(ddg2.max_results, 10);
    }

    #[test]
    fn test_name_consistent_across_configs() {
        let ddg1 = DuckDuckGoSearchTool::builder().max_results(1).build();
        let ddg2 = DuckDuckGoSearchTool::builder().max_results(100).build();
        assert_eq!(ddg1.name(), ddg2.name());
    }

    #[test]
    fn test_description_consistent_across_configs() {
        let ddg1 = DuckDuckGoSearchTool::builder().max_results(1).build();
        let ddg2 = DuckDuckGoSearchTool::builder().max_results(100).build();
        assert_eq!(ddg1.description(), ddg2.description());
    }

    #[test]
    fn test_schema_consistent_across_configs() {
        let ddg1 = DuckDuckGoSearchTool::builder().max_results(1).build();
        let ddg2 = DuckDuckGoSearchTool::builder().max_results(100).build();
        assert_eq!(ddg1.args_schema(), ddg2.args_schema());
    }

    // ========================================================================
    // Edge Case Tests
    // ========================================================================

    #[tokio::test]
    async fn test_call_str_empty_query() {
        let ddg = DuckDuckGoSearchTool::new();
        let result = ddg._call_str("".to_string()).await;
        // Empty query might succeed or fail depending on DuckDuckGo behavior
        // But should not panic
        let _ = result;
    }

    #[tokio::test]
    async fn test_call_str_whitespace_only() {
        let ddg = DuckDuckGoSearchTool::new();
        let result = ddg._call_str("   ".to_string()).await;
        // Whitespace query might succeed or fail, should not panic
        let _ = result;
    }

    #[tokio::test]
    async fn test_call_str_very_long_query() {
        let ddg = DuckDuckGoSearchTool::new();
        let long_query = "rust ".repeat(1000);
        let result = ddg._call_str(long_query).await;
        // Long query might fail, should not panic
        let _ = result;
    }

    #[tokio::test]
    async fn test_call_str_unicode_query() {
        let ddg = DuckDuckGoSearchTool::new();
        let result = ddg._call_str("日本語プログラミング".to_string()).await;
        // Unicode query might succeed or fail, should not panic
        let _ = result;
    }

    #[tokio::test]
    async fn test_call_str_emoji_query() {
        let ddg = DuckDuckGoSearchTool::new();
        let result = ddg._call_str("🦀 rust programming".to_string()).await;
        // Emoji query might succeed or fail, should not panic
        let _ = result;
    }

    #[tokio::test]
    async fn test_call_str_special_html_chars() {
        let ddg = DuckDuckGoSearchTool::new();
        let result = ddg
            ._call_str("<script>alert('xss')</script>".to_string())
            .await;
        // HTML chars should be safely encoded, not cause issues
        let _ = result;
    }

    #[tokio::test]
    async fn test_call_str_sql_injection_attempt() {
        let ddg = DuckDuckGoSearchTool::new();
        let result = ddg
            ._call_str("'; DROP TABLE users; --".to_string())
            .await;
        // SQL injection should be harmless in URL encoding
        let _ = result;
    }

    // ========================================================================
    // Concurrent Usage Tests
    // ========================================================================

    #[tokio::test]
    async fn test_clone_can_be_used_concurrently() {
        let ddg = DuckDuckGoSearchTool::new();
        let ddg_clone = ddg.clone();

        // Both should be usable (even if network fails)
        let handle1 = tokio::spawn(async move {
            let _ = ddg._call_str("query1".to_string()).await;
        });

        let handle2 = tokio::spawn(async move {
            let _ = ddg_clone._call_str("query2".to_string()).await;
        });

        // Both should complete without panic
        let _ = handle1.await;
        let _ = handle2.await;
    }

    #[tokio::test]
    async fn test_shared_reference_concurrent() {
        use std::sync::Arc;

        let ddg = Arc::new(DuckDuckGoSearchTool::new());

        let ddg1 = Arc::clone(&ddg);
        let ddg2 = Arc::clone(&ddg);

        let handle1 = tokio::spawn(async move {
            let _ = ddg1._call_str("query1".to_string()).await;
        });

        let handle2 = tokio::spawn(async move {
            let _ = ddg2._call_str("query2".to_string()).await;
        });

        // Both should complete without panic
        let _ = handle1.await;
        let _ = handle2.await;
    }

    // ========================================================================
    // Builder Pattern Tests
    // ========================================================================

    #[test]
    fn test_builder_new_equals_default() {
        let builder1 = DuckDuckGoSearchTool::builder();
        let builder2 = DuckDuckGoSearchToolBuilder::default();
        let ddg1 = builder1.build();
        let ddg2 = builder2.build();
        assert_eq!(ddg1.max_results, ddg2.max_results);
    }

    #[test]
    fn test_builder_can_be_stored() {
        let builder = DuckDuckGoSearchTool::builder().max_results(7);
        // Store builder for later use
        let stored = builder;
        let ddg = stored.build();
        assert_eq!(ddg.max_results, 7);
    }

    #[test]
    fn test_builder_partial_config() {
        // Only set some options
        let ddg = DuckDuckGoSearchTool::builder()
            .max_results(3)
            // Don't set other options (if any existed)
            .build();
        assert_eq!(ddg.max_results, 3);
    }

    // ========================================================================
    // Integration tests (require network access)
    // ========================================================================

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_search_basic() {
        let ddg = DuckDuckGoSearchTool::new();
        let output = ddg
            ._call_str("Rust programming language".to_string())
            .await
            .expect("DuckDuckGo search failed");
        assert!(output.contains("Rust"));
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_search_with_call() {
        let ddg = DuckDuckGoSearchTool::new();
        let input = ToolInput::Structured(json!({
            "query": "quantum computing"
        }));
        ddg._call(input).await.expect("DuckDuckGo search failed");
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_search_multiple_results() {
        let ddg = DuckDuckGoSearchTool::builder().max_results(3).build();
        let output = ddg
            ._call_str("machine learning".to_string())
            .await
            .expect("DuckDuckGo search failed");
        assert!(output.contains("Search Results"));
    }
}
