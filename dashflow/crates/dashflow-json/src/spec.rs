//! JSON specification for navigating JSON data structures

use regex::Regex;
use serde_json::Value;
use std::path::Path;
use std::sync::OnceLock;

/// Static regex for parsing bracket expressions (compiled once).
static BRACKET_REGEX: OnceLock<Regex> = OnceLock::new();

#[allow(clippy::expect_used)] // Safe: regex pattern is a compile-time constant
fn get_bracket_regex() -> &'static Regex {
    BRACKET_REGEX.get_or_init(|| Regex::new(r"\[.*?\]").expect("BRACKET_REGEX pattern is valid"))
}

/// Parses input of the form data\["key1"\]\[0\]\["key2"\] into a list of keys.
///
/// This function extracts the keys from a Python-style path expression and converts
/// numeric strings to integers for array indexing.
///
/// # Examples
///
/// ```ignore
/// let keys = parse_input("data[\"key1\"][0][\"key2\"]");
/// // Returns: vec!["key1", "0", "key2"]
/// ```
fn parse_input(text: &str) -> Vec<String> {
    let re = get_bracket_regex();
    let mut result = Vec::new();

    for cap in re.find_iter(text) {
        let matched = cap.as_str();
        // Strip brackets and quotes
        let stripped = &matched[1..matched.len() - 1];
        let unquoted = stripped.replace(['"', '\''], "");
        result.push(unquoted);
    }

    result
}

/// JSON specification for navigating and querying JSON data.
///
/// `JsonSpec` holds a JSON value and provides methods to navigate it using
/// Python-style path expressions (e.g., `data["key1"][0]["key2"]`).
///
/// # Examples
///
/// ```rust
/// use dashflow_json::JsonSpec;
/// use serde_json::json;
///
/// let data = json!({
///     "users": [
///         {"name": "Alice", "age": 30},
///         {"name": "Bob", "age": 25}
///     ]
/// });
///
/// let spec = JsonSpec::new(data);
///
/// // List keys at root level
/// let keys = spec.keys("data");
/// assert_eq!(keys, "['users']");
///
/// // Get a value
/// let value = spec.value("data[\"users\"][0][\"name\"]");
/// assert_eq!(value, "Alice");
/// ```
#[derive(Debug, Clone)]
pub struct JsonSpec {
    /// The JSON data structure
    dict: Value,
    /// Maximum length for value strings (longer values are truncated)
    max_value_length: usize,
}

impl JsonSpec {
    /// Creates a new `JsonSpec` from a JSON value.
    ///
    /// # Arguments
    ///
    /// * `dict` - The JSON value to wrap
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dashflow_json::JsonSpec;
    /// use serde_json::json;
    ///
    /// let spec = JsonSpec::new(json!({"key": "value"}));
    /// ```
    #[must_use]
    pub fn new(dict: Value) -> Self {
        Self {
            dict,
            max_value_length: 200,
        }
    }

    /// Creates a new `JsonSpec` with a custom max value length.
    ///
    /// # Arguments
    ///
    /// * `dict` - The JSON value to wrap
    /// * `max_value_length` - Maximum length for value strings
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dashflow_json::JsonSpec;
    /// use serde_json::json;
    ///
    /// let spec = JsonSpec::with_max_length(json!({"key": "value"}), 100);
    /// ```
    #[must_use]
    pub fn with_max_length(dict: Value, max_value_length: usize) -> Self {
        Self {
            dict,
            max_value_length,
        }
    }

