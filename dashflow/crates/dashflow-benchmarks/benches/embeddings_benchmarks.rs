//! Performance benchmarks for embeddings models
//!
//! Run with: cargo bench -p dashflow-benchmarks --bench embeddings_benchmarks

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use dashflow::core::embeddings::{Embeddings, MockEmbeddings};

// ============================================================================
// Single Document Embedding Benchmarks
// ============================================================================

fn bench_embed_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("embed_single");
    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        return;
    };

    // Test different embedding dimensions
    for dim in [128, 384, 768, 1536].iter() {
        group.bench_with_input(BenchmarkId::new("embed_dim", dim), dim, |b, &dim| {
            let embeddings = MockEmbeddings::new(dim);
            b.to_async(&runtime).iter(|| async {
                embeddings
                    ._embed_query("What is the capital of France?")
                    .await
                    .unwrap_or_default()
            });
        });
    }

    // Test different text lengths (384-dim embeddings)
    let embeddings_384 = MockEmbeddings::new(384);

    group.bench_function("embed_short_text", |b| {
        b.to_async(&runtime)
            .iter(|| async { embeddings_384._embed_query("Hello world").await.unwrap_or_default() });
    });

    group.bench_function("embed_medium_text", |b| {
        let text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(10);
        b.to_async(&runtime)
            .iter(|| async { embeddings_384._embed_query(&text).await.unwrap_or_default() });
    });

    group.bench_function("embed_long_text", |b| {
        let text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(100);
        b.to_async(&runtime)
            .iter(|| async { embeddings_384._embed_query(&text).await.unwrap_or_default() });
    });

    group.finish();
}

// ============================================================================
// Batch Document Embedding Benchmarks
// ============================================================================

fn bench_embed_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("embed_batch");
    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        return;
    };

    let embeddings = MockEmbeddings::new(384);

    // Test different batch sizes
    for batch_size in [1, 10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                let documents: Vec<String> = (0..batch_size)
                    .map(|i| format!("Document {} with some content", i))
                    .collect();

                b.to_async(&runtime)
                    .iter(|| async { embeddings._embed_documents(&documents).await.unwrap_or_default() });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Mixed Query and Document Embedding Benchmarks
// ============================================================================

fn bench_embed_mixed(c: &mut Criterion) {
    let mut group = c.benchmark_group("embed_mixed");
    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        return;
    };

    let embeddings = MockEmbeddings::new(384);

    // Simulate typical RAG workload: embed 1 query + compare with 100 documents
    group.bench_function("rag_workload_query_100docs", |b| {
        let documents: Vec<String> = (0..100)
            .map(|i| format!("Document {} with some content", i))
            .collect();

        b.to_async(&runtime).iter(|| async {
            // Embed query
            let _ = embeddings
                ._embed_query("What is the answer?")
                .await
                .unwrap_or_default();

            // Embed documents (in real RAG, these would be pre-computed)
            let _ = embeddings._embed_documents(&documents).await.unwrap_or_default();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_embed_single,
    bench_embed_batch,
    bench_embed_mixed,
);
criterion_main!(benches);
