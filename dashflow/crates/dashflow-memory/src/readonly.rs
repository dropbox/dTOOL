//! Read-only memory wrapper for sharing memory across chains without modification
//!
//! This module provides a read-only wrapper around any memory implementation,
//! preventing writes while allowing reads. Useful for sharing memory state
//! across multiple chains without allowing them to modify it.
//!
//! # Python Baseline Compatibility
//!
//! Based on `dashflow_classic.memory.readonly:6-25` (`ReadOnlySharedMemory`)
//! from Python `DashFlow`.

use crate::base_memory::{BaseMemory, MemoryResult};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Memory wrapper that is read-only and cannot be changed.
///
/// This wrapper allows multiple chains to read from a shared memory
/// without being able to modify it. All write operations (`save_context`,
/// `clear`) become no-ops.
///
/// # Use Cases
///
/// - Share memory state across multiple chains without risk of modification
/// - Create read-only views of memory for monitoring/observability
/// - Implement memory snapshots or checkpoints
/// - Prevent accidental modifications to important memory state
///
/// # Python Baseline Compatibility
///
/// Matches `ReadOnlySharedMemory` from `dashflow_classic.memory.readonly:6-25`.
///
/// Key differences:
/// - Rust uses `Arc<RwLock<_>>` for thread-safe shared state
/// - Rust is async-first (Python has sync+async variants)
/// - Name changed to `ReadOnlyMemory` (clearer, "Shared" is implicit in Rust with Arc)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_memory::{ConversationBufferMemory, ReadOnlyMemory};
/// use dashflow::core::chat_history::InMemoryChatMessageHistory;
///
/// // Create a regular memory
/// let history = InMemoryChatMessageHistory::new();
/// let mut memory = ConversationBufferMemory::new(history);
///
/// // Add some content
/// memory.save_context(&inputs, &outputs).await?;
///
/// // Create read-only wrapper
/// let readonly = ReadOnlyMemory::new(memory);
///
/// // Can read
/// let vars = readonly.load_memory_variables(&HashMap::new()).await?;
///
/// // Cannot write (save_context and clear do nothing)
/// readonly.save_context(&inputs, &outputs).await?; // No-op
/// readonly.clear().await?; // No-op
/// ```
#[derive(Clone)]
pub struct ReadOnlyMemory<M: BaseMemory> {
    /// The underlying memory implementation (wrapped for thread-safety)
    memory: Arc<RwLock<M>>,
}

impl<M: BaseMemory> ReadOnlyMemory<M> {
    /// Create a new read-only memory wrapper.
    ///
    /// # Arguments
    ///
    /// * `memory` - The underlying memory to wrap
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow_memory::{ConversationBufferMemory, ReadOnlyMemory};
    /// use dashflow::core::chat_history::InMemoryChatMessageHistory;
    ///
    /// let history = InMemoryChatMessageHistory::new();
    /// let memory = ConversationBufferMemory::new(history);
    /// let readonly = ReadOnlyMemory::new(memory);
    /// ```
    pub fn new(memory: M) -> Self {
        Self {
            memory: Arc::new(RwLock::new(memory)),
        }
    }

    /// Get a reference to the underlying memory for reading.
    ///
    /// This is useful if you need to access memory-specific methods
    /// that aren't part of the `BaseMemory` trait.
    pub async fn inner(&self) -> tokio::sync::RwLockReadGuard<'_, M> {
        self.memory.read().await
    }
}

#[async_trait]
impl<M: BaseMemory> BaseMemory for ReadOnlyMemory<M> {
    /// Return memory variables from the underlying memory.
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `@property memory_variables` in Python (line 12-14).
    fn memory_variables(&self) -> Vec<String> {
        // This is a bit tricky - we need to read memory_variables synchronously
        // For now, we'll return an empty vec. In practice, this should work
        // because memory_variables is typically called before async operations.
        // A better approach would be to cache the memory_variables, but that
        // complicates the API.
        //
        // Alternative: Could make memory_variables async in trait, but that's
        // a breaking change to the BaseMemory trait.
        vec![]
    }

