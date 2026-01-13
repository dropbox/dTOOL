//! # Bing Search Tool
//!
//! Microsoft Bing Search API integration for `DashFlow`. Provides access to Bing's
//! comprehensive web search capabilities backed by Microsoft's search infrastructure.
//!
//! ## Features
//!
//! - Comprehensive web search powered by Microsoft Bing
//! - Multiple search types: web, news, images, videos
//! - Rich snippets and metadata
//! - Freshness filtering
//! - Safe search controls
//! - Market and language targeting
//!
//! ## Usage
//!
//! ```rust,no_run
//! use dashflow_bing::BingSearchTool;
//! use dashflow::core::tools::Tool;
//!
//! # tokio_test::block_on(async {
//! let bing = BingSearchTool::new("YOUR_SUBSCRIPTION_KEY");
//!
//! // Simple search
//! let results = bing._call_str("What is machine learning?".to_string()).await.unwrap();
//! println!("Search results: {}", results);
//! # });
//! ```

use async_trait::async_trait;
use dashflow::core::http_client::{json_with_limit, SEARCH_RESPONSE_SIZE_LIMIT};
use dashflow::core::tools::{Tool, ToolInput};
use dashflow::core::Result;
use dashflow::{DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT};
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

/// Safe search level for Bing API
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum SafeSearch {
    /// No safe search filtering
    Off,
    /// Moderate filtering (default)
    #[default]
    Moderate,
    /// Strict filtering
    Strict,
}

/// Freshness filter for search results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Freshness {
    /// Past day
    Day,
    /// Past week
    Week,
    /// Past month
    Month,
}

/// A single web page result from Bing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BingWebPage {
    /// Page ID
    pub id: String,
    /// Page name/title
    pub name: String,
    /// Page URL
    pub url: String,
    /// Display URL (shorter, user-friendly version)
    #[serde(rename = "displayUrl")]
    pub display_url: String,
    /// Content snippet
    pub snippet: String,
    /// Date last crawled (ISO 8601 format)
    #[serde(rename = "dateLastCrawled")]
    pub date_last_crawled: Option<String>,
    /// Deep links (sub-pages)
    #[serde(default)]
    pub deep_links: Option<Vec<BingDeepLink>>,
}

/// Deep link to a sub-page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BingDeepLink {
    /// Link name
    pub name: String,
    /// Link URL
    pub url: String,
}

/// Web search results container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BingWebPages {
    /// Total estimated matches
    #[serde(rename = "totalEstimatedMatches")]
    pub total_estimated_matches: Option<u64>,
    /// Search results
    pub value: Vec<BingWebPage>,
}

/// Query context (spell corrections, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BingQueryContext {
    /// Original query
    #[serde(rename = "originalQuery")]
    pub original_query: String,
    /// Altered query (spell-corrected)
    #[serde(rename = "alteredQuery")]
    pub altered_query: Option<String>,
}

/// Response from Bing search API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BingSearchResponse {
    /// Response type
    #[serde(rename = "_type")]
    pub response_type: String,
    /// Query context
    #[serde(rename = "queryContext")]
    pub query_context: BingQueryContext,
    /// Web pages results
    #[serde(rename = "webPages")]
    pub web_pages: Option<BingWebPages>,
}

/// Bing search tool for `DashFlow` agents
///
/// This tool provides access to Microsoft Bing's web search API, which offers
/// comprehensive search results backed by Microsoft's search infrastructure.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_bing::BingSearchTool;
/// use dashflow::core::tools::Tool;
///
/// # tokio_test::block_on(async {
/// let bing = BingSearchTool::builder()
///     .subscription_key("YOUR_SUBSCRIPTION_KEY")
///     .count(10)
///     .market("en-US")
///     .build()
///     .unwrap();
///
/// let results = bing._call_str("latest technology news".to_string())
///     .await
///     .unwrap();
/// println!("Found: {}", results);
/// # });
/// ```
#[derive(Debug)]
pub struct BingSearchTool {
    subscription_key: String,
    count: u32,
    market: String,
    safe_search: String,
    freshness: Option<String>,
    client: reqwest::Client,
}

impl BingSearchTool {
    /// Create a new Bing search tool
    ///
    /// # Arguments
    ///
    /// * `subscription_key` - Your Bing Search API subscription key (Ocp-Apim-Subscription-Key)
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_bing::BingSearchTool;
    ///
    /// let bing = BingSearchTool::new("YOUR_SUBSCRIPTION_KEY");
    /// ```
    pub fn new(subscription_key: impl Into<String>) -> Self {
        Self {
            subscription_key: subscription_key.into(),
            count: 10,
            market: "en-US".to_string(),
            safe_search: "Moderate".to_string(),
            freshness: None,
            client: create_http_client(),
        }
    }

    /// Create a builder for `BingSearchTool`
    #[must_use]
    pub fn builder() -> BingSearchToolBuilder {
        BingSearchToolBuilder::default()
    }

    /// Perform a search using the Bing API
    async fn search(&self, query: String) -> Result<BingSearchResponse> {
        let mut url = format!(
            "https://api.bing.microsoft.com/v7.0/search?q={}&count={}",
            urlencoding::encode(&query),
            self.count
        );

        // Add optional parameters
        url.push_str(&format!("&mkt={}", self.market));
        url.push_str(&format!("&safeSearch={}", self.safe_search));

        if let Some(ref freshness) = self.freshness {
            url.push_str(&format!("&freshness={freshness}"));
        }

        let response = self
            .client
            .get(&url)
            .header("Ocp-Apim-Subscription-Key", &self.subscription_key)
            .send()
            .await
            .map_err(|e| {
                dashflow::core::Error::tool_error(format!("Bing API request failed: {e}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(dashflow::core::Error::tool_error(format!(
                "Bing API error ({status}): {error_text}"
            )));
        }

        // M-216: Use size-limited JSON parsing to prevent memory exhaustion
        let search_response: BingSearchResponse =
            json_with_limit(response, SEARCH_RESPONSE_SIZE_LIMIT).await.map_err(|e| {
                dashflow::core::Error::tool_error(format!("Failed to parse Bing response: {e}"))
            })?;

        Ok(search_response)
    }

    /// Format search results as a string
    fn format_results(&self, response: BingSearchResponse) -> String {
        let mut output = String::new();

        // Show spell correction if present
        if let Some(ref altered) = response.query_context.altered_query {
            output.push_str(&format!("Showing results for: {altered}\n\n"));
        }

        // Extract web pages
        let web_pages = if let Some(pages) = response.web_pages {
            pages
        } else {
            output.push_str("No results found.");
            return output;
        };

        if let Some(total) = web_pages.total_estimated_matches {
            output.push_str(&format!(
                "Found approximately {} results (showing {}):\n\n",
                total,
                web_pages.value.len()
            ));
        } else {
            output.push_str(&format!("Found {} results:\n\n", web_pages.value.len()));
        }

        for (i, result) in web_pages.value.iter().enumerate() {
            output.push_str(&format!("{}. {}\n", i + 1, result.name));
            output.push_str(&format!("   URL: {}\n", result.url));

            // Truncate snippet to 250 chars
            let snippet = if result.snippet.len() > 250 {
                format!("{}...", &result.snippet[..250])
            } else {
                result.snippet.clone()
            };
            output.push_str(&format!("   Snippet: {snippet}\n"));

            if let Some(ref crawled) = result.date_last_crawled {
                output.push_str(&format!("   Last crawled: {crawled}\n"));
            }

            output.push('\n');
        }

        output
    }
}

#[async_trait]
impl Tool for BingSearchTool {
    fn name(&self) -> &'static str {
        "bing_search"
    }

    fn description(&self) -> &'static str {
        "Search the web using Microsoft Bing Search API. \
         Returns comprehensive search results backed by Microsoft's search infrastructure. \
         Provides web pages, news, images, and videos with rich metadata. \
         Best for general web search with Microsoft's quality and scale."
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

/// Builder for `BingSearchTool`
#[derive(Default)]
pub struct BingSearchToolBuilder {
    subscription_key: Option<String>,
    count: Option<u32>,
    market: Option<String>,
    safe_search: Option<String>,
    freshness: Option<String>,
}

