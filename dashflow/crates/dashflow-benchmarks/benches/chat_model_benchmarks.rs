//! Chat model provider benchmarks
//!
//! Run with: cargo bench -p dashflow-benchmarks --bench chat_model_benchmarks

use criterion::{criterion_group, criterion_main, Criterion};
use dashflow::core::language_models::{ChatGeneration, ChatGenerationChunk, ChatModel, ChatResult};
use dashflow::core::messages::{AIMessage, AIMessageChunk, BaseMessage, HumanMessage};
use futures::StreamExt;
use std::time::Duration;

// ============================================================================
// Mock Chat Model for Benchmarking
// ============================================================================

/// A mock chat model that simulates API call latency without network overhead
#[derive(Clone)]
struct MockChatModel {
    /// Simulated latency per token (default: 0 for pure overhead measurement)
    latency_per_token: Duration,
    /// Response template
    response: String,
}

impl MockChatModel {
    fn new() -> Self {
        Self {
            latency_per_token: Duration::from_micros(0),
            response: "This is a mock response for benchmarking.".to_string(),
        }
    }

    fn with_response(response: String) -> Self {
        Self {
            latency_per_token: Duration::from_micros(0),
            response,
        }
    }
}

#[async_trait::async_trait]
impl ChatModel for MockChatModel {
    async fn _generate(
        &self,
        messages: &[BaseMessage],
        _stop: Option<&[String]>,
        _tools: Option<&[dashflow::core::language_models::ToolDefinition]>,
        _tool_choice: Option<&dashflow::core::language_models::ToolChoice>,
        _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
    ) -> dashflow::core::error::Result<ChatResult> {
        // Simulate processing delay based on input message length
        let total_tokens = messages
            .iter()
            .map(|m| m.content().as_text().len())
            .sum::<usize>();
        let delay = self.latency_per_token * total_tokens as u32;
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }

