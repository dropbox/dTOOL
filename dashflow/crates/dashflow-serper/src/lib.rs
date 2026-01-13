//! # Google Serper Tool
//!
//! Google Serper API integration for `DashFlow`. Provides fast, affordable access to Google
//! search results through the Serper.dev API.
//!
//! ## Features
//!
//! - Fast Google search results via Serper.dev
//! - Affordable pricing (cheaper than official Google API)
//! - Multiple search types: web, images, news, places
//! - Rich snippets and knowledge graphs
//! - No rate limiting on paid plans
//!
//! ## Usage
//!
//! ```rust,no_run
//! use dashflow_serper::SerperTool;
//! use dashflow::core::tools::Tool;
//!
//! # tokio_test::block_on(async {
//! let serper = SerperTool::new("YOUR_API_KEY");
//!
//! // Simple search
//! let results = serper._call_str("What is deep learning?".to_string()).await.unwrap();
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

/// Search type for Serper API
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SearchType {
    /// Web search (default)
    #[default]
    Search,
    /// News search
    News,
    /// Image search
    Images,
    /// Video search
    Videos,
    /// Places search
    Places,
}

/// A single organic search result from Serper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerperOrganic {
    /// Page title
    pub title: String,
    /// Page URL
    pub link: String,
    /// Content snippet
    pub snippet: String,
    /// Position in results
    pub position: u32,
    /// Sitelinks (sub-pages)
    #[serde(default)]
    pub sitelinks: Option<Vec<SerperSitelink>>,
}

/// Sitelink to a sub-page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerperSitelink {
    /// Link title
    pub title: String,
    /// Link URL
    pub link: String,
}

/// Knowledge graph result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerperKnowledgeGraph {
    /// Title
    pub title: String,
    /// Type (e.g., "Person", "Organization")
    #[serde(rename = "type")]
    pub result_type: Option<String>,
    /// Description
    pub description: Option<String>,
    /// Website
    pub website: Option<String>,
}

/// Search parameters for Serper API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerperSearchParams {
    /// Search query
    pub q: String,
    /// Number of results (default: 10)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num: Option<u32>,
    /// Geographic location (e.g., "United States", "us")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gl: Option<String>,
    /// Language (e.g., "en", "es")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hl: Option<String>,
}

/// Response from Serper search API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerperSearchResponse {
    /// Organic search results
    #[serde(default)]
    pub organic: Vec<SerperOrganic>,
    /// Knowledge graph (if present)
    #[serde(default)]
    pub knowledge_graph: Option<SerperKnowledgeGraph>,
    /// Answer box (direct answer)
    #[serde(default)]
    pub answer_box: Option<serde_json::Value>,
    /// Related searches
    #[serde(default)]
    pub related_searches: Option<Vec<serde_json::Value>>,
}

/// Google Serper search tool for `DashFlow` agents
///
/// This tool provides access to Google search results through the Serper.dev API,
/// which is faster and more affordable than the official Google Search API.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_serper::SerperTool;
/// use dashflow::core::tools::Tool;
///
/// # tokio_test::block_on(async {
/// let serper = SerperTool::builder()
///     .api_key("YOUR_API_KEY")
///     .num_results(10)
///     .location("United States")
///     .language("en")
///     .build()
///     .unwrap();
///
/// let results = serper._call_str("latest AI developments".to_string())
///     .await
///     .unwrap();
/// println!("Found: {}", results);
/// # });
/// ```
#[derive(Debug)]
pub struct SerperTool {
    api_key: String,
    num_results: u32,
    location: Option<String>,
    language: Option<String>,
    search_type: SearchType,
    client: reqwest::Client,
}

