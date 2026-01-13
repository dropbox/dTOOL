//! JSON tools for listing keys and retrieving values

use crate::spec::JsonSpec;
use async_trait::async_trait;
use dashflow::core::error::Result;
use dashflow::core::tools::{Tool, ToolInput};

/// Tool for listing keys in a JSON spec at a given path.
///
/// This tool allows agents to explore JSON data structures by listing
/// the keys available at any path in the JSON document.
///
/// # Examples
///
/// ```rust
/// use dashflow_json::{JsonSpec, JsonListKeysTool};
/// use dashflow::core::tools::Tool;
/// use serde_json::json;
///
/// # #[tokio::main]
/// # async fn main() {
/// let data = json!({
///     "users": {"alice": {}, "bob": {}},
///     "metadata": {}
/// });
/// let spec = JsonSpec::new(data);
/// let tool = JsonListKeysTool::new(spec);
///
/// let result = tool._call_str("data[\"users\"]".to_string()).await.unwrap();
/// // Result contains: "['alice', 'bob']"
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct JsonListKeysTool {
    spec: JsonSpec,
}

impl JsonListKeysTool {
    /// Creates a new `JsonListKeysTool` with the given JSON spec.
    ///
    /// # Arguments
    ///
    /// * `spec` - The JSON specification to query
    #[must_use]
    pub fn new(spec: JsonSpec) -> Self {
        Self { spec }
    }
}

#[async_trait]
impl Tool for JsonListKeysTool {
    fn name(&self) -> &'static str {
        "json_spec_list_keys"
    }

    fn description(&self) -> &'static str {
        r#"Can be used to list all keys at a given path.
Before calling this you should be SURE that the path to this exists.
The input is a text representation of the path to the dict in Python syntax (e.g. data["key1"][0]["key2"])."#
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let path = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(v) => {
                // Try to extract "input" field from structured input
                v.get("input")
                    .and_then(|v| v.as_str())
                    .unwrap_or("data")
                    .to_string()
            }
        };
        Ok(self.spec.keys(&path))
    }
}

/// Tool for getting a value in a JSON spec at a given path.
///
/// This tool allows agents to retrieve values from JSON data structures
/// using Python-style path expressions.
///
/// # Examples
///
/// ```rust
/// use dashflow_json::{JsonSpec, JsonGetValueTool};
/// use dashflow::core::tools::Tool;
/// use serde_json::json;
///
/// # #[tokio::main]
/// # async fn main() {
/// let data = json!({
///     "users": [
///         {"name": "Alice", "age": 30}
///     ]
/// });
/// let spec = JsonSpec::new(data);
/// let tool = JsonGetValueTool::new(spec);
///
/// let result = tool._call_str("data[\"users\"][0][\"name\"]".to_string()).await.unwrap();
/// assert_eq!(result, "Alice");
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct JsonGetValueTool {
    spec: JsonSpec,
}

impl JsonGetValueTool {
    /// Creates a new `JsonGetValueTool` with the given JSON spec.
    ///
    /// # Arguments
    ///
    /// * `spec` - The JSON specification to query
    #[must_use]
    pub fn new(spec: JsonSpec) -> Self {
        Self { spec }
    }
}

#[async_trait]
impl Tool for JsonGetValueTool {
    fn name(&self) -> &'static str {
        "json_spec_get_value"
    }

    fn description(&self) -> &'static str {
        r#"Can be used to see value in string format at a given path.
