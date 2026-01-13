//! RunnableLambda - A Runnable that wraps a function
//!
//! This module provides `RunnableLambda`, which creates Runnables from
//! closures or functions for simple transformations.

use async_trait::async_trait;
use std::collections::HashMap;

use crate::core::config::RunnableConfig;
use crate::core::error::Result;

use super::Runnable;

/// A Runnable that wraps a function
///
/// Useful for creating simple Runnables from closures or functions.
pub struct RunnableLambda<F, Input, Output>
where
    F: Fn(Input) -> Output,
{
    func: F,
    name: String,
    _phantom: std::marker::PhantomData<(Input, Output)>,
}

impl<F, Input, Output> RunnableLambda<F, Input, Output>
where
    F: Fn(Input) -> Output,
{
    /// Create a new `RunnableLambda`
    pub fn new(func: F) -> Self {
        Self {
            func,
            name: "Lambda".to_string(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Create a new `RunnableLambda` with a custom name
    #[must_use]
    pub fn with_name(func: F, name: impl Into<String>) -> Self {
        Self {
            func,
            name: name.into(),
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<F, Input, Output> Runnable for RunnableLambda<F, Input, Output>
where
    F: Fn(Input) -> Output + Send + Sync,
    Input: Send + Sync,
    Output: Send + Sync,
{
    type Input = Input;
    type Output = Output;

    fn name(&self) -> String {
        self.name.clone()
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
                &HashMap::new(),
                run_id,
                None,
                &config.tags,
                &config.metadata,
            )
            .await?;

        // Execute function
        let result = (self.func)(input);

        // End chain
        callback_manager
            .on_chain_end(&HashMap::new(), run_id, None)
            .await?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // RunnableLambda Construction Tests
    // ============================================

    #[test]
    fn test_lambda_new() {
        let lambda = RunnableLambda::new(|x: i32| x + 1);
        assert_eq!(lambda.name(), "Lambda");
    }

    #[test]
    fn test_lambda_with_name() {
        let lambda = RunnableLambda::with_name(|x: i32| x + 1, "AddOne");
        assert_eq!(lambda.name(), "AddOne");
    }

    #[test]
    fn test_lambda_with_name_string() {
        let lambda = RunnableLambda::with_name(|x: i32| x * 2, String::from("Doubler"));
        assert_eq!(lambda.name(), "Doubler");
    }

    #[test]
    fn test_lambda_with_empty_name() {
        let lambda = RunnableLambda::with_name(|x: i32| x, "");
        assert_eq!(lambda.name(), "");
    }

    #[test]
    fn test_lambda_with_unicode_name() {
        let lambda = RunnableLambda::with_name(|x: i32| x, "计算器🔢");
        assert_eq!(lambda.name(), "计算器🔢");
    }

    // ============================================
    // RunnableLambda Invoke Tests - Basic Types
    // ============================================

    #[tokio::test]
    async fn test_lambda_invoke_identity() {
        let lambda = RunnableLambda::new(|x: i32| x);
        let result = lambda.invoke(42, None).await.unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_lambda_invoke_add() {
        let lambda = RunnableLambda::new(|x: i32| x + 10);
        let result = lambda.invoke(5, None).await.unwrap();
        assert_eq!(result, 15);
    }

    #[tokio::test]
    async fn test_lambda_invoke_multiply() {
        let lambda = RunnableLambda::new(|x: i32| x * 3);
        let result = lambda.invoke(7, None).await.unwrap();
        assert_eq!(result, 21);
    }

    #[tokio::test]
    async fn test_lambda_invoke_subtract() {
        let lambda = RunnableLambda::new(|x: i32| x - 5);
        let result = lambda.invoke(10, None).await.unwrap();
        assert_eq!(result, 5);
    }

    #[tokio::test]
    async fn test_lambda_invoke_divide() {
        let lambda = RunnableLambda::new(|x: i32| x / 2);
        let result = lambda.invoke(10, None).await.unwrap();
        assert_eq!(result, 5);
    }

    #[tokio::test]
    async fn test_lambda_invoke_modulo() {
        let lambda = RunnableLambda::new(|x: i32| x % 3);
        let result = lambda.invoke(10, None).await.unwrap();
        assert_eq!(result, 1);
    }

    #[tokio::test]
    async fn test_lambda_invoke_negate() {
        let lambda = RunnableLambda::new(|x: i32| -x);
        let result = lambda.invoke(42, None).await.unwrap();
        assert_eq!(result, -42);
    }

    #[tokio::test]
    async fn test_lambda_invoke_abs() {
        let lambda = RunnableLambda::new(|x: i32| x.abs());
        let result = lambda.invoke(-42, None).await.unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_lambda_invoke_zero() {
        let lambda = RunnableLambda::new(|x: i32| x * 2);
        let result = lambda.invoke(0, None).await.unwrap();
        assert_eq!(result, 0);
    }

    #[tokio::test]
    async fn test_lambda_invoke_negative() {
        let lambda = RunnableLambda::new(|x: i32| x + 100);
        let result = lambda.invoke(-50, None).await.unwrap();
        assert_eq!(result, 50);
    }

    #[tokio::test]
    async fn test_lambda_invoke_max_int() {
        let lambda = RunnableLambda::new(|x: i32| x);
        let result = lambda.invoke(i32::MAX, None).await.unwrap();
        assert_eq!(result, i32::MAX);
    }

    #[tokio::test]
    async fn test_lambda_invoke_min_int() {
        let lambda = RunnableLambda::new(|x: i32| x);
        let result = lambda.invoke(i32::MIN, None).await.unwrap();
        assert_eq!(result, i32::MIN);
    }

    // ============================================
    // RunnableLambda Invoke Tests - Floating Point
    // ============================================

    #[tokio::test]
    async fn test_lambda_invoke_float() {
        let lambda = RunnableLambda::new(|x: f64| x * 2.5);
        let result = lambda.invoke(4.0, None).await.unwrap();
        assert!((result - 10.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_lambda_invoke_float_sqrt() {
        let lambda = RunnableLambda::new(|x: f64| x.sqrt());
        let result = lambda.invoke(16.0, None).await.unwrap();
        assert!((result - 4.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_lambda_invoke_float_round() {
        let lambda = RunnableLambda::new(|x: f64| x.round());
        let result = lambda.invoke(3.7, None).await.unwrap();
        assert!((result - 4.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_lambda_invoke_float_floor() {
        let lambda = RunnableLambda::new(|x: f64| x.floor());
        let result = lambda.invoke(3.9, None).await.unwrap();
        assert!((result - 3.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_lambda_invoke_float_ceil() {
        let lambda = RunnableLambda::new(|x: f64| x.ceil());
        let result = lambda.invoke(3.1, None).await.unwrap();
        assert!((result - 4.0).abs() < f64::EPSILON);
    }

    // ============================================
    // RunnableLambda Invoke Tests - Strings
    // ============================================

    #[tokio::test]
    async fn test_lambda_invoke_string_identity() {
        let lambda = RunnableLambda::new(|s: String| s);
        let result = lambda.invoke("hello".to_string(), None).await.unwrap();
        assert_eq!(result, "hello");
    }

    #[tokio::test]
    async fn test_lambda_invoke_string_uppercase() {
        let lambda = RunnableLambda::new(|s: String| s.to_uppercase());
        let result = lambda.invoke("hello".to_string(), None).await.unwrap();
        assert_eq!(result, "HELLO");
    }

    #[tokio::test]
    async fn test_lambda_invoke_string_lowercase() {
        let lambda = RunnableLambda::new(|s: String| s.to_lowercase());
        let result = lambda.invoke("WORLD".to_string(), None).await.unwrap();
        assert_eq!(result, "world");
    }

    #[tokio::test]
    async fn test_lambda_invoke_string_reverse() {
        let lambda = RunnableLambda::new(|s: String| s.chars().rev().collect::<String>());
        let result = lambda.invoke("hello".to_string(), None).await.unwrap();
        assert_eq!(result, "olleh");
    }

    #[tokio::test]
    async fn test_lambda_invoke_string_len() {
        let lambda = RunnableLambda::new(|s: String| s.len());
        let result = lambda.invoke("hello world".to_string(), None).await.unwrap();
        assert_eq!(result, 11);
    }

    #[tokio::test]
    async fn test_lambda_invoke_string_trim() {
        let lambda = RunnableLambda::new(|s: String| s.trim().to_string());
        let result = lambda.invoke("  hello  ".to_string(), None).await.unwrap();
        assert_eq!(result, "hello");
    }

    #[tokio::test]
    async fn test_lambda_invoke_string_concat() {
        let lambda = RunnableLambda::new(|s: String| format!("{} world", s));
        let result = lambda.invoke("hello".to_string(), None).await.unwrap();
        assert_eq!(result, "hello world");
    }

    #[tokio::test]
    async fn test_lambda_invoke_string_replace() {
        let lambda = RunnableLambda::new(|s: String| s.replace("hello", "hi"));
        let result = lambda.invoke("hello world".to_string(), None).await.unwrap();
        assert_eq!(result, "hi world");
    }

    #[tokio::test]
    async fn test_lambda_invoke_empty_string() {
        let lambda = RunnableLambda::new(|s: String| s.is_empty());
        let result = lambda.invoke(String::new(), None).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_lambda_invoke_string_unicode() {
        let lambda = RunnableLambda::new(|s: String| s.chars().count());
        let result = lambda.invoke("你好世界🌍".to_string(), None).await.unwrap();
        assert_eq!(result, 5);
    }

    // ============================================
    // RunnableLambda Invoke Tests - Type Transformations
    // ============================================

    #[tokio::test]
    async fn test_lambda_invoke_int_to_string() {
        let lambda = RunnableLambda::new(|x: i32| x.to_string());
        let result = lambda.invoke(42, None).await.unwrap();
        assert_eq!(result, "42");
    }

    #[tokio::test]
    async fn test_lambda_invoke_string_to_int() {
        let lambda = RunnableLambda::new(|s: String| s.parse::<i32>().unwrap_or(0));
        let result = lambda.invoke("42".to_string(), None).await.unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_lambda_invoke_int_to_float() {
        let lambda = RunnableLambda::new(|x: i32| x as f64);
        let result = lambda.invoke(42, None).await.unwrap();
        assert!((result - 42.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_lambda_invoke_float_to_int() {
        let lambda = RunnableLambda::new(|x: f64| x as i32);
        let result = lambda.invoke(42.7, None).await.unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_lambda_invoke_int_to_bool() {
        let lambda = RunnableLambda::new(|x: i32| x != 0);
        let result = lambda.invoke(1, None).await.unwrap();
        assert!(result);

        let result2 = lambda.invoke(0, None).await.unwrap();
        assert!(!result2);
    }

    #[tokio::test]
    async fn test_lambda_invoke_bool_to_int() {
        let lambda = RunnableLambda::new(|b: bool| if b { 1 } else { 0 });
        let result = lambda.invoke(true, None).await.unwrap();
        assert_eq!(result, 1);

        let result2 = lambda.invoke(false, None).await.unwrap();
        assert_eq!(result2, 0);
    }

    // ============================================
    // RunnableLambda Invoke Tests - Collections
    // ============================================

    #[tokio::test]
    async fn test_lambda_invoke_vec_sum() {
        let lambda = RunnableLambda::new(|v: Vec<i32>| v.iter().sum::<i32>());
        let result = lambda.invoke(vec![1, 2, 3, 4, 5], None).await.unwrap();
        assert_eq!(result, 15);
    }

    #[tokio::test]
    async fn test_lambda_invoke_vec_len() {
        let lambda = RunnableLambda::new(|v: Vec<i32>| v.len());
        let result = lambda.invoke(vec![1, 2, 3], None).await.unwrap();
        assert_eq!(result, 3);
    }

    #[tokio::test]
    async fn test_lambda_invoke_vec_max() {
        let lambda = RunnableLambda::new(|v: Vec<i32>| *v.iter().max().unwrap_or(&0));
        let result = lambda.invoke(vec![1, 5, 3, 9, 2], None).await.unwrap();
        assert_eq!(result, 9);
    }

    #[tokio::test]
    async fn test_lambda_invoke_vec_min() {
        let lambda = RunnableLambda::new(|v: Vec<i32>| *v.iter().min().unwrap_or(&0));
        let result = lambda.invoke(vec![1, 5, 3, 9, 2], None).await.unwrap();
        assert_eq!(result, 1);
    }

    #[tokio::test]
    async fn test_lambda_invoke_vec_filter() {
        let lambda = RunnableLambda::new(|v: Vec<i32>| v.into_iter().filter(|x| *x > 3).collect::<Vec<_>>());
        let result = lambda.invoke(vec![1, 5, 3, 9, 2], None).await.unwrap();
        assert_eq!(result, vec![5, 9]);
    }

    #[tokio::test]
    async fn test_lambda_invoke_vec_map() {
        let lambda = RunnableLambda::new(|v: Vec<i32>| v.into_iter().map(|x| x * 2).collect::<Vec<_>>());
        let result = lambda.invoke(vec![1, 2, 3], None).await.unwrap();
        assert_eq!(result, vec![2, 4, 6]);
    }

    #[tokio::test]
    async fn test_lambda_invoke_vec_sort() {
        let lambda = RunnableLambda::new(|mut v: Vec<i32>| {
            v.sort();
            v
        });
        let result = lambda.invoke(vec![3, 1, 4, 1, 5], None).await.unwrap();
        assert_eq!(result, vec![1, 1, 3, 4, 5]);
    }

    #[tokio::test]
    async fn test_lambda_invoke_vec_reverse() {
        let lambda = RunnableLambda::new(|mut v: Vec<i32>| {
            v.reverse();
            v
        });
        let result = lambda.invoke(vec![1, 2, 3], None).await.unwrap();
        assert_eq!(result, vec![3, 2, 1]);
    }

    #[tokio::test]
    async fn test_lambda_invoke_empty_vec() {
        let lambda = RunnableLambda::new(|v: Vec<i32>| v.is_empty());
        let result = lambda.invoke(vec![], None).await.unwrap();
        assert!(result);
    }

    // ============================================
    // RunnableLambda Invoke Tests - Option/Result
    // ============================================

    #[tokio::test]
    async fn test_lambda_invoke_option_some() {
        let lambda = RunnableLambda::new(|o: Option<i32>| o.unwrap_or(-1));
        let result = lambda.invoke(Some(42), None).await.unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_lambda_invoke_option_none() {
        let lambda = RunnableLambda::new(|o: Option<i32>| o.unwrap_or(-1));
        let result = lambda.invoke(None, None).await.unwrap();
        assert_eq!(result, -1);
    }

    #[tokio::test]
    async fn test_lambda_invoke_option_map() {
        let lambda = RunnableLambda::new(|o: Option<i32>| o.map(|x| x * 2));
        let result = lambda.invoke(Some(21), None).await.unwrap();
        assert_eq!(result, Some(42));
    }

    #[tokio::test]
    async fn test_lambda_invoke_option_is_some() {
        let lambda = RunnableLambda::new(|o: Option<i32>| o.is_some());
        let result = lambda.invoke(Some(42), None).await.unwrap();
        assert!(result);

        let result2 = lambda.invoke(None, None).await.unwrap();
        assert!(!result2);
    }

    // ============================================
    // RunnableLambda with Config Tests
    // ============================================

    #[tokio::test]
    async fn test_lambda_invoke_with_config() {
        let lambda = RunnableLambda::new(|x: i32| x * 2);
        let config = RunnableConfig::default();
        let result = lambda.invoke(21, Some(config)).await.unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_lambda_invoke_with_tags() {
        let lambda = RunnableLambda::new(|x: i32| x);
        let mut config = RunnableConfig::default();
        config.tags.push("test-tag".to_string());
        let result = lambda.invoke(42, Some(config)).await.unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_lambda_invoke_with_metadata() {
        let lambda = RunnableLambda::new(|x: i32| x);
        let mut config = RunnableConfig::default();
        config.metadata.insert("key".to_string(), serde_json::json!("value"));
        let result = lambda.invoke(42, Some(config)).await.unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_lambda_invoke_with_run_name() {
        let lambda = RunnableLambda::new(|x: i32| x);
        let mut config = RunnableConfig::default();
        config.run_name = Some("test-run".to_string());
        let result = lambda.invoke(42, Some(config)).await.unwrap();
        assert_eq!(result, 42);
    }

    // ============================================
    // RunnableLambda Edge Cases
    // ============================================

    #[tokio::test]
    async fn test_lambda_invoke_constant() {
        let lambda = RunnableLambda::new(|_: i32| 42);
        let result = lambda.invoke(0, None).await.unwrap();
        assert_eq!(result, 42);

        let result2 = lambda.invoke(100, None).await.unwrap();
        assert_eq!(result2, 42);
    }

    #[tokio::test]
    async fn test_lambda_invoke_unit_input() {
        let lambda = RunnableLambda::new(|_: ()| 42);
        let result = lambda.invoke((), None).await.unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_lambda_invoke_unit_output() {
        let lambda = RunnableLambda::new(|_: i32| ());
        let result = lambda.invoke(42, None).await.unwrap();
        assert_eq!(result, ());
    }

    #[tokio::test]
    async fn test_lambda_invoke_tuple_input() {
        let lambda = RunnableLambda::new(|(a, b): (i32, i32)| a + b);
        let result = lambda.invoke((10, 20), None).await.unwrap();
        assert_eq!(result, 30);
    }

    #[tokio::test]
    async fn test_lambda_invoke_tuple_output() {
        let lambda = RunnableLambda::new(|x: i32| (x, x * 2));
        let result = lambda.invoke(5, None).await.unwrap();
        assert_eq!(result, (5, 10));
    }

    #[tokio::test]
    async fn test_lambda_invoke_complex_transformation() {
        let lambda = RunnableLambda::new(|v: Vec<i32>| {
            let sum: i32 = v.iter().sum();
            let avg = if v.is_empty() { 0.0 } else { sum as f64 / v.len() as f64 };
            format!("sum={}, avg={:.2}", sum, avg)
        });
        let result = lambda.invoke(vec![1, 2, 3, 4, 5], None).await.unwrap();
        assert_eq!(result, "sum=15, avg=3.00");
    }

    // ============================================
    // RunnableLambda Multiple Invocations
    // ============================================

    #[tokio::test]
    async fn test_lambda_multiple_invocations() {
        let lambda = RunnableLambda::new(|x: i32| x * 2);

        for i in 0..10 {
            let result = lambda.invoke(i, None).await.unwrap();
            assert_eq!(result, i * 2);
        }
    }

    #[tokio::test]
    async fn test_lambda_concurrent_invocations() {
        use std::sync::Arc;

        let lambda = Arc::new(RunnableLambda::new(|x: i32| x * 2));

        let mut handles = vec![];
        for i in 0..5 {
            let lambda_clone = Arc::clone(&lambda);
            handles.push(tokio::spawn(async move {
                lambda_clone.invoke(i, None).await.unwrap()
            }));
        }

        let mut results = vec![];
        for handle in handles {
            results.push(handle.await.unwrap());
        }
        results.sort();

        assert_eq!(results, vec![0, 2, 4, 6, 8]);
    }
}
