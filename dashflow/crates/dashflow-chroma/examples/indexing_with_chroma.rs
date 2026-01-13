//! Document Indexing with Chroma Vector Store
//!
//! Demonstrates intelligent document indexing with Chroma using the indexing API.
//! Shows how to:
//! - Track indexed documents with RecordManager
//! - Detect and skip unchanged documents using content hashing
//! - Incrementally update documents with automatic cleanup
//! - Prevent duplicate content in vector stores
//!
//! **Requirements**:
//! - Running Chroma server: `docker run -p 8000:8000 chromadb/chroma`
//! - OpenAI API key for embeddings (set OPENAI_API_KEY environment variable)
//!
//! Run with: cargo run --example indexing_with_chroma

use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::indexing::{
    index, CleanupMode, HashAlgorithm, InMemoryRecordManager, RecordManager,
};
use dashflow_chroma::ChromaVectorStore;
use std::sync::Arc;

/// Mock embeddings for testing without API calls
/// In production, use OpenAIEmbeddings or other real embeddings
struct MockEmbeddings;

#[async_trait::async_trait]
impl Embeddings for MockEmbeddings {
    async fn _embed_documents(
        &self,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, dashflow::core::Error> {
        // Simple deterministic embeddings based on text length
        Ok(texts
            .iter()
            .map(|t| {
                let len = t.len() as f32;
                vec![len / 100.0, (len % 10.0) / 10.0, 1.0 - (len / 200.0)]
            })
            .collect())
    }

    async fn _embed_query(&self, text: &str) -> Result<Vec<f32>, dashflow::core::Error> {
        let len = text.len() as f32;
        Ok(vec![len / 100.0, (len % 10.0) / 10.0, 1.0 - (len / 200.0)])
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Document Indexing with Chroma Example\n");
    println!("========================================\n");

    // Create embeddings provider (use mock for demo, or OpenAI for production)
    let embeddings = Arc::new(MockEmbeddings);
    // For production:
    // let embeddings = Arc::new(OpenAIEmbeddings::default());

    // Create Chroma vector store
    println!("📦 Connecting to Chroma vector store...");
    let vector_store =
        ChromaVectorStore::new("indexing_demo", embeddings, Some("http://localhost:8000")).await?;
    println!("✅ Connected to Chroma collection: indexing_demo\n");

    // Create record manager to track indexed documents
    let record_manager = Arc::new(InMemoryRecordManager::new("indexing_demo_namespace"));

    // Initialize schema
    record_manager
        .create_schema()
        .await
        .map_err(|e| format!("Failed to create schema: {}", e))?;
    println!("✅ Initialized record manager\n");

    // ========================================================================
    // Example 1: Initial Indexing
    // ========================================================================
    println!("📝 Example 1: Initial Indexing");
    println!("------------------------------");

    let initial_docs = vec![
        Document::new("The quick brown fox jumps over the lazy dog")
            .with_metadata("source", "doc1.txt")
            .with_metadata("chunk", 0),
        Document::new("Lorem ipsum dolor sit amet, consectetur adipiscing elit")
            .with_metadata("source", "doc2.txt")
            .with_metadata("chunk", 0),
        Document::new("Rust is a systems programming language focused on safety")
            .with_metadata("source", "doc3.txt")
            .with_metadata("chunk", 0),
        Document::new("Machine learning is a subset of artificial intelligence")
            .with_metadata("source", "doc4.txt")
            .with_metadata("chunk", 0),
        Document::new("Vector databases store high-dimensional embeddings efficiently")
            .with_metadata("source", "doc5.txt")
            .with_metadata("chunk", 0),
    ];

    let result = index(
        initial_docs.clone(),
        record_manager.as_ref(),
        &vector_store,
        CleanupMode::None, // No cleanup on initial indexing
        Some("source"),
        64,    // Batch size
        64,    // Cleanup batch size
        false, // Don't force update
        HashAlgorithm::Sha256,
        None, // No custom key encoder
    )
    .await
    .map_err(|e| format!("Indexing failed: {}", e))?;

    println!("Results:");
    println!("  📥 Added: {}", result.num_added);
    println!("  🔄 Updated: {}", result.num_updated);
    let num_unchanged = result.num_skipped;
    println!("  ⏭️  Unchanged: {}", num_unchanged);
    println!("  🗑️  Deleted: {}", result.num_deleted);
    println!();

    // ========================================================================
    // Example 2: Re-indexing Unchanged Documents (All Skipped)
    // ========================================================================
    println!("📝 Example 2: Re-indexing Unchanged Documents");
    println!("--------------------------------------------");

    let result = index(
        initial_docs.clone(), // Same documents, no changes
        record_manager.as_ref(),
        &vector_store,
        CleanupMode::None,
        Some("source"),
        64,
        64,
        false,
        HashAlgorithm::Sha256,
        None,
    )
    .await
    .map_err(|e| format!("Indexing failed: {}", e))?;

    println!("Results:");
    println!("  📥 Added: {}", result.num_added);
    println!("  🔄 Updated: {}", result.num_updated);
    let num_unchanged = result.num_skipped;
    println!("  ⏭️  Unchanged: {} (all unchanged!)", num_unchanged);
    println!("  🗑️  Deleted: {}", result.num_deleted);
    println!();

    // ========================================================================
    // Example 3: Incremental Update with Cleanup
    // ========================================================================
    println!("📝 Example 3: Incremental Update with Cleanup");
    println!("--------------------------------------------");

    // Update doc1.txt (content changed)
    // Keep doc2.txt, doc3.txt (unchanged)
    // Remove doc4.txt, doc5.txt (no longer in source)
    // Add doc6.txt (new document)
    let updated_docs = vec![
        Document::new("The quick brown fox jumps over the fence") // Changed!
            .with_metadata("source", "doc1.txt")
            .with_metadata("chunk", 0),
        Document::new("Lorem ipsum dolor sit amet, consectetur adipiscing elit") // Unchanged
            .with_metadata("source", "doc2.txt")
            .with_metadata("chunk", 0),
        Document::new("Rust is a systems programming language focused on safety") // Unchanged
            .with_metadata("source", "doc3.txt")
            .with_metadata("chunk", 0),
        Document::new("DashFlow provides a framework for building LLM applications") // New!
            .with_metadata("source", "doc6.txt")
            .with_metadata("chunk", 0),
    ];

    let result = index(
        updated_docs,
        record_manager.as_ref(),
        &vector_store,
        CleanupMode::Incremental, // Clean up old docs from same sources
        Some("source"),
        64,
        64,
        false,
        HashAlgorithm::Sha256,
        None,
    )
    .await
    .map_err(|e| format!("Indexing failed: {}", e))?;

    println!("Results:");
    println!("  📥 Added: {} (doc6.txt)", result.num_added);
    println!("  🔄 Updated: {} (doc1.txt changed)", result.num_updated);
    println!(
        "  ⏭️  Skipped: {} (doc2.txt, doc3.txt unchanged)",
        result.num_skipped
    );
    println!(
        "  🗑️  Deleted: {} (doc4.txt, doc5.txt removed from source)",
        result.num_deleted
    );
    println!();

    // ========================================================================
    // Example 4: Force Update (Re-embed All)
    // ========================================================================
    println!("📝 Example 4: Force Update (Re-embed All)");
    println!("----------------------------------------");
    println!("Useful when embeddings model changes or vector store needs refresh\n");

    let current_docs = vec![
        Document::new("The quick brown fox jumps over the fence")
            .with_metadata("source", "doc1.txt")
            .with_metadata("chunk", 0),
        Document::new("Lorem ipsum dolor sit amet, consectetur adipiscing elit")
            .with_metadata("source", "doc2.txt")
            .with_metadata("chunk", 0),
    ];

    let result = index(
        current_docs,
        record_manager.as_ref(),
        &vector_store,
        CleanupMode::None,
        Some("source"),
        64,
        64,
        true, // Force update = re-index even if unchanged
        HashAlgorithm::Sha256,
        None,
    )
    .await
    .map_err(|e| format!("Indexing failed: {}", e))?;

    println!("Results:");
    println!("  📥 Added: {}", result.num_added);
    println!(
        "  🔄 Updated: {} (all documents re-indexed)",
        result.num_updated
    );
    println!(
        "  ⏭️  Skipped: {} (force update overrides)",
        result.num_skipped
    );
    println!("  🗑️  Deleted: {}", result.num_deleted);
    println!();

    // ========================================================================
    // Example 5: Deduplication
    // ========================================================================
    println!("📝 Example 5: Automatic Deduplication");
    println!("------------------------------------");

    let docs_with_duplicates = vec![
        Document::new("Unique document 1").with_metadata("source", "unique1.txt"),
        Document::new("Unique document 2").with_metadata("source", "unique2.txt"),
        Document::new("Duplicate content").with_metadata("source", "dup1.txt"), // Duplicate
        Document::new("Unique document 3").with_metadata("source", "unique3.txt"),
        Document::new("Duplicate content").with_metadata("source", "dup2.txt"), // Duplicate
    ];

    println!(
        "Input: {} documents (2 duplicates)",
        docs_with_duplicates.len()
    );

    let result = index(
        docs_with_duplicates,
        record_manager.as_ref(),
        &vector_store,
        CleanupMode::None,
        Some("source"),
        64,
        64,
        false,
        HashAlgorithm::Sha256,
        None,
    )
    .await
    .map_err(|e| format!("Indexing failed: {}", e))?;

    println!("\nResults:");
    println!("  📥 Added: {}", result.num_added);
    println!("  🔄 Updated: {}", result.num_updated);
    println!(
        "  ⏭️  Skipped: {} (duplicates detected by hash)",
        result.num_skipped
    );
    println!("  🗑️  Deleted: {}", result.num_deleted);
    println!();

    // ========================================================================
    // Example 6: Different Hash Algorithms
    // ========================================================================
    println!("📝 Example 6: Hash Algorithm Comparison");
    println!("--------------------------------------");

    let test_doc = [Document::new("Test document for hashing comparison")
        .with_metadata("source", "hash_test.txt")];

    println!("Same content, different algorithms:");
    println!();

    for algo in &[
        HashAlgorithm::Sha1,
        HashAlgorithm::Sha256,
        HashAlgorithm::Sha512,
        HashAlgorithm::Blake2b,
    ] {
        let hash_result = dashflow::core::indexing::hash_document(&test_doc[0], *algo);
        println!(
            "  {:?}: {}",
            algo,
            &hash_result[..32.min(hash_result.len())]
        ); // First 32 chars
    }

    println!();
    println!("✅ All examples completed!");
    println!();
    println!("💡 Key Takeaways:");
    println!("  1. Index() avoids re-indexing unchanged documents (saves API calls)");
    println!("  2. Incremental cleanup removes outdated docs from same source");
    println!("  3. Force update re-indexes all docs (useful for model changes)");
    println!("  4. Deduplication prevents duplicate content in vector stores");
    println!("  5. Different hash algorithms provide different ID formats");

    Ok(())
}
