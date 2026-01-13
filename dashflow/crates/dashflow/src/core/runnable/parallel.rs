//! RunnableParallel and RunnableAssign - Parallel execution of runnables
//!
//! Runs multiple runnables in parallel and collects their outputs.

use async_trait::async_trait;
use futures::stream::Stream;
use futures::StreamExt;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use super::graph::{Edge, Graph, Node};
use super::stream_events::{StreamEvent, StreamEventData, StreamEventType, StreamEventsOptions};
use super::Runnable;
use crate::core::config::RunnableConfig;
use crate::core::error::{Error, Result};

/// A Runnable that assigns key-value pairs to `HashMap` inputs
///
/// `RunnableAssign` takes input `HashMaps` and, through a `RunnableParallel` instance,
/// applies transformations, then combines these with the original data, introducing
/// new key-value pairs based on the mapper's logic.
///
/// This is typically created via `RunnablePassthrough::assign()`.
///
/// # Example
///
/// ```rust,ignore
/// use std::collections::HashMap;
/// use dashflow::core::runnable::{RunnableAssign, RunnableParallel, RunnableLambda};
///
/// let mut mapper = RunnableParallel::new();
/// mapper.add("add_step", RunnableLambda::new(|x: HashMap<String, serde_json::Value>| {
///     let input = x.get("input").unwrap().as_i64().unwrap();
///     serde_json::json!({"added": input + 10})
/// }));
///
/// let assign = RunnableAssign::new(mapper);
/// let input = HashMap::from([("input".to_string(), serde_json::json!(5))]);
/// let result = assign.invoke(input, None).await?;
/// // result = {"input": 5, "add_step": {"added": 15}}
/// ```
pub struct RunnableAssign {
    mapper: RunnableParallel<HashMap<String, serde_json::Value>, serde_json::Value>,
}

impl RunnableAssign {
    /// Create a new `RunnableAssign`
    #[must_use]
    pub fn new(
        mapper: RunnableParallel<HashMap<String, serde_json::Value>, serde_json::Value>,
    ) -> Self {
        Self { mapper }
    }
}

#[async_trait]
impl Runnable for RunnableAssign {
    type Input = HashMap<String, serde_json::Value>;
    type Output = HashMap<String, serde_json::Value>;

    fn name(&self) -> String {
        let keys: Vec<String> = self.mapper.keys().collect();
        format!("RunnableAssign<{}>", keys.join(","))
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

        // Run mapper to compute new values
        let mapper_output = match self
            .mapper
            .invoke(input.clone(), Some(config.clone()))
            .await
        {
            Ok(output) => output,
            Err(e) => {
                if let Err(callback_err) = callback_manager
                    .on_chain_error(&e.to_string(), run_id, None)
                    .await
                {
                    tracing::warn!(
                        run_id = %run_id,
                        error = %callback_err,
                        "Failed to invoke on_chain_error callback"
                    );
                }
                return Err(e);
            }
        };

        // Merge original input with mapper output
        let mut result = input;
        for (key, value) in mapper_output {
            result.insert(key, value);
        }

        // End chain
        callback_manager.on_chain_end(&result, run_id, None).await?;

        Ok(result)
    }
}

/// A Runnable that runs multiple Runnables in parallel and returns a map of their outputs.
///
/// `RunnableParallel` provides the same input to all child Runnables and collects
/// their outputs into a `HashMap` keyed by the names provided.
///
/// # Example
///
/// ```rust,ignore
/// use std::collections::HashMap;
/// use dashflow::core::runnable::{RunnableParallel, RunnableLambda};
///
/// let mut parallel = RunnableParallel::new();
/// parallel.add("double", RunnableLambda::new(|x: i32| x * 2));
/// parallel.add("triple", RunnableLambda::new(|x: i32| x * 3));
///
/// let result = parallel.invoke(5, None).await?;
/// // result = {"double": 10, "triple": 15}
/// ```
pub struct RunnableParallel<Input, Output>
where
    Input: Clone + Send + Sync,
    Output: Send + Sync,
{
    runnables: HashMap<String, Arc<dyn Runnable<Input = Input, Output = Output>>>,
}

