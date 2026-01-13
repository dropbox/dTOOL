use dashflow::core::embeddings::MockEmbeddings;
use dashflow::core::vector_stores::VectorStore;
use dashflow_neo4j::{DistanceStrategy, Neo4jVector, Neo4jVectorConfig};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Neo4j Vector Store Example ===\n");

    // Note: This example requires Neo4j 5.11+ running locally
    // Start Neo4j with: docker run -p 7474:7474 -p 7687:7687 -e NEO4J_AUTH=neo4j/password neo4j:5.11

    // Create embeddings model (384-dimensional mock embeddings)
    let embeddings = Arc::new(MockEmbeddings::new(384));

    // Configure Neo4j connection
    let uri = "bolt://localhost:7687";
    let user = "neo4j";
    let password = "password"; // Change to your password

    println!("Connecting to Neo4j at {}...", uri);

    // Create custom configuration
    let config = Neo4jVectorConfig {
        database: "neo4j".to_string(),
        node_label: "Document".to_string(),
        embedding_property: "embedding".to_string(),
        text_property: "text".to_string(),
        id_property: "id".to_string(),
        index_name: "document_vectors".to_string(),
        distance_strategy: DistanceStrategy::Cosine,
    };

    // Connect to Neo4j and create vector store
    let mut store = match Neo4jVector::with_config(uri, user, password, embeddings, config).await {
        Ok(s) => {
            println!("✓ Connected to Neo4j successfully\n");
            s
        }
        Err(e) => {
            eprintln!("✗ Failed to connect to Neo4j: {}", e);
            eprintln!("\nMake sure Neo4j 5.11+ is running:");
            eprintln!("  docker run -p 7474:7474 -p 7687:7687 -e NEO4J_AUTH=neo4j/password neo4j:5.11");
            return Err(e.into());
        }
    };

    // Sample texts about databases and AI
    let texts = vec![
        "Neo4j is a graph database management system that uses Cypher query language",
        "PostgreSQL is a powerful open-source relational database system",
        "MongoDB is a document-oriented NoSQL database program",
        "Vector databases enable semantic similarity search using embeddings",
        "Machine learning models can generate embeddings for text data",
        "Graph databases excel at handling complex relationships between data",
    ];

    println!("Adding {} texts to Neo4j...", texts.len());
    let ids = store.add_texts(&texts, None, None).await?;
    println!("✓ Added texts with IDs: {:?}\n", ids);

    // Similarity search example
    println!("--- Similarity Search ---");
    let query1 = "What is a graph database?";
    println!("Query: \"{}\"", query1);

    let results = store.similarity_search(query1, 3, None).await?;
    println!("Top 3 results:");
    for (i, doc) in results.iter().enumerate() {
        println!("  {}. {}", i + 1, doc.page_content);
    }
    println!();

    // Similarity search with scores
    println!("--- Similarity Search with Scores ---");
    let query2 = "vector embeddings";
    println!("Query: \"{}\"", query2);

    let results_with_scores = store.similarity_search_with_score(query2, 3, None).await?;
    println!("Top 3 results:");
    for (i, (doc, score)) in results_with_scores.iter().enumerate() {
        println!("  {}. [Score: {:.4}] {}", i + 1, score, doc.page_content);
    }
    println!();

    // Search by vector
    println!("--- Search by Vector ---");
    let query_text = "NoSQL database";
    println!("Query: \"{}\"", query_text);

    let vector_results = store.similarity_search(query_text, 2, None).await?;
    println!("Top 2 results:");
    for (i, doc) in vector_results.iter().enumerate() {
        println!("  {}. {}", i + 1, doc.page_content);
    }
    println!();

    // Get documents by IDs
    println!("--- Get Documents by IDs ---");
    let first_two_ids = &ids[..2.min(ids.len())];
    println!("Fetching documents: {:?}", first_two_ids);

    let retrieved_docs = store.get_by_ids(first_two_ids).await?;
    println!("Retrieved {} documents:", retrieved_docs.len());
    for doc in &retrieved_docs {
        println!("  - {}", doc.page_content);
    }
    println!();

    // Delete documents
    println!("--- Delete Documents ---");
    println!("Deleting first 2 documents...");
    store.delete(first_two_ids).await?;
    println!("✓ Deleted documents\n");

    // Verify deletion
    let remaining = store.similarity_search("database", 10, None).await?;
    println!("Remaining documents: {}", remaining.len());
    for doc in &remaining {
        println!("  - {}", doc.page_content);
    }
    println!();

    // Cleanup
    println!("--- Cleanup ---");
    println!("Dropping vector index...");
    store.drop_index().await?;
    println!("✓ Index dropped");

    println!("\n=== Example Complete ===");
    println!("\nNote: In a real application:");
    println!("  - Use a proper embeddings model (not MockEmbeddings)");
    println!("  - Store credentials securely (not hardcoded)");
    println!("  - Handle errors appropriately for your use case");
    println!("  - Consider connection pooling for high-throughput applications");

    Ok(())
}
