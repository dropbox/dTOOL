//! # Brave Search Tool
//!
//! Brave Search is a privacy-focused web search API built on Brave's independent web index.
//! It provides high-quality search results without tracking users, with reduced SEO spam.
//!
//! ## Features
//!
//! - Privacy-focused web search with independent index
//! - No user tracking or data collection
//! - Fresh and recent content
//! - Multiple search types: web, news, images, videos
//! - Reduced SEO spam
//!
//! ## Usage
//!
//! ```no_run
//! use dashflow_brave::BraveSearchTool;
//! use dashflow::core::tools::Tool;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let brave = BraveSearchTool::new("YOUR_API_KEY");
//!
//! // Simple search
//! let results = brave._call_str("Who is Leo Messi?".to_string()).await?;
//! println!("Search results: {}", results);
//! # Ok(())
//! # }
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

/// Search type for Brave API
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SearchType {
    /// Web search (default)
    #[default]
    Web,
    /// News search
    News,
    /// Image search
    Images,
    /// Video search
    Videos,
}

/// Freshness filter for search results
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Freshness {
    /// Past day
    #[serde(rename = "pd")]
    PastDay,
    /// Past week
    #[serde(rename = "pw")]
    PastWeek,
    /// Past month
    #[serde(rename = "pm")]
    PastMonth,
    /// Past year
    #[serde(rename = "py")]
    PastYear,
}

/// A single web search result from Brave
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BraveWebResult {
    /// Page title
    pub title: String,
    /// Page URL
    pub url: String,
    /// Content description/snippet
    pub description: String,
    /// Age of content (e.g., "2 days ago")
    #[serde(default)]
    pub age: Option<String>,
    /// Extra snippets
    #[serde(default)]
    pub extra_snippets: Option<Vec<String>>,
}

/// Query metadata from Brave
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BraveQuery {
    /// Original query
    pub original: String,
    /// Altered query (if spell-corrected)
    #[serde(default)]
    pub altered: Option<String>,
}

/// Mixed response type containing web results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BraveMixed {
    /// Type of result (should be "search")
    #[serde(rename = "type")]
    pub result_type: String,
    /// Web search results
    #[serde(default)]
    pub results: Option<Vec<BraveWebResult>>,
}

/// Response from Brave search API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BraveSearchResponse {
    /// Query information
    pub query: BraveQuery,
    /// Mixed results (web, news, etc.)
    #[serde(default)]
    pub mixed: Option<BraveMixed>,
    /// Web results (alternative location)
    #[serde(default)]
    pub web: Option<WebResults>,
}

/// Web results container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebResults {
    /// Search results
    pub results: Vec<BraveWebResult>,
}

/// Brave search tool for `DashFlow` agents
///
/// This tool provides access to Brave's privacy-focused search API, which uses
/// Brave's independent web index to return high-quality search results without
/// user tracking.
///
/// # Example
///
/// ```no_run
/// use dashflow_brave::BraveSearchTool;
/// use dashflow::core::tools::Tool;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let brave = BraveSearchTool::builder()
///     .api_key("YOUR_API_KEY")
///     .count(5)
///     .build()?;
///
/// let results = brave._call_str("latest AI research".to_string())
///     .await?;
/// println!("Found: {}", results);
/// # Ok(())
/// # }
/// ```
pub struct BraveSearchTool {
    api_key: String,
    count: u32,
    search_lang: String,
    country: String,
    safesearch: String,
    freshness: Option<String>,
    client: reqwest::Client,
}