impl<Input, Output> RunnableParallel<Input, Output>
where
    Input: Clone + Send + Sync,
    Output: Send + Sync,
{
    /// Create a new empty `RunnableParallel`
    #[must_use]
    pub fn new() -> Self {
        Self {
            runnables: HashMap::new(),
        }
    }

    /// Add a named Runnable to the parallel execution
    pub fn add<R>(&mut self, name: impl Into<String>, runnable: R)
    where
        R: Runnable<Input = Input, Output = Output> + 'static,
    {
        self.runnables.insert(name.into(), Arc::new(runnable));
    }

    /// Create from a `HashMap` of Runnables
    #[must_use]
    pub fn from_map(
        runnables: HashMap<String, Arc<dyn Runnable<Input = Input, Output = Output>>>,
    ) -> Self {
        Self { runnables }
    }

    /// Get an iterator over the keys of the parallel runnables
    pub fn keys(&self) -> impl Iterator<Item = String> + '_ {
        self.runnables.keys().cloned()
    }
}

impl<Input, Output> Default for RunnableParallel<Input, Output>
where
    Input: Clone + Send + Sync,
    Output: Send + Sync,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<Input, Output> Clone for RunnableParallel<Input, Output>
where
    Input: Clone + Send + Sync,
    Output: Send + Sync,
{
    fn clone(&self) -> Self {
        Self {
            runnables: self.runnables.clone(),
        }
    }
}

