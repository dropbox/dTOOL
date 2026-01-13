//! Base memory trait and types for conversation state management
//!
//! This module provides the core abstractions for memory in `DashFlow` Rust.
//! Memory maintains state across chain executions, storing information from
//! past interactions to inform future responses.

use async_trait::async_trait;
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during memory operations
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum MemoryError {
    #[error("Memory operation failed: {0}")]
    OperationFailed(String),

    #[error("Invalid memory configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("LLM error: {0}")]
    LLMError(String),

    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}

pub type MemoryResult<T> = Result<T, MemoryError>;

/// Abstract base trait for memory in chains.
///
/// Memory maintains state across chain executions, storing information from
/// past interactions and injecting that information into future chain inputs.
///
/// # Design Philosophy
///
/// - **Session-Based**: Each memory instance represents one conversation session
/// - **Async-First**: All methods are async to support various storage backends
/// - **Key-Value Interface**: Memory loads/saves using dictionary-like structures
/// - **Flexible Storage**: Can store any serializable data structure
///
/// # Implementation Requirements
///
/// Implementors must provide:
/// - `memory_variables()`: List of keys this memory will provide
/// - `load_memory_variables()`: Load memory state as key-value pairs
/// - `save_context()`: Save new conversation turn to memory
/// - `clear()`: Clear all memory contents
///
/// # Python Baseline Compatibility
///
/// Matches `BaseMemory` from `dashflow.memory.base:27-117`.
///
/// Key differences:
/// - Rust uses `HashMap<String, String>` instead of Python's `dict[str, Any]`
/// - Rust is async-first (Python has sync+async variants)
/// - Rust uses Result types (Python uses exceptions)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_memory::{BaseMemory, MemoryResult};
/// use async_trait::async_trait;
/// use std::collections::HashMap;
///
/// struct SimpleMemory {
///     data: HashMap<String, String>,
/// }
///
/// #[async_trait]
/// impl BaseMemory for SimpleMemory {
///     fn memory_variables(&self) -> Vec<String> {
///         self.data.keys().cloned().collect()
///     }
///
///     async fn load_memory_variables(
///         &self,
///         _inputs: &HashMap<String, String>,
///     ) -> MemoryResult<HashMap<String, String>> {
///         Ok(self.data.clone())
///     }
///
///     async fn save_context(
///         &mut self,
///         inputs: &HashMap<String, String>,
///         outputs: &HashMap<String, String>,
///     ) -> MemoryResult<()> {
///         // Store conversation turn
///         if let (Some(input), Some(output)) = (inputs.get("input"), outputs.get("output")) {
///             self.data.insert("last_input".to_string(), input.clone());
///             self.data.insert("last_output".to_string(), output.clone());
///         }
///         Ok(())
///     }
///
///     async fn clear(&mut self) -> MemoryResult<()> {
///         self.data.clear();
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait BaseMemory: Send + Sync {
    /// The keys this memory class will add to chain inputs.
    ///
    /// These are the variable names that will be populated when
    /// `load_memory_variables()` is called.
    ///
    /// # Returns
    ///
    /// List of variable names (e.g., `["history"]`)
    fn memory_variables(&self) -> Vec<String>;

    /// Load memory variables for chain input.
    ///
    /// Given the current chain inputs, return the memory variables that should
    /// be added to the chain input. This typically returns historical context
    /// like conversation history, summaries, or entity information.
    ///
    /// # Arguments
    ///
    /// * `inputs` - Current chain inputs (may be used to filter or contextualize memory)
    ///
    /// # Returns
    ///
    /// Key-value pairs to add to chain input (e.g., `{"history": "..."}`)
    async fn load_memory_variables(
        &self,
        inputs: &HashMap<String, String>,
    ) -> MemoryResult<HashMap<String, String>>;

    /// Save context from this chain run to memory.
    ///
    /// Called after each chain execution to store the conversation turn.
    /// Implementations should extract relevant information and update their
    /// internal state.
    ///
    /// # Arguments
    ///
    /// * `inputs` - The inputs to the chain for this turn
    /// * `outputs` - The outputs from the chain for this turn
    async fn save_context(
        &mut self,
        inputs: &HashMap<String, String>,
        outputs: &HashMap<String, String>,
    ) -> MemoryResult<()>;

    /// Clear all memory contents.
    ///
    /// Resets the memory to its initial empty state. Useful for starting
    /// a new conversation session.
    async fn clear(&mut self) -> MemoryResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== MemoryError Display tests ==========

    #[test]
    fn test_memory_error_operation_failed_display() {
        let err = MemoryError::OperationFailed("failed to load".to_string());
        assert_eq!(err.to_string(), "Memory operation failed: failed to load");
    }

    #[test]
    fn test_memory_error_invalid_configuration_display() {
        let err = MemoryError::InvalidConfiguration("missing key".to_string());
        assert_eq!(err.to_string(), "Invalid memory configuration: missing key");
    }

    #[test]
    fn test_memory_error_llm_error_display() {
        let err = MemoryError::LLMError("rate limit exceeded".to_string());
        assert_eq!(err.to_string(), "LLM error: rate limit exceeded");
    }

    // ========== MemoryError From implementations tests ==========

    #[test]
    fn test_memory_error_from_serde_json_error() {
        let mem_err: MemoryError = match serde_json::from_str::<String>("invalid json") {
            Ok(_) => MemoryError::Other(anyhow::anyhow!("Expected serde_json error, got Ok")),
            Err(json_err) => json_err.into(),
        };

        assert!(matches!(mem_err, MemoryError::SerializationError(_)));
        assert!(mem_err.to_string().contains("Serialization error"));
    }

    #[test]
    fn test_memory_error_from_anyhow_error() {
        let anyhow_err = anyhow::anyhow!("some error");
        let mem_err: MemoryError = anyhow_err.into();

        assert!(matches!(mem_err, MemoryError::Other(_)));
        assert!(mem_err.to_string().contains("Other error"));
    }

    // ========== MemoryError Debug test ==========

    #[test]
    fn test_memory_error_debug() {
        let err = MemoryError::OperationFailed("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("OperationFailed"));
        assert!(debug_str.contains("test"));
    }

    // ========== MemoryResult tests ==========

    #[test]
    fn test_memory_result_ok() {
        let result: MemoryResult<i32> = Ok(42);
        assert!(
            matches!(&result, Ok(42)),
            "Expected Ok(42) result, got {result:?}"
        );
    }

    #[test]
    fn test_memory_result_err() {
        let result: MemoryResult<i32> = Err(MemoryError::OperationFailed("fail".to_string()));
        assert!(result.is_err());
    }

    // ========== MemoryError variant construction tests ==========

    #[test]
    fn test_memory_error_variants_constructible() {
        // Verify all variants can be constructed
        let _ = MemoryError::OperationFailed("op".to_string());
        let _ = MemoryError::InvalidConfiguration("cfg".to_string());
        let _ = MemoryError::LLMError("llm".to_string());

        // SerializationError and Other require actual errors to wrap
        // (tested in From tests above)
    }

    // ========== BaseMemory trait object safety test ==========

    #[test]
    fn test_base_memory_trait_is_object_safe() {
        // This test verifies that BaseMemory can be used as a trait object
        // If this compiles, the trait is object-safe
        fn _accepts_trait_object(_: &dyn BaseMemory) {}

        // Note: We don't actually call this function, we just verify it compiles
    }
}
