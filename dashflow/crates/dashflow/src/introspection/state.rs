//! Live State Querying
//!
//! This module provides the [`StateIntrospection`] trait and helper functions
//! for AI agents to query their state at runtime.

// Live State Querying
// ============================================================================

/// Trait for introspecting graph state at runtime
///
/// This trait enables AI agents to query their current state, examine fields,
/// and monitor memory usage. It is automatically implemented for any type that
/// implements `GraphState` (via Serialize/Deserialize).
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::introspection::StateIntrospection;
///
/// // AI checks its state
/// if state.has_field("pending_tool_calls") {
///     let calls = state.get_field("pending_tool_calls")?;
///     // AI knows it has pending work
/// }
///
/// // AI monitors state size
/// if state.state_size_bytes() > 1_000_000 {
///     // AI knows to truncate or summarize
/// }
///
/// // AI lists all fields
/// for field in state.list_fields() {
///     println!("Field: {}", field);
/// }
/// ```
pub trait StateIntrospection {
    /// Get a field value by path (supports dot notation for nested fields)
    ///
    /// # Arguments
    /// * `path` - Field path, e.g., "messages" or "user.name" for nested fields
    ///
    /// # Returns
    /// * `Some(value)` if the field exists
    /// * `None` if the field does not exist
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let messages = state.get_field("messages");
    /// let user_name = state.get_field("user.name"); // nested access
    /// ```
    fn get_field(&self, path: &str) -> Option<serde_json::Value>;

    /// Check if a field exists at the given path
    ///
    /// # Arguments
    /// * `path` - Field path to check
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if state.has_field("pending_tool_calls") {
    ///     // Process pending calls
    /// }
    /// ```
    fn has_field(&self, path: &str) -> bool;

    /// List all top-level field names in the state
    ///
    /// # Returns
    /// Vector of field names (empty if state is not an object)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// for field in state.list_fields() {
    ///     println!("Available field: {}", field);
    /// }
    /// ```
    fn list_fields(&self) -> Vec<String>;

    /// Get the approximate size of the state in bytes
    ///
    /// This is useful for AI agents to monitor memory usage and decide
    /// when to truncate or summarize state.
    ///
    /// # Returns
    /// Approximate size in bytes (based on JSON serialization)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if state.state_size_bytes() > 1_000_000 {
    ///     // State is over 1MB, consider summarizing
    /// }
    /// ```
    fn state_size_bytes(&self) -> usize;

    /// Get all nested field paths (for deep introspection)
    ///
    /// Returns paths for all fields, including nested ones, using dot notation.
    ///
    /// # Arguments
    /// * `max_depth` - Maximum depth to traverse (0 = top-level only)
    ///
    /// # Returns
    /// Vector of all field paths
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let all_paths = state.list_all_fields(3);
    /// // Returns: ["messages", "user", "user.name", "user.email", ...]
    /// ```
    fn list_all_fields(&self, max_depth: usize) -> Vec<String>;

    /// Get field type information
    ///
    /// # Arguments
    /// * `path` - Field path to check
    ///
    /// # Returns
    /// String describing the field type (e.g., "string", "number", "array", "object", "null", "boolean")
    fn field_type(&self, path: &str) -> Option<String>;

    /// Convert the entire state to a JSON value for introspection
    ///
    /// # Returns
    /// The state as a `serde_json::Value`
    fn to_introspection_value(&self) -> serde_json::Value;
}

/// Blanket implementation of StateIntrospection for any GraphState type
///
/// This implementation uses serde_json to serialize the state and then
/// introspect the resulting JSON structure.
impl<T> StateIntrospection for T
where
    T: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    fn get_field(&self, path: &str) -> Option<serde_json::Value> {
        let value = serde_json::to_value(self).ok()?;
        get_nested_value(&value, path)
    }

    fn has_field(&self, path: &str) -> bool {
        self.get_field(path).is_some()
    }

    fn list_fields(&self) -> Vec<String> {
        let Ok(value) = serde_json::to_value(self) else {
            return Vec::new();
        };

        match value {
            serde_json::Value::Object(map) => map.keys().cloned().collect(),
            _ => Vec::new(),
        }
    }