#[async_trait]
impl<Input, Output> Runnable for RunnableParallel<Input, Output>
where
    Input: Clone + Send + Sync + 'static,
    Output: Send + Sync + 'static,
{
    type Input = Input;
    type Output = HashMap<String, Output>;

    fn name(&self) -> String {
        format!("Parallel[{}]", self.runnables.len())
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

        // Execute parallel tasks with optional concurrency limit
        let max_concurrency = config.max_concurrency;

        let result: Result<HashMap<String, Output>> = async {
            let mut tasks = Vec::new();

            for (name, runnable) in &self.runnables {
                let name = name.clone();
                let runnable = Arc::clone(runnable);
                let input = input.clone();
                let config_clone = Some(config.clone());

                tasks.push(async move {
                    let result = runnable.invoke(input, config_clone).await?;
                    Ok::<_, Error>((name, result))
                });
            }

            // If max_concurrency is set, use bounded concurrency
            let results = if let Some(limit) = max_concurrency {
                futures::stream::iter(tasks)
                    .buffer_unordered(limit.max(1))
                    .collect::<Vec<_>>()
                    .await
            } else {
                futures::future::join_all(tasks).await
            };

            let mut output = HashMap::new();
            for result in results {
                let (name, value) = result?;
                output.insert(name, value);
            }

            Ok(output)
        }
        .await;

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
        let mut results = Vec::new();
        for input in inputs {
            results.push(self.invoke(input, config.clone()).await?);
        }
        Ok(results)
    }

    fn get_graph(&self, config: Option<&RunnableConfig>) -> Graph {
        let mut graph = Graph::new();

        // Create a root node for the parallel execution
        let root_node = Node::new(self.name(), self.name());
        graph.add_node(root_node);

        // Add each branch and connect it to the root
        for (branch_name, runnable) in &self.runnables {
            let branch_graph = runnable.get_graph(config);

            // Add nodes from branch graph with prefix to avoid conflicts
            for node in branch_graph.nodes.values() {
                let new_id = format!("{}:{}", branch_name, node.id);
                let new_node = node.with_id(new_id);
                graph.add_node(new_node);
            }

            // Add edges from branch graph with updated IDs
            for edge in &branch_graph.edges {
                let new_source = format!("{}:{}", branch_name, edge.source);
                let new_target = format!("{}:{}", branch_name, edge.target);
                graph.add_edge(Edge::new(new_source, new_target));
            }

            // Connect root to the first node of this branch
            if let Some(first_node) = branch_graph.first_node() {
                let first_node_id = format!("{}:{}", branch_name, first_node.id);
                graph.add_edge(Edge::new(self.name(), first_node_id));
            }
        }

        graph
    }

    async fn stream_events(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
        options: Option<StreamEventsOptions>,
    ) -> Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send + 'static>>>
    where
        Self: Sized + Clone + 'static,
        Self::Input: Clone + Send + serde::Serialize + 'static,
        Self::Output: Clone + Send + serde::Serialize + 'static,
    {
        // Generate run ID
        let parallel_run_id = uuid::Uuid::new_v4();
        let name = self.name();
        let options = options.unwrap_or_default();

        // Extract tags and metadata from config
        let (tags, metadata) = if let Some(ref cfg) = config {
            (cfg.tags.clone(), cfg.metadata.clone())
        } else {
            (Vec::new(), HashMap::new())
        };

        // Serialize input for the start event
        let input_value = serde_json::to_value(&input).unwrap_or(serde_json::Value::Null);

        // Clone self to move into stream
        let parallel = self.clone();
        let input_clone = input.clone();

        let stream = async_stream::stream! {
            // Emit start event
            let start_event = StreamEvent::new(
                StreamEventType::ChainStart,
                name.clone(),
                parallel_run_id,
                StreamEventData::Input(input_value),
            )
            .with_tags(tags.clone())
            .with_metadata(metadata.clone());

            // Apply filters
            if options.should_include(&start_event) {
                yield start_event;
            }

            // Execute parallel tasks with optional concurrency limit
            let max_concurrency = config.as_ref().and_then(|c| c.max_concurrency);

            let result: Result<HashMap<String, Output>> = async {
                let mut tasks = Vec::new();

                for (branch_name, runnable) in &parallel.runnables {
                    let branch_name = branch_name.clone();
                    let runnable = Arc::clone(runnable);
                    let input = input_clone.clone();
                    let config_clone = config.clone();

                    tasks.push(async move {
                        let result = runnable.invoke(input, config_clone).await?;
                        Ok::<_, Error>((branch_name, result))
                    });
                }

                // If max_concurrency is set, use bounded concurrency
                let results = if let Some(limit) = max_concurrency {
                    futures::stream::iter(tasks)
                        .buffer_unordered(limit.max(1))
                        .collect::<Vec<_>>()
                        .await
                } else {
                    futures::future::join_all(tasks).await
                };

                let mut output = HashMap::new();
                for result in results {
                    let (branch_name, value) = result?;
                    output.insert(branch_name, value);
                }

                Ok(output)
            }
            .await;

            // Emit end event with result
            match result {
                Ok(output) => {
                    let output_value = serde_json::to_value(&output).unwrap_or(serde_json::Value::Null);
                    let end_event = StreamEvent::new(
                        StreamEventType::ChainEnd,
                        name,
                        parallel_run_id,
                        StreamEventData::Output(output_value),
                    )
                    .with_tags(tags)
                    .with_metadata(metadata);

                    // Apply filters
                    if options.should_include(&end_event) {
                        yield end_event;
                    }
                }
                Err(e) => {
                    let error_event = StreamEvent::new(
                        StreamEventType::ChainEnd,
                        name,
                        parallel_run_id,
                        StreamEventData::Error(e.to_string()),
                    )
                    .with_tags(tags)
                    .with_metadata(metadata);

                    // Apply filters
                    if options.should_include(&error_event) {
                        yield error_event;
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::RunnableConfig;
    use crate::core::runnable::RunnableLambda;
    use async_trait::async_trait;

    // ============================================================================
    // Helper Structs for Testing
    // ============================================================================

    /// A runnable that always fails
    struct AlwaysFailRunnable;

    #[async_trait]
    impl Runnable for AlwaysFailRunnable {
        type Input = i32;
        type Output = i32;

        async fn invoke(
            &self,
            _input: Self::Input,
            _config: Option<RunnableConfig>,
        ) -> Result<Self::Output> {
            Err(Error::InvalidInput("test error".to_string()))
        }
    }

    // ============================================================================
    // RunnableParallel Tests
    // ============================================================================

    #[test]
    fn test_runnable_parallel_new() {
        let parallel: RunnableParallel<i32, i32> = RunnableParallel::new();
        assert_eq!(parallel.keys().count(), 0);
    }

    #[test]
    fn test_runnable_parallel_default() {
        let parallel: RunnableParallel<i32, i32> = RunnableParallel::default();
        assert_eq!(parallel.keys().count(), 0);
    }

    #[test]
    fn test_runnable_parallel_add() {
        let mut parallel: RunnableParallel<i32, i32> = RunnableParallel::new();
        parallel.add("double", RunnableLambda::new(|x: i32| x * 2));
        parallel.add("triple", RunnableLambda::new(|x: i32| x * 3));

        let keys: Vec<_> = parallel.keys().collect();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"double".to_string()));
        assert!(keys.contains(&"triple".to_string()));
    }

    #[test]
    fn test_runnable_parallel_name() {
        let mut parallel: RunnableParallel<i32, i32> = RunnableParallel::new();
        assert_eq!(parallel.name(), "Parallel[0]");

        parallel.add("a", RunnableLambda::new(|x: i32| x));
        assert_eq!(parallel.name(), "Parallel[1]");

        parallel.add("b", RunnableLambda::new(|x: i32| x));
        assert_eq!(parallel.name(), "Parallel[2]");
    }

    #[test]
    fn test_runnable_parallel_clone() {
        let mut parallel: RunnableParallel<i32, i32> = RunnableParallel::new();
        parallel.add("test", RunnableLambda::new(|x: i32| x));

        let cloned = parallel.clone();
        assert_eq!(cloned.keys().count(), 1);
        assert_eq!(cloned.name(), parallel.name());
    }

    #[tokio::test]
    async fn test_runnable_parallel_invoke_empty() {
        let parallel: RunnableParallel<i32, i32> = RunnableParallel::new();
        let result = parallel
            .invoke(5, None)
            .await
            .expect("invoke should succeed");

        // Empty parallel returns empty map
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_runnable_parallel_invoke_single() {
        let mut parallel: RunnableParallel<i32, i32> = RunnableParallel::new();
        parallel.add("double", RunnableLambda::new(|x: i32| x * 2));

        let result = parallel
            .invoke(5, None)
            .await
            .expect("invoke should succeed");

        assert_eq!(result.len(), 1);
        assert_eq!(result.get("double"), Some(&10));
    }

    #[tokio::test]
    async fn test_runnable_parallel_invoke_multiple() {
        let mut parallel: RunnableParallel<i32, i32> = RunnableParallel::new();
        parallel.add("double", RunnableLambda::new(|x: i32| x * 2));
        parallel.add("triple", RunnableLambda::new(|x: i32| x * 3));
        parallel.add("square", RunnableLambda::new(|x: i32| x * x));

        let result = parallel
            .invoke(5, None)
            .await
            .expect("invoke should succeed");

        assert_eq!(result.len(), 3);
        assert_eq!(result.get("double"), Some(&10));
        assert_eq!(result.get("triple"), Some(&15));
        assert_eq!(result.get("square"), Some(&25));
    }

    #[tokio::test]
    async fn test_runnable_parallel_invoke_with_config() {
        let mut parallel: RunnableParallel<i32, i32> = RunnableParallel::new();
        parallel.add("inc", RunnableLambda::new(|x: i32| x + 1));

        let config = RunnableConfig::default();
        let result = parallel
            .invoke(10, Some(config))
            .await
            .expect("invoke should succeed");

        assert_eq!(result.get("inc"), Some(&11));
    }

    #[tokio::test]
    async fn test_runnable_parallel_invoke_with_concurrency_limit() {
        let mut parallel: RunnableParallel<i32, i32> = RunnableParallel::new();
        parallel.add("a", RunnableLambda::new(|x: i32| x + 1));
        parallel.add("b", RunnableLambda::new(|x: i32| x + 2));
        parallel.add("c", RunnableLambda::new(|x: i32| x + 3));

        let mut config = RunnableConfig::default();
        config.max_concurrency = Some(1); // Force sequential execution

        let result = parallel
            .invoke(0, Some(config))
            .await
            .expect("invoke should succeed");

        assert_eq!(result.len(), 3);
        assert_eq!(result.get("a"), Some(&1));
        assert_eq!(result.get("b"), Some(&2));
        assert_eq!(result.get("c"), Some(&3));
    }

    #[tokio::test]
    async fn test_runnable_parallel_invoke_error_propagation() {
        let mut parallel: RunnableParallel<i32, i32> = RunnableParallel::new();
        parallel.add("ok", RunnableLambda::new(|x: i32| x));
        parallel.add("error", AlwaysFailRunnable);

        let result = parallel.invoke(5, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_runnable_parallel_batch() {
        let mut parallel: RunnableParallel<i32, i32> = RunnableParallel::new();
        parallel.add("double", RunnableLambda::new(|x: i32| x * 2));

        let results = parallel
            .batch(vec![1, 2, 3], None)
            .await
            .expect("batch should succeed");

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].get("double"), Some(&2));
        assert_eq!(results[1].get("double"), Some(&4));
        assert_eq!(results[2].get("double"), Some(&6));
    }

    #[test]
    fn test_runnable_parallel_from_map() {
        let mut map: HashMap<String, Arc<dyn Runnable<Input = i32, Output = i32>>> = HashMap::new();
        map.insert(
            "test".to_string(),
            Arc::new(RunnableLambda::new(|x: i32| x)),
        );

        let parallel = RunnableParallel::from_map(map);
        assert_eq!(parallel.keys().count(), 1);
    }

    #[test]
    fn test_runnable_parallel_get_graph() {
        let mut parallel: RunnableParallel<i32, i32> = RunnableParallel::new();
        parallel.add("a", RunnableLambda::new(|x: i32| x));
        parallel.add("b", RunnableLambda::new(|x: i32| x));

        let graph = parallel.get_graph(None);

        // Graph should have root node plus branch nodes
        assert!(!graph.nodes.is_empty());
        assert!(graph.nodes.contains_key(&parallel.name()));
    }

    // ============================================================================
    // RunnableAssign Tests
    // ============================================================================

    /// A runnable that always fails for assign tests
    struct AlwaysFailJsonRunnable;

    #[async_trait]
    impl Runnable for AlwaysFailJsonRunnable {
        type Input = HashMap<String, serde_json::Value>;
        type Output = serde_json::Value;

        async fn invoke(
            &self,
            _input: Self::Input,
            _config: Option<RunnableConfig>,
        ) -> Result<Self::Output> {
            Err(Error::InvalidInput("assign error".to_string()))
        }
    }

    #[tokio::test]
    async fn test_runnable_assign_basic() {
        let mut mapper: RunnableParallel<HashMap<String, serde_json::Value>, serde_json::Value> =
            RunnableParallel::new();
        mapper.add(
            "computed",
            RunnableLambda::new(|_: HashMap<String, serde_json::Value>| serde_json::json!(42)),
        );

        let assign = RunnableAssign::new(mapper);

        let mut input = HashMap::new();
        input.insert("original".to_string(), serde_json::json!("value"));

        let result = assign
            .invoke(input, None)
            .await
            .expect("invoke should succeed");

        // Result should contain both original and computed values
        assert_eq!(result.get("original"), Some(&serde_json::json!("value")));
        assert_eq!(result.get("computed"), Some(&serde_json::json!(42)));
    }

    #[test]
    fn test_runnable_assign_name() {
        let mut mapper: RunnableParallel<HashMap<String, serde_json::Value>, serde_json::Value> =
            RunnableParallel::new();
        mapper.add(
            "key1",
            RunnableLambda::new(|_: HashMap<String, serde_json::Value>| serde_json::json!(1)),
        );
        mapper.add(
            "key2",
            RunnableLambda::new(|_: HashMap<String, serde_json::Value>| serde_json::json!(2)),
        );

        let assign = RunnableAssign::new(mapper);
        let name = assign.name();

        // Name should include the keys
        assert!(name.starts_with("RunnableAssign<"));
        assert!(name.contains("key1") || name.contains("key2"));
    }

    #[tokio::test]
    async fn test_runnable_assign_overwrites_keys() {
        let mut mapper: RunnableParallel<HashMap<String, serde_json::Value>, serde_json::Value> =
            RunnableParallel::new();
        mapper.add(
            "key",
            RunnableLambda::new(|_: HashMap<String, serde_json::Value>| {
                serde_json::json!("new_value")
            }),
        );

        let assign = RunnableAssign::new(mapper);

        let mut input = HashMap::new();
        input.insert("key".to_string(), serde_json::json!("old_value"));

        let result = assign
            .invoke(input, None)
            .await
            .expect("invoke should succeed");

        // Mapper output should overwrite original value
        assert_eq!(result.get("key"), Some(&serde_json::json!("new_value")));
    }

    #[tokio::test]
    async fn test_runnable_assign_error_propagation() {
        let mut mapper: RunnableParallel<HashMap<String, serde_json::Value>, serde_json::Value> =
            RunnableParallel::new();
        mapper.add("error_key", AlwaysFailJsonRunnable);

        let assign = RunnableAssign::new(mapper);
        let input = HashMap::new();

        let result = assign.invoke(input, None).await;
        assert!(result.is_err());
    }
}
