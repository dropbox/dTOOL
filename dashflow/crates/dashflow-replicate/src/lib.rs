//! Replicate integration for `DashFlow` Rust
//!
//! This crate provides Replicate implementations for `DashFlow` Rust.
//!
//! # Features
//! - `ChatReplicate`: Chat model using Replicate's OpenAI-compatible API
//! - Access to thousands of open-source models via Replicate
//! - Streaming support for real-time responses
//! - Function/tool calling support
//! - Support for custom model deployments
//!
//! # Example
//! ```no_run
//! use dashflow_replicate::ChatReplicate;
//! use dashflow::core::language_models::ChatModel;
//! use dashflow::core::messages::Message;
//!
//! #[tokio::main]
//! async fn main() {
//!     let model = ChatReplicate::new()
//!         .with_model("meta/meta-llama-3-70b-instruct")
//!         .with_temperature(0.7);
//!
//!     let messages = vec![Message::human("Hello!")];
//!     let result = model.generate(&messages, None, None, None, None).await.unwrap();
//!     println!("{:?}", result);
//! }
//! ```
//!
//! # Authentication
//! Set the `REPLICATE_API_TOKEN` environment variable with your Replicate API token.
//! You can also provide the token directly using `.with_api_key()`.
//!
//! # See Also
//!
//! - [`dashflow::core::language_models::ChatModel`] - The trait implemented by chat models
//! - [`dashflow_openai`](https://docs.rs/dashflow-openai) - OpenAI integration
//! - [`dashflow_together`](https://docs.rs/dashflow-together) - Together AI alternative
//! - [Replicate](https://replicate.com/) - Model hub and API management

pub mod chat_models;

pub use chat_models::ChatReplicate;
