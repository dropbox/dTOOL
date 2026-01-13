//! Perplexity AI provider for `DashFlow` Rust
//!
//! This crate provides integration with Perplexity AI's chat models using their
//! OpenAI-compatible API.
//!
//! # Example
//!
//! ```no_run
//! use dashflow_perplexity::ChatPerplexity;
//! use dashflow::core::language_models::ChatModel;
//! use dashflow::core::messages::Message;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let model = ChatPerplexity::default();
//!     let messages = vec![Message::human("Hello, how are you?")];
//!     let result = model.generate(&messages, None, None, None, None).await?;
//!     println!("Response: {}", result.generations[0].message.as_text());
//!     Ok(())
//! }
//! ```
//!
//! # See Also
//!
//! - [`dashflow::core::language_models::ChatModel`] - The trait implemented by chat models
//! - [`dashflow_openai`](https://docs.rs/dashflow-openai) - OpenAI integration
//! - [Perplexity API](https://docs.perplexity.ai/) - API documentation

/// Perplexity API base URL (default value when `PPLX_API_BASE` env var is not set)
pub const PPLX_DEFAULT_API_BASE: &str =
    dashflow::core::config_loader::env_vars::DEFAULT_PPLX_API_BASE;

mod chat_models;

pub use chat_models::{models, ChatPerplexity};

mod config_ext;
pub use config_ext::{build_chat_model, build_llm_node};
