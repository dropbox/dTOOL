use dashflow::core::embeddings::Embeddings;
use dashflow::core::vector_stores::VectorStore;
use dashflow::core::Error;
use dashflow_sqlitevss::SQLiteVSSStore;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// Mock embeddings for demonstration.
/// In production, use a real embeddings implementation like OpenAI, Ollama, etc.
#[derive(Clone)]
struct MockEmbeddings {
    dimensions: usize,
}

#[async_trait]
impl Embeddings for MockEmbeddings {
    async fn embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, Error> {
        // Generate simple mock embeddings based on text length and content
        Ok(texts
            .iter()
            .map(|text| {
                let mut embedding = vec![0.0; self.dimensions];
                // Simple hash-based embedding for demo
                let bytes = text.as_bytes();
                for (i, byte) in bytes.iter().enumerate() {
                    embedding[i % self.dimensions] += *byte as f32 / 255.0;
                }
                // Normalize
                let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    embedding.iter_mut().for_each(|x| *x /= norm);
                }
                embedding
            })
            .collect())
    }

    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, Error> {
        let result = self.embed_documents(&[text.to_string()]).await?;
        Ok(result.into_iter().next().unwrap())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("SQLite VSS Vector Store Example\n");

    // Create mock embeddings (384 dimensions)
    let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dimensions: 384 });

    // Create in-memory SQLite VSS store
    println!("Creating SQLite VSS store (in-memory)...");
    let mut store = SQLiteVSSStore::new(embeddings.clone(), ":memory:", 384, None)?;

    // Add some documents
    println!("\nAdding documents...");
    let texts = vec![
        "The cat sat on the mat",
        "The dog played in the park",
        "The bird flew over the tree",
        "A quick brown fox jumps over the lazy dog",
        "The weather is sunny today",
    ];

    let metadatas = vec![
        HashMap::from([
            ("category".to_string(), serde_json::json!("animals")),
            ("type".to_string(), serde_json::json!("cat")),
        ]),
        HashMap::from([
            ("category".to_string(), serde_json::json!("animals")),
            ("type".to_string(), serde_json::json!("dog")),
        ]),
        HashMap::from([
            ("category".to_string(), serde_json::json!("animals")),
            ("type".to_string(), serde_json::json!("bird")),
        ]),
        HashMap::from([
            ("category".to_string(), serde_json::json!("animals")),
            ("type".to_string(), serde_json::json!("fox")),
        ]),
        HashMap::from([
            ("category".to_string(), serde_json::json!("weather")),
            ("type".to_string(), serde_json::json!("forecast")),
        ]),
    ];

    let ids = store.add_texts(&texts, Some(&metadatas), None).await?;
    println!("Added {} documents", ids.len());
    for (i, id) in ids.iter().enumerate() {
        println!("  [{}] {} (ID: {})", i + 1, texts[i], id);
    }

    // Search for similar documents
    println!("\n--- Similarity Search ---");
    let query = "feline on carpet";
    println!("Query: \"{}\"", query);
    let results = store.similarity_search(query, 3, None).await?;
    println!("Top 3 results:");
    for (i, doc) in results.iter().enumerate() {
        println!(
            "  [{}] {} (category: {})",
            i + 1,
            doc.page_content,
            doc.metadata.get("category").unwrap_or(&serde_json::json!("N/A"))
        );
    }

    // Search with scores
    println!("\n--- Search with Relevance Scores ---");
    let query = "animal activities";
    println!("Query: \"{}\"", query);
    let results = store.similarity_search_with_score(query, 3, None).await?;
    println!("Top 3 results:");
    for (i, (doc, score)) in results.iter().enumerate() {
        println!(
            "  [{}] {} (score: {:.4})",
            i + 1,
            doc.page_content,
            score
        );
    }

    // Search with metadata filter
    println!("\n--- Search with Metadata Filter ---");
    let query = "outdoor activities";
    let filter = HashMap::from([
        ("category".to_string(), serde_json::json!("animals")),
    ]);
    println!("Query: \"{}\"", query);
    println!("Filter: category = animals");
    let results = store.similarity_search(query, 5, Some(&filter)).await?;
    println!("Results:");
    for (i, doc) in results.iter().enumerate() {
        println!(
            "  [{}] {} (type: {})",
            i + 1,
            doc.page_content,
            doc.metadata.get("type").unwrap_or(&serde_json::json!("N/A"))
        );
    }

    // Delete a document
    println!("\n--- Delete Document ---");
    let id_to_delete = &ids[0];
    println!("Deleting document with ID: {}", id_to_delete);
    store.delete(Some(&[id_to_delete.clone()])).await?;

    // Verify deletion
    let remaining = store.get_by_ids(&ids[1..]).await?;
    println!("Remaining documents: {}", remaining.len());

    // File-based example
    println!("\n--- File-based Database Example ---");
    let db_path = "example_vectors.db";
    println!("Creating file-based store: {}", db_path);
    let mut file_store = SQLiteVSSStore::new(embeddings, db_path, 384, None)?;

    let texts = vec!["First document", "Second document"];
    file_store.add_texts(&texts, None, None).await?;
    println!("Added documents to file-based store");
    println!("Database persisted to: {}", db_path);
    println!("(Note: You can delete {} after this example)", db_path);

    println!("\n✓ Example completed successfully!");

    Ok(())
}
