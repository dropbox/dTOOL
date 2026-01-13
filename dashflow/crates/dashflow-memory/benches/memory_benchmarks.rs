//! Performance benchmarks for DashFlow Memory implementations
//!
//! This benchmark suite measures the performance of:
//! - Memory operations (save_context, load_memory_variables, clear) for memory types
//! - Backend operations (add_messages, get_messages, clear) for chat message history backends
//!
//! Memory types covered:
//! - ConversationBufferMemory (3 benchmarks)
//! - ConversationBufferWindowMemory (3 benchmarks)
//! - ConversationTokenBufferMemory (3 benchmarks)
//! - SimpleMemory (2 benchmarks)
//! - CombinedMemory (3 benchmarks)
//! - ReadOnlyMemory (3 benchmarks)
//!
//! Backend types covered:
//! - InMemoryChatMessageHistory (3 benchmarks)
//! - FileChatMessageHistory (3 benchmarks)
//!
//! ## Important Note on TokenBuffer Benchmarks
//!
//! TokenBuffer benchmarks were updated to use `iter_batched` pattern,
//! which excludes tiktoken initialization overhead (~27ms one-time cost) from
//! per-operation timing. This improved MEASUREMENT ACCURACY, not code performance.
//!
//! The tokenizer was already cached in the struct (`tokenizer: Arc<CoreBPE>`),
//! but benchmarks were previously measuring init + operation time. The update
//! ensures benchmarks correctly measure operation-only time, reflecting real-world
//! usage where instances are reused.
//!
//! This was a benchmark methodology fix, not a code optimization.
//!
//! Run benchmarks with:
//! ```bash
//! cargo bench --package dashflow-memory
//! ```
//!
//! Run specific benchmark groups:
//! ```bash
//! cargo bench --package dashflow-memory -- memory_operations
//! cargo bench --package dashflow-memory -- backend_operations
//! cargo bench --package dashflow-memory -- readonly_memory
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use dashflow::core::chat_history::{BaseChatMessageHistory, InMemoryChatMessageHistory};
use dashflow::core::messages::Message;
use dashflow_memory::{
    BaseMemory, CombinedMemory, ConversationBufferMemory, ConversationBufferWindowMemory,
    ConversationTokenBufferMemory, FileChatMessageHistory, ReadOnlyMemory, SimpleMemory,
};
use std::collections::HashMap;
use tokio::runtime::Runtime;

// ============================================================================
// Memory Operations Benchmarks
// ============================================================================

/// Benchmark adding messages to ConversationBufferMemory
fn bench_conversation_buffer_save(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_operations/conversation_buffer");

    for size in [1, 10, 100] {
        group.bench_with_input(BenchmarkId::new("save_context", size), &size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let history = InMemoryChatMessageHistory::new();
                let mut memory = ConversationBufferMemory::new(history);

                for i in 0..size {
                    let mut inputs = HashMap::new();
                    inputs.insert("input".to_string(), format!("Message {}", i));
                    let mut outputs = HashMap::new();
                    outputs.insert("output".to_string(), format!("Response {}", i));
                    memory.save_context(&inputs, &outputs).await.unwrap();
                }
            });
        });
    }

    group.finish();
}

