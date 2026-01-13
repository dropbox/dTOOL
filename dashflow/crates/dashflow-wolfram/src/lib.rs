// Note: Audited - only 1 expect() in production (line 290 in build(), documented # Panics).
// All other unwrap/expect calls are in test code.

//! `WolframAlpha` Computational Knowledge Engine Tool for `DashFlow` Rust
//!
//! This crate provides a tool for querying the `WolframAlpha` computational knowledge engine
//! to get answers to factual questions, mathematical computations, and scientific queries.
//!
//! # Overview
//!
//! `WolframAlpha` is a computational knowledge engine that answers factual queries by
//! computing answers from curated data. Unlike search engines, it computes answers
//! rather than finding documents.
//!
//! # Features
//!
//! - Simple API for computational queries
//! - Support for mathematics, science, geography, history, and more
//! - Short answer format optimized for LLM integration
//! - Optional full result format for detailed responses
//!
//! # Example
//!
//! ```rust,no_run
//! use dashflow_wolfram::WolframAlpha;
//! use dashflow::core::tools::{Tool, ToolInput};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let tool = WolframAlpha::new("YOUR_APP_ID");
//!
//! let input = ToolInput::String("What is the population of France?".to_string());
//! let result = tool._call(input).await?;
//! println!("Answer: {}", result);
//! # Ok(())
//! # }
//! ```
//!
//! # API Details
//!
//! The tool uses the `WolframAlpha` Short Answers API:
//! - **Endpoint**: `https://api.wolframalpha.com/v1/result`
//! - **Authentication**: App ID required (get one at <https://products.wolframalpha.com/api/>)
//! - **Rate Limits**: 2,000 queries per month on free tier
//!
//! # References
//!
//! - [WolframAlpha API](https://products.wolframalpha.com/api/)
//! - [Short Answers API Documentation](https://products.wolframalpha.com/short-answers-api/documentation/)

use async_trait::async_trait;
use dashflow::core::error::{Error, Result};
use dashflow::core::tools::{Tool, ToolInput};
use reqwest::Client;
use serde_json::json;
use url::Url;

const WOLFRAM_SHORT_API_URL: &str = "https://api.wolframalpha.com/v1/result";

/// `WolframAlpha` computational knowledge engine tool
///
/// Provides access to `WolframAlpha`'s computational knowledge engine for answering
/// factual questions, performing calculations, and providing scientific information.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_wolfram::WolframAlpha;
/// use dashflow::core::tools::{Tool, ToolInput};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let tool = WolframAlpha::builder()
///     .app_id("YOUR_APP_ID")
///     .build();
///
/// let input = ToolInput::String("What is the square root of 144?".to_string());
/// let result = tool._call(input).await?;
/// println!("{}", result);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct WolframAlpha {
    /// HTTP client
    client: Client,
    /// `WolframAlpha` App ID (required)
    app_id: String,
    /// Units system: "metric" or "nonmetric" (default: "metric")
    units: String,
    /// Timeout in seconds (default: 30)
    timeout: u64,
}

