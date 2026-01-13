//! `HuggingFace` Hub integration for `DashFlow` Rust
//!
//! This crate provides integration with `HuggingFace` Hub's Inference API,
//! allowing you to use thousands of models hosted on `HuggingFace`.
//!
//! # Chat Models
//! ```no_run
//! use dashflow_huggingface::{build_chat_model, ChatHuggingFace};
//! use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
//! use dashflow::core::language_models::ChatModel;
//! use dashflow::core::messages::Message;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Config-driven instantiation (recommended)
//!     let config = ChatModelConfig::HuggingFace {
//!         model: "meta-llama/Llama-2-7b-chat-hf".to_string(),
//!         api_key: SecretReference::from_env("HF_TOKEN"),
//!         temperature: Some(0.7),
//!     };
//!     let model = build_chat_model(&config)?;
//!
//!     let messages = vec![Message::human("Hello!")];
//!     let result = model.generate(&messages, None, None, None, None).await?;
//!     println!("{:?}", result);
//!     Ok(())
//! }
//! ```
//!
//! # Embeddings
//! ```no_run
//! use dashflow_huggingface::HuggingFaceEmbeddings;
//! use dashflow::core::embeddings::Embeddings;
//!
//! #[tokio::main]
//! async fn main() {
//!     let embedder = HuggingFaceEmbeddings::new()
//!         .with_model("sentence-transformers/all-mpnet-base-v2");
//!
//!     let embedding = embedder._embed_query("Hello, world!").await.unwrap();
//!     println!("Embedding dimension: {}", embedding.len());
//! }
//! ```

pub mod chat_models;
pub mod embeddings;

pub use chat_models::ChatHuggingFace;
pub use embeddings::HuggingFaceEmbeddings;

mod config_ext;
pub use config_ext::{build_chat_model, build_embeddings, build_llm_node};
