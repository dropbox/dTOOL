//! Provider Helper Macros
//!
//! This module provides macros and utilities to reduce boilerplate code
//! in LLM provider crates (dashflow-openai, dashflow-anthropic, etc.).
//!
//! # Provider Registration Macro
//!
//! The original scope of Originally proposed a complex proc macro to generate
//! entire provider registrations. After analysis, this was deemed over-engineered
//! because:
//! 1. The config_ext.rs files are already ~145 lines average (not 200+ as estimated)
//! 2. Provider-specific builder patterns vary too much for unified macro generation
//! 3. The ChatModel trait implementation is provider-specific (API calls, streaming)
//!
//! Instead, this module provides simple declarative macros that extract the
//! truly common code:
//! 1. `impl_build_llm_node!` - Generates the identical `build_llm_node` function
//! 2. `wrong_provider_error!` - Generates consistent error messages
//!
//! ## Usage
//!
//! In your provider crate's config_ext.rs:
//!
//! ```rust,ignore
//! use dashflow::core::config_loader::provider_helpers::{impl_build_llm_node, wrong_provider_error};
//!
//! // Your custom build_chat_model implementation
//! pub fn build_chat_model(config: &ChatModelConfig) -> Result<Arc<dyn ChatModel>, DashFlowError> {
//!     match config {
//!         ChatModelConfig::MyProvider { ... } => { /* provider-specific */ },
//!         other => Err(wrong_provider_error!("dashflow-myprovider", "MyProvider", other)),
//!     }
//! }
//!
//! // Generate the boilerplate build_llm_node function
//! impl_build_llm_node!();
//! ```
//!
//! ## Estimated Savings
//!
//! - ~15-20 lines per provider crate (build_llm_node function + docs + tests)
//! - Consistent error messages across all providers
//! - Single source of truth for the LLMNode wrapping pattern

/// Generate the `build_llm_node` function that wraps `build_chat_model`
///
/// This macro generates a function with the signature:
/// ```rust,ignore
/// pub fn build_llm_node<S: GraphState>(
///     config: &ChatModelConfig,
///     signature: Signature,
/// ) -> Result<LLMNode<S>, DashFlowError>
/// ```
///
/// The generated function simply calls `build_chat_model(config)` and wraps
/// the result in an `LLMNode`. This pattern is identical across all 15+
/// provider crates.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::provider_helpers::impl_build_llm_node;
///
/// // Your build_chat_model must be defined in the same module
/// pub fn build_chat_model(config: &ChatModelConfig) -> Result<Arc<dyn ChatModel>, DashFlowError> {
///     // ... provider-specific implementation
/// }
///
/// // This generates the build_llm_node function
/// impl_build_llm_node!();
/// ```
#[macro_export]
macro_rules! impl_build_llm_node {
    () => {
        /// Build an optimizable LLMNode from a ChatModelConfig and Signature
        ///
        /// This creates an LLMNode that can be used with DashOptimize algorithms
        /// (BootstrapFewShot, MIPROv2, GRPO, etc.) for automatic prompt optimization.
        ///
        /// # Type Parameters
        ///
        /// * `S` - The graph state type (must implement `GraphState`)
        ///
        /// # Errors
        ///
        /// Returns an error if:
        /// - The config is not supported by this provider
        /// - Secret resolution fails (e.g., environment variable not set)
        pub fn build_llm_node<S: $crate::state::GraphState>(
            config: &$crate::core::config_loader::ChatModelConfig,
            signature: $crate::optimize::Signature,
        ) -> Result<$crate::optimize::LLMNode<S>, $crate::core::Error> {
            let llm = build_chat_model(config)?;
            Ok($crate::optimize::LLMNode::new(signature, llm))
        }
    };

    // Variant with custom doc comment
    (doc = $doc:expr) => {
        #[doc = $doc]
        pub fn build_llm_node<S: $crate::state::GraphState>(
            config: &$crate::core::config_loader::ChatModelConfig,
            signature: $crate::optimize::Signature,
        ) -> Result<$crate::optimize::LLMNode<S>, $crate::core::Error> {
            let llm = build_chat_model(config)?;
            Ok($crate::optimize::LLMNode::new(signature, llm))
        }
    };
}

/// Generate a consistent error for wrong provider configuration
///
/// Use this macro when a `build_chat_model` or `build_embeddings` function
/// receives a configuration for a different provider.
///
/// # Arguments
///
/// * `$crate_name` - The name of the crate (e.g., "dashflow-openai")
/// * `$provider_name` - The expected provider (e.g., "OpenAI")
/// * `$config` - The actual configuration received (implements `.provider()`)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::provider_helpers::wrong_provider_error;
///
/// match config {
///     ChatModelConfig::OpenAI { ... } => { /* handle */ },
///     other => Err(wrong_provider_error!("dashflow-openai", "OpenAI", other)),
/// }
/// ```
#[macro_export]
macro_rules! wrong_provider_error {
    ($crate_name:expr, $provider_name:expr, $config:expr) => {
        $crate::core::Error::InvalidInput(format!(
            "build_chat_model from {} only supports {} configs, got {} config. \
             Use the appropriate provider crate for this config type.",
            $crate_name,
            $provider_name,
            $config.provider()
        ))
    };
}

/// Generate a consistent error for wrong embedding provider configuration
///
/// Similar to `wrong_provider_error!` but for embedding configurations.
#[macro_export]
macro_rules! wrong_embedding_provider_error {
    ($crate_name:expr, $provider_name:expr, $config:expr) => {
        $crate::core::Error::InvalidInput(format!(
            "build_embeddings from {} only supports {} configs, got {} config. \
             Use the appropriate provider crate for this config type.",
            $crate_name,
            $provider_name,
            $config.provider()
        ))
    };
}

// Re-export macros at module level for easier imports
pub use crate::impl_build_llm_node;
pub use crate::wrong_embedding_provider_error;
pub use crate::wrong_provider_error;

#[cfg(test)]
#[allow(dead_code)] // Test: Macro-generated functions for compile verification only
mod tests {
    // Tests would require setting up mock providers, which is complex.
    // The macros are tested by the provider crates that use them.

    #[test]
    fn test_macro_expansion_compiles() {
        // This test verifies that the macro syntax is valid.
        // Actual functionality is tested in provider crates.
        use crate::core::config_loader::ChatModelConfig;
        use crate::core::language_models::ChatModel;
        use crate::core::Error as DashFlowError;
        use std::sync::Arc;

        // Mock build_chat_model for testing macro expansion
        fn build_chat_model(
            _config: &ChatModelConfig,
        ) -> Result<Arc<dyn ChatModel>, DashFlowError> {
            Err(DashFlowError::InvalidInput("test".to_string()))
        }

        // This should compile - macro generates valid code
        impl_build_llm_node!();
    }
}