impl SerperTool {
    /// Create a new Serper search tool
    ///
    /// # Arguments
    ///
    /// * `api_key` - Your Serper API key (X-API-KEY)
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_serper::SerperTool;
    ///
    /// let serper = SerperTool::new("YOUR_API_KEY");
    /// ```
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            num_results: 10,
            location: None,
            language: None,
            search_type: SearchType::Search,
            client: create_http_client(),
        }
    }

    /// Create a builder for `SerperTool`
    #[must_use]
    pub fn builder() -> SerperToolBuilder {
        SerperToolBuilder::default()
    }

    /// Perform a search using the Serper API
    async fn search(&self, query: String) -> Result<SerperSearchResponse> {
        let endpoint = match self.search_type {
            SearchType::Search => "https://google.serper.dev/search",
            SearchType::News => "https://google.serper.dev/news",
            SearchType::Images => "https://google.serper.dev/images",
            SearchType::Videos => "https://google.serper.dev/videos",
            SearchType::Places => "https://google.serper.dev/places",
        };

        let params = SerperSearchParams {
            q: query,
            num: Some(self.num_results),
            gl: self.location.clone(),
            hl: self.language.clone(),
        };

        let response = self
            .client
            .post(endpoint)
            .header("X-API-KEY", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&params)
            .send()
            .await
            .map_err(|e| {
                dashflow::core::Error::tool_error(format!("Serper API request failed: {e}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(dashflow::core::Error::tool_error(format!(
                "Serper API error ({status}): {error_text}"
            )));
        }

        // M-216: Use size-limited JSON parsing to prevent memory exhaustion
        let search_response: SerperSearchResponse =
            json_with_limit(response, SEARCH_RESPONSE_SIZE_LIMIT).await.map_err(|e| {
                dashflow::core::Error::tool_error(format!("Failed to parse Serper response: {e}"))
            })?;

        Ok(search_response)
    }

    /// Format search results as a string
    fn format_results(&self, response: SerperSearchResponse) -> String {
        let mut output = String::new();

        // Include knowledge graph if present
        if let Some(kg) = response.knowledge_graph {
            output.push_str("Knowledge Graph:\n");
            output.push_str(&format!("  Title: {}\n", kg.title));
            if let Some(result_type) = kg.result_type {
                output.push_str(&format!("  Type: {result_type}\n"));
            }
            if let Some(description) = kg.description {
                output.push_str(&format!("  Description: {description}\n"));
            }
            if let Some(website) = kg.website {
                output.push_str(&format!("  Website: {website}\n"));
            }
            output.push('\n');
        }

        // Include answer box if present
        if let Some(answer) = response.answer_box {
            output.push_str("Answer Box:\n");
            output.push_str(&format!("  {answer}\n\n"));
        }

        // Include organic results
        if response.organic.is_empty() {
            output.push_str("No results found.");
            return output;
        }

        output.push_str(&format!("Found {} results:\n\n", response.organic.len()));

        for result in &response.organic {
            output.push_str(&format!("{}. {}\n", result.position, result.title));
            output.push_str(&format!("   URL: {}\n", result.link));

            // Truncate snippet to 250 chars
            let snippet = if result.snippet.len() > 250 {
                format!("{}...", &result.snippet[..250])
            } else {
                result.snippet.clone()
            };
            output.push_str(&format!("   Snippet: {snippet}\n"));

            // Include sitelinks if present
            if let Some(sitelinks) = &result.sitelinks {
                if !sitelinks.is_empty() {
                    output.push_str("   Sitelinks:\n");
                    for (i, sitelink) in sitelinks.iter().take(3).enumerate() {
                        output.push_str(&format!(
                            "     {}: {} ({})\n",
                            i + 1,
                            sitelink.title,
                            sitelink.link
                        ));
                    }
                }
            }

            output.push('\n');
        }

        output
    }
}

#[async_trait]
impl Tool for SerperTool {
    fn name(&self) -> &'static str {
        "serper_search"
    }

    fn description(&self) -> &'static str {
        "Search Google using the Serper.dev API. \
         Fast and affordable access to Google search results with knowledge graphs, \
         answer boxes, and rich snippets. Returns comprehensive web search results \
         from Google's index. Best for general Google search queries."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query to execute on Google"
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

/// Builder for `SerperTool`
#[derive(Default)]
pub struct SerperToolBuilder {
    api_key: Option<String>,
    num_results: Option<u32>,
    location: Option<String>,
    language: Option<String>,
    search_type: Option<SearchType>,
}

impl SerperToolBuilder {
    /// Set the API key
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set the maximum number of results (1-100)
    #[must_use]
    pub fn num_results(mut self, num: u32) -> Self {
        self.num_results = Some(num.clamp(1, 100));
        self
    }