impl WolframAlpha {
    /// Create a new `WolframAlpha` tool with the given App ID
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_wolfram::WolframAlpha;
    ///
    /// let tool = WolframAlpha::new("YOUR_APP_ID");
    /// ```
    pub fn new(app_id: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            app_id: app_id.into(),
            units: "metric".to_string(),
            timeout: 30,
        }
    }

    /// Create a builder for configuring the `WolframAlpha` tool
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_wolfram::WolframAlpha;
    ///
    /// let tool = WolframAlpha::builder()
    ///     .app_id("YOUR_APP_ID")
    ///     .units("nonmetric")
    ///     .timeout(60)
    ///     .build();
    /// ```
    #[must_use]
    pub fn builder() -> WolframAlphaBuilder {
        WolframAlphaBuilder::default()
    }

    /// Set the units system (metric or nonmetric)
    pub fn units(mut self, units: impl Into<String>) -> Self {
        self.units = units.into();
        self
    }

    /// Set the timeout in seconds
    #[must_use]
    pub fn timeout(mut self, timeout: u64) -> Self {
        self.timeout = timeout;
        self
    }

    /// Build the `WolframAlpha` tool
    #[must_use]
    pub fn build(self) -> Self {
        self
    }

    /// Query `WolframAlpha` and return the result
    ///
    /// # Arguments
    ///
    /// * `query` - The question or computation to send to `WolframAlpha`
    ///
    /// # Returns
    ///
    /// The short answer from `WolframAlpha` as a string
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The query is empty
    /// - The API request fails
    /// - The API returns an error (invalid query, no result found, etc.)
    async fn query(&self, query: &str) -> Result<String> {
        if query.trim().is_empty() {
            return Err(Error::tool_error("Query cannot be empty"));
        }

        // Build the request URL
        let mut url = Url::parse(WOLFRAM_SHORT_API_URL)
            .map_err(|e| Error::tool_error(format!("Failed to parse API URL: {e}")))?;

        url.query_pairs_mut()
            .append_pair("appid", &self.app_id)
            .append_pair("i", query)
            .append_pair("units", &self.units);

        // Make the request
        let response = self
            .client
            .get(url)
            .timeout(std::time::Duration::from_secs(self.timeout))
            .send()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to query WolframAlpha: {e}")))?;

        // Check for HTTP errors
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();

            return Err(Error::tool_error(format!(
                "WolframAlpha API error (status {}): {}",
                status,
                if error_text.is_empty() {
                    "No result found or invalid query"
                } else {
                    &error_text
                }
            )));
        }

        // Get the response text
        let result = response
            .text()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to read response: {e}")))?;

        Ok(result)
    }
}

#[async_trait]
impl Tool for WolframAlpha {
    fn name(&self) -> &'static str {
        "wolfram_alpha"
    }

    fn description(&self) -> &'static str {
        "A computational knowledge engine for answering factual questions. \
         Use this tool for: mathematical calculations, scientific data, \
         conversions, geography, history, and general factual information. \
         Input should be a clear question or computation."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The question or computation to send to WolframAlpha"
                }
            },
            "required": ["query"]
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let query = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(value) => value
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::tool_error("Missing 'query' field in input"))?
                .to_string(),
        };

        self.query(&query).await
    }
}

/// Builder for `WolframAlpha` tool
#[derive(Debug, Default)]
pub struct WolframAlphaBuilder {
    app_id: Option<String>,
    units: Option<String>,
    timeout: Option<u64>,
}

impl WolframAlphaBuilder {
    /// Set the `WolframAlpha` App ID (required)
    pub fn app_id(mut self, app_id: impl Into<String>) -> Self {
        self.app_id = Some(app_id.into());
        self
    }

    /// Set the units system: "metric" or "nonmetric" (default: "metric")
    pub fn units(mut self, units: impl Into<String>) -> Self {
        self.units = Some(units.into());
        self
    }

