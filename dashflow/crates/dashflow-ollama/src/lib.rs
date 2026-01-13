// Note: Audited - zero production unwrap/expect calls. All are in doc comments or tests.
// clone_on_ref_ptr: Moved to function-level allows where Arc::clone() pattern needed

//! Ollama integration for DashFlow
//!
//! This crate provides Ollama support for the DashFlow Rust library,
//! enabling **local LLM inference** without external API dependencies.
//!
//! # Features
//!
//! - [`ChatOllama`] - Chat models with streaming support
//! - [`OllamaEmbeddings`] - Local embedding generation
//! - Multi-modal support for vision models (llava, bakllava)
//! - **No API keys required** - fully local inference
//! - Support for all Ollama-compatible models (Llama 3, Mistral, Phi-3, etc.)
//!
//! # Quick Start
//!
//! ```no_run
//! use dashflow_ollama::ChatOllama;
//! use dashflow::core::language_models::ChatModel;
//! use dashflow::core::messages::Message;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Initialize with local Ollama server
//!     let model = ChatOllama::with_base_url("http://localhost:11434")
//!         .with_model("llama3")
//!         .with_temperature(0.7);
//!
//!     let messages = vec![Message::human("Hello!")];
//!     let result = model.generate(&messages, None, None, None, None).await.unwrap();
//!     println!("{}", result.generations[0].text());
//! }
//! ```
//!
//! # With Embeddings
//!
//! ```rust,ignore
//! use dashflow_ollama::OllamaEmbeddings;
//! use dashflow::core::embeddings::Embeddings;
//!
//! let embedder = OllamaEmbeddings::new()
//!     .with_model("nomic-embed-text");
//!
//! let vectors = embedder.embed_documents(&["Hello world".to_string()]).await?;
//! ```
//!
//! # Configuration via YAML
//!
//! ```rust,ignore
//! use dashflow::core::language_models::ChatModelConfig;
//!
//! let config: ChatModelConfig = serde_yml::from_str(r#"
//!     provider: ollama
//!     model: llama3
//!     base_url: http://localhost:11434
//!     temperature: 0.7
//! "#)?;
//!
//! let model = config.build()?;
//! ```
//!
//! # See Also
//!
//! - [`dashflow::core::language_models::ChatModel`] - The trait implemented by chat models
//! - [`dashflow_openai`](https://docs.rs/dashflow-openai) - OpenAI cloud models
//! - [`dashflow_anthropic`](https://docs.rs/dashflow-anthropic) - Anthropic/Claude models
//! - [Ollama Model Library](https://ollama.ai/library) - Available models

pub mod chat_models;
pub mod embeddings;

pub use chat_models::ChatOllama;
pub use embeddings::OllamaEmbeddings;

mod config_ext;
pub use config_ext::{build_chat_model, build_embeddings, build_llm_node};
