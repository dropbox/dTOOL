//! Batch processing functionality for Runnables
//!
//! This module provides:
//! - `RunnableEach`: Apply a runnable to each element of an input list
//! - `RunnableGenerator`: Stream transformation with custom generator functions

use async_trait::async_trait;
use futures::stream::{Stream, StreamExt};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use super::Runnable;
use crate::core::config::RunnableConfig;
use crate::core::error::{Error, Result};

/// `RunnableEach` applies a runnable to each element of an input list.
///
/// This is a convenience wrapper that transforms a `Runnable<Input, Output>`
/// into a `Runnable<Vec<Input>, Vec<Output>>` by calling the bound runnable's
/// batch method.
///
/// # Type Parameters
///
/// * `Input` - The input type for each element (must implement Clone for batching)
/// * `Output` - The output type for each element
///
/// # Examples
///
/// ```
/// use dashflow::core::runnable::{Runnable, RunnableEach, RunnableLambda};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Create a runnable that squares a number
/// let square = RunnableLambda::new(|x: i32| x * x);
///
/// // Wrap it in RunnableEach to process lists
/// let square_each = RunnableEach::new(Box::new(square));
///
/// // Process multiple inputs at once
/// let inputs = vec![1, 2, 3, 4, 5];
/// let outputs = square_each.invoke(inputs, None).await?;
///
/// assert_eq!(outputs, vec![1, 4, 9, 16, 25]);
/// # Ok(())
/// # }
/// ```
pub struct RunnableEach<Input, Output>
where
    Input: Send,
    Output: Send,
{
    /// The runnable to apply to each element
    bound: Box<dyn Runnable<Input = Input, Output = Output> + Send + Sync>,
}

impl<Input, Output> RunnableEach<Input, Output>
where
    Input: Send + 'static,
    Output: Send + 'static,
{
    /// Create a new `RunnableEach` with the given runnable
    ///
    /// # Arguments
    ///
    /// * `bound` - The runnable to apply to each element of the input list
    #[must_use]
    pub fn new(bound: Box<dyn Runnable<Input = Input, Output = Output> + Send + Sync>) -> Self {
        Self { bound }
    }

    /// Get a reference to the bound runnable
    #[must_use]
    pub fn bound(&self) -> &dyn Runnable<Input = Input, Output = Output> {
        &*self.bound
    }
}

