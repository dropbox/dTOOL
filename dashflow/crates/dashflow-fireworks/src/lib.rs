//! Fireworks AI integration for `DashFlow` Rust
//!
//! This crate provides Fireworks AI implementations for `DashFlow` Rust.
//!
//! Fireworks AI provides fast LLM inference using OpenAI-compatible API endpoints.
//!
//! # Features
//! - `ChatFireworks`: Chat model using Fireworks AI's API
//! - `FireworksEmbeddings`: Embedding models using Fireworks AI's API
//! - OpenAI-compatible API (uses async-openai with custom base URL)
//! - Streaming support for real-time responses
//! - Function/tool calling support
//! - Configurable retry logic and rate limiting
//!
//! # Available Chat Models
//! - accounts/fireworks/models/llama-v3p1-8b-instruct (fast, efficient)
//! - accounts/fireworks/models/llama-v3p3-70b-instruct (powerful Llama 3.3)
//! - accounts/fireworks/models/qwen2p5-72b-instruct (Qwen 2.5 72B)
//! - accounts/fireworks/models/mixtral-8x7b-instruct (Mixtral `MoE`)
//!
//! # Available Embedding Models
//! - nomic-ai/nomic-embed-text-v1.5 (768 dimensions)
//! - WhereIsAI/UAE-Large-V1 (1024 dimensions)
//! - thenlper/gte-large (1024 dimensions)
//!
//! # Chat Example
//! ```no_run
//! use dashflow_fireworks::ChatFireworks;
//! use dashflow::core::language_models::ChatModel;
//! use dashflow::core::messages::Message;
//!
//! #[tokio::main]
//! async fn main() {
//!     let model = ChatFireworks::new()
//!         .with_model("accounts/fireworks/models/llama-v3p1-8b-instruct")
//!         .with_temperature(0.7);
//!
//!     let messages = vec![Message::human("Hello!")];
//!     let result = model.generate(&messages, None, None, None, None).await.unwrap();
//!     println!("{:?}", result);
//! }
//! ```
//!
//! # Embeddings Example
//! ```no_run
//! use dashflow_fireworks::FireworksEmbeddings;
//! use dashflow::core::embeddings::Embeddings;
//!
//! #[tokio::main]
//! async fn main() {
//!     let embedder = FireworksEmbeddings::new()
//!         .with_model("nomic-ai/nomic-embed-text-v1.5");
//!
//!     let embedding = embedder.embed_query("Hello, world!").await.unwrap();
//!     println!("Embedding dimension: {}", embedding.len());
//! }
//! ```
//!
//! # See Also
//!
//! - [`dashflow::core::language_models::ChatModel`] - The trait implemented by chat models
//! - [`dashflow::core::embeddings::Embeddings`] - The trait for embedding models
//! - [`dashflow_openai`](https://docs.rs/dashflow-openai) - OpenAI integration
//! - [`dashflow_groq`](https://docs.rs/dashflow-groq) - Groq fast inference alternative
//! - [Fireworks AI](https://fireworks.ai/) - API key management

/// Fireworks AI API base URL (OpenAI-compatible endpoint)
pub const FIREWORKS_API_BASE: &str = "https://api.fireworks.ai/inference/v1";

pub mod chat_models;
pub mod embeddings;

pub use chat_models::ChatFireworks;
pub use embeddings::FireworksEmbeddings;

mod config_ext;
pub use config_ext::{build_chat_model, build_llm_node};

// Re-export OpenAIConfig for manual configuration
pub use async_openai::config::OpenAIConfig;
