//! Common utilities for integration tests and example applications
//!
//! Factory functions are now provided by `dashflow-factories` crate. This module
//! re-exports them for backward compatibility. For new code, consider using
//! `dashflow-factories` directly.

pub mod app_builder;
pub mod quality_judge;
pub mod test_tools;

// Re-export factories from dashflow-factories crate
// These were previously defined locally in llm_factory, embeddings_factory, tools_factory
pub use dashflow_factories::{
    // Embeddings factory
    create_embeddings,
    create_embeddings_from_config,
    // LLM factory
    create_llm,
    create_llm_from_config,
    create_llm_node,
    // Tools factory
    create_tool,
    create_tool_from_config,
    detect_available_embedding_providers,
    detect_available_providers,
    detect_available_tool_providers,
    EmbeddingProviderInfo,
    EmbeddingRequirements,
    LLMRequirements,
    ProviderInfo,
    ToolProviderInfo,
    ToolRequirements,
};

pub use app_builder::{DashFlowApp, DashFlowAppBuilder, DashFlowAppConfig};
pub use quality_judge::{QualityJudge, QualityScore};
pub use test_tools::{VectorStoreSearchTool, WebSearchTool};

// Deprecated: These modules are kept for backward compatibility but new code
// should use dashflow_factories directly
#[cfg_attr(
    not(test),
    deprecated(since = "1.11.3", note = "Use dashflow_factories::create_embeddings instead")
)]
pub mod embeddings_factory;
#[cfg_attr(
    not(test),
    deprecated(since = "1.11.3", note = "Use dashflow_factories::create_llm instead")
)]
pub mod llm_factory;
#[cfg_attr(
    not(test),
    deprecated(since = "1.11.3", note = "Use dashflow_factories::create_tool instead")
)]
pub mod tools_factory;
