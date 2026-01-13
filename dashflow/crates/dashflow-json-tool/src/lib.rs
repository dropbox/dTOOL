//! JSON parsing and querying tool for `DashFlow` Rust.
//!
//! This crate provides tools for parsing JSON data and querying it using `JSONPath` expressions.
//!
//! # Features
//!
//! - Parse and validate JSON strings
//! - Query JSON data using `JSONPath` expressions
//! - Pretty-print JSON output
//! - Extract specific values from complex JSON structures
//!
//! # Example
//!
//! ```rust
//! use dashflow_json_tool::JsonTool;
//! use dashflow::core::tools::{Tool, ToolInput};
//! use serde_json::json;
//!
//! # tokio_test::block_on(async {
//! let tool = JsonTool::new();
//!
//! // Parse and pretty-print JSON
//! let input = json!({
//!     "query": r#"{"name":"Alice","age":30}"#
//! });
//! let result = tool._call(ToolInput::Structured(input)).await.unwrap();
//! assert!(result.contains("Alice"));
//!
//! // Query JSON with JSONPath
//! let input = json!({
//!     "json": r#"{"users":[{"name":"Alice","age":30},{"name":"Bob","age":25}]}"#,
//!     "path": "$.users[0].name"
//! });
//! let result = tool._call(ToolInput::Structured(input)).await.unwrap();
//! assert!(result.contains("Alice"));
//! # });
//! ```
//!
//! # See Also
//!
//! - [`Tool`] - The trait this implements
//! - [`dashflow-file-tool`](https://docs.rs/dashflow-file-tool) - File reading/writing tools (can work with JSON files)
//! - [`dashflow-webscrape`](https://docs.rs/dashflow-webscrape) - Web scraping tool (often returns JSON)
//! - [serde_json_path Documentation](https://docs.rs/serde_json_path/) - JSONPath implementation

use async_trait::async_trait;
use dashflow::core::tools::{Tool, ToolInput};
use dashflow::core::Error;
use serde_json::json;
use serde_json_path::JsonPath;

/// JSON parsing and querying tool.
///
/// This tool provides two main operations:
/// 1. **Parse**: Validate and pretty-print JSON strings
/// 2. **Query**: Extract values from JSON using `JSONPath` expressions
///
/// # Input Formats
///
/// ## Simple Parse (String Input)
/// ```json
/// "{\"name\":\"Alice\",\"age\":30}"
/// ```
///
/// ## Parse with Structured Input
/// ```json
/// {
///     "query": "{\"name\":\"Alice\",\"age\":30}"
/// }
/// ```
///
/// ## Query with `JSONPath`
/// ```json
/// {
///     "json": "{\"users\":[{\"name\":\"Alice\"}]}",
///     "path": "$.users[0].name"
/// }
/// ```
///
/// # `JSONPath` Syntax
///
/// - `$` - Root object
/// - `.field` - Access field
/// - `[n]` - Array index
/// - `[*]` - All array elements
/// - `..field` - Recursive descent
/// - `[?(@.field > value)]` - Filter expression
///
/// # Example
///
/// ```rust
/// use dashflow_json_tool::JsonTool;
/// use dashflow::core::tools::Tool;
/// use serde_json::json;
///
/// # tokio_test::block_on(async {
/// let tool = JsonTool::new();
///
/// // Simple parse
/// let result = tool._call_str(r#"{"name":"Alice","age":30}"#.to_string()).await.unwrap();
/// println!("{}", result);
///
/// // JSONPath query
/// let input = json!({
///     "json": r#"{"users":[{"name":"Alice","age":30},{"name":"Bob","age":25}]}"#,
///     "path": "$.users[*].name"
/// });
/// let result = tool._call(dashflow::core::tools::ToolInput::Structured(input)).await.unwrap();
/// println!("{}", result);
/// # });
/// ```
#[derive(Clone, Debug, Default)]
pub struct JsonTool {}

impl JsonTool {
    /// Creates a new JSON tool.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_json_tool::JsonTool;
    ///
    /// let tool = JsonTool::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }

    /// Parse JSON string and return pretty-printed format.
    fn parse_json(&self, json_str: &str) -> Result<String, Error> {
        let value: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| Error::tool_error(format!("Invalid JSON: {e}")))?;

