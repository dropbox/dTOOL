// Note: Audited - this crate has zero unwrap/expect calls in production code.
// clone_on_ref_ptr: Moved to function-level allows where Arc::clone() pattern needed

//! Azure OpenAI integration for DashFlow.
//!
//! This crate provides Azure-specific OpenAI API integration with support for
//! Azure-hosted deployments, region-specific endpoints, and Azure authentication.
//!
//! # Features
//!
//! - **Chat Models**: GPT-4, GPT-4 Turbo, GPT-3.5 Turbo
//! - **Embeddings**: text-embedding-3-large, text-embedding-3-small, text-embedding-ada-002
//!
//! # Examples
//!
//! ## Chat Model
//!
//! ```rust,no_run
//! use dashflow_azure_openai::ChatAzureOpenAI;
//! use dashflow::core::config_loader::env_vars::AZURE_OPENAI_API_KEY;
//! use std::env;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let chat = ChatAzureOpenAI::new()
//!     .with_deployment_name("gpt-4")
//!     .with_endpoint("https://my-resource.openai.azure.com")
//!     .with_api_key(env::var(AZURE_OPENAI_API_KEY)?);
//! # Ok(())
//! # }
//! ```
//!
//! ## Embeddings
//!
//! ```rust,no_run
//! use dashflow_azure_openai::AzureOpenAIEmbeddings;
//! use dashflow::core::config_loader::env_vars::AZURE_OPENAI_API_KEY;
//! use dashflow::embed_query;
//! use std::env;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let embedder = Arc::new(AzureOpenAIEmbeddings::new()
//!     .with_deployment_name("text-embedding-3-large")
//!     .with_endpoint("https://my-resource.openai.azure.com")
//!     .with_api_key(env::var(AZURE_OPENAI_API_KEY)?));
//!
//! let vector = embed_query(embedder, "What is machine learning?").await?;
//! # Ok(())
//! # }
//! ```
//!
//! # See Also
//!
//! - [`dashflow::core::language_models::ChatModel`] - The trait implemented by chat models
//! - [`dashflow::core::embeddings::Embeddings`] - The trait for embedding models
//! - [`dashflow_openai`](https://docs.rs/dashflow-openai) - Standard OpenAI API integration
//! - [`dashflow_bedrock`](https://docs.rs/dashflow-bedrock) - AWS Bedrock (alternative managed service)
//! - [Azure OpenAI Service](https://azure.microsoft.com/en-us/products/ai-services/openai-service) - Azure portal

pub mod chat_models;
pub mod embeddings;

pub use chat_models::ChatAzureOpenAI;
pub use embeddings::AzureOpenAIEmbeddings;