    /// Set the geographic location (e.g., "United States", "us")
    pub fn location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    /// Set the language (e.g., "en", "es", "fr")
    pub fn language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Set the search type (search, news, images, videos, places)
    #[must_use]
    pub fn search_type(mut self, search_type: SearchType) -> Self {
        self.search_type = Some(search_type);
        self
    }

    /// Build the `SerperTool`
    pub fn build(self) -> Result<SerperTool> {
        let api_key = self.api_key.ok_or_else(|| {
            dashflow::core::Error::tool_error("API key is required for Serper search".to_string())
        })?;

        Ok(SerperTool {
            api_key,
            num_results: self.num_results.unwrap_or(10),
            location: self.location,
            language: self.language,
            search_type: self.search_type.unwrap_or_default(),
            client: create_http_client(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // ============================================================================
    // Constructor and Builder Tests (12 tests)
    // ============================================================================

    #[test]
    fn test_serper_tool_creation() {
        let serper = SerperTool::new("test-key");
        assert_eq!(serper.name(), "serper_search");
        assert!(serper.description().contains("Serper"));
        assert!(serper.description().contains("Google"));
    }

    #[test]
    fn test_new_with_string() {
        let serper = SerperTool::new(String::from("api-key-123"));
        assert_eq!(serper.api_key, "api-key-123");
    }

    #[test]
    fn test_new_with_str() {
        let serper = SerperTool::new("api-key-456");
        assert_eq!(serper.api_key, "api-key-456");
    }

    #[test]
    fn test_new_default_values() {
        let serper = SerperTool::new("test-key");
        assert_eq!(serper.num_results, 10);
        assert!(serper.location.is_none());
        assert!(serper.language.is_none());
        assert!(matches!(serper.search_type, SearchType::Search));
    }

    #[test]
    fn test_serper_tool_builder() {
        let serper = SerperTool::builder()
            .api_key("test-key")
            .num_results(20)
            .location("US")
            .language("en")
            .build()
            .unwrap();

        assert_eq!(serper.api_key, "test-key");
        assert_eq!(serper.num_results, 20);
        assert_eq!(serper.location, Some("US".to_string()));
        assert_eq!(serper.language, Some("en".to_string()));
    }

    #[test]
    fn test_serper_builder_missing_key() {
        let result = SerperTool::builder().num_results(10).build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_default_num_results() {
        let serper = SerperTool::builder().api_key("test").build().unwrap();
        assert_eq!(serper.num_results, 10);
    }

    #[test]
    fn test_builder_chaining() {
        let result = SerperTool::builder()
            .api_key("key")
            .num_results(5)
            .location("UK")
            .language("en")
            .search_type(SearchType::News)
            .build();
        assert!(result.is_ok());
        let serper = result.unwrap();
        assert_eq!(serper.num_results, 5);
        assert_eq!(serper.location, Some("UK".to_string()));
    }

    #[test]
    fn test_num_results_clamping_high() {
        let serper = SerperTool::builder()
            .api_key("test")
            .num_results(200) // Should be clamped to 100
            .build()
            .unwrap();

        assert_eq!(serper.num_results, 100);
    }

    #[test]
    fn test_num_results_clamping_zero() {
        let serper = SerperTool::builder()
            .api_key("test")
            .num_results(0) // Should be clamped to 1
            .build()
            .unwrap();

        assert_eq!(serper.num_results, 1);
    }

    #[test]
    fn test_num_results_valid_range() {
        for num in [1, 10, 50, 100] {
            let serper = SerperTool::builder()
                .api_key("test")
                .num_results(num)
                .build()
                .unwrap();
            assert_eq!(serper.num_results, num);
        }
    }

    #[test]
    fn test_builder_with_all_search_types() {
        for search_type in [
            SearchType::Search,
            SearchType::News,
            SearchType::Images,
            SearchType::Videos,
            SearchType::Places,
        ] {
            let serper = SerperTool::builder()
                .api_key("test")
                .search_type(search_type.clone())
                .build()
                .unwrap();
            assert!(matches!(serper.search_type, _ if std::mem::discriminant(&serper.search_type) == std::mem::discriminant(&search_type)));
        }
    }

    // ============================================================================
    // Tool Trait Implementation Tests (10 tests)
    // ============================================================================

    #[test]
    fn test_serper_args_schema() {
        let serper = SerperTool::new("test-key");
        let schema = serper.args_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
        assert_eq!(schema["required"][0], "query");
    }

    #[test]
    fn test_name_is_static() {
        let serper = SerperTool::new("test-key");
        assert_eq!(serper.name(), "serper_search");
    }

    #[test]
    fn test_name_same_across_instances() {
        let s1 = SerperTool::new("key1");
        let s2 = SerperTool::new("key2");
        assert_eq!(s1.name(), s2.name());
    }

    #[test]
    fn test_description_not_empty() {
        let serper = SerperTool::new("test-key");
        assert!(!serper.description().is_empty());
    }

    #[test]
    fn test_description_contains_google() {
        let serper = SerperTool::new("test-key");
        assert!(serper.description().contains("Google"));
    }

    #[test]
    fn test_description_contains_serper() {
        let serper = SerperTool::new("test-key");
        assert!(serper.description().contains("Serper"));
    }

    #[test]
    fn test_args_schema_has_properties() {
        let serper = SerperTool::new("test-key");
        let schema = serper.args_schema();
        assert!(schema.get("properties").is_some());
    }

    #[test]
    fn test_args_schema_query_is_string() {
        let serper = SerperTool::new("test-key");
        let schema = serper.args_schema();
        assert_eq!(schema["properties"]["query"]["type"], "string");
    }

    #[test]
    fn test_args_schema_query_has_description() {
        let serper = SerperTool::new("test-key");
        let schema = serper.args_schema();
        assert!(schema["properties"]["query"]["description"].is_string());
    }

    #[test]
    fn test_args_schema_required_is_array() {
        let serper = SerperTool::new("test-key");
        let schema = serper.args_schema();
        assert!(schema["required"].is_array());
    }

    // ============================================================================
    // SearchType Enum Tests (8 tests)
    // ============================================================================

    #[test]
    fn test_search_type_default() {
        let st = SearchType::default();
        assert!(matches!(st, SearchType::Search));
    }

    #[test]
    fn test_search_type_clone() {
        let st = SearchType::News;
        let st2 = st.clone();
        assert!(matches!(st2, SearchType::News));
    }

    #[test]
    fn test_search_type_debug() {
        let st = SearchType::Images;
        let debug = format!("{:?}", st);
        assert!(debug.contains("Images"));
    }

    #[test]
    fn test_search_type_serialize_search() {
        let st = SearchType::Search;
        let json = serde_json::to_string(&st).unwrap();
        assert_eq!(json, "\"search\"");
    }

    #[test]
    fn test_search_type_serialize_news() {
        let st = SearchType::News;
        let json = serde_json::to_string(&st).unwrap();
        assert_eq!(json, "\"news\"");
    }

    #[test]
    fn test_search_type_serialize_images() {
        let st = SearchType::Images;
        let json = serde_json::to_string(&st).unwrap();
        assert_eq!(json, "\"images\"");
    }

    #[test]
    fn test_search_type_deserialize() {
        let st: SearchType = serde_json::from_str("\"videos\"").unwrap();
        assert!(matches!(st, SearchType::Videos));
    }

    #[test]
    fn test_search_type_deserialize_places() {
        let st: SearchType = serde_json::from_str("\"places\"").unwrap();
        assert!(matches!(st, SearchType::Places));
    }

    // ============================================================================
    // Data Structure Tests (14 tests)
    // ============================================================================

    #[test]
    fn test_serper_organic_serialize() {
        let organic = SerperOrganic {
            title: "Test".to_string(),
            link: "https://example.com".to_string(),
            snippet: "Snippet".to_string(),
            position: 1,
            sitelinks: None,
        };
        let json = serde_json::to_string(&organic).unwrap();
        assert!(json.contains("\"title\":\"Test\""));
        assert!(json.contains("\"position\":1"));
    }

    #[test]
    fn test_serper_organic_deserialize() {
        let json = r#"{"title":"Test","link":"https://example.com","snippet":"Desc","position":2}"#;
        let organic: SerperOrganic = serde_json::from_str(json).unwrap();
        assert_eq!(organic.title, "Test");
        assert_eq!(organic.position, 2);
    }

    #[test]
    fn test_serper_organic_clone() {
        let organic = SerperOrganic {
            title: "Test".to_string(),
            link: "https://example.com".to_string(),
            snippet: "Snippet".to_string(),
            position: 1,
            sitelinks: None,
        };
        let cloned = organic.clone();
        assert_eq!(cloned.title, organic.title);
        assert_eq!(cloned.position, organic.position);
    }

    #[test]
    fn test_serper_organic_debug() {
        let organic = SerperOrganic {
            title: "Debug Test".to_string(),
            link: "https://test.com".to_string(),
            snippet: "Snippet".to_string(),
            position: 5,
            sitelinks: None,
        };
        let debug = format!("{:?}", organic);
        assert!(debug.contains("Debug Test"));
        assert!(debug.contains("5"));
    }

    #[test]
    fn test_serper_sitelink_serialize() {
        let sitelink = SerperSitelink {
            title: "Link Title".to_string(),
            link: "https://example.com/page".to_string(),
        };
        let json = serde_json::to_string(&sitelink).unwrap();
        assert!(json.contains("Link Title"));
    }

    #[test]
    fn test_serper_sitelink_deserialize() {
        let json = r#"{"title":"Page","link":"https://example.com"}"#;
        let sitelink: SerperSitelink = serde_json::from_str(json).unwrap();
        assert_eq!(sitelink.title, "Page");
    }

    #[test]
    fn test_knowledge_graph_serialize() {
        let kg = SerperKnowledgeGraph {
            title: "Test KG".to_string(),
            result_type: Some("Organization".to_string()),
            description: Some("Desc".to_string()),
            website: Some("https://kg.com".to_string()),
        };
        let json = serde_json::to_string(&kg).unwrap();
        assert!(json.contains("Test KG"));
        assert!(json.contains("Organization"));
    }

    #[test]
    fn test_knowledge_graph_deserialize() {
        let json = r#"{"title":"KG","type":"Person","description":"A person","website":"https://person.com"}"#;
        let kg: SerperKnowledgeGraph = serde_json::from_str(json).unwrap();
        assert_eq!(kg.title, "KG");
        assert_eq!(kg.result_type, Some("Person".to_string()));
    }

    #[test]
    fn test_knowledge_graph_optional_fields() {
        let json = r#"{"title":"Minimal"}"#;
        let kg: SerperKnowledgeGraph = serde_json::from_str(json).unwrap();
        assert_eq!(kg.title, "Minimal");
        assert!(kg.result_type.is_none());
        assert!(kg.description.is_none());
        assert!(kg.website.is_none());
    }

    #[test]
    fn test_search_params_serialize() {
        let params = SerperSearchParams {
            q: "test query".to_string(),
            num: Some(10),
            gl: Some("us".to_string()),
            hl: Some("en".to_string()),
        };
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("test query"));
        assert!(json.contains("\"num\":10"));
    }

    #[test]
    fn test_search_params_skip_none() {
        let params = SerperSearchParams {
            q: "query".to_string(),
            num: None,
            gl: None,
            hl: None,
        };
        let json = serde_json::to_string(&params).unwrap();
        assert!(!json.contains("num"));
        assert!(!json.contains("gl"));
        assert!(!json.contains("hl"));
    }

    #[test]
    fn test_search_response_deserialize() {
        let json = r#"{"organic":[{"title":"Result","link":"https://r.com","snippet":"s","position":1}]}"#;
        let response: SerperSearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.organic.len(), 1);
        assert_eq!(response.organic[0].title, "Result");
    }

    #[test]
    fn test_search_response_empty_organic() {
        let json = r#"{"organic":[]}"#;
        let response: SerperSearchResponse = serde_json::from_str(json).unwrap();
        assert!(response.organic.is_empty());
    }

    #[test]
    fn test_search_response_with_related_searches() {
        let json = r#"{"organic":[],"relatedSearches":[{"query":"related"}]}"#;
        let response: SerperSearchResponse = serde_json::from_str(json).unwrap();
        assert!(response.related_searches.is_some());
    }

    // ============================================================================
    // format_results Tests (12 tests)
    // ============================================================================

    #[test]
    fn test_format_results_basic() {
        let serper = SerperTool::new("test-key");
        let response = SerperSearchResponse {
            organic: vec![SerperOrganic {
                title: "Test Result".to_string(),
                link: "https://example.com".to_string(),
                snippet: "This is test content".to_string(),
                position: 1,
                sitelinks: None,
            }],
            knowledge_graph: None,
            answer_box: None,
            related_searches: None,
        };

        let formatted = serper.format_results(response);
        assert!(formatted.contains("Test Result"));
        assert!(formatted.contains("https://example.com"));
        assert!(formatted.contains("This is test content"));
    }

    #[test]
    fn test_format_results_with_knowledge_graph() {
        let serper = SerperTool::new("test-key");
        let response = SerperSearchResponse {
            organic: vec![SerperOrganic {
                title: "Test Result".to_string(),
                link: "https://example.com".to_string(),
                snippet: "This is test content".to_string(),
                position: 1,
                sitelinks: None,
            }],
            knowledge_graph: Some(SerperKnowledgeGraph {
                title: "Test KG".to_string(),
                result_type: Some("Website".to_string()),
                description: Some("Test description".to_string()),
                website: Some("https://example.com".to_string()),
            }),
            answer_box: None,
            related_searches: None,
        };

        let formatted = serper.format_results(response);
        assert!(formatted.contains("Knowledge Graph"));
        assert!(formatted.contains("Test KG"));
    }

    #[test]
    fn test_format_empty_results() {
        let serper = SerperTool::new("test-key");
        let response = SerperSearchResponse {
            organic: vec![],
            knowledge_graph: None,
            answer_box: None,
            related_searches: None,
        };

        let formatted = serper.format_results(response);
        assert_eq!(formatted, "No results found.");
    }

    #[test]
    fn test_format_results_multiple_organic() {
        let serper = SerperTool::new("test-key");
        let response = SerperSearchResponse {
            organic: vec![
                SerperOrganic {
                    title: "First".to_string(),
                    link: "https://first.com".to_string(),
                    snippet: "First snippet".to_string(),
                    position: 1,
                    sitelinks: None,
                },
                SerperOrganic {
                    title: "Second".to_string(),
                    link: "https://second.com".to_string(),
                    snippet: "Second snippet".to_string(),
                    position: 2,
                    sitelinks: None,
                },
            ],
            knowledge_graph: None,
            answer_box: None,
            related_searches: None,
        };

        let formatted = serper.format_results(response);
        assert!(formatted.contains("Found 2 results"));
        assert!(formatted.contains("First"));
        assert!(formatted.contains("Second"));
    }

    #[test]
    fn test_format_results_long_snippet_truncated() {
        let serper = SerperTool::new("test-key");
        let long_snippet = "A".repeat(300);
        let response = SerperSearchResponse {
            organic: vec![SerperOrganic {
                title: "Long".to_string(),
                link: "https://long.com".to_string(),
                snippet: long_snippet.clone(),
                position: 1,
                sitelinks: None,
            }],
            knowledge_graph: None,
            answer_box: None,
            related_searches: None,
        };

        let formatted = serper.format_results(response);
        assert!(formatted.contains("..."));
        // Should truncate to 250 chars + "..."
        assert!(!formatted.contains(&long_snippet));
    }

    #[test]
    fn test_format_results_short_snippet_not_truncated() {
        let serper = SerperTool::new("test-key");
        let short_snippet = "Short snippet";
        let response = SerperSearchResponse {
            organic: vec![SerperOrganic {
                title: "Short".to_string(),
                link: "https://short.com".to_string(),
                snippet: short_snippet.to_string(),
                position: 1,
                sitelinks: None,
            }],
            knowledge_graph: None,
            answer_box: None,
            related_searches: None,
        };

        let formatted = serper.format_results(response);
        assert!(formatted.contains(short_snippet));
    }

    #[test]
    fn test_format_results_with_sitelinks() {
        let serper = SerperTool::new("test-key");
        let response = SerperSearchResponse {
            organic: vec![SerperOrganic {
                title: "Main".to_string(),
                link: "https://main.com".to_string(),
                snippet: "Main snippet".to_string(),
                position: 1,
                sitelinks: Some(vec![
                    SerperSitelink {
                        title: "Sub 1".to_string(),
                        link: "https://main.com/sub1".to_string(),
                    },
                    SerperSitelink {
                        title: "Sub 2".to_string(),
                        link: "https://main.com/sub2".to_string(),
                    },
                ]),
            }],
            knowledge_graph: None,
            answer_box: None,
            related_searches: None,
        };

        let formatted = serper.format_results(response);
        assert!(formatted.contains("Sitelinks"));
        assert!(formatted.contains("Sub 1"));
        assert!(formatted.contains("Sub 2"));
    }

    #[test]
    fn test_format_results_sitelinks_limited_to_3() {
        let serper = SerperTool::new("test-key");
        let response = SerperSearchResponse {
            organic: vec![SerperOrganic {
                title: "Main".to_string(),
                link: "https://main.com".to_string(),
                snippet: "Main snippet".to_string(),
                position: 1,
                sitelinks: Some(vec![
                    SerperSitelink { title: "L1".to_string(), link: "https://l1.com".to_string() },
                    SerperSitelink { title: "L2".to_string(), link: "https://l2.com".to_string() },
                    SerperSitelink { title: "L3".to_string(), link: "https://l3.com".to_string() },
                    SerperSitelink { title: "L4".to_string(), link: "https://l4.com".to_string() },
                    SerperSitelink { title: "L5".to_string(), link: "https://l5.com".to_string() },
                ]),
            }],
            knowledge_graph: None,
            answer_box: None,
            related_searches: None,
        };

        let formatted = serper.format_results(response);
        assert!(formatted.contains("L1"));
        assert!(formatted.contains("L2"));
        assert!(formatted.contains("L3"));
        // L4 and L5 should be cut off by take(3)
        assert!(!formatted.contains("L4"));
        assert!(!formatted.contains("L5"));
    }

    #[test]
    fn test_format_results_with_answer_box() {
        let serper = SerperTool::new("test-key");
        let response = SerperSearchResponse {
            organic: vec![SerperOrganic {
                title: "Result".to_string(),
                link: "https://r.com".to_string(),
                snippet: "Snippet".to_string(),
                position: 1,
                sitelinks: None,
            }],
            knowledge_graph: None,
            answer_box: Some(json!({"answer": "42"})),
            related_searches: None,
        };

        let formatted = serper.format_results(response);
        assert!(formatted.contains("Answer Box"));
    }

    #[test]
    fn test_format_results_kg_without_optional_fields() {
        let serper = SerperTool::new("test-key");
        let response = SerperSearchResponse {
            organic: vec![SerperOrganic {
                title: "Result".to_string(),
                link: "https://r.com".to_string(),
                snippet: "Snippet".to_string(),
                position: 1,
                sitelinks: None,
            }],
            knowledge_graph: Some(SerperKnowledgeGraph {
                title: "Minimal KG".to_string(),
                result_type: None,
                description: None,
                website: None,
            }),
            answer_box: None,
            related_searches: None,
        };

        let formatted = serper.format_results(response);
        assert!(formatted.contains("Minimal KG"));
        assert!(!formatted.contains("Type:"));
        assert!(!formatted.contains("Description:"));
    }

    #[test]
    fn test_format_results_position_ordering() {
        let serper = SerperTool::new("test-key");
        let response = SerperSearchResponse {
            organic: vec![
                SerperOrganic {
                    title: "Third".to_string(),
                    link: "https://3.com".to_string(),
                    snippet: "3".to_string(),
                    position: 3,
                    sitelinks: None,
                },
                SerperOrganic {
                    title: "First".to_string(),
                    link: "https://1.com".to_string(),
                    snippet: "1".to_string(),
                    position: 1,
                    sitelinks: None,
                },
            ],
            knowledge_graph: None,
            answer_box: None,
            related_searches: None,
        };

        let formatted = serper.format_results(response);
        // Position numbers should be displayed as received (3. Third, then 1. First)
        assert!(formatted.contains("3. Third"));
        assert!(formatted.contains("1. First"));
    }

    // ============================================================================
    // Send + Sync Bounds Tests (4 tests)
    // ============================================================================

    #[test]
    fn test_serper_tool_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<SerperTool>();
    }

    #[test]
    fn test_serper_tool_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<SerperTool>();
    }

    #[test]
    fn test_search_type_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<SearchType>();
    }

