//! Core abstractions for DashFlow
//!
//! This module provides the fundamental types and traits for building LLM-powered
//! applications in Rust. It offers the same core abstractions as Python DashFlow with
//! compile-time type safety and native performance.
//!
//! # Key Features
//!
//! - **Runnable Chains**: Compose LLM operations with type-safe, async pipelines
//! - **Chat Models**: Unified interface for OpenAI, Anthropic, Ollama, and more
//! - **Streaming**: First-class support for streaming responses
//! - **Tools & Agents**: Build AI agents with tool calling and ReAct patterns
//! - **Observability**: Built-in tracing with LangSmith integration
//! - **Performance**: 2-10x faster than Python DashFlow
//!
//! # Core Concepts
//!
//! ## Runnables
//!
//! The [`runnable::Runnable`] trait is the foundation of DashFlow. All components
//! (chat models, prompts, chains, agents) implement this trait, enabling composition.
//!
//! ## Messages
//!
//! The [`messages::Message`] enum provides type-safe message representation.
//!
//! ## Chat Models
//!
//! The [`language_models::ChatModel`] trait provides a unified interface for all LLM providers.
//!
//! ## Callbacks & Tracing
//!
//! The [`callbacks`] module provides event handling for observability.
//!
//! ## Agents
//!
//! The [`agents`] module provides modern agent patterns with middleware.
//!
//! # Module Overview
//!
//! - [`runnable`] - Core trait for composable operations
//! - [`messages`] - Message types for chat models
//! - [`language_models`] - Chat model and LLM traits
//! - [`prompts`] - Prompt templates (FString, Jinja2, Mustache)
//! - [`callbacks`] - Event handling and tracing
//! - [`caches`] - LLM response caching
//! - [`stores`] - Key-value storage for caching and persistence
//! - [`agents`] - Agent framework with middleware
//! - [`tools`] - Tool definitions and execution
//! - [`embeddings`] - Text embedding interfaces
//! - [`vector_stores`] - Vector storage and retrieval
//! - [`retrievers`] - Document retrieval interfaces
//! - [`indexing`] - Document indexing with change detection
//! - [`chat_history`] - Chat message history for stateful conversations
//! - [`output_parsers`] - Structured output parsing
//! - [`chains`] - Pre-built chains (RAG, QA, etc.)
//! - [`documents`] - Document types
//! - [`document_transformers`] - Document transformers for filtering and processing
//! - [`serde_helpers`] - JSON serialization helpers
//! - [`config`] - Runtime configuration
//! - [`error`] - Error types and handling

pub mod agent_patterns;
pub mod agents;
pub mod caches;
pub mod callbacks;
pub mod chains;
pub mod chat_history;
pub mod config;
pub mod config_loader;
pub mod deserialization;
pub mod document_loaders;
pub mod document_transformers;
pub mod documents;
pub mod embeddings;
pub mod error;
pub mod http_client;
pub mod indexing;
pub mod language_models;
pub mod mcp;
pub mod messages;
pub mod observability;
pub mod output_parsers;
pub mod prompt_values;
pub mod prompts;
pub mod rate_limiters;
pub mod retrievers;
pub mod retry;
pub mod runnable;
pub mod schema;
pub mod serde_helpers;
pub mod serialization;
pub mod stores;
pub mod structured_query;
pub mod tools;
pub mod tracers;
pub mod usage;
pub mod utils;
pub mod vector_stores;

pub use error::{Error, Result};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_smoke_error_roundtrip() {
        let err = Error::InvalidInput("bad".to_string());
        assert!(matches!(
            err.category(),
            crate::core::error::ErrorCategory::Validation
        ));

        let result: Result<()> = Err(err);
        assert!(result.is_err());
    }
}
