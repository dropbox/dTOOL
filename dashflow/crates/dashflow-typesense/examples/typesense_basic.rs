//! Basic example of using Typesense vector store.
//!
//! This example demonstrates:
//! 1. Creating a Typesense vector store
//! 2. Adding documents with embeddings
//! 3. Performing similarity search
//! 4. Searching with metadata filters
//!
//! # Setup
//!
//! Before running this example, start Typesense with Docker:
//!
//! ```bash
//! docker run -d \
//!   -p 8108:8108 \
//!   -v /tmp/typesense-data:/data \
//!   -e TYPESENSE_DATA_DIR=/data \
//!   -e TYPESENSE_API_KEY=xyz \
//!   typesense/typesense:27.0
//! ```
//!
//! # Running
//!
//! ```bash
//! export TYPESENSE_API_KEY=xyz
//! cargo run --example typesense_basic
//! ```

use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::vector_stores::VectorStore;
use dashflow_typesense::TypesenseVectorStore;
use std::collections::HashMap;
use std::sync::Arc;

/// Mock embeddings for testing (uses simple character count as embedding).
///
/// In production, use a real embeddings model like OpenAI or Sentence Transformers.
#[derive(Clone)]
struct MockEmbeddings {
    dimension: usize,
}

impl MockEmbeddings {
    fn new(dimension: usize) -> Self {
        Self { dimension }
    }
}

#[async_trait::async_trait]
impl Embeddings for MockEmbeddings {
    async fn _embed_documents(&self, texts: &[String]) -> dashflow::core::Result<Vec<Vec<f32>>> {
        let embeddings = texts
            .iter()
            .map(|text| self.create_mock_embedding(text))
            .collect();
        Ok(embeddings)
    }

    async fn _embed_query(&self, text: &str) -> dashflow::core::Result<Vec<f32>> {
        Ok(self.create_mock_embedding(text))
    }
}

impl MockEmbeddings {
    /// Creates a mock embedding based on text characteristics.
    ///
    /// This is for demonstration only. Real embeddings capture semantic meaning.
    fn create_mock_embedding(&self, text: &str) -> Vec<f32> {
        let mut embedding = vec![0.0; self.dimension];

        // Use text length and character frequencies as simple features
        let normalized_length = (text.len() as f32 / 100.0).min(1.0);
        embedding[0] = normalized_length;

        if self.dimension > 1 {
            let lowercase_ratio =
                text.chars().filter(|c| c.is_lowercase()).count() as f32 / text.len().max(1) as f32;
            embedding[1] = lowercase_ratio;
        }

        if self.dimension > 2 {
            let space_ratio = text.chars().filter(|c| c.is_whitespace()).count() as f32
                / text.len().max(1) as f32;
            embedding[2] = space_ratio;
        }

        // Fill remaining dimensions with normalized character frequencies
        for (i, c) in text.chars().enumerate() {
            let idx = 3 + (i % (self.dimension.saturating_sub(3)));
            if idx < self.dimension {
                embedding[idx] = (c as u32 as f32 / 128.0).min(1.0);
            }
        }

        // Normalize to unit length
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            embedding.iter_mut().for_each(|x| *x /= magnitude);
        }

        embedding
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Typesense Vector Store Example ===\n");

    // 1. Create mock embeddings (384 dimensions, typical for sentence transformers)
    let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));

    // 2. Create Typesense vector store
    println!("Creating Typesense vector store...");
    let mut store = TypesenseVectorStore::new(
        "http://localhost:8108",
        "xyz", // API key
        "dashflow_docs",
        embeddings,
        384,
        "text", // text field name
    )
    .await?;
    println!("✓ Vector store created\n");

    // 3. Prepare sample documents about programming languages
    let documents = vec![
        "Rust is a systems programming language focused on safety and performance.",
        "Python is a high-level interpreted language known for its simplicity.",
        "JavaScript is the language of the web, running in browsers and Node.js.",
        "Go is a compiled language designed for building scalable network services.",
        "TypeScript adds static types to JavaScript for better tooling.",
    ];

    let mut metadatas = Vec::new();
    for (i, _) in documents.iter().enumerate() {
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), serde_json::json!("example"));
        metadata.insert("index".to_string(), serde_json::json!(i));
        metadatas.push(metadata);
    }

    // 4. Add documents to the store
    println!("Adding {} documents...", documents.len());
    let ids = store.add_texts(&documents, Some(&metadatas), None).await?;
    println!("✓ Added documents with IDs:");
    for (i, id) in ids.iter().enumerate() {
        println!("  [{}] {}", i, id);
    }
    println!();

    // 5. Perform similarity search
    println!("Searching for: 'compiled programming language'");
    let results = store
        ._similarity_search("compiled programming language", 3, None)
        .await?;

    println!("Top 3 results:");
    for (i, doc) in results.iter().enumerate() {
        println!("  {}. {}", i + 1, doc.page_content);
    }
    println!();

    // 6. Search with scores
    println!("Searching with scores for: 'web development'");
    let results_with_scores = store
        .similarity_search_with_score("web development", 3, None)
        .await?;

    println!("Top 3 results with scores:");
    for (i, (doc, score)) in results_with_scores.iter().enumerate() {
        println!("  {}. [score: {:.4}] {}", i + 1, score, doc.page_content);
    }
    println!();

    // 7. Search with metadata filter
    println!("Searching with filter (index < 3): 'programming language'");
    let mut filter = HashMap::new();
    filter.insert("index".to_string(), serde_json::json!(2));

    let filtered_results = store
        ._similarity_search("programming language", 5, Some(&filter))
        .await?;

    println!("Filtered results:");
    for (i, doc) in filtered_results.iter().enumerate() {
        println!(
            "  {}. {} (index: {})",
            i + 1,
            doc.page_content,
            doc.metadata
                .get("index")
                .unwrap_or(&serde_json::json!(null))
        );
    }
    println!();

    // 8. Add more documents using Document struct
    println!("Adding documents using Document struct...");
    let new_docs = vec![
        Document {
            id: None,
            page_content: "C++ is an object-oriented extension of C with powerful features."
                .to_string(),
            metadata: {
                let mut m = HashMap::new();
                m.insert("source".to_string(), serde_json::json!("example"));
                m.insert("category".to_string(), serde_json::json!("systems"));
                m
            },
        },
        Document {
            id: None,
            page_content: "Swift is Apple's modern language for iOS and macOS development."
                .to_string(),
            metadata: {
                let mut m = HashMap::new();
                m.insert("source".to_string(), serde_json::json!("example"));
                m.insert("category".to_string(), serde_json::json!("mobile"));
                m
            },
        },
    ];

    let new_ids = store.add_documents(&new_docs, None).await?;
    println!("✓ Added {} new documents", new_ids.len());
    println!();

    // 9. Search again to include new documents
    println!("Searching for: 'Apple mobile development'");
    let final_results = store
        ._similarity_search("Apple mobile development", 2, None)
        .await?;

    println!("Results:");
    for (i, doc) in final_results.iter().enumerate() {
        println!("  {}. {}", i + 1, doc.page_content);
    }
    println!();

    // 10. Clean up - delete specific documents
    println!("Deleting first 2 documents...");
    let deleted = store.delete(Some(&ids[0..2])).await?;
    println!("✓ Deletion successful: {}", deleted);
    println!();

    println!("=== Example completed successfully ===");
    Ok(())
}