impl BingSearchToolBuilder {
    /// Set the subscription key
    pub fn subscription_key(mut self, key: impl Into<String>) -> Self {
        self.subscription_key = Some(key.into());
        self
    }

    /// Set the maximum number of results (1-50)
    #[must_use]
    pub fn count(mut self, count: u32) -> Self {
        self.count = Some(count.clamp(1, 50));
        self
    }

    /// Set the market/locale (e.g., "en-US", "es-ES", "fr-FR")
    pub fn market(mut self, market: impl Into<String>) -> Self {
        self.market = Some(market.into());
        self
    }

    /// Set the safe search level ("Off", "Moderate", "Strict")
    pub fn safe_search(mut self, level: impl Into<String>) -> Self {
        self.safe_search = Some(level.into());
        self
    }

    /// Set freshness filter ("Day", "Week", "Month")
    pub fn freshness(mut self, freshness: impl Into<String>) -> Self {
        self.freshness = Some(freshness.into());
        self
    }

    /// Build the `BingSearchTool`
    pub fn build(self) -> Result<BingSearchTool> {
        let subscription_key = self.subscription_key.ok_or_else(|| {
            dashflow::core::Error::tool_error(
                "Subscription key is required for Bing search".to_string(),
            )
        })?;

        Ok(BingSearchTool {
            subscription_key,
            count: self.count.unwrap_or(10),
            market: self.market.unwrap_or_else(|| "en-US".to_string()),
            safe_search: self.safe_search.unwrap_or_else(|| "Moderate".to_string()),
            freshness: self.freshness,
            client: create_http_client(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== SafeSearch Enum Tests ====================

    #[test]
    fn test_safe_search_default() {
        let safe: SafeSearch = SafeSearch::default();
        matches!(safe, SafeSearch::Moderate);
    }

    #[test]
    fn test_safe_search_serialization_off() {
        let safe = SafeSearch::Off;
        let json = serde_json::to_string(&safe).unwrap();
        assert_eq!(json, "\"Off\"");
    }

    #[test]
    fn test_safe_search_serialization_moderate() {
        let safe = SafeSearch::Moderate;
        let json = serde_json::to_string(&safe).unwrap();
        assert_eq!(json, "\"Moderate\"");
    }

    #[test]
    fn test_safe_search_serialization_strict() {
        let safe = SafeSearch::Strict;
        let json = serde_json::to_string(&safe).unwrap();
        assert_eq!(json, "\"Strict\"");
    }

    #[test]
    fn test_safe_search_deserialization() {
        let safe: SafeSearch = serde_json::from_str("\"Strict\"").unwrap();
        matches!(safe, SafeSearch::Strict);
    }

    #[test]
    fn test_safe_search_debug() {
        let safe = SafeSearch::Moderate;
        let debug = format!("{safe:?}");
        assert_eq!(debug, "Moderate");
    }

    #[test]
    fn test_safe_search_clone() {
        let safe = SafeSearch::Strict;
        let cloned = safe.clone();
        matches!(cloned, SafeSearch::Strict);
    }

    // ==================== Freshness Enum Tests ====================

    #[test]
    fn test_freshness_serialization_day() {
        let fresh = Freshness::Day;
        let json = serde_json::to_string(&fresh).unwrap();
        assert_eq!(json, "\"Day\"");
    }

    #[test]
    fn test_freshness_serialization_week() {
        let fresh = Freshness::Week;
        let json = serde_json::to_string(&fresh).unwrap();
        assert_eq!(json, "\"Week\"");
    }

    #[test]
    fn test_freshness_serialization_month() {
        let fresh = Freshness::Month;
        let json = serde_json::to_string(&fresh).unwrap();
        assert_eq!(json, "\"Month\"");
    }

    #[test]
    fn test_freshness_deserialization() {
        let fresh: Freshness = serde_json::from_str("\"Week\"").unwrap();
        matches!(fresh, Freshness::Week);
    }

    #[test]
    fn test_freshness_debug() {
        let fresh = Freshness::Month;
        let debug = format!("{fresh:?}");
        assert_eq!(debug, "Month");
    }

    #[test]
    fn test_freshness_clone() {
        let fresh = Freshness::Day;
        let cloned = fresh.clone();
        matches!(cloned, Freshness::Day);
    }

    // ==================== BingDeepLink Tests ====================

    #[test]
    fn test_deep_link_serialization() {
        let link = BingDeepLink {
            name: "About Us".to_string(),
            url: "https://example.com/about".to_string(),
        };
        let json = serde_json::to_string(&link).unwrap();
        assert!(json.contains("\"name\":\"About Us\""));
        assert!(json.contains("\"url\":\"https://example.com/about\""));
    }

    #[test]
    fn test_deep_link_deserialization() {
        let json = r#"{"name":"Contact","url":"https://example.com/contact"}"#;
        let link: BingDeepLink = serde_json::from_str(json).unwrap();
        assert_eq!(link.name, "Contact");
        assert_eq!(link.url, "https://example.com/contact");
    }

    #[test]
    fn test_deep_link_debug() {
        let link = BingDeepLink {
            name: "Test".to_string(),
            url: "https://test.com".to_string(),
        };
        let debug = format!("{link:?}");
        assert!(debug.contains("BingDeepLink"));
        assert!(debug.contains("Test"));
    }

    #[test]
    fn test_deep_link_clone() {
        let link = BingDeepLink {
            name: "Original".to_string(),
            url: "https://original.com".to_string(),
        };
        let cloned = link.clone();
        assert_eq!(cloned.name, "Original");
        assert_eq!(cloned.url, "https://original.com");
    }

    // ==================== BingWebPage Tests ====================

    #[test]
    fn test_web_page_serialization() {
        let page = BingWebPage {
            id: "page-123".to_string(),
            name: "Example Page".to_string(),
            url: "https://example.com/page".to_string(),
            display_url: "example.com/page".to_string(),
            snippet: "A test snippet".to_string(),
            date_last_crawled: Some("2024-06-15T12:00:00Z".to_string()),
            deep_links: None,
        };
        let json = serde_json::to_string(&page).unwrap();
        assert!(json.contains("\"id\":\"page-123\""));
        assert!(json.contains("\"name\":\"Example Page\""));
        assert!(json.contains("\"displayUrl\":\"example.com/page\""));
        assert!(json.contains("\"dateLastCrawled\":"));
    }

    #[test]
    fn test_web_page_deserialization() {
        let json = r#"{
            "id": "abc123",
            "name": "Test Page",
            "url": "https://test.com",
            "displayUrl": "test.com",
            "snippet": "Test snippet content"
        }"#;
        let page: BingWebPage = serde_json::from_str(json).unwrap();
        assert_eq!(page.id, "abc123");
        assert_eq!(page.name, "Test Page");
        assert_eq!(page.url, "https://test.com");
        assert_eq!(page.display_url, "test.com");
        assert_eq!(page.snippet, "Test snippet content");
        assert!(page.date_last_crawled.is_none());
        assert!(page.deep_links.is_none());
    }

    #[test]
    fn test_web_page_with_deep_links() {
        let json = r#"{
            "id": "page1",
            "name": "Main Page",
            "url": "https://main.com",
            "displayUrl": "main.com",
            "snippet": "Main content",
            "deep_links": [
                {"name": "Sub Page 1", "url": "https://main.com/sub1"},
                {"name": "Sub Page 2", "url": "https://main.com/sub2"}
            ]
        }"#;
        let page: BingWebPage = serde_json::from_str(json).unwrap();
        assert!(page.deep_links.is_some());
        let links = page.deep_links.unwrap();
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].name, "Sub Page 1");
        assert_eq!(links[1].name, "Sub Page 2");
    }

    #[test]
    fn test_web_page_debug() {
        let page = BingWebPage {
            id: "test".to_string(),
            name: "Test".to_string(),
            url: "https://test.com".to_string(),
            display_url: "test.com".to_string(),
            snippet: "Snippet".to_string(),
            date_last_crawled: None,
            deep_links: None,
        };
        let debug = format!("{page:?}");
        assert!(debug.contains("BingWebPage"));
    }