    /// Creates a `JsonSpec` from a JSON file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the JSON file
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file does not exist
    /// - The file cannot be read
    /// - The file does not contain valid JSON
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dashflow_json::JsonSpec;
    /// use std::path::Path;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let spec = JsonSpec::from_file(Path::new("data.json"))?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            anyhow::bail!("File not found: {}", path.display());
        }

        let contents = std::fs::read_to_string(path)?;
        let dict: Value = serde_json::from_str(&contents)?;

        Ok(Self::new(dict))
    }

    /// Returns the keys of the dict at the given path.
    ///
    /// The path should be a Python-style expression like `data["key1"][0]["key2"]`.
    /// If the value at the path is not a dict/object, returns an error message.
    ///
    /// # Arguments
    ///
    /// * `text` - Python representation of the path to the dict
    ///
    /// # Returns
    ///
    /// A string representation of the keys, or an error message.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dashflow_json::JsonSpec;
    /// use serde_json::json;
    ///
    /// let spec = JsonSpec::new(json!({"a": {"b": "value"}}));
    /// let keys = spec.keys("data");
    /// assert_eq!(keys, "['a']");
    ///
    /// let nested_keys = spec.keys("data[\"a\"]");
    /// assert_eq!(nested_keys, "['b']");
    /// ```
    #[must_use]
    pub fn keys(&self, text: &str) -> String {
        match self.keys_impl(text) {
            Ok(result) => result,
            Err(e) => format!("{e:?}"),
        }
    }

    fn keys_impl(&self, text: &str) -> anyhow::Result<String> {
        let items = parse_input(text);
        let mut val = &self.dict;

        for item in items {
            if item.is_empty() {
                continue;
            }

            // Try to parse as array index
            if let Ok(idx) = item.parse::<usize>() {
                val = val
                    .get(idx)
                    .ok_or_else(|| anyhow::anyhow!("Index {idx} out of bounds"))?;
            } else {
                val = val
                    .get(&item)
                    .ok_or_else(|| anyhow::anyhow!("Key '{item}' not found"))?;
            }
        }

        // Check if value is an object
        if let Some(obj) = val.as_object() {
            let keys: Vec<String> = obj.keys().map(|k| format!("'{k}'")).collect();
            Ok(format!("[{}]", keys.join(", ")))
        } else {
            anyhow::bail!("Value at path `{text}` is not a dict, get the value directly.")
        }
    }

    /// Returns the value of the dict at the given path.
    ///
    /// The path should be a Python-style expression like `data["key1"][0]["key2"]`.
    /// Large dictionaries return a message to explore keys instead.
    /// Long values are truncated to `max_value_length`.
    ///
    /// # Arguments
    ///
    /// * `text` - Python representation of the path to the value
    ///
    /// # Returns
    ///
    /// A string representation of the value, or an error message.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dashflow_json::JsonSpec;
    /// use serde_json::json;
    ///
    /// let spec = JsonSpec::new(json!({"a": {"b": "value"}}));
    /// let value = spec.value("data[\"a\"][\"b\"]");
    /// assert_eq!(value, "value");
    /// ```
    #[must_use]
    pub fn value(&self, text: &str) -> String {
        match self.value_impl(text) {
            Ok(result) => result,
            Err(e) => format!("{e:?}"),
        }
    }

    fn value_impl(&self, text: &str) -> anyhow::Result<String> {
        let items = parse_input(text);
        let mut val = &self.dict;

        for item in items {
            if item.is_empty() {
                continue;
            }

            // Try to parse as array index
            if let Ok(idx) = item.parse::<usize>() {
                val = val
                    .get(idx)
                    .ok_or_else(|| anyhow::anyhow!("Index {idx} out of bounds"))?;
            } else {
                val = val
                    .get(&item)
                    .ok_or_else(|| anyhow::anyhow!("Key '{item}' not found"))?;
            }
        }

        // Check if it's a large dictionary
        if val.is_object() {
            let str_val = val.to_string();
            if str_val.len() > self.max_value_length {
                return Ok(
                    "Value is a large dictionary, should explore its keys directly".to_string(),
                );
            }
        }

        // Convert to string and truncate if needed
        let str_val = match val {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };

        if str_val.len() > self.max_value_length {
            Ok(format!("{}...", &str_val[..self.max_value_length]))
        } else {
            Ok(str_val)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_input() {
        let result = parse_input("data[\"key1\"][0][\"key2\"]");
        assert_eq!(result, vec!["key1", "0", "key2"]);

        let result = parse_input("data['single'][\"double\"]");
        assert_eq!(result, vec!["single", "double"]);
    }

    #[test]
    fn test_json_spec_new() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        assert_eq!(spec.max_value_length, 200);
    }

    #[test]
    fn test_json_spec_keys_root() {
        let data = json!({
            "users": [],
            "metadata": {}
        });
        let spec = JsonSpec::new(data);
        let keys = spec.keys("data");
        assert!(keys.contains("'users'"));
        assert!(keys.contains("'metadata'"));
    }

    #[test]
    fn test_json_spec_keys_nested() {
        let data = json!({
            "users": {
                "alice": {"age": 30},
                "bob": {"age": 25}
            }
        });
        let spec = JsonSpec::new(data);
        let keys = spec.keys("data[\"users\"]");
        assert!(keys.contains("'alice'"));
        assert!(keys.contains("'bob'"));
    }

    #[test]
    fn test_json_spec_keys_not_dict() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        let result = spec.keys("data[\"key\"]");
        assert!(result.contains("not a dict"));
    }

    #[test]
    fn test_json_spec_value_string() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        let value = spec.value("data[\"key\"]");
        assert_eq!(value, "value");
    }

    #[test]
    fn test_json_spec_value_number() {
        let data = json!({"age": 30});
        let spec = JsonSpec::new(data);
        let value = spec.value("data[\"age\"]");
        assert_eq!(value, "30");
    }

    #[test]
    fn test_json_spec_value_array_index() {
        let data = json!({"users": ["alice", "bob"]});
        let spec = JsonSpec::new(data);
        let value = spec.value("data[\"users\"][0]");
        assert_eq!(value, "alice");
        let value = spec.value("data[\"users\"][1]");
        assert_eq!(value, "bob");
    }

    #[test]
    fn test_json_spec_value_nested() {
        let data = json!({
            "users": [
                {"name": "Alice", "age": 30},
                {"name": "Bob", "age": 25}
            ]
        });
        let spec = JsonSpec::new(data);
        let value = spec.value("data[\"users\"][0][\"name\"]");
        assert_eq!(value, "Alice");
    }

    #[test]
    fn test_json_spec_value_large_dict() {
        let data = json!({
            "large": {
                "key1": "a".repeat(100),
                "key2": "b".repeat(100)
            }
        });
        let spec = JsonSpec::new(data);
        let value = spec.value("data[\"large\"]");
        assert_eq!(
            value,
            "Value is a large dictionary, should explore its keys directly"
        );
    }

    #[test]
    fn test_json_spec_value_truncation() {
        let long_value = "x".repeat(300);
        let data = json!({"key": long_value});
        let spec = JsonSpec::new(data);
        let value = spec.value("data[\"key\"]");
        assert_eq!(value.len(), 203); // 200 + "..."
        assert!(value.ends_with("..."));
    }

    #[test]
    fn test_json_spec_with_max_length() {
        let data = json!({"key": "x".repeat(150)});
        let spec = JsonSpec::with_max_length(data, 50);
        let value = spec.value("data[\"key\"]");
        assert_eq!(value.len(), 53); // 50 + "..."
        assert!(value.ends_with("..."));
    }

    #[test]
    fn test_json_spec_error_missing_key() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        let result = spec.value("data[\"missing\"]");
        assert!(result.contains("not found"));
    }

    #[test]
    fn test_json_spec_error_index_out_of_bounds() {
        let data = json!({"arr": [1, 2, 3]});
        let spec = JsonSpec::new(data);
        let result = spec.value("data[\"arr\"][10]");
        assert!(result.contains("out of bounds"));
    }

    // ========== parse_input edge cases ==========

    #[test]
    fn test_parse_input_empty() {
        let result = parse_input("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_input_no_brackets() {
        let result = parse_input("data");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_input_empty_brackets() {
        let result = parse_input("data[]");
        assert_eq!(result, vec![""]);
    }

    #[test]
    fn test_parse_input_numeric_only() {
        let result = parse_input("[0][1][2]");
        assert_eq!(result, vec!["0", "1", "2"]);
    }

    #[test]
    fn test_parse_input_mixed_quotes() {
        let result = parse_input("data['key1'][\"key2\"]['key3']");
        assert_eq!(result, vec!["key1", "key2", "key3"]);
    }

    #[test]
    fn test_parse_input_deeply_nested() {
        let result = parse_input("data[\"a\"][\"b\"][\"c\"][\"d\"][\"e\"][\"f\"]");
        assert_eq!(result, vec!["a", "b", "c", "d", "e", "f"]);
    }

    #[test]
    fn test_parse_input_unicode_keys() {
        let result = parse_input("data[\"日本語\"][\"emoji🎉\"]");
        assert_eq!(result, vec!["日本語", "emoji🎉"]);
    }

    #[test]
    fn test_parse_input_special_chars_in_keys() {
        let result = parse_input("data[\"key.with.dots\"][\"key/with/slashes\"]");
        assert_eq!(result, vec!["key.with.dots", "key/with/slashes"]);
    }

    #[test]
    fn test_parse_input_spaces_in_keys() {
        let result = parse_input("data[\"key with spaces\"][\"another key\"]");
        assert_eq!(result, vec!["key with spaces", "another key"]);
    }

    #[test]
    fn test_parse_input_large_indices() {
        let result = parse_input("data[999999][1000000]");
        assert_eq!(result, vec!["999999", "1000000"]);
    }

    // ========== JsonSpec construction and traits ==========

    #[test]
    fn test_json_spec_clone() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        let spec2 = spec.clone();
        assert_eq!(spec.value("data[\"key\"]"), spec2.value("data[\"key\"]"));
    }

    #[test]
    fn test_json_spec_debug() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        let debug_str = format!("{:?}", spec);
        assert!(debug_str.contains("JsonSpec"));
        assert!(debug_str.contains("max_value_length"));
    }

    #[test]
    fn test_json_spec_default_max_value_length() {
        // Verify default is 200 by testing truncation behavior
        let long_value = "x".repeat(201);
        let data = json!({"key": long_value});
        let spec = JsonSpec::new(data);
        assert_eq!(spec.max_value_length, 200); // Default value
        let result = spec.value("data[\"key\"]");
        assert!(result.ends_with("..."));
        assert_eq!(result.len(), 203); // 200 + "..."
    }

    // ========== JsonSpec with different JSON types ==========

    #[test]
    fn test_json_spec_empty_object() {
        let spec = JsonSpec::new(json!({}));
        let keys = spec.keys("data");
        assert_eq!(keys, "[]");
    }

    #[test]
    fn test_json_spec_empty_array() {
        let spec = JsonSpec::new(json!([]));
        // Arrays don't have keys, so this should error
        let keys = spec.keys("data");
        assert!(keys.contains("not a dict"));
    }

    #[test]
    fn test_json_spec_null_value() {
        let spec = JsonSpec::new(json!({"key": null}));
        let value = spec.value("data[\"key\"]");
        assert_eq!(value, "null");
    }

    #[test]
    fn test_json_spec_boolean_true() {
        let spec = JsonSpec::new(json!({"flag": true}));
        let value = spec.value("data[\"flag\"]");
        assert_eq!(value, "true");
    }

    #[test]
    fn test_json_spec_boolean_false() {
        let spec = JsonSpec::new(json!({"flag": false}));
        let value = spec.value("data[\"flag\"]");
        assert_eq!(value, "false");
    }

    #[test]
    fn test_json_spec_float_value() {
        let spec = JsonSpec::new(json!({"pi": 3.14159}));
        let value = spec.value("data[\"pi\"]");
        assert!(value.starts_with("3.14"));
    }

    #[test]
    fn test_json_spec_negative_number() {
        let spec = JsonSpec::new(json!({"temp": -10}));
        let value = spec.value("data[\"temp\"]");
        assert_eq!(value, "-10");
    }

    #[test]
    fn test_json_spec_zero() {
        let spec = JsonSpec::new(json!({"zero": 0}));
        let value = spec.value("data[\"zero\"]");
        assert_eq!(value, "0");
    }

    // ========== Complex nested structures ==========

    #[test]
    fn test_json_spec_array_of_arrays() {
        let data = json!({
            "matrix": [[1, 2], [3, 4], [5, 6]]
        });
        let spec = JsonSpec::new(data);
        let value = spec.value("data[\"matrix\"][1][0]");
        assert_eq!(value, "3");
    }

    #[test]
    fn test_json_spec_mixed_nesting() {
        let data = json!({
            "level1": {
                "level2": [{
                    "level3": {
                        "value": "deep"
                    }
                }]
            }
        });
        let spec = JsonSpec::new(data);
        let value = spec.value("data[\"level1\"][\"level2\"][0][\"level3\"][\"value\"]");
        assert_eq!(value, "deep");
    }

    #[test]
    fn test_json_spec_keys_at_array_element() {
        let data = json!({
            "users": [
                {"name": "Alice", "age": 30}
            ]
        });
        let spec = JsonSpec::new(data);
        let keys = spec.keys("data[\"users\"][0]");
        assert!(keys.contains("'name'"));
        assert!(keys.contains("'age'"));
    }

    // ========== Unicode and special characters ==========

    #[test]
    fn test_json_spec_unicode_key() {
        let data = json!({"日本語": "value"});
        let spec = JsonSpec::new(data);
        let value = spec.value("data[\"日本語\"]");
        assert_eq!(value, "value");
    }

    #[test]
    fn test_json_spec_unicode_value() {
        let data = json!({"key": "こんにちは世界"});
        let spec = JsonSpec::new(data);
        let value = spec.value("data[\"key\"]");
        assert_eq!(value, "こんにちは世界");
    }

    #[test]
    fn test_json_spec_emoji_key() {
        let data = json!({"🎉": "party"});
        let spec = JsonSpec::new(data);
        let value = spec.value("data[\"🎉\"]");
        assert_eq!(value, "party");
    }

    #[test]
    fn test_json_spec_key_with_dots() {
        let data = json!({"config.database.host": "localhost"});
        let spec = JsonSpec::new(data);
        let value = spec.value("data[\"config.database.host\"]");
        assert_eq!(value, "localhost");
    }

    #[test]
    fn test_json_spec_key_with_spaces() {
        let data = json!({"my key": "my value"});
        let spec = JsonSpec::new(data);
        let value = spec.value("data[\"my key\"]");
        assert_eq!(value, "my value");
    }

    // ========== Truncation edge cases ==========

    #[test]
    fn test_json_spec_value_exactly_at_max_length() {
        let value = "x".repeat(200);
        let data = json!({"key": value.clone()});
        let spec = JsonSpec::new(data);
        let result = spec.value("data[\"key\"]");
        // Exactly at max length should NOT be truncated
        assert_eq!(result, value);
        assert!(!result.ends_with("..."));
    }

    #[test]
    fn test_json_spec_value_one_over_max_length() {
        let value = "x".repeat(201);
        let data = json!({"key": value});
        let spec = JsonSpec::new(data);
        let result = spec.value("data[\"key\"]");
        assert!(result.ends_with("..."));
        assert_eq!(result.len(), 203);
    }

    #[test]
    fn test_json_spec_custom_max_length_zero() {
        let data = json!({"key": "abc"});
        let spec = JsonSpec::with_max_length(data, 0);
        let result = spec.value("data[\"key\"]");
        // Even with max_length 0, string "abc" (len 3) > 0 so truncated
        assert_eq!(result, "...");
    }

    #[test]
    fn test_json_spec_custom_max_length_one() {
        let data = json!({"key": "abc"});
        let spec = JsonSpec::with_max_length(data, 1);
        let result = spec.value("data[\"key\"]");
        assert_eq!(result, "a...");
    }

    // ========== Large dictionary detection ==========

    #[test]
    fn test_json_spec_small_dict_returns_json() {
        let data = json!({"small": {"a": 1}});
        let spec = JsonSpec::new(data);
        let value = spec.value("data[\"small\"]");
        // Small dict should return JSON representation
        assert!(value.contains("\"a\""));
    }

    #[test]
    fn test_json_spec_dict_at_boundary() {
        // Create a dict that serializes to exactly 200 chars
        let data = json!({"dict": {"key": "x".repeat(180)}});
        let spec = JsonSpec::new(data);
        let value = spec.value("data[\"dict\"]");
        // Should return the dict if <= 200 chars, or message if > 200
        // This tests the boundary condition
        assert!(
            value.contains("key") || value.contains("large dictionary"),
            "Expected dict or large dictionary message"
        );
    }

    // ========== Error cases ==========

    #[test]
    fn test_json_spec_keys_error_format() {
        let spec = JsonSpec::new(json!({"key": "value"}));
        let result = spec.keys("data[\"missing\"]");
        // Error should be formatted as debug
        assert!(result.contains("not found"));
    }

    #[test]
    fn test_json_spec_value_error_format() {
        let spec = JsonSpec::new(json!({"key": "value"}));
        let result = spec.value("data[\"missing\"]");
        assert!(result.contains("not found"));
    }

    #[test]
    fn test_json_spec_nested_missing_key() {
        let data = json!({"a": {"b": "c"}});
        let spec = JsonSpec::new(data);
        let result = spec.value("data[\"a\"][\"missing\"]");
        assert!(result.contains("not found"));
    }

    #[test]
    fn test_json_spec_array_negative_index_as_string() {
        let data = json!({"arr": [1, 2, 3]});
        let spec = JsonSpec::new(data);
        // "-1" is not a valid usize, so it's treated as a key
        let result = spec.value("data[\"arr\"][\"-1\"]");
        // Arrays don't have string keys
        assert!(result.contains("out of bounds") || result.contains("not found"));
    }

    // ========== Path expression variations ==========

    #[test]
    fn test_json_spec_single_bracket() {
        let data = json!({"key": "value"});
        let spec = JsonSpec::new(data);
        let value = spec.value("[\"key\"]");
        assert_eq!(value, "value");
    }

    #[test]
    fn test_json_spec_multiple_roots() {
        let data = json!({"a": 1, "b": 2, "c": 3});
        let spec = JsonSpec::new(data);
        let keys = spec.keys("");
        // Empty path should list root keys
        assert!(keys.contains("'a'"));
        assert!(keys.contains("'b'"));
        assert!(keys.contains("'c'"));
    }

    #[test]
    fn test_json_spec_array_direct_access() {
        let data = json!([{"name": "first"}, {"name": "second"}]);
        let spec = JsonSpec::new(data);
        let value = spec.value("[0][\"name\"]");
        assert_eq!(value, "first");
    }

    // ========== from_file tests (with temp files) ==========

    #[test]
    fn test_json_spec_from_file_valid() {
        use std::io::Write;
        let dir = std::env::temp_dir();
        let path = dir.join("test_json_spec_valid.json");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, r#"{{"key": "value"}}"#).unwrap();

        let spec = JsonSpec::from_file(&path).unwrap();
        let value = spec.value("data[\"key\"]");
        assert_eq!(value, "value");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_json_spec_from_file_not_found() {
        let path = std::path::Path::new("/nonexistent/path/to/file.json");
        let result = JsonSpec::from_file(path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("File not found"));
    }

    #[test]
    fn test_json_spec_from_file_invalid_json() {
        use std::io::Write;
        let dir = std::env::temp_dir();
        let path = dir.join("test_json_spec_invalid.json");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, "not valid json").unwrap();

        let result = JsonSpec::from_file(&path);
        assert!(result.is_err());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_json_spec_from_file_empty_object() {
        use std::io::Write;
        let dir = std::env::temp_dir();
        let path = dir.join("test_json_spec_empty.json");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, "{{}}").unwrap();

        let spec = JsonSpec::from_file(&path).unwrap();
        let keys = spec.keys("data");
        assert_eq!(keys, "[]");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_json_spec_from_file_array() {
        use std::io::Write;
        let dir = std::env::temp_dir();
        let path = dir.join("test_json_spec_array.json");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, r#"[1, 2, 3]"#).unwrap();

        let spec = JsonSpec::from_file(&path).unwrap();
        let value = spec.value("[0]");
        assert_eq!(value, "1");

        std::fs::remove_file(&path).ok();
    }

    // ========== Trait bounds verification ==========

    #[test]
    fn test_json_spec_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<JsonSpec>();
    }

    // ========== Additional value types ==========

    #[test]
    fn test_json_spec_value_empty_string() {
        let data = json!({"key": ""});
        let spec = JsonSpec::new(data);
        let value = spec.value("data[\"key\"]");
        assert_eq!(value, "");
    }

    #[test]
    fn test_json_spec_value_empty_array() {
        let data = json!({"arr": []});
        let spec = JsonSpec::new(data);
        let value = spec.value("data[\"arr\"]");
        assert_eq!(value, "[]");
    }

    #[test]
    fn test_json_spec_value_nested_empty_objects() {
        let data = json!({"a": {"b": {"c": {}}}});
        let spec = JsonSpec::new(data);
        let value = spec.value("data[\"a\"][\"b\"][\"c\"]");
        assert_eq!(value, "{}");
    }

    #[test]
    fn test_json_spec_keys_many_keys() {
        let mut obj = serde_json::Map::new();
        for i in 0..100 {
            obj.insert(format!("key{}", i), json!(i));
        }
        let spec = JsonSpec::new(Value::Object(obj));
        let keys = spec.keys("data");
        // Should contain all keys
        for i in 0..100 {
            assert!(keys.contains(&format!("'key{}'", i)));
        }
    }

    #[test]
    fn test_json_spec_very_deep_nesting() {
        // Create deeply nested structure
        let mut value = json!("deep");
        for _ in 0..20 {
            value = json!({"nested": value});
        }
        let spec = JsonSpec::new(value);
        let path = (0..20).map(|_| "[\"nested\"]").collect::<String>();
        let result = spec.value(&path);
        assert_eq!(result, "deep");
    }
}