Before calling this you should be SURE that the path to this exists.
The input is a text representation of the path to the dict in Python syntax (e.g. data["key1"][0]["key2"])."#
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let path = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(v) => {
                // Try to extract "input" field from structured input
                v.get("input")
                    .and_then(|v| v.as_str())
                    .unwrap_or("data")
                    .to_string()
            }
        };
        Ok(self.spec.value(&path))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::redundant_clone)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_json_list_keys_tool() {
        let data = json!({
            "users": {"alice": {}, "bob": {}},
            "metadata": {}
        });
        let spec = JsonSpec::new(data);
        let tool = JsonListKeysTool::new(spec);

        // Test name and description
        assert_eq!(tool.name(), "json_spec_list_keys");
        assert!(tool.description().contains("list all keys"));

        // Test root level
        let result = tool._call_str("data".to_string()).await.unwrap();
        assert!(result.contains("'users'"));
        assert!(result.contains("'metadata'"));

        // Test nested level
        let result = tool._call_str("data[\"users\"]".to_string()).await.unwrap();
        assert!(result.contains("'alice'"));
        assert!(result.contains("'bob'"));
    }

    #[tokio::test]
    async fn test_json_list_keys_tool_error() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        let tool = JsonListKeysTool::new(spec);

        // Try to list keys on a non-dict value
        let result = tool._call_str("data[\"key\"]".to_string()).await.unwrap();
        assert!(result.contains("not a dict"));
    }

    #[tokio::test]
    async fn test_json_get_value_tool() {
        let data = json!({
            "users": [
                {"name": "Alice", "age": 30},
                {"name": "Bob", "age": 25}
            ],
            "count": 2
        });
        let spec = JsonSpec::new(data);
        let tool = JsonGetValueTool::new(spec);

        // Test name and description
        assert_eq!(tool.name(), "json_spec_get_value");
        assert!(tool.description().contains("value in string format"));

        // Test simple value
        let result = tool._call_str("data[\"count\"]".to_string()).await.unwrap();
        assert_eq!(result, "2");

        // Test nested value
        let result = tool
            ._call_str("data[\"users\"][0][\"name\"]".to_string())
            .await
            .unwrap();
        assert_eq!(result, "Alice");

        // Test array index
        let result = tool
            ._call_str("data[\"users\"][1][\"age\"]".to_string())
            .await
            .unwrap();
        assert_eq!(result, "25");
    }

    #[tokio::test]
    async fn test_json_get_value_tool_error() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        let tool = JsonGetValueTool::new(spec);

        // Try to get a missing key
        let result = tool
            ._call_str("data[\"missing\"]".to_string())
            .await
            .unwrap();
        assert!(result.contains("not found"));
    }

    #[tokio::test]
    async fn test_json_tools_clone() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);

        // Test that tools can be cloned
        let list_tool = JsonListKeysTool::new(spec.clone());
        let _list_tool2 = list_tool.clone();

        let get_tool = JsonGetValueTool::new(spec);
        let _get_tool2 = get_tool.clone();
    }

    #[tokio::test]
    async fn test_json_tools_as_arc() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);

        // Test that tools can be used as Arc<dyn Tool>
        let list_tool: Arc<dyn Tool> = Arc::new(JsonListKeysTool::new(spec.clone()));
        let result = list_tool._call_str("data".to_string()).await.unwrap();
        assert!(result.contains("'key'"));

        let get_tool: Arc<dyn Tool> = Arc::new(JsonGetValueTool::new(spec));
        let result = get_tool
            ._call_str("data[\"key\"]".to_string())
            .await
            .unwrap();
        assert_eq!(result, "value");
    }

    // ========== Structured input tests ==========

    #[tokio::test]
    async fn test_json_list_keys_structured_input() {
        let data = json!({
            "users": {"alice": {}, "bob": {}}
        });
        let spec = JsonSpec::new(data);
        let tool = JsonListKeysTool::new(spec);

        // Test structured input with "input" field
        let input = ToolInput::Structured(json!({"input": "data[\"users\"]"}));
        let result = tool._call(input).await.unwrap();
        assert!(result.contains("'alice'"));
        assert!(result.contains("'bob'"));
    }

    #[tokio::test]
    async fn test_json_list_keys_structured_input_fallback() {
        let data = json!({
            "root": {"key": "value"}
        });
        let spec = JsonSpec::new(data);
        let tool = JsonListKeysTool::new(spec);

        // Test structured input WITHOUT "input" field - should fall back to "data"
        let input = ToolInput::Structured(json!({"other_field": "ignored"}));
        let result = tool._call(input).await.unwrap();
        // Falls back to "data", which lists root keys
        assert!(result.contains("'root'"));
    }

    #[tokio::test]
    async fn test_json_get_value_structured_input() {
        let data = json!({
            "config": {"host": "localhost", "port": 8080}
        });
        let spec = JsonSpec::new(data);
        let tool = JsonGetValueTool::new(spec);

        // Test structured input with "input" field
        let input = ToolInput::Structured(json!({"input": "data[\"config\"][\"host\"]"}));
        let result = tool._call(input).await.unwrap();
        assert_eq!(result, "localhost");
    }

    #[tokio::test]
    async fn test_json_get_value_structured_input_fallback() {
        let data = json!({
            "key": "value"
        });
        let spec = JsonSpec::new(data);
        let tool = JsonGetValueTool::new(spec);

        // Test structured input WITHOUT "input" field - should fall back to "data"
        let input = ToolInput::Structured(json!({"path": "data[\"key\"]"}));
        let result = tool._call(input).await.unwrap();
        // Falls back to "data", which returns the whole object
        assert!(result.contains("key"));
    }

    #[tokio::test]
    async fn test_json_get_value_structured_input_null() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        let tool = JsonGetValueTool::new(spec);

        // Test structured input with null "input" field
        let input = ToolInput::Structured(json!({"input": null}));
        let result = tool._call(input).await.unwrap();
        // Should fall back to "data" since null is not a string
        assert!(result.contains("key"));
    }

    #[tokio::test]
    async fn test_json_list_keys_structured_input_empty_object() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        let tool = JsonListKeysTool::new(spec);

        // Empty structured input - should fall back to "data"
        let input = ToolInput::Structured(json!({}));
        let result = tool._call(input).await.unwrap();
        assert!(result.contains("'key'"));
    }

    // ========== Debug trait tests ==========

    #[test]
    fn test_json_list_keys_tool_debug() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        let tool = JsonListKeysTool::new(spec);

        let debug_str = format!("{:?}", tool);
        assert!(debug_str.contains("JsonListKeysTool"));
        assert!(debug_str.contains("spec"));
    }

    #[test]
    fn test_json_get_value_tool_debug() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        let tool = JsonGetValueTool::new(spec);

        let debug_str = format!("{:?}", tool);
        assert!(debug_str.contains("JsonGetValueTool"));
        assert!(debug_str.contains("spec"));
    }

    // ========== Send + Sync trait bounds ==========

    #[test]
    fn test_json_list_keys_tool_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<JsonListKeysTool>();
    }

    #[test]
    fn test_json_get_value_tool_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<JsonGetValueTool>();
    }

    // ========== Edge cases ==========

    #[tokio::test]
    async fn test_json_list_keys_empty_object() {
        let spec = JsonSpec::new(json!({}));
        let tool = JsonListKeysTool::new(spec);

        let result = tool._call_str("data".to_string()).await.unwrap();
        assert_eq!(result, "[]");
    }

    #[tokio::test]
    async fn test_json_get_value_empty_string() {
        let data = json!({"key": ""});
        let spec = JsonSpec::new(data);
        let tool = JsonGetValueTool::new(spec);

        let result = tool._call_str("data[\"key\"]".to_string()).await.unwrap();
        assert_eq!(result, "");
    }

    #[tokio::test]
    async fn test_json_list_keys_deep_nesting() {
        let data = json!({
            "a": {
                "b": {
                    "c": {
                        "d": {}
                    }
                }
            }
        });
        let spec = JsonSpec::new(data);
        let tool = JsonListKeysTool::new(spec);

        let result = tool
            ._call_str("data[\"a\"][\"b\"][\"c\"]".to_string())
            .await
            .unwrap();
        assert!(result.contains("'d'"));
    }

    #[tokio::test]
    async fn test_json_get_value_array_element() {
        let data = json!({
            "arr": [10, 20, 30, 40, 50]
        });
        let spec = JsonSpec::new(data);
        let tool = JsonGetValueTool::new(spec);

        let result = tool
            ._call_str("data[\"arr\"][2]".to_string())
            .await
            .unwrap();
        assert_eq!(result, "30");
    }

    #[tokio::test]
    async fn test_json_get_value_nested_array() {
        let data = json!({
            "matrix": [[1, 2], [3, 4], [5, 6]]
        });
        let spec = JsonSpec::new(data);
        let tool = JsonGetValueTool::new(spec);

        let result = tool
            ._call_str("data[\"matrix\"][1][1]".to_string())
            .await
            .unwrap();
        assert_eq!(result, "4");
    }

    #[tokio::test]
    async fn test_json_list_keys_unicode_keys() {
        let data = json!({
            "日本語": {},
            "中文": {},
            "한국어": {}
        });
        let spec = JsonSpec::new(data);
        let tool = JsonListKeysTool::new(spec);

        let result = tool._call_str("data".to_string()).await.unwrap();
        assert!(result.contains("'日本語'"));
        assert!(result.contains("'中文'"));
        assert!(result.contains("'한국어'"));
    }

    #[tokio::test]
    async fn test_json_get_value_unicode_value() {
        let data = json!({"greeting": "こんにちは"});
        let spec = JsonSpec::new(data);
        let tool = JsonGetValueTool::new(spec);

        let result = tool
            ._call_str("data[\"greeting\"]".to_string())
            .await
            .unwrap();
        assert_eq!(result, "こんにちは");
    }

    #[tokio::test]
    async fn test_json_list_keys_error_on_non_dict() {
        let data = json!({"arr": [1, 2, 3]});
        let spec = JsonSpec::new(data);
        let tool = JsonListKeysTool::new(spec);

        // Arrays don't have keys
        let result = tool
            ._call_str("data[\"arr\"]".to_string())
            .await
            .unwrap();
        assert!(result.contains("not a dict"));
    }

    #[tokio::test]
    async fn test_json_get_value_missing_nested_key() {
        let data = json!({"a": {"b": "c"}});
        let spec = JsonSpec::new(data);
        let tool = JsonGetValueTool::new(spec);

        let result = tool
            ._call_str("data[\"a\"][\"missing\"]".to_string())
            .await
            .unwrap();
        assert!(result.contains("not found"));
    }

    #[tokio::test]
    async fn test_json_get_value_index_out_of_bounds() {
        let data = json!({"arr": [1, 2, 3]});
        let spec = JsonSpec::new(data);
        let tool = JsonGetValueTool::new(spec);

        let result = tool
            ._call_str("data[\"arr\"][100]".to_string())
            .await
            .unwrap();
        assert!(result.contains("out of bounds"));
    }

    // ========== Tool name and description verification ==========

    #[test]
    fn test_json_list_keys_tool_name() {
        let spec = JsonSpec::new(json!({}));
        let tool = JsonListKeysTool::new(spec);

        assert_eq!(tool.name(), "json_spec_list_keys");
    }

    #[test]
    fn test_json_get_value_tool_name() {
        let spec = JsonSpec::new(json!({}));
        let tool = JsonGetValueTool::new(spec);

        assert_eq!(tool.name(), "json_spec_get_value");
    }

    #[test]
    fn test_json_list_keys_tool_description_content() {
        let spec = JsonSpec::new(json!({}));
        let tool = JsonListKeysTool::new(spec);

        let desc = tool.description();
        assert!(desc.contains("list"));
        assert!(desc.contains("keys"));
        assert!(desc.contains("path"));
    }

    #[test]
    fn test_json_get_value_tool_description_content() {
        let spec = JsonSpec::new(json!({}));
        let tool = JsonGetValueTool::new(spec);

        let desc = tool.description();
        assert!(desc.contains("value"));
        assert!(desc.contains("path"));
    }

    // ========== Boolean and null values ==========

    #[tokio::test]
    async fn test_json_get_value_boolean_true() {
        let data = json!({"flag": true});
        let spec = JsonSpec::new(data);
        let tool = JsonGetValueTool::new(spec);

        let result = tool._call_str("data[\"flag\"]".to_string()).await.unwrap();
        assert_eq!(result, "true");
    }

    #[tokio::test]
    async fn test_json_get_value_boolean_false() {
        let data = json!({"flag": false});
        let spec = JsonSpec::new(data);
        let tool = JsonGetValueTool::new(spec);

        let result = tool._call_str("data[\"flag\"]".to_string()).await.unwrap();
        assert_eq!(result, "false");
    }

    #[tokio::test]
    async fn test_json_get_value_null() {
        let data = json!({"empty": null});
        let spec = JsonSpec::new(data);
        let tool = JsonGetValueTool::new(spec);

        let result = tool
            ._call_str("data[\"empty\"]".to_string())
            .await
            .unwrap();
        assert_eq!(result, "null");
    }

    // ========== Numbers ==========

    #[tokio::test]
    async fn test_json_get_value_integer() {
        let data = json!({"count": 42});
        let spec = JsonSpec::new(data);
        let tool = JsonGetValueTool::new(spec);

        let result = tool
            ._call_str("data[\"count\"]".to_string())
            .await
            .unwrap();
        assert_eq!(result, "42");
    }

    #[tokio::test]
    async fn test_json_get_value_float() {
        let data = json!({"pi": 3.14159});
        let spec = JsonSpec::new(data);
        let tool = JsonGetValueTool::new(spec);

        let result = tool._call_str("data[\"pi\"]".to_string()).await.unwrap();
        assert!(result.starts_with("3.14"));
    }

    #[tokio::test]
    async fn test_json_get_value_negative() {
        let data = json!({"temp": -10});
        let spec = JsonSpec::new(data);
        let tool = JsonGetValueTool::new(spec);

        let result = tool._call_str("data[\"temp\"]".to_string()).await.unwrap();
        assert_eq!(result, "-10");
    }

    // ========== Concurrent access ==========

    #[tokio::test]
    async fn test_json_tools_concurrent_calls() {
        let data = json!({
            "a": "1",
            "b": "2",
            "c": "3"
        });
        let spec = JsonSpec::new(data);
        let tool = Arc::new(JsonGetValueTool::new(spec));

        let t1 = Arc::clone(&tool);
        let t2 = Arc::clone(&tool);
        let t3 = Arc::clone(&tool);

        let (r1, r2, r3) = tokio::join!(
            t1._call_str("data[\"a\"]".to_string()),
            t2._call_str("data[\"b\"]".to_string()),
            t3._call_str("data[\"c\"]".to_string())
        );

        assert_eq!(r1.unwrap(), "1");
        assert_eq!(r2.unwrap(), "2");
        assert_eq!(r3.unwrap(), "3");
    }

    // ========== Path variations ==========

    #[tokio::test]
    async fn test_json_get_value_single_quotes() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        let tool = JsonGetValueTool::new(spec);

        // Single quotes should also work
        let result = tool._call_str("data['key']".to_string()).await.unwrap();
        assert_eq!(result, "value");
    }

    #[tokio::test]
    async fn test_json_get_value_mixed_quotes() {
        let data = json!({"a": {"b": "c"}});
        let spec = JsonSpec::new(data);
        let tool = JsonGetValueTool::new(spec);

        let result = tool
            ._call_str("data['a'][\"b\"]".to_string())
            .await
            .unwrap();
        assert_eq!(result, "c");
    }

    #[tokio::test]
    async fn test_json_list_keys_at_root() {
        let data = json!({
            "first": {},
            "second": {},
            "third": {}
        });
        let spec = JsonSpec::new(data);
        let tool = JsonListKeysTool::new(spec);

        // "data" should list root keys
        let result = tool._call_str("data".to_string()).await.unwrap();
        assert!(result.contains("'first'"));
        assert!(result.contains("'second'"));
        assert!(result.contains("'third'"));
    }

    #[tokio::test]
    async fn test_json_list_keys_empty_path() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        let tool = JsonListKeysTool::new(spec);

        // Empty path should list root keys
        let result = tool._call_str("".to_string()).await.unwrap();
        assert!(result.contains("'key'"));
    }

    // ========== Large data ==========

    #[tokio::test]
    async fn test_json_list_keys_many_keys() {
        let mut obj = serde_json::Map::new();
        for i in 0..50 {
            obj.insert(format!("key{}", i), json!(i));
        }
        let spec = JsonSpec::new(serde_json::Value::Object(obj));
        let tool = JsonListKeysTool::new(spec);

        let result = tool._call_str("data".to_string()).await.unwrap();
        for i in 0..50 {
            assert!(result.contains(&format!("'key{}'", i)));
        }
    }

    #[tokio::test]
    async fn test_json_get_value_truncation() {
        let long_value = "x".repeat(300);
        let data = json!({"long": long_value});
        let spec = JsonSpec::new(data);
        let tool = JsonGetValueTool::new(spec);

        let result = tool._call_str("data[\"long\"]".to_string()).await.unwrap();
        assert!(result.ends_with("..."));
        assert_eq!(result.len(), 203); // 200 + "..."
    }
}
