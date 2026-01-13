//! # Chroma Vector Store CRUD Validation
//!
//! Validates Rust Chroma implementation against Python DashFlow baseline.
//! Run Python baseline first: `./test_chroma_crud_parity.py`
//! Then run this example: `cargo run --package dashflow-chroma --example chroma_validation`
//!
//! **IMPORTANT**: This example uses `ConsistentFakeEmbeddings` for validation testing only.
//! For production use, replace with a real embeddings provider like:
//! - `dashflow_openai::OpenAIEmbeddings` for OpenAI text-embedding models
//! - `dashflow_huggingface::HuggingFaceEmbeddings` for local/hosted HuggingFace models
//!
//! **Prerequisites:**
//! - Start Chroma server: `docker run -p 8000:8000 chromadb/chroma`
//!
//! Test Coverage:
//! 1. Basic add_texts + similarity_search
//! 2. Add with custom IDs
//! 3. Add with metadata and metadata filtering
//! 4. Delete operations
//! 5. MMR (Maximal Marginal Relevance) search
//! 6. Similarity search with scores
//!
//! Expected outputs should match Python baseline functionally (not byte-for-byte).

use async_trait::async_trait;
use dashflow::core::{
    documents::Document, embeddings::Embeddings, vector_stores::VectorStore, Error,
};
use dashflow_chroma::ChromaVectorStore;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// ConsistentFakeEmbeddings - matches Python baseline implementation
///
/// **WARNING**: This is a FAKE embeddings implementation for validation testing only!
/// It returns deterministic embeddings based on text order, not semantic meaning.
/// This ensures identical behavior to Python for validation, but is NOT suitable
/// for production use. For real applications, use a proper embeddings model like
/// OpenAI, HuggingFace, or other embedding providers.
struct ConsistentFakeEmbeddings {
    known_texts: Arc<Mutex<Vec<String>>>,
    dimensionality: usize,
}

impl ConsistentFakeEmbeddings {
    fn new(dimensionality: usize) -> Self {
        Self {
            known_texts: Arc::new(Mutex::new(Vec::new())),
            dimensionality,
        }
    }

    fn embed_text(&self, text: &str) -> Vec<f32> {
        let mut known = match self.known_texts.lock() {
            Ok(known) => known,
            Err(poisoned) => poisoned.into_inner(),
        };

        let index = match known.iter().position(|t| t == text) {
            Some(index) => index,
            None => {
                known.push(text.to_string());
                known.len() - 1
            }
        };

        // Create embedding: [1.0, 1.0, ..., index] (dimensionality-1 ones + index)
        let mut embedding = vec![1.0; self.dimensionality - 1];
        embedding.push(index as f32);

        embedding
    }
}

#[async_trait]
impl Embeddings for ConsistentFakeEmbeddings {
    async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, Error> {
        Ok(texts.iter().map(|text| self.embed_text(text)).collect())
    }

    async fn _embed_query(&self, text: &str) -> Result<Vec<f32>, Error> {
        Ok(self.embed_text(text))
    }
}

async fn test_basic_add_and_search() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Test 1: Basic add_texts and similarity_search ===");

    let embeddings: Arc<dyn Embeddings> = Arc::new(ConsistentFakeEmbeddings::new(10));
    let mut store =
        ChromaVectorStore::new("test_basic", embeddings, Some("http://localhost:8000")).await?;

    // Add texts
    let texts = vec!["Hello world", "Machine learning is great", "Rust is fast"];
    let ids = store.add_texts(&texts, None, None).await?;
    println!("Added {} documents", ids.len());
    println!("Generated IDs: {:?}", ids);

    // Search
    let results = store._similarity_search("learning", 2, None).await?;
    println!("\nSearch for 'learning' (k=2):");
    for (i, doc) in results.iter().enumerate() {
        println!(
            "  Result {}: '{}' (id: {:?})",
            i + 1,
            doc.page_content,
            doc.id
        );
    }

    // Validate
    assert_eq!(
        results.len(),
        2,
        "Expected 2 results, got {}",
        results.len()
    );
    assert!(texts.contains(&results[0].page_content.as_str()));

    // Cleanup
    store.delete(None).await?;

    println!("✅ Test 1 passed");
    Ok(())
}