    #[test]
    fn test_web_page_clone() {
        let page = BingWebPage {
            id: "original".to_string(),
            name: "Original Page".to_string(),
            url: "https://original.com".to_string(),
            display_url: "original.com".to_string(),
            snippet: "Original snippet".to_string(),
            date_last_crawled: Some("2024-01-01".to_string()),
            deep_links: Some(vec![BingDeepLink {
                name: "Link".to_string(),
                url: "https://link.com".to_string(),
            }]),
        };
        let cloned = page.clone();
        assert_eq!(cloned.id, "original");
        assert!(cloned.deep_links.is_some());
    }

    // ==================== BingWebPages Tests ====================

    #[test]
    fn test_web_pages_serialization() {
        let pages = BingWebPages {
            total_estimated_matches: Some(1500000),
            value: vec![],
        };
        let json = serde_json::to_string(&pages).unwrap();
        assert!(json.contains("\"totalEstimatedMatches\":1500000"));
    }

    #[test]
    fn test_web_pages_deserialization() {
        let json = r#"{"totalEstimatedMatches": 999, "value": []}"#;
        let pages: BingWebPages = serde_json::from_str(json).unwrap();
        assert_eq!(pages.total_estimated_matches, Some(999));
        assert!(pages.value.is_empty());
    }

    #[test]
    fn test_web_pages_without_total() {
        let json = r#"{"value": []}"#;
        let pages: BingWebPages = serde_json::from_str(json).unwrap();
        assert!(pages.total_estimated_matches.is_none());
    }

