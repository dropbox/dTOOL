//! Provider-agnostic Factories for DashFlow
//!
//! This crate provides factory functions for creating LLMs, embeddings, and tools
//! without hardcoding to specific providers. The factories detect available credentials
//! and select the best available provider.
//!
//! # Features
//!
//! Provider availability is controlled by features:
//! - `anthropic` - Anthropic Claude support
//! - `ollama` - Local Ollama support
//! - `bedrock` - AWS Bedrock support
//! - `duckduckgo` - DuckDuckGo search tool
//! - `all-providers` - All LLM providers
//! - `all-tools` - All tool providers
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_factories::{create_llm, LLMRequirements};
//!
//! // Get any available LLM based on environment credentials
//! let llm = create_llm(LLMRequirements::default()).await?;
//!
//! // Prefer local inference
//! let llm = create_llm(LLMRequirements {
//!     prefer_local: true,
//!     ..Default::default()
//! }).await?;
//! ```

mod embeddings;
mod llm;
mod tools;

pub use embeddings::{
    create_embeddings, create_embeddings_from_config, detect_available_embedding_providers,
    EmbeddingProviderInfo, EmbeddingRequirements,
};
pub use llm::{
    create_llm, create_llm_from_config, create_llm_node, detect_available_providers,
    LLMRequirements, ProviderInfo,
};
pub use tools::{
    create_tool, create_tool_from_config, detect_available_tool_providers, ToolProviderInfo,
    ToolRequirements,
};

// Re-export core types for convenience
pub use anyhow;
pub use dashflow::core::embeddings::Embeddings;
pub use dashflow::core::language_models::ChatModel;
pub use dashflow::core::tools::Tool;
