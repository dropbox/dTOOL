//! JSON toolkit for bundling JSON tools

use crate::spec::JsonSpec;
use crate::tools::{JsonGetValueTool, JsonListKeysTool};
use dashflow::core::tools::{BaseToolkit, Tool};
use std::sync::Arc;

/// Toolkit for interacting with JSON data structures.
///
/// `JsonToolkit` bundles JSON navigation tools that allow agents to explore
/// and query JSON documents by listing keys and retrieving values.
///
/// # Tools Included
///
/// 1. **`JsonListKeysTool`** - Lists keys at a given path
/// 2. **`JsonGetValueTool`** - Retrieves values at a given path
///
/// # Examples
///
/// ```rust
/// use dashflow_json::{JsonSpec, JsonToolkit};
/// use dashflow::core::tools::BaseToolkit;
/// use serde_json::json;
///
/// # #[tokio::main]
/// # async fn main() {
/// let data = json!({
///     "users": [
///         {"name": "Alice", "age": 30},
///         {"name": "Bob", "age": 25}
///     ]
/// });
///
/// let spec = JsonSpec::new(data);
/// let toolkit = JsonToolkit::new(spec);
///
/// // Get tools for use with an agent
/// let tools = toolkit.get_tools();
/// assert_eq!(tools.len(), 2);
/// # }
/// ```
///
/// # Integration with Agents
///
/// ```rust
/// use dashflow_json::{JsonSpec, JsonToolkit};
/// use dashflow::core::tools::BaseToolkit;
/// use serde_json::json;
///
/// # #[tokio::main]
/// # async fn main() {
/// // Load JSON data
/// let data = json!({
///     "products": [
///         {"id": 1, "name": "Widget", "price": 19.99},
///         {"id": 2, "name": "Gadget", "price": 29.99}
///     ]
/// });
///
/// // Create toolkit
/// let spec = JsonSpec::new(data);
/// let toolkit = JsonToolkit::new(spec);
///
/// // Use with agent (pseudo-code)
/// // let agent = AgentExecutor::new(llm, toolkit.get_tools());
/// // let result = agent.run("What's the price of the Widget?").await;
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct JsonToolkit {
    spec: JsonSpec,
}

impl JsonToolkit {
    /// Creates a new `JsonToolkit` with the given JSON spec.
    ///
    /// # Arguments
    ///
    /// * `spec` - The JSON specification to query
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dashflow_json::{JsonSpec, JsonToolkit};
    /// use serde_json::json;
    ///
    /// let spec = JsonSpec::new(json!({"key": "value"}));
    /// let toolkit = JsonToolkit::new(spec);
    /// ```
    #[must_use]
    pub fn new(spec: JsonSpec) -> Self {
        Self { spec }
    }
}