/// Benchmark getting memory variables from ConversationBufferMemory
fn bench_conversation_buffer_load(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_operations/conversation_buffer");

    for size in [1, 10, 100] {
        group.bench_with_input(
            BenchmarkId::new("load_memory_variables", size),
            &size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let history = InMemoryChatMessageHistory::new();
                    let mut memory = ConversationBufferMemory::new(history);

                    // Pre-populate
                    for i in 0..size {
                        let mut inputs = HashMap::new();
                        inputs.insert("input".to_string(), format!("Message {}", i));
                        let mut outputs = HashMap::new();
                        outputs.insert("output".to_string(), format!("Response {}", i));
                        memory.save_context(&inputs, &outputs).await.unwrap();
                    }

                    // Benchmark retrieval
                    memory.load_memory_variables(&HashMap::new()).await.unwrap()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark clearing ConversationBufferMemory
fn bench_conversation_buffer_clear(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_operations/conversation_buffer");

    for size in [1, 10, 100] {
        group.bench_with_input(BenchmarkId::new("clear", size), &size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let history = InMemoryChatMessageHistory::new();
                let mut memory = ConversationBufferMemory::new(history);

                // Pre-populate
                for i in 0..size {
                    let mut inputs = HashMap::new();
                    inputs.insert("input".to_string(), format!("Message {}", i));
                    let mut outputs = HashMap::new();
                    outputs.insert("output".to_string(), format!("Response {}", i));
                    memory.save_context(&inputs, &outputs).await.unwrap();
                }

                // Benchmark clear
                memory.clear().await.unwrap()
            });
        });
    }

    group.finish();
}

/// Benchmark adding messages to ConversationBufferWindowMemory
fn bench_conversation_window_save(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_operations/conversation_window");

    for size in [1, 10, 100] {
        group.bench_with_input(BenchmarkId::new("save_context", size), &size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let history = InMemoryChatMessageHistory::new();
                let mut memory = ConversationBufferWindowMemory::new(history).with_k(5);

                for i in 0..size {
                    let mut inputs = HashMap::new();
                    inputs.insert("input".to_string(), format!("Message {}", i));
                    let mut outputs = HashMap::new();
                    outputs.insert("output".to_string(), format!("Response {}", i));
                    memory.save_context(&inputs, &outputs).await.unwrap();
                }
            });
        });
    }

    group.finish();
}

/// Benchmark getting memory variables from ConversationBufferWindowMemory
fn bench_conversation_window_load(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_operations/conversation_window");

    for size in [1, 10, 100] {
        group.bench_with_input(
            BenchmarkId::new("load_memory_variables", size),
            &size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let history = InMemoryChatMessageHistory::new();
                    let mut memory = ConversationBufferWindowMemory::new(history).with_k(5);

                    // Pre-populate
                    for i in 0..size {
                        let mut inputs = HashMap::new();
                        inputs.insert("input".to_string(), format!("Message {}", i));
                        let mut outputs = HashMap::new();
                        outputs.insert("output".to_string(), format!("Response {}", i));
                        memory.save_context(&inputs, &outputs).await.unwrap();
                    }

                    // Benchmark retrieval
                    memory.load_memory_variables(&HashMap::new()).await.unwrap()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark clearing ConversationBufferWindowMemory
fn bench_conversation_window_clear(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_operations/conversation_window");

    for size in [1, 10, 100] {
        group.bench_with_input(BenchmarkId::new("clear", size), &size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let history = InMemoryChatMessageHistory::new();
                let mut memory = ConversationBufferWindowMemory::new(history).with_k(5);

                // Pre-populate
                for i in 0..size {
                    let mut inputs = HashMap::new();
                    inputs.insert("input".to_string(), format!("Message {}", i));
                    let mut outputs = HashMap::new();
                    outputs.insert("output".to_string(), format!("Response {}", i));
                    memory.save_context(&inputs, &outputs).await.unwrap();
                }

                // Benchmark clear
                memory.clear().await.unwrap()
            });
        });
    }

    group.finish();
}

