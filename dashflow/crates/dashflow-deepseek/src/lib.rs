//! `DeepSeek` integration for `DashFlow` Rust
//!
//! This crate provides integration with `DeepSeek`'s language models.
//! `DeepSeek` uses an OpenAI-compatible API, so this crate wraps the
//! `OpenAI` implementation with DeepSeek-specific defaults.
//!
//! # Example
//!
//! ```no_run
//! use dashflow_deepseek::ChatDeepSeek;
//! use dashflow::core::messages::Message;
//! use dashflow::core::language_models::ChatModel;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let model = ChatDeepSeek::with_api_key("your-api-key")
//!     .with_model("deepseek-chat");
//!
//! let messages = vec![Message::human("Hello, DeepSeek!")];
//! let response = model.generate(&messages, None, None, None, None).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # See Also
//!
//! - [`dashflow::core::language_models::ChatModel`] - The trait implemented by chat models
//! - [`dashflow_openai`](https://docs.rs/dashflow-openai) - OpenAI integration
//! - [`dashflow_groq`](https://docs.rs/dashflow-groq) - Groq fast inference (DeepSeek models available)
//! - [DeepSeek Platform](https://platform.deepseek.com/) - API key management

/// DeepSeek API base URL (default value when `DEEPSEEK_API_BASE` env var is not set)
pub const DEEPSEEK_DEFAULT_API_BASE: &str =
    dashflow::core::config_loader::env_vars::DEFAULT_DEEPSEEK_API_BASE;

pub mod chat_models;

pub use chat_models::ChatDeepSeek;

mod config_ext;
pub use config_ext::{build_chat_model, build_llm_node};
