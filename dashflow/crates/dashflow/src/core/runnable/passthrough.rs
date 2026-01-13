//! RunnablePassthrough and RunnablePick - Passthrough and key selection Runnables
//!
//! This module provides:
//! - `RunnablePassthrough`: Passes input unchanged (useful for parallel patterns)
//! - `RunnablePick`: Extracts specific keys from HashMap inputs

use async_trait::async_trait;
use std::collections::HashMap;

use crate::core::config::RunnableConfig;
use crate::core::error::Result;

use super::Runnable;

/// A Runnable that passes through its input unchanged
///
/// Useful as a placeholder or for parallel execution patterns.
#[derive(Clone)]
pub struct RunnablePassthrough<T> {
    _phantom: std::marker::PhantomData<T>,
}

impl<T> RunnablePassthrough<T> {
    /// Create a new `RunnablePassthrough`
    #[must_use]
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl RunnablePassthrough<HashMap<String, serde_json::Value>> {
    /// Create a `RunnablePick` that extracts specific keys from input dict
    ///
    /// This creates a new `HashMap` containing only the specified keys from the input.
    /// Keys that don't exist in the input are omitted from the output.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use std::collections::HashMap;
    /// use dashflow::core::runnable::RunnablePassthrough;
    ///
    /// let pick = RunnablePassthrough::pick(vec!["name".to_string(), "age".to_string()]);
    /// let input = HashMap::from([
    ///     ("name".to_string(), serde_json::json!("John")),
    ///     ("age".to_string(), serde_json::json!(30)),
    ///     ("city".to_string(), serde_json::json!("NYC")),
    /// ]);
    /// let result = pick.invoke(input, None).await?;
    /// // result = {"name": "John", "age": 30}
    /// ```
    #[must_use]
    pub fn pick(keys: Vec<String>) -> RunnablePick {
        RunnablePick::new(keys)
    }
}

impl<T> Default for RunnablePassthrough<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<T: Send + Sync + serde::Serialize + Clone> Runnable for RunnablePassthrough<T> {
    type Input = T;
    type Output = T;

    fn name(&self) -> String {
        "Passthrough".to_string()
    }

    async fn invoke(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        // Setup callbacks
        let mut config = config.unwrap_or_default();
        let run_id = config.ensure_run_id();
        let callback_manager = config.get_callback_manager();

        // Create serialized info
        let mut serialized = HashMap::new();
        serialized.insert("name".to_string(), serde_json::json!(self.name()));

        // Serialize input for RunTree
        let inputs_json = serde_json::to_value(&input).unwrap_or_else(|_| serde_json::json!({}));

        // Start chain
        callback_manager
            .on_chain_start(
                &serialized,
                &HashMap::from([("input".to_string(), inputs_json)]),
                run_id,
                None,
                &config.tags,
                &config.metadata,
            )
            .await?;

        // Clone input for output (passthrough)
        let output = input.clone();

        // Serialize output for RunTree
        let outputs_json = serde_json::to_value(&output).unwrap_or_else(|_| serde_json::json!({}));

        // End chain (passthrough always succeeds)
        callback_manager
            .on_chain_end(
                &HashMap::from([("output".to_string(), outputs_json)]),
                run_id,
                None,
            )
            .await?;

        Ok(output)
    }
}

/// A Runnable that picks specific keys from a `HashMap` input
///
/// `RunnablePick` selectively extracts keys from a `HashMap`, returning a new `HashMap`
/// containing only the specified keys. Keys that don't exist in the input are omitted.
///
/// This is typically created via `RunnablePassthrough::pick()`.
///
/// # Example
///
/// ```rust,ignore
/// use std::collections::HashMap;
/// use dashflow::core::runnable::RunnablePick;
///
/// let pick = RunnablePick::new(vec!["name".to_string(), "age".to_string()]);
/// let input = HashMap::from([
///     ("name".to_string(), serde_json::json!("John")),
///     ("age".to_string(), serde_json::json!(30)),
///     ("city".to_string(), serde_json::json!("New York")),
///     ("country".to_string(), serde_json::json!("USA")),
/// ]);
/// let result = pick.invoke(input, None).await?;
/// // result = {"name": "John", "age": 30}
/// ```
pub struct RunnablePick {
    keys: Vec<String>,
}

impl RunnablePick {
    /// Create a new `RunnablePick` with specified keys
    #[must_use]
    pub fn new(keys: Vec<String>) -> Self {
        Self { keys }
    }

