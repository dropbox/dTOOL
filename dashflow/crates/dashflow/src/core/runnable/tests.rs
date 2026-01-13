
use crate::test_prelude::*;
use futures::StreamExt;

// Helper function runnable for testing
#[derive(Clone)]
struct AddOne;

#[async_trait]
impl Runnable for AddOne {
    type Input = i32;
    type Output = i32;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        Ok(input + 1)
    }
}

#[derive(Clone)]
struct MultiplyTwo;

#[async_trait]
impl Runnable for MultiplyTwo {
    type Input = i32;
    type Output = i32;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        Ok(input * 2)
    }
}

#[tokio::test]
async fn test_simple_runnable() {
    let runnable = AddOne;
    let result = runnable.invoke(5, None).await.unwrap();
    assert_eq!(result, 6);
}

#[tokio::test]
async fn test_runnable_sequence() {
    let chain = AddOne.pipe(MultiplyTwo);
    let result = chain.invoke(5, None).await.unwrap();
    assert_eq!(result, 12); // (5 + 1) * 2
}

#[tokio::test]
async fn test_runnable_batch() {
    let runnable = AddOne;
    let results = runnable.batch(vec![1, 2, 3], None).await.unwrap();
    assert_eq!(results, vec![2, 3, 4]);
}

#[tokio::test]
async fn test_runnable_lambda() {
    let lambda = RunnableLambda::new(|x: i32| x * 3);
    let result = lambda.invoke(5, None).await.unwrap();
    assert_eq!(result, 15);
}

#[tokio::test]
async fn test_runnable_passthrough() {
    let passthrough: RunnablePassthrough<i32> = RunnablePassthrough::new();
    let result = passthrough.invoke(42, None).await.unwrap();
    assert_eq!(result, 42);
}

#[tokio::test]
async fn test_complex_chain() {
    let chain = AddOne.pipe(MultiplyTwo).pipe(AddOne);
    let result = chain.invoke(5, None).await.unwrap();
    assert_eq!(result, 13); // ((5 + 1) * 2) + 1
}

#[tokio::test]
async fn test_sequence_batch() {
    let chain = AddOne.pipe(MultiplyTwo);
    let results = chain.batch(vec![1, 2, 3], None).await.unwrap();
    assert_eq!(results, vec![4, 6, 8]); // [(1+1)*2, (2+1)*2, (3+1)*2]
}

#[tokio::test]
async fn test_runnable_parallel() {
    let mut parallel: RunnableParallel<i32, i32> = RunnableParallel::new();
    parallel.add("add_one", AddOne);
    parallel.add("multiply_two", MultiplyTwo);

    let result = parallel.invoke(5, None).await.unwrap();
    assert_eq!(result.get("add_one"), Some(&6));
    assert_eq!(result.get("multiply_two"), Some(&10));
}

#[tokio::test]
async fn test_parallel_with_lambdas() {
    let mut parallel: RunnableParallel<i32, i32> = RunnableParallel::new();
    parallel.add("double", RunnableLambda::new(|x: i32| x * 2));
    parallel.add("triple", RunnableLambda::new(|x: i32| x * 3));

    let result = parallel.invoke(5, None).await.unwrap();
    assert_eq!(result.get("double"), Some(&10));
    assert_eq!(result.get("triple"), Some(&15));
}

#[tokio::test]
async fn test_stream_events_parallel() {
    // Test that RunnableParallel emits events for parallel execution
    let mut parallel: RunnableParallel<i32, i32> = RunnableParallel::new();
    parallel.add("add_one", AddOne);
    parallel.add("multiply_two", MultiplyTwo);

    let mut stream = parallel.stream_events(5, None, None).await.unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Should have 2 events: start and end for the parallel execution
    assert_eq!(events.len(), 2, "Expected 2 events from parallel");

    // Check start event
    assert!(matches!(events[0].event_type, StreamEventType::ChainStart));
    assert_eq!(events[0].name, "Parallel[2]");

    // Input should be 5
    if let StreamEventData::Input(input) = &events[0].data {
        assert_eq!(input, &serde_json::json!(5));
    } else {
        panic!("Expected input data in start event");
    }

    // Check end event
    assert!(matches!(events[1].event_type, StreamEventType::ChainEnd));
    assert_eq!(events[1].name, "Parallel[2]");

    // Output should be a map with both results
    if let StreamEventData::Output(output) = &events[1].data {
        let output_map = output.as_object().expect("Expected object output");
        assert_eq!(
            output_map.get("add_one"),
            Some(&serde_json::json!(6)),
            "add_one result should be 6"
        );
        assert_eq!(
            output_map.get("multiply_two"),
            Some(&serde_json::json!(10)),
            "multiply_two result should be 10"
        );
    } else {
        panic!("Expected output data in end event");
    }

    // Both events should have the same run_id
    assert_eq!(events[0].run_id, events[1].run_id);
}

#[tokio::test]
async fn test_runnable_branch() {
    let branch = RunnableBranch::new(RunnableLambda::new(|x: i32| -x))
        .add_branch(|x: &i32| *x > 10, RunnableLambda::new(|x: i32| x * 2))
        .add_branch(|x: &i32| *x > 0, RunnableLambda::new(|x: i32| x * 3));

    // Test first branch (x > 10)
    let result = branch.invoke(15, None).await.unwrap();
    assert_eq!(result, 30);

    // Test second branch (x > 0 but <= 10)
    let result = branch.invoke(5, None).await.unwrap();
    assert_eq!(result, 15);

    // Test default branch (x <= 0)
    let result = branch.invoke(-5, None).await.unwrap();
    assert_eq!(result, 5); // -5 * -1
}

#[tokio::test]
async fn test_stream_events_basic() {
    // Test that stream_events emits start and end events
    let runnable = AddOne;
    let mut stream = runnable.stream_events(5, None, None).await.unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Should have exactly 2 events: start and end
    assert_eq!(events.len(), 2);

    // Check start event
    assert!(matches!(events[0].event_type, StreamEventType::ChainStart));
    assert_eq!(events[0].name, "AddOne");
    assert!(matches!(events[0].data, StreamEventData::Input(_)));

    // Check end event
    assert!(matches!(events[1].event_type, StreamEventType::ChainEnd));
    assert_eq!(events[1].name, "AddOne");
    assert!(matches!(events[1].data, StreamEventData::Output(_)));

    // Both events should have same run_id
    assert_eq!(events[0].run_id, events[1].run_id);
}

