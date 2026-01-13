//! xAI AI integration for `DashFlow` Rust
//!
//! This crate provides xAI AI implementations for `DashFlow` Rust.
//!
//! xAI AI provides powerful LLM inference using OpenAI-compatible API endpoints.
//!
//! # Features
//! - `ChatxAI`: Chat model using xAI AI's API
//! - OpenAI-compatible API (uses async-openai with custom base URL)
//! - Streaming support for real-time responses
//! - Function/tool calling support
//! - Configurable retry logic and rate limiting
//!
//! # Available Models
//! - grok-beta (general-purpose Grok model)
//! - grok-vision-beta (Grok with vision capabilities)
//!
//! # Example
//! ```no_run
//! use dashflow_xai::ChatXAI;
//! use dashflow::core::language_models::ChatModel;
//! use dashflow::core::messages::Message;
//!
//! #[tokio::main]
//! async fn main() {
//!     let model = ChatXAI::new()
//!         .with_model("grok-beta")
//!         .with_temperature(0.7);
//!
//!     let messages = vec![Message::human("Hello!")];
//!     let result = model.generate(&messages, None, None, None, None).await.unwrap();
//!     println!("{:?}", result);
//! }
//! ```

/// xAI API base URL (OpenAI-compatible endpoint)
pub const XAI_API_BASE: &str = dashflow::core::config_loader::env_vars::DEFAULT_XAI_API_BASE;

/// xAI default API base URL (OpenAI-compatible endpoint)
pub const XAI_DEFAULT_API_BASE: &str = XAI_API_BASE;

pub mod chat_models;

pub use chat_models::ChatXAI;

mod config_ext;
pub use config_ext::{build_chat_model, build_llm_node};

// Re-export OpenAIConfig for manual configuration
pub use async_openai::config::OpenAIConfig;