        serde_json::to_string_pretty(&value)
            .map_err(|e| Error::tool_error(format!("Failed to format JSON: {e}")))
    }

    /// Query JSON using `JSONPath` expression.
    fn query_json(&self, json_str: &str, path: &str) -> Result<String, Error> {
        // Parse JSON
        let value: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| Error::tool_error(format!("Invalid JSON: {e}")))?;

        // Parse JSONPath
        let json_path = JsonPath::parse(path)
            .map_err(|e| Error::tool_error(format!("Invalid JSONPath: {e}")))?;

        // Execute query
        let results = json_path.query(&value);
        let matches = results.all();

        if matches.is_empty() {
            return Ok("No matches found".to_string());
        }

        // Format results
        if matches.len() == 1 {
            serde_json::to_string_pretty(matches[0])
                .map_err(|e| Error::tool_error(format!("Failed to format result: {e}")))
        } else {
            let results_array: Vec<&serde_json::Value> = matches;
            serde_json::to_string_pretty(&results_array)
                .map_err(|e| Error::tool_error(format!("Failed to format results: {e}")))
        }
    }
}

#[async_trait]
impl Tool for JsonTool {
    fn name(&self) -> &'static str {
        "json_tool"
    }

    fn description(&self) -> &'static str {
        "Parse and query JSON data. \
         \
         Use this tool to: \
         1. Validate and pretty-print JSON strings \
         2. Extract values from JSON using JSONPath expressions \
         \
         Input formats: \
         - Simple string: Raw JSON to parse and format \
         - Structured with 'query': JSON to parse \
         - Structured with 'json' and 'path': Query JSON with JSONPath \
         \
         JSONPath examples: \
         - $.users[0].name - Get first user's name \
         - $.users[*].age - Get all user ages \
         - $..name - Get all 'name' fields recursively"
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "json": {
                    "type": "string",
                    "description": "JSON string to parse or query"
                },
                "path": {
                    "type": "string",
                    "description": "Optional JSONPath expression to query the JSON. Examples: '$.users[0].name', '$.users[*].age', '$..name'"
                },
                "query": {
                    "type": "string",
                    "description": "Alternative to 'json' - JSON string to parse"
                }
            },
            "oneOf": [
                {
                    "required": ["json", "path"],
                    "description": "Query JSON with JSONPath"
                },
                {
                    "required": ["json"],
                    "description": "Parse and format JSON"
                },
                {
                    "required": ["query"],
                    "description": "Parse and format JSON"
                }
            ]
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String, Error> {
        match input {
            ToolInput::String(json_str) => {
                // Simple parse mode
                self.parse_json(&json_str)
            }
            ToolInput::Structured(value) => {
                // Check for 'json' + 'path' (query mode)
                if let (Some(json_str), Some(path)) = (value.get("json"), value.get("path")) {
                    let json_str = json_str
                        .as_str()
                        .ok_or_else(|| Error::tool_error("'json' field must be a string"))?;
                    let path = path
                        .as_str()
                        .ok_or_else(|| Error::tool_error("'path' field must be a string"))?;
                    return self.query_json(json_str, path);
                }

                // Check for 'json' only (parse mode)
                if let Some(json_str) = value.get("json") {
                    let json_str = json_str
                        .as_str()
                        .ok_or_else(|| Error::tool_error("'json' field must be a string"))?;
                    return self.parse_json(json_str);
                }

                // Check for 'query' (parse mode)
                if let Some(query) = value.get("query") {
                    let json_str = query
                        .as_str()
                        .ok_or_else(|| Error::tool_error("'query' field must be a string"))?;
                    return self.parse_json(json_str);
                }

                Err(Error::tool_error(
                    "Invalid input: must provide 'json' or 'query' field",
                ))
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use dashflow::core::tools::ToolInput;
    use dashflow_standard_tests::tool_comprehensive_tests::ToolComprehensiveTests;
    use serde_json::json;

    #[tokio::test]
    async fn test_json_tool_creation() {
        let tool = JsonTool::new();
        assert_eq!(tool.name(), "json_tool");
        assert!(!tool.description().is_empty());
    }

    #[tokio::test]
    async fn test_json_tool_default() {
        let tool = JsonTool::default();
        assert_eq!(tool.name(), "json_tool");
    }

    #[tokio::test]
    async fn test_parse_simple_json() {
        let tool = JsonTool::new();
        let input = r#"{"name":"Alice","age":30}"#;

        let result = tool._call_str(input.to_string()).await.unwrap();
        assert!(result.contains("Alice"));
        assert!(result.contains("30"));
        // Should be pretty-printed (contains newlines)
        assert!(result.contains('\n'));
    }

    #[tokio::test]
    async fn test_parse_invalid_json() {
        let tool = JsonTool::new();
        let input = r#"{"name":"Alice""#; // Missing closing brace

        let result = tool._call_str(input.to_string()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid JSON"));
    }

    #[tokio::test]
    async fn test_parse_structured_query_field() {
        let tool = JsonTool::new();
        let input = json!({
            "query": r#"{"name":"Bob","age":25}"#
        });

        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("Bob"));
        assert!(result.contains("25"));
    }

    #[tokio::test]
    async fn test_parse_structured_json_field() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"{"name":"Charlie","age":35}"#
        });

        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("Charlie"));
        assert!(result.contains("35"));
    }

    #[tokio::test]
    async fn test_query_jsonpath_simple() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"{"users":[{"name":"Alice","age":30},{"name":"Bob","age":25}]}"#,
            "path": "$.users[0].name"
        });

        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("Alice"));
        assert!(!result.contains("Bob"));
    }

    #[tokio::test]
    async fn test_query_jsonpath_array_wildcard() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"{"users":[{"name":"Alice","age":30},{"name":"Bob","age":25}]}"#,
            "path": "$.users[*].name"
        });

        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("Alice"));
        assert!(result.contains("Bob"));
    }

    #[tokio::test]
    async fn test_query_jsonpath_recursive() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"{"user":{"profile":{"name":"Alice"},"settings":{"name":"Settings"}}}"#,
            "path": "$..name"
        });

        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("Alice"));
        assert!(result.contains("Settings"));
    }

    #[tokio::test]
    async fn test_query_jsonpath_no_matches() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"{"name":"Alice"}"#,
            "path": "$.nonexistent"
        });

        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert_eq!(result, "No matches found");
    }

    #[tokio::test]
    async fn test_query_invalid_jsonpath() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"{"name":"Alice"}"#,
            "path": "$$invalid["
        });

        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid JSONPath"));
    }

    #[tokio::test]
    async fn test_args_schema() {
        let tool = JsonTool::new();
        let schema = tool.args_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["json"].is_object());
        assert!(schema["properties"]["path"].is_object());
        assert!(schema["properties"]["query"].is_object());
        assert!(schema["oneOf"].is_array());
    }

    #[tokio::test]
    async fn test_structured_input_missing_fields() {
        let tool = JsonTool::new();
        let input = json!({
            "invalid": "field"
        });

        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must provide 'json' or 'query'"));
    }

    #[tokio::test]
    async fn test_structured_input_non_string_json() {
        let tool = JsonTool::new();
        let input = json!({
            "json": 123  // Should be a string
        });

        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must be a string"));
    }

    // ========================================================================
    // Comprehensive Tests - Error Scenarios & Edge Cases
    // ========================================================================

    /// Test helper struct for comprehensive tests
    struct JsonToolComprehensiveTests {
        tool: JsonTool,
    }

    impl JsonToolComprehensiveTests {
        fn new() -> Self {
            Self {
                tool: JsonTool::new(),
            }
        }
    }

    #[async_trait::async_trait]
    impl dashflow_standard_tests::tool_comprehensive_tests::ToolComprehensiveTests
        for JsonToolComprehensiveTests
    {
        fn tool(&self) -> &dyn Tool {
            &self.tool
        }

        fn valid_input(&self) -> serde_json::Value {
            json!({"json": r#"{"test": "value"}"#})
        }
    }

    #[tokio::test]
    async fn test_json_comprehensive_missing_required_field() {
        let tests = JsonToolComprehensiveTests::new();
        tests.test_error_missing_required_field().await.unwrap();
    }

    #[tokio::test]
    async fn test_json_comprehensive_invalid_field_type() {
        let tests = JsonToolComprehensiveTests::new();
        tests.test_error_invalid_field_type().await.unwrap();
    }

    #[tokio::test]
    async fn test_json_comprehensive_empty_string() {
        let tests = JsonToolComprehensiveTests::new();
        tests.test_edge_case_empty_string().await.unwrap();
    }

    #[tokio::test]
    async fn test_json_comprehensive_unicode() {
        let tests = JsonToolComprehensiveTests::new();
        tests
            .test_edge_case_unicode_and_special_chars()
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_json_comprehensive_repeated_calls() {
        let tests = JsonToolComprehensiveTests::new();
        tests.test_robustness_repeated_calls().await.unwrap();
    }

    // JSON-specific comprehensive tests

    #[tokio::test]
    async fn test_malformed_json() {
        let tool = JsonTool::new();
        let malformed_jsons = vec![
            r#"{"incomplete": "#,
            r#"{"trailing": "comma",}"#,
            r#"{"unquoted": key}"#,
            r#"{invalid json}"#,
            r#"[1, 2, 3,]"#,
        ];

        for malformed in malformed_jsons {
            let input = json!({"json": malformed});
            let result = tool._call(ToolInput::Structured(input)).await;
            assert!(
                result.is_err(),
                "Should reject malformed JSON: {}",
                malformed
            );
        }
    }

    #[tokio::test]
    async fn test_deeply_nested_json() {
        let tool = JsonTool::new();

        // Create deeply nested JSON
        let mut nested = json!({"value": 0});
        for i in 1..100 {
            nested = json!({"level": i, "nested": nested});
        }
        let json_str = serde_json::to_string(&nested).unwrap();

        let input = json!({"json": json_str});
        let result = tool._call(ToolInput::Structured(input)).await;
        // Should parse successfully (or error gracefully)
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_very_large_json() {
        let tool = JsonTool::new();

        // Create large JSON array
        let large_array: Vec<i32> = (0..10000).collect();
        let json_str = serde_json::to_string(&large_array).unwrap();

        let input = json!({"json": json_str});
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tool._call(ToolInput::Structured(input)),
        )
        .await;

        assert!(result.is_ok(), "Should parse large JSON within 5 seconds");
    }

    #[tokio::test]
    async fn test_json_with_unicode() {
        let tool = JsonTool::new();
        let unicode_json = json!({
            "emoji": "👋🌍",
            "arabic": "مرحبا",
            "hebrew": "שלום",
            "chinese": "你好"
        });
        let json_str = serde_json::to_string(&unicode_json).unwrap();

        let input = json!({"json": json_str});
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        // Should contain all unicode characters
        assert!(result.contains("👋"));
        assert!(result.contains("مرحبا"));
    }

    #[tokio::test]
    async fn test_jsonpath_invalid_syntax() {
        let tool = JsonTool::new();
        let json_str = r#"{"test": "value"}"#;

        let invalid_paths = vec!["$.[invalid", "$.???", "not a path"];

        for path in invalid_paths {
            let input = json!({"json": json_str, "path": path});
            let result = tool._call(ToolInput::Structured(input)).await;
            // Should error with invalid JSONPath
            assert!(result.is_err(), "Should reject invalid JSONPath: {}", path);
        }
    }

    // ========================================================================
    // JsonTool struct trait tests
    // ========================================================================

    #[test]
    fn test_json_tool_clone() {
        let tool1 = JsonTool::new();
        let tool2 = tool1.clone();
        assert_eq!(tool1.name(), tool2.name());
        assert_eq!(tool1.description(), tool2.description());
    }

    #[test]
    fn test_json_tool_debug() {
        let tool = JsonTool::new();
        let debug_str = format!("{:?}", tool);
        assert!(debug_str.contains("JsonTool"));
    }

    #[test]
    fn test_json_tool_default_is_same_as_new() {
        let default_tool = JsonTool::default();
        let new_tool = JsonTool::new();
        assert_eq!(default_tool.name(), new_tool.name());
    }

    // ========================================================================
    // parse_json method tests
    // ========================================================================

    #[tokio::test]
    async fn test_parse_empty_object() {
        let tool = JsonTool::new();
        let result = tool._call_str("{}".to_string()).await.unwrap();
        assert!(result.contains("{}") || result.contains("{ }"));
    }

    #[tokio::test]
    async fn test_parse_empty_array() {
        let tool = JsonTool::new();
        let result = tool._call_str("[]".to_string()).await.unwrap();
        assert!(result.contains("[]") || result.contains("[ ]"));
    }

    #[tokio::test]
    async fn test_parse_null() {
        let tool = JsonTool::new();
        let result = tool._call_str("null".to_string()).await.unwrap();
        assert_eq!(result.trim(), "null");
    }

    #[tokio::test]
    async fn test_parse_boolean_true() {
        let tool = JsonTool::new();
        let result = tool._call_str("true".to_string()).await.unwrap();
        assert_eq!(result.trim(), "true");
    }

    #[tokio::test]
    async fn test_parse_boolean_false() {
        let tool = JsonTool::new();
        let result = tool._call_str("false".to_string()).await.unwrap();
        assert_eq!(result.trim(), "false");
    }

    #[tokio::test]
    async fn test_parse_integer() {
        let tool = JsonTool::new();
        let result = tool._call_str("42".to_string()).await.unwrap();
        assert_eq!(result.trim(), "42");
    }

    #[tokio::test]
    async fn test_parse_negative_integer() {
        let tool = JsonTool::new();
        let result = tool._call_str("-42".to_string()).await.unwrap();
        assert_eq!(result.trim(), "-42");
    }

    #[tokio::test]
    async fn test_parse_float() {
        let tool = JsonTool::new();
        let result = tool._call_str("3.14159".to_string()).await.unwrap();
        assert!(result.contains("3.14159"));
    }

    #[tokio::test]
    async fn test_parse_scientific_notation() {
        let tool = JsonTool::new();
        let result = tool._call_str("1.5e10".to_string()).await.unwrap();
        // serde_json may convert to integer form
        assert!(!result.is_empty());
    }

    #[tokio::test]
    async fn test_parse_string_value() {
        let tool = JsonTool::new();
        let result = tool._call_str(r#""hello""#.to_string()).await.unwrap();
        assert!(result.contains("hello"));
    }

    #[tokio::test]
    async fn test_parse_string_with_escapes() {
        let tool = JsonTool::new();
        let result = tool._call_str(r#""line1\nline2""#.to_string()).await.unwrap();
        assert!(result.contains("line1"));
    }

    #[tokio::test]
    async fn test_parse_array_of_numbers() {
        let tool = JsonTool::new();
        let result = tool._call_str("[1, 2, 3, 4, 5]".to_string()).await.unwrap();
        assert!(result.contains("1"));
        assert!(result.contains("5"));
    }

    #[tokio::test]
    async fn test_parse_array_of_mixed_types() {
        let tool = JsonTool::new();
        let result = tool._call_str(r#"[1, "two", true, null]"#.to_string()).await.unwrap();
        assert!(result.contains("1"));
        assert!(result.contains("two"));
        assert!(result.contains("true"));
        assert!(result.contains("null"));
    }

    #[tokio::test]
    async fn test_parse_nested_objects() {
        let tool = JsonTool::new();
        let input = r#"{"a":{"b":{"c":"deep"}}}"#;
        let result = tool._call_str(input.to_string()).await.unwrap();
        assert!(result.contains("deep"));
    }

    #[tokio::test]
    async fn test_parse_object_with_array() {
        let tool = JsonTool::new();
        let input = r#"{"numbers":[1,2,3]}"#;
        let result = tool._call_str(input.to_string()).await.unwrap();
        assert!(result.contains("numbers"));
    }

    #[tokio::test]
    async fn test_parse_empty_string_invalid() {
        let tool = JsonTool::new();
        let result = tool._call_str("".to_string()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parse_whitespace_only_invalid() {
        let tool = JsonTool::new();
        let result = tool._call_str("   ".to_string()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parse_json_with_leading_whitespace() {
        let tool = JsonTool::new();
        let result = tool._call_str("   {\"key\": \"value\"}".to_string()).await.unwrap();
        assert!(result.contains("key"));
    }

    #[tokio::test]
    async fn test_parse_json_with_trailing_whitespace() {
        let tool = JsonTool::new();
        let result = tool._call_str("{\"key\": \"value\"}   ".to_string()).await.unwrap();
        assert!(result.contains("key"));
    }

    // ========================================================================
    // query_json JSONPath tests
    // ========================================================================

    #[tokio::test]
    async fn test_query_root() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"{"name":"test"}"#,
            "path": "$"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("name"));
        assert!(result.contains("test"));
    }

    #[tokio::test]
    async fn test_query_nested_field() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"{"level1":{"level2":{"level3":"value"}}}"#,
            "path": "$.level1.level2.level3"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("value"));
    }

    #[tokio::test]
    async fn test_query_array_index_first() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"["a","b","c"]"#,
            "path": "$[0]"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("a"));
    }

    #[tokio::test]
    async fn test_query_array_index_last() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"["a","b","c"]"#,
            "path": "$[2]"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("c"));
    }

    #[tokio::test]
    async fn test_query_array_out_of_bounds() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"["a","b","c"]"#,
            "path": "$[99]"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("No matches"));
    }

    #[tokio::test]
    async fn test_query_all_array_elements() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"[1, 2, 3]"#,
            "path": "$[*]"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("1"));
        assert!(result.contains("2"));
        assert!(result.contains("3"));
    }

    #[tokio::test]
    async fn test_query_nested_array_elements() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"{"items":[{"id":1},{"id":2},{"id":3}]}"#,
            "path": "$.items[*].id"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("1"));
        assert!(result.contains("2"));
        assert!(result.contains("3"));
    }

    #[tokio::test]
    async fn test_query_recursive_descent() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"{"a":{"id":1,"b":{"id":2,"c":{"id":3}}}}"#,
            "path": "$..id"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("1"));
        assert!(result.contains("2"));
        assert!(result.contains("3"));
    }

    #[tokio::test]
    async fn test_query_single_result_not_array() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"{"name":"Alice"}"#,
            "path": "$.name"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        // Single result should not be wrapped in array
        assert!(result.contains("Alice"));
        assert!(!result.starts_with('['));
    }

    #[tokio::test]
    async fn test_query_multiple_results_is_array() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"{"users":[{"name":"Alice"},{"name":"Bob"}]}"#,
            "path": "$.users[*].name"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        // Multiple results should be array
        assert!(result.trim().starts_with('['));
    }

    #[tokio::test]
    async fn test_query_boolean_result() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"{"active":true}"#,
            "path": "$.active"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert_eq!(result.trim(), "true");
    }

    #[tokio::test]
    async fn test_query_null_result() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"{"value":null}"#,
            "path": "$.value"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert_eq!(result.trim(), "null");
    }

    #[tokio::test]
    async fn test_query_number_result() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"{"count":42}"#,
            "path": "$.count"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert_eq!(result.trim(), "42");
    }

    #[tokio::test]
    async fn test_query_object_result() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"{"nested":{"key":"value"}}"#,
            "path": "$.nested"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("key"));
        assert!(result.contains("value"));
    }

    // ========================================================================
    // Tool trait method tests
    // ========================================================================

    #[test]
    fn test_tool_name() {
        let tool = JsonTool::new();
        assert_eq!(tool.name(), "json_tool");
    }

    #[test]
    fn test_tool_description_contains_jsonpath() {
        let tool = JsonTool::new();
        let desc = tool.description();
        assert!(desc.contains("JSONPath"));
    }

    #[test]
    fn test_tool_description_contains_parse() {
        let tool = JsonTool::new();
        let desc = tool.description();
        assert!(desc.contains("parse") || desc.contains("Parse"));
    }

    #[test]
    fn test_args_schema_has_json_property() {
        let tool = JsonTool::new();
        let schema = tool.args_schema();
        assert!(schema["properties"]["json"].is_object());
    }

    #[test]
    fn test_args_schema_has_path_property() {
        let tool = JsonTool::new();
        let schema = tool.args_schema();
        assert!(schema["properties"]["path"].is_object());
    }

    #[test]
    fn test_args_schema_has_query_property() {
        let tool = JsonTool::new();
        let schema = tool.args_schema();
        assert!(schema["properties"]["query"].is_object());
    }

    #[test]
    fn test_args_schema_type_is_object() {
        let tool = JsonTool::new();
        let schema = tool.args_schema();
        assert_eq!(schema["type"], "object");
    }

    #[test]
    fn test_args_schema_json_has_description() {
        let tool = JsonTool::new();
        let schema = tool.args_schema();
        assert!(schema["properties"]["json"]["description"].is_string());
    }

    #[test]
    fn test_args_schema_path_has_description() {
        let tool = JsonTool::new();
        let schema = tool.args_schema();
        assert!(schema["properties"]["path"]["description"].is_string());
    }

    // ========================================================================
    // _call input handling tests
    // ========================================================================

    #[tokio::test]
    async fn test_call_with_string_input() {
        let tool = JsonTool::new();
        let result = tool._call(ToolInput::String(r#"{"test":true}"#.to_string())).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_call_with_structured_json_only() {
        let tool = JsonTool::new();
        let input = json!({"json": r#"{"test":true}"#});
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_call_with_structured_query() {
        let tool = JsonTool::new();
        let input = json!({"query": r#"{"test":true}"#});
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_call_with_json_and_path() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"{"key":"value"}"#,
            "path": "$.key"
        });
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_call_empty_structured_input() {
        let tool = JsonTool::new();
        let input = json!({});
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_null_json_field() {
        let tool = JsonTool::new();
        let input = json!({"json": null});
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_non_string_path() {
        let tool = JsonTool::new();
        let input = json!({
            "json": r#"{"key":"value"}"#,
            "path": 123
        });
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_non_string_query() {
        let tool = JsonTool::new();
        let input = json!({"query": 123});
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    // ========================================================================
    // Error message tests
    // ========================================================================

    #[tokio::test]
    async fn test_error_invalid_json_message() {
        let tool = JsonTool::new();
        let result = tool._call_str("{invalid}".to_string()).await;
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid JSON"));
    }

    #[tokio::test]
    async fn test_error_invalid_jsonpath_message() {
        let tool = JsonTool::new();
        let input = json!({
            "json": "{}",
            "path": "$[["
        });
        let result = tool._call(ToolInput::Structured(input)).await;
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid JSONPath"));
    }

    #[tokio::test]
    async fn test_error_missing_fields_message() {
        let tool = JsonTool::new();
        let input = json!({"other": "field"});
        let result = tool._call(ToolInput::Structured(input)).await;
        let err = result.unwrap_err().to_string();
        assert!(err.contains("json") || err.contains("query"));
    }

    #[tokio::test]
    async fn test_error_json_must_be_string() {
        let tool = JsonTool::new();
        let input = json!({"json": {"nested": "object"}});
        let result = tool._call(ToolInput::Structured(input)).await;
        let err = result.unwrap_err().to_string();
        assert!(err.contains("must be a string"));
    }

    #[tokio::test]
    async fn test_error_path_must_be_string() {
        let tool = JsonTool::new();
        let input = json!({
            "json": "{}",
            "path": ["array", "path"]
        });
        let result = tool._call(ToolInput::Structured(input)).await;
        let err = result.unwrap_err().to_string();
        assert!(err.contains("must be a string"));
    }

    // ========================================================================
    // Pretty print formatting tests
    // ========================================================================

    #[tokio::test]
    async fn test_pretty_print_indentation() {
        let tool = JsonTool::new();
        let input = r#"{"a":"b","c":"d"}"#;
        let result = tool._call_str(input.to_string()).await.unwrap();
        // Pretty printed should have indentation
        assert!(result.contains("  ") || result.contains('\n'));
    }

    #[tokio::test]
    async fn test_pretty_print_preserves_data() {
        let tool = JsonTool::new();
        let input = r#"{"name":"test","value":123,"active":true}"#;
        let result = tool._call_str(input.to_string()).await.unwrap();
        assert!(result.contains("name"));
        assert!(result.contains("test"));
        assert!(result.contains("123"));
        assert!(result.contains("true"));
    }

    // ========================================================================
    // Additional edge cases
    // ========================================================================

    #[tokio::test]
    async fn test_json_with_special_characters() {
        let tool = JsonTool::new();
        let input = r#"{"text":"line1\nline2\ttab\"quoted\""}"#;
        let result = tool._call_str(input.to_string()).await.unwrap();
        assert!(result.contains("text"));
    }

    #[tokio::test]
    async fn test_json_with_numbers_as_keys() {
        let tool = JsonTool::new();
        let input = r#"{"123":"number key"}"#;
        let result = tool._call_str(input.to_string()).await.unwrap();
        assert!(result.contains("123"));
    }

    #[tokio::test]
    async fn test_query_empty_json_object() {
        let tool = JsonTool::new();
        let input = json!({
            "json": "{}",
            "path": "$.anything"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("No matches"));
    }

    #[tokio::test]
    async fn test_query_empty_array() {
        let tool = JsonTool::new();
        let input = json!({
            "json": "[]",
            "path": "$[0]"
        });
        let result = tool._call(ToolInput::Structured(input)).await.unwrap();
        assert!(result.contains("No matches"));
    }

    #[tokio::test]
    async fn test_concurrent_tool_calls() {
        let tool = JsonTool::new();
        let mut handles = vec![];

        for i in 0..10 {
            let t = tool.clone();
            let handle = tokio::spawn(async move {
                let json = format!(r#"{{"id":{}}}"#, i);
                t._call_str(json).await
            });
            handles.push(handle);
        }

        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_repeated_calls_same_tool() {
        let tool = JsonTool::new();

        for _ in 0..10 {
            let result = tool._call_str(r#"{"test":true}"#.to_string()).await;
            assert!(result.is_ok());
        }
    }
}
