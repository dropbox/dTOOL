//! Basic usage example for WeaviateVectorStore
//!
//! This example demonstrates:
//! - Creating a WeaviateVectorStore instance
//! - Adding documents with embeddings
//! - Performing similarity search
//! - Retrieving documents by ID
//! - Deleting documents
//!
//! # Prerequisites
//!
//! Before running this example, ensure you have:
//! 1. A running Weaviate instance (e.g., via Docker):
//!    ```bash
//!    docker run -d -p 8080:8080 \
//!      -e QUERY_DEFAULTS_LIMIT=20 \
//!      -e AUTHENTICATION_ANONYMOUS_ACCESS_ENABLED=true \
//!      -e PERSISTENCE_DATA_PATH=/var/lib/weaviate \
//!      -e DEFAULT_VECTORIZER_MODULE=none \
//!      -e CLUSTER_HOSTNAME=node1 \
//!      semitechnologies/weaviate:latest
//!    ```
//! 2. Set environment variable:
//!    ```bash
//!    export OPENAI_API_KEY=your-api-key
//!    ```
//!
//! # Usage
//!
//! Run with:
//! ```bash
//! cargo run --example basic_usage --package dashflow-weaviate
//! ```

use dashflow::core::vector_stores::VectorStore;
use dashflow_openai::embeddings::OpenAIEmbeddings;
use dashflow_weaviate::WeaviateVectorStore;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Weaviate Vector Store Basic Usage Example ===\n");

    // 1. Create embeddings provider
    println!("1. Creating OpenAI embeddings provider...");
    let embeddings = Arc::new(OpenAIEmbeddings::default());

    // 2. Connect to Weaviate and create vector store
    println!("2. Connecting to Weaviate (http://localhost:8080)...");
    let mut store =
        WeaviateVectorStore::new("http://localhost:8080", "LangchainDocs", embeddings).await?;
    println!("   ✓ Connected successfully\n");

    // 3. Add documents
    println!("3. Adding documents to vector store...");
    let texts = vec![
        "DashFlow is a framework for developing applications powered by language models.",
        "Vector stores are databases optimized for storing and searching high-dimensional vectors.",
        "Weaviate is an open-source vector database with support for semantic search.",
        "Embeddings convert text into numerical vectors that capture semantic meaning.",
    ];

    let ids = store.add_texts(&texts, None, None).await?;
    println!("   ✓ Added {} documents", ids.len());
    println!("   Document IDs: {:?}\n", ids);

    // 4. Similarity search
    println!("4. Performing similarity search...");
    let query = "What is a vector database?";
    println!("   Query: \"{}\"", query);

    let results = store._similarity_search(query, 2, None).await?;
    println!("   ✓ Found {} similar documents:", results.len());
    for (i, doc) in results.iter().enumerate() {
        println!(
            "     {}. {}",
            i + 1,
            &doc.page_content[..80.min(doc.page_content.len())]
        );
    }
    println!();

    // 5. Similarity search with scores
    println!("5. Similarity search with scores...");
    let results_with_scores = store.similarity_search_with_score(query, 2, None).await?;
    println!("   ✓ Results with similarity scores:");
    for (i, (doc, score)) in results_with_scores.iter().enumerate() {
        println!(
            "     {}. [Score: {:.4}] {}",
            i + 1,
            score,
            &doc.page_content[..60.min(doc.page_content.len())]
        );
    }
    println!();

    // 6. Get documents by ID
    println!("6. Retrieving documents by ID...");
    let retrieved_docs = store.get_by_ids(&ids[..2]).await?;
    println!("   ✓ Retrieved {} documents:", retrieved_docs.len());
    for doc in &retrieved_docs {
        println!(
            "     - {}",
            &doc.page_content[..60.min(doc.page_content.len())]
        );
    }
    println!();

    // 7. Delete documents
    println!("7. Deleting documents...");
    let deleted = store.delete(Some(&ids)).await?;
    println!("   ✓ Deletion successful: {}\n", deleted);

    // 8. Verify deletion
    println!("8. Verifying deletion...");
    let search_after_delete = store._similarity_search("DashFlow", 5, None).await?;
    println!(
        "   ✓ Documents remaining after deletion: {}",
        search_after_delete.len()
    );
    if search_after_delete.is_empty() {
        println!("   (All documents successfully deleted)");
    }

    println!("\n=== Example completed successfully! ===");
    Ok(())
}