        Ok(ChatResult {
            generations: vec![ChatGeneration {
                message: AIMessage::new(self.response.as_str()).into(),
                generation_info: None,
            }],
            llm_output: None,
        })
    }

    async fn _stream(
        &self,
        _messages: &[BaseMessage],
        _stop: Option<&[String]>,
        _tools: Option<&[dashflow::core::language_models::ToolDefinition]>,
        _tool_choice: Option<&dashflow::core::language_models::ToolChoice>,
        _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
    ) -> dashflow::core::error::Result<
        std::pin::Pin<
            Box<
                dyn futures::Stream<Item = dashflow::core::error::Result<ChatGenerationChunk>>
                    + Send,
            >,
        >,
    > {
        // Simulate token-by-token streaming
        let response = self.response.clone();
        let latency = self.latency_per_token;

        let chunks: Vec<_> = response.chars().collect();
        let stream = futures::stream::iter(chunks.into_iter().map(move |c| {
            let latency = latency;
            async move {
                if !latency.is_zero() {
                    tokio::time::sleep(latency).await;
                }
                Ok(ChatGenerationChunk {
                    message: AIMessageChunk::new(c.to_string()),
                    generation_info: None,
                })
            }
        }))
        .then(|fut| fut);

        Ok(Box::pin(stream))
    }

    fn llm_type(&self) -> &str {
        "mock_chat_model"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ============================================================================
// Benchmark: Basic Invocation
// ============================================================================

fn bench_basic_invocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("chat_model_invoke");
    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        return;
    };

    // Single message invocation
    group.bench_function("single_message", |b| {
        let model = MockChatModel::new();
        let messages = vec![HumanMessage::new("Hello!").into()];

        b.to_async(&runtime).iter(|| async {
            match model.generate(&messages, None, None, None, None).await {
                Ok(result) => result,
                Err(_) => ChatResult::with_generations(Vec::new()),
            }
        });
    });

    // Multi-turn conversation
    group.bench_function("multi_turn_conversation", |b| {
        let model = MockChatModel::new();
        let messages = vec![
            HumanMessage::new("What is the capital of France?").into(),
            AIMessage::new("The capital of France is Paris.").into(),
            HumanMessage::new("What is its population?").into(),
        ];

        b.to_async(&runtime).iter(|| async {
            match model.generate(&messages, None, None, None, None).await {
                Ok(result) => result,
                Err(_) => ChatResult::with_generations(Vec::new()),
            }
        });
    });

    // Long message (1KB content)
    group.bench_function("long_message_1kb", |b| {
        let model = MockChatModel::new();
        let long_text = "x".repeat(1024);
        let messages = vec![HumanMessage::new(long_text.as_str()).into()];

        b.to_async(&runtime).iter(|| async {
            match model.generate(&messages, None, None, None, None).await {
                Ok(result) => result,
                Err(_) => ChatResult::with_generations(Vec::new()),
            }
        });
    });

    // Very long message (10KB content)
    group.bench_function("long_message_10kb", |b| {
        let model = MockChatModel::new();
        let long_text = "x".repeat(10 * 1024);
        let messages = vec![HumanMessage::new(long_text.as_str()).into()];

        b.to_async(&runtime).iter(|| async {
            match model.generate(&messages, None, None, None, None).await {
                Ok(result) => result,
                Err(_) => ChatResult::with_generations(Vec::new()),
            }
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark: Streaming
// ============================================================================

fn bench_streaming(c: &mut Criterion) {
    let mut group = c.benchmark_group("chat_model_stream");
    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        return;
    };

    // Stream short response (10 tokens)
    group.bench_function("stream_short_10_tokens", |b| {
        let model = MockChatModel::with_response("Short msg!".to_string());
        let messages = vec![HumanMessage::new("Hello!").into()];

        b.to_async(&runtime).iter(|| async {
            if let Ok(mut stream) = model.stream(&messages, None, None, None, None).await {
                let mut count = 0;
                while let Some(_chunk) = stream.next().await {
                    count += 1;
                }
                count
            } else {
                0
            }
        });
    });

    // Stream medium response (50 tokens)
    group.bench_function("stream_medium_50_tokens", |b| {
        let model = MockChatModel::with_response("a".repeat(50));
        let messages = vec![HumanMessage::new("Tell me a story").into()];

        b.to_async(&runtime).iter(|| async {
            if let Ok(mut stream) = model.stream(&messages, None, None, None, None).await {
                let mut count = 0;
                while let Some(_chunk) = stream.next().await {
                    count += 1;
                }
                count
            } else {
                0
            }
        });
    });

    // Stream long response (200 tokens)
    group.bench_function("stream_long_200_tokens", |b| {
        let model = MockChatModel::with_response("a".repeat(200));
        let messages = vec![HumanMessage::new("Write a paragraph").into()];

        b.to_async(&runtime).iter(|| async {
            if let Ok(mut stream) = model.stream(&messages, None, None, None, None).await {
                let mut count = 0;
                while let Some(_chunk) = stream.next().await {
                    count += 1;
                }
                count
            } else {
                0
            }
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark: Message Construction
// ============================================================================

fn bench_message_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_construction");

    // Simple human message
    group.bench_function("human_message_simple", |b| {
        b.iter(|| HumanMessage::new("Hello!"));
    });

    // AI message
    group.bench_function("ai_message_simple", |b| {
        b.iter(|| AIMessage::new("Response"));
    });

    // Convert to BaseMessage
    group.bench_function("convert_to_base_message", |b| {
        b.iter(|| {
            let msg = HumanMessage::new("Hello!");
            let _base: BaseMessage = msg.into();
        });
    });

    // Batch message creation (10 messages)
    group.bench_function("batch_create_10_messages", |b| {
        b.iter(|| {
            let messages: Vec<BaseMessage> = (0..10)
                .map(|i| HumanMessage::new(format!("Message {}", i)).into())
                .collect();
            messages
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark: Batch Invocation
// ============================================================================

fn bench_batch_invocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("chat_model_batch");
    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        return;
    };

    // Batch of 5 single messages
    group.bench_function("batch_5_single_messages", |b| {
        let model = MockChatModel::new();

        b.to_async(&runtime).iter(|| async {
            let tasks: Vec<_> = (0..5)
                .map(|i| {
                    let model = model.clone();
                    let messages = vec![HumanMessage::new(format!("Message {}", i)).into()];
                    async move { model.generate(&messages, None, None, None, None).await }
                })
                .collect();

            futures::future::join_all(tasks).await
        });
    });

    // Batch of 10 single messages
    group.bench_function("batch_10_single_messages", |b| {
        let model = MockChatModel::new();

        b.to_async(&runtime).iter(|| async {
            let tasks: Vec<_> = (0..10)
                .map(|i| {
                    let model = model.clone();
                    let messages = vec![HumanMessage::new(format!("Message {}", i)).into()];
                    async move { model.generate(&messages, None, None, None, None).await }
                })
                .collect();

            futures::future::join_all(tasks).await
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark: Config Overhead
// ============================================================================

fn bench_config_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_overhead");
    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        return;
    };

    // No config
    group.bench_function("invoke_no_config", |b| {
        let model = MockChatModel::new();
        let messages = vec![HumanMessage::new("Hello!").into()];

        b.to_async(&runtime).iter(|| async {
            match model.generate(&messages, None, None, None, None).await {
                Ok(result) => result,
                Err(_) => ChatResult::with_generations(Vec::new()),
            }
        });
    });

    // With config (RunnableConfig has tags, metadata, callbacks)
    group.bench_function("invoke_with_config", |b| {
        let model = MockChatModel::new();
        let messages = vec![HumanMessage::new("Hello!").into()];
        let config = dashflow::core::config::RunnableConfig::new().with_tag("benchmark");

        b.to_async(&runtime).iter(|| async {
            match model.generate(&messages, None, None, None, Some(&config)).await {
                Ok(result) => result,
                Err(_) => ChatResult::with_generations(Vec::new()),
            }
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    benches,
    bench_basic_invocation,
    bench_streaming,
    bench_message_construction,
    bench_batch_invocation,
    bench_config_overhead,
);
criterion_main!(benches);
