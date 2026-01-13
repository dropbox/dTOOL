//! Router functionality for conditional execution paths
//!
//! This module provides:
//! - `RouterInput`: Input type specifying which runnable to route to
//! - `RouterRunnable`: Routes to different runnables based on a key

use async_trait::async_trait;
use futures::stream::Stream;
use futures::StreamExt;
use std::collections::HashMap;
use std::pin::Pin;

use super::Runnable;
use crate::core::config::RunnableConfig;
use crate::core::error::{Error, Result};

/// Input type for `RouterRunnable`
///
/// Specifies which runnable to route to and what input to pass.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RouterInput<T> {
    /// The key identifying which runnable to route to
    pub key: String,
    /// The input to pass to the selected runnable
    pub input: T,
}

impl<T> RouterInput<T> {
    /// Create a new `RouterInput`
    pub fn new(key: impl Into<String>, input: T) -> Self {
        Self {
            key: key.into(),
            input,
        }
    }
}

/// Runnable that routes to different runnables based on a key.
///
/// `RouterRunnable` allows conditional routing of inputs to different processing
/// paths based on a string key. This is useful for:
///
/// - Different handling strategies based on input type
/// - Multi-model routing (route to different LLMs based on complexity)
/// - A/B testing different implementations
/// - Language-specific processing pipelines
///
/// # Example
///
/// ```rust
/// use dashflow::core::runnable::{Runnable, RunnableLambda, RouterRunnable, RouterInput};
/// use std::collections::HashMap;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Create different processing pipelines
/// let add_one = RunnableLambda::new(|x: i32| x + 1);
/// let square = RunnableLambda::new(|x: i32| x * x);
/// let negate = RunnableLambda::new(|x: i32| -x);
///
/// // Build router
/// let mut runnables = HashMap::new();
/// runnables.insert("add".to_string(), Box::new(add_one) as Box<dyn Runnable<Input = i32, Output = i32> + Send + Sync>);
/// runnables.insert("square".to_string(), Box::new(square) as Box<dyn Runnable<Input = i32, Output = i32> + Send + Sync>);
/// runnables.insert("negate".to_string(), Box::new(negate) as Box<dyn Runnable<Input = i32, Output = i32> + Send + Sync>);
///
/// let router = RouterRunnable::new(runnables);
///
/// // Route to different pipelines
/// let result1 = router.invoke(RouterInput::new("add", 5), None).await?;
/// assert_eq!(result1, 6);
///
/// let result2 = router.invoke(RouterInput::new("square", 5), None).await?;
/// assert_eq!(result2, 25);
///
/// let result3 = router.invoke(RouterInput::new("negate", 5), None).await?;
/// assert_eq!(result3, -5);
/// # Ok(())
/// # }
/// ```
pub struct RouterRunnable<Input, Output>
where
    Input: Send,
    Output: Send,
{
    /// Map of route keys to runnables
    runnables: HashMap<String, Box<dyn Runnable<Input = Input, Output = Output> + Send + Sync>>,
}

impl<Input, Output> RouterRunnable<Input, Output>
where
    Input: Send + 'static,
    Output: Send + 'static,
{
    /// Create a new `RouterRunnable` with the given routes
    ///
    /// # Arguments
    ///
    /// * `runnables` - Map of route keys to runnables
    #[must_use]
    pub fn new(
        runnables: HashMap<String, Box<dyn Runnable<Input = Input, Output = Output> + Send + Sync>>,
    ) -> Self {
        Self { runnables }
    }

    /// Create a new empty `RouterRunnable`
    #[must_use]
    pub fn empty() -> Self {
        Self {
            runnables: HashMap::new(),
        }
    }

    /// Add a route to the router
    ///
    /// # Arguments
    ///
    /// * `key` - The route key
    /// * `runnable` - The runnable to execute for this key
    pub fn add_route(
        &mut self,
        key: impl Into<String>,
        runnable: Box<dyn Runnable<Input = Input, Output = Output> + Send + Sync>,
    ) {
        self.runnables.insert(key.into(), runnable);
    }

    /// Check if a route exists
    #[must_use]
    pub fn has_route(&self, key: &str) -> bool {
        self.runnables.contains_key(key)
    }

    /// Get the number of routes
    #[must_use]
    pub fn route_count(&self) -> usize {
        self.runnables.len()
    }

    /// Get all available route keys
    #[must_use]
    pub fn routes(&self) -> Vec<&String> {
        self.runnables.keys().collect()
    }
}

