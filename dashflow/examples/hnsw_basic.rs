use dashflow::core::embeddings::MockEmbeddings;
use dashflow::core::vector_stores::VectorStore;
use dashflow_hnsw::{HNSWVectorStore, HNSWConfig, DistanceMetric};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== DashFlow HNSW Vector Store Example ===\n");

    // Create embeddings model (384-dimensional)
    let embeddings = MockEmbeddings::new(384);

    // Configure HNSW parameters
    let config = HNSWConfig {
        dimension: 384,
        max_elements: 10000,
        m: 16,                    // 16 connections per element
        ef_construction: 200,     // Search quality during construction
        distance_metric: DistanceMetric::Cosine,
    };

    println!("Creating HNSW vector store...");
    println!("  Dimension: {}", config.dimension);
    println!("  Max Elements: {}", config.max_elements);
    println!("  M (connections): {}", config.m);
    println!("  ef_construction: {}", config.ef_construction);
    println!("  Distance Metric: {:?}\n", config.distance_metric);

    let mut store = HNSWVectorStore::new(embeddings, config)?;

    // Add programming language descriptions
    println!("Adding documents...");
    let texts = vec![
        "Rust is a systems programming language focused on safety and performance".to_string(),
        "Python is a high-level interpreted language great for data science and scripting".to_string(),
        "JavaScript is the language of the web, running in browsers and Node.js".to_string(),
        "Go is a statically typed compiled language designed for simplicity and concurrency".to_string(),
        "C++ is a powerful language used for game development and performance-critical applications".to_string(),
        "Java is a popular object-oriented language running on the JVM".to_string(),
        "TypeScript adds static typing to JavaScript for better developer experience".to_string(),
        "Swift is Apple's modern programming language for iOS and macOS development".to_string(),
    ];

    let metadatas = vec![
        serde_json::json!({"category": "systems", "year": 2010}),
        serde_json::json!({"category": "scripting", "year": 1991}),
        serde_json::json!({"category": "web", "year": 1995}),
        serde_json::json!({"category": "systems", "year": 2009}),
        serde_json::json!({"category": "systems", "year": 1985}),
        serde_json::json!({"category": "enterprise", "year": 1995}),
        serde_json::json!({"category": "web", "year": 2012}),
        serde_json::json!({"category": "mobile", "year": 2014}),
    ];

    let ids = store.add_texts(texts, Some(metadatas)).await?;
    println!("Added {} documents with IDs: {:?}\n", ids.len(), ids);

    // Similarity search
    println!("=== Similarity Search ===");
    let query1 = "languages for system programming";
    println!("\nQuery: '{}'", query1);
    let results = store.similarity_search(query1, 3, None).await?;
    println!("Top 3 results:");
    for (i, doc) in results.iter().enumerate() {
        println!("  {}. {}", i + 1, doc.page_content);
        if let Some(category) = doc.metadata.get("category") {
            println!("     Category: {}", category);
        }
    }

    // Search with scores
    println!("\n=== Search with Similarity Scores ===");
    let query2 = "web development languages";
    println!("\nQuery: '{}'", query2);
    let results_with_scores = store.similarity_search_with_score(query2, 3, None).await?;
    println!("Top 3 results with scores:");
    for (i, (doc, score)) in results_with_scores.iter().enumerate() {
        println!("  {}. [Score: {:.4}] {}", i + 1, score, doc.page_content);
    }

    // Search by vector
    println!("\n=== Search by Vector ===");
    println!("Searching by the embedding vector of 'compiled languages'...");
    let query_embedding = MockEmbeddings::new(384).embed_query("compiled languages").await?;
    let results = store.similarity_search_by_vector(&query_embedding, 3, None).await?;
    println!("Top 3 results:");
    for (i, doc) in results.iter().enumerate() {
        println!("  {}. {}", i + 1, doc.page_content);
    }

    // Get by IDs
    println!("\n=== Get by IDs ===");
    let get_ids = vec![ids[0].clone(), ids[2].clone()];
    println!("Retrieving documents with IDs: {:?}", get_ids);
    let docs = store.get_by_ids(&get_ids).await?;
    println!("Retrieved {} documents:", docs.len());
    for doc in docs {
        println!("  - {}", doc.page_content.split_whitespace().take(5).collect::<Vec<_>>().join(" ") + "...");
    }

    // Store statistics
    println!("\n=== Store Statistics ===");
    println!("Total documents in store: {}", store.size());

    // Save to disk (optional)
    println!("\n=== Persistence ===");
    let index_path = "/tmp/hnsw_example.hnsw";
    println!("Saving index to: {}", index_path);
    store.save(index_path)?;
    println!("Index saved successfully!");
    println!("Note: Due to hnsw_rs limitations, loading requires rebuilding the index");

    println!("\n=== Performance Notes ===");
    println!("HNSW provides:");
    println!("  - Fast approximate nearest neighbor search (O(log N))");
    println!("  - High recall (95%+ with proper tuning)");
    println!("  - Efficient memory usage (4-8 bytes per dimension)");
    println!("  - Excellent for 10K-10M+ vectors");
    println!("\nFor production use:");
    println!("  - Increase M for better recall (32-48)");
    println!("  - Increase ef_construction for better quality (400-500)");
    println!("  - Tune ef_search per query for speed/accuracy tradeoff");

    Ok(())
}