impl BraveSearchTool {
    /// Create a new Brave search tool
    ///
    /// # Arguments
    ///
    /// * `api_key` - Your Brave Search API key (X-Subscription-Token)
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_brave::BraveSearchTool;
    ///
    /// let brave = BraveSearchTool::new("YOUR_API_KEY");
    /// ```
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            count: 10,
            search_lang: "en".to_string(),
            country: "US".to_string(),
            safesearch: "moderate".to_string(),
            freshness: None,
            client: create_http_client(),
        }
    }

    /// Create a builder for `BraveSearchTool`
    #[must_use]
    pub fn builder() -> BraveSearchToolBuilder {
        BraveSearchToolBuilder::default()
    }

    /// Perform a search using the Brave API
    async fn search(&self, query: String) -> Result<BraveSearchResponse> {
        let mut url = format!(
            "https://api.search.brave.com/res/v1/web/search?q={}&count={}",
            urlencoding::encode(&query),
            self.count
        );

        // Add optional parameters
        url.push_str(&format!("&search_lang={}", self.search_lang));
        url.push_str(&format!("&country={}", self.country));
        url.push_str(&format!("&safesearch={}", self.safesearch));

        if let Some(ref freshness) = self.freshness {
            url.push_str(&format!("&freshness={freshness}"));
        }

        let response = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .header("X-Subscription-Token", &self.api_key)
            .send()
            .await
            .map_err(|e| {
                dashflow::core::Error::tool_error(format!("Brave API request failed: {e}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(dashflow::core::Error::tool_error(format!(
                "Brave API error ({status}): {error_text}"
            )));
        }

        // M-216: Use size-limited JSON parsing to prevent memory exhaustion
        let search_response: BraveSearchResponse =
            json_with_limit(response, SEARCH_RESPONSE_SIZE_LIMIT).await.map_err(|e| {
                dashflow::core::Error::tool_error(format!("Failed to parse Brave response: {e}"))
            })?;

        Ok(search_response)
    }

    /// Format search results as a string
    fn format_results(&self, response: BraveSearchResponse) -> String {
        let mut output = String::new();

        // Show spell correction if present
        if let Some(ref altered) = response.query.altered {
            output.push_str(&format!("Showing results for: {altered}\n\n"));
        }

        // Extract results from either mixed or web
        let results = if let Some(mixed) = response.mixed {
            mixed.results.unwrap_or_default()
        } else if let Some(web) = response.web {
            web.results
        } else {
            vec![]
        };

        if results.is_empty() {
            output.push_str("No results found.");
            return output;
        }

        output.push_str(&format!("Found {} results:\n\n", results.len()));

        for (i, result) in results.iter().enumerate() {
            output.push_str(&format!("{}. {}\n", i + 1, result.title));
            output.push_str(&format!("   URL: {}\n", result.url));

            // Truncate description to 250 chars
            let snippet = if result.description.len() > 250 {
                format!("{}...", &result.description[..250])
            } else {
                result.description.clone()
            };
            output.push_str(&format!("   Description: {snippet}\n"));

            if let Some(ref age) = result.age {
                output.push_str(&format!("   Age: {age}\n"));
            }

            output.push('\n');
        }

        output
    }
}

#[async_trait]
impl Tool for BraveSearchTool {
    fn name(&self) -> &'static str {
        "brave_search"
    }

    fn description(&self) -> &'static str {
        "Search the web using Brave's privacy-focused search API. \
         Returns high-quality search results from Brave's independent web index. \
         No user tracking, reduced SEO spam, and fresh content. \
         Best for privacy-conscious applications and general web search."
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

/// Builder for `BraveSearchTool`
#[derive(Default)]
pub struct BraveSearchToolBuilder {
    api_key: Option<String>,
    count: Option<u32>,
    search_lang: Option<String>,
    country: Option<String>,
    safesearch: Option<String>,
    freshness: Option<String>,
}

impl BraveSearchToolBuilder {
    /// Set the API key
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set the maximum number of results (1-20)
    #[must_use]
    pub fn count(mut self, count: u32) -> Self {
        self.count = Some(count.clamp(1, 20));
        self
    }

    /// Set the search language (default: "en")
    pub fn search_lang(mut self, lang: impl Into<String>) -> Self {
        self.search_lang = Some(lang.into());
        self
    }

    /// Set the country code (default: "US")
    pub fn country(mut self, country: impl Into<String>) -> Self {
        self.country = Some(country.into());
        self
    }

    /// Set the safe search level ("off", "moderate", "strict")
    pub fn safesearch(mut self, level: impl Into<String>) -> Self {
        self.safesearch = Some(level.into());
        self
    }

    /// Set freshness filter ("pd" = past day, "pw" = past week, "pm" = past month, "py" = past year)
    pub fn freshness(mut self, freshness: impl Into<String>) -> Self {
        self.freshness = Some(freshness.into());
        self
    }

