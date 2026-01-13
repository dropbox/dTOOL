use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::vector_stores::VectorStore;
use dashflow_opensearch::OpenSearchVectorStore;
use std::sync::Arc;

/// Mock embeddings for demonstration
struct MockEmbeddings;

#[async_trait::async_trait]
impl Embeddings for MockEmbeddings {
    async fn embed_documents(
        &self,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, dashflow::core::Error> {
        // Generate simple embeddings based on text length and character distribution
        Ok(texts
            .iter()
            .map(|t| {
                let mut embedding = vec![0.0; 1536];
                let bytes = t.as_bytes();
                for (i, &byte) in bytes.iter().enumerate() {
                    embedding[i % 1536] += byte as f32 / 255.0;
                }
                // Normalize
                let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                if magnitude > 0.0 {
                    embedding.iter_mut().for_each(|x| *x /= magnitude);
                }
                embedding
            })
            .collect())
    }

    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, dashflow::core::Error> {
        let mut embedding = vec![0.0; 1536];
        let bytes = text.as_bytes();
        for (i, &byte) in bytes.iter().enumerate() {
            embedding[i % 1536] += byte as f32 / 255.0;
        }
        // Normalize
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            embedding.iter_mut().for_each(|x| *x /= magnitude);
        }
        Ok(embedding)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("OpenSearch Vector Store Example");
    println!("=================================\n");

    // Create embeddings instance
    let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);

    // Connect to OpenSearch (requires OpenSearch running on localhost:9200)
    // NOTE: This is an example with a hardcoded default for simplicity.
    // In production, use std::env::var("OPENSEARCH_URL").unwrap_or_else(|_| "https://localhost:9200".to_string())
    let opensearch_url =
        std::env::var("OPENSEARCH_URL").unwrap_or_else(|_| "https://localhost:9200".to_string());
    println!("Connecting to OpenSearch at {opensearch_url}...");
    let mut store = OpenSearchVectorStore::new(
        "dashflow_example", // index name
        embeddings.clone(),
        &opensearch_url,
    )
    .await?;
    println!("Connected successfully!\n");

    // Add some documents
    let docs = vec![
        Document::new("OpenSearch is an open-source search and analytics suite"),
        Document::new("It supports vector similarity search using the k-NN plugin"),
        Document::new("HNSW algorithm provides fast approximate nearest neighbor search"),
        Document::new("Rust is a systems programming language focused on safety and performance"),
        Document::new("DashFlow helps build applications with large language models"),
    ];

    println!("Adding {} documents...", docs.len());
    let ids = store.add_documents(&docs, None).await?;
    println!("✓ Added documents with IDs: {:?}\n", ids);

    // Search for similar documents
    let query = "vector search algorithms";
    println!("Searching for: '{}'", query);
    let results = store.similarity_search(query, 3, None).await?;

    println!("\n✓ Found {} results:", results.len());
    for (i, doc) in results.iter().enumerate() {
        println!("  {}. {}", i + 1, doc.page_content);
    }

    // Search with scores
    println!("\n\nSearching with scores:");
    let results_with_scores = store
        .similarity_search_with_score(query, 3, None)
        .await?;

    println!("✓ Results with similarity scores:");
    for (doc, score) in &results_with_scores {
        println!("  Score: {:.4} - {}", score, doc.page_content);
    }

    // Get documents by IDs
    println!("\n\nRetrieving specific documents by ID:");
    let retrieved = store.get_by_ids(&ids[0..2]).await?;
    println!("✓ Retrieved {} documents:", retrieved.len());
    for doc in retrieved {
        println!("  - {}", doc.page_content);
    }

    // Search by vector directly
    println!("\n\nSearching by vector directly:");
    let query_embedding = embeddings.embed_query("search technology").await?;
    let vector_results = store
        .similarity_search_by_vector(&query_embedding, 2, None)
        .await?;
    println!("✓ Found {} results:", vector_results.len());
    for (i, doc) in vector_results.iter().enumerate() {
        println!("  {}. {}", i + 1, doc.page_content);
    }

    // Delete documents
    println!("\n\nCleaning up: Deleting documents...");
    store.delete(Some(&ids)).await?;
    println!("✓ Documents deleted successfully");

    println!("\nExample completed successfully!");
    Ok(())
}