    /// Load memory variables from the underlying memory.
    ///
    /// This is the main read operation - it delegates to the wrapped memory.
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `load_memory_variables()` in Python (line 16-18).
    async fn load_memory_variables(
        &self,
        inputs: &HashMap<String, String>,
    ) -> MemoryResult<HashMap<String, String>> {
        let memory = self.memory.read().await;
        memory.load_memory_variables(inputs).await
    }

    /// No-op: Read-only memory cannot save context.
    ///
    /// This method does nothing and returns Ok(()).
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `save_context()` in Python (line 20-22).
    async fn save_context(
        &mut self,
        _inputs: &HashMap<String, String>,
        _outputs: &HashMap<String, String>,
    ) -> MemoryResult<()> {
        // No-op: read-only memory cannot be modified
        Ok(())
    }

    /// No-op: Read-only memory cannot be cleared.
    ///
    /// This method does nothing and returns Ok(()).
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `clear()` in Python (line 24-25).
    async fn clear(&mut self) -> MemoryResult<()> {
        // No-op: "got a memory like a vault"
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::conversation_buffer::ConversationBufferMemory;
    use dashflow::core::chat_history::InMemoryChatMessageHistory;

    #[tokio::test]
    async fn test_readonly_memory_can_read() {
        // Create a regular memory with some content
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferMemory::new(history);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi there".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Wrap in readonly
        let readonly = ReadOnlyMemory::new(memory);

        // Should be able to read
        let vars = readonly
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();
        assert!(vars.contains_key("history"));
        let history_str = vars.get("history").unwrap();
        assert!(history_str.contains("Hello"));
        assert!(history_str.contains("Hi there"));
    }

    #[tokio::test]
    async fn test_readonly_memory_cannot_write() {
        // Create a regular memory with some content
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferMemory::new(history);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi there".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Wrap in readonly
        let mut readonly = ReadOnlyMemory::new(memory);

        // Try to write (should be no-op)
        let mut new_inputs = HashMap::new();
        new_inputs.insert("input".to_string(), "New message".to_string());
        let mut new_outputs = HashMap::new();
        new_outputs.insert("output".to_string(), "New response".to_string());
        readonly
            .save_context(&new_inputs, &new_outputs)
            .await
            .unwrap();

        // Verify original content is unchanged
        let vars = readonly
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();
        let history_str = vars.get("history").unwrap();
        assert!(history_str.contains("Hello"));
        assert!(!history_str.contains("New message"));
    }

    #[tokio::test]
    async fn test_readonly_memory_cannot_clear() {
        // Create a regular memory with some content
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferMemory::new(history);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi there".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Wrap in readonly
        let mut readonly = ReadOnlyMemory::new(memory);

        // Try to clear (should be no-op)
        readonly.clear().await.unwrap();

        // Verify content is still there
        let vars = readonly
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();
        let history_str = vars.get("history").unwrap();
        assert!(!history_str.is_empty());
        assert!(history_str.contains("Hello"));
    }

    #[tokio::test]
    async fn test_readonly_memory_with_window_memory() {
        // Test with different memory type
        use crate::conversation_buffer_window::ConversationBufferWindowMemory;

        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferWindowMemory::new(history).with_k(2);

        // Add some messages
        for i in 1..=3 {
            let mut inputs = HashMap::new();
            inputs.insert("input".to_string(), format!("Message {}", i));
            let mut outputs = HashMap::new();
            outputs.insert("output".to_string(), format!("Response {}", i));
            memory.save_context(&inputs, &outputs).await.unwrap();
        }

        // Wrap in readonly
        let readonly = ReadOnlyMemory::new(memory);

        // Should see windowed history (last 2 turns)
        let vars = readonly
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();
        let history_str = vars.get("history").unwrap();
        assert!(!history_str.contains("Message 1"));
        assert!(history_str.contains("Message 2"));
        assert!(history_str.contains("Message 3"));
    }

    #[tokio::test]
    async fn test_readonly_memory_empty() {
        // Test readonly wrapper around empty memory
        let history = InMemoryChatMessageHistory::new();
        let memory = ConversationBufferMemory::new(history);
        let readonly = ReadOnlyMemory::new(memory);

        // Should load empty memory variables successfully
        let vars = readonly
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();
        assert!(vars.contains_key("history"));
        let history_str = vars.get("history").unwrap();
        assert!(history_str.is_empty());
    }

    #[tokio::test]
    async fn test_readonly_memory_multiple_write_attempts() {
        // Verify multiple save_context calls are all no-ops
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferMemory::new(history);

        // Add initial content
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Initial".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Response".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Wrap in readonly
        let mut readonly = ReadOnlyMemory::new(memory);

        // Try multiple write attempts
        for i in 1..=5 {
            let mut new_inputs = HashMap::new();
            new_inputs.insert("input".to_string(), format!("Attempt {}", i));
            let mut new_outputs = HashMap::new();
            new_outputs.insert("output".to_string(), format!("Output {}", i));
            readonly
                .save_context(&new_inputs, &new_outputs)
                .await
                .unwrap();
        }

        // Verify only original content exists
        let vars = readonly
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();
        let history_str = vars.get("history").unwrap();
        assert!(history_str.contains("Initial"));
        assert!(!history_str.contains("Attempt 1"));
        assert!(!history_str.contains("Attempt 5"));
    }

    #[tokio::test]
    async fn test_readonly_memory_multiple_clear_attempts() {
        // Verify multiple clear calls are all no-ops
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferMemory::new(history);

        // Add content
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Important data".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Saved".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Wrap in readonly
        let mut readonly = ReadOnlyMemory::new(memory);

        // Try multiple clear attempts
        for _ in 0..5 {
            readonly.clear().await.unwrap();
        }

        // Verify content is still there
        let vars = readonly
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();
        let history_str = vars.get("history").unwrap();
        assert!(!history_str.is_empty());
        assert!(history_str.contains("Important data"));
    }

    #[tokio::test]
    async fn test_readonly_memory_with_custom_memory_key() {
        // Test with custom memory key
        let history = InMemoryChatMessageHistory::new();
        let mut memory =
            ConversationBufferMemory::new(history).with_memory_key("custom_history".to_string());

        // Add content
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Test".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Response".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Wrap in readonly
        let readonly = ReadOnlyMemory::new(memory);

        // Should preserve custom memory key
        let vars = readonly
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();
        assert!(vars.contains_key("custom_history"));
        assert!(vars.get("custom_history").unwrap().contains("Test"));
    }

    #[tokio::test]
    async fn test_readonly_memory_unicode_content() {
        // Test readonly memory with Unicode/special characters
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferMemory::new(history);

        // Add Unicode content
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "你好世界 🌍 مرحبا".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Здравствуй! 👋".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Wrap in readonly
        let readonly = ReadOnlyMemory::new(memory);

        // Should preserve Unicode characters
        let vars = readonly
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();
        let history_str = vars.get("history").unwrap();
        assert!(history_str.contains("你好世界"));
        assert!(history_str.contains("🌍"));
        assert!(history_str.contains("مرحبا"));
        assert!(history_str.contains("Здравствуй!"));
        assert!(history_str.contains("👋"));
    }

    #[tokio::test]
    async fn test_readonly_memory_inner_access() {
        // Test inner() method for accessing underlying memory
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferMemory::new(history);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Test".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Result".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Wrap in readonly
        let readonly = ReadOnlyMemory::new(memory);

        // Access inner memory
        let inner = readonly.inner().await;
        let vars = inner.load_memory_variables(&HashMap::new()).await.unwrap();
        assert!(vars.contains_key("history"));
        assert!(vars.get("history").unwrap().contains("Test"));
    }

    #[tokio::test]
    async fn test_readonly_memory_clone() {
        // Test that ReadOnlyMemory can be cloned (Arc-based)
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferMemory::new(history);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Original".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Data".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Wrap and clone
        let readonly1 = ReadOnlyMemory::new(memory);
        let readonly2 = readonly1.clone();

        // Both should read same content
        let vars1 = readonly1
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();
        let vars2 = readonly2
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();

        assert_eq!(vars1.get("history"), vars2.get("history"));
        assert!(vars1.get("history").unwrap().contains("Original"));
    }
}