impl BaseToolkit for JsonToolkit {
    fn get_tools(&self) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(JsonListKeysTool::new(self.spec.clone())),
            Arc::new(JsonGetValueTool::new(self.spec.clone())),
        ]
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::redundant_clone)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_toolkit_creation() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        let toolkit = JsonToolkit::new(spec);

        let tools = toolkit.get_tools();
        assert_eq!(tools.len(), 2);
    }

    #[test]
    fn test_json_toolkit_tool_names() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        let toolkit = JsonToolkit::new(spec);

        let tools = toolkit.get_tools();
        let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();

        assert!(names.contains(&"json_spec_list_keys"));
        assert!(names.contains(&"json_spec_get_value"));
    }

    #[tokio::test]
    async fn test_json_toolkit_tools_functional() {
        let data = json!({
            "users": {"alice": {"age": 30}},
            "count": 1
        });
        let spec = JsonSpec::new(data);
        let toolkit = JsonToolkit::new(spec);

        let tools = toolkit.get_tools();

        // Find the list keys tool
        let list_tool = tools
            .iter()
            .find(|t| t.name() == "json_spec_list_keys")
            .unwrap();

        let result = list_tool._call_str("data".to_string()).await.unwrap();
        assert!(result.contains("'users'"));
        assert!(result.contains("'count'"));

        // Find the get value tool
        let get_tool = tools
            .iter()
            .find(|t| t.name() == "json_spec_get_value")
            .unwrap();

        let result = get_tool
            ._call_str("data[\"count\"]".to_string())
            .await
            .unwrap();
        assert_eq!(result, "1");
    }

    #[test]
    fn test_json_toolkit_clone() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        let toolkit = JsonToolkit::new(spec);

        let _toolkit2 = toolkit.clone();
        // Should compile and work
    }

    #[test]
    fn test_json_toolkit_base_toolkit_trait() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);

        // Verify JsonToolkit implements BaseToolkit
        let toolkit: Box<dyn BaseToolkit> = Box::new(JsonToolkit::new(spec));
        let tools = toolkit.get_tools();
        assert_eq!(tools.len(), 2);
    }

    // ========== Additional toolkit tests ==========

    #[test]
    fn test_json_toolkit_debug() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        let toolkit = JsonToolkit::new(spec);

        let debug_str = format!("{:?}", toolkit);
        assert!(debug_str.contains("JsonToolkit"));
        assert!(debug_str.contains("spec"));
    }

    #[test]
    fn test_json_toolkit_with_empty_json() {
        let spec = JsonSpec::new(json!({}));
        let toolkit = JsonToolkit::new(spec);

        let tools = toolkit.get_tools();
        assert_eq!(tools.len(), 2);
    }

    #[test]
    fn test_json_toolkit_with_complex_json() {
        let data = json!({
            "users": [
                {"id": 1, "name": "Alice", "roles": ["admin", "user"]},
                {"id": 2, "name": "Bob", "roles": ["user"]}
            ],
            "metadata": {
                "version": "1.0.0",
                "created": "2024-01-01",
                "settings": {
                    "theme": "dark",
                    "notifications": true
                }
            },
            "tags": ["important", "featured", "public"]
        });
        let spec = JsonSpec::new(data);
        let toolkit = JsonToolkit::new(spec);

        let tools = toolkit.get_tools();
        assert_eq!(tools.len(), 2);

        // Verify tool names
        let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        assert!(names.contains(&"json_spec_list_keys"));
        assert!(names.contains(&"json_spec_get_value"));
    }

    #[tokio::test]
    async fn test_json_toolkit_tools_deep_navigation() {
        let data = json!({
            "level1": {
                "level2": {
                    "level3": {
                        "value": "found"
                    }
                }
            }
        });
        let spec = JsonSpec::new(data);
        let toolkit = JsonToolkit::new(spec);

        let tools = toolkit.get_tools();
        let get_tool = tools
            .iter()
            .find(|t| t.name() == "json_spec_get_value")
            .unwrap();

        let result = get_tool
            ._call_str("data[\"level1\"][\"level2\"][\"level3\"][\"value\"]".to_string())
            .await
            .unwrap();
        assert_eq!(result, "found");
    }

    #[tokio::test]
    async fn test_json_toolkit_tools_array_navigation() {
        let data = json!({
            "items": [
                {"name": "first"},
                {"name": "second"},
                {"name": "third"}
            ]
        });
        let spec = JsonSpec::new(data);
        let toolkit = JsonToolkit::new(spec);

        let tools = toolkit.get_tools();
        let get_tool = tools
            .iter()
            .find(|t| t.name() == "json_spec_get_value")
            .unwrap();

        let result = get_tool
            ._call_str("data[\"items\"][2][\"name\"]".to_string())
            .await
            .unwrap();
        assert_eq!(result, "third");
    }

    #[test]
    fn test_json_toolkit_tools_return_new_instances() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        let toolkit = JsonToolkit::new(spec);

        // Each call should return tools backed by the same spec
        let tools1 = toolkit.get_tools();
        let tools2 = toolkit.get_tools();

        assert_eq!(tools1.len(), tools2.len());
    }

    #[test]
    fn test_json_toolkit_clone_independence() {
        let data = json!({"original": true});
        let spec = JsonSpec::new(data);
        let toolkit1 = JsonToolkit::new(spec);
        let toolkit2 = toolkit1.clone();

        // Both should work independently
        let tools1 = toolkit1.get_tools();
        let tools2 = toolkit2.get_tools();

        assert_eq!(tools1.len(), 2);
        assert_eq!(tools2.len(), 2);
    }

    #[test]
    fn test_json_toolkit_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<JsonToolkit>();
    }

    #[tokio::test]
    async fn test_json_toolkit_list_keys_nested() {
        let data = json!({
            "root": {
                "child1": {},
                "child2": {},
                "child3": {}
            }
        });
        let spec = JsonSpec::new(data);
        let toolkit = JsonToolkit::new(spec);

        let tools = toolkit.get_tools();
        let list_tool = tools
            .iter()
            .find(|t| t.name() == "json_spec_list_keys")
            .unwrap();

        let result = list_tool
            ._call_str("data[\"root\"]".to_string())
            .await
            .unwrap();
        assert!(result.contains("'child1'"));
        assert!(result.contains("'child2'"));
        assert!(result.contains("'child3'"));
    }

    #[tokio::test]
    async fn test_json_toolkit_get_value_types() {
        let data = json!({
            "string": "hello",
            "number": 42,
            "float": 3.14,
            "bool": true,
            "null": null,
            "array": [1, 2, 3]
        });
        let spec = JsonSpec::new(data);
        let toolkit = JsonToolkit::new(spec);

        let tools = toolkit.get_tools();
        let get_tool = tools
            .iter()
            .find(|t| t.name() == "json_spec_get_value")
            .unwrap();

        // Test string
        let result = get_tool
            ._call_str("data[\"string\"]".to_string())
            .await
            .unwrap();
        assert_eq!(result, "hello");

        // Test number
        let result = get_tool
            ._call_str("data[\"number\"]".to_string())
            .await
            .unwrap();
        assert_eq!(result, "42");

        // Test bool
        let result = get_tool
            ._call_str("data[\"bool\"]".to_string())
            .await
            .unwrap();
        assert_eq!(result, "true");

        // Test null
        let result = get_tool
            ._call_str("data[\"null\"]".to_string())
            .await
            .unwrap();
        assert_eq!(result, "null");
    }

    #[test]
    fn test_json_toolkit_with_unicode_data() {
        let data = json!({
            "日本語": "こんにちは",
            "emoji": "🎉🎊🎁"
        });
        let spec = JsonSpec::new(data);
        let toolkit = JsonToolkit::new(spec);

        let tools = toolkit.get_tools();
        assert_eq!(tools.len(), 2);
    }

    #[tokio::test]
    async fn test_json_toolkit_error_handling() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        let toolkit = JsonToolkit::new(spec);

        let tools = toolkit.get_tools();
        let get_tool = tools
            .iter()
            .find(|t| t.name() == "json_spec_get_value")
            .unwrap();

        // Missing key should return error message (not panic)
        let result = get_tool
            ._call_str("data[\"missing\"]".to_string())
            .await
            .unwrap();
        assert!(result.contains("not found"));
    }

    #[test]
    fn test_json_toolkit_tool_descriptions() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        let toolkit = JsonToolkit::new(spec);

        let tools = toolkit.get_tools();

        for tool in tools {
            // Every tool should have a non-empty description
            assert!(!tool.description().is_empty());
            // Description should mention path or data
            assert!(
                tool.description().contains("path")
                    || tool.description().contains("keys")
                    || tool.description().contains("value"),
                "Tool {} has unexpected description",
                tool.name()
            );
        }
    }

    #[test]
    fn test_json_toolkit_with_null_root() {
        let spec = JsonSpec::new(json!(null));
        let toolkit = JsonToolkit::new(spec);

        let tools = toolkit.get_tools();
        assert_eq!(tools.len(), 2);
    }

    #[test]
    fn test_json_toolkit_with_array_root() {
        let data = json!([1, 2, 3, {"nested": true}]);
        let spec = JsonSpec::new(data);
        let toolkit = JsonToolkit::new(spec);

        let tools = toolkit.get_tools();
        assert_eq!(tools.len(), 2);
    }

    #[tokio::test]
    async fn test_json_toolkit_concurrent_access() {
        use std::sync::Arc;

        let data = json!({
            "shared": "value"
        });
        let spec = JsonSpec::new(data);
        let toolkit = Arc::new(JsonToolkit::new(spec));

        let mut handles = vec![];

        for _ in 0..10 {
            let tk = Arc::clone(&toolkit);
            let handle = tokio::spawn(async move {
                let tools = tk.get_tools();
                let get_tool = tools
                    .iter()
                    .find(|t| t.name() == "json_spec_get_value")
                    .unwrap();
                get_tool
                    ._call_str("data[\"shared\"]".to_string())
                    .await
                    .unwrap()
            });
            handles.push(handle);
        }

        for handle in handles {
            let result = handle.await.unwrap();
            assert_eq!(result, "value");
        }
    }
}