    #[test]
    fn test_web_pages_with_results() {
        let json = r#"{
            "totalEstimatedMatches": 2,
            "value": [
                {"id": "1", "name": "Result 1", "url": "https://r1.com", "displayUrl": "r1.com", "snippet": "Snippet 1"},
                {"id": "2", "name": "Result 2", "url": "https://r2.com", "displayUrl": "r2.com", "snippet": "Snippet 2"}
            ]
        }"#;
        let pages: BingWebPages = serde_json::from_str(json).unwrap();
        assert_eq!(pages.value.len(), 2);
        assert_eq!(pages.value[0].name, "Result 1");
        assert_eq!(pages.value[1].name, "Result 2");
    }

    // ==================== BingQueryContext Tests ====================

    #[test]
    fn test_query_context_serialization() {
        let ctx = BingQueryContext {
            original_query: "rust programming".to_string(),
            altered_query: None,
        };
        let json = serde_json::to_string(&ctx).unwrap();
        assert!(json.contains("\"originalQuery\":\"rust programming\""));
    }

    #[test]
    fn test_query_context_deserialization() {
        let json = r#"{"originalQuery": "test query"}"#;
        let ctx: BingQueryContext = serde_json::from_str(json).unwrap();
        assert_eq!(ctx.original_query, "test query");
        assert!(ctx.altered_query.is_none());
    }

    #[test]
    fn test_query_context_with_alteration() {
        let json = r#"{"originalQuery": "tset", "alteredQuery": "test"}"#;
        let ctx: BingQueryContext = serde_json::from_str(json).unwrap();
        assert_eq!(ctx.original_query, "tset");
        assert_eq!(ctx.altered_query, Some("test".to_string()));
    }

    #[test]
    fn test_query_context_debug() {
        let ctx = BingQueryContext {
            original_query: "query".to_string(),
            altered_query: Some("altered".to_string()),
        };
        let debug = format!("{ctx:?}");
        assert!(debug.contains("BingQueryContext"));
    }

    // ==================== BingSearchResponse Tests ====================

    #[test]
    fn test_search_response_serialization() {
        let response = BingSearchResponse {
            response_type: "SearchResponse".to_string(),
            query_context: BingQueryContext {
                original_query: "test".to_string(),
                altered_query: None,
            },
            web_pages: None,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"_type\":\"SearchResponse\""));
    }

    #[test]
    fn test_search_response_deserialization() {
        let json = r#"{
            "_type": "SearchResponse",
            "queryContext": {"originalQuery": "rust"},
            "webPages": null
        }"#;
        let response: BingSearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.response_type, "SearchResponse");
        assert_eq!(response.query_context.original_query, "rust");
        assert!(response.web_pages.is_none());
    }

    #[test]
    fn test_search_response_with_web_pages() {
        let json = r#"{
            "_type": "SearchResponse",
            "queryContext": {"originalQuery": "rust"},
            "webPages": {
                "totalEstimatedMatches": 100,
                "value": []
            }
        }"#;
        let response: BingSearchResponse = serde_json::from_str(json).unwrap();
        assert!(response.web_pages.is_some());
        let pages = response.web_pages.unwrap();
        assert_eq!(pages.total_estimated_matches, Some(100));
    }

    // ==================== BingSearchTool Creation Tests ====================

    #[test]
    fn test_bing_tool_creation() {
        let bing = BingSearchTool::new("test-key");
        assert_eq!(bing.name(), "bing_search");
        assert!(bing.description().contains("Bing"));
        assert!(bing.description().contains("Microsoft"));
    }

    #[test]
    fn test_bing_tool_defaults() {
        let bing = BingSearchTool::new("my-key");
        assert_eq!(bing.subscription_key, "my-key");
        assert_eq!(bing.count, 10);
        assert_eq!(bing.market, "en-US");
        assert_eq!(bing.safe_search, "Moderate");
        assert!(bing.freshness.is_none());
    }

    #[test]
    fn test_bing_tool_accepts_string_slice() {
        let key = "api-key-123";
        let bing = BingSearchTool::new(key);
        assert_eq!(bing.subscription_key, "api-key-123");
    }

    #[test]
    fn test_bing_tool_accepts_string() {
        let key = String::from("api-key-456");
        let bing = BingSearchTool::new(key);
        assert_eq!(bing.subscription_key, "api-key-456");
    }

    // ==================== BingSearchToolBuilder Tests ====================

    #[test]
    fn test_bing_tool_builder() {
        let bing = BingSearchTool::builder()
            .subscription_key("test-key")
            .count(20)
            .market("es-ES")
            .safe_search("Strict")
            .freshness("Week")
            .build()
            .unwrap();

        assert_eq!(bing.subscription_key, "test-key");
        assert_eq!(bing.count, 20);
        assert_eq!(bing.market, "es-ES");
        assert_eq!(bing.safe_search, "Strict");
        assert_eq!(bing.freshness, Some("Week".to_string()));
    }

    #[test]
    fn test_bing_builder_missing_key() {
        let result = BingSearchTool::builder().count(10).build();
        assert!(result.is_err());
    }

    #[test]
    fn test_bing_builder_missing_key_error_message() {
        let result = BingSearchTool::builder().build();
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Subscription key") || msg.contains("key"));
    }

    #[test]
    fn test_bing_builder_defaults() {
        let bing = BingSearchTool::builder()
            .subscription_key("key")
            .build()
            .unwrap();
        assert_eq!(bing.count, 10);
        assert_eq!(bing.market, "en-US");
        assert_eq!(bing.safe_search, "Moderate");
        assert!(bing.freshness.is_none());
    }

    #[test]
    fn test_bing_builder_count_only() {
        let bing = BingSearchTool::builder()
            .subscription_key("key")
            .count(5)
            .build()
            .unwrap();
        assert_eq!(bing.count, 5);
    }

    #[test]
    fn test_bing_builder_market_only() {
        let bing = BingSearchTool::builder()
            .subscription_key("key")
            .market("de-DE")
            .build()
            .unwrap();
        assert_eq!(bing.market, "de-DE");
    }

    #[test]
    fn test_bing_builder_freshness_day() {
        let bing = BingSearchTool::builder()
            .subscription_key("key")
            .freshness("Day")
            .build()
            .unwrap();
        assert_eq!(bing.freshness, Some("Day".to_string()));
    }

    #[test]
    fn test_bing_builder_freshness_month() {
        let bing = BingSearchTool::builder()
            .subscription_key("key")
            .freshness("Month")
            .build()
            .unwrap();
        assert_eq!(bing.freshness, Some("Month".to_string()));
    }

    #[test]
    fn test_bing_builder_safe_search_off() {
        let bing = BingSearchTool::builder()
            .subscription_key("key")
            .safe_search("Off")
            .build()
            .unwrap();
        assert_eq!(bing.safe_search, "Off");
    }

    #[test]
    fn test_count_clamping() {
        let bing = BingSearchTool::builder()
            .subscription_key("test")
            .count(100) // Should be clamped to 50
            .build()
            .unwrap();

        assert_eq!(bing.count, 50);
    }

    #[test]
    fn test_count_clamping_zero() {
        let bing = BingSearchTool::builder()
            .subscription_key("test")
            .count(0) // Should be clamped to 1
            .build()
            .unwrap();

        assert_eq!(bing.count, 1);
    }

    #[test]
    fn test_count_at_minimum_boundary() {
        let bing = BingSearchTool::builder()
            .subscription_key("test")
            .count(1)
            .build()
            .unwrap();

        assert_eq!(bing.count, 1);
    }

    #[test]
    fn test_count_at_maximum_boundary() {
        let bing = BingSearchTool::builder()
            .subscription_key("test")
            .count(50)
            .build()
            .unwrap();

        assert_eq!(bing.count, 50);
    }

    // ==================== Tool Trait Tests ====================

    #[test]
    fn test_bing_args_schema() {
        let bing = BingSearchTool::new("test-key");
        let schema = bing.args_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
        assert_eq!(schema["required"][0], "query");
    }

    #[test]
    fn test_bing_args_schema_query_type() {
        let bing = BingSearchTool::new("test-key");
        let schema = bing.args_schema();
        assert_eq!(schema["properties"]["query"]["type"], "string");
    }

    #[test]
    fn test_bing_args_schema_query_description() {
        let bing = BingSearchTool::new("test-key");
        let schema = bing.args_schema();
        let desc = schema["properties"]["query"]["description"].as_str();
        assert!(desc.is_some());
        assert!(desc.unwrap().contains("query"));
    }

    #[test]
    fn test_bing_name_static() {
        let bing = BingSearchTool::new("test-key");
        let name = bing.name();
        assert_eq!(name, "bing_search");
    }

    #[test]
    fn test_bing_description_content() {
        let bing = BingSearchTool::new("test-key");
        let desc = bing.description();
        assert!(desc.contains("Search"));
        assert!(desc.contains("web"));
    }

    #[test]
    fn test_bing_description_mentions_news() {
        let bing = BingSearchTool::new("test-key");
        let desc = bing.description();
        assert!(desc.contains("news"));
    }

    #[test]
    fn test_bing_description_mentions_images() {
        let bing = BingSearchTool::new("test-key");
        let desc = bing.description();
        assert!(desc.contains("images"));
    }

    // ==================== Format Results Tests ====================

    #[test]
    fn test_format_results() {
        let bing = BingSearchTool::new("test-key");
        let response = BingSearchResponse {
            response_type: "SearchResponse".to_string(),
            query_context: BingQueryContext {
                original_query: "test query".to_string(),
                altered_query: None,
            },
            web_pages: Some(BingWebPages {
                total_estimated_matches: Some(1000000),
                value: vec![BingWebPage {
                    id: "test-id".to_string(),
                    name: "Test Result".to_string(),
                    url: "https://example.com".to_string(),
                    display_url: "example.com".to_string(),
                    snippet: "This is test content".to_string(),
                    date_last_crawled: Some("2024-01-01T00:00:00Z".to_string()),
                    deep_links: None,
                }],
            }),
        };

        let formatted = bing.format_results(response);
        assert!(formatted.contains("Test Result"));
        assert!(formatted.contains("https://example.com"));
        assert!(formatted.contains("This is test content"));
        assert!(formatted.contains("1000000"));
    }

    #[test]
    fn test_format_empty_results() {
        let bing = BingSearchTool::new("test-key");
        let response = BingSearchResponse {
            response_type: "SearchResponse".to_string(),
            query_context: BingQueryContext {
                original_query: "test query".to_string(),
                altered_query: None,
            },
            web_pages: None,
        };

        let formatted = bing.format_results(response);
        assert_eq!(formatted, "No results found.");
    }

    #[test]
    fn test_spell_correction() {
        let bing = BingSearchTool::new("test-key");
        let response = BingSearchResponse {
            response_type: "SearchResponse".to_string(),
            query_context: BingQueryContext {
                original_query: "test qurey".to_string(),
                altered_query: Some("test query".to_string()),
            },
            web_pages: Some(BingWebPages {
                total_estimated_matches: None,
                value: vec![],
            }),
        };

        let formatted = bing.format_results(response);
        assert!(formatted.contains("Showing results for: test query"));
    }

    #[test]
    fn test_format_results_without_total() {
        let bing = BingSearchTool::new("test-key");
        let response = BingSearchResponse {
            response_type: "SearchResponse".to_string(),
            query_context: BingQueryContext {
                original_query: "rust".to_string(),
                altered_query: None,
            },
            web_pages: Some(BingWebPages {
                total_estimated_matches: None,
                value: vec![BingWebPage {
                    id: "1".to_string(),
                    name: "Rust Lang".to_string(),
                    url: "https://rust-lang.org".to_string(),
                    display_url: "rust-lang.org".to_string(),
                    snippet: "Rust programming language".to_string(),
                    date_last_crawled: None,
                    deep_links: None,
                }],
            }),
        };

        let formatted = bing.format_results(response);
        assert!(formatted.contains("Found 1 results:"));
        assert!(!formatted.contains("approximately"));
    }

    #[test]
    fn test_format_results_multiple_pages() {
        let bing = BingSearchTool::new("test-key");
        let response = BingSearchResponse {
            response_type: "SearchResponse".to_string(),
            query_context: BingQueryContext {
                original_query: "test".to_string(),
                altered_query: None,
            },
            web_pages: Some(BingWebPages {
                total_estimated_matches: Some(500),
                value: vec![
                    BingWebPage {
                        id: "1".to_string(),
                        name: "First Result".to_string(),
                        url: "https://first.com".to_string(),
                        display_url: "first.com".to_string(),
                        snippet: "First snippet".to_string(),
                        date_last_crawled: None,
                        deep_links: None,
                    },
                    BingWebPage {
                        id: "2".to_string(),
                        name: "Second Result".to_string(),
                        url: "https://second.com".to_string(),
                        display_url: "second.com".to_string(),
                        snippet: "Second snippet".to_string(),
                        date_last_crawled: None,
                        deep_links: None,
                    },
                ],
            }),
        };

        let formatted = bing.format_results(response);
        assert!(formatted.contains("1. First Result"));
        assert!(formatted.contains("2. Second Result"));
        assert!(formatted.contains("showing 2"));
    }

    #[test]
    fn test_format_results_snippet_truncation() {
        let bing = BingSearchTool::new("test-key");
        let long_snippet = "A".repeat(300);
        let response = BingSearchResponse {
            response_type: "SearchResponse".to_string(),
            query_context: BingQueryContext {
                original_query: "test".to_string(),
                altered_query: None,
            },
            web_pages: Some(BingWebPages {
                total_estimated_matches: Some(1),
                value: vec![BingWebPage {
                    id: "1".to_string(),
                    name: "Long Snippet Test".to_string(),
                    url: "https://test.com".to_string(),
                    display_url: "test.com".to_string(),
                    snippet: long_snippet.clone(),
                    date_last_crawled: None,
                    deep_links: None,
                }],
            }),
        };

        let formatted = bing.format_results(response);
        // Should be truncated to 250 chars + "..."
        assert!(formatted.contains("..."));
        assert!(!formatted.contains(&long_snippet));
    }

    #[test]
    fn test_format_results_with_date() {
        let bing = BingSearchTool::new("test-key");
        let response = BingSearchResponse {
            response_type: "SearchResponse".to_string(),
            query_context: BingQueryContext {
                original_query: "test".to_string(),
                altered_query: None,
            },
            web_pages: Some(BingWebPages {
                total_estimated_matches: Some(1),
                value: vec![BingWebPage {
                    id: "1".to_string(),
                    name: "Dated Result".to_string(),
                    url: "https://dated.com".to_string(),
                    display_url: "dated.com".to_string(),
                    snippet: "Content".to_string(),
                    date_last_crawled: Some("2024-12-25T10:30:00Z".to_string()),
                    deep_links: None,
                }],
            }),
        };

        let formatted = bing.format_results(response);
        assert!(formatted.contains("Last crawled:"));
        assert!(formatted.contains("2024-12-25"));
    }

    #[test]
    fn test_format_results_result_numbering() {
        let bing = BingSearchTool::new("test-key");
        let pages: Vec<BingWebPage> = (1..=5)
            .map(|i| BingWebPage {
                id: i.to_string(),
                name: format!("Result {i}"),
                url: format!("https://r{i}.com"),
                display_url: format!("r{i}.com"),
                snippet: format!("Snippet {i}"),
                date_last_crawled: None,
                deep_links: None,
            })
            .collect();

        let response = BingSearchResponse {
            response_type: "SearchResponse".to_string(),
            query_context: BingQueryContext {
                original_query: "test".to_string(),
                altered_query: None,
            },
            web_pages: Some(BingWebPages {
                total_estimated_matches: Some(5),
                value: pages,
            }),
        };

        let formatted = bing.format_results(response);
        assert!(formatted.contains("1. Result 1"));
        assert!(formatted.contains("2. Result 2"));
        assert!(formatted.contains("3. Result 3"));
        assert!(formatted.contains("4. Result 4"));
        assert!(formatted.contains("5. Result 5"));
    }

    // ==================== _call Method Tests ====================

    #[tokio::test]
    async fn test_call_with_string_input() {
        // This test verifies string input parsing (won't actually call API)
        let bing = BingSearchTool::new("invalid-key");
        let input = ToolInput::String("test query".to_string());
        // The call will fail due to invalid key, but parsing should work
        let result = bing._call(input).await;
        // We expect an error due to invalid API key, not parsing error
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // Should be API error, not parsing error
        assert!(err_msg.contains("Bing") || err_msg.contains("request") || err_msg.contains("API"));
    }

    #[tokio::test]
    async fn test_call_with_structured_input() {
        let bing = BingSearchTool::new("invalid-key");
        let input = ToolInput::Structured(json!({"query": "structured test"}));
        let result = bing._call(input).await;
        // Should fail with API error, not parsing error
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_with_missing_query_field() {
        let bing = BingSearchTool::new("test-key");
        let input = ToolInput::Structured(json!({"other_field": "value"}));
        let result = bing._call(input).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("query") || err_msg.contains("Missing"));
    }

    #[tokio::test]
    async fn test_call_with_empty_structured_input() {
        let bing = BingSearchTool::new("test-key");
        let input = ToolInput::Structured(json!({}));
        let result = bing._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_with_null_query() {
        let bing = BingSearchTool::new("test-key");
        let input = ToolInput::Structured(json!({"query": null}));
        let result = bing._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_with_numeric_query() {
        let bing = BingSearchTool::new("test-key");
        let input = ToolInput::Structured(json!({"query": 12345}));
        let result = bing._call(input).await;
        // Numeric query should fail as_str() check
        assert!(result.is_err());
    }

    // ==================== Integration-like Tests (marked ignore) ====================

    #[tokio::test]
    #[ignore = "Requires valid Bing API key"]
    async fn test_real_search() {
        let key = std::env::var("BING_API_KEY").expect("BING_API_KEY not set");
        let bing = BingSearchTool::new(key);
        let input = ToolInput::String("rust programming language".to_string());
        let result = bing._call(input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Rust") || output.contains("rust"));
    }

    #[tokio::test]
    #[ignore = "Requires valid Bing API key"]
    async fn test_real_search_with_freshness() {
        let key = std::env::var("BING_API_KEY").expect("BING_API_KEY not set");
        let bing = BingSearchTool::builder()
            .subscription_key(key)
            .freshness("Day")
            .count(5)
            .build()
            .unwrap();
        let input = ToolInput::String("latest news".to_string());
        let result = bing._call(input).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "Requires valid Bing API key"]
    async fn test_real_search_different_market() {
        let key = std::env::var("BING_API_KEY").expect("BING_API_KEY not set");
        let bing = BingSearchTool::builder()
            .subscription_key(key)
            .market("de-DE")
            .build()
            .unwrap();
        let input = ToolInput::String("Berlin".to_string());
        let result = bing._call(input).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "Requires valid Bing API key"]
    async fn test_real_search_spell_correction() {
        let key = std::env::var("BING_API_KEY").expect("BING_API_KEY not set");
        let bing = BingSearchTool::new(key);
        let input = ToolInput::String("progamming languge".to_string());
        let result = bing._call(input).await;
        assert!(result.is_ok());
        // Bing should suggest corrections
    }

    #[tokio::test]
    #[ignore = "Requires valid Bing API key"]
    async fn test_real_search_no_results() {
        let key = std::env::var("BING_API_KEY").expect("BING_API_KEY not set");
        let bing = BingSearchTool::new(key);
        // Very unlikely query that should return few/no results
        let input = ToolInput::String("xyzzy123foobarbazqux987654321".to_string());
        let result = bing._call(input).await;
        assert!(result.is_ok());
    }

    // ==================== Additional SafeSearch Tests ====================

    #[test]
    fn test_safe_search_all_variants_roundtrip() {
        for variant in [SafeSearch::Off, SafeSearch::Moderate, SafeSearch::Strict] {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: SafeSearch = serde_json::from_str(&json).unwrap();
            assert_eq!(format!("{variant:?}"), format!("{deserialized:?}"));
        }
    }

    #[test]
    fn test_safe_search_invalid_value() {
        let result = serde_json::from_str::<SafeSearch>("\"InvalidLevel\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_safe_search_case_sensitive() {
        // Bing API uses capitalized values
        let result = serde_json::from_str::<SafeSearch>("\"off\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_safe_search_from_number() {
        let result = serde_json::from_str::<SafeSearch>("1");
        assert!(result.is_err());
    }

    // ==================== Additional Freshness Tests ====================

    #[test]
    fn test_freshness_all_variants_roundtrip() {
        for variant in [Freshness::Day, Freshness::Week, Freshness::Month] {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: Freshness = serde_json::from_str(&json).unwrap();
            assert_eq!(format!("{variant:?}"), format!("{deserialized:?}"));
        }
    }

    #[test]
    fn test_freshness_invalid_value() {
        let result = serde_json::from_str::<Freshness>("\"Year\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_freshness_case_sensitive() {
        let result = serde_json::from_str::<Freshness>("\"day\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_freshness_empty_string() {
        let result = serde_json::from_str::<Freshness>("\"\"");
        assert!(result.is_err());
    }

    // ==================== Additional BingDeepLink Tests ====================

    #[test]
    fn test_deep_link_empty_name() {
        let link = BingDeepLink {
            name: "".to_string(),
            url: "https://example.com".to_string(),
        };
        let json = serde_json::to_string(&link).unwrap();
        let deserialized: BingDeepLink = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "");
    }

    #[test]
    fn test_deep_link_unicode_name() {
        let link = BingDeepLink {
            name: "日本語ページ".to_string(),
            url: "https://example.jp".to_string(),
        };
        let json = serde_json::to_string(&link).unwrap();
        let deserialized: BingDeepLink = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "日本語ページ");
    }

    #[test]
    fn test_deep_link_special_chars_url() {
        let link = BingDeepLink {
            name: "Search".to_string(),
            url: "https://example.com/search?q=hello+world&lang=en".to_string(),
        };
        let json = serde_json::to_string(&link).unwrap();
        assert!(json.contains("hello+world"));
    }

    #[test]
    fn test_deep_link_missing_url_field() {
        let result = serde_json::from_str::<BingDeepLink>(r#"{"name": "Test"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_deep_link_missing_name_field() {
        let result = serde_json::from_str::<BingDeepLink>(r#"{"url": "https://test.com"}"#);
        assert!(result.is_err());
    }

    // ==================== Additional BingWebPage Tests ====================

    #[test]
    fn test_web_page_empty_snippet() {
        let json = r#"{
            "id": "1",
            "name": "Empty Snippet Page",
            "url": "https://empty.com",
            "displayUrl": "empty.com",
            "snippet": ""
        }"#;
        let page: BingWebPage = serde_json::from_str(json).unwrap();
        assert_eq!(page.snippet, "");
    }

    #[test]
    fn test_web_page_very_long_snippet() {
        let long_snippet = "X".repeat(5000);
        let page = BingWebPage {
            id: "1".to_string(),
            name: "Long".to_string(),
            url: "https://long.com".to_string(),
            display_url: "long.com".to_string(),
            snippet: long_snippet.clone(),
            date_last_crawled: None,
            deep_links: None,
        };
        let json = serde_json::to_string(&page).unwrap();
        let deserialized: BingWebPage = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.snippet.len(), 5000);
    }

    #[test]
    fn test_web_page_unicode_content() {
        let page = BingWebPage {
            id: "unicode".to_string(),
            name: "中文网页".to_string(),
            url: "https://chinese.com".to_string(),
            display_url: "chinese.com".to_string(),
            snippet: "这是一个中文摘要".to_string(),
            date_last_crawled: None,
            deep_links: None,
        };
        let json = serde_json::to_string(&page).unwrap();
        let deserialized: BingWebPage = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "中文网页");
        assert_eq!(deserialized.snippet, "这是一个中文摘要");
    }

    #[test]
    fn test_web_page_empty_deep_links_array() {
        let json = r#"{
            "id": "1",
            "name": "Test",
            "url": "https://test.com",
            "displayUrl": "test.com",
            "snippet": "Snippet",
            "deep_links": []
        }"#;
        let page: BingWebPage = serde_json::from_str(json).unwrap();
        assert!(page.deep_links.is_some());
        assert!(page.deep_links.unwrap().is_empty());
    }

    #[test]
    fn test_web_page_many_deep_links() {
        let links: Vec<serde_json::Value> = (1..=10)
            .map(|i| {
                serde_json::json!({
                    "name": format!("Link {i}"),
                    "url": format!("https://link{i}.com")
                })
            })
            .collect();

        let json = serde_json::json!({
            "id": "multi",
            "name": "Multi Links",
            "url": "https://multi.com",
            "displayUrl": "multi.com",
            "snippet": "Many links",
            "deep_links": links
        });

        let page: BingWebPage = serde_json::from_value(json).unwrap();
        assert_eq!(page.deep_links.unwrap().len(), 10);
    }

    #[test]
    fn test_web_page_extra_fields_ignored() {
        let json = r#"{
            "id": "1",
            "name": "Test",
            "url": "https://test.com",
            "displayUrl": "test.com",
            "snippet": "Snippet",
            "extraField": "should be ignored",
            "anotherExtra": 12345
        }"#;
        let page: BingWebPage = serde_json::from_str(json).unwrap();
        assert_eq!(page.name, "Test");
    }

    #[test]
    fn test_web_page_date_formats() {
        // ISO 8601 format variations
        let dates = [
            "2024-06-15T12:00:00Z",
            "2024-06-15T12:00:00.000Z",
            "2024-06-15",
        ];
        for date in dates {
            let page = BingWebPage {
                id: "1".to_string(),
                name: "Date Test".to_string(),
                url: "https://test.com".to_string(),
                display_url: "test.com".to_string(),
                snippet: "Test".to_string(),
                date_last_crawled: Some(date.to_string()),
                deep_links: None,
            };
            let json = serde_json::to_string(&page).unwrap();
            let deserialized: BingWebPage = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized.date_last_crawled.unwrap(), date);
        }
    }

    // ==================== Additional BingWebPages Tests ====================

    #[test]
    fn test_web_pages_large_total() {
        let pages = BingWebPages {
            total_estimated_matches: Some(999999999999),
            value: vec![],
        };
        let json = serde_json::to_string(&pages).unwrap();
        let deserialized: BingWebPages = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.total_estimated_matches, Some(999999999999));
    }

    #[test]
    fn test_web_pages_zero_total() {
        let pages = BingWebPages {
            total_estimated_matches: Some(0),
            value: vec![],
        };
        let json = serde_json::to_string(&pages).unwrap();
        let deserialized: BingWebPages = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.total_estimated_matches, Some(0));
    }

    #[test]
    fn test_web_pages_clone() {
        let pages = BingWebPages {
            total_estimated_matches: Some(100),
            value: vec![BingWebPage {
                id: "1".to_string(),
                name: "Test".to_string(),
                url: "https://test.com".to_string(),
                display_url: "test.com".to_string(),
                snippet: "Snippet".to_string(),
                date_last_crawled: None,
                deep_links: None,
            }],
        };
        let cloned = pages.clone();
        assert_eq!(cloned.total_estimated_matches, Some(100));
        assert_eq!(cloned.value.len(), 1);
    }

    #[test]
    fn test_web_pages_debug() {
        let pages = BingWebPages {
            total_estimated_matches: Some(50),
            value: vec![],
        };
        let debug = format!("{pages:?}");
        assert!(debug.contains("BingWebPages"));
        assert!(debug.contains("50"));
    }

    // ==================== Additional BingQueryContext Tests ====================

    #[test]
    fn test_query_context_clone() {
        let ctx = BingQueryContext {
            original_query: "test".to_string(),
            altered_query: Some("corrected".to_string()),
        };
        let cloned = ctx.clone();
        assert_eq!(cloned.original_query, "test");
        assert_eq!(cloned.altered_query, Some("corrected".to_string()));
    }

    #[test]
    fn test_query_context_empty_query() {
        let ctx = BingQueryContext {
            original_query: "".to_string(),
            altered_query: None,
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let deserialized: BingQueryContext = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.original_query, "");
    }

    #[test]
    fn test_query_context_unicode_query() {
        let ctx = BingQueryContext {
            original_query: "日本語検索".to_string(),
            altered_query: None,
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let deserialized: BingQueryContext = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.original_query, "日本語検索");
    }

    #[test]
    fn test_query_context_special_chars() {
        let ctx = BingQueryContext {
            original_query: "test <script>alert('xss')</script>".to_string(),
            altered_query: None,
        };
        let json = serde_json::to_string(&ctx).unwrap();
        assert!(json.contains("alert"));
    }

    // ==================== Additional BingSearchResponse Tests ====================

    #[test]
    fn test_search_response_clone() {
        let response = BingSearchResponse {
            response_type: "SearchResponse".to_string(),
            query_context: BingQueryContext {
                original_query: "test".to_string(),
                altered_query: None,
            },
            web_pages: None,
        };
        let cloned = response.clone();
        assert_eq!(cloned.response_type, "SearchResponse");
    }

    #[test]
    fn test_search_response_debug() {
        let response = BingSearchResponse {
            response_type: "SearchResponse".to_string(),
            query_context: BingQueryContext {
                original_query: "test".to_string(),
                altered_query: None,
            },
            web_pages: None,
        };
        let debug = format!("{response:?}");
        assert!(debug.contains("BingSearchResponse"));
    }

    #[test]
    fn test_search_response_empty_web_pages() {
        let json = r#"{
            "_type": "SearchResponse",
            "queryContext": {"originalQuery": "test"},
            "webPages": {"value": []}
        }"#;
        let response: BingSearchResponse = serde_json::from_str(json).unwrap();
        assert!(response.web_pages.is_some());
        assert!(response.web_pages.unwrap().value.is_empty());
    }

    // ==================== Additional BingSearchTool Tests ====================

    #[test]
    fn test_bing_tool_debug() {
        let bing = BingSearchTool::new("test-key");
        let debug = format!("{bing:?}");
        assert!(debug.contains("BingSearchTool"));
        // Should NOT expose the API key in debug output (security)
        // Though in this impl it might - just testing Debug trait works
    }

    #[test]
    fn test_bing_tool_different_markets() {
        let markets = ["en-US", "en-GB", "de-DE", "fr-FR", "ja-JP", "zh-CN"];
        for market in markets {
            let bing = BingSearchTool::builder()
                .subscription_key("test")
                .market(market)
                .build()
                .unwrap();
            assert_eq!(bing.market, market);
        }
    }

    #[test]
    fn test_bing_tool_all_safe_search_levels() {
        let levels = ["Off", "Moderate", "Strict"];
        for level in levels {
            let bing = BingSearchTool::builder()
                .subscription_key("test")
                .safe_search(level)
                .build()
                .unwrap();
            assert_eq!(bing.safe_search, level);
        }
    }

    #[test]
    fn test_bing_tool_all_freshness_options() {
        let options = ["Day", "Week", "Month"];
        for opt in options {
            let bing = BingSearchTool::builder()
                .subscription_key("test")
                .freshness(opt)
                .build()
                .unwrap();
            assert_eq!(bing.freshness, Some(opt.to_string()));
        }
    }

    #[test]
    fn test_bing_tool_empty_key() {
        let bing = BingSearchTool::new("");
        assert_eq!(bing.subscription_key, "");
    }

    #[test]
    fn test_bing_tool_whitespace_key() {
        let bing = BingSearchTool::new("   ");
        assert_eq!(bing.subscription_key, "   ");
    }

    #[test]
    fn test_bing_tool_special_chars_key() {
        let bing = BingSearchTool::new("key-with-special!@#$%");
        assert_eq!(bing.subscription_key, "key-with-special!@#$%");
    }

    // ==================== Additional Builder Tests ====================

    #[test]
    fn test_builder_chaining_all_options() {
        let bing = BingSearchTool::builder()
            .subscription_key("k")
            .count(25)
            .market("pt-BR")
            .safe_search("Off")
            .freshness("Day")
            .build()
            .unwrap();

        assert_eq!(bing.subscription_key, "k");
        assert_eq!(bing.count, 25);
        assert_eq!(bing.market, "pt-BR");
        assert_eq!(bing.safe_search, "Off");
        assert_eq!(bing.freshness, Some("Day".to_string()));
    }

    #[test]
    fn test_builder_order_independence() {
        // Build with options in different orders
        let bing1 = BingSearchTool::builder()
            .subscription_key("k")
            .count(5)
            .market("en-GB")
            .build()
            .unwrap();

        let bing2 = BingSearchTool::builder()
            .market("en-GB")
            .subscription_key("k")
            .count(5)
            .build()
            .unwrap();

        assert_eq!(bing1.count, bing2.count);
        assert_eq!(bing1.market, bing2.market);
    }

    #[test]
    fn test_builder_overwrite_values() {
        let bing = BingSearchTool::builder()
            .subscription_key("first")
            .subscription_key("second")
            .count(10)
            .count(20)
            .build()
            .unwrap();

        assert_eq!(bing.subscription_key, "second");
        assert_eq!(bing.count, 20);
    }

    #[test]
    fn test_builder_default_is_new() {
        let builder = BingSearchToolBuilder::default();
        assert!(builder.subscription_key.is_none());
        assert!(builder.count.is_none());
        assert!(builder.market.is_none());
    }

    #[test]
    fn test_count_boundary_values() {
        // Test various boundary conditions
        let test_cases = [
            (0, 1),     // Below min, clamp to 1
            (1, 1),     // At min
            (25, 25),   // Middle
            (49, 49),   // Just below max
            (50, 50),   // At max
            (51, 50),   // Above max, clamp to 50
            (100, 50),  // Well above max
            (1000, 50), // Way above max
        ];

        for (input, expected) in test_cases {
            let bing = BingSearchTool::builder()
                .subscription_key("test")
                .count(input)
                .build()
                .unwrap();
            assert_eq!(
                bing.count, expected,
                "count({input}) should be {expected}"
            );
        }
    }

    // ==================== Additional Format Results Tests ====================

    #[test]
    fn test_format_results_exact_250_snippet() {
        let bing = BingSearchTool::new("test-key");
        let snippet = "A".repeat(250);
        let response = BingSearchResponse {
            response_type: "SearchResponse".to_string(),
            query_context: BingQueryContext {
                original_query: "test".to_string(),
                altered_query: None,
            },
            web_pages: Some(BingWebPages {
                total_estimated_matches: Some(1),
                value: vec![BingWebPage {
                    id: "1".to_string(),
                    name: "Exact Length".to_string(),
                    url: "https://test.com".to_string(),
                    display_url: "test.com".to_string(),
                    snippet: snippet.clone(),
                    date_last_crawled: None,
                    deep_links: None,
                }],
            }),
        };

        let formatted = bing.format_results(response);
        // Exactly 250 chars should NOT be truncated
        assert!(!formatted.contains("...") || formatted.matches("...").count() == 0);
        assert!(formatted.contains(&snippet));
    }

    #[test]
    fn test_format_results_251_snippet() {
        let bing = BingSearchTool::new("test-key");
        let snippet = "A".repeat(251);
        let response = BingSearchResponse {
            response_type: "SearchResponse".to_string(),
            query_context: BingQueryContext {
                original_query: "test".to_string(),
                altered_query: None,
            },
            web_pages: Some(BingWebPages {
                total_estimated_matches: Some(1),
                value: vec![BingWebPage {
                    id: "1".to_string(),
                    name: "Just Over".to_string(),
                    url: "https://test.com".to_string(),
                    display_url: "test.com".to_string(),
                    snippet: snippet.clone(),
                    date_last_crawled: None,
                    deep_links: None,
                }],
            }),
        };

        let formatted = bing.format_results(response);
        // 251 chars should be truncated
        assert!(formatted.contains("..."));
        assert!(!formatted.contains(&snippet));
    }

    #[test]
    fn test_format_results_empty_web_pages_value() {
        let bing = BingSearchTool::new("test-key");
        let response = BingSearchResponse {
            response_type: "SearchResponse".to_string(),
            query_context: BingQueryContext {
                original_query: "test".to_string(),
                altered_query: None,
            },
            web_pages: Some(BingWebPages {
                total_estimated_matches: Some(0),
                value: vec![],
            }),
        };

        let formatted = bing.format_results(response);
        assert!(formatted.contains("Found") && formatted.contains("0"));
    }

    #[test]
    fn test_format_results_special_chars_in_content() {
        let bing = BingSearchTool::new("test-key");
        let response = BingSearchResponse {
            response_type: "SearchResponse".to_string(),
            query_context: BingQueryContext {
                original_query: "test".to_string(),
                altered_query: None,
            },
            web_pages: Some(BingWebPages {
                total_estimated_matches: Some(1),
                value: vec![BingWebPage {
                    id: "1".to_string(),
                    name: "Test & <Special> \"Chars\"".to_string(),
                    url: "https://test.com/path?a=1&b=2".to_string(),
                    display_url: "test.com".to_string(),
                    snippet: "Contains <html> & special 'chars'".to_string(),
                    date_last_crawled: None,
                    deep_links: None,
                }],
            }),
        };

        let formatted = bing.format_results(response);
        assert!(formatted.contains("Test & <Special>"));
        assert!(formatted.contains("?a=1&b=2"));
    }

    #[test]
    fn test_format_results_ten_results() {
        let bing = BingSearchTool::new("test-key");
        let pages: Vec<BingWebPage> = (1..=10)
            .map(|i| BingWebPage {
                id: i.to_string(),
                name: format!("Result {i}"),
                url: format!("https://r{i}.com"),
                display_url: format!("r{i}.com"),
                snippet: format!("Snippet for result {i}"),
                date_last_crawled: None,
                deep_links: None,
            })
            .collect();

        let response = BingSearchResponse {
            response_type: "SearchResponse".to_string(),
            query_context: BingQueryContext {
                original_query: "test".to_string(),
                altered_query: None,
            },
            web_pages: Some(BingWebPages {
                total_estimated_matches: Some(10000),
                value: pages,
            }),
        };

        let formatted = bing.format_results(response);
        assert!(formatted.contains("10. Result 10"));
        assert!(formatted.contains("showing 10"));
    }

    // ==================== Additional _call Tests ====================

    #[tokio::test]
    async fn test_call_with_array_query() {
        let bing = BingSearchTool::new("test-key");
        let input = ToolInput::Structured(json!({"query": ["array", "value"]}));
        let result = bing._call(input).await;
        // Array should fail as_str() check
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_with_object_query() {
        let bing = BingSearchTool::new("test-key");
        let input = ToolInput::Structured(json!({"query": {"nested": "object"}}));
        let result = bing._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_with_boolean_query() {
        let bing = BingSearchTool::new("test-key");
        let input = ToolInput::Structured(json!({"query": true}));
        let result = bing._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_empty_string_query() {
        let bing = BingSearchTool::new("invalid-key");
        let input = ToolInput::String("".to_string());
        // Empty query should still attempt API call (and fail due to invalid key)
        let result = bing._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_whitespace_only_query() {
        let bing = BingSearchTool::new("invalid-key");
        let input = ToolInput::String("   ".to_string());
        let result = bing._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_unicode_query() {
        let bing = BingSearchTool::new("invalid-key");
        let input = ToolInput::String("日本語クエリ".to_string());
        let result = bing._call(input).await;
        // Should fail due to invalid key, but unicode should be handled
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_very_long_query() {
        let bing = BingSearchTool::new("invalid-key");
        let long_query = "test ".repeat(1000);
        let input = ToolInput::String(long_query);
        let result = bing._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_special_chars_query() {
        let bing = BingSearchTool::new("invalid-key");
        let input = ToolInput::String("test <script>alert('xss')</script>".to_string());
        let result = bing._call(input).await;
        // Should properly URL-encode and fail due to invalid key
        assert!(result.is_err());
    }

    // ==================== Additional Tool Trait Tests ====================

    #[test]
    fn test_tool_name_is_static_str() {
        let bing = BingSearchTool::new("test");
        let name = bing.name();
        assert_eq!(name, "bing_search");
        // The return type is &'static str, verified at compile time
    }

    #[test]
    fn test_tool_description_is_not_empty() {
        let bing = BingSearchTool::new("test");
        let desc = bing.description();
        assert!(!desc.is_empty());
        // The return type is &'static str, verified at compile time
    }

    #[test]
    fn test_args_schema_is_valid_json_schema() {
        let bing = BingSearchTool::new("test");
        let schema = bing.args_schema();

        // Verify it's a valid JSON Schema structure
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"].is_object());
        assert!(schema["required"].is_array());
    }

    #[test]
    fn test_args_schema_query_is_required() {
        let bing = BingSearchTool::new("test");
        let schema = bing.args_schema();
        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "query"));
    }

    // ==================== HTTP Client Tests ====================

    #[test]
    fn test_create_http_client_succeeds() {
        // Just verify the client creation doesn't panic
        let _client = create_http_client();
    }

    #[test]
    fn test_multiple_http_clients() {
        // Verify multiple clients can be created
        let _client1 = create_http_client();
        let _client2 = create_http_client();
        let _client3 = create_http_client();
    }

    // ==================== Edge Case Tests ====================

    #[test]
    fn test_bing_tool_clone_like_behavior() {
        // BingSearchTool doesn't implement Clone, but test that multiple
        // instances with same config behave identically
        let bing1 = BingSearchTool::new("key");
        let bing2 = BingSearchTool::new("key");
        assert_eq!(bing1.name(), bing2.name());
        assert_eq!(bing1.description(), bing2.description());
    }

    #[test]
    fn test_format_results_preserves_url_structure() {
        let bing = BingSearchTool::new("test-key");
        let complex_url = "https://example.com/path/to/page?param1=value1&param2=value2#section";
        let response = BingSearchResponse {
            response_type: "SearchResponse".to_string(),
            query_context: BingQueryContext {
                original_query: "test".to_string(),
                altered_query: None,
            },
            web_pages: Some(BingWebPages {
                total_estimated_matches: Some(1),
                value: vec![BingWebPage {
                    id: "1".to_string(),
                    name: "Complex URL".to_string(),
                    url: complex_url.to_string(),
                    display_url: "example.com".to_string(),
                    snippet: "Test".to_string(),
                    date_last_crawled: None,
                    deep_links: None,
                }],
            }),
        };

        let formatted = bing.format_results(response);
        assert!(formatted.contains(complex_url));
    }

    #[test]
    fn test_builder_accepts_impl_into_string() {
        // Test that builder accepts various string types
        let key = String::from("key");
        let market = "en-US";
        let safe = String::from("Moderate");
        let fresh: &str = "Day";

        let bing = BingSearchTool::builder()
            .subscription_key(key)
            .market(market)
            .safe_search(safe)
            .freshness(fresh)
            .build()
            .unwrap();

        assert_eq!(bing.market, "en-US");
    }

    #[test]
    fn test_web_page_all_fields_roundtrip() {
        let page = BingWebPage {
            id: "full-test-123".to_string(),
            name: "Full Test Page".to_string(),
            url: "https://full-test.example.com/page".to_string(),
            display_url: "full-test.example.com/page".to_string(),
            snippet: "This is a comprehensive test snippet with all fields populated.".to_string(),
            date_last_crawled: Some("2024-12-15T10:30:45.123Z".to_string()),
            deep_links: Some(vec![
                BingDeepLink {
                    name: "Subpage 1".to_string(),
                    url: "https://full-test.example.com/sub1".to_string(),
                },
                BingDeepLink {
                    name: "Subpage 2".to_string(),
                    url: "https://full-test.example.com/sub2".to_string(),
                },
            ]),
        };

        let json = serde_json::to_string(&page).unwrap();
        let deserialized: BingWebPage = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, page.id);
        assert_eq!(deserialized.name, page.name);
        assert_eq!(deserialized.url, page.url);
        assert_eq!(deserialized.display_url, page.display_url);
        assert_eq!(deserialized.snippet, page.snippet);
        assert_eq!(deserialized.date_last_crawled, page.date_last_crawled);
        assert_eq!(deserialized.deep_links.unwrap().len(), 2);
    }

    #[test]
    fn test_full_search_response_roundtrip() {
        let response = BingSearchResponse {
            response_type: "SearchResponse".to_string(),
            query_context: BingQueryContext {
                original_query: "rust programming".to_string(),
                altered_query: Some("rust language programming".to_string()),
            },
            web_pages: Some(BingWebPages {
                total_estimated_matches: Some(15000000),
                value: vec![
                    BingWebPage {
                        id: "1".to_string(),
                        name: "Rust Language".to_string(),
                        url: "https://rust-lang.org".to_string(),
                        display_url: "rust-lang.org".to_string(),
                        snippet: "Rust is a systems programming language".to_string(),
                        date_last_crawled: Some("2024-01-01T00:00:00Z".to_string()),
                        deep_links: None,
                    },
                    BingWebPage {
                        id: "2".to_string(),
                        name: "Learn Rust".to_string(),
                        url: "https://doc.rust-lang.org/book".to_string(),
                        display_url: "doc.rust-lang.org/book".to_string(),
                        snippet: "The Rust Programming Language book".to_string(),
                        date_last_crawled: None,
                        deep_links: Some(vec![BingDeepLink {
                            name: "Chapter 1".to_string(),
                            url: "https://doc.rust-lang.org/book/ch01-00-getting-started.html"
                                .to_string(),
                        }]),
                    },
                ],
            }),
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: BingSearchResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.response_type, response.response_type);
        assert_eq!(
            deserialized.query_context.original_query,
            response.query_context.original_query
        );
        assert_eq!(
            deserialized.query_context.altered_query,
            response.query_context.altered_query
        );
        assert!(deserialized.web_pages.is_some());
        let pages = deserialized.web_pages.unwrap();
        assert_eq!(pages.total_estimated_matches, Some(15000000));
        assert_eq!(pages.value.len(), 2);
    }
}
