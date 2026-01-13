//! RAG Chain Validation - Rust Implementation
//!
//! End-to-end RAG validation: document indexing -> retrieval -> generation
//!
//! Matches Python baseline: test_rag_chain_parity.py
//!
//! Requirements:
//! - OPENAI_API_KEY environment variable
//! - Docker Chroma: `docker run -p 8000:8000 chromadb/chroma`
//!
//! Corpus: 15 documents covering 7 topics (test_data/rag_corpus.txt)
//! Vector Store: Chroma (Docker)
//! Embeddings: OpenAI text-embedding-3-small
//! LLM: OpenAI gpt-4o-mini
//! Retrieval: Top 3 documents by similarity
//!
//! Test queries:
//! 1. Technical (Rust): "How does Rust ensure memory safety?"
//! 2. Scientific (ML): "What makes transformers different from RNNs?"
//! 3. Practical (Cooking): "Why does bread develop a brown crust?"
//! 4. Out-of-domain: "How do solar panels work?" (should admit lack of knowledge)

use dashflow::core::{
    chains::{format_documents, DEFAULT_DOCUMENT_SEPARATOR},
    config_loader::{ChatModelConfig, EmbeddingConfig, SecretReference},
    documents::Document,
    embeddings::Embeddings,
    language_models::ChatModel,
    messages::Message,
    prompts::PromptTemplate,
    vector_stores::VectorStore,
};
use dashflow_chroma::ChromaVectorStore;
use dashflow_openai::{build_chat_model, build_embeddings};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

/// Parse the RAG corpus file into documents
fn parse_corpus(corpus_path: &Path) -> Result<Vec<Document>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(corpus_path)?;
    let lines: Vec<&str> = content.lines().collect();

    let mut documents = Vec::new();
    let mut current_topic: Option<String> = None;
    let mut current_id: Option<String> = None;
    let mut current_content: Vec<String> = Vec::new();

    for line in lines {
        let line = line.trim();

        if line.starts_with("TOPIC:") {
            // Save previous document if exists
            if let (Some(id), Some(topic)) = (current_id.as_ref(), current_topic.as_ref()) {
                if !current_content.is_empty() {
                    let mut metadata = HashMap::new();
                    metadata.insert("doc_id".to_string(), serde_json::json!(id));
                    metadata.insert("topic".to_string(), serde_json::json!(topic));

                    documents.push(Document {
                        id: None,
                        page_content: current_content.join(" "),
                        metadata,
                    });
                }
            }

            current_topic = Some(line.split(':').nth(1).unwrap_or("").trim().to_string());
            current_id = None;
            current_content.clear();
        } else if line.starts_with("DOC_ID:") {
            current_id = Some(line.split(':').nth(1).unwrap_or("").trim().to_string());
        } else if !line.is_empty() && !line.starts_with('#') {
            // Content line
            current_content.push(line.to_string());
        }
    }

    // Save last document
    if let (Some(id), Some(topic)) = (current_id.as_ref(), current_topic.as_ref()) {
        if !current_content.is_empty() {
            let mut metadata = HashMap::new();
            metadata.insert("doc_id".to_string(), serde_json::json!(id));
            metadata.insert("topic".to_string(), serde_json::json!(topic));

            documents.push(Document {
                id: None,
                page_content: current_content.join(" "),
                metadata,
            });
        }
    }

    Ok(documents)
}

