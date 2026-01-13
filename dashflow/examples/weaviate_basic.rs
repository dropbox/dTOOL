use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::vector_stores::VectorStore;
use dashflow_weaviate::WeaviateVectorStore;
use std::sync::Arc;

/// Mock embeddings for demonstration
struct MockEmbeddings;

#[async_trait::async_trait]
impl Embeddings for MockEmbeddings {
    async fn embed_documents(
        &self,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, dashflow::core::Error> {
        // Generate simple embeddings based on text length
        Ok(texts
            .iter()
            .map(|t| vec![t.len() as f32; 384])
            .collect())
    }

    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, dashflow::core::Error> {
        Ok(vec![text.len() as f32; 384])
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create embeddings instance
    let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);

    // Connect to Weaviate (requires Weaviate server running on localhost:8080)
    // NOTE: This is an example with a hardcoded default for simplicity.
    // In production, use std::env::var("WEAVIATE_URL") to make it configurable.
    let weaviate_url =
        std::env::var("WEAVIATE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let mut store = WeaviateVectorStore::new(
        &weaviate_url,
        "ExampleDocs",
        embeddings.clone(),
    )
    .await?;

    println!("Connected to Weaviate");

    // Add some documents
    let docs = vec![
        Document::new("The quick brown fox jumps over the lazy dog"),
        Document::new("Machine learning is a subset of artificial intelligence"),
        Document::new("Rust is a systems programming language"),
    ];

    println!("Adding {} documents...", docs.len());
    let ids = store.add_documents(&docs, None).await?;
    println!("Added documents with IDs: {:?}", ids);

    // Search for similar documents
    let query = "programming languages";
    println!("\nSearching for: '{}'", query);
    let results = store.similarity_search(query, 2, None).await?;

    println!("\nFound {} results:", results.len());
    for (i, doc) in results.iter().enumerate() {
        println!("  {}. {}", i + 1, doc.page_content);
    }

    // Search with scores
    println!("\nSearching with scores:");
    let results_with_scores = store
        .similarity_search_with_score(query, 2, None)
        .await?;

    for (doc, score) in results_with_scores {
        println!("  Score: {:.4} - {}", score, doc.page_content);
    }

    // Get documents by IDs
    println!("\nRetrieving documents by ID:");
    let retrieved = store.get_by_ids(&ids[0..1]).await?;
    for doc in retrieved {
        println!("  Retrieved: {}", doc.page_content);
    }

    // Delete documents
    println!("\nDeleting documents...");
    store.delete(Some(&ids)).await?;
    println!("Documents deleted successfully");

    Ok(())
}