async fn test_add_with_custom_ids() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Test 2: Add texts with custom IDs ===");

    let embeddings: Arc<dyn Embeddings> = Arc::new(ConsistentFakeEmbeddings::new(10));
    let mut store =
        ChromaVectorStore::new("test_ids", embeddings, Some("http://localhost:8000")).await?;

    // Add with custom IDs
    let texts = vec!["First doc", "Second doc", "Third doc"];
    let custom_ids = vec!["id_0".to_string(), "id_1".to_string(), "id_2".to_string()];
    let returned_ids = store.add_texts(&texts, None, Some(&custom_ids)).await?;

    println!("Custom IDs: {:?}", custom_ids);
    println!("Returned IDs: {:?}", returned_ids);

    // Search and verify IDs
    let results = store._similarity_search("First", 1, None).await?;
    println!("\nSearch for 'First' (k=1):");
    println!(
        "  Result: '{}' (id: {:?})",
        results[0].page_content, results[0].id
    );

    // Validate
    assert_eq!(returned_ids, custom_ids, "IDs should match");
    assert_eq!(
        results[0].id,
        Some("id_0".to_string()),
        "Expected id_0, got {:?}",
        results[0].id
    );

    // Cleanup
    store.delete(None).await?;

    println!("✅ Test 2 passed");
    Ok(())
}

async fn test_metadata_filtering() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Test 3: Metadata filtering ===");

    let embeddings: Arc<dyn Embeddings> = Arc::new(ConsistentFakeEmbeddings::new(10));
    let mut store =
        ChromaVectorStore::new("test_metadata", embeddings, Some("http://localhost:8000")).await?;

    // Add documents with metadata
    let docs = [
        Document {
            id: None,
            page_content: "Python tutorial".to_string(),
            metadata: {
                let mut m = HashMap::new();
                m.insert("lang".to_string(), json!("python"));
                m.insert("level".to_string(), json!("beginner"));
                m
            },
        },
        Document {
            id: None,
            page_content: "Rust tutorial".to_string(),
            metadata: {
                let mut m = HashMap::new();
                m.insert("lang".to_string(), json!("rust"));
                m.insert("level".to_string(), json!("beginner"));
                m
            },
        },
        Document {
            id: None,
            page_content: "Advanced Rust patterns".to_string(),
            metadata: {
                let mut m = HashMap::new();
                m.insert("lang".to_string(), json!("rust"));
                m.insert("level".to_string(), json!("advanced"));
                m
            },
        },
    ];

    // Extract texts and metadatas for add_texts
    let texts: Vec<String> = docs.iter().map(|d| d.page_content.clone()).collect();
    let metadatas: Vec<HashMap<String, serde_json::Value>> =
        docs.iter().map(|d| d.metadata.clone()).collect();

    let ids = store.add_texts(&texts, Some(&metadatas), None).await?;
    println!("Added {} documents with metadata", ids.len());

    // Search without filter
    let all_results = store._similarity_search("tutorial", 3, None).await?;
    println!(
        "\nSearch 'tutorial' (no filter, k=3): {} results",
        all_results.len()
    );

    // Search with metadata filter
    let mut filter = HashMap::new();
    filter.insert("lang".to_string(), json!("rust"));

    let rust_results = store
        ._similarity_search("tutorial", 3, Some(&filter))
        .await?;
    println!(
        "Search 'tutorial' (filter lang=rust, k=3): {} results",
        rust_results.len()
    );
    for (i, doc) in rust_results.iter().enumerate() {
        let lang = doc
            .metadata
            .get("lang")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        println!("  Result {}: '{}' (lang={})", i + 1, doc.page_content, lang);
    }

    // Validate
    assert_eq!(
        rust_results.len(),
        2,
        "Expected 2 Rust docs, got {}",
        rust_results.len()
    );
    for doc in &rust_results {
        assert_eq!(
            doc.metadata.get("lang").and_then(|v| v.as_str()),
            Some("rust"),
            "All results should have lang=rust"
        );
    }

    // Cleanup
    store.delete(None).await?;

    println!("✅ Test 3 passed");
    Ok(())
}

async fn test_search_with_scores() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Test 4: Similarity search with scores ===");

    let embeddings: Arc<dyn Embeddings> = Arc::new(ConsistentFakeEmbeddings::new(10));
    let mut store =
        ChromaVectorStore::new("test_scores", embeddings, Some("http://localhost:8000")).await?;

    // Add documents
    let texts = vec!["exact match", "similar text", "completely different topic"];
    let custom_ids = vec![
        "doc_0".to_string(),
        "doc_1".to_string(),
        "doc_2".to_string(),
    ];
    store.add_texts(&texts, None, Some(&custom_ids)).await?;

    // Search with scores
    let results = store
        .similarity_search_with_score("exact match", 2, None)
        .await?;
    println!("\nSearch 'exact match' with scores (k=2):");
    for (i, (doc, score)) in results.iter().enumerate() {
        println!(
            "  Result {}: '{}' (id: {:?}, score: {:.4})",
            i + 1,
            doc.page_content,
            doc.id,
            score
        );
    }

    // Validate
    assert_eq!(
        results.len(),
        2,
        "Expected 2 results, got {}",
        results.len()
    );
    assert_eq!(
        results[0].0.page_content, "exact match",
        "First result should be exact match"
    );
    println!("  Best score: {:.4}", results[0].1);

    // Cleanup
    store.delete(None).await?;

    println!("✅ Test 4 passed");
    Ok(())
}