    fn state_size_bytes(&self) -> usize {
        serde_json::to_string(self).map(|s| s.len()).unwrap_or(0)
    }

    fn list_all_fields(&self, max_depth: usize) -> Vec<String> {
        let Ok(value) = serde_json::to_value(self) else {
            return Vec::new();
        };

        let mut paths = Vec::new();
        collect_field_paths(&value, "", max_depth, 0, &mut paths);
        paths
    }

    fn field_type(&self, path: &str) -> Option<String> {
        let value = self.get_field(path)?;
        Some(json_type_name(&value))
    }

    fn to_introspection_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

/// Helper function to get a nested value using dot notation
pub fn get_nested_value(value: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = value;

    for part in parts {
        match current {
            serde_json::Value::Object(map) => {
                current = map.get(part)?;
            }
            serde_json::Value::Array(arr) => {
                // Support array index access like "items.0"
                let index: usize = part.parse().ok()?;
                current = arr.get(index)?;
            }
            _ => return None,
        }
    }

    Some(current.clone())
}

/// Helper function to recursively collect all field paths
fn collect_field_paths(
    value: &serde_json::Value,
    prefix: &str,
    max_depth: usize,
    current_depth: usize,
    paths: &mut Vec<String>,
) {
    if let serde_json::Value::Object(map) = value {
        for (key, val) in map {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", prefix, key)
            };
            paths.push(path.clone());

            // Recurse into nested objects if within depth limit
            if current_depth < max_depth {
                if let serde_json::Value::Object(_) = val {
                    collect_field_paths(val, &path, max_depth, current_depth + 1, paths);
                }
            }
        }
    }
}

/// Helper function to get JSON type name
pub fn json_type_name(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(_) => "boolean".to_string(),
        serde_json::Value::Number(_) => "number".to_string(),
        serde_json::Value::String(_) => "string".to_string(),
        serde_json::Value::Array(_) => "array".to_string(),
        serde_json::Value::Object(_) => "object".to_string(),
    }
}

// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    // Test struct for StateIntrospection trait
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestState {
        name: String,
        count: i32,
        active: bool,
        tags: Vec<String>,
        metadata: Option<Metadata>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct Metadata {
        version: String,
        author: String,
        nested: NestedData,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct NestedData {
        level: u32,
        flag: bool,
    }

    fn create_test_state() -> TestState {
        TestState {
            name: "test".to_string(),
            count: 42,
            active: true,
            tags: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            metadata: Some(Metadata {
                version: "1.0".to_string(),
                author: "Alice".to_string(),
                nested: NestedData {
                    level: 3,
                    flag: true,
                },
            }),
        }
    }

    // ========================================================================
    // StateIntrospection Trait Tests
    // ========================================================================

    #[test]
    fn test_get_field_top_level() {
        let state = create_test_state();

        let name = state.get_field("name");
        assert!(name.is_some());
        assert_eq!(name.unwrap(), json!("test"));

        let count = state.get_field("count");
        assert!(count.is_some());
        assert_eq!(count.unwrap(), json!(42));

        let active = state.get_field("active");
        assert!(active.is_some());
        assert_eq!(active.unwrap(), json!(true));
    }

    #[test]
    fn test_get_field_nested() {
        let state = create_test_state();

        let version = state.get_field("metadata.version");
        assert!(version.is_some());
        assert_eq!(version.unwrap(), json!("1.0"));

        let author = state.get_field("metadata.author");
        assert!(author.is_some());
        assert_eq!(author.unwrap(), json!("Alice"));
    }

    #[test]
    fn test_get_field_deeply_nested() {
        let state = create_test_state();

        let level = state.get_field("metadata.nested.level");
        assert!(level.is_some());
        assert_eq!(level.unwrap(), json!(3));

        let flag = state.get_field("metadata.nested.flag");
        assert!(flag.is_some());
        assert_eq!(flag.unwrap(), json!(true));
    }

    #[test]
    fn test_get_field_array() {
        let state = create_test_state();

        let tags = state.get_field("tags");
        assert!(tags.is_some());
        assert_eq!(tags.unwrap(), json!(["a", "b", "c"]));
    }

    #[test]
    fn test_get_field_array_index() {
        let state = create_test_state();

        let first = state.get_field("tags.0");
        assert!(first.is_some());
        assert_eq!(first.unwrap(), json!("a"));

        let second = state.get_field("tags.1");
        assert!(second.is_some());
        assert_eq!(second.unwrap(), json!("b"));

        let third = state.get_field("tags.2");
        assert!(third.is_some());
        assert_eq!(third.unwrap(), json!("c"));
    }

    #[test]
    fn test_get_field_nonexistent() {
        let state = create_test_state();

        assert!(state.get_field("nonexistent").is_none());
        assert!(state.get_field("metadata.nonexistent").is_none());
        assert!(state.get_field("tags.10").is_none());
    }

    #[test]
    fn test_has_field() {
        let state = create_test_state();

        assert!(state.has_field("name"));
        assert!(state.has_field("count"));
        assert!(state.has_field("metadata"));
        assert!(state.has_field("metadata.version"));
        assert!(state.has_field("metadata.nested.level"));
        assert!(state.has_field("tags.0"));

        assert!(!state.has_field("nonexistent"));
        assert!(!state.has_field("metadata.nonexistent"));
    }

    #[test]
    fn test_list_fields() {
        let state = create_test_state();

        let fields = state.list_fields();
        assert_eq!(fields.len(), 5);
        assert!(fields.contains(&"name".to_string()));
        assert!(fields.contains(&"count".to_string()));
        assert!(fields.contains(&"active".to_string()));
        assert!(fields.contains(&"tags".to_string()));
        assert!(fields.contains(&"metadata".to_string()));
    }

    #[test]
    fn test_list_fields_non_object() {
        // For non-object types, list_fields should return empty
        let value = 42i32;
        let fields = value.list_fields();
        assert!(fields.is_empty());

        // String implements the trait
        let value = String::from("test");
        let fields = value.list_fields();
        assert!(fields.is_empty());
    }

    #[test]
    fn test_state_size_bytes() {
        let state = create_test_state();

        let size = state.state_size_bytes();
        assert!(size > 0);
        // The serialized JSON should be at least as long as the field names
        assert!(size > 50);
    }

    #[test]
    fn test_state_size_bytes_empty() {
        #[derive(Serialize, Deserialize)]
        struct Empty {}

        let state = Empty {};
        let size = state.state_size_bytes();
        assert_eq!(size, 2); // "{}"
    }

    #[test]
    fn test_list_all_fields_depth_0() {
        let state = create_test_state();

        let fields = state.list_all_fields(0);
        assert_eq!(fields.len(), 5);
        // Should only have top-level fields
        assert!(fields.contains(&"name".to_string()));
        assert!(fields.contains(&"metadata".to_string()));
        // Should not have nested fields
        assert!(!fields.contains(&"metadata.version".to_string()));
    }

    #[test]
    fn test_list_all_fields_depth_1() {
        let state = create_test_state();

        let fields = state.list_all_fields(1);
        // Should have top-level + first level nested
        assert!(fields.contains(&"name".to_string()));
        assert!(fields.contains(&"metadata".to_string()));
        assert!(fields.contains(&"metadata.version".to_string()));
        assert!(fields.contains(&"metadata.author".to_string()));
        assert!(fields.contains(&"metadata.nested".to_string()));
        // Should not have deeply nested
        assert!(!fields.contains(&"metadata.nested.level".to_string()));
    }

    #[test]
    fn test_list_all_fields_depth_2() {
        let state = create_test_state();

        let fields = state.list_all_fields(2);
        // Should have all nested fields
        assert!(fields.contains(&"metadata.nested.level".to_string()));
        assert!(fields.contains(&"metadata.nested.flag".to_string()));
    }

    #[test]
    fn test_field_type() {
        let state = create_test_state();

        assert_eq!(state.field_type("name"), Some("string".to_string()));
        assert_eq!(state.field_type("count"), Some("number".to_string()));
        assert_eq!(state.field_type("active"), Some("boolean".to_string()));
        assert_eq!(state.field_type("tags"), Some("array".to_string()));
        assert_eq!(state.field_type("metadata"), Some("object".to_string()));
        assert_eq!(state.field_type("nonexistent"), None);
    }

    #[test]
    fn test_field_type_null() {
        #[derive(Serialize, Deserialize)]
        struct StateWithNull {
            value: Option<String>,
        }

        let state = StateWithNull { value: None };
        assert_eq!(state.field_type("value"), Some("null".to_string()));
    }

    #[test]
    fn test_to_introspection_value() {
        let state = create_test_state();

        let value = state.to_introspection_value();
        assert!(value.is_object());

        let obj = value.as_object().unwrap();
        assert!(obj.contains_key("name"));
        assert!(obj.contains_key("count"));
        assert_eq!(obj.get("name"), Some(&json!("test")));
    }

    // ========================================================================
    // Helper Function Tests
    // ========================================================================

    #[test]
    fn test_get_nested_value_simple() {
        let value = json!({"a": {"b": {"c": 42}}});

        assert_eq!(get_nested_value(&value, "a"), Some(json!({"b": {"c": 42}})));
        assert_eq!(get_nested_value(&value, "a.b"), Some(json!({"c": 42})));
        assert_eq!(get_nested_value(&value, "a.b.c"), Some(json!(42)));
    }

    #[test]
    fn test_get_nested_value_array() {
        let value = json!({"items": [1, 2, 3]});

        assert_eq!(get_nested_value(&value, "items"), Some(json!([1, 2, 3])));
        assert_eq!(get_nested_value(&value, "items.0"), Some(json!(1)));
        assert_eq!(get_nested_value(&value, "items.1"), Some(json!(2)));
        assert_eq!(get_nested_value(&value, "items.2"), Some(json!(3)));
    }

    #[test]
    fn test_get_nested_value_mixed() {
        let value = json!({
            "users": [
                {"name": "Alice", "age": 30},
                {"name": "Bob", "age": 25}
            ]
        });

        assert_eq!(
            get_nested_value(&value, "users.0.name"),
            Some(json!("Alice"))
        );
        assert_eq!(get_nested_value(&value, "users.1.age"), Some(json!(25)));
    }

    #[test]
    fn test_get_nested_value_nonexistent() {
        let value = json!({"a": 1});

        assert!(get_nested_value(&value, "b").is_none());
        assert!(get_nested_value(&value, "a.b").is_none());
        assert!(get_nested_value(&value, "a.0").is_none()); // a is not an array
    }

    #[test]
    fn test_get_nested_value_empty_path() {
        let value = json!({"a": 1});
        // Empty string should return the whole value (empty split gives one empty string)
        let result = get_nested_value(&value, "");
        // The function will try to get key "" which won't exist
        assert!(result.is_none());
    }

    #[test]
    fn test_json_type_name_all_types() {
        assert_eq!(json_type_name(&json!(null)), "null");
        assert_eq!(json_type_name(&json!(true)), "boolean");
        assert_eq!(json_type_name(&json!(false)), "boolean");
        assert_eq!(json_type_name(&json!(42)), "number");
        assert_eq!(json_type_name(&json!(314.0 / 100.0)), "number");
        assert_eq!(json_type_name(&json!("hello")), "string");
        assert_eq!(json_type_name(&json!([])), "array");
        assert_eq!(json_type_name(&json!([1, 2, 3])), "array");
        assert_eq!(json_type_name(&json!({})), "object");
        assert_eq!(json_type_name(&json!({"key": "value"})), "object");
    }

    // ========================================================================
    // Edge Case Tests
    // ========================================================================

    #[test]
    fn test_unicode_field_names() {
        #[derive(Serialize, Deserialize)]
        struct UnicodeState {
            #[serde(rename = "名前")]
            name: String,
            #[serde(rename = "数量")]
            count: i32,
        }

        let state = UnicodeState {
            name: "テスト".to_string(),
            count: 100,
        };

        assert!(state.has_field("名前"));
        assert!(state.has_field("数量"));
        assert_eq!(state.get_field("名前"), Some(json!("テスト")));
    }

    #[test]
    fn test_empty_struct() {
        #[derive(Serialize, Deserialize)]
        struct Empty {}

        let state = Empty {};
        assert!(state.list_fields().is_empty());
        assert_eq!(state.state_size_bytes(), 2);
        assert!(!state.has_field("anything"));
    }

    #[test]
    fn test_deeply_nested_structure() {
        #[derive(Serialize, Deserialize)]
        struct Level1 {
            level2: Level2,
        }
        #[derive(Serialize, Deserialize)]
        struct Level2 {
            level3: Level3,
        }
        #[derive(Serialize, Deserialize)]
        struct Level3 {
            level4: Level4,
        }
        #[derive(Serialize, Deserialize)]
        struct Level4 {
            value: i32,
        }

        let state = Level1 {
            level2: Level2 {
                level3: Level3 {
                    level4: Level4 { value: 42 },
                },
            },
        };

        assert!(state.has_field("level2.level3.level4.value"));
        assert_eq!(
            state.get_field("level2.level3.level4.value"),
            Some(json!(42))
        );
    }

    #[test]
    fn test_special_characters_in_values() {
        #[derive(Serialize, Deserialize)]
        struct SpecialState {
            text: String,
        }

        let state = SpecialState {
            text: "Line1\nLine2\t\"quoted\" \\escaped".to_string(),
        };

        let value = state.get_field("text");
        assert!(value.is_some());
        // The JSON value contains the actual newline character
        let text = value.unwrap();
        assert!(text.as_str().unwrap().contains('\n'));
        assert!(text.as_str().unwrap().contains('\t'));
    }

    #[test]
    fn test_large_array() {
        #[derive(Serialize, Deserialize)]
        struct ArrayState {
            items: Vec<i32>,
        }

        let state = ArrayState {
            items: (0..1000).collect(),
        };

        assert!(state.has_field("items"));
        assert!(state.has_field("items.0"));
        assert!(state.has_field("items.999"));
        assert!(!state.has_field("items.1000"));

        assert_eq!(state.get_field("items.500"), Some(json!(500)));
    }

    #[test]
    fn test_optional_fields() {
        #[derive(Serialize, Deserialize)]
        struct OptionalState {
            required: String,
            optional: Option<String>,
        }

        let with_value = OptionalState {
            required: "present".to_string(),
            optional: Some("also present".to_string()),
        };

        let without_value = OptionalState {
            required: "present".to_string(),
            optional: None,
        };

        assert!(with_value.has_field("optional"));
        assert_eq!(
            with_value.get_field("optional"),
            Some(json!("also present"))
        );

        assert!(without_value.has_field("optional"));
        assert_eq!(without_value.get_field("optional"), Some(json!(null)));
    }

    #[test]
    fn test_state_with_hashmap() {
        use std::collections::HashMap;

        #[derive(Serialize, Deserialize)]
        struct MapState {
            data: HashMap<String, i32>,
        }

        let mut data = HashMap::new();
        data.insert("one".to_string(), 1);
        data.insert("two".to_string(), 2);

        let state = MapState { data };

        assert!(state.has_field("data"));
        assert!(state.has_field("data.one"));
        assert!(state.has_field("data.two"));
        assert_eq!(state.get_field("data.one"), Some(json!(1)));
    }

    #[test]
    fn test_introspection_with_primitive_types() {
        // Test that the blanket impl works with primitive types
        let number: i32 = 42;
        let string: String = "hello".to_string();
        let boolean: bool = true;

        // These don't have fields but shouldn't panic
        assert!(number.list_fields().is_empty());
        assert!(string.list_fields().is_empty());
        assert!(boolean.list_fields().is_empty());

        // Size should be non-zero
        assert!(number.state_size_bytes() > 0);
        assert!(string.state_size_bytes() > 0);
        assert!(boolean.state_size_bytes() > 0);
    }

    #[test]
    fn test_introspection_value_preserves_types() {
        let state = create_test_state();
        let value = state.to_introspection_value();

        // Verify types are preserved
        assert!(value["name"].is_string());
        assert!(value["count"].is_number());
        assert!(value["active"].is_boolean());
        assert!(value["tags"].is_array());
        assert!(value["metadata"].is_object());
    }
}