    #[test]
    fn test_search_type_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<SearchType>();
    }

    // ============================================================================
    // Edge Case Tests (8 tests)
    // ============================================================================

    #[test]
    fn test_empty_api_key() {
        let serper = SerperTool::new("");
        assert_eq!(serper.api_key, "");
    }

    #[test]
    fn test_api_key_with_special_chars() {
        let serper = SerperTool::new("key-with-special_chars.123");
        assert_eq!(serper.api_key, "key-with-special_chars.123");
    }

    #[test]
    fn test_unicode_location() {
        let serper = SerperTool::builder()
            .api_key("key")
            .location("日本")
            .build()
            .unwrap();
        assert_eq!(serper.location, Some("日本".to_string()));
    }

    #[test]
    fn test_unicode_language() {
        let serper = SerperTool::builder()
            .api_key("key")
            .language("中文")
            .build()
            .unwrap();
        assert_eq!(serper.language, Some("中文".to_string()));
    }

    #[test]
    fn test_empty_location() {
        let serper = SerperTool::builder()
            .api_key("key")
            .location("")
            .build()
            .unwrap();
        assert_eq!(serper.location, Some("".to_string()));
    }

    #[test]
    fn test_organic_with_empty_sitelinks() {
        let organic = SerperOrganic {
            title: "Test".to_string(),
            link: "https://t.com".to_string(),
            snippet: "S".to_string(),
            position: 1,
            sitelinks: Some(vec![]),
        };
        let serper = SerperTool::new("key");
        let response = SerperSearchResponse {
            organic: vec![organic],
            knowledge_graph: None,
            answer_box: None,
            related_searches: None,
        };
        let formatted = serper.format_results(response);
        // Empty sitelinks should not show "Sitelinks:" section
        assert!(!formatted.contains("Sitelinks"));
    }

    #[test]
    fn test_special_chars_in_snippet() {
        let organic = SerperOrganic {
            title: "Test".to_string(),
            link: "https://t.com".to_string(),
            snippet: "Line1\nLine2\t\"quoted\" <tag>".to_string(),
            position: 1,
            sitelinks: None,
        };
        let serper = SerperTool::new("key");
        let response = SerperSearchResponse {
            organic: vec![organic],
            knowledge_graph: None,
            answer_box: None,
            related_searches: None,
        };
        let formatted = serper.format_results(response);
        assert!(formatted.contains("Line1"));
    }

    #[test]
    fn test_very_long_title() {
        let long_title = "T".repeat(500);
        let organic = SerperOrganic {
            title: long_title.clone(),
            link: "https://t.com".to_string(),
            snippet: "S".to_string(),
            position: 1,
            sitelinks: None,
        };
        let serper = SerperTool::new("key");
        let response = SerperSearchResponse {
            organic: vec![organic],
            knowledge_graph: None,
            answer_box: None,
            related_searches: None,
        };
        let formatted = serper.format_results(response);
        // Titles are not truncated
        assert!(formatted.contains(&long_title));
    }

    // ============================================================================
    // Concurrent Usage Tests (2 tests)
    // ============================================================================

    #[test]
    fn test_concurrent_arc_access() {
        let serper = Arc::new(SerperTool::new("shared-key"));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let s = Arc::clone(&serper);
                std::thread::spawn(move || {
                    assert_eq!(s.name(), "serper_search");
                    s.description().len()
                })
            })
            .collect();

        for handle in handles {
            let _ = handle.join().unwrap();
        }
    }

    #[test]
    fn test_multiple_instances_independent() {
        let s1 = SerperTool::builder()
            .api_key("key1")
            .num_results(5)
            .build()
            .unwrap();
        let s2 = SerperTool::builder()
            .api_key("key2")
            .num_results(20)
            .build()
            .unwrap();

        assert_eq!(s1.api_key, "key1");
        assert_eq!(s2.api_key, "key2");
        assert_eq!(s1.num_results, 5);
        assert_eq!(s2.num_results, 20);
    }

    // ============================================================================
    // Builder Error Message Tests (2 tests)
    // ============================================================================

    #[test]
    fn test_builder_error_message_contains_api_key() {
        let result = SerperTool::builder().build();
        let err = result.unwrap_err();
        let err_str = format!("{:?}", err);
        assert!(err_str.contains("API key") || err_str.contains("api_key"));
    }

    #[test]
    fn test_builder_returns_result() {
        let ok_result = SerperTool::builder().api_key("test").build();
        assert!(ok_result.is_ok());

        let err_result = SerperTool::builder().build();
        assert!(err_result.is_err());
    }
}
