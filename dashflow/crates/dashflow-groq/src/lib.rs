//! Groq integration for `DashFlow` Rust
//!
//! This crate provides Groq implementations for `DashFlow` Rust.
//!
//! Groq provides fast LLM inference using OpenAI-compatible API endpoints.
//!
//! # Features
//! - `ChatGroq`: Chat model using Groq's API
//! - OpenAI-compatible API (uses async-openai with custom base URL)
//! - Streaming support for real-time responses
//! - Function/tool calling support
//! - Configurable retry logic and rate limiting
//!
//! # Available Models
//! - llama-3.1-8b-instant (fast, efficient)
//! - llama-3.1-70b-versatile (powerful, balanced)
//! - llama-3.3-70b-versatile (latest, improved)
//! - mixtral-8x7b-32768 (large context window)
//! - gemma-7b-it (Google's Gemma)
//! - deepseek-r1-distill-llama-70b (reasoning-focused)
//!
//! # Example
//! ```no_run
//! use dashflow_groq::ChatGroq;
//! use dashflow::core::language_models::ChatModel;
//! use dashflow::core::messages::Message;
//!
//! #[tokio::main]
//! async fn main() {
//!     let model = ChatGroq::new()
//!         .with_model("llama-3.1-8b-instant")
//!         .with_temperature(0.7);
//!
//!     let messages = vec![Message::human("Hello!")];
//!     let result = model.generate(&messages, None, None, None, None).await.unwrap();
//!     println!("{:?}", result);
//! }
//! ```
//!
//! # See Also
//!
//! - [`dashflow::core::language_models::ChatModel`] - The trait implemented by chat models
//! - [`dashflow_openai`](https://docs.rs/dashflow-openai) - OpenAI integration
//! - [`dashflow_ollama`](https://docs.rs/dashflow-ollama) - Local inference alternative
//! - [Groq Console](https://console.groq.com/) - API key management

/// Groq API base URL (OpenAI-compatible endpoint)
pub const GROQ_API_BASE: &str = "https://api.groq.com/openai/v1";

pub mod chat_models;

pub use chat_models::ChatGroq;

mod config_ext;
pub use config_ext::{build_chat_model, build_llm_node};

// Re-export OpenAIConfig for manual configuration
pub use async_openai::config::OpenAIConfig;
