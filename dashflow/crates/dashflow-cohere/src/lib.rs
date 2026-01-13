//! Cohere integration for `DashFlow` Rust
//!
//! This crate provides integration with Cohere's language models including
//! Command, Command-R, and Command-R+ series. It implements the `ChatModel`
//! trait from dashflow::core, as well as document reranking via the Cohere
//! Rerank API, and embeddings via the Cohere Embed API.
//!
//! # Examples
//!
//! ## Chat Models
//!
//! ```no_run
//! use dashflow_cohere::ChatCohere;
//! use dashflow::core::messages::Message;
//! use dashflow::core::language_models::ChatModel;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let model = ChatCohere::new()
//!     .with_api_key("your-api-key")
//!     .with_model("command-r-plus");
//!
//! let messages = vec![Message::human("Hello, Cohere!")];
//! let response = model.generate(&messages, None, None, None, None).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Embeddings
//!
//! ```no_run
//! use dashflow_cohere::CohereEmbeddings;
//! use dashflow::core::embeddings::Embeddings;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let embedder = CohereEmbeddings::new()
//!     .with_api_key("your-api-key");
//!
//! let query_vector = embedder._embed_query("What is machine learning?").await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Document Reranking
//!
//! ```no_run
//! use dashflow_cohere::CohereRerank;
//! use dashflow::core::documents::{Document, DocumentCompressor};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let reranker = CohereRerank::new()
//!     .with_api_key("your-api-key")
//!     .with_top_n(Some(3));
//!
//! let documents = vec![
//!     Document::new("Document about cats"),
//!     Document::new("Document about dogs"),
//! ];
//!
//! let reranked = reranker
//!     .compress_documents(documents, "tell me about cats", None)
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! # See Also
//!
//! - [`dashflow::core::language_models::ChatModel`] - The trait implemented by chat models
//! - [`dashflow::core::embeddings::Embeddings`] - The trait implemented by embedding models
//! - [`dashflow::core::documents::DocumentCompressor`] - The trait for reranking
//! - [`dashflow_openai`](https://docs.rs/dashflow-openai) - OpenAI integration
//! - [Cohere Dashboard](https://dashboard.cohere.com/) - API key management

pub mod chat_models;
pub mod embeddings;
pub mod rerank;

pub use chat_models::ChatCohere;
pub use embeddings::{CohereEmbeddings, EmbeddingType, InputType, Truncate};
pub use rerank::CohereRerank;