/// Run a RAG query: retrieve documents + generate answer
async fn run_rag_query(
    query: &str,
    store: &ChromaVectorStore,
    llm: &dyn ChatModel,
    prompt_template: &PromptTemplate,
    doc_template: &PromptTemplate,
    query_num: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n{}", "=".repeat(80));
    println!("Query {}: {}", query_num, query);
    println!("{}\n", "=".repeat(80));

    // Step 1: Retrieve documents
    let retrieved_docs = store._similarity_search(query, 3, None).await?;

    println!("Retrieved Documents ({}):", retrieved_docs.len());
    for (i, doc) in retrieved_docs.iter().enumerate() {
        let doc_id = doc
            .metadata
            .get("doc_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let topic = doc
            .metadata
            .get("topic")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        println!("\n  [{}] doc_id={}, topic={}", i + 1, doc_id, topic);

        let preview = if doc.page_content.len() > 150 {
            format!("{}...", &doc.page_content[..150])
        } else {
            doc.page_content.clone()
        };
        println!("      {}", preview);
    }

    // Step 2: Format documents into context
    let context = format_documents(&retrieved_docs, doc_template, DEFAULT_DOCUMENT_SEPARATOR)?;

    // Step 3: Format prompt with context + question
    let mut prompt_vars = HashMap::new();
    prompt_vars.insert("context".to_string(), context);
    prompt_vars.insert("question".to_string(), query.to_string());

    let formatted_prompt = prompt_template.format(&prompt_vars)?;

    // Step 4: Generate answer with LLM
    println!("\nGenerating answer...");
    let messages = vec![Message::human(formatted_prompt)];
    let result = llm.generate(&messages, None, None, None, None).await?;

    let answer = result.generations[0].message.as_text();

    println!("\nFinal Answer:");
    println!("  {}", answer);
    println!();

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "=".repeat(80));
    println!("RAG Chain Validation - Rust Implementation");
    println!("{}", "=".repeat(80));

    // Check environment
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("ERROR: OPENAI_API_KEY environment variable not set");
        std::process::exit(1);
    }

    // Step 1: Parse corpus
    let corpus_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .ok_or_else(|| {
            std::io::Error::other(
                "CARGO_MANIFEST_DIR is missing expected parent directories",
            )
        })?;
    let corpus_path = corpus_root.join("test_data/rag_corpus.txt");

    println!("\n1. Parsing corpus: {}", corpus_path.display());
    let documents = parse_corpus(&corpus_path)?;
    println!("   Loaded {} documents", documents.len());

    // Step 2: Create embeddings
    println!("\n2. Creating embeddings (OpenAI text-embedding-3-small)");
    let embedding_config = EmbeddingConfig::OpenAI {
        model: "text-embedding-3-small".to_string(),
        api_key: SecretReference::from_env("OPENAI_API_KEY"),
        batch_size: 32,
    };
    let embeddings: Arc<dyn Embeddings> = build_embeddings(&embedding_config)?;

    // Step 3: Index in Chroma
    println!("\n3. Indexing in Chroma (Docker)");
    let mut store = ChromaVectorStore::new(
        "rag_validation_rust",
        Arc::<dyn Embeddings>::clone(&embeddings),
        Some("http://localhost:8000"),
    )
    .await?;

    // Delete collection if exists (clean slate)
    let _ = store.delete(None).await;

    // Add documents
    let texts: Vec<&str> = documents.iter().map(|d| d.page_content.as_str()).collect();
    let metadatas: Vec<HashMap<String, serde_json::Value>> =
        documents.iter().map(|d| d.metadata.clone()).collect();

    store.add_texts(&texts, Some(&metadatas), None).await?;
    println!("   Indexed {} documents", documents.len());

    // Step 4: Create LLM
    println!("\n4. Creating LLM (OpenAI gpt-4o-mini, temperature=0)");
    let llm_config = ChatModelConfig::OpenAI {
        model: "gpt-4o-mini".to_string(),
        api_key: SecretReference::from_env("OPENAI_API_KEY"),
        temperature: Some(0.0),
        max_tokens: None,
        base_url: None,
        organization: None,
    };
    let llm = build_chat_model(&llm_config)?;
    println!("   LLM ready");

    // Step 5: Create prompt templates
    println!("\n5. Creating prompt templates");

    let prompt_template = PromptTemplate::from_template(
        r#"Answer the question based only on the following context:

{context}

Question: {question}

Answer: "#,
    )?;

    // Simple document template (just content)
    let doc_template = PromptTemplate::from_template("Document {document_index}:\n{page_content}")?;

    println!("   Prompt templates ready");

    // Step 6: Run test queries
    println!("\n6. Running test queries");

    let test_queries = [
        "How does Rust ensure memory safety?",
        "What makes transformers different from RNNs?",
        "Why does bread develop a brown crust?",
        "How do solar panels work?",
    ];

    for (i, query) in test_queries.iter().enumerate() {
        run_rag_query(
            query,
            &store,
            llm.as_ref(),
            &prompt_template,
            &doc_template,
            i + 1,
        )
        .await?;
    }

    println!("{}", "=".repeat(80));
    println!("Rust Validation Complete");
    println!("{}", "=".repeat(80));

    Ok(())
}