#[async_trait]
impl<Input, Output> Runnable for RouterRunnable<Input, Output>
where
    Input: Send + Clone + 'static,
    Output: Send + Clone + 'static,
{
    type Input = RouterInput<Input>;
    type Output = Output;

    fn name(&self) -> String {
        let routes: Vec<&str> = self
            .runnables
            .keys()
            .map(std::string::String::as_str)
            .collect();
        format!("RouterRunnable[{} routes]", routes.join(", "))
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
        serialized.insert("route_key".to_string(), serde_json::json!(input.key));

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

        // Look up the runnable
        let runnable = self.runnables.get(&input.key).ok_or_else(|| {
            Error::RunnableExecution(format!(
                "No runnable found for route key '{}'. Available routes: [{}]",
                input.key,
                self.runnables
                    .keys()
                    .map(std::string::String::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?;

        // Execute the selected runnable
        let result = runnable.invoke(input.input, Some(config.clone())).await;

        // End chain or report error
        match &result {
            Ok(_) => {
                callback_manager
                    .on_chain_end(&HashMap::new(), run_id, None)
                    .await?;
            }
            Err(e) => {
                callback_manager
                    .on_chain_error(&e.to_string(), run_id, None)
                    .await?;
            }
        }

        result
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>>
    where
        Self::Input: Clone,
    {
        // Validate all keys exist
        for input in &inputs {
            if !self.runnables.contains_key(&input.key) {
                return Err(Error::RunnableExecution(format!(
                    "No runnable found for route key '{}'. Available routes: [{}]",
                    input.key,
                    self.runnables
                        .keys()
                        .map(std::string::String::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
            }
        }

        // Extract max_concurrency from config if set
        let max_concurrency = config.as_ref().and_then(|c| c.max_concurrency);

        // Process all inputs concurrently
        let tasks: Vec<_> = inputs
            .into_iter()
            .map(|input| {
                let config = config.clone();
                async move { self.invoke(input, config).await }
            })
            .collect();

        // If max_concurrency is set, use bounded concurrency; otherwise run all at once
        let results = if let Some(limit) = max_concurrency {
            futures::stream::iter(tasks)
                .buffer_unordered(limit.max(1))
                .collect::<Vec<_>>()
                .await
        } else {
            futures::future::join_all(tasks).await
        };

        // Collect results, returning early if any failed
        results.into_iter().collect()
    }

    async fn stream(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Self::Output>> + Send + 'static>>>
    where
        Self::Output: Clone + 'static,
    {
        // Look up the runnable
        let runnable = self.runnables.get(&input.key).ok_or_else(|| {
            Error::RunnableExecution(format!(
                "No runnable found for route key '{}'. Available routes: [{}]",
                input.key,
                self.runnables
                    .keys()
                    .map(std::string::String::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?;

        // Delegate streaming to the selected runnable
        runnable.stream(input.input, config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::runnable::RunnableLambda;

    // ============================================
    // RouterInput Construction Tests
    // ============================================

    #[test]
    fn test_router_input_new() {
        let input = RouterInput::new("key", 42);
        assert_eq!(input.key, "key");
        assert_eq!(input.input, 42);
    }

    #[test]
    fn test_router_input_new_with_string() {
        let input = RouterInput::new(String::from("route"), "data");
        assert_eq!(input.key, "route");
        assert_eq!(input.input, "data");
    }

    #[test]
    fn test_router_input_new_empty_key() {
        let input = RouterInput::new("", 42);
        assert_eq!(input.key, "");
        assert_eq!(input.input, 42);
    }

    #[test]
    fn test_router_input_clone() {
        let input = RouterInput::new("key", 42);
        let cloned = input.clone();
        assert_eq!(cloned.key, "key");
        assert_eq!(cloned.input, 42);
    }

    #[test]
    fn test_router_input_debug() {
        let input = RouterInput::new("key", 42);
        let debug_str = format!("{:?}", input);
        assert!(debug_str.contains("key"));
        assert!(debug_str.contains("42"));
    }

    // ============================================
    // RouterRunnable Construction Tests
    // ============================================

    #[test]
    fn test_router_new() {
        let mut runnables: HashMap<String, Box<dyn Runnable<Input = i32, Output = i32> + Send + Sync>> = HashMap::new();
        runnables.insert("add".to_string(), Box::new(RunnableLambda::new(|x: i32| x + 1)));
        let router = RouterRunnable::new(runnables);
        assert_eq!(router.route_count(), 1);
    }

    #[test]
    fn test_router_empty() {
        let router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        assert_eq!(router.route_count(), 0);
    }

    #[test]
    fn test_router_add_route() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("add", Box::new(RunnableLambda::new(|x: i32| x + 1)));
        assert_eq!(router.route_count(), 1);
        assert!(router.has_route("add"));
    }

    #[test]
    fn test_router_add_multiple_routes() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("add", Box::new(RunnableLambda::new(|x: i32| x + 1)));
        router.add_route("multiply", Box::new(RunnableLambda::new(|x: i32| x * 2)));
        router.add_route("negate", Box::new(RunnableLambda::new(|x: i32| -x)));
        assert_eq!(router.route_count(), 3);
    }

    #[test]
    fn test_router_has_route() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("exists", Box::new(RunnableLambda::new(|x: i32| x)));
        assert!(router.has_route("exists"));
        assert!(!router.has_route("missing"));
    }

    #[test]
    fn test_router_route_count() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        assert_eq!(router.route_count(), 0);
        router.add_route("a", Box::new(RunnableLambda::new(|x: i32| x)));
        assert_eq!(router.route_count(), 1);
        router.add_route("b", Box::new(RunnableLambda::new(|x: i32| x)));
        assert_eq!(router.route_count(), 2);
    }

    #[test]
    fn test_router_routes() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("add", Box::new(RunnableLambda::new(|x: i32| x + 1)));
        router.add_route("mul", Box::new(RunnableLambda::new(|x: i32| x * 2)));
        let routes = router.routes();
        assert_eq!(routes.len(), 2);
        assert!(routes.contains(&&"add".to_string()));
        assert!(routes.contains(&&"mul".to_string()));
    }

    #[test]
    fn test_router_replace_route() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("add", Box::new(RunnableLambda::new(|x: i32| x + 1)));
        router.add_route("add", Box::new(RunnableLambda::new(|x: i32| x + 10)));
        assert_eq!(router.route_count(), 1); // Still only one route
    }

    // ============================================
    // RouterRunnable Name Tests
    // ============================================

    #[test]
    fn test_router_name_empty() {
        let router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        let name = router.name();
        assert!(name.contains("RouterRunnable"));
        assert!(name.contains("routes"));
    }

    #[test]
    fn test_router_name_with_routes() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("add", Box::new(RunnableLambda::new(|x: i32| x + 1)));
        router.add_route("mul", Box::new(RunnableLambda::new(|x: i32| x * 2)));
        let name = router.name();
        assert!(name.contains("RouterRunnable"));
    }

    // ============================================
    // RouterRunnable Invoke Tests
    // ============================================

    #[tokio::test]
    async fn test_router_invoke_single_route() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("add", Box::new(RunnableLambda::new(|x: i32| x + 10)));

        let result = router.invoke(RouterInput::new("add", 5), None).await.unwrap();
        assert_eq!(result, 15);
    }

    #[tokio::test]
    async fn test_router_invoke_multiple_routes() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("add", Box::new(RunnableLambda::new(|x: i32| x + 1)));
        router.add_route("square", Box::new(RunnableLambda::new(|x: i32| x * x)));
        router.add_route("negate", Box::new(RunnableLambda::new(|x: i32| -x)));

        let result1 = router.invoke(RouterInput::new("add", 5), None).await.unwrap();
        assert_eq!(result1, 6);

        let result2 = router.invoke(RouterInput::new("square", 5), None).await.unwrap();
        assert_eq!(result2, 25);

        let result3 = router.invoke(RouterInput::new("negate", 5), None).await.unwrap();
        assert_eq!(result3, -5);
    }

    #[tokio::test]
    async fn test_router_invoke_missing_route() {
        let router: RouterRunnable<i32, i32> = RouterRunnable::empty();

        let result = router.invoke(RouterInput::new("missing", 5), None).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("No runnable found for route key 'missing'"));
    }

    #[tokio::test]
    async fn test_router_invoke_error_shows_available_routes() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("add", Box::new(RunnableLambda::new(|x: i32| x + 1)));
        router.add_route("mul", Box::new(RunnableLambda::new(|x: i32| x * 2)));

        let result = router.invoke(RouterInput::new("missing", 5), None).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // Error message should show available routes
        assert!(err_msg.contains("add") || err_msg.contains("mul"));
    }

    #[tokio::test]
    async fn test_router_invoke_with_zero() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("double", Box::new(RunnableLambda::new(|x: i32| x * 2)));

        let result = router.invoke(RouterInput::new("double", 0), None).await.unwrap();
        assert_eq!(result, 0);
    }

    #[tokio::test]
    async fn test_router_invoke_with_negative() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("abs", Box::new(RunnableLambda::new(|x: i32| x.abs())));

        let result = router.invoke(RouterInput::new("abs", -42), None).await.unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_router_invoke_with_string_input() {
        let mut router: RouterRunnable<String, String> = RouterRunnable::empty();
        router.add_route("upper", Box::new(RunnableLambda::new(|s: String| s.to_uppercase())));
        router.add_route("lower", Box::new(RunnableLambda::new(|s: String| s.to_lowercase())));

        let result1 = router.invoke(RouterInput::new("upper", "hello".to_string()), None).await.unwrap();
        assert_eq!(result1, "HELLO");

        let result2 = router.invoke(RouterInput::new("lower", "WORLD".to_string()), None).await.unwrap();
        assert_eq!(result2, "world");
    }

    #[tokio::test]
    async fn test_router_invoke_with_type_transformation() {
        let mut router: RouterRunnable<i32, String> = RouterRunnable::empty();
        router.add_route("stringify", Box::new(RunnableLambda::new(|x: i32| x.to_string())));
        router.add_route("binary", Box::new(RunnableLambda::new(|x: i32| format!("{:b}", x))));

        let result1 = router.invoke(RouterInput::new("stringify", 42), None).await.unwrap();
        assert_eq!(result1, "42");

        let result2 = router.invoke(RouterInput::new("binary", 42), None).await.unwrap();
        assert_eq!(result2, "101010");
    }

    // ============================================
    // RouterRunnable Invoke with Config Tests
    // ============================================

    #[tokio::test]
    async fn test_router_invoke_with_config() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("add", Box::new(RunnableLambda::new(|x: i32| x + 1)));

        let config = RunnableConfig::default();
        let result = router.invoke(RouterInput::new("add", 5), Some(config)).await.unwrap();
        assert_eq!(result, 6);
    }

    #[tokio::test]
    async fn test_router_invoke_with_tags() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("add", Box::new(RunnableLambda::new(|x: i32| x + 1)));

        let mut config = RunnableConfig::default();
        config.tags.push("test-tag".to_string());
        let result = router.invoke(RouterInput::new("add", 5), Some(config)).await.unwrap();
        assert_eq!(result, 6);
    }

    #[tokio::test]
    async fn test_router_invoke_with_metadata() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("add", Box::new(RunnableLambda::new(|x: i32| x + 1)));

        let mut config = RunnableConfig::default();
        config.metadata.insert("key".to_string(), serde_json::json!("value"));
        let result = router.invoke(RouterInput::new("add", 5), Some(config)).await.unwrap();
        assert_eq!(result, 6);
    }

    // ============================================
    // RouterRunnable Batch Tests
    // ============================================

    #[tokio::test]
    async fn test_router_batch_single_route() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("double", Box::new(RunnableLambda::new(|x: i32| x * 2)));

        let inputs = vec![
            RouterInput::new("double", 1),
            RouterInput::new("double", 2),
            RouterInput::new("double", 3),
        ];

        let results = router.batch(inputs, None).await.unwrap();
        assert_eq!(results, vec![2, 4, 6]);
    }

    #[tokio::test]
    async fn test_router_batch_multiple_routes() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("add", Box::new(RunnableLambda::new(|x: i32| x + 1)));
        router.add_route("mul", Box::new(RunnableLambda::new(|x: i32| x * 2)));
        router.add_route("neg", Box::new(RunnableLambda::new(|x: i32| -x)));

        let inputs = vec![
            RouterInput::new("add", 10),
            RouterInput::new("mul", 10),
            RouterInput::new("neg", 10),
        ];

        let results = router.batch(inputs, None).await.unwrap();
        assert_eq!(results, vec![11, 20, -10]);
    }

    #[tokio::test]
    async fn test_router_batch_empty() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("add", Box::new(RunnableLambda::new(|x: i32| x + 1)));

        let inputs: Vec<RouterInput<i32>> = vec![];
        let results = router.batch(inputs, None).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_router_batch_missing_route() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("add", Box::new(RunnableLambda::new(|x: i32| x + 1)));

        let inputs = vec![
            RouterInput::new("add", 1),
            RouterInput::new("missing", 2),
        ];

        let result = router.batch(inputs, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_router_batch_with_max_concurrency() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("add", Box::new(RunnableLambda::new(|x: i32| x + 1)));

        let inputs = vec![
            RouterInput::new("add", 1),
            RouterInput::new("add", 2),
            RouterInput::new("add", 3),
            RouterInput::new("add", 4),
            RouterInput::new("add", 5),
        ];

        let mut config = RunnableConfig::default();
        config.max_concurrency = Some(2);

        let results = router.batch(inputs, Some(config)).await.unwrap();
        assert_eq!(results, vec![2, 3, 4, 5, 6]);
    }

    // ============================================
    // RouterRunnable Edge Cases
    // ============================================

    #[tokio::test]
    async fn test_router_empty_key() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("", Box::new(RunnableLambda::new(|x: i32| x * 2)));

        let result = router.invoke(RouterInput::new("", 5), None).await.unwrap();
        assert_eq!(result, 10);
    }

    #[tokio::test]
    async fn test_router_unicode_key() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("加算", Box::new(RunnableLambda::new(|x: i32| x + 1)));
        router.add_route("乗算", Box::new(RunnableLambda::new(|x: i32| x * 2)));

        let result1 = router.invoke(RouterInput::new("加算", 5), None).await.unwrap();
        assert_eq!(result1, 6);

        let result2 = router.invoke(RouterInput::new("乗算", 5), None).await.unwrap();
        assert_eq!(result2, 10);
    }

    #[tokio::test]
    async fn test_router_special_char_key() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("route-with-dashes", Box::new(RunnableLambda::new(|x: i32| x + 1)));
        router.add_route("route.with.dots", Box::new(RunnableLambda::new(|x: i32| x * 2)));

        let result1 = router.invoke(RouterInput::new("route-with-dashes", 5), None).await.unwrap();
        assert_eq!(result1, 6);

        let result2 = router.invoke(RouterInput::new("route.with.dots", 5), None).await.unwrap();
        assert_eq!(result2, 10);
    }

    #[tokio::test]
    async fn test_router_whitespace_key() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("route with spaces", Box::new(RunnableLambda::new(|x: i32| x + 1)));

        let result = router.invoke(RouterInput::new("route with spaces", 5), None).await.unwrap();
        assert_eq!(result, 6);
    }

    #[tokio::test]
    async fn test_router_case_sensitive_keys() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("add", Box::new(RunnableLambda::new(|x: i32| x + 1)));
        router.add_route("ADD", Box::new(RunnableLambda::new(|x: i32| x + 100)));

        let result1 = router.invoke(RouterInput::new("add", 5), None).await.unwrap();
        assert_eq!(result1, 6);

        let result2 = router.invoke(RouterInput::new("ADD", 5), None).await.unwrap();
        assert_eq!(result2, 105);
    }

    // ============================================
    // RouterRunnable Multiple Invocations
    // ============================================

    #[tokio::test]
    async fn test_router_multiple_invocations_same_route() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("add", Box::new(RunnableLambda::new(|x: i32| x + 1)));

        for i in 0..10 {
            let result = router.invoke(RouterInput::new("add", i), None).await.unwrap();
            assert_eq!(result, i + 1);
        }
    }

    #[tokio::test]
    async fn test_router_multiple_invocations_different_routes() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("add", Box::new(RunnableLambda::new(|x: i32| x + 1)));
        router.add_route("mul", Box::new(RunnableLambda::new(|x: i32| x * 2)));

        let routes = ["add", "mul", "add", "mul", "add"];
        let expected = [6, 10, 6, 10, 6];

        for (route, exp) in routes.iter().zip(expected.iter()) {
            let result = router.invoke(RouterInput::new(*route, 5), None).await.unwrap();
            assert_eq!(result, *exp);
        }
    }

    // ============================================
    // RouterRunnable with Vec Input
    // ============================================

    #[tokio::test]
    async fn test_router_vec_input() {
        let mut router: RouterRunnable<Vec<i32>, i32> = RouterRunnable::empty();
        router.add_route("sum", Box::new(RunnableLambda::new(|v: Vec<i32>| v.iter().sum())));
        router.add_route("len", Box::new(RunnableLambda::new(|v: Vec<i32>| v.len() as i32)));
        router.add_route("max", Box::new(RunnableLambda::new(|v: Vec<i32>| *v.iter().max().unwrap_or(&0))));

        let result1 = router.invoke(RouterInput::new("sum", vec![1, 2, 3, 4, 5]), None).await.unwrap();
        assert_eq!(result1, 15);

        let result2 = router.invoke(RouterInput::new("len", vec![1, 2, 3]), None).await.unwrap();
        assert_eq!(result2, 3);

        let result3 = router.invoke(RouterInput::new("max", vec![3, 1, 4, 1, 5]), None).await.unwrap();
        assert_eq!(result3, 5);
    }

    // ============================================
    // RouterRunnable with HashMap Input
    // ============================================

    #[tokio::test]
    async fn test_router_hashmap_input() {
        let mut router: RouterRunnable<HashMap<String, i32>, i32> = RouterRunnable::empty();
        router.add_route("count", Box::new(RunnableLambda::new(|m: HashMap<String, i32>| m.len() as i32)));
        router.add_route("sum", Box::new(RunnableLambda::new(|m: HashMap<String, i32>| m.values().sum())));

        let mut input = HashMap::new();
        input.insert("a".to_string(), 10);
        input.insert("b".to_string(), 20);
        input.insert("c".to_string(), 30);

        let result1 = router.invoke(RouterInput::new("count", input.clone()), None).await.unwrap();
        assert_eq!(result1, 3);

        let result2 = router.invoke(RouterInput::new("sum", input), None).await.unwrap();
        assert_eq!(result2, 60);
    }

    // ============================================
    // RouterRunnable with Option Output
    // ============================================

    #[tokio::test]
    async fn test_router_option_output() {
        let mut router: RouterRunnable<i32, Option<i32>> = RouterRunnable::empty();
        router.add_route("some", Box::new(RunnableLambda::new(|x: i32| Some(x))));
        router.add_route("none", Box::new(RunnableLambda::new(|_: i32| None)));

        let result1 = router.invoke(RouterInput::new("some", 42), None).await.unwrap();
        assert_eq!(result1, Some(42));

        let result2 = router.invoke(RouterInput::new("none", 42), None).await.unwrap();
        assert_eq!(result2, None);
    }

    // ============================================
    // RouterRunnable Large Scale Tests
    // ============================================

    #[tokio::test]
    async fn test_router_many_routes() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        for i in 0..100 {
            let offset = i;
            router.add_route(format!("route_{}", i), Box::new(RunnableLambda::new(move |x: i32| x + offset)));
        }

        assert_eq!(router.route_count(), 100);

        let result = router.invoke(RouterInput::new("route_50", 10), None).await.unwrap();
        assert_eq!(result, 60);
    }

    #[tokio::test]
    async fn test_router_batch_large() {
        let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
        router.add_route("double", Box::new(RunnableLambda::new(|x: i32| x * 2)));

        let inputs: Vec<RouterInput<i32>> = (0..100)
            .map(|i| RouterInput::new("double", i))
            .collect();

        let results = router.batch(inputs, None).await.unwrap();
        assert_eq!(results.len(), 100);
        for (i, result) in results.iter().enumerate() {
            assert_eq!(*result, i as i32 * 2);
        }
    }
}
