//! Performance benchmarks for vector stores
//!
//! Run with: cargo bench -p dashflow-benchmarks --bench vectorstore_benchmarks

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use dashflow::core::documents::Document;
use dashflow::core::embeddings::MockEmbeddings;
use dashflow::core::vector_stores::{InMemoryVectorStore, VectorStore};
use std::sync::Arc;

// ============================================================================
// Vector Store Setup Helpers
// ============================================================================

async fn create_test_store(num_docs: usize) -> InMemoryVectorStore {
    let embeddings = Arc::new(MockEmbeddings::new(384)); // 384-dimensional embeddings
    let mut store = InMemoryVectorStore::new(embeddings);

    // Add documents
    let mut documents = Vec::new();
    for i in 0..num_docs {
        let doc = Document::new(format!("Document {} content with some text", i))
            .with_metadata("id", i.to_string())
            .with_metadata("category", format!("cat_{}", i % 10));
        documents.push(doc);
    }

    if store.add_documents(&documents, None).await.is_err() {
        return store;
    }
    store
}

// ============================================================================
// Vector Store Addition Benchmarks
// ============================================================================

fn bench_vector_store_add(c: &mut Criterion) {
    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        return;
    };
    let mut group = c.benchmark_group("vectorstore_add");

    // Test different document batch sizes
    for size in [1, 10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.to_async(&runtime).iter(|| async move {
                let embeddings = Arc::new(MockEmbeddings::new(384));
                let mut store = InMemoryVectorStore::new(embeddings);

                let mut documents = Vec::new();
                for i in 0..size {
                    let doc = Document::new(format!("Document {}", i));
                    documents.push(doc);
                }

                drop(store.add_documents(&documents, None).await);
            });
        });
    }

    group.finish();
}

// ============================================================================
// Similarity Search Benchmarks
// ============================================================================

fn bench_similarity_search(c: &mut Criterion) {
    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        return;
    };
    let mut group = c.benchmark_group("similarity_search");

    // Pre-create stores with different sizes
    let store_100 = runtime.block_on(create_test_store(100));
    let store_1000 = runtime.block_on(create_test_store(1000));
    let store_10000 = runtime.block_on(create_test_store(10000));

    // Benchmark: Search in 100-document store (k=5)
    group.bench_function("search_100_docs_k5", |b| {
        b.to_async(&runtime).iter(|| async {
            store_100
                ._similarity_search("query about documents", 5, None)
                .await
                .unwrap_or_default()
        });
    });

    // Benchmark: Search in 1000-document store (k=5)
    group.bench_function("search_1000_docs_k5", |b| {
        b.to_async(&runtime).iter(|| async {
            store_1000
                ._similarity_search("query about documents", 5, None)
                .await
                .unwrap_or_default()
        });
    });

    // Benchmark: Search in 10000-document store (k=5)
    group.bench_function("search_10000_docs_k5", |b| {
        b.to_async(&runtime).iter(|| async {
            store_10000
                ._similarity_search("query about documents", 5, None)
                .await
                .unwrap_or_default()
        });
    });

    // Benchmark: Different k values (1000-doc store)
    group.bench_function("search_1000_docs_k1", |b| {
        b.to_async(&runtime).iter(|| async {
            store_1000
                ._similarity_search("query about documents", 1, None)
                .await
                .unwrap_or_default()
        });
    });

    group.bench_function("search_1000_docs_k10", |b| {
        b.to_async(&runtime).iter(|| async {
            store_1000
                ._similarity_search("query about documents", 10, None)
                .await
                .unwrap_or_default()
        });
    });

    group.bench_function("search_1000_docs_k50", |b| {
        b.to_async(&runtime).iter(|| async {
            store_1000
                ._similarity_search("query about documents", 50, None)
                .await
                .unwrap_or_default()
        });
    });

    group.finish();
}

// ============================================================================
// Similarity Search with Score Benchmarks
// ============================================================================

fn bench_similarity_search_with_score(c: &mut Criterion) {
    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        return;
    };
    let mut group = c.benchmark_group("similarity_search_with_score");

    let store_1000 = runtime.block_on(create_test_store(1000));

    group.bench_function("search_with_score_1000_docs_k5", |b| {
        b.to_async(&runtime).iter(|| async {
            store_1000
                .similarity_search_with_score("query about documents", 5, None)
                .await
                .unwrap_or_default()
        });
    });

    group.finish();
}

// ============================================================================
// Maximum Marginal Relevance (MMR) Benchmarks
// ============================================================================

fn bench_mmr_search(c: &mut Criterion) {
    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        return;
    };
    let mut group = c.benchmark_group("mmr_search");

    let store_1000 = runtime.block_on(create_test_store(1000));

    // MMR with default parameters (lambda=0.5, fetch_k=20, k=5)
    group.bench_function("mmr_1000_docs_default", |b| {
        b.to_async(&runtime).iter(|| async {
            store_1000
                .max_marginal_relevance_search(
                    "query about documents",
                    5,    // k
                    20,   // fetch_k
                    0.5,  // lambda (diversity vs relevance)
                    None, // no metadata filter
                )
                .await
                .unwrap_or_default()
        });
    });

    // MMR with high diversity (lambda=0.1)
    group.bench_function("mmr_1000_docs_high_diversity", |b| {
        b.to_async(&runtime).iter(|| async {
            store_1000
                .max_marginal_relevance_search(
                    "query about documents",
                    5,    // k
                    20,   // fetch_k
                    0.1,  // lambda (more diverse)
                    None, // no metadata filter
                )
                .await
                .unwrap_or_default()
        });
    });

    // MMR with high relevance (lambda=0.9)
    group.bench_function("mmr_1000_docs_high_relevance", |b| {
        b.to_async(&runtime).iter(|| async {
            store_1000
                .max_marginal_relevance_search(
                    "query about documents",
                    5,    // k
                    20,   // fetch_k
                    0.9,  // lambda (more relevant)
                    None, // no metadata filter
                )
                .await
                .unwrap_or_default()
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_vector_store_add,
    bench_similarity_search,
    bench_similarity_search_with_score,
    bench_mmr_search,
);
criterion_main!(benches);