#[tokio::test]
async fn test_stream_events_with_config() {
    // Test that tags and metadata are propagated to events
    let runnable = AddOne;

    let config = RunnableConfig {
        tags: vec!["test".to_string(), "important".to_string()],
        metadata: {
            let mut map = HashMap::new();
            map.insert("user".to_string(), serde_json::json!("alice"));
            map
        },
        ..Default::default()
    };

    let mut stream = runnable
        .stream_events(10, Some(config), None)
        .await
        .unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Check that tags are present in all events
    for event in &events {
        assert_eq!(
            event.tags,
            vec!["test".to_string(), "important".to_string()]
        );
        assert_eq!(
            event.metadata.get("user"),
            Some(&serde_json::json!("alice"))
        );
    }
}

#[tokio::test]
async fn test_stream_events_error_handling() {
    // Test that errors are captured in events
    #[derive(Clone)]
    struct FailingRunnableLocal;

    #[async_trait]
    impl Runnable for FailingRunnableLocal {
        type Input = i32;
        type Output = i32;

        async fn invoke(
            &self,
            _input: Self::Input,
            _config: Option<RunnableConfig>,
        ) -> Result<Self::Output> {
            Err(Error::Other("Test error".to_string()))
        }
    }

    let runnable = FailingRunnableLocal;
    let mut stream = runnable.stream_events(5, None, None).await.unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Should have 2 events: start and error end
    assert_eq!(events.len(), 2);

    // Check start event
    assert!(matches!(events[0].event_type, StreamEventType::ChainStart));

    // Check error in end event
    assert!(matches!(events[1].event_type, StreamEventType::ChainEnd));
    if let StreamEventData::Error(msg) = &events[1].data {
        assert!(msg.contains("Test error"));
    } else {
        panic!("Expected error data in end event");
    }
}

#[tokio::test]
async fn test_stream_events_sequence() {
    // Test that RunnableSequence emits events for the overall sequence
    // This tests the override of stream_events in RunnableSequence
    let add_one = AddOne;
    let multiply_two = MultiplyTwo;
    let sequence = add_one.pipe(multiply_two);

    let mut stream = sequence.stream_events(5, None, None).await.unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Should have 2 events: start and end for the sequence
    // (The nested runnables invoke, but we only emit sequence-level events)
    assert_eq!(events.len(), 2, "Expected 2 events from sequence");

    // Check start event
    assert!(matches!(events[0].event_type, StreamEventType::ChainStart));
    assert_eq!(events[0].name, "AddOne|MultiplyTwo");

    // Input should be 5
    if let StreamEventData::Input(input) = &events[0].data {
        assert_eq!(input, &serde_json::json!(5));
    } else {
        panic!("Expected input data in start event");
    }

    // Check end event
    assert!(matches!(events[1].event_type, StreamEventType::ChainEnd));
    assert_eq!(events[1].name, "AddOne|MultiplyTwo");

    // Output should be (5+1)*2 = 12
    if let StreamEventData::Output(output) = &events[1].data {
        assert_eq!(output, &serde_json::json!(12));
    } else {
        panic!("Expected output data in end event");
    }

    // Both events should have the same run_id
    assert_eq!(events[0].run_id, events[1].run_id);
}

#[tokio::test]
async fn test_stream_events_with_filters() {
    // Test filtering by event type
    let runnable = AddOne;
    let options = StreamEventsOptions::new().include_type("chain".to_string());

    let mut stream = runnable
        .stream_events(5, None, Some(options))
        .await
        .unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Should have 2 events (both chain events)
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0].event_type, StreamEventType::ChainStart));
    assert!(matches!(events[1].event_type, StreamEventType::ChainEnd));

    // Test filtering by name (exclude)
    let options = StreamEventsOptions::new().exclude_name("AddOne".to_string());

    let mut stream = runnable
        .stream_events(5, None, Some(options))
        .await
        .unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Should have 0 events (AddOne is excluded)
    assert_eq!(events.len(), 0);

    // Test with tags
    let config = RunnableConfig {
        tags: vec!["test_tag".to_string()],
        ..Default::default()
    };

    let options = StreamEventsOptions::new().include_tag("test_tag".to_string());

    let mut stream = runnable
        .stream_events(5, Some(config.clone()), Some(options))
        .await
        .unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Should have 2 events (both have the tag)
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].tags, vec!["test_tag".to_string()]);
    assert_eq!(events[1].tags, vec!["test_tag".to_string()]);

    // Test excluding by tag
    let options = StreamEventsOptions::new().exclude_tag("test_tag".to_string());

    let mut stream = runnable
        .stream_events(5, Some(config), Some(options))
        .await
        .unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Should have 0 events (tag is excluded)
    assert_eq!(events.len(), 0);
}

#[tokio::test]
async fn test_stream_events_sequence_with_filters() {
    // Test that filters work with sequences
    let add_one = AddOne;
    let multiply_two = MultiplyTwo;
    let sequence = add_one.pipe(multiply_two);

    let options = StreamEventsOptions::new().include_type("chain".to_string());

    let mut stream = sequence
        .stream_events(5, None, Some(options))
        .await
        .unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Should have 2 events for the sequence
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0].event_type, StreamEventType::ChainStart));
    assert!(matches!(events[1].event_type, StreamEventType::ChainEnd));
}

#[tokio::test]
async fn test_stream_events_parallel_with_filters() {
    // Test that filters work with parallel execution
    let mut parallel: RunnableParallel<i32, i32> = RunnableParallel::new();
    parallel.add("add_one", AddOne);
    parallel.add("multiply_two", MultiplyTwo);

    let options = StreamEventsOptions::new().include_type("chain".to_string());

    let mut stream = parallel
        .stream_events(5, None, Some(options))
        .await
        .unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Should have 2 events for the parallel runnable
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0].event_type, StreamEventType::ChainStart));
    assert!(matches!(events[1].event_type, StreamEventType::ChainEnd));
}

struct FailingRunnable;

#[async_trait]
impl Runnable for FailingRunnable {
    type Input = i32;
    type Output = i32;

    async fn invoke(
        &self,
        _input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        Err(Error::RunnableExecution("Always fails".to_string()))
    }
}

#[tokio::test]
async fn test_runnable_with_fallbacks() {
    let with_fallback = RunnableWithFallbacks::new(FailingRunnable).add_fallback(MultiplyTwo);

    let result = with_fallback.invoke(5, None).await.unwrap();
    assert_eq!(result, 10); // Fallback succeeded
}

#[tokio::test]
async fn test_fallbacks_primary_succeeds() {
    let with_fallback = RunnableWithFallbacks::new(AddOne).add_fallback(MultiplyTwo);

    let result = with_fallback.invoke(5, None).await.unwrap();
    assert_eq!(result, 6); // Primary succeeded
}