/// Benchmark adding messages to ConversationTokenBufferMemory
fn bench_token_buffer_save(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_operations/token_buffer");

    for size in [1, 10, 100] {
        group.bench_with_input(BenchmarkId::new("save_context", size), &size, |b, &size| {
            b.to_async(&rt).iter_batched(
                || {
                    // Setup (not timed): Create memory instance with tokenizer initialized
                    let history = InMemoryChatMessageHistory::new();
                    ConversationTokenBufferMemory::new(history, 1000, "history").unwrap()
                },
                |mut memory| async move {
                    // Timed section: Only measure save_context operations
                    for i in 0..size {
                        let mut inputs = HashMap::new();
                        inputs.insert("input".to_string(), format!("Message {}", i));
                        let mut outputs = HashMap::new();
                        outputs.insert("output".to_string(), format!("Response {}", i));
                        memory.save_context(&inputs, &outputs).await.unwrap();
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark getting memory variables from ConversationTokenBufferMemory
///
/// Note: This benchmark includes initialization cost (tiktoken_rs::cl100k_base())
/// on first call per iteration batch. In real-world usage, users would typically
/// create one TokenBuffer instance and reuse it, amortizing initialization cost.
fn bench_token_buffer_load(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_operations/token_buffer");

    for size in [1, 10, 100] {
        group.bench_with_input(
            BenchmarkId::new("load_memory_variables", size),
            &size,
            |b, &size| {
                b.to_async(&rt).iter_batched(
                    || {
                        // Setup: Create memory (includes tiktoken initialization)
                        let history = InMemoryChatMessageHistory::new();
                        let memory =
                            ConversationTokenBufferMemory::new(history, 1000, "history").unwrap();
                        (memory, size)
                    },
                    |(mut memory, size)| async move {
                        // Pre-populate + measure load (timed section)
                        for i in 0..size {
                            let mut inputs = HashMap::new();
                            inputs.insert("input".to_string(), format!("Message {}", i));
                            let mut outputs = HashMap::new();
                            outputs.insert("output".to_string(), format!("Response {}", i));
                            memory.save_context(&inputs, &outputs).await.unwrap();
                        }
                        memory.load_memory_variables(&HashMap::new()).await.unwrap()
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark clearing ConversationTokenBufferMemory
///
/// Note: This benchmark includes initialization cost (tiktoken_rs::cl100k_base())
/// on first call per iteration batch. In real-world usage, users would typically
/// create one TokenBuffer instance and reuse it, amortizing initialization cost.
fn bench_token_buffer_clear(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_operations/token_buffer");

    for size in [1, 10, 100] {
        group.bench_with_input(BenchmarkId::new("clear", size), &size, |b, &size| {
            b.to_async(&rt).iter_batched(
                || {
                    // Setup: Create memory (includes tiktoken initialization)
                    let history = InMemoryChatMessageHistory::new();
                    let memory =
                        ConversationTokenBufferMemory::new(history, 1000, "history").unwrap();
                    (memory, size)
                },
                |(mut memory, size)| async move {
                    // Pre-populate + measure clear (timed section)
                    for i in 0..size {
                        let mut inputs = HashMap::new();
                        inputs.insert("input".to_string(), format!("Message {}", i));
                        let mut outputs = HashMap::new();
                        outputs.insert("output".to_string(), format!("Response {}", i));
                        memory.save_context(&inputs, &outputs).await.unwrap();
                    }
                    memory.clear().await.unwrap()
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark creating and loading SimpleMemory
fn bench_simple_memory_load(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_operations/simple_memory");

    for size in [1, 10, 100] {
        group.bench_with_input(
            BenchmarkId::new("load_memory_variables", size),
            &size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let mut vars = HashMap::new();
                    for i in 0..size {
                        vars.insert(format!("key_{}", i), format!("value_{}", i));
                    }
                    let memory = SimpleMemory::new(vars);

                    // Benchmark retrieval
                    memory.load_memory_variables(&HashMap::new()).await.unwrap()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark clearing SimpleMemory
fn bench_simple_memory_clear(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_operations/simple_memory");

    for size in [1, 10, 100] {
        group.bench_with_input(BenchmarkId::new("clear", size), &size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let mut vars = HashMap::new();
                for i in 0..size {
                    vars.insert(format!("key_{}", i), format!("value_{}", i));
                }
                let mut memory = SimpleMemory::new(vars);

                // Benchmark clear
                memory.clear().await.unwrap()
            });
        });
    }

    group.finish();
}

/// Benchmark adding messages to CombinedMemory
fn bench_combined_memory_save(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_operations/combined_memory");

    for size in [1, 10, 100] {
        group.bench_with_input(BenchmarkId::new("save_context", size), &size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                // Create two sub-memories with different memory variables
                let history1 = InMemoryChatMessageHistory::new();
                let memory1 =
                    ConversationBufferMemory::new(history1).with_memory_key("history1".to_string());

                let history2 = InMemoryChatMessageHistory::new();
                let memory2 =
                    ConversationBufferMemory::new(history2).with_memory_key("history2".to_string());

                let mut combined = CombinedMemory::new(vec![
                    Box::new(memory1) as Box<dyn BaseMemory>,
                    Box::new(memory2) as Box<dyn BaseMemory>,
                ])
                .unwrap();

                for i in 0..size {
                    let mut inputs = HashMap::new();
                    inputs.insert("input".to_string(), format!("Message {}", i));
                    let mut outputs = HashMap::new();
                    outputs.insert("output".to_string(), format!("Response {}", i));
                    combined.save_context(&inputs, &outputs).await.unwrap();
                }
            });
        });
    }

    group.finish();
}

/// Benchmark getting memory variables from CombinedMemory
fn bench_combined_memory_load(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_operations/combined_memory");

    for size in [1, 10, 100] {
        group.bench_with_input(
            BenchmarkId::new("load_memory_variables", size),
            &size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    // Create two sub-memories with different memory variables
                    let history1 = InMemoryChatMessageHistory::new();
                    let mut memory1 = ConversationBufferMemory::new(history1)
                        .with_memory_key("history1".to_string());

                    let history2 = InMemoryChatMessageHistory::new();
                    let mut memory2 = ConversationBufferMemory::new(history2)
                        .with_memory_key("history2".to_string());

                    // Pre-populate both sub-memories
                    for i in 0..size {
                        let mut inputs = HashMap::new();
                        inputs.insert("input".to_string(), format!("Message {}", i));
                        let mut outputs = HashMap::new();
                        outputs.insert("output".to_string(), format!("Response {}", i));
                        memory1.save_context(&inputs, &outputs).await.unwrap();
                        memory2.save_context(&inputs, &outputs).await.unwrap();
                    }

                    let combined = CombinedMemory::new(vec![
                        Box::new(memory1) as Box<dyn BaseMemory>,
                        Box::new(memory2) as Box<dyn BaseMemory>,
                    ])
                    .unwrap();

                    // Benchmark retrieval
                    combined
                        .load_memory_variables(&HashMap::new())
                        .await
                        .unwrap()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark clearing CombinedMemory
fn bench_combined_memory_clear(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_operations/combined_memory");

    for size in [1, 10, 100] {
        group.bench_with_input(BenchmarkId::new("clear", size), &size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                // Create two sub-memories with different memory variables
                let history1 = InMemoryChatMessageHistory::new();
                let mut memory1 =
                    ConversationBufferMemory::new(history1).with_memory_key("history1".to_string());

                let history2 = InMemoryChatMessageHistory::new();
                let mut memory2 =
                    ConversationBufferMemory::new(history2).with_memory_key("history2".to_string());

                // Pre-populate both sub-memories
                for i in 0..size {
                    let mut inputs = HashMap::new();
                    inputs.insert("input".to_string(), format!("Message {}", i));
                    let mut outputs = HashMap::new();
                    outputs.insert("output".to_string(), format!("Response {}", i));
                    memory1.save_context(&inputs, &outputs).await.unwrap();
                    memory2.save_context(&inputs, &outputs).await.unwrap();
                }

                let mut combined = CombinedMemory::new(vec![
                    Box::new(memory1) as Box<dyn BaseMemory>,
                    Box::new(memory2) as Box<dyn BaseMemory>,
                ])
                .unwrap();

                // Benchmark clear
                combined.clear().await.unwrap()
            });
        });
    }

    group.finish();
}

/// Benchmark loading memory variables from ReadOnlyMemory
fn bench_readonly_memory_load(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_operations/readonly_memory");

    for size in [1, 10, 100] {
        group.bench_with_input(
            BenchmarkId::new("load_memory_variables", size),
            &size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let history = InMemoryChatMessageHistory::new();
                    let mut memory = ConversationBufferMemory::new(history);

                    // Pre-populate
                    for i in 0..size {
                        let mut inputs = HashMap::new();
                        inputs.insert("input".to_string(), format!("Message {}", i));
                        let mut outputs = HashMap::new();
                        outputs.insert("output".to_string(), format!("Response {}", i));
                        memory.save_context(&inputs, &outputs).await.unwrap();
                    }

                    // Wrap in readonly
                    let readonly = ReadOnlyMemory::new(memory);

                    // Benchmark retrieval
                    readonly
                        .load_memory_variables(&HashMap::new())
                        .await
                        .unwrap()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark save_context on ReadOnlyMemory (should be no-op)
fn bench_readonly_memory_save(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_operations/readonly_memory");

    for size in [1, 10, 100] {
        group.bench_with_input(
            BenchmarkId::new("save_context_noop", size),
            &size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let history = InMemoryChatMessageHistory::new();
                    let mut memory = ConversationBufferMemory::new(history);

                    // Pre-populate
                    let mut inputs = HashMap::new();
                    inputs.insert("input".to_string(), "Initial".to_string());
                    let mut outputs = HashMap::new();
                    outputs.insert("output".to_string(), "Data".to_string());
                    memory.save_context(&inputs, &outputs).await.unwrap();

                    // Wrap in readonly
                    let mut readonly = ReadOnlyMemory::new(memory);

                    // Benchmark no-op writes
                    for i in 0..size {
                        let mut inputs = HashMap::new();
                        inputs.insert("input".to_string(), format!("Message {}", i));
                        let mut outputs = HashMap::new();
                        outputs.insert("output".to_string(), format!("Response {}", i));
                        readonly.save_context(&inputs, &outputs).await.unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark clear on ReadOnlyMemory (should be no-op)
fn bench_readonly_memory_clear(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_operations/readonly_memory");

    for size in [1, 10, 100] {
        group.bench_with_input(BenchmarkId::new("clear_noop", size), &size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let history = InMemoryChatMessageHistory::new();
                let mut memory = ConversationBufferMemory::new(history);

                // Pre-populate
                for i in 0..size {
                    let mut inputs = HashMap::new();
                    inputs.insert("input".to_string(), format!("Message {}", i));
                    let mut outputs = HashMap::new();
                    outputs.insert("output".to_string(), format!("Response {}", i));
                    memory.save_context(&inputs, &outputs).await.unwrap();
                }

                // Wrap in readonly
                let mut readonly = ReadOnlyMemory::new(memory);

                // Benchmark no-op clear
                readonly.clear().await.unwrap()
            });
        });
    }

    group.finish();
}

// ============================================================================
// Backend Operations Benchmarks
// ============================================================================

/// Benchmark adding messages to InMemoryChatMessageHistory
fn bench_backend_inmemory_add(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("backend_operations/inmemory");

    for size in [1, 10, 100] {
        group.bench_with_input(BenchmarkId::new("add_messages", size), &size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let history = InMemoryChatMessageHistory::new();

                for i in 0..size {
                    let msg = Message::human(format!("Message {}", i));
                    history.add_messages(&[msg]).await.unwrap();
                }
            });
        });
    }

    group.finish();
}

/// Benchmark getting messages from InMemoryChatMessageHistory
fn bench_backend_inmemory_get(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("backend_operations/inmemory");

    for size in [1, 10, 100] {
        group.bench_with_input(BenchmarkId::new("get_messages", size), &size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let history = InMemoryChatMessageHistory::new();

                // Pre-populate
                for i in 0..size {
                    let msg = Message::human(format!("Message {}", i));
                    history.add_messages(&[msg]).await.unwrap();
                }

                // Benchmark retrieval
                history.get_messages().await.unwrap()
            });
        });
    }

    group.finish();
}

/// Benchmark clearing InMemoryChatMessageHistory
fn bench_backend_inmemory_clear(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("backend_operations/inmemory");

    for size in [1, 10, 100] {
        group.bench_with_input(BenchmarkId::new("clear", size), &size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let history = InMemoryChatMessageHistory::new();

                // Pre-populate
                for i in 0..size {
                    let msg = Message::human(format!("Message {}", i));
                    history.add_messages(&[msg]).await.unwrap();
                }

                // Benchmark clear
                history.clear().await.unwrap()
            });
        });
    }

    group.finish();
}

/// Benchmark adding messages to FileChatMessageHistory
fn bench_backend_file_add(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("backend_operations/file");

    for size in [1, 10, 100] {
        group.bench_with_input(BenchmarkId::new("add_messages", size), &size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let temp_dir = tempfile::tempdir().unwrap();
                let file_path = temp_dir.path().join("history.json");
                let history = FileChatMessageHistory::new(&file_path).await.unwrap();

                for i in 0..size {
                    let msg = Message::human(format!("Message {}", i));
                    history.add_messages(&[msg]).await.unwrap();
                }
            });
        });
    }

    group.finish();
}

/// Benchmark getting messages from FileChatMessageHistory
fn bench_backend_file_get(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("backend_operations/file");

    for size in [1, 10, 100] {
        group.bench_with_input(BenchmarkId::new("get_messages", size), &size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let temp_dir = tempfile::tempdir().unwrap();
                let file_path = temp_dir.path().join("history.json");
                let history = FileChatMessageHistory::new(&file_path).await.unwrap();

                // Pre-populate
                for i in 0..size {
                    let msg = Message::human(format!("Message {}", i));
                    history.add_messages(&[msg]).await.unwrap();
                }

                // Benchmark retrieval
                history.get_messages().await.unwrap()
            });
        });
    }

    group.finish();
}

/// Benchmark clearing FileChatMessageHistory
fn bench_backend_file_clear(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("backend_operations/file");

    for size in [1, 10, 100] {
        group.bench_with_input(BenchmarkId::new("clear", size), &size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let temp_dir = tempfile::tempdir().unwrap();
                let file_path = temp_dir.path().join("history.json");
                let history = FileChatMessageHistory::new(&file_path).await.unwrap();

                // Pre-populate
                for i in 0..size {
                    let msg = Message::human(format!("Message {}", i));
                    history.add_messages(&[msg]).await.unwrap();
                }

                // Benchmark clear
                history.clear().await.unwrap()
            });
        });
    }

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    memory_operations,
    bench_conversation_buffer_save,
    bench_conversation_buffer_load,
    bench_conversation_buffer_clear,
    bench_conversation_window_save,
    bench_conversation_window_load,
    bench_conversation_window_clear,
    bench_token_buffer_save,
    bench_token_buffer_load,
    bench_token_buffer_clear,
    bench_simple_memory_load,
    bench_simple_memory_clear,
    bench_combined_memory_save,
    bench_combined_memory_load,
    bench_combined_memory_clear,
    bench_readonly_memory_load,
    bench_readonly_memory_save,
    bench_readonly_memory_clear,
);

criterion_group!(
    backend_operations,
    bench_backend_inmemory_add,
    bench_backend_inmemory_get,
    bench_backend_inmemory_clear,
    bench_backend_file_add,
    bench_backend_file_get,
    bench_backend_file_clear,
);

criterion_main!(memory_operations, backend_operations);