    /// Set the timeout in seconds (default: 30)
    #[must_use]
    pub fn timeout(mut self, timeout: u64) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Build the `WolframAlpha` tool
    ///
    /// # Panics
    ///
    /// Panics if `app_id` is not set
    #[must_use]
    #[allow(clippy::expect_used)] // Documented panic for missing required field
    pub fn build(self) -> WolframAlpha {
        let app_id = self.app_id.expect("app_id is required");

        WolframAlpha {
            client: Client::new(),
            app_id,
            units: self.units.unwrap_or_else(|| "metric".to_string()),
            timeout: self.timeout.unwrap_or(30),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // CONSTRUCTOR TESTS
    // ========================================================================

    #[test]
    fn test_new() {
        let tool = WolframAlpha::new("test_app_id");
        assert_eq!(tool.app_id, "test_app_id");
        assert_eq!(tool.units, "metric");
        assert_eq!(tool.timeout, 30);
    }

    #[test]
    fn test_new_empty_app_id() {
        let tool = WolframAlpha::new("");
        assert_eq!(tool.app_id, "");
    }

    #[test]
    fn test_new_with_string_type() {
        let tool = WolframAlpha::new(String::from("my_app_id"));
        assert_eq!(tool.app_id, "my_app_id");
    }

    #[test]
    fn test_new_with_special_characters() {
        let tool = WolframAlpha::new("app-id_123!@#");
        assert_eq!(tool.app_id, "app-id_123!@#");
    }

    #[test]
    fn test_new_with_unicode() {
        let tool = WolframAlpha::new("日本語_key");
        assert_eq!(tool.app_id, "日本語_key");
    }

    // ========================================================================
    // BUILDER TESTS
    // ========================================================================

    #[test]
    fn test_builder() {
        let tool = WolframAlpha::builder()
            .app_id("test_app_id")
            .units("nonmetric")
            .timeout(60)
            .build();

        assert_eq!(tool.app_id, "test_app_id");
        assert_eq!(tool.units, "nonmetric");
        assert_eq!(tool.timeout, 60);
    }

    #[test]
    fn test_builder_defaults() {
        let tool = WolframAlpha::builder().app_id("test_id").build();

        assert_eq!(tool.app_id, "test_id");
        assert_eq!(tool.units, "metric"); // default
        assert_eq!(tool.timeout, 30); // default
    }

    #[test]
    fn test_builder_metric_units() {
        let tool = WolframAlpha::builder()
            .app_id("test")
            .units("metric")
            .build();

        assert_eq!(tool.units, "metric");
    }

    #[test]
    fn test_builder_nonmetric_units() {
        let tool = WolframAlpha::builder()
            .app_id("test")
            .units("nonmetric")
            .build();

        assert_eq!(tool.units, "nonmetric");
    }

    #[test]
    fn test_builder_custom_timeout() {
        let tool = WolframAlpha::builder()
            .app_id("test")
            .timeout(120)
            .build();

        assert_eq!(tool.timeout, 120);
    }

    #[test]
    fn test_builder_zero_timeout() {
        let tool = WolframAlpha::builder().app_id("test").timeout(0).build();

        assert_eq!(tool.timeout, 0);
    }

    #[test]
    fn test_builder_large_timeout() {
        let tool = WolframAlpha::builder()
            .app_id("test")
            .timeout(3600)
            .build();

        assert_eq!(tool.timeout, 3600);
    }

    #[test]
    #[should_panic(expected = "app_id is required")]
    fn test_builder_missing_app_id() {
        let _ = WolframAlpha::builder().build();
    }

    #[test]
    fn test_builder_chain_order_independent() {
        let tool1 = WolframAlpha::builder()
            .app_id("test")
            .units("nonmetric")
            .timeout(45)
            .build();

        let tool2 = WolframAlpha::builder()
            .timeout(45)
            .app_id("test")
            .units("nonmetric")
            .build();

        assert_eq!(tool1.app_id, tool2.app_id);
        assert_eq!(tool1.units, tool2.units);
        assert_eq!(tool1.timeout, tool2.timeout);
    }

    // ========================================================================
    // FLUENT API TESTS
    // ========================================================================

    #[test]
    fn test_fluent_units() {
        let tool = WolframAlpha::new("test").units("nonmetric");
        assert_eq!(tool.units, "nonmetric");
    }

    #[test]
    fn test_fluent_timeout() {
        let tool = WolframAlpha::new("test").timeout(90);
        assert_eq!(tool.timeout, 90);
    }

    #[test]
    fn test_fluent_chaining() {
        let tool = WolframAlpha::new("test").units("nonmetric").timeout(90).build();

        assert_eq!(tool.units, "nonmetric");
        assert_eq!(tool.timeout, 90);
    }

    #[test]
    fn test_fluent_build_identity() {
        let tool = WolframAlpha::new("test").build();
        assert_eq!(tool.app_id, "test");
    }

    // ========================================================================
    // TOOL TRAIT TESTS
    // ========================================================================

    #[test]
    fn test_tool_trait() {
        let tool = WolframAlpha::new("test_app_id");
        assert_eq!(tool.name(), "wolfram_alpha");
        assert!(tool.description().contains("computational knowledge"));
    }

    #[test]
    fn test_tool_name_constant() {
        let tool1 = WolframAlpha::new("id1");
        let tool2 = WolframAlpha::new("id2");
        assert_eq!(tool1.name(), tool2.name());
    }

    #[test]
    fn test_tool_description_content() {
        let tool = WolframAlpha::new("test");
        let desc = tool.description();

        assert!(desc.contains("computational"));
        assert!(desc.contains("mathematical"));
        assert!(desc.contains("scientific"));
    }

    #[test]
    fn test_tool_description_not_empty() {
        let tool = WolframAlpha::new("test");
        assert!(!tool.description().is_empty());
    }

    // ========================================================================
    // ARGS SCHEMA TESTS
    // ========================================================================

    #[test]
    fn test_args_schema() {
        let tool = WolframAlpha::new("test_app_id");
        let schema = tool.args_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
        assert_eq!(schema["required"][0], "query");
    }

    #[test]
    fn test_args_schema_query_type() {
        let tool = WolframAlpha::new("test");
        let schema = tool.args_schema();

        assert_eq!(schema["properties"]["query"]["type"], "string");
    }

    #[test]
    fn test_args_schema_query_description() {
        let tool = WolframAlpha::new("test");
        let schema = tool.args_schema();

        let desc = schema["properties"]["query"]["description"]
            .as_str()
            .unwrap_or("");
        assert!(!desc.is_empty());
        assert!(desc.to_lowercase().contains("wolfram"));
    }

    #[test]
    fn test_args_schema_has_required() {
        let tool = WolframAlpha::new("test");
        let schema = tool.args_schema();

        assert!(schema["required"].is_array());
        assert!(!schema["required"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_args_schema_is_valid_json() {
        let tool = WolframAlpha::new("test");
        let schema = tool.args_schema();

        // Verify we can serialize and deserialize
        let json_str = serde_json::to_string(&schema).unwrap();
        let _: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    }

    // ========================================================================
    // CLONE AND DEBUG TESTS
    // ========================================================================

    #[test]
    fn test_clone() {
        let tool = WolframAlpha::new("test").units("nonmetric").timeout(45);
        let cloned = tool.clone();

        assert_eq!(cloned.app_id, "test");
        assert_eq!(cloned.units, "nonmetric");
        assert_eq!(cloned.timeout, 45);
    }

    #[test]
    fn test_clone_independence() {
        let tool1 = WolframAlpha::new("original");
        let tool2 = tool1.clone();

        // Cloned should be independent (modifying one doesn't affect other)
        // Since WolframAlpha is immutable after construction, just verify they're equal
        assert_eq!(tool1.app_id, tool2.app_id);
    }

    #[test]
    fn test_debug() {
        let tool = WolframAlpha::new("test_id");
        let debug_str = format!("{:?}", tool);

        assert!(debug_str.contains("WolframAlpha"));
        assert!(debug_str.contains("test_id"));
    }

    // ========================================================================
    // BUILDER DEFAULT TESTS
    // ========================================================================

    #[test]
    fn test_builder_default_instance() {
        let builder = WolframAlphaBuilder::default();
        let debug = format!("{:?}", builder);
        assert!(debug.contains("WolframAlphaBuilder"));
    }

    // ========================================================================
    // API URL CONSTANT TEST
    // ========================================================================

    #[test]
    fn test_api_url_constant() {
        assert_eq!(WOLFRAM_SHORT_API_URL, "https://api.wolframalpha.com/v1/result");
    }

    #[test]
    fn test_api_url_is_https() {
        assert!(WOLFRAM_SHORT_API_URL.starts_with("https://"));
    }

    #[test]
    fn test_api_url_valid_format() {
        let url = Url::parse(WOLFRAM_SHORT_API_URL);
        assert!(url.is_ok());
    }

    // ========================================================================
    // ASYNC QUERY TESTS (DETERMINISTIC)
    // ========================================================================

    #[tokio::test]
    async fn test_empty_query() {
        let tool = WolframAlpha::new("test_app_id");
        let result = tool.query("").await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[tokio::test]
    async fn test_whitespace_only_query() {
        let tool = WolframAlpha::new("test_app_id");
        let result = tool.query("   ").await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[tokio::test]
    async fn test_tabs_only_query() {
        let tool = WolframAlpha::new("test_app_id");
        let result = tool.query("\t\t").await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_newlines_only_query() {
        let tool = WolframAlpha::new("test_app_id");
        let result = tool.query("\n\n").await;

        assert!(result.is_err());
    }

    // ========================================================================
    // TOOL INPUT PARSING TESTS
    // ========================================================================

    #[tokio::test]
    async fn test_call_missing_query_field() {
        let tool = WolframAlpha::new("test_app_id");
        let value = json!({"other_field": "value"});
        let input = ToolInput::Structured(value);
        let result = tool._call(input).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("query"));
    }

    #[tokio::test]
    async fn test_call_empty_string_input() {
        let tool = WolframAlpha::new("test_app_id");
        let input = ToolInput::String(String::new());
        let result = tool._call(input).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_structured_empty_query() {
        let tool = WolframAlpha::new("test_app_id");
        let value = json!({"query": ""});
        let input = ToolInput::Structured(value);
        let result = tool._call(input).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_structured_null_query() {
        let tool = WolframAlpha::new("test_app_id");
        let value = json!({"query": null});
        let input = ToolInput::Structured(value);
        let result = tool._call(input).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_structured_numeric_query() {
        let tool = WolframAlpha::new("test_app_id");
        let value = json!({"query": 123}); // number instead of string
        let input = ToolInput::Structured(value);
        let result = tool._call(input).await;

        assert!(result.is_err());
    }

    // ========================================================================
    // INTEGRATION TESTS (REQUIRE API KEY)
    // ========================================================================

    #[tokio::test]
    #[ignore = "requires WOLFRAM_APP_ID"]
    async fn test_query_integration() {
        let app_id = std::env::var("WOLFRAM_APP_ID").expect("WOLFRAM_APP_ID must be set");

        let tool = WolframAlpha::new(app_id);
        let result = tool.query("What is 2 + 2?").await;

        assert!(result.is_ok());
        let answer = result.unwrap();
        assert!(answer.contains("4") || answer.to_lowercase().contains("four"));
    }

    #[tokio::test]
    #[ignore = "requires WOLFRAM_APP_ID"]
    async fn test_tool_call_string_input() {
        let app_id = std::env::var("WOLFRAM_APP_ID").expect("WOLFRAM_APP_ID must be set");

        let tool = WolframAlpha::new(app_id);
        let input = ToolInput::String("What is the capital of France?".to_string());
        let result = tool._call(input).await;

        assert!(result.is_ok());
        let answer = result.unwrap();
        assert!(answer.to_lowercase().contains("paris"));
    }

    #[tokio::test]
    #[ignore = "requires WOLFRAM_APP_ID"]
    async fn test_tool_call_structured_input() {
        let app_id = std::env::var("WOLFRAM_APP_ID").expect("WOLFRAM_APP_ID must be set");

        let tool = WolframAlpha::new(app_id);
        let value = json!({"query": "What is the speed of light?"});
        let input = ToolInput::Structured(value);
        let result = tool._call(input).await;

        // Verify the response contains speed of light information
        let answer = result.expect("WolframAlpha should return a response");
        assert!(
            answer.to_lowercase().contains("299")
                || answer.to_lowercase().contains("light")
                || answer.to_lowercase().contains("m/s"),
            "Answer should contain speed of light information, got: {}",
            answer
        );
    }

    // ========================================================================
    // ADDITIONAL CONSTRUCTOR TESTS
    // ========================================================================

    #[test]
    fn test_new_very_long_app_id() {
        let long_id = "x".repeat(1000);
        let tool = WolframAlpha::new(&long_id);
        assert_eq!(tool.app_id.len(), 1000);
    }

    #[test]
    fn test_new_app_id_with_whitespace() {
        let tool = WolframAlpha::new("  id with spaces  ");
        assert_eq!(tool.app_id, "  id with spaces  ");
    }

    #[test]
    fn test_new_preserves_case() {
        let tool = WolframAlpha::new("MyAppID");
        assert_eq!(tool.app_id, "MyAppID");
    }

    // ========================================================================
    // ADDITIONAL BUILDER TESTS
    // ========================================================================

    #[test]
    fn test_builder_overwrite_app_id() {
        let tool = WolframAlpha::builder()
            .app_id("first")
            .app_id("second")
            .build();
        assert_eq!(tool.app_id, "second");
    }

    #[test]
    fn test_builder_overwrite_units() {
        let tool = WolframAlpha::builder()
            .app_id("test")
            .units("metric")
            .units("nonmetric")
            .build();
        assert_eq!(tool.units, "nonmetric");
    }

    #[test]
    fn test_builder_overwrite_timeout() {
        let tool = WolframAlpha::builder()
            .app_id("test")
            .timeout(10)
            .timeout(100)
            .build();
        assert_eq!(tool.timeout, 100);
    }

    #[test]
    fn test_builder_max_timeout() {
        let tool = WolframAlpha::builder()
            .app_id("test")
            .timeout(u64::MAX)
            .build();
        assert_eq!(tool.timeout, u64::MAX);
    }

    #[test]
    fn test_builder_empty_units() {
        let tool = WolframAlpha::builder()
            .app_id("test")
            .units("")
            .build();
        assert_eq!(tool.units, "");
    }

    #[test]
    fn test_builder_custom_units_string() {
        let tool = WolframAlpha::builder()
            .app_id("test")
            .units("imperial")
            .build();
        assert_eq!(tool.units, "imperial");
    }

    #[test]
    fn test_builder_app_id_string_ownership() {
        let id = String::from("owned_id");
        let tool = WolframAlpha::builder().app_id(id).build();
        assert_eq!(tool.app_id, "owned_id");
    }

    #[test]
    fn test_builder_units_string_ownership() {
        let units = String::from("owned_units");
        let tool = WolframAlpha::builder()
            .app_id("test")
            .units(units)
            .build();
        assert_eq!(tool.units, "owned_units");
    }

    // ========================================================================
    // ADDITIONAL FLUENT API TESTS
    // ========================================================================

    #[test]
    fn test_fluent_units_empty() {
        let tool = WolframAlpha::new("test").units("");
        assert_eq!(tool.units, "");
    }

    #[test]
    fn test_fluent_multiple_units_calls() {
        let tool = WolframAlpha::new("test").units("a").units("b").units("c");
        assert_eq!(tool.units, "c");
    }

    #[test]
    fn test_fluent_multiple_timeout_calls() {
        let tool = WolframAlpha::new("test").timeout(1).timeout(2).timeout(3);
        assert_eq!(tool.timeout, 3);
    }

    #[test]
    fn test_fluent_units_preserves_other_fields() {
        let tool = WolframAlpha::new("original_id").timeout(99).units("custom");
        assert_eq!(tool.app_id, "original_id");
        assert_eq!(tool.timeout, 99);
        assert_eq!(tool.units, "custom");
    }

    #[test]
    fn test_fluent_timeout_preserves_other_fields() {
        let tool = WolframAlpha::new("original_id").units("custom").timeout(99);
        assert_eq!(tool.app_id, "original_id");
        assert_eq!(tool.units, "custom");
        assert_eq!(tool.timeout, 99);
    }

    // ========================================================================
    // ADDITIONAL TOOL TRAIT TESTS
    // ========================================================================

    #[test]
    fn test_tool_name_is_lowercase() {
        let tool = WolframAlpha::new("test");
        assert_eq!(tool.name(), tool.name().to_lowercase());
    }

    #[test]
    fn test_tool_name_no_spaces() {
        let tool = WolframAlpha::new("test");
        assert!(!tool.name().contains(' '));
    }

    #[test]
    fn test_tool_description_mentions_factual() {
        let tool = WolframAlpha::new("test");
        assert!(tool.description().to_lowercase().contains("factual"));
    }

    #[test]
    fn test_tool_description_mentions_calculations() {
        let tool = WolframAlpha::new("test");
        assert!(tool.description().to_lowercase().contains("calculation"));
    }

    // ========================================================================
    // ADDITIONAL ARGS SCHEMA TESTS
    // ========================================================================

    #[test]
    fn test_args_schema_no_additional_properties() {
        let tool = WolframAlpha::new("test");
        let schema = tool.args_schema();

        // Schema should only have known properties
        let props = schema["properties"].as_object().unwrap();
        assert_eq!(props.len(), 1);
        assert!(props.contains_key("query"));
    }

    #[test]
    fn test_args_schema_single_required_field() {
        let tool = WolframAlpha::new("test");
        let schema = tool.args_schema();

        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 1);
    }

    #[test]
    fn test_args_schema_stability() {
        let tool = WolframAlpha::new("test");
        let schema1 = tool.args_schema();
        let schema2 = tool.args_schema();

        assert_eq!(schema1, schema2);
    }

    // ========================================================================
    // ADDITIONAL CLONE AND DEBUG TESTS
    // ========================================================================

    #[test]
    fn test_clone_all_fields() {
        let tool = WolframAlpha::builder()
            .app_id("clone_id")
            .units("clone_units")
            .timeout(999)
            .build();

        let cloned = tool.clone();
        assert_eq!(cloned.app_id, "clone_id");
        assert_eq!(cloned.units, "clone_units");
        assert_eq!(cloned.timeout, 999);
    }

    #[test]
    fn test_debug_contains_units() {
        let tool = WolframAlpha::new("test").units("debug_units");
        let debug_str = format!("{:?}", tool);
        assert!(debug_str.contains("debug_units"));
    }

    #[test]
    fn test_debug_contains_timeout() {
        let tool = WolframAlpha::new("test").timeout(12345);
        let debug_str = format!("{:?}", tool);
        assert!(debug_str.contains("12345"));
    }

    #[test]
    fn test_builder_debug() {
        let builder = WolframAlpha::builder().app_id("debug_test");
        let debug_str = format!("{:?}", builder);
        assert!(debug_str.contains("debug_test"));
    }

    #[test]
    fn test_builder_debug_default() {
        let builder = WolframAlphaBuilder::default();
        let debug_str = format!("{:?}", builder);
        // Should contain None for unset fields
        assert!(debug_str.contains("None"));
    }

    // ========================================================================
    // ADDITIONAL QUERY VALIDATION TESTS
    // ========================================================================

    #[tokio::test]
    async fn test_mixed_whitespace_query() {
        let tool = WolframAlpha::new("test");
        let result = tool.query("  \t\n  ").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_unicode_whitespace_query() {
        let tool = WolframAlpha::new("test");
        // Non-breaking space and other unicode whitespace
        let result = tool.query("\u{00A0}\u{2003}").await;
        // These may or may not be considered empty depending on trim behavior
        // At minimum, it should not panic
        let _ = result;
    }

    // ========================================================================
    // ADDITIONAL TOOL INPUT TESTS
    // ========================================================================

    #[tokio::test]
    async fn test_call_structured_boolean_query() {
        let tool = WolframAlpha::new("test");
        let value = json!({"query": true});
        let input = ToolInput::Structured(value);
        let result = tool._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_structured_array_query() {
        let tool = WolframAlpha::new("test");
        let value = json!({"query": ["item1", "item2"]});
        let input = ToolInput::Structured(value);
        let result = tool._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_structured_object_query() {
        let tool = WolframAlpha::new("test");
        let value = json!({"query": {"nested": "object"}});
        let input = ToolInput::Structured(value);
        let result = tool._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_structured_empty_object() {
        let tool = WolframAlpha::new("test");
        let value = json!({});
        let input = ToolInput::Structured(value);
        let result = tool._call(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("query"));
    }

    #[tokio::test]
    async fn test_call_string_whitespace_only() {
        let tool = WolframAlpha::new("test");
        let input = ToolInput::String("   ".to_string());
        let result = tool._call(input).await;
        assert!(result.is_err());
    }

    // ========================================================================
    // ERROR MESSAGE QUALITY TESTS
    // ========================================================================

    #[tokio::test]
    async fn test_empty_query_error_message_quality() {
        let tool = WolframAlpha::new("test");
        let result = tool.query("").await;
        let err = result.unwrap_err();
        let err_str = err.to_string().to_lowercase();
        assert!(
            err_str.contains("empty") || err_str.contains("query"),
            "Error message should mention 'empty' or 'query', got: {err_str}"
        );
    }

    #[tokio::test]
    async fn test_missing_query_field_error_message_quality() {
        let tool = WolframAlpha::new("test");
        let value = json!({"not_query": "value"});
        let input = ToolInput::Structured(value);
        let result = tool._call(input).await;
        let err = result.unwrap_err();
        let err_str = err.to_string().to_lowercase();
        assert!(
            err_str.contains("query"),
            "Error message should mention 'query', got: {err_str}"
        );
    }

    // ========================================================================
    // STRUCT FIELD VERIFICATION TESTS
    // ========================================================================

    #[test]
    fn test_default_units_is_metric() {
        let tool = WolframAlpha::new("test");
        assert_eq!(tool.units, "metric");
    }

    #[test]
    fn test_default_timeout_is_30() {
        let tool = WolframAlpha::new("test");
        assert_eq!(tool.timeout, 30);
    }

    #[test]
    fn test_client_is_created() {
        let tool = WolframAlpha::new("test");
        // Just verify the client field exists and is accessible
        let _ = &tool.client;
    }

    // ========================================================================
    // URL BUILDING VERIFICATION TESTS
    // ========================================================================

    #[test]
    fn test_api_url_host() {
        let url = Url::parse(WOLFRAM_SHORT_API_URL).unwrap();
        assert_eq!(url.host_str(), Some("api.wolframalpha.com"));
    }

    #[test]
    fn test_api_url_path() {
        let url = Url::parse(WOLFRAM_SHORT_API_URL).unwrap();
        assert_eq!(url.path(), "/v1/result");
    }

    #[test]
    fn test_api_url_no_query_params() {
        let url = Url::parse(WOLFRAM_SHORT_API_URL).unwrap();
        assert!(url.query().is_none());
    }

    #[test]
    fn test_api_url_scheme() {
        let url = Url::parse(WOLFRAM_SHORT_API_URL).unwrap();
        assert_eq!(url.scheme(), "https");
    }

    // ========================================================================
    // BOUNDARY AND EDGE CASE TESTS
    // ========================================================================

    #[test]
    fn test_builder_timeout_boundary_1() {
        let tool = WolframAlpha::builder().app_id("test").timeout(1).build();
        assert_eq!(tool.timeout, 1);
    }

    #[test]
    fn test_app_id_with_url_special_chars() {
        let tool = WolframAlpha::new("id?with&special=chars");
        assert_eq!(tool.app_id, "id?with&special=chars");
    }

    #[test]
    fn test_units_with_url_special_chars() {
        let tool = WolframAlpha::new("test").units("metric?nonmetric");
        assert_eq!(tool.units, "metric?nonmetric");
    }

    #[tokio::test]
    async fn test_query_with_special_url_chars() {
        let tool = WolframAlpha::new("test");
        // This will fail at HTTP level (no valid API key), but should not error on URL building
        let result = tool.query("what is 2+2?").await;
        // Either succeeds (unlikely without key) or fails with HTTP error, not URL error
        if let Err(e) = result {
            let err_str = e.to_string().to_lowercase();
            assert!(
                !err_str.contains("url") || err_str.contains("api") || err_str.contains("http"),
                "Error should be HTTP-related, not URL parsing: {err_str}"
            );
        }
    }

    #[tokio::test]
    async fn test_query_with_unicode() {
        let tool = WolframAlpha::new("test");
        let result = tool.query("日本の首都は何ですか？").await;
        // Should not panic, will fail at HTTP level
        let _ = result;
    }

    #[tokio::test]
    async fn test_query_with_emoji() {
        let tool = WolframAlpha::new("test");
        let result = tool.query("What is 🔢 + 🔢?").await;
        // Should not panic
        let _ = result;
    }

    #[tokio::test]
    async fn test_query_very_long() {
        let tool = WolframAlpha::new("test");
        let long_query = "a".repeat(10000);
        let result = tool.query(&long_query).await;
        // Should not panic; may fail at HTTP level
        let _ = result;
    }
}
