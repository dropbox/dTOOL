//! RunnableSequence - Sequential composition of two runnables
//!
//! Created via the `pipe()` method on the `Runnable` trait.

use async_trait::async_trait;
use futures::stream::Stream;
use std::collections::HashMap;
use std::ops::BitOr;
use std::pin::Pin;

use super::graph::{Edge, Graph};
use super::lambda::RunnableLambda;
use super::stream_events::{StreamEvent, StreamEventData, StreamEventType, StreamEventsOptions};
use super::Runnable;
use crate::core::config::RunnableConfig;
use crate::core::error::Result;

/// A sequence of two Runnables executed one after another
///
/// Created via the `pipe()` method.
#[derive(Clone)]
pub struct RunnableSequence<First, Second> {
    first: First,
    second: Second,
}

impl<First, Second> RunnableSequence<First, Second> {
    /// Create a new `RunnableSequence`
    pub fn new(first: First, second: Second) -> Self {
        Self { first, second }
    }
}

#[async_trait]
impl<First, Second> Runnable for RunnableSequence<First, Second>
where
    First: Runnable + Send + Sync,
    Second: Runnable<Input = First::Output> + Send + Sync,
    First::Input: Send,
    First::Output: Send,
    Second::Output: Send,
{
    type Input = First::Input;
    type Output = Second::Output;

    fn name(&self) -> String {
        format!("{}|{}", self.first.name(), self.second.name())
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

        // Execute sequence
        let result = async {
            let intermediate = self.first.invoke(input, Some(config.clone())).await?;
            self.second.invoke(intermediate, Some(config)).await
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
        // Process through first runnable
        let mut intermediates = Vec::new();
        for input in inputs {
            intermediates.push(self.first.invoke(input, config.clone()).await?);
        }

        // Process through second runnable
        let mut results = Vec::new();
        for intermediate in intermediates {
            results.push(self.second.invoke(intermediate, config.clone()).await?);
        }

        Ok(results)
    }

    async fn stream(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Self::Output>> + Send + 'static>>>
    where
        Self::Output: Clone + 'static,
    {
        // Stream through the sequence by:
        // 1. Getting result from first runnable
        // 2. Streaming the second runnable with that result
        let intermediate = self.first.invoke(input, config.clone()).await?;
        self.second.stream(intermediate, config).await
    }

    fn get_graph(&self, config: Option<&RunnableConfig>) -> Graph {
        // Get graphs from both runnables
        let first_graph = self.first.get_graph(config);
        let second_graph = self.second.get_graph(config);

        // Create combined graph
        let mut graph = Graph::new();

        // Add nodes from first graph with prefix
        for node in first_graph.nodes.values() {
            graph.add_node(node.clone());
        }

        // Add edges from first graph
        for edge in &first_graph.edges {
            graph.add_edge(edge.clone());
        }

        // Add nodes from second graph with prefix to avoid conflicts
        for node in second_graph.nodes.values() {
            let new_id = if first_graph.nodes.contains_key(&node.id) {
                // Avoid ID conflicts by prefixing
                format!("{}_{}", self.second.name(), node.id)
            } else {
                node.id.clone()
            };
            let new_node = node.with_id(new_id);
            graph.add_node(new_node);
        }

        // Add edges from second graph
        for edge in &second_graph.edges {
            let new_source = if first_graph.nodes.contains_key(&edge.source) {
                format!("{}_{}", self.second.name(), edge.source)
            } else {
                edge.source.clone()
            };
            let new_target = if first_graph.nodes.contains_key(&edge.target) {
                format!("{}_{}", self.second.name(), edge.target)
            } else {
                edge.target.clone()
            };
            graph.add_edge(Edge::new(new_source, new_target));
        }

        // Connect the last node of first graph to first node of second graph
        if let (Some(first_last), Some(second_first)) =
            (first_graph.last_node(), second_graph.first_node())
        {
            let second_first_id = if first_graph.nodes.contains_key(&second_first.id) {
                format!("{}_{}", self.second.name(), second_first.id)
            } else {
                second_first.id.clone()
            };
            graph.add_edge(Edge::new(&first_last.id, second_first_id));
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
        // Generate run IDs
        let sequence_run_id = uuid::Uuid::new_v4();
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

        // Clone self to move into stream (Self: Clone is guaranteed by where clause)
        let sequence = self.clone();
        let input_clone = input.clone();

        let stream = async_stream::stream! {
            // Emit start event for the sequence
            let start_event = StreamEvent::new(
                StreamEventType::ChainStart,
                name.clone(),
                sequence_run_id,
                StreamEventData::Input(input_value),
            )
            .with_tags(tags.clone())
            .with_metadata(metadata.clone());

            // Apply filters
            if options.should_include(&start_event) {
                yield start_event;
            }

            // Execute the sequence
            let result = async {
                let intermediate = sequence.first.invoke(input_clone, config.clone()).await?;
                sequence.second.invoke(intermediate, config).await
            }
            .await;

            // Emit end event with result
            match result {
                Ok(output) => {
                    let output_value = serde_json::to_value(&output).unwrap_or(serde_json::Value::Null);
                    let end_event = StreamEvent::new(
                        StreamEventType::ChainEnd,
                        name,
                        sequence_run_id,
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
                        sequence_run_id,
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

/// Implement `BitOr` operator (|) for `RunnableSequence`
///
/// Allows composing `RunnableSequence` with other Runnables using the pipe operator:
///
/// ```rust,ignore
/// let chain = (runnable1 | runnable2) | runnable3;
/// ```
impl<First, Second, R> BitOr<R> for RunnableSequence<First, Second>
where
    First: Runnable + Send + Sync,
    Second: Runnable<Input = First::Output> + Send + Sync,
    First::Input: Send,
    First::Output: Send,
    Second::Output: Send,
    R: Runnable<Input = Second::Output>,
{
    type Output = RunnableSequence<Self, R>;

    fn bitor(self, rhs: R) -> Self::Output {
        self.pipe(rhs)
    }
}

/// Implement `BitOr` operator (|) for `RunnableLambda`
///
/// Allows composing `RunnableLambda` with other Runnables using the pipe operator:
///
/// ```rust,ignore
/// let chain = lambda1 | lambda2 | lambda3;
/// ```
impl<F, Input, Output, R> BitOr<R> for RunnableLambda<F, Input, Output>
where
    F: Fn(Input) -> Output + Send + Sync,
    Input: Send + Sync,
    Output: Send + Sync,
    R: Runnable<Input = Output>,
{
    type Output = RunnableSequence<Self, R>;

    fn bitor(self, rhs: R) -> Self::Output {
        self.pipe(rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    // ==================== RunnableSequence Construction Tests ====================

    #[test]
    fn test_runnable_sequence_new() {
        let first = RunnableLambda::new(|x: i32| x * 2);
        let second = RunnableLambda::new(|x: i32| x + 1);
        let seq = RunnableSequence::new(first, second);

        // Name should combine both runnable names
        let name = seq.name();
        assert!(name.contains("|"));
    }

    #[test]
    fn test_runnable_sequence_name_format() {
        let first = RunnableLambda::new(|x: i32| x);
        let second = RunnableLambda::new(|x: i32| x);
        let seq = RunnableSequence::new(first, second);

        let name = seq.name();
        // Should be "first_name|second_name"
        let parts: Vec<&str> = name.split('|').collect();
        assert_eq!(parts.len(), 2);
    }

    // ==================== RunnableSequence Invoke Tests ====================

    #[tokio::test]
    async fn test_runnable_sequence_invoke_simple() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let add_one = RunnableLambda::new(|x: i32| x + 1);
        let seq = RunnableSequence::new(double, add_one);

        // 5 * 2 = 10, 10 + 1 = 11
        let result = seq.invoke(5, None).await.unwrap();
        assert_eq!(result, 11);
    }

    #[tokio::test]
    async fn test_runnable_sequence_invoke_order_matters() {
        // Order 1: double then add_one
        let double1 = RunnableLambda::new(|x: i32| x * 2);
        let add_one1 = RunnableLambda::new(|x: i32| x + 1);
        let seq1 = RunnableSequence::new(double1, add_one1);
        // 5 * 2 + 1 = 11
        let result1 = seq1.invoke(5, None).await.unwrap();
        assert_eq!(result1, 11);

        // Order 2: add_one then double
        let add_one2 = RunnableLambda::new(|x: i32| x + 1);
        let double2 = RunnableLambda::new(|x: i32| x * 2);
        let seq2 = RunnableSequence::new(add_one2, double2);
        // (5 + 1) * 2 = 12
        let result2 = seq2.invoke(5, None).await.unwrap();
        assert_eq!(result2, 12);
    }

    #[tokio::test]
    async fn test_runnable_sequence_invoke_type_transform() {
        let to_string = RunnableLambda::new(|x: i32| x.to_string());
        let len = RunnableLambda::new(|s: String| s.len());
        let seq = RunnableSequence::new(to_string, len);

        // 12345 -> "12345" -> 5
        let result = seq.invoke(12345, None).await.unwrap();
        assert_eq!(result, 5);
    }

    #[tokio::test]
    async fn test_runnable_sequence_invoke_strings() {
        let to_upper = RunnableLambda::new(|s: String| s.to_uppercase());
        let exclaim = RunnableLambda::new(|s: String| format!("{}!", s));
        let seq = RunnableSequence::new(to_upper, exclaim);

        let result = seq.invoke("hello".to_string(), None).await.unwrap();
        assert_eq!(result, "HELLO!");
    }

    #[tokio::test]
    async fn test_runnable_sequence_invoke_with_config() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let triple = RunnableLambda::new(|x: i32| x * 3);
        let seq = RunnableSequence::new(double, triple);

        let config = RunnableConfig::default();
        // 5 * 2 = 10, 10 * 3 = 30
        let result = seq.invoke(5, Some(config)).await.unwrap();
        assert_eq!(result, 30);
    }

    #[tokio::test]
    async fn test_runnable_sequence_invoke_with_tagged_config() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let add_one = RunnableLambda::new(|x: i32| x + 1);
        let seq = RunnableSequence::new(double, add_one);

        let mut config = RunnableConfig::default();
        config.tags = vec!["test".to_string()];

        let result = seq.invoke(5, Some(config)).await.unwrap();
        assert_eq!(result, 11);
    }

    // ==================== RunnableSequence Batch Tests ====================

    #[tokio::test]
    async fn test_runnable_sequence_batch_empty() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let add_one = RunnableLambda::new(|x: i32| x + 1);
        let seq = RunnableSequence::new(double, add_one);

        let result: Vec<i32> = seq.batch(vec![], None).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_runnable_sequence_batch_single() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let add_one = RunnableLambda::new(|x: i32| x + 1);
        let seq = RunnableSequence::new(double, add_one);

        let result = seq.batch(vec![5], None).await.unwrap();
        assert_eq!(result, vec![11]);
    }

    #[tokio::test]
    async fn test_runnable_sequence_batch_multiple() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let add_one = RunnableLambda::new(|x: i32| x + 1);
        let seq = RunnableSequence::new(double, add_one);

        // [1, 2, 3] -> [2, 4, 6] -> [3, 5, 7]
        let result = seq.batch(vec![1, 2, 3], None).await.unwrap();
        assert_eq!(result, vec![3, 5, 7]);
    }

    #[tokio::test]
    async fn test_runnable_sequence_batch_preserves_order() {
        let identity = RunnableLambda::new(|x: i32| x);
        let double = RunnableLambda::new(|x: i32| x * 2);
        let seq = RunnableSequence::new(identity, double);

        let inputs = vec![5, 3, 8, 1, 9, 2, 7];
        let result = seq.batch(inputs.clone(), None).await.unwrap();

        let expected: Vec<i32> = inputs.iter().map(|x| x * 2).collect();
        assert_eq!(result, expected);
    }

    // ==================== RunnableSequence Stream Tests ====================

    #[tokio::test]
    async fn test_runnable_sequence_stream() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let add_one = RunnableLambda::new(|x: i32| x + 1);
        let seq = RunnableSequence::new(double, add_one);

        let mut stream = seq.stream(5, None).await.unwrap();

        // Should receive the final result
        let first = stream.next().await;
        assert!(first.is_some());
        let result = first.unwrap().unwrap();
        assert_eq!(result, 11);
    }

    // ==================== BitOr Operator Tests ====================

    #[tokio::test]
    async fn test_bitor_operator_basic() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let add_one = RunnableLambda::new(|x: i32| x + 1);

        let seq = double | add_one;

        let result = seq.invoke(5, None).await.unwrap();
        assert_eq!(result, 11);
    }

    #[tokio::test]
    async fn test_bitor_operator_chained() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let add_one = RunnableLambda::new(|x: i32| x + 1);
        let square = RunnableLambda::new(|x: i32| x * x);

        let seq = double | add_one | square;

        // 5 * 2 = 10, 10 + 1 = 11, 11 * 11 = 121
        let result = seq.invoke(5, None).await.unwrap();
        assert_eq!(result, 121);
    }

    #[tokio::test]
    async fn test_bitor_operator_four_runnables() {
        let a = RunnableLambda::new(|x: i32| x + 1);
        let b = RunnableLambda::new(|x: i32| x * 2);
        let c = RunnableLambda::new(|x: i32| x - 1);
        let d = RunnableLambda::new(|x: i32| x * 3);

        let seq = a | b | c | d;

        // 5: +1=6, *2=12, -1=11, *3=33
        let result = seq.invoke(5, None).await.unwrap();
        assert_eq!(result, 33);
    }

    #[tokio::test]
    async fn test_bitor_operator_type_transform() {
        let to_string = RunnableLambda::new(|x: i32| x.to_string());
        let add_exclaim = RunnableLambda::new(|s: String| format!("{}!", s));

        let seq = to_string | add_exclaim;

        let result = seq.invoke(42, None).await.unwrap();
        assert_eq!(result, "42!");
    }

    #[tokio::test]
    async fn test_bitor_on_sequence() {
        // Test BitOr on RunnableSequence (not just RunnableLambda)
        let double = RunnableLambda::new(|x: i32| x * 2);
        let add_one = RunnableLambda::new(|x: i32| x + 1);
        let negate = RunnableLambda::new(|x: i32| -x);

        // Create a sequence, then pipe it to another runnable
        let first_seq = RunnableSequence::new(double, add_one);
        let full_seq = first_seq | negate;

        // 5 * 2 = 10, 10 + 1 = 11, -11
        let result = full_seq.invoke(5, None).await.unwrap();
        assert_eq!(result, -11);
    }

    // ==================== Graph Tests ====================

    #[test]
    fn test_runnable_sequence_get_graph_simple() {
        let first = RunnableLambda::new(|x: i32| x);
        let second = RunnableLambda::new(|x: i32| x);
        let seq = RunnableSequence::new(first, second);

        let graph = seq.get_graph(None);

        // Should have nodes from both
        assert!(!graph.nodes.is_empty());
    }

    #[test]
    fn test_runnable_sequence_get_graph_connected() {
        let first = RunnableLambda::new(|x: i32| x);
        let second = RunnableLambda::new(|x: i32| x);
        let seq = RunnableSequence::new(first, second);

        let graph = seq.get_graph(None);

        // Should have edges connecting first to second
        // The graph should represent the flow: first -> second
        assert!(!graph.edges.is_empty() || graph.nodes.len() <= 1);
    }

    #[test]
    fn test_runnable_sequence_get_graph_chained() {
        let a = RunnableLambda::new(|x: i32| x);
        let b = RunnableLambda::new(|x: i32| x);
        let c = RunnableLambda::new(|x: i32| x);

        let seq = RunnableSequence::new(RunnableSequence::new(a, b), c);

        let graph = seq.get_graph(None);

        // Should represent the full chain
        assert!(!graph.nodes.is_empty());
    }

    // Note: stream_events tests are skipped because they require Clone trait
    // which RunnableLambda with closures doesn't implement.
    // The stream_events functionality is tested via other Runnable implementations
    // that do implement Clone.

    // ==================== Complex Chain Tests ====================

    #[tokio::test]
    async fn test_runnable_sequence_complex_transform() {
        // Parse string to int, double, then stringify
        let parse = RunnableLambda::new(|s: String| s.parse::<i32>().unwrap_or(0));
        let double = RunnableLambda::new(|x: i32| x * 2);
        let stringify = RunnableLambda::new(|x: i32| x.to_string());

        let seq = parse | double | stringify;

        let result = seq.invoke("21".to_string(), None).await.unwrap();
        assert_eq!(result, "42");
    }

    #[tokio::test]
    async fn test_runnable_sequence_with_struct() {
        #[derive(Clone)]
        struct Data {
            value: i32,
        }

        let extract = RunnableLambda::new(|d: Data| d.value);
        let double = RunnableLambda::new(|x: i32| x * 2);

        let seq = extract | double;

        let result = seq.invoke(Data { value: 5 }, None).await.unwrap();
        assert_eq!(result, 10);
    }

    #[tokio::test]
    async fn test_runnable_sequence_stateful() {
        use std::sync::atomic::{AtomicI32, Ordering};
        use std::sync::Arc;

        let counter = Arc::new(AtomicI32::new(0));

        let counter1 = counter.clone();
        let increment1 = RunnableLambda::new(move |x: i32| {
            counter1.fetch_add(1, Ordering::SeqCst);
            x
        });

        let counter2 = counter.clone();
        let increment2 = RunnableLambda::new(move |x: i32| {
            counter2.fetch_add(1, Ordering::SeqCst);
            x
        });

        let seq = increment1 | increment2;

        seq.invoke(5, None).await.unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 2);

        seq.invoke(5, None).await.unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn test_runnable_sequence_identity() {
        let identity1 = RunnableLambda::new(|x: i32| x);
        let identity2 = RunnableLambda::new(|x: i32| x);

        let seq = identity1 | identity2;

        for i in -100..100 {
            let result = seq.invoke(i, None).await.unwrap();
            assert_eq!(result, i);
        }
    }

    #[tokio::test]
    async fn test_runnable_sequence_large_chain() {
        // Build a chain of 10 runnables that each add 1
        // Create 10 separate lambdas (can't clone closures)
        let add1 = RunnableLambda::new(|x: i32| x + 1);
        let add2 = RunnableLambda::new(|x: i32| x + 1);
        let add3 = RunnableLambda::new(|x: i32| x + 1);
        let add4 = RunnableLambda::new(|x: i32| x + 1);
        let add5 = RunnableLambda::new(|x: i32| x + 1);
        let add6 = RunnableLambda::new(|x: i32| x + 1);
        let add7 = RunnableLambda::new(|x: i32| x + 1);
        let add8 = RunnableLambda::new(|x: i32| x + 1);
        let add9 = RunnableLambda::new(|x: i32| x + 1);
        let add10 = RunnableLambda::new(|x: i32| x + 1);

        let chain = add1 | add2 | add3 | add4 | add5 | add6 | add7 | add8 | add9 | add10;

        // Starting from 0, adding 1 ten times = 10
        let result = chain.invoke(0, None).await.unwrap();
        assert_eq!(result, 10);
    }

    #[tokio::test]
    async fn test_runnable_sequence_zero_input() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let add_one = RunnableLambda::new(|x: i32| x + 1);

        let seq = double | add_one;

        // 0 * 2 = 0, 0 + 1 = 1
        let result = seq.invoke(0, None).await.unwrap();
        assert_eq!(result, 1);
    }

    #[tokio::test]
    async fn test_runnable_sequence_negative_numbers() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let negate = RunnableLambda::new(|x: i32| -x);

        let seq = double | negate;

        // -5 * 2 = -10, -(-10) = 10
        let result = seq.invoke(-5, None).await.unwrap();
        assert_eq!(result, 10);
    }

    #[tokio::test]
    async fn test_runnable_sequence_empty_string() {
        let to_upper = RunnableLambda::new(|s: String| s.to_uppercase());
        let len = RunnableLambda::new(|s: String| s.len());

        let seq = to_upper | len;

        let result = seq.invoke(String::new(), None).await.unwrap();
        assert_eq!(result, 0);
    }

    #[tokio::test]
    async fn test_runnable_sequence_batch_with_chained() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let add_one = RunnableLambda::new(|x: i32| x + 1);
        let square = RunnableLambda::new(|x: i32| x * x);

        let seq = double | add_one | square;

        // [1, 2, 3] -> [2, 4, 6] -> [3, 5, 7] -> [9, 25, 49]
        let result = seq.batch(vec![1, 2, 3], None).await.unwrap();
        assert_eq!(result, vec![9, 25, 49]);
    }

    // ==================== Edge Cases ====================

    #[tokio::test]
    async fn test_runnable_sequence_two_instances_same_behavior() {
        // Create two identical sequences to verify they behave the same
        let double1 = RunnableLambda::new(|x: i32| x * 2);
        let add_one1 = RunnableLambda::new(|x: i32| x + 1);
        let seq1 = RunnableSequence::new(double1, add_one1);

        let double2 = RunnableLambda::new(|x: i32| x * 2);
        let add_one2 = RunnableLambda::new(|x: i32| x + 1);
        let seq2 = RunnableSequence::new(double2, add_one2);

        let result1 = seq1.invoke(5, None).await.unwrap();
        let result2 = seq2.invoke(5, None).await.unwrap();

        assert_eq!(result1, result2);
        assert_eq!(result1, 11);
    }

    #[tokio::test]
    async fn test_runnable_sequence_repeated_invocation() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let add_one = RunnableLambda::new(|x: i32| x + 1);
        let seq = RunnableSequence::new(double, add_one);

        // Should be able to invoke multiple times with same sequence
        for i in 0..100 {
            let result = seq.invoke(i, None).await.unwrap();
            let expected = i * 2 + 1;
            assert_eq!(result, expected);
        }
    }

    #[tokio::test]
    async fn test_runnable_sequence_name_propagation() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let add_one = RunnableLambda::new(|x: i32| x + 1);
        let seq = RunnableSequence::new(double, add_one);

        let name = seq.name();
        // Name should combine both components with |
        assert!(name.contains("|"));
        // Name should not be empty
        assert!(!name.is_empty());
    }
}