    /// Get the list of keys this `RunnablePick` will extract
    #[must_use]
    pub fn keys(&self) -> &[String] {
        &self.keys
    }

    /// Pick specified keys from the input `HashMap`
    fn pick(
        &self,
        input: &HashMap<String, serde_json::Value>,
    ) -> HashMap<String, serde_json::Value> {
        let mut result = HashMap::new();
        for key in &self.keys {
            if let Some(value) = input.get(key) {
                result.insert(key.clone(), value.clone());
            }
        }
        result
    }
}

#[async_trait]
impl Runnable for RunnablePick {
    type Input = HashMap<String, serde_json::Value>;
    type Output = HashMap<String, serde_json::Value>;

    fn name(&self) -> String {
        format!("RunnablePick<{}>", self.keys.join(","))
    }

    async fn invoke(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        // Setup callbacks
        let mut config = config.unwrap_or_default();
        let run_id = config.ensure_run_id();
        let callback_manager = config.get_callback_manager();

        // Create serialized info
        let mut serialized = HashMap::new();
        serialized.insert("name".to_string(), serde_json::json!(self.name()));

        // Start chain
        callback_manager
            .on_chain_start(
                &serialized,
                &input,
                run_id,
                None,
                &config.tags,
                &config.metadata,
            )
            .await?;

        // Pick specified keys
        let result = self.pick(&input);

        // End chain
        callback_manager.on_chain_end(&result, run_id, None).await?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // RunnablePassthrough Construction Tests
    // ============================================

    #[test]
    fn test_passthrough_new() {
        let passthrough: RunnablePassthrough<i32> = RunnablePassthrough::new();
        assert_eq!(passthrough.name(), "Passthrough");
    }

    #[test]
    fn test_passthrough_default() {
        let passthrough: RunnablePassthrough<i32> = RunnablePassthrough::default();
        assert_eq!(passthrough.name(), "Passthrough");
    }

    #[test]
    fn test_passthrough_clone() {
        let passthrough: RunnablePassthrough<i32> = RunnablePassthrough::new();
        let cloned = passthrough.clone();
        assert_eq!(cloned.name(), "Passthrough");
    }

    // ============================================
    // RunnablePassthrough Invoke Tests - Basic Types
    // ============================================

    #[tokio::test]
    async fn test_passthrough_invoke_int() {
        let passthrough: RunnablePassthrough<i32> = RunnablePassthrough::new();
        let result = passthrough.invoke(42, None).await.unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_passthrough_invoke_zero() {
        let passthrough: RunnablePassthrough<i32> = RunnablePassthrough::new();
        let result = passthrough.invoke(0, None).await.unwrap();
        assert_eq!(result, 0);
    }

    #[tokio::test]
    async fn test_passthrough_invoke_negative() {
        let passthrough: RunnablePassthrough<i32> = RunnablePassthrough::new();
        let result = passthrough.invoke(-42, None).await.unwrap();
        assert_eq!(result, -42);
    }

    #[tokio::test]
    async fn test_passthrough_invoke_max_int() {
        let passthrough: RunnablePassthrough<i32> = RunnablePassthrough::new();
        let result = passthrough.invoke(i32::MAX, None).await.unwrap();
        assert_eq!(result, i32::MAX);
    }

    #[tokio::test]
    async fn test_passthrough_invoke_min_int() {
        let passthrough: RunnablePassthrough<i32> = RunnablePassthrough::new();
        let result = passthrough.invoke(i32::MIN, None).await.unwrap();
        assert_eq!(result, i32::MIN);
    }

    #[tokio::test]
    async fn test_passthrough_invoke_float() {
        let passthrough: RunnablePassthrough<f64> = RunnablePassthrough::new();
        let pi = std::f64::consts::PI;
        let result = passthrough.invoke(pi, None).await.unwrap();
        assert!((result - pi).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_passthrough_invoke_bool_true() {
        let passthrough: RunnablePassthrough<bool> = RunnablePassthrough::new();
        let result = passthrough.invoke(true, None).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_passthrough_invoke_bool_false() {
        let passthrough: RunnablePassthrough<bool> = RunnablePassthrough::new();
        let result = passthrough.invoke(false, None).await.unwrap();
        assert!(!result);
    }

    // ============================================
    // RunnablePassthrough Invoke Tests - Strings
    // ============================================

    #[tokio::test]
    async fn test_passthrough_invoke_string() {
        let passthrough: RunnablePassthrough<String> = RunnablePassthrough::new();
        let result = passthrough.invoke("hello world".to_string(), None).await.unwrap();
        assert_eq!(result, "hello world");
    }

    #[tokio::test]
    async fn test_passthrough_invoke_empty_string() {
        let passthrough: RunnablePassthrough<String> = RunnablePassthrough::new();
        let result = passthrough.invoke(String::new(), None).await.unwrap();
        assert_eq!(result, "");
    }

    #[tokio::test]
    async fn test_passthrough_invoke_unicode_string() {
        let passthrough: RunnablePassthrough<String> = RunnablePassthrough::new();
        let result = passthrough.invoke("你好世界🌍".to_string(), None).await.unwrap();
        assert_eq!(result, "你好世界🌍");
    }

    #[tokio::test]
    async fn test_passthrough_invoke_long_string() {
        let passthrough: RunnablePassthrough<String> = RunnablePassthrough::new();
        let long_str = "x".repeat(10000);
        let result = passthrough.invoke(long_str.clone(), None).await.unwrap();
        assert_eq!(result, long_str);
    }

    // ============================================
    // RunnablePassthrough Invoke Tests - Collections
    // ============================================

    #[tokio::test]
    async fn test_passthrough_invoke_vec() {
        let passthrough: RunnablePassthrough<Vec<i32>> = RunnablePassthrough::new();
        let result = passthrough.invoke(vec![1, 2, 3], None).await.unwrap();
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_passthrough_invoke_empty_vec() {
        let passthrough: RunnablePassthrough<Vec<i32>> = RunnablePassthrough::new();
        let result = passthrough.invoke(vec![], None).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_passthrough_invoke_vec_strings() {
        let passthrough: RunnablePassthrough<Vec<String>> = RunnablePassthrough::new();
        let result = passthrough.invoke(vec!["a".to_string(), "b".to_string()], None).await.unwrap();
        assert_eq!(result, vec!["a", "b"]);
    }

    // ============================================
    // RunnablePassthrough Invoke Tests - HashMap
    // ============================================

    #[tokio::test]
    async fn test_passthrough_invoke_hashmap() {
        let passthrough: RunnablePassthrough<HashMap<String, serde_json::Value>> = RunnablePassthrough::new();
        let mut input = HashMap::new();
        input.insert("key".to_string(), serde_json::json!("value"));
        let result = passthrough.invoke(input.clone(), None).await.unwrap();
        assert_eq!(result, input);
    }

    #[tokio::test]
    async fn test_passthrough_invoke_empty_hashmap() {
        let passthrough: RunnablePassthrough<HashMap<String, serde_json::Value>> = RunnablePassthrough::new();
        let input: HashMap<String, serde_json::Value> = HashMap::new();
        let result = passthrough.invoke(input.clone(), None).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_passthrough_invoke_hashmap_multiple_keys() {
        let passthrough: RunnablePassthrough<HashMap<String, serde_json::Value>> = RunnablePassthrough::new();
        let mut input = HashMap::new();
        input.insert("name".to_string(), serde_json::json!("John"));
        input.insert("age".to_string(), serde_json::json!(30));
        input.insert("city".to_string(), serde_json::json!("NYC"));
        let result = passthrough.invoke(input.clone(), None).await.unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result.get("name"), input.get("name"));
        assert_eq!(result.get("age"), input.get("age"));
        assert_eq!(result.get("city"), input.get("city"));
    }

    // ============================================
    // RunnablePassthrough with Config Tests
    // ============================================

    #[tokio::test]
    async fn test_passthrough_invoke_with_config() {
        let passthrough: RunnablePassthrough<i32> = RunnablePassthrough::new();
        let config = RunnableConfig::default();
        let result = passthrough.invoke(42, Some(config)).await.unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_passthrough_invoke_with_tags() {
        let passthrough: RunnablePassthrough<i32> = RunnablePassthrough::new();
        let mut config = RunnableConfig::default();
        config.tags.push("test-tag".to_string());
        let result = passthrough.invoke(42, Some(config)).await.unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_passthrough_invoke_with_metadata() {
        let passthrough: RunnablePassthrough<i32> = RunnablePassthrough::new();
        let mut config = RunnableConfig::default();
        config.metadata.insert("key".to_string(), serde_json::json!("value"));
        let result = passthrough.invoke(42, Some(config)).await.unwrap();
        assert_eq!(result, 42);
    }

    // ============================================
    // RunnablePassthrough Multiple Invocations
    // ============================================

    #[tokio::test]
    async fn test_passthrough_multiple_invocations() {
        let passthrough: RunnablePassthrough<i32> = RunnablePassthrough::new();
        for i in 0..10 {
            let result = passthrough.invoke(i, None).await.unwrap();
            assert_eq!(result, i);
        }
    }

    // ============================================
    // RunnablePassthrough::pick Tests
    // ============================================

    #[test]
    fn test_passthrough_pick_creates_runnable_pick() {
        let pick = RunnablePassthrough::pick(vec!["name".to_string()]);
        assert!(pick.name().contains("name"));
    }

    #[test]
    fn test_passthrough_pick_multiple_keys() {
        let pick = RunnablePassthrough::pick(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        assert!(pick.name().contains("a"));
        assert!(pick.name().contains("b"));
        assert!(pick.name().contains("c"));
    }

    // ============================================
    // RunnablePick Construction Tests
    // ============================================

    #[test]
    fn test_pick_new() {
        let pick = RunnablePick::new(vec!["key1".to_string(), "key2".to_string()]);
        assert_eq!(pick.keys(), &["key1".to_string(), "key2".to_string()]);
    }

    #[test]
    fn test_pick_new_empty() {
        let pick = RunnablePick::new(vec![]);
        assert!(pick.keys().is_empty());
    }

    #[test]
    fn test_pick_name() {
        let pick = RunnablePick::new(vec!["a".to_string(), "b".to_string()]);
        assert_eq!(pick.name(), "RunnablePick<a,b>");
    }

    #[test]
    fn test_pick_name_single_key() {
        let pick = RunnablePick::new(vec!["name".to_string()]);
        assert_eq!(pick.name(), "RunnablePick<name>");
    }

    #[test]
    fn test_pick_name_empty() {
        let pick = RunnablePick::new(vec![]);
        assert_eq!(pick.name(), "RunnablePick<>");
    }

    #[test]
    fn test_pick_keys() {
        let pick = RunnablePick::new(vec!["x".to_string(), "y".to_string(), "z".to_string()]);
        assert_eq!(pick.keys(), &["x", "y", "z"]);
    }

    // ============================================
    // RunnablePick Invoke Tests
    // ============================================

    #[tokio::test]
    async fn test_pick_invoke_single_key() {
        let pick = RunnablePick::new(vec!["name".to_string()]);
        let mut input = HashMap::new();
        input.insert("name".to_string(), serde_json::json!("John"));
        input.insert("age".to_string(), serde_json::json!(30));

        let result = pick.invoke(input, None).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("name"), Some(&serde_json::json!("John")));
    }

    #[tokio::test]
    async fn test_pick_invoke_multiple_keys() {
        let pick = RunnablePick::new(vec!["name".to_string(), "age".to_string()]);
        let mut input = HashMap::new();
        input.insert("name".to_string(), serde_json::json!("John"));
        input.insert("age".to_string(), serde_json::json!(30));
        input.insert("city".to_string(), serde_json::json!("NYC"));

        let result = pick.invoke(input, None).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result.get("name"), Some(&serde_json::json!("John")));
        assert_eq!(result.get("age"), Some(&serde_json::json!(30)));
        assert!(!result.contains_key("city"));
    }

    #[tokio::test]
    async fn test_pick_invoke_missing_key() {
        let pick = RunnablePick::new(vec!["name".to_string(), "missing".to_string()]);
        let mut input = HashMap::new();
        input.insert("name".to_string(), serde_json::json!("John"));

        let result = pick.invoke(input, None).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("name"), Some(&serde_json::json!("John")));
        assert!(!result.contains_key("missing"));
    }

    #[tokio::test]
    async fn test_pick_invoke_all_keys_missing() {
        let pick = RunnablePick::new(vec!["missing1".to_string(), "missing2".to_string()]);
        let mut input = HashMap::new();
        input.insert("name".to_string(), serde_json::json!("John"));

        let result = pick.invoke(input, None).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_pick_invoke_empty_input() {
        let pick = RunnablePick::new(vec!["name".to_string()]);
        let input: HashMap<String, serde_json::Value> = HashMap::new();

        let result = pick.invoke(input, None).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_pick_invoke_empty_keys() {
        let pick = RunnablePick::new(vec![]);
        let mut input = HashMap::new();
        input.insert("name".to_string(), serde_json::json!("John"));

        let result = pick.invoke(input, None).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_pick_invoke_all_keys() {
        let pick = RunnablePick::new(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        let mut input = HashMap::new();
        input.insert("a".to_string(), serde_json::json!(1));
        input.insert("b".to_string(), serde_json::json!(2));
        input.insert("c".to_string(), serde_json::json!(3));

        let result = pick.invoke(input, None).await.unwrap();
        assert_eq!(result.len(), 3);
    }

    // ============================================
    // RunnablePick Invoke Tests - Value Types
    // ============================================

    #[tokio::test]
    async fn test_pick_invoke_string_value() {
        let pick = RunnablePick::new(vec!["name".to_string()]);
        let mut input = HashMap::new();
        input.insert("name".to_string(), serde_json::json!("Hello World"));

        let result = pick.invoke(input, None).await.unwrap();
        assert_eq!(result.get("name"), Some(&serde_json::json!("Hello World")));
    }

    #[tokio::test]
    async fn test_pick_invoke_number_value() {
        let pick = RunnablePick::new(vec!["count".to_string()]);
        let mut input = HashMap::new();
        input.insert("count".to_string(), serde_json::json!(42));

        let result = pick.invoke(input, None).await.unwrap();
        assert_eq!(result.get("count"), Some(&serde_json::json!(42)));
    }

    #[tokio::test]
    async fn test_pick_invoke_float_value() {
        let pick = RunnablePick::new(vec!["pi".to_string()]);
        let mut input = HashMap::new();
        let pi = std::f64::consts::PI;
        input.insert("pi".to_string(), serde_json::json!(pi));

        let result = pick.invoke(input, None).await.unwrap();
        assert_eq!(result.get("pi"), Some(&serde_json::json!(pi)));
    }

    #[tokio::test]
    async fn test_pick_invoke_bool_value() {
        let pick = RunnablePick::new(vec!["active".to_string()]);
        let mut input = HashMap::new();
        input.insert("active".to_string(), serde_json::json!(true));

        let result = pick.invoke(input, None).await.unwrap();
        assert_eq!(result.get("active"), Some(&serde_json::json!(true)));
    }

    #[tokio::test]
    async fn test_pick_invoke_null_value() {
        let pick = RunnablePick::new(vec!["data".to_string()]);
        let mut input = HashMap::new();
        input.insert("data".to_string(), serde_json::json!(null));

        let result = pick.invoke(input, None).await.unwrap();
        assert_eq!(result.get("data"), Some(&serde_json::json!(null)));
    }

    #[tokio::test]
    async fn test_pick_invoke_array_value() {
        let pick = RunnablePick::new(vec!["items".to_string()]);
        let mut input = HashMap::new();
        input.insert("items".to_string(), serde_json::json!([1, 2, 3]));

        let result = pick.invoke(input, None).await.unwrap();
        assert_eq!(result.get("items"), Some(&serde_json::json!([1, 2, 3])));
    }

    #[tokio::test]
    async fn test_pick_invoke_object_value() {
        let pick = RunnablePick::new(vec!["nested".to_string()]);
        let mut input = HashMap::new();
        input.insert("nested".to_string(), serde_json::json!({"a": 1, "b": 2}));

        let result = pick.invoke(input, None).await.unwrap();
        assert_eq!(result.get("nested"), Some(&serde_json::json!({"a": 1, "b": 2})));
    }

    // ============================================
    // RunnablePick with Config Tests
    // ============================================

    #[tokio::test]
    async fn test_pick_invoke_with_config() {
        let pick = RunnablePick::new(vec!["name".to_string()]);
        let mut input = HashMap::new();
        input.insert("name".to_string(), serde_json::json!("John"));

        let config = RunnableConfig::default();
        let result = pick.invoke(input, Some(config)).await.unwrap();
        assert_eq!(result.get("name"), Some(&serde_json::json!("John")));
    }

    #[tokio::test]
    async fn test_pick_invoke_with_tags() {
        let pick = RunnablePick::new(vec!["name".to_string()]);
        let mut input = HashMap::new();
        input.insert("name".to_string(), serde_json::json!("John"));

        let mut config = RunnableConfig::default();
        config.tags.push("test-tag".to_string());
        let result = pick.invoke(input, Some(config)).await.unwrap();
        assert_eq!(result.len(), 1);
    }

    // ============================================
    // RunnablePick Edge Cases
    // ============================================

    #[tokio::test]
    async fn test_pick_invoke_duplicate_keys() {
        // Duplicate keys in the pick list should still work
        let pick = RunnablePick::new(vec!["name".to_string(), "name".to_string()]);
        let mut input = HashMap::new();
        input.insert("name".to_string(), serde_json::json!("John"));

        let result = pick.invoke(input, None).await.unwrap();
        // Result should have only one "name" key since it's a HashMap
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_pick_invoke_unicode_keys() {
        let pick = RunnablePick::new(vec!["名前".to_string(), "年齢".to_string()]);
        let mut input = HashMap::new();
        input.insert("名前".to_string(), serde_json::json!("田中"));
        input.insert("年齢".to_string(), serde_json::json!(25));
        input.insert("住所".to_string(), serde_json::json!("東京"));

        let result = pick.invoke(input, None).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result.get("名前"), Some(&serde_json::json!("田中")));
        assert_eq!(result.get("年齢"), Some(&serde_json::json!(25)));
    }

    #[tokio::test]
    async fn test_pick_invoke_special_char_keys() {
        let pick = RunnablePick::new(vec!["key.with.dots".to_string(), "key-with-dashes".to_string()]);
        let mut input = HashMap::new();
        input.insert("key.with.dots".to_string(), serde_json::json!("value1"));
        input.insert("key-with-dashes".to_string(), serde_json::json!("value2"));

        let result = pick.invoke(input, None).await.unwrap();
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn test_pick_invoke_case_sensitive_keys() {
        let pick = RunnablePick::new(vec!["Name".to_string()]);
        let mut input = HashMap::new();
        input.insert("name".to_string(), serde_json::json!("lowercase"));
        input.insert("Name".to_string(), serde_json::json!("uppercase"));

        let result = pick.invoke(input, None).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("Name"), Some(&serde_json::json!("uppercase")));
    }

    // ============================================
    // RunnablePick Multiple Invocations
    // ============================================

    #[tokio::test]
    async fn test_pick_multiple_invocations() {
        let pick = RunnablePick::new(vec!["x".to_string()]);

        for i in 0..5 {
            let mut input = HashMap::new();
            input.insert("x".to_string(), serde_json::json!(i));
            input.insert("y".to_string(), serde_json::json!(i * 2));

            let result = pick.invoke(input, None).await.unwrap();
            assert_eq!(result.get("x"), Some(&serde_json::json!(i)));
            assert!(!result.contains_key("y"));
        }
    }

    // ============================================
    // RunnablePick Large Input Tests
    // ============================================

    #[tokio::test]
    async fn test_pick_invoke_large_input() {
        let pick = RunnablePick::new(vec!["key_5".to_string(), "key_50".to_string()]);
        let mut input = HashMap::new();
        for i in 0..100 {
            input.insert(format!("key_{}", i), serde_json::json!(i));
        }

        let result = pick.invoke(input, None).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result.get("key_5"), Some(&serde_json::json!(5)));
        assert_eq!(result.get("key_50"), Some(&serde_json::json!(50)));
    }

    #[tokio::test]
    async fn test_pick_invoke_many_keys() {
        let keys: Vec<String> = (0..50).map(|i| format!("key_{}", i)).collect();
        let pick = RunnablePick::new(keys);
        let mut input = HashMap::new();
        for i in 0..100 {
            input.insert(format!("key_{}", i), serde_json::json!(i));
        }

        let result = pick.invoke(input, None).await.unwrap();
        assert_eq!(result.len(), 50);
    }
}