#[tokio::test]
async fn test_fallbacks_all_fail() {
    let with_fallback = RunnableWithFallbacks::new(FailingRunnable).add_fallback(FailingRunnable);

    let result = with_fallback.invoke(5, None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_complex_composition() {
    // Create a parallel execution
    let mut parallel: RunnableParallel<i32, i32> = RunnableParallel::new();
    parallel.add("add", AddOne);
    parallel.add("mul", MultiplyTwo);

    // Chain it with a lambda that sums the values
    let sum_lambda = RunnableLambda::new(|map: HashMap<String, i32>| map.values().sum::<i32>());

    let chain = parallel.pipe(sum_lambda);
    let result = chain.invoke(5, None).await.unwrap();
    assert_eq!(result, 16); // 6 + 10 = 16
}

#[tokio::test]
async fn test_stream_basic() {
    let runnable = AddOne;
    let mut stream = runnable.stream(5, None).await.unwrap();

    let mut results = Vec::new();
    while let Some(result) = stream.next().await {
        results.push(result.unwrap());
    }

    assert_eq!(results, vec![6]);
}

#[tokio::test]
async fn test_stream_sequence() {
    let chain = AddOne.pipe(MultiplyTwo);
    let mut stream = chain.stream(5, None).await.unwrap();

    let mut results = Vec::new();
    while let Some(result) = stream.next().await {
        results.push(result.unwrap());
    }

    assert_eq!(results, vec![12]); // (5 + 1) * 2
}

#[tokio::test]
async fn test_callbacks_invoked() {
    use crate::core::callbacks::{CallbackHandler, CallbackManager};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc as StdArc;

    // Create a custom callback handler that counts invocations
    struct CountingHandler {
        start_count: StdArc<AtomicUsize>,
        end_count: StdArc<AtomicUsize>,
        error_count: StdArc<AtomicUsize>,
    }

    #[async_trait]
    impl CallbackHandler for CountingHandler {
        async fn on_chain_start(
            &self,
            _serialized: &HashMap<String, serde_json::Value>,
            _inputs: &HashMap<String, serde_json::Value>,
            _run_id: uuid::Uuid,
            _parent_run_id: Option<uuid::Uuid>,
            _tags: &[String],
            _metadata: &HashMap<String, serde_json::Value>,
        ) -> crate::core::Result<()> {
            self.start_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn on_chain_end(
            &self,
            _outputs: &HashMap<String, serde_json::Value>,
            _run_id: uuid::Uuid,
            _parent_run_id: Option<uuid::Uuid>,
        ) -> crate::core::Result<()> {
            self.end_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn on_chain_error(
            &self,
            _error: &str,
            _run_id: uuid::Uuid,
            _parent_run_id: Option<uuid::Uuid>,
        ) -> crate::core::Result<()> {
            self.error_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    let start_count = StdArc::new(AtomicUsize::new(0));
    let end_count = StdArc::new(AtomicUsize::new(0));
    let error_count = StdArc::new(AtomicUsize::new(0));

    let handler = CountingHandler {
        start_count: start_count.clone(),
        end_count: end_count.clone(),
        error_count: error_count.clone(),
    };

    let mut manager = CallbackManager::new();
    manager.add_handler(StdArc::new(handler));

    // Test successful invocation
    let config = RunnableConfig::new().with_callbacks(manager.clone());
    let lambda = RunnableLambda::new(|x: i32| x + 1);
    let result = lambda.invoke(5, Some(config)).await.unwrap();
    assert_eq!(result, 6);

    // Verify callbacks were called
    assert_eq!(start_count.load(Ordering::SeqCst), 1);
    assert_eq!(end_count.load(Ordering::SeqCst), 1);
    assert_eq!(error_count.load(Ordering::SeqCst), 0);

    // Test sequence invocation (should call callbacks for the sequence wrapper)
    start_count.store(0, Ordering::SeqCst);
    end_count.store(0, Ordering::SeqCst);

    let config = RunnableConfig::new().with_callbacks(manager);
    let sequence = AddOne.pipe(MultiplyTwo);
    let result = sequence.invoke(5, Some(config)).await.unwrap();
    assert_eq!(result, 12);

    // Sequence should call on_chain_start/end once for the wrapper
    assert_eq!(start_count.load(Ordering::SeqCst), 1);
    assert_eq!(end_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_runnable_retry_succeeds_after_failures() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc as StdArc;

    let attempt_count = StdArc::new(AtomicUsize::new(0));
    let count_clone = attempt_count.clone();

    // Create a failing runnable that succeeds on the 3rd attempt
    struct FlakyRunnable {
        count: StdArc<AtomicUsize>,
    }

    #[async_trait]
    impl Runnable for FlakyRunnable {
        type Input = i32;
        type Output = i32;

        async fn invoke(
            &self,
            input: Self::Input,
            _config: Option<RunnableConfig>,
        ) -> Result<Self::Output> {
            let attempt = self.count.fetch_add(1, Ordering::SeqCst);
            if attempt < 2 {
                Err(Error::RunnableExecution(format!(
                    "Attempt {} failed",
                    attempt + 1
                )))
            } else {
                Ok(input * 2)
            }
        }
    }

    let flaky = FlakyRunnable { count: count_clone };
    let retry = RunnableRetry::new(flaky)
        .with_max_attempts(3)
        .with_initial_interval(10) // Short interval for testing
        .with_jitter(false); // Disable jitter for predictable tests

    let result = retry.invoke(5, None).await.unwrap();
    assert_eq!(result, 10);
    assert_eq!(attempt_count.load(Ordering::SeqCst), 3); // Took 3 attempts
}

#[tokio::test]
async fn test_runnable_retry_all_fail() {
    // Create an always-failing runnable
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
            Err(Error::RunnableExecution("Always fails".to_string()))
        }
    }

    let retry = RunnableRetry::new(AlwaysFailRunnable)
        .with_max_attempts(2)
        .with_initial_interval(10);

    let result = retry.invoke(5, None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_runnable_retry_succeeds_first_attempt() {
    let retry = RunnableRetry::new(AddOne)
        .with_max_attempts(3)
        .with_initial_interval(10);

    let result = retry.invoke(5, None).await.unwrap();
    assert_eq!(result, 6);
}

#[tokio::test]
async fn test_with_listeners() {
    use crate::core::tracers::AsyncListener;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let start_count = Arc::new(AtomicUsize::new(0));
    let end_count = Arc::new(AtomicUsize::new(0));

    let start_count_clone = start_count.clone();
    let end_count_clone = end_count.clone();

    let on_start: AsyncListener = Arc::new(move |_run, _config| {
        let counter = start_count_clone.clone();
        Box::pin(async move {
            counter.fetch_add(1, Ordering::SeqCst);
        })
    });

    let on_end: AsyncListener = Arc::new(move |_run, _config| {
        let counter = end_count_clone.clone();
        Box::pin(async move {
            counter.fetch_add(1, Ordering::SeqCst);
        })
    });

    let runnable = AddOne.with_listeners(Some(on_start), Some(on_end), None);
    let result = runnable.invoke(5, None).await.unwrap();

    assert_eq!(result, 6);
    // Note: Listeners are only called if callbacks are triggered during invoke
    // which requires callback support in the base Runnable implementation
    // For now, just verify the chain works correctly
}

// Mock streaming runnable that emits multiple chunks
struct MockStreamingRunnable {
    chunks: Vec<String>,
}

#[async_trait]
impl Runnable for MockStreamingRunnable {
    type Input = ();
    type Output = String;

    async fn invoke(
        &self,
        _input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        // Non-streaming invoke just returns all chunks concatenated
        Ok(self.chunks.join(""))
    }

    // Override stream() to provide real streaming behavior
    async fn stream(
        &self,
        _input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Self::Output>> + Send + 'static>>> {
        let chunks = self.chunks.clone();
        Ok(Box::pin(async_stream::stream! {
            for chunk in chunks {
                // Simulate async delay (like network latency)
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                yield Ok(chunk);
            }
        }))
    }
}

#[tokio::test]
async fn test_real_streaming_multiple_chunks() {
    use futures::StreamExt;

    // Create a runnable that streams multiple chunks
    let streamer = MockStreamingRunnable {
        chunks: vec![
            "Hello".to_string(),
            " ".to_string(),
            "World".to_string(),
            "!".to_string(),
        ],
    };

    let mut stream = streamer.stream((), None).await.unwrap();

    let mut received_chunks = Vec::new();
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.unwrap();
        received_chunks.push(chunk);
    }

    // Verify we received multiple chunks (not just one)
    assert_eq!(received_chunks.len(), 4, "Should stream multiple chunks");
    assert_eq!(received_chunks, vec!["Hello", " ", "World", "!"]);
    assert_eq!(received_chunks.join(""), "Hello World!");
}

#[tokio::test]
async fn test_streaming_collects_to_same_result() {
    use futures::StreamExt;

    let streamer = MockStreamingRunnable {
        chunks: vec!["Hello".to_string(), " ".to_string(), "World".to_string()],
    };

    // Get result from invoke
    let invoke_result = streamer.invoke((), None).await.unwrap();

    // Get result from streaming
    let mut stream = streamer.stream((), None).await.unwrap();
    let mut stream_result = String::new();
    while let Some(chunk_result) = stream.next().await {
        stream_result.push_str(&chunk_result.unwrap());
    }

    // Both methods should produce the same final result
    assert_eq!(invoke_result, stream_result);
}

#[tokio::test]
async fn test_router_runnable_basic() {
    let mut runnables: HashMap<String, Box<dyn Runnable<Input = i32, Output = i32> + Send + Sync>> =
        HashMap::new();
    runnables.insert("add".to_string(), Box::new(AddOne));
    runnables.insert("multiply".to_string(), Box::new(MultiplyTwo));

    let router = RouterRunnable::new(runnables);

    // Test routing to add
    let result = router
        .invoke(RouterInput::new("add", 5), None)
        .await
        .unwrap();
    assert_eq!(result, 6);

    // Test routing to multiply
    let result = router
        .invoke(RouterInput::new("multiply", 5), None)
        .await
        .unwrap();
    assert_eq!(result, 10);
}

#[tokio::test]
async fn test_router_runnable_empty_and_add_route() {
    let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
    assert_eq!(router.route_count(), 0);

    router.add_route("add", Box::new(AddOne));
    router.add_route("multiply", Box::new(MultiplyTwo));
    assert_eq!(router.route_count(), 2);

    assert!(router.has_route("add"));
    assert!(router.has_route("multiply"));
    assert!(!router.has_route("nonexistent"));

    let result = router
        .invoke(RouterInput::new("add", 10), None)
        .await
        .unwrap();
    assert_eq!(result, 11);
}

#[tokio::test]
async fn test_router_runnable_invalid_key() {
    let mut runnables: HashMap<String, Box<dyn Runnable<Input = i32, Output = i32> + Send + Sync>> =
        HashMap::new();
    runnables.insert("add".to_string(), Box::new(AddOne));

    let router = RouterRunnable::new(runnables);

    // Test with invalid key
    let result = router
        .invoke(RouterInput::new("nonexistent", 5), None)
        .await;
    assert!(result.is_err());

    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("No runnable found for route key 'nonexistent'"));
    assert!(err_msg.contains("Available routes"));
}

#[tokio::test]
async fn test_router_runnable_batch() {
    let mut runnables: HashMap<String, Box<dyn Runnable<Input = i32, Output = i32> + Send + Sync>> =
        HashMap::new();
    runnables.insert("add".to_string(), Box::new(AddOne));
    runnables.insert("multiply".to_string(), Box::new(MultiplyTwo));

    let router = RouterRunnable::new(runnables);

    let inputs = vec![
        RouterInput::new("add", 5),
        RouterInput::new("multiply", 5),
        RouterInput::new("add", 10),
    ];

    let results = router.batch(inputs, None).await.unwrap();
    assert_eq!(results, vec![6, 10, 11]);
}

#[tokio::test]
async fn test_router_runnable_batch_invalid_key() {
    let mut runnables: HashMap<String, Box<dyn Runnable<Input = i32, Output = i32> + Send + Sync>> =
        HashMap::new();
    runnables.insert("add".to_string(), Box::new(AddOne));

    let router = RouterRunnable::new(runnables);

    let inputs = vec![
        RouterInput::new("add", 5),
        RouterInput::new("nonexistent", 5), // Invalid key
    ];

    let result = router.batch(inputs, None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_router_runnable_stream() {
    use futures::StreamExt;

    let mut runnables: HashMap<String, Box<dyn Runnable<Input = i32, Output = i32> + Send + Sync>> =
        HashMap::new();
    runnables.insert("add".to_string(), Box::new(AddOne));

    let router = RouterRunnable::new(runnables);

    let mut stream = router
        .stream(RouterInput::new("add", 5), None)
        .await
        .unwrap();
    let mut results = Vec::new();
    while let Some(result) = stream.next().await {
        results.push(result.unwrap());
    }

    // Default stream implementation yields single result
    assert_eq!(results, vec![6]);
}

#[tokio::test]
async fn test_router_runnable_routes_list() {
    let mut router: RouterRunnable<i32, i32> = RouterRunnable::empty();
    router.add_route("add", Box::new(AddOne));
    router.add_route("multiply", Box::new(MultiplyTwo));
    router.add_route("identity", Box::new(RunnablePassthrough::new()));

    let routes = router.routes();
    assert_eq!(routes.len(), 3);
    assert!(routes.contains(&&"add".to_string()));
    assert!(routes.contains(&&"multiply".to_string()));
    assert!(routes.contains(&&"identity".to_string()));
}

#[tokio::test]
async fn test_router_runnable_with_lambda() {
    let mut runnables: HashMap<String, Box<dyn Runnable<Input = i32, Output = i32> + Send + Sync>> =
        HashMap::new();
    runnables.insert(
        "triple".to_string(),
        Box::new(RunnableLambda::new(|x: i32| x * 3)),
    );
    runnables.insert(
        "negate".to_string(),
        Box::new(RunnableLambda::new(|x: i32| -x)),
    );

    let router = RouterRunnable::new(runnables);

    let result1 = router
        .invoke(RouterInput::new("triple", 7), None)
        .await
        .unwrap();
    assert_eq!(result1, 21);

    let result2 = router
        .invoke(RouterInput::new("negate", 7), None)
        .await
        .unwrap();
    assert_eq!(result2, -7);
}

// RunnableEach tests

#[tokio::test]
async fn test_runnable_each_basic() {
    // Create a simple lambda that squares numbers
    let square = RunnableLambda::new(|x: i32| x * x);
    let each = RunnableEach::new(Box::new(square));

    let inputs = vec![1, 2, 3, 4, 5];
    let outputs = each.invoke(inputs, None).await.unwrap();

    assert_eq!(outputs, vec![1, 4, 9, 16, 25]);
}

#[tokio::test]
async fn test_runnable_each_empty_list() {
    let double = RunnableLambda::new(|x: i32| x * 2);
    let each = RunnableEach::new(Box::new(double));

    let inputs: Vec<i32> = vec![];
    let outputs = each.invoke(inputs, None).await.unwrap();

    assert_eq!(outputs, Vec::<i32>::new());
}

#[tokio::test]
async fn test_runnable_each_single_element() {
    let increment = RunnableLambda::new(|x: i32| x + 1);
    let each = RunnableEach::new(Box::new(increment));

    let inputs = vec![42];
    let outputs = each.invoke(inputs, None).await.unwrap();

    assert_eq!(outputs, vec![43]);
}

#[tokio::test]
async fn test_runnable_each_batch() {
    let negate = RunnableLambda::new(|x: i32| -x);
    let each = RunnableEach::new(Box::new(negate));

    // Batch with multiple input lists
    let batch_inputs = vec![vec![1, 2, 3], vec![4, 5], vec![6]];

    let outputs = each.batch(batch_inputs, None).await.unwrap();

    assert_eq!(outputs, vec![vec![-1, -2, -3], vec![-4, -5], vec![-6],]);
}

#[tokio::test]
async fn test_runnable_each_with_passthrough() {
    let passthrough = RunnablePassthrough::new();
    let each = RunnableEach::new(Box::new(passthrough));

    let inputs = vec![10, 20, 30];
    let outputs = each.invoke(inputs, None).await.unwrap();

    assert_eq!(outputs, vec![10, 20, 30]);
}

#[tokio::test]
async fn test_runnable_each_name() {
    let lambda = RunnableLambda::new(|x: i32| x + 1);
    let each = RunnableEach::new(Box::new(lambda));

    let name = each.name();
    assert!(name.contains("RunnableEach"));
    // The bound runnable's name should be included
    assert!(name.contains("<"));
    assert!(name.contains(">"));
}

#[tokio::test]
async fn test_runnable_each_stream() {
    use futures::StreamExt;

    let triple = RunnableLambda::new(|x: i32| x * 3);
    let each = RunnableEach::new(Box::new(triple));

    let inputs = vec![1, 2, 3];
    let mut stream = each.stream(inputs, None).await.unwrap();

    let mut results = Vec::new();
    while let Some(result) = stream.next().await {
        results.push(result.unwrap());
    }

    // Stream should yield the entire result vec as a single item
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], vec![3, 6, 9]);
}

#[tokio::test]
async fn test_runnable_each_with_sequence() {
    // Test RunnableEach with a sequence of runnables
    let add_one = RunnableLambda::new(|x: i32| x + 1);
    let double = RunnableLambda::new(|x: i32| x * 2);
    let sequence = RunnableSequence::new(add_one, double);

    let each = RunnableEach::new(Box::new(sequence));

    let inputs = vec![1, 2, 3];
    let outputs = each.invoke(inputs, None).await.unwrap();

    // (1+1)*2=4, (2+1)*2=6, (3+1)*2=8
    assert_eq!(outputs, vec![4, 6, 8]);
}

#[tokio::test]
async fn test_runnable_each_bound_accessor() {
    let lambda = RunnableLambda::new(|x: i32| x * 5);
    let each = RunnableEach::new(Box::new(lambda));

    let bound = each.bound();
    let result = bound.invoke(3, None).await.unwrap();
    assert_eq!(result, 15);
}

#[tokio::test]
async fn test_runnable_pick_basic() {
    use std::collections::HashMap;

    let pick = RunnablePick::new(vec!["name".to_string(), "age".to_string()]);

    let mut input = HashMap::new();
    input.insert("name".to_string(), serde_json::json!("John"));
    input.insert("age".to_string(), serde_json::json!(30));
    input.insert("city".to_string(), serde_json::json!("New York"));
    input.insert("country".to_string(), serde_json::json!("USA"));

    let result = pick.invoke(input, None).await.unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result.get("name").unwrap(), &serde_json::json!("John"));
    assert_eq!(result.get("age").unwrap(), &serde_json::json!(30));
    assert!(!result.contains_key("city"));
    assert!(!result.contains_key("country"));
}

#[tokio::test]
async fn test_runnable_pick_single_key() {
    use std::collections::HashMap;

    let pick = RunnablePick::new(vec!["name".to_string()]);

    let mut input = HashMap::new();
    input.insert("name".to_string(), serde_json::json!("Alice"));
    input.insert("age".to_string(), serde_json::json!(25));

    let result = pick.invoke(input, None).await.unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result.get("name").unwrap(), &serde_json::json!("Alice"));
}

#[tokio::test]
async fn test_runnable_pick_missing_keys() {
    use std::collections::HashMap;

    let pick = RunnablePick::new(vec!["name".to_string(), "email".to_string()]);

    let mut input = HashMap::new();
    input.insert("name".to_string(), serde_json::json!("Bob"));
    input.insert("age".to_string(), serde_json::json!(35));

    let result = pick.invoke(input, None).await.unwrap();

    // Only "name" should be present, "email" doesn't exist in input
    assert_eq!(result.len(), 1);
    assert_eq!(result.get("name").unwrap(), &serde_json::json!("Bob"));
    assert!(!result.contains_key("email"));
}

#[tokio::test]
async fn test_runnable_pick_all_missing_keys() {
    use std::collections::HashMap;

    let pick = RunnablePick::new(vec!["email".to_string(), "phone".to_string()]);

    let mut input = HashMap::new();
    input.insert("name".to_string(), serde_json::json!("Charlie"));
    input.insert("age".to_string(), serde_json::json!(40));

    let result = pick.invoke(input, None).await.unwrap();

    // No keys match, result should be empty
    assert_eq!(result.len(), 0);
}

#[tokio::test]
async fn test_runnable_pick_empty_keys() {
    use std::collections::HashMap;

    let pick = RunnablePick::new(vec![]);

    let mut input = HashMap::new();
    input.insert("name".to_string(), serde_json::json!("Dave"));
    input.insert("age".to_string(), serde_json::json!(45));

    let result = pick.invoke(input, None).await.unwrap();

    // No keys to pick, result should be empty
    assert_eq!(result.len(), 0);
}

#[tokio::test]
async fn test_runnable_pick_name() {
    let pick = RunnablePick::new(vec!["name".to_string(), "age".to_string()]);
    let name = pick.name();
    assert_eq!(name, "RunnablePick<name,age>");
}

#[tokio::test]
async fn test_runnable_pick_keys_accessor() {
    let pick = RunnablePick::new(vec![
        "foo".to_string(),
        "bar".to_string(),
        "baz".to_string(),
    ]);
    let keys = pick.keys();
    assert_eq!(keys, &["foo", "bar", "baz"]);
}

#[tokio::test]
async fn test_runnable_pick_via_passthrough() {
    use std::collections::HashMap;

    let pick = RunnablePassthrough::pick(vec!["x".to_string(), "y".to_string()]);

    let mut input = HashMap::new();
    input.insert("x".to_string(), serde_json::json!(10));
    input.insert("y".to_string(), serde_json::json!(20));
    input.insert("z".to_string(), serde_json::json!(30));

    let result = pick.invoke(input, None).await.unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result.get("x").unwrap(), &serde_json::json!(10));
    assert_eq!(result.get("y").unwrap(), &serde_json::json!(20));
    assert!(!result.contains_key("z"));
}

#[tokio::test]
async fn test_runnable_pick_preserves_types() {
    use std::collections::HashMap;

    let pick = RunnablePick::new(vec![
        "string".to_string(),
        "number".to_string(),
        "bool".to_string(),
        "array".to_string(),
    ]);

    let mut input = HashMap::new();
    input.insert("string".to_string(), serde_json::json!("hello"));
    input.insert("number".to_string(), serde_json::json!(42));
    input.insert("bool".to_string(), serde_json::json!(true));
    input.insert("array".to_string(), serde_json::json!([1, 2, 3]));
    input.insert("ignore".to_string(), serde_json::json!("ignored"));

    let result = pick.invoke(input, None).await.unwrap();

    assert_eq!(result.len(), 4);
    assert_eq!(result.get("string").unwrap(), &serde_json::json!("hello"));
    assert_eq!(result.get("number").unwrap(), &serde_json::json!(42));
    assert_eq!(result.get("bool").unwrap(), &serde_json::json!(true));
    assert_eq!(result.get("array").unwrap(), &serde_json::json!([1, 2, 3]));
}

// RunnableGenerator Tests

#[tokio::test]
async fn test_runnable_generator_basic_invoke() {
    use futures::Stream;
    use std::pin::Pin;

    let uppercaser = RunnableGenerator::new(
        |mut input: Pin<Box<dyn Stream<Item = Result<String>> + Send>>| {
            Box::pin(async_stream::stream! {
                while let Some(result) = input.next().await {
                    match result {
                        Ok(s) => yield Ok(s.to_uppercase()),
                        Err(e) => yield Err(e),
                    }
                }
            }) as Pin<Box<dyn Stream<Item = Result<String>> + Send>>
        },
        Some("Uppercaser".to_string()),
    );

    let result = uppercaser.invoke("hello".to_string(), None).await.unwrap();
    assert_eq!(result, "HELLO");
}

#[tokio::test]
async fn test_runnable_generator_stream() {
    use futures::Stream;
    use std::pin::Pin;

    // Generator that splits words
    let word_splitter = RunnableGenerator::new(
        |mut input: Pin<Box<dyn Stream<Item = Result<String>> + Send>>| {
            Box::pin(async_stream::stream! {
                while let Some(result) = input.next().await {
                    match result {
                        Ok(sentence) => {
                            for word in sentence.split_whitespace() {
                                yield Ok(word.to_string());
                            }
                        }
                        Err(e) => yield Err(e),
                    }
                }
            }) as Pin<Box<dyn Stream<Item = Result<String>> + Send>>
        },
        None,
    );

    let mut stream = word_splitter
        .stream("hello world".to_string(), None)
        .await
        .unwrap();
    let mut results = Vec::new();
    while let Some(result) = stream.next().await {
        results.push(result.unwrap());
    }

    assert_eq!(results, vec!["hello", "world"]);
}

#[tokio::test]
async fn test_runnable_generator_with_numbers() {
    use futures::Stream;
    use std::pin::Pin;

    let doubler = RunnableGenerator::new(
        |mut input: Pin<Box<dyn Stream<Item = Result<i32>> + Send>>| {
            Box::pin(async_stream::stream! {
                while let Some(result) = input.next().await {
                    match result {
                        Ok(n) => yield Ok(n * 2),
                        Err(e) => yield Err(e),
                    }
                }
            }) as Pin<Box<dyn Stream<Item = Result<i32>> + Send>>
        },
        Some("Doubler".to_string()),
    );

    let result = doubler.invoke(21, None).await.unwrap();
    assert_eq!(result, 42);
}

#[tokio::test]
async fn test_runnable_generator_batch() {
    use futures::Stream;
    use std::pin::Pin;

    let exclaimer = RunnableGenerator::new(
        |mut input: Pin<Box<dyn Stream<Item = Result<String>> + Send>>| {
            Box::pin(async_stream::stream! {
                while let Some(result) = input.next().await {
                    match result {
                        Ok(s) => yield Ok(format!("{}!", s)),
                        Err(e) => yield Err(e),
                    }
                }
            }) as Pin<Box<dyn Stream<Item = Result<String>> + Send>>
        },
        None,
    );

    let results = exclaimer
        .batch(vec!["hello".to_string(), "world".to_string()], None)
        .await
        .unwrap();

    assert_eq!(results, vec!["hello!", "world!"]);
}

#[tokio::test]
async fn test_runnable_generator_name() {
    use futures::Stream;
    use std::pin::Pin;

    let named = RunnableGenerator::new(
        |mut input: Pin<Box<dyn Stream<Item = Result<String>> + Send>>| {
            Box::pin(async_stream::stream! {
                while let Some(result) = input.next().await {
                    if let Ok(s) = result {
                        yield Ok(s);
                    }
                }
            }) as Pin<Box<dyn Stream<Item = Result<String>> + Send>>
        },
        Some("CustomGenerator".to_string()),
    );

    assert_eq!(named.name(), "CustomGenerator");

    let unnamed = RunnableGenerator::new(
        |mut input: Pin<Box<dyn Stream<Item = Result<String>> + Send>>| {
            Box::pin(async_stream::stream! {
                while let Some(result) = input.next().await {
                    if let Ok(s) = result {
                        yield Ok(s);
                    }
                }
            }) as Pin<Box<dyn Stream<Item = Result<String>> + Send>>
        },
        None,
    );

    assert_eq!(unnamed.name(), "RunnableGenerator");
}

#[tokio::test]
async fn test_runnable_generator_error_passthrough() {
    use futures::Stream;
    use std::pin::Pin;

    let passthrough = RunnableGenerator::new(
        |mut input: Pin<Box<dyn Stream<Item = Result<String>> + Send>>| {
            Box::pin(async_stream::stream! {
                while let Some(result) = input.next().await {
                    match result {
                        Ok(s) => yield Ok(s),
                        Err(e) => yield Err(e),
                    }
                }
            }) as Pin<Box<dyn Stream<Item = Result<String>> + Send>>
        },
        None,
    );

    // Normal case
    let result = passthrough.invoke("test".to_string(), None).await.unwrap();
    assert_eq!(result, "test");
}

#[tokio::test]
async fn test_runnable_generator_empty_output() {
    use futures::Stream;
    use std::pin::Pin;

    // Generator that filters everything out
    let filter_all = RunnableGenerator::new(
        |mut input: Pin<Box<dyn Stream<Item = Result<String>> + Send>>| {
            Box::pin(async_stream::stream! {
                while let Some(_result) = input.next().await {
                    // Don't yield anything - consume input but produce no output
                }
                // This is needed to establish the stream's Item type
                if false {
                    yield Ok(String::new());
                }
            }) as Pin<Box<dyn Stream<Item = Result<String>> + Send>>
        },
        None,
    );

    let result = filter_all.invoke("test".to_string(), None).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("no output"));
}

#[tokio::test]
async fn test_runnable_generator_character_streaming() {
    use futures::Stream;
    use std::pin::Pin;

    // Simulates LLM token streaming by emitting characters
    let char_streamer = RunnableGenerator::new(
        |mut input: Pin<Box<dyn Stream<Item = Result<String>> + Send>>| {
            Box::pin(async_stream::stream! {
                while let Some(result) = input.next().await {
                    match result {
                        Ok(text) => {
                            for ch in text.chars() {
                                yield Ok(ch.to_string());
                            }
                        }
                        Err(e) => yield Err(e),
                    }
                }
            }) as Pin<Box<dyn Stream<Item = Result<String>> + Send>>
        },
        None,
    );

    let mut stream = char_streamer.stream("Hi".to_string(), None).await.unwrap();
    let mut chars = Vec::new();
    while let Some(result) = stream.next().await {
        chars.push(result.unwrap());
    }

    assert_eq!(chars, vec!["H", "i"]);
}

// ========================================================================
// RunnableBindingBase / RunnableBinding Tests
// ========================================================================

#[tokio::test]
async fn test_runnable_binding_basic_config() {
    // Test that bound config is merged with runtime config
    let lambda = RunnableLambda::new(|input: String| input.to_uppercase());

    let bound_config = RunnableConfig::new()
        .with_tag("bound_tag")
        .with_run_name("bound_run");

    let binding = RunnableBindingBase::new(lambda, HashMap::new(), Some(bound_config), vec![]);

    let result = binding.invoke("hello".to_string(), None).await.unwrap();
    assert_eq!(result, "HELLO");
}

#[tokio::test]
async fn test_runnable_binding_config_merge() {
    // Test that runtime config is properly merged with bound config
    // We can't directly access config in RunnableLambda, but we can test
    // that the binding properly merges configs without errors
    let lambda = RunnableLambda::new(|input: String| input.to_uppercase());

    let bound_config = RunnableConfig::new().with_tag("bound_tag");

    let binding = RunnableBindingBase::new(lambda, HashMap::new(), Some(bound_config), vec![]);

    let runtime_config = RunnableConfig::new().with_tag("runtime_tag");

    let result = binding
        .invoke("test".to_string(), Some(runtime_config))
        .await
        .unwrap();
    assert_eq!(result, "TEST");
}

#[tokio::test]
async fn test_runnable_binding_config_factory() {
    // Test config factories for dynamic configuration
    let lambda = RunnableLambda::new(|input: String| input.to_uppercase());

    let factory = Arc::new(|mut config: RunnableConfig| {
        config.tags.push("factory_tag".to_string());
        config
    });

    let binding = RunnableBindingBase::new(lambda, HashMap::new(), None, vec![factory]);

    let result = binding.invoke("test".to_string(), None).await.unwrap();
    assert_eq!(result, "TEST");
}

#[tokio::test]
async fn test_runnable_binding_multiple_factories() {
    // Test multiple config factories applied in sequence
    let lambda = RunnableLambda::new(|input: String| input.to_uppercase());

    let factory1 = Arc::new(|mut config: RunnableConfig| {
        config.tags.push("factory1".to_string());
        config
    });

    let factory2 = Arc::new(|mut config: RunnableConfig| {
        config.tags.push("factory2".to_string());
        config
    });

    let binding = RunnableBindingBase::new(lambda, HashMap::new(), None, vec![factory1, factory2]);

    let result = binding.invoke("test".to_string(), None).await.unwrap();
    assert_eq!(result, "TEST");
}

#[tokio::test]
async fn test_runnable_binding_batch() {
    // Test that batch() works correctly with bound config
    let lambda = RunnableLambda::new(|input: i32| input * 2);

    let binding = RunnableBindingBase::new(lambda, HashMap::new(), None, vec![]);

    let results = binding.batch(vec![1, 2, 3], None).await.unwrap();
    assert_eq!(results, vec![2, 4, 6]);
}

#[tokio::test]
async fn test_runnable_binding_stream() {
    // Test that stream() works correctly with bound config
    let lambda = RunnableLambda::new(|input: String| input.to_uppercase());

    let binding = RunnableBindingBase::new(lambda, HashMap::new(), None, vec![]);

    let mut stream = binding.stream("hello".to_string(), None).await.unwrap();
    let result = stream.next().await.unwrap().unwrap();
    assert_eq!(result, "HELLO");
}

#[tokio::test]
async fn test_runnable_binding_name_delegation() {
    // Test that name() delegates to the bound runnable
    let lambda = RunnableLambda::new(|input: String| input);

    let binding = RunnableBindingBase::new(lambda, HashMap::new(), None, vec![]);

    assert_eq!(binding.name(), "Lambda");
}

#[tokio::test]
async fn test_runnable_binding_composition() {
    // Test that bindings can be composed in sequences
    let lambda1 = RunnableLambda::new(|input: String| input.to_uppercase());

    let lambda2 = RunnableLambda::new(|input: String| format!("{}!!!", input));

    let binding1 = RunnableBindingBase::new(lambda1, HashMap::new(), None, vec![]);

    let binding2 = RunnableBindingBase::new(lambda2, HashMap::new(), None, vec![]);

    let sequence = RunnableSequence::new(binding1, binding2);
    let result = sequence.invoke("hello".to_string(), None).await.unwrap();
    assert_eq!(result, "HELLO!!!");
}

#[test]
fn test_configurable_field_spec_new() {
    let spec = ConfigurableFieldSpec::new("session_id", "String");
    assert_eq!(spec.id, "session_id");
    assert_eq!(spec.annotation, "String");
    assert_eq!(spec.name, None);
    assert_eq!(spec.description, None);
    assert_eq!(spec.default, None);
    assert!(!spec.is_shared);
    assert_eq!(spec.dependencies, None);
}

#[test]
fn test_configurable_field_spec_builder() {
    let spec = ConfigurableFieldSpec::new("user_id", "String")
        .with_name("User ID")
        .with_description("Unique identifier for the user")
        .with_default(serde_json::json!(""))
        .with_shared(true)
        .with_dependencies(vec!["session_id".to_string()]);

    assert_eq!(spec.id, "user_id");
    assert_eq!(spec.annotation, "String");
    assert_eq!(spec.name, Some("User ID".to_string()));
    assert_eq!(
        spec.description,
        Some("Unique identifier for the user".to_string())
    );
    assert_eq!(spec.default, Some(serde_json::json!("")));
    assert!(spec.is_shared);
    assert_eq!(spec.dependencies, Some(vec!["session_id".to_string()]));
}

#[test]
fn test_configurable_field_spec_equality() {
    let spec1 = ConfigurableFieldSpec::new("session_id", "String")
        .with_name("Session ID")
        .with_shared(true);

    let spec2 = ConfigurableFieldSpec::new("session_id", "String")
        .with_name("Session ID")
        .with_shared(true);

    let spec3 = ConfigurableFieldSpec::new("session_id", "String")
        .with_name("Different Name")
        .with_shared(true);

    assert_eq!(spec1, spec2);
    assert_ne!(spec1, spec3);
}

#[test]
fn test_get_unique_config_specs_no_duplicates() {
    let spec1 = ConfigurableFieldSpec::new("session_id", "String");
    let spec2 = ConfigurableFieldSpec::new("user_id", "String");
    let spec3 = ConfigurableFieldSpec::new("conversation_id", "String");

    let specs = vec![spec1.clone(), spec2.clone(), spec3.clone()];
    let unique = get_unique_config_specs(specs).unwrap();

    assert_eq!(unique.len(), 3);
    // Results should be sorted by ID
    assert_eq!(unique[0].id, "conversation_id");
    assert_eq!(unique[1].id, "session_id");
    assert_eq!(unique[2].id, "user_id");
}

#[test]
fn test_get_unique_config_specs_with_duplicates() {
    let spec1 = ConfigurableFieldSpec::new("session_id", "String").with_name("Session ID");
    let spec2 = ConfigurableFieldSpec::new("session_id", "String").with_name("Session ID"); // identical
    let spec3 = ConfigurableFieldSpec::new("user_id", "String");

    let specs = vec![spec1.clone(), spec2, spec3.clone()];
    let unique = get_unique_config_specs(specs).unwrap();

    // Should deduplicate identical specs
    assert_eq!(unique.len(), 2);
    assert_eq!(unique[0].id, "session_id");
    assert_eq!(unique[1].id, "user_id");
}

#[test]
fn test_get_unique_config_specs_conflict() {
    let spec1 = ConfigurableFieldSpec::new("session_id", "String").with_name("Session ID");
    let spec2 = ConfigurableFieldSpec::new("session_id", "String").with_name("Different Name"); // conflict!

    let specs = vec![spec1, spec2];
    let result = get_unique_config_specs(specs);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Conflicting config specs"));
    assert!(err_msg.contains("session_id"));
}

#[test]
fn test_get_unique_config_specs_empty() {
    let specs: Vec<ConfigurableFieldSpec> = vec![];
    let unique = get_unique_config_specs(specs).unwrap();
    assert_eq!(unique.len(), 0);
}

#[test]
fn test_get_unique_config_specs_complex() {
    // Multiple specs with varying duplicates
    let spec1 = ConfigurableFieldSpec::new("a", "String");
    let spec2 = ConfigurableFieldSpec::new("a", "String"); // duplicate
    let spec3 = ConfigurableFieldSpec::new("b", "i32");
    let spec4 = ConfigurableFieldSpec::new("b", "i32"); // duplicate
    let spec5 = ConfigurableFieldSpec::new("b", "i32"); // triplicate
    let spec6 = ConfigurableFieldSpec::new("c", "bool");

    let specs = vec![
        spec1.clone(),
        spec2,
        spec3.clone(),
        spec4,
        spec5,
        spec6.clone(),
    ];
    let unique = get_unique_config_specs(specs).unwrap();

    assert_eq!(unique.len(), 3);
    assert_eq!(unique[0].id, "a");
    assert_eq!(unique[1].id, "b");
    assert_eq!(unique[2].id, "c");
}