async fn test_delete_operations() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Test 5: Delete operations ===");

    let embeddings: Arc<dyn Embeddings> = Arc::new(ConsistentFakeEmbeddings::new(10));
    let mut store =
        ChromaVectorStore::new("test_delete", embeddings, Some("http://localhost:8000")).await?;

    // Add documents
    let texts = vec!["Doc 1", "Doc 2", "Doc 3", "Doc 4"];
    let ids = vec![
        "id_1".to_string(),
        "id_2".to_string(),
        "id_3".to_string(),
        "id_4".to_string(),
    ];
    store.add_texts(&texts, None, Some(&ids)).await?;

    // Count before delete
    let count_before = store._similarity_search("Doc", 10, None).await?.len();
    println!("Documents before delete: {}", count_before);

    // Delete specific IDs
    let delete_ids = vec!["id_2".to_string(), "id_4".to_string()];
    store.delete(Some(&delete_ids)).await?;

    // Count after delete
    let count_after = store._similarity_search("Doc", 10, None).await?.len();
    println!("Documents after delete: {}", count_after);

    // Validate
    assert_eq!(
        count_before, 4,
        "Expected 4 docs initially, got {}",
        count_before
    );
    assert_eq!(
        count_after, 2,
        "Expected 2 docs after delete, got {}",
        count_after
    );

    // Verify deleted IDs not present
    let all_results = store._similarity_search("Doc", 10, None).await?;
    let result_ids: Vec<String> = all_results.iter().filter_map(|d| d.id.clone()).collect();
    println!("Remaining IDs: {:?}", result_ids);
    assert!(!result_ids.contains(&"id_2".to_string()));
    assert!(!result_ids.contains(&"id_4".to_string()));

    // Cleanup
    store.delete(None).await?;

    println!("✅ Test 5 passed");
    Ok(())
}

async fn test_mmr_search() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Test 6: MMR search ===");

    let embeddings: Arc<dyn Embeddings> = Arc::new(ConsistentFakeEmbeddings::new(10));
    let mut store =
        ChromaVectorStore::new("test_mmr", embeddings, Some("http://localhost:8000")).await?;

    // Add documents - some very similar, some diverse
    let texts = vec![
        "Machine learning fundamentals",
        "Machine learning basics", // Very similar to first
        "Deep learning with neural networks",
        "Rust programming language",
        "Python programming guide",
    ];
    store.add_texts(&texts, None, None).await?;

    // Regular similarity search
    let regular_results = store._similarity_search("machine learning", 3, None).await?;
    println!("\nRegular search 'machine learning' (k=3):");
    for (i, doc) in regular_results.iter().enumerate() {
        println!("  Result {}: '{}'", i + 1, doc.page_content);
    }

    // MMR search
    let mmr_results = store
        .max_marginal_relevance_search(
            "machine learning",
            3,   // k
            5,   // fetch_k
            0.5, // lambda
            None,
        )
        .await?;
    println!("\nMMR search 'machine learning' (k=3, lambda=0.5):");
    for (i, doc) in mmr_results.iter().enumerate() {
        println!("  Result {}: '{}'", i + 1, doc.page_content);
    }

    // Validate
    assert_eq!(
        mmr_results.len(),
        3,
        "Expected 3 results, got {}",
        mmr_results.len()
    );

    // Count how many results contain "machine learning"
    let ml_count_regular = regular_results
        .iter()
        .filter(|d| d.page_content.to_lowercase().contains("machine learning"))
        .count();
    let ml_count_mmr = mmr_results
        .iter()
        .filter(|d| d.page_content.to_lowercase().contains("machine learning"))
        .count();

    println!(
        "\n  Regular search: {}/3 contain 'machine learning'",
        ml_count_regular
    );
    println!(
        "  MMR search: {}/3 contain 'machine learning'",
        ml_count_mmr
    );
    println!(
        "  MMR provided {} diversity",
        if ml_count_mmr < ml_count_regular {
            "more"
        } else {
            "same or less"
        }
    );

    // Cleanup
    store.delete(None).await?;

    println!("✅ Test 6 passed");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!("Chroma Vector Store CRUD Validation - Rust Implementation");
    println!("============================================================");
    println!("\nPrerequisites:");
    println!("  1. Docker Chroma running: docker run -p 8000:8000 chromadb/chroma");
    println!("  2. Run Python baseline first: ./test_chroma_crud_parity.py");

    // Run all tests
    test_basic_add_and_search().await?;
    test_add_with_custom_ids().await?;
    test_metadata_filtering().await?;
    test_search_with_scores().await?;
    test_delete_operations().await?;
    test_mmr_search().await?;

    println!("\n============================================================");
    println!("✅ ALL TESTS PASSED");
    println!("============================================================");
    println!("\nValidation complete! Compare outputs with Python baseline.");

    Ok(())
}