#[async_trait]
impl<Input, Output> Runnable for RunnableEach<Input, Output>
where
    Input: Send + Clone + 'static,
    Output: Send + Clone + 'static,
{
    type Input = Vec<Input>;
    type Output = Vec<Output>;

    fn name(&self) -> String {
        format!("RunnableEach<{}>", self.bound.name())
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
        serialized.insert("count".to_string(), serde_json::json!(input.len()));

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

        // Process each element concurrently using invoke (since batch requires Sized)
        let tasks: Vec<_> = input
            .into_iter()
            .map(|item| {
                let bound = &self.bound;
                let config = config.clone();
                async move { bound.invoke(item, Some(config)).await }
            })
            .collect();

        let results = futures::future::join_all(tasks).await;

        // Collect results, returning early on first error
        let result: Result<Vec<_>> = results.into_iter().collect();

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
        // For batch, we need to process each input list separately
        // Each input is a Vec<Input>, and we want to return Vec<Vec<Output>>
        let mut results = Vec::new();
        for input in inputs {
            let result = self.invoke(input, config.clone()).await?;
            results.push(result);
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
        // Use invoke to process all items, then stream the result
        let result = self.invoke(input, config).await?;
        Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
    }
}

/// A Runnable that wraps a stream transformation function
///
/// `RunnableGenerator` allows you to wrap a function that transforms a stream of inputs
/// into a stream of outputs. This is useful for implementing custom streaming behavior,
/// such as custom output parsers that need to process tokens as they arrive.
///
/// Unlike `RunnableLambda` which waits for the entire input before producing output,
/// `RunnableGenerator` can emit output chunks as soon as input chunks are available.
///
/// # Type Parameters
///
/// * `Input` - The type of input elements in the stream
/// * `Output` - The type of output elements in the stream
///
/// # Example
///
/// ```rust
/// use dashflow::core::runnable::{Runnable, RunnableGenerator};
/// use futures::stream::{self, StreamExt};
/// use std::pin::Pin;
/// use futures::Stream;
/// use dashflow::core::error::Result;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     // Create a generator that adds exclamation marks to tokens
///     let generator = RunnableGenerator::new(
///         |mut input: Pin<Box<dyn Stream<Item = Result<String>> + Send>>| {
///             Box::pin(async_stream::stream! {
///                 while let Some(result) = input.next().await {
///                     match result {
///                         Ok(token) => yield Ok(format!("{}!", token)),
///                         Err(e) => yield Err(e),
///                     }
///                 }
///             }) as Pin<Box<dyn Stream<Item = Result<String>> + Send>>
///         },
///         Some("Exclaimer".to_string()),
///     );
///
///     // Stream input through the generator
///     let mut stream = generator.stream("hello".to_string(), None).await?;
///     let mut results = Vec::new();
///     while let Some(result) = stream.next().await {
///         results.push(result?);
///     }
///
///     assert_eq!(results, vec!["hello!"]);
///
///     Ok(())
/// }
/// ```
pub struct RunnableGenerator<Input, Output>
where
    Input: Send + Clone + 'static,
    Output: Send + 'static,
{
    /// The transformation function that processes the input stream
    #[allow(clippy::type_complexity)] // Stream transformer: Stream<Input> → Stream<Output>
    transform: Arc<
        dyn Fn(
                Pin<Box<dyn Stream<Item = Result<Input>> + Send>>,
            ) -> Pin<Box<dyn Stream<Item = Result<Output>> + Send>>
            + Send
            + Sync,
    >,
    /// Optional name for the generator (used in debugging and tracing)
    name: Option<String>,
}

impl<Input, Output> RunnableGenerator<Input, Output>
where
    Input: Send + Clone + 'static,
    Output: Send + 'static,
{
    /// Create a new `RunnableGenerator` with a transformation function
    ///
    /// # Arguments
    ///
    /// * `transform` - A function that takes an input stream and returns an output stream
    /// * `name` - Optional name for this generator (for debugging)
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow::core::runnable::RunnableGenerator;
    /// use futures::stream::StreamExt;
    /// use std::pin::Pin;
    /// use futures::Stream;
    /// use dashflow::core::error::Result;
    ///
    /// let generator = RunnableGenerator::new(
    ///     |mut input: Pin<Box<dyn Stream<Item = Result<String>> + Send>>| {
    ///         Box::pin(async_stream::stream! {
    ///             while let Some(result) = input.next().await {
    ///                 match result {
    ///                     Ok(s) => yield Ok(s.to_uppercase()),
    ///                     Err(e) => yield Err(e),
    ///                 }
    ///             }
    ///         }) as Pin<Box<dyn Stream<Item = Result<String>> + Send>>
    ///     },
    ///     Some("Uppercaser".to_string()),
    /// );
    /// ```
    pub fn new<F>(transform: F, name: Option<String>) -> Self
    where
        F: Fn(
                Pin<Box<dyn Stream<Item = Result<Input>> + Send>>,
            ) -> Pin<Box<dyn Stream<Item = Result<Output>> + Send>>
            + Send
            + Sync
            + 'static,
    {
        Self {
            transform: Arc::new(transform),
            name,
        }
    }
}

#[async_trait]
impl<Input, Output> Runnable for RunnableGenerator<Input, Output>
where
    Input: Send + Clone + 'static,
    Output: Send + Clone + 'static,
{
    type Input = Input;
    type Output = Output;

    fn name(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| "RunnableGenerator".to_string())
    }

    async fn invoke(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        // For invoke, we stream through the generator and collect the final result
        let mut stream = self.stream(input, config).await?;

        let mut final_output: Option<Output> = None;

        while let Some(result) = stream.next().await {
            let output = result?;
            final_output = Some(output);
        }

        final_output.ok_or_else(|| Error::other("Generator produced no output"))
    }

    async fn stream(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Self::Output>> + Send + 'static>>> {
        // Create a stream with a single input element
        let input_stream: Pin<Box<dyn Stream<Item = Result<Input>> + Send>> =
            Box::pin(futures::stream::once(async move { Ok(input) }));

        // Apply the transformation function
        let output_stream = (self.transform)(input_stream);

        Ok(output_stream)
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>> {
        // Process each input through the generator
        let mut results = Vec::new();
        for input in inputs {
            let output = self.invoke(input, config.clone()).await?;
            results.push(output);
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::runnable::RunnableLambda;
    use futures::StreamExt;

    // ==================== RunnableEach Tests ====================

    #[tokio::test]
    async fn test_runnable_each_new() {
        let square = RunnableLambda::new(|x: i32| x * x);
        let each = RunnableEach::new(Box::new(square));
        assert!(each.name().contains("RunnableEach"));
    }

    #[tokio::test]
    async fn test_runnable_each_name() {
        let square = RunnableLambda::new(|x: i32| x * x);
        let each = RunnableEach::new(Box::new(square));
        let name = each.name();
        assert!(name.starts_with("RunnableEach<"));
        // RunnableLambda name comes from impl, may just show "RunnableLambda" without closure details
    }

    #[tokio::test]
    async fn test_runnable_each_bound() {
        let square = RunnableLambda::new(|x: i32| x * x);
        let each = RunnableEach::new(Box::new(square));
        let bound = each.bound();
        // The bound returns a dyn Runnable, its name may not contain "RunnableLambda"
        // but should have a non-empty name
        assert!(!bound.name().is_empty());
    }

    #[tokio::test]
    async fn test_runnable_each_invoke_empty() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let each = RunnableEach::new(Box::new(double));

        let result = each.invoke(vec![], None).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_runnable_each_invoke_single() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let each = RunnableEach::new(Box::new(double));

        let result = each.invoke(vec![5], None).await.unwrap();
        assert_eq!(result, vec![10]);
    }

    #[tokio::test]
    async fn test_runnable_each_invoke_multiple() {
        let square = RunnableLambda::new(|x: i32| x * x);
        let each = RunnableEach::new(Box::new(square));

        let inputs = vec![1, 2, 3, 4, 5];
        let result = each.invoke(inputs, None).await.unwrap();
        assert_eq!(result, vec![1, 4, 9, 16, 25]);
    }

    #[tokio::test]
    async fn test_runnable_each_invoke_strings() {
        let to_upper = RunnableLambda::new(|s: String| s.to_uppercase());
        let each = RunnableEach::new(Box::new(to_upper));

        let inputs = vec![
            "hello".to_string(),
            "world".to_string(),
            "rust".to_string(),
        ];
        let result = each.invoke(inputs, None).await.unwrap();
        assert_eq!(
            result,
            vec![
                "HELLO".to_string(),
                "WORLD".to_string(),
                "RUST".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn test_runnable_each_invoke_with_transform() {
        let len_calc = RunnableLambda::new(|s: String| s.len());
        let each = RunnableEach::new(Box::new(len_calc));

        let inputs = vec!["a".to_string(), "ab".to_string(), "abc".to_string()];
        let result = each.invoke(inputs, None).await.unwrap();
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_runnable_each_invoke_preserves_order() {
        let identity = RunnableLambda::new(|x: i32| x);
        let each = RunnableEach::new(Box::new(identity));

        let inputs = vec![5, 3, 8, 1, 9, 2, 7];
        let result = each.invoke(inputs.clone(), None).await.unwrap();
        assert_eq!(result, inputs);
    }

    #[tokio::test]
    async fn test_runnable_each_batch_empty() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let each = RunnableEach::new(Box::new(double));

        let result: Vec<Vec<i32>> = each.batch(vec![], None).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_runnable_each_batch_single() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let each = RunnableEach::new(Box::new(double));

        let inputs = vec![vec![1, 2, 3]];
        let result = each.batch(inputs, None).await.unwrap();
        assert_eq!(result, vec![vec![2, 4, 6]]);
    }

    #[tokio::test]
    async fn test_runnable_each_batch_multiple() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let each = RunnableEach::new(Box::new(double));

        let inputs = vec![vec![1, 2], vec![3, 4], vec![5, 6]];
        let result = each.batch(inputs, None).await.unwrap();
        assert_eq!(result, vec![vec![2, 4], vec![6, 8], vec![10, 12]]);
    }

    #[tokio::test]
    async fn test_runnable_each_batch_variable_sizes() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let each = RunnableEach::new(Box::new(double));

        let inputs = vec![vec![1], vec![2, 3, 4], vec![], vec![5, 6]];
        let result = each.batch(inputs, None).await.unwrap();
        assert_eq!(result, vec![vec![2], vec![4, 6, 8], vec![], vec![10, 12]]);
    }

    #[tokio::test]
    async fn test_runnable_each_stream_returns_once() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let each = RunnableEach::new(Box::new(double));

        let inputs = vec![1, 2, 3];
        let mut stream = each.stream(inputs, None).await.unwrap();

        // Should return a single result containing all processed items
        let first = stream.next().await;
        assert!(first.is_some());
        let result = first.unwrap().unwrap();
        assert_eq!(result, vec![2, 4, 6]);

        // No more items
        let second = stream.next().await;
        assert!(second.is_none());
    }

    #[tokio::test]
    async fn test_runnable_each_with_config() {
        let double = RunnableLambda::new(|x: i32| x * 2);
        let each = RunnableEach::new(Box::new(double));

        let config = RunnableConfig::default();
        let result = each.invoke(vec![1, 2, 3], Some(config)).await.unwrap();
        assert_eq!(result, vec![2, 4, 6]);
    }

    #[tokio::test]
    async fn test_runnable_each_with_complex_transform() {
        // Transform struct
        #[derive(Clone)]
        struct Point {
            x: i32,
            y: i32,
        }

        let magnitude = RunnableLambda::new(|p: Point| ((p.x * p.x + p.y * p.y) as f64).sqrt());
        let each = RunnableEach::new(Box::new(magnitude));

        let inputs = vec![
            Point { x: 3, y: 4 },  // magnitude 5
            Point { x: 0, y: 5 },  // magnitude 5
            Point { x: 6, y: 8 },  // magnitude 10
            Point { x: 5, y: 12 }, // magnitude 13
        ];
        let result = each.invoke(inputs, None).await.unwrap();
        assert_eq!(result, vec![5.0, 5.0, 10.0, 13.0]);
    }

    // ==================== RunnableGenerator Tests ====================

    #[tokio::test]
    async fn test_runnable_generator_new() {
        let generator = RunnableGenerator::<String, String>::new(
            |input| {
                Box::pin(input.map(|r| r.map(|s| format!("{}!", s))))
                    as Pin<Box<dyn Stream<Item = Result<String>> + Send>>
            },
            Some("Exclaimer".to_string()),
        );

        assert_eq!(generator.name(), "Exclaimer");
    }

    #[tokio::test]
    async fn test_runnable_generator_name_default() {
        let generator = RunnableGenerator::<String, String>::new(
            |input| {
                Box::pin(input.map(|r| r.map(|s| s.to_uppercase())))
                    as Pin<Box<dyn Stream<Item = Result<String>> + Send>>
            },
            None,
        );

        assert_eq!(generator.name(), "RunnableGenerator");
    }

    #[tokio::test]
    async fn test_runnable_generator_invoke_simple() {
        let generator = RunnableGenerator::new(
            |input: Pin<Box<dyn Stream<Item = Result<String>> + Send>>| {
                Box::pin(input.map(|r| r.map(|s| s.to_uppercase())))
                    as Pin<Box<dyn Stream<Item = Result<String>> + Send>>
            },
            Some("Uppercaser".to_string()),
        );

        let result = generator.invoke("hello".to_string(), None).await.unwrap();
        assert_eq!(result, "HELLO");
    }

    #[tokio::test]
    async fn test_runnable_generator_invoke_transform() {
        let generator = RunnableGenerator::new(
            |input: Pin<Box<dyn Stream<Item = Result<i32>> + Send>>| {
                Box::pin(input.map(|r| r.map(|x| x * 2)))
                    as Pin<Box<dyn Stream<Item = Result<i32>> + Send>>
            },
            Some("Doubler".to_string()),
        );

        let result = generator.invoke(5, None).await.unwrap();
        assert_eq!(result, 10);
    }

    #[tokio::test]
    async fn test_runnable_generator_stream() {
        let generator = RunnableGenerator::new(
            |input: Pin<Box<dyn Stream<Item = Result<String>> + Send>>| {
                Box::pin(input.map(|r| r.map(|s| format!("[{}]", s))))
                    as Pin<Box<dyn Stream<Item = Result<String>> + Send>>
            },
            Some("Bracketer".to_string()),
        );

        let mut stream = generator.stream("test".to_string(), None).await.unwrap();

        let first = stream.next().await;
        assert!(first.is_some());
        assert_eq!(first.unwrap().unwrap(), "[test]");

        let second = stream.next().await;
        assert!(second.is_none());
    }

    #[tokio::test]
    async fn test_runnable_generator_batch() {
        let generator = RunnableGenerator::new(
            |input: Pin<Box<dyn Stream<Item = Result<i32>> + Send>>| {
                Box::pin(input.map(|r| r.map(|x| x * 3)))
                    as Pin<Box<dyn Stream<Item = Result<i32>> + Send>>
            },
            Some("Tripler".to_string()),
        );

        let inputs = vec![1, 2, 3, 4, 5];
        let result = generator.batch(inputs, None).await.unwrap();
        assert_eq!(result, vec![3, 6, 9, 12, 15]);
    }

    #[tokio::test]
    async fn test_runnable_generator_batch_empty() {
        let generator = RunnableGenerator::new(
            |input: Pin<Box<dyn Stream<Item = Result<i32>> + Send>>| {
                Box::pin(input.map(|r| r.map(|x| x * 2)))
                    as Pin<Box<dyn Stream<Item = Result<i32>> + Send>>
            },
            None,
        );

        let result: Vec<i32> = generator.batch(vec![], None).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_runnable_generator_with_config() {
        let generator = RunnableGenerator::new(
            |input: Pin<Box<dyn Stream<Item = Result<String>> + Send>>| {
                Box::pin(input.map(|r| r.map(|s| s.len().to_string())))
                    as Pin<Box<dyn Stream<Item = Result<String>> + Send>>
            },
            Some("LengthCalculator".to_string()),
        );

        let config = RunnableConfig::default();
        let result = generator
            .invoke("hello".to_string(), Some(config))
            .await
            .unwrap();
        assert_eq!(result, "5");
    }

    #[tokio::test]
    async fn test_runnable_generator_transform_type() {
        // Transform from String to usize
        let generator = RunnableGenerator::new(
            |input: Pin<Box<dyn Stream<Item = Result<String>> + Send>>| {
                Box::pin(input.map(|r| r.map(|s| s.len())))
                    as Pin<Box<dyn Stream<Item = Result<usize>> + Send>>
            },
            Some("StringToLength".to_string()),
        );

        let result = generator.invoke("test".to_string(), None).await.unwrap();
        assert_eq!(result, 4);
    }

    #[tokio::test]
    async fn test_runnable_generator_stateful_transform() {
        use std::sync::atomic::{AtomicI32, Ordering};
        use std::sync::Arc;

        let counter = Arc::new(AtomicI32::new(0));
        let counter_clone = counter.clone();

        let generator = RunnableGenerator::new(
            move |input: Pin<Box<dyn Stream<Item = Result<i32>> + Send>>| {
                let counter = counter_clone.clone();
                Box::pin(input.map(move |r| {
                    r.map(|x| {
                        let prev = counter.fetch_add(1, Ordering::SeqCst);
                        x + prev
                    })
                })) as Pin<Box<dyn Stream<Item = Result<i32>> + Send>>
            },
            Some("Accumulator".to_string()),
        );

        // Each invocation increments the counter
        let result1 = generator.invoke(10, None).await.unwrap();
        assert_eq!(result1, 10); // 10 + 0 (counter starts at 0)

        let result2 = generator.invoke(10, None).await.unwrap();
        assert_eq!(result2, 11); // 10 + 1 (counter is now 1)

        let result3 = generator.invoke(10, None).await.unwrap();
        assert_eq!(result3, 12); // 10 + 2 (counter is now 2)
    }

    #[tokio::test]
    async fn test_runnable_generator_with_filter() {
        // Generator that filters out negative numbers
        let generator = RunnableGenerator::new(
            |input: Pin<Box<dyn Stream<Item = Result<i32>> + Send>>| {
                Box::pin(input.filter_map(|r| async move {
                    match r {
                        Ok(x) if x >= 0 => Some(Ok(x)),
                        Ok(_) => None,
                        Err(e) => Some(Err(e)),
                    }
                })) as Pin<Box<dyn Stream<Item = Result<i32>> + Send>>
            },
            Some("NonNegativeFilter".to_string()),
        );

        // Since invoke returns the last item, and we're filtering:
        let result = generator.invoke(5, None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 5);
    }

    #[tokio::test]
    async fn test_runnable_generator_passthrough() {
        // Identity generator - passes input through unchanged
        let generator = RunnableGenerator::new(
            |input: Pin<Box<dyn Stream<Item = Result<String>> + Send>>| input,
            Some("Identity".to_string()),
        );

        let result = generator.invoke("unchanged".to_string(), None).await.unwrap();
        assert_eq!(result, "unchanged");
    }

    #[tokio::test]
    async fn test_runnable_generator_multiple_outputs() {
        // Generator that produces multiple outputs per input via flat_map
        let generator = RunnableGenerator::new(
            |input: Pin<Box<dyn Stream<Item = Result<i32>> + Send>>| {
                Box::pin(input.flat_map(|r| {
                    futures::stream::iter(match r {
                        Ok(x) => vec![Ok(x), Ok(x * 2), Ok(x * 3)],
                        Err(e) => vec![Err(e)],
                    })
                })) as Pin<Box<dyn Stream<Item = Result<i32>> + Send>>
            },
            Some("Multiplier".to_string()),
        );

        let mut stream = generator.stream(2, None).await.unwrap();

        let item1 = stream.next().await.unwrap().unwrap();
        assert_eq!(item1, 2);

        let item2 = stream.next().await.unwrap().unwrap();
        assert_eq!(item2, 4);

        let item3 = stream.next().await.unwrap().unwrap();
        assert_eq!(item3, 6);

        assert!(stream.next().await.is_none());
    }

    // ==================== Integration Tests ====================

    #[tokio::test]
    async fn test_runnable_each_with_runnable_each() {
        // Nested RunnableEach: process list of lists
        let double = RunnableLambda::new(|x: i32| x * 2);
        let inner_each = RunnableEach::new(Box::new(double));

        // Now wrap that in another RunnableEach to process Vec<Vec<i32>>
        let outer_each = RunnableEach::new(Box::new(inner_each));

        let inputs: Vec<Vec<i32>> = vec![vec![1, 2], vec![3, 4, 5]];
        let result = outer_each.invoke(inputs, None).await.unwrap();
        assert_eq!(result, vec![vec![2, 4], vec![6, 8, 10]]);
    }

    #[tokio::test]
    async fn test_runnable_generator_as_transformer() {
        // Using generator as a string transformer
        let transformer = RunnableGenerator::new(
            |input: Pin<Box<dyn Stream<Item = Result<String>> + Send>>| {
                Box::pin(input.map(|r| {
                    r.map(|s| {
                        s.chars()
                            .map(|c| {
                                if c.is_alphabetic() {
                                    if c.is_lowercase() {
                                        c.to_ascii_uppercase()
                                    } else {
                                        c.to_ascii_lowercase()
                                    }
                                } else {
                                    c
                                }
                            })
                            .collect()
                    })
                })) as Pin<Box<dyn Stream<Item = Result<String>> + Send>>
            },
            Some("CaseSwapper".to_string()),
        );

        let result = transformer
            .invoke("Hello World".to_string(), None)
            .await
            .unwrap();
        assert_eq!(result, "hELLO wORLD");
    }

    #[tokio::test]
    async fn test_runnable_each_concurrent_processing() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let slow_op = RunnableLambda::new(move |x: i32| {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
            x * 2
        });

        let each = RunnableEach::new(Box::new(slow_op));

        let start = std::time::Instant::now();
        let result = each.invoke(vec![1, 2, 3, 4, 5], None).await.unwrap();
        let _elapsed = start.elapsed();

        // All items should be processed
        assert_eq!(result, vec![2, 4, 6, 8, 10]);
        // All calls should have been made
        assert_eq!(call_count.load(Ordering::SeqCst), 5);
    }

    #[tokio::test]
    async fn test_runnable_generator_preserves_errors() {
        let generator = RunnableGenerator::new(
            |input: Pin<Box<dyn Stream<Item = Result<i32>> + Send>>| {
                Box::pin(input.map(|r| r.map(|x| x * 2)))
                    as Pin<Box<dyn Stream<Item = Result<i32>> + Send>>
            },
            Some("Doubler".to_string()),
        );

        // With valid input
        let result = generator.invoke(5, None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 10);
    }

    #[tokio::test]
    async fn test_runnable_each_large_batch() {
        let square = RunnableLambda::new(|x: i32| x * x);
        let each = RunnableEach::new(Box::new(square));

        // Process 1000 items
        let inputs: Vec<i32> = (1..=1000).collect();
        let result = each.invoke(inputs.clone(), None).await.unwrap();

        assert_eq!(result.len(), 1000);
        for (i, &val) in result.iter().enumerate() {
            let expected = ((i + 1) as i32).pow(2);
            assert_eq!(val, expected);
        }
    }

    #[tokio::test]
    async fn test_runnable_generator_batch_preserves_order() {
        let generator = RunnableGenerator::new(
            |input: Pin<Box<dyn Stream<Item = Result<String>> + Send>>| {
                Box::pin(input.map(|r| r.map(|s| format!("({s})"))))
                    as Pin<Box<dyn Stream<Item = Result<String>> + Send>>
            },
            None,
        );

        let inputs = vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
        ];
        let result = generator.batch(inputs, None).await.unwrap();
        assert_eq!(
            result,
            vec![
                "(a)".to_string(),
                "(b)".to_string(),
                "(c)".to_string(),
                "(d)".to_string()
            ]
        );
    }
}
