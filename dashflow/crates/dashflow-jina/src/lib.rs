//! # `DashFlow` Jina
//!
//! Jina AI integration for `DashFlow` Rust.
//!
//! This crate provides integration with Jina AI's services, including:
//! - Text embeddings with Jina Embeddings API
//! - Document reranking with Jina Rerank API
//!
//! ## Features
//!
//! ### Text Embeddings
//!
//! Jina AI provides state-of-the-art embedding models with long context support.
//!
//! ```no_run
//! use dashflow_jina::JinaEmbeddings;
//! use dashflow::core::embeddings::Embeddings;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let embedder = JinaEmbeddings::new()
//!     .with_api_key("your-api-key");
//!
//! let query_vector = embedder.embed_query("What is semantic search?").await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Document Reranking
//!
//! Jina Rerank uses specialized reranking models to improve retrieval quality
//! by reordering documents based on their relevance to a query.
//!
//! ```no_run
//! use dashflow_jina::rerank::JinaRerank;
//! use dashflow::core::documents::{Document, DocumentCompressor};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Set JINA_API_KEY environment variable
//! std::env::set_var("JINA_API_KEY", "your-api-key");
//!
//! let reranker = JinaRerank::builder()
//!     .model("jina-reranker-v1-base-en".to_string())
//!     .top_n(Some(3))
//!     .build()?;
//!
//! let docs = vec![
//!     Document::new("Paris is the capital of France."),
//!     Document::new("Berlin is the capital of Germany."),
//!     Document::new("The sky is blue."),
//! ];
//!
//! let reranked = reranker
//!     .compress_documents(docs, "What is the capital of France?", None)
//!     .await?;
//!
//! // Returns documents ordered by relevance with relevance_score in metadata
//! for doc in reranked {
//!     println!("{}: score={:?}",
//!         doc.page_content,
//!         doc.metadata.get("relevance_score")
//!     );
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Environment Variables
//!
//! - `JINA_API_KEY`: Required for all Jina services. Get your API key from <https://jina.ai/>
//!
//! ## Models
//!
//! ### Embedding Models
//! - `jina-embeddings-v3`: Multilingual model supporting 89 languages (default)
//! - `jina-embeddings-v2-base-en`: English-optimized model (8192 tokens)
//! - `jina-embeddings-v2-small-en`: Lightweight English model
//! - `jina-clip-v2`: Multimodal text and image embeddings
//!
//! ### Reranking Models
//! - `jina-reranker-v1-base-en`: Base English reranker (default)
//! - `jina-reranker-v1-turbo-en`: Faster English reranker
//! - `jina-reranker-v1-tiny-en`: Smallest/fastest English reranker

pub mod config_ext;
pub mod embeddings;
pub mod rerank;

pub use config_ext::build_reranker;
pub use embeddings::{EmbeddingType, JinaEmbeddings, TaskType};