    /// Build the `BraveSearchTool`
    pub fn build(self) -> Result<BraveSearchTool> {
        let api_key = self.api_key.ok_or_else(|| {
            dashflow::core::Error::tool_error("API key is required for Brave search".to_string())
        })?;

        Ok(BraveSearchTool {
            api_key,
            count: self.count.unwrap_or(10),
            search_lang: self.search_lang.unwrap_or_else(|| "en".to_string()),
            country: self.country.unwrap_or_else(|| "US".to_string()),
            safesearch: self.safesearch.unwrap_or_else(|| "moderate".to_string()),
            freshness: self.freshness,
            client: create_http_client(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =============================================================================
    // SearchType enum tests
    // =============================================================================

    #[test]
    fn test_search_type_default() {
        let default = SearchType::default();
        assert!(matches!(default, SearchType::Web));
    }

    #[test]
    fn test_search_type_web_serialize() {
        let search_type = SearchType::Web;
        let json = serde_json::to_string(&search_type).unwrap();
        assert_eq!(json, "\"web\"");
    }

    #[test]
    fn test_search_type_news_serialize() {
        let search_type = SearchType::News;
        let json = serde_json::to_string(&search_type).unwrap();
        assert_eq!(json, "\"news\"");
    }

    #[test]
    fn test_search_type_images_serialize() {
        let search_type = SearchType::Images;
        let json = serde_json::to_string(&search_type).unwrap();
        assert_eq!(json, "\"images\"");
    }

    #[test]
    fn test_search_type_videos_serialize() {
        let search_type = SearchType::Videos;
        let json = serde_json::to_string(&search_type).unwrap();
        assert_eq!(json, "\"videos\"");
    }

    #[test]
    fn test_search_type_deserialize_web() {
        let search_type: SearchType = serde_json::from_str("\"web\"").unwrap();
        assert!(matches!(search_type, SearchType::Web));
    }

    #[test]
    fn test_search_type_deserialize_news() {
        let search_type: SearchType = serde_json::from_str("\"news\"").unwrap();
        assert!(matches!(search_type, SearchType::News));
    }

    #[test]
    fn test_search_type_deserialize_images() {
        let search_type: SearchType = serde_json::from_str("\"images\"").unwrap();
        assert!(matches!(search_type, SearchType::Images));
    }

    #[test]
    fn test_search_type_deserialize_videos() {
        let search_type: SearchType = serde_json::from_str("\"videos\"").unwrap();
        assert!(matches!(search_type, SearchType::Videos));
    }

    #[test]
    fn test_search_type_clone() {
        let original = SearchType::News;
        let cloned = original.clone();
        assert!(matches!(cloned, SearchType::News));
    }

    #[test]
    fn test_search_type_debug() {
        let search_type = SearchType::Web;
        let debug_str = format!("{:?}", search_type);
        assert_eq!(debug_str, "Web");
    }

    // =============================================================================
    // Freshness enum tests
    // =============================================================================

    #[test]
    fn test_freshness_past_day_serialize() {
        let freshness = Freshness::PastDay;
        let json = serde_json::to_string(&freshness).unwrap();
        assert_eq!(json, "\"pd\"");
    }

    #[test]
    fn test_freshness_past_week_serialize() {
        let freshness = Freshness::PastWeek;
        let json = serde_json::to_string(&freshness).unwrap();
        assert_eq!(json, "\"pw\"");
    }

    #[test]
    fn test_freshness_past_month_serialize() {
        let freshness = Freshness::PastMonth;
        let json = serde_json::to_string(&freshness).unwrap();
        assert_eq!(json, "\"pm\"");
    }

    #[test]
    fn test_freshness_past_year_serialize() {
        let freshness = Freshness::PastYear;
        let json = serde_json::to_string(&freshness).unwrap();
        assert_eq!(json, "\"py\"");
    }

    #[test]
    fn test_freshness_deserialize_pd() {
        let freshness: Freshness = serde_json::from_str("\"pd\"").unwrap();
        assert!(matches!(freshness, Freshness::PastDay));
    }

    #[test]
    fn test_freshness_deserialize_pw() {
        let freshness: Freshness = serde_json::from_str("\"pw\"").unwrap();
        assert!(matches!(freshness, Freshness::PastWeek));
    }

    #[test]
    fn test_freshness_deserialize_pm() {
        let freshness: Freshness = serde_json::from_str("\"pm\"").unwrap();
        assert!(matches!(freshness, Freshness::PastMonth));
    }

    #[test]
    fn test_freshness_deserialize_py() {
        let freshness: Freshness = serde_json::from_str("\"py\"").unwrap();
        assert!(matches!(freshness, Freshness::PastYear));
    }

    #[test]
    fn test_freshness_clone() {
        let original = Freshness::PastWeek;
        let cloned = original.clone();
        assert!(matches!(cloned, Freshness::PastWeek));
    }

    #[test]
    fn test_freshness_debug() {
        let freshness = Freshness::PastMonth;
        let debug_str = format!("{:?}", freshness);
        assert_eq!(debug_str, "PastMonth");
    }

    // =============================================================================
    // BraveWebResult tests
    // =============================================================================

    #[test]
    fn test_brave_web_result_serialize() {
        let result = BraveWebResult {
            title: "Test Title".to_string(),
            url: "https://example.com".to_string(),
            description: "Test description".to_string(),
            age: Some("1 day ago".to_string()),
            extra_snippets: Some(vec!["snippet1".to_string(), "snippet2".to_string()]),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"title\":\"Test Title\""));
        assert!(json.contains("\"url\":\"https://example.com\""));
        assert!(json.contains("\"description\":\"Test description\""));
        assert!(json.contains("\"age\":\"1 day ago\""));
        assert!(json.contains("\"extra_snippets\""));
    }

    #[test]
    fn test_brave_web_result_deserialize_full() {
        let json = r#"{
            "title": "Result Title",
            "url": "https://test.com",
            "description": "A test result",
            "age": "3 hours ago",
            "extra_snippets": ["extra1", "extra2"]
        }"#;
        let result: BraveWebResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.title, "Result Title");
        assert_eq!(result.url, "https://test.com");
        assert_eq!(result.description, "A test result");
        assert_eq!(result.age, Some("3 hours ago".to_string()));
        assert_eq!(result.extra_snippets, Some(vec!["extra1".to_string(), "extra2".to_string()]));
    }

    #[test]
    fn test_brave_web_result_deserialize_minimal() {
        let json = r#"{
            "title": "Title",
            "url": "https://url.com",
            "description": "Desc"
        }"#;
        let result: BraveWebResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.title, "Title");
        assert_eq!(result.url, "https://url.com");
        assert_eq!(result.description, "Desc");
        assert_eq!(result.age, None);
        assert_eq!(result.extra_snippets, None);
    }

    #[test]
    fn test_brave_web_result_clone() {
        let original = BraveWebResult {
            title: "Title".to_string(),
            url: "https://url.com".to_string(),
            description: "Desc".to_string(),
            age: Some("1 hour ago".to_string()),
            extra_snippets: None,
        };
        let cloned = original.clone();
        assert_eq!(cloned.title, "Title");
        assert_eq!(cloned.age, Some("1 hour ago".to_string()));
    }

    #[test]
    fn test_brave_web_result_debug() {
        let result = BraveWebResult {
            title: "Debug Test".to_string(),
            url: "https://debug.com".to_string(),
            description: "Testing debug".to_string(),
            age: None,
            extra_snippets: None,
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("BraveWebResult"));
        assert!(debug_str.contains("Debug Test"));
    }

    // =============================================================================
    // BraveQuery tests
    // =============================================================================

    #[test]
    fn test_brave_query_serialize() {
        let query = BraveQuery {
            original: "rust programming".to_string(),
            altered: Some("rust language".to_string()),
        };
        let json = serde_json::to_string(&query).unwrap();
        assert!(json.contains("\"original\":\"rust programming\""));
        assert!(json.contains("\"altered\":\"rust language\""));
    }

    #[test]
    fn test_brave_query_deserialize_with_altered() {
        let json = r#"{"original": "tset query", "altered": "test query"}"#;
        let query: BraveQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.original, "tset query");
        assert_eq!(query.altered, Some("test query".to_string()));
    }

    #[test]
    fn test_brave_query_deserialize_without_altered() {
        let json = r#"{"original": "test query"}"#;
        let query: BraveQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.original, "test query");
        assert_eq!(query.altered, None);
    }

    #[test]
    fn test_brave_query_clone() {
        let original = BraveQuery {
            original: "query".to_string(),
            altered: None,
        };
        let cloned = original.clone();
        assert_eq!(cloned.original, "query");
    }

    #[test]
    fn test_brave_query_debug() {
        let query = BraveQuery {
            original: "debug query".to_string(),
            altered: None,
        };
        let debug_str = format!("{:?}", query);
        assert!(debug_str.contains("BraveQuery"));
        assert!(debug_str.contains("debug query"));
    }

    // =============================================================================
    // BraveMixed tests
    // =============================================================================

    #[test]
    fn test_brave_mixed_serialize() {
        let mixed = BraveMixed {
            result_type: "search".to_string(),
            results: Some(vec![BraveWebResult {
                title: "Mixed Result".to_string(),
                url: "https://mixed.com".to_string(),
                description: "Mixed description".to_string(),
                age: None,
                extra_snippets: None,
            }]),
        };
        let json = serde_json::to_string(&mixed).unwrap();
        assert!(json.contains("\"type\":\"search\""));
        assert!(json.contains("\"results\""));
        assert!(json.contains("Mixed Result"));
    }

    #[test]
    fn test_brave_mixed_deserialize() {
        let json = r#"{
            "type": "search",
            "results": [{
                "title": "Test",
                "url": "https://test.com",
                "description": "Test desc"
            }]
        }"#;
        let mixed: BraveMixed = serde_json::from_str(json).unwrap();
        assert_eq!(mixed.result_type, "search");
        assert!(mixed.results.is_some());
        assert_eq!(mixed.results.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_brave_mixed_deserialize_no_results() {
        let json = r#"{"type": "search"}"#;
        let mixed: BraveMixed = serde_json::from_str(json).unwrap();
        assert_eq!(mixed.result_type, "search");
        assert_eq!(mixed.results, None);
    }

    #[test]
    fn test_brave_mixed_clone() {
        let original = BraveMixed {
            result_type: "search".to_string(),
            results: None,
        };
        let cloned = original.clone();
        assert_eq!(cloned.result_type, "search");
    }

    #[test]
    fn test_brave_mixed_debug() {
        let mixed = BraveMixed {
            result_type: "search".to_string(),
            results: None,
        };
        let debug_str = format!("{:?}", mixed);
        assert!(debug_str.contains("BraveMixed"));
    }

    // =============================================================================
    // BraveSearchResponse tests
    // =============================================================================

    #[test]
    fn test_brave_search_response_serialize() {
        let response = BraveSearchResponse {
            query: BraveQuery {
                original: "test".to_string(),
                altered: None,
            },
            mixed: None,
            web: None,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"query\""));
        assert!(json.contains("\"original\":\"test\""));
    }

    #[test]
    fn test_brave_search_response_deserialize_with_web() {
        let json = r#"{
            "query": {"original": "search term"},
            "web": {
                "results": [{
                    "title": "Web Result",
                    "url": "https://web.com",
                    "description": "Web desc"
                }]
            }
        }"#;
        let response: BraveSearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.query.original, "search term");
        assert!(response.web.is_some());
        assert!(response.mixed.is_none());
    }

    #[test]
    fn test_brave_search_response_deserialize_with_mixed() {
        let json = r#"{
            "query": {"original": "mixed search"},
            "mixed": {
                "type": "search",
                "results": [{
                    "title": "Mixed Result",
                    "url": "https://mixed.com",
                    "description": "Mixed desc"
                }]
            }
        }"#;
        let response: BraveSearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.query.original, "mixed search");
        assert!(response.mixed.is_some());
        assert!(response.web.is_none());
    }

    #[test]
    fn test_brave_search_response_clone() {
        let original = BraveSearchResponse {
            query: BraveQuery {
                original: "clone test".to_string(),
                altered: None,
            },
            mixed: None,
            web: None,
        };
        let cloned = original.clone();
        assert_eq!(cloned.query.original, "clone test");
    }

    #[test]
    fn test_brave_search_response_debug() {
        let response = BraveSearchResponse {
            query: BraveQuery {
                original: "debug".to_string(),
                altered: None,
            },
            mixed: None,
            web: None,
        };
        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("BraveSearchResponse"));
    }

    // =============================================================================
    // WebResults tests
    // =============================================================================

    #[test]
    fn test_web_results_serialize() {
        let web_results = WebResults {
            results: vec![
                BraveWebResult {
                    title: "First".to_string(),
                    url: "https://first.com".to_string(),
                    description: "First desc".to_string(),
                    age: None,
                    extra_snippets: None,
                },
                BraveWebResult {
                    title: "Second".to_string(),
                    url: "https://second.com".to_string(),
                    description: "Second desc".to_string(),
                    age: None,
                    extra_snippets: None,
                },
            ],
        };
        let json = serde_json::to_string(&web_results).unwrap();
        assert!(json.contains("First"));
        assert!(json.contains("Second"));
    }

    #[test]
    fn test_web_results_deserialize() {
        let json = r#"{
            "results": [
                {"title": "R1", "url": "https://r1.com", "description": "D1"},
                {"title": "R2", "url": "https://r2.com", "description": "D2"}
            ]
        }"#;
        let web_results: WebResults = serde_json::from_str(json).unwrap();
        assert_eq!(web_results.results.len(), 2);
        assert_eq!(web_results.results[0].title, "R1");
        assert_eq!(web_results.results[1].title, "R2");
    }

    #[test]
    fn test_web_results_clone() {
        let original = WebResults {
            results: vec![BraveWebResult {
                title: "Clone".to_string(),
                url: "https://clone.com".to_string(),
                description: "Clone desc".to_string(),
                age: None,
                extra_snippets: None,
            }],
        };
        let cloned = original.clone();
        assert_eq!(cloned.results.len(), 1);
    }

    #[test]
    fn test_web_results_debug() {
        let web_results = WebResults { results: vec![] };
        let debug_str = format!("{:?}", web_results);
        assert!(debug_str.contains("WebResults"));
    }

    // =============================================================================
    // BraveSearchTool basic tests
    // =============================================================================

    #[test]
    fn test_brave_tool_creation() {
        let brave = BraveSearchTool::new("test-key");
        assert_eq!(brave.name(), "brave_search");
        assert!(brave.description().contains("Brave"));
        assert!(brave.description().contains("privacy"));
    }

    #[test]
    fn test_brave_tool_new_with_string() {
        let brave = BraveSearchTool::new(String::from("string-key"));
        assert_eq!(brave.api_key, "string-key");
    }

    #[test]
    fn test_brave_tool_new_with_str() {
        let brave = BraveSearchTool::new("str-key");
        assert_eq!(brave.api_key, "str-key");
    }

    #[test]
    fn test_brave_tool_default_count() {
        let brave = BraveSearchTool::new("key");
        assert_eq!(brave.count, 10);
    }

    #[test]
    fn test_brave_tool_default_search_lang() {
        let brave = BraveSearchTool::new("key");
        assert_eq!(brave.search_lang, "en");
    }

    #[test]
    fn test_brave_tool_default_country() {
        let brave = BraveSearchTool::new("key");
        assert_eq!(brave.country, "US");
    }

    #[test]
    fn test_brave_tool_default_safesearch() {
        let brave = BraveSearchTool::new("key");
        assert_eq!(brave.safesearch, "moderate");
    }

    #[test]
    fn test_brave_tool_default_freshness() {
        let brave = BraveSearchTool::new("key");
        assert_eq!(brave.freshness, None);
    }

    // =============================================================================
    // BraveSearchToolBuilder tests
    // =============================================================================

    #[test]
    fn test_brave_tool_builder() {
        let brave = BraveSearchTool::builder()
            .api_key("test-key")
            .count(5)
            .search_lang("es")
            .country("ES")
            .safesearch("strict")
            .freshness("pw")
            .build()
            .unwrap();

        assert_eq!(brave.api_key, "test-key");
        assert_eq!(brave.count, 5);
        assert_eq!(brave.search_lang, "es");
        assert_eq!(brave.country, "ES");
        assert_eq!(brave.safesearch, "strict");
        assert_eq!(brave.freshness, Some("pw".to_string()));
    }

    #[test]
    fn test_brave_builder_missing_api_key() {
        let result = BraveSearchTool::builder().count(5).build();
        assert!(result.is_err());
    }

    #[test]
    fn test_brave_builder_api_key_only() {
        let brave = BraveSearchTool::builder()
            .api_key("minimal")
            .build()
            .unwrap();
        assert_eq!(brave.api_key, "minimal");
        assert_eq!(brave.count, 10);
        assert_eq!(brave.search_lang, "en");
        assert_eq!(brave.country, "US");
        assert_eq!(brave.safesearch, "moderate");
        assert_eq!(brave.freshness, None);
    }

    #[test]
    fn test_brave_builder_count_zero_clamps_to_one() {
        let brave = BraveSearchTool::builder()
            .api_key("test")
            .count(0)
            .build()
            .unwrap();
        assert_eq!(brave.count, 1);
    }

    #[test]
    fn test_count_clamping() {
        let brave = BraveSearchTool::builder()
            .api_key("test")
            .count(100) // Should be clamped to 20
            .build()
            .unwrap();

        assert_eq!(brave.count, 20);
    }

    #[test]
    fn test_brave_builder_count_at_max() {
        let brave = BraveSearchTool::builder()
            .api_key("test")
            .count(20)
            .build()
            .unwrap();
        assert_eq!(brave.count, 20);
    }

    #[test]
    fn test_brave_builder_count_at_min() {
        let brave = BraveSearchTool::builder()
            .api_key("test")
            .count(1)
            .build()
            .unwrap();
        assert_eq!(brave.count, 1);
    }

    #[test]
    fn test_brave_builder_api_key_string() {
        let brave = BraveSearchTool::builder()
            .api_key(String::from("owned-string"))
            .build()
            .unwrap();
        assert_eq!(brave.api_key, "owned-string");
    }

    #[test]
    fn test_brave_builder_search_lang_string() {
        let brave = BraveSearchTool::builder()
            .api_key("key")
            .search_lang(String::from("fr"))
            .build()
            .unwrap();
        assert_eq!(brave.search_lang, "fr");
    }

    #[test]
    fn test_brave_builder_country_string() {
        let brave = BraveSearchTool::builder()
            .api_key("key")
            .country(String::from("DE"))
            .build()
            .unwrap();
        assert_eq!(brave.country, "DE");
    }

    #[test]
    fn test_brave_builder_safesearch_off() {
        let brave = BraveSearchTool::builder()
            .api_key("key")
            .safesearch("off")
            .build()
            .unwrap();
        assert_eq!(brave.safesearch, "off");
    }

    #[test]
    fn test_brave_builder_freshness_pd() {
        let brave = BraveSearchTool::builder()
            .api_key("key")
            .freshness("pd")
            .build()
            .unwrap();
        assert_eq!(brave.freshness, Some("pd".to_string()));
    }

    #[test]
    fn test_brave_builder_freshness_pm() {
        let brave = BraveSearchTool::builder()
            .api_key("key")
            .freshness("pm")
            .build()
            .unwrap();
        assert_eq!(brave.freshness, Some("pm".to_string()));
    }

    #[test]
    fn test_brave_builder_freshness_py() {
        let brave = BraveSearchTool::builder()
            .api_key("key")
            .freshness("py")
            .build()
            .unwrap();
        assert_eq!(brave.freshness, Some("py".to_string()));
    }

    #[test]
    fn test_brave_builder_default() {
        let builder = BraveSearchToolBuilder::default();
        let result = builder.build();
        assert!(result.is_err()); // API key required
    }

    // =============================================================================
    // Tool trait tests
    // =============================================================================

    #[test]
    fn test_brave_args_schema() {
        let brave = BraveSearchTool::new("test-key");
        let schema = brave.args_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
        assert_eq!(schema["required"][0], "query");
    }

    #[test]
    fn test_brave_args_schema_query_type() {
        let brave = BraveSearchTool::new("test-key");
        let schema = brave.args_schema();
        assert_eq!(schema["properties"]["query"]["type"], "string");
    }

    #[test]
    fn test_brave_args_schema_query_description() {
        let brave = BraveSearchTool::new("test-key");
        let schema = brave.args_schema();
        assert!(schema["properties"]["query"]["description"].is_string());
    }

    #[test]
    fn test_brave_name() {
        let brave = BraveSearchTool::new("key");
        assert_eq!(brave.name(), "brave_search");
    }

    #[test]
    fn test_brave_description_content() {
        let brave = BraveSearchTool::new("key");
        let desc = brave.description();
        assert!(desc.contains("Search"));
        assert!(desc.contains("web"));
        assert!(desc.contains("privacy"));
        assert!(desc.contains("independent"));
        assert!(desc.contains("index"));
    }

    #[tokio::test]
    async fn test_call_with_string_input() {
        let brave = BraveSearchTool::new("test-key");
        let input = ToolInput::String("test query".to_string());
        // This will fail because we don't have a real API key, but we can test the parsing
        let result = brave._call(input).await;
        assert!(result.is_err()); // Expected to fail without real API
    }

    #[tokio::test]
    async fn test_call_with_structured_input() {
        let brave = BraveSearchTool::new("test-key");
        let input = ToolInput::Structured(json!({"query": "structured test"}));
        let result = brave._call(input).await;
        assert!(result.is_err()); // Expected to fail without real API
    }

    #[tokio::test]
    async fn test_call_with_missing_query() {
        let brave = BraveSearchTool::new("test-key");
        let input = ToolInput::Structured(json!({"not_query": "test"}));
        let result = brave._call(input).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("query"));
    }

    #[tokio::test]
    async fn test_call_with_empty_structured_input() {
        let brave = BraveSearchTool::new("test-key");
        let input = ToolInput::Structured(json!({}));
        let result = brave._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_with_null_query() {
        let brave = BraveSearchTool::new("test-key");
        let input = ToolInput::Structured(json!({"query": null}));
        let result = brave._call(input).await;
        assert!(result.is_err());
    }

    // =============================================================================
    // format_results tests
    // =============================================================================

    #[tokio::test]
    async fn test_format_results() {
        let brave = BraveSearchTool::new("test-key");
        let response = BraveSearchResponse {
            query: BraveQuery {
                original: "test query".to_string(),
                altered: None,
            },
            mixed: None,
            web: Some(WebResults {
                results: vec![BraveWebResult {
                    title: "Test Result".to_string(),
                    url: "https://example.com".to_string(),
                    description: "This is test content".to_string(),
                    age: Some("2 days ago".to_string()),
                    extra_snippets: None,
                }],
            }),
        };

        let formatted = brave.format_results(response);
        assert!(formatted.contains("Test Result"));
        assert!(formatted.contains("https://example.com"));
        assert!(formatted.contains("This is test content"));
        assert!(formatted.contains("2 days ago"));
    }

    #[tokio::test]
    async fn test_format_empty_results() {
        let brave = BraveSearchTool::new("test-key");
        let response = BraveSearchResponse {
            query: BraveQuery {
                original: "test query".to_string(),
                altered: None,
            },
            mixed: None,
            web: None,
        };

        let formatted = brave.format_results(response);
        assert_eq!(formatted, "No results found.");
    }

    #[tokio::test]
    async fn test_spell_correction() {
        let brave = BraveSearchTool::new("test-key");
        let response = BraveSearchResponse {
            query: BraveQuery {
                original: "test qurey".to_string(),
                altered: Some("test query".to_string()),
            },
            mixed: None,
            web: Some(WebResults { results: vec![] }),
        };

        let formatted = brave.format_results(response);
        assert!(formatted.contains("Showing results for: test query"));
    }

    #[test]
    fn test_format_results_multiple() {
        let brave = BraveSearchTool::new("test-key");
        let response = BraveSearchResponse {
            query: BraveQuery {
                original: "multi".to_string(),
                altered: None,
            },
            mixed: None,
            web: Some(WebResults {
                results: vec![
                    BraveWebResult {
                        title: "Result 1".to_string(),
                        url: "https://r1.com".to_string(),
                        description: "First result".to_string(),
                        age: None,
                        extra_snippets: None,
                    },
                    BraveWebResult {
                        title: "Result 2".to_string(),
                        url: "https://r2.com".to_string(),
                        description: "Second result".to_string(),
                        age: None,
                        extra_snippets: None,
                    },
                    BraveWebResult {
                        title: "Result 3".to_string(),
                        url: "https://r3.com".to_string(),
                        description: "Third result".to_string(),
                        age: None,
                        extra_snippets: None,
                    },
                ],
            }),
        };
        let formatted = brave.format_results(response);
        assert!(formatted.contains("Found 3 results"));
        assert!(formatted.contains("1. Result 1"));
        assert!(formatted.contains("2. Result 2"));
        assert!(formatted.contains("3. Result 3"));
    }

    #[test]
    fn test_format_results_with_mixed() {
        let brave = BraveSearchTool::new("test-key");
        let response = BraveSearchResponse {
            query: BraveQuery {
                original: "mixed test".to_string(),
                altered: None,
            },
            mixed: Some(BraveMixed {
                result_type: "search".to_string(),
                results: Some(vec![BraveWebResult {
                    title: "Mixed Result".to_string(),
                    url: "https://mixed.com".to_string(),
                    description: "From mixed".to_string(),
                    age: None,
                    extra_snippets: None,
                }]),
            }),
            web: None,
        };
        let formatted = brave.format_results(response);
        assert!(formatted.contains("Mixed Result"));
        assert!(formatted.contains("From mixed"));
    }

    #[test]
    fn test_format_results_long_description_truncation() {
        let brave = BraveSearchTool::new("test-key");
        let long_desc = "A".repeat(300); // 300 chars, should be truncated to 250
        let response = BraveSearchResponse {
            query: BraveQuery {
                original: "long".to_string(),
                altered: None,
            },
            mixed: None,
            web: Some(WebResults {
                results: vec![BraveWebResult {
                    title: "Long Desc".to_string(),
                    url: "https://long.com".to_string(),
                    description: long_desc,
                    age: None,
                    extra_snippets: None,
                }],
            }),
        };
        let formatted = brave.format_results(response);
        // Should contain truncated description (250 chars + "...")
        assert!(formatted.contains("..."));
        // The full 300 A's should not appear
        assert!(!formatted.contains(&"A".repeat(300)));
    }

    #[test]
    fn test_format_results_exact_250_chars_no_truncation() {
        let brave = BraveSearchTool::new("test-key");
        let desc_250 = "B".repeat(250);
        let response = BraveSearchResponse {
            query: BraveQuery {
                original: "exact".to_string(),
                altered: None,
            },
            mixed: None,
            web: Some(WebResults {
                results: vec![BraveWebResult {
                    title: "Exact 250".to_string(),
                    url: "https://exact.com".to_string(),
                    description: desc_250.clone(),
                    age: None,
                    extra_snippets: None,
                }],
            }),
        };
        let formatted = brave.format_results(response);
        // Should contain the full 250 chars without truncation marker
        assert!(formatted.contains(&desc_250));
    }

    #[test]
    fn test_format_results_without_age() {
        let brave = BraveSearchTool::new("test-key");
        let response = BraveSearchResponse {
            query: BraveQuery {
                original: "no age".to_string(),
                altered: None,
            },
            mixed: None,
            web: Some(WebResults {
                results: vec![BraveWebResult {
                    title: "No Age".to_string(),
                    url: "https://noage.com".to_string(),
                    description: "Without age".to_string(),
                    age: None,
                    extra_snippets: None,
                }],
            }),
        };
        let formatted = brave.format_results(response);
        assert!(formatted.contains("No Age"));
        assert!(!formatted.contains("Age:"));
    }

    #[test]
    fn test_format_results_with_empty_web_results() {
        let brave = BraveSearchTool::new("test-key");
        let response = BraveSearchResponse {
            query: BraveQuery {
                original: "empty web".to_string(),
                altered: None,
            },
            mixed: None,
            web: Some(WebResults { results: vec![] }),
        };
        let formatted = brave.format_results(response);
        assert_eq!(formatted, "No results found.");
    }

    #[test]
    fn test_format_results_mixed_empty_results() {
        let brave = BraveSearchTool::new("test-key");
        let response = BraveSearchResponse {
            query: BraveQuery {
                original: "empty mixed".to_string(),
                altered: None,
            },
            mixed: Some(BraveMixed {
                result_type: "search".to_string(),
                results: Some(vec![]),
            }),
            web: None,
        };
        let formatted = brave.format_results(response);
        assert_eq!(formatted, "No results found.");
    }

    #[test]
    fn test_format_results_mixed_none_results() {
        let brave = BraveSearchTool::new("test-key");
        let response = BraveSearchResponse {
            query: BraveQuery {
                original: "none results".to_string(),
                altered: None,
            },
            mixed: Some(BraveMixed {
                result_type: "search".to_string(),
                results: None,
            }),
            web: None,
        };
        let formatted = brave.format_results(response);
        assert_eq!(formatted, "No results found.");
    }

    // =============================================================================
    // Network tests (require API key - marked #[ignore])
    // =============================================================================

    #[tokio::test]
    #[ignore = "Requires BRAVE_API_KEY environment variable"]
    async fn test_live_search() {
        let api_key = std::env::var("BRAVE_API_KEY").expect("BRAVE_API_KEY must be set");
        let brave = BraveSearchTool::new(api_key);
        let result = brave._call_str("Rust programming language".to_string()).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Rust"));
    }

    #[tokio::test]
    #[ignore = "Requires BRAVE_API_KEY environment variable"]
    async fn test_live_search_with_freshness() {
        let api_key = std::env::var("BRAVE_API_KEY").expect("BRAVE_API_KEY must be set");
        let brave = BraveSearchTool::builder()
            .api_key(api_key)
            .freshness("pd") // Past day
            .count(5)
            .build()
            .unwrap();
        let result = brave._call_str("news".to_string()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "Requires BRAVE_API_KEY environment variable"]
    async fn test_live_search_with_localization() {
        let api_key = std::env::var("BRAVE_API_KEY").expect("BRAVE_API_KEY must be set");
        let brave = BraveSearchTool::builder()
            .api_key(api_key)
            .search_lang("de")
            .country("DE")
            .build()
            .unwrap();
        let result = brave._call_str("Berlin".to_string()).await;
        assert!(result.is_ok());
    }
}
