//! Simple memory implementation for storing static context
//!
//! This module provides `SimpleMemory`, a basic memory type that stores
//! key-value pairs that never change between chain executions. This is useful
//! for storing constants, configuration, or other static context that should
//! be available to chains but never modified.
//!
//! # Python Baseline
//!
//! Matches `SimpleMemory` from `dashflow.memory.simple:8-30`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::base_memory::{BaseMemory, MemoryResult};

/// Simple memory for storing static context.
///
/// This memory type stores key-value pairs that are never modified. The values
/// are always returned exactly as they were initialized, and `save_context()`
/// does nothing. This is useful for:
///
/// - Global constants available to all chains
/// - Configuration values
/// - Static context that shouldn't change
/// - Read-only reference data
///
/// # Python Baseline
///
/// Matches `SimpleMemory` from `dashflow.memory.simple:8-30`.
///
/// Key differences:
/// - Rust uses `String` values (Python uses `Any`)
/// - Rust requires explicit Send+Sync for async
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_memory::{SimpleMemory, BaseMemory};
/// use std::collections::HashMap;
///
/// let mut memories = HashMap::new();
/// memories.insert("company".to_string(), "Acme Corp".to_string());
/// memories.insert("year".to_string(), "2024".to_string());
///
/// let memory = SimpleMemory::new(memories);
///
/// // These values will always be returned, never modified
/// let vars = memory.load_memory_variables(&HashMap::new()).await?;
/// assert_eq!(vars.get("company"), Some(&"Acme Corp".to_string()));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleMemory {
    /// The static memories that are always returned
    pub memories: HashMap<String, String>,
}

impl SimpleMemory {
    /// Create a new `SimpleMemory` with the given key-value pairs.
    ///
    /// # Arguments
    ///
    /// * `memories` - Static key-value pairs to store
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_memory::SimpleMemory;
    /// use std::collections::HashMap;
    ///
    /// let mut memories = HashMap::new();
    /// memories.insert("api_version".to_string(), "v2".to_string());
    ///
    /// let memory = SimpleMemory::new(memories);
    /// ```
    #[must_use]
    pub fn new(memories: HashMap<String, String>) -> Self {
        Self { memories }
    }

    /// Create an empty `SimpleMemory`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_memory::SimpleMemory;
    ///
    /// let memory = SimpleMemory::empty();
    /// assert!(memory.memories.is_empty());
    /// ```
    #[must_use]
    pub fn empty() -> Self {
        Self {
            memories: HashMap::new(),
        }
    }

    /// Add or update a memory value.
    ///
    /// # Arguments
    ///
    /// * `key` - The memory key
    /// * `value` - The value to store
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_memory::SimpleMemory;
    ///
    /// let mut memory = SimpleMemory::empty();
    /// memory.insert("status".to_string(), "active".to_string());
    /// ```
    pub fn insert(&mut self, key: String, value: String) {
        self.memories.insert(key, value);
    }
}

impl Default for SimpleMemory {
    fn default() -> Self {
        Self::empty()
    }
}

#[async_trait]
impl BaseMemory for SimpleMemory {
    fn memory_variables(&self) -> Vec<String> {
        self.memories.keys().cloned().collect()
    }

    async fn load_memory_variables(
        &self,
        _inputs: &HashMap<String, String>,
    ) -> MemoryResult<HashMap<String, String>> {
        // Always return the same static memories
        Ok(self.memories.clone())
    }

    async fn save_context(
        &mut self,
        _inputs: &HashMap<String, String>,
        _outputs: &HashMap<String, String>,
    ) -> MemoryResult<()> {
        // Nothing should be saved or changed, memory is set in stone
        Ok(())
    }

    async fn clear(&mut self) -> MemoryResult<()> {
        // Nothing to clear, got a memory like a vault
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simple_memory_returns_static_values() {
        let mut memories = HashMap::new();
        memories.insert("company".to_string(), "Acme Corp".to_string());
        memories.insert("year".to_string(), "2024".to_string());

        let memory = SimpleMemory::new(memories);

        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert_eq!(vars.get("company"), Some(&"Acme Corp".to_string()));
        assert_eq!(vars.get("year"), Some(&"2024".to_string()));
    }

    #[tokio::test]
    async fn test_memory_variables_returns_keys() {
        let mut memories = HashMap::new();
        memories.insert("key1".to_string(), "value1".to_string());
        memories.insert("key2".to_string(), "value2".to_string());

        let memory = SimpleMemory::new(memories);

        let vars = memory.memory_variables();
        assert_eq!(vars.len(), 2);
        assert!(vars.contains(&"key1".to_string()));
        assert!(vars.contains(&"key2".to_string()));
    }

    #[tokio::test]
    async fn test_save_context_does_nothing() {
        let mut memories = HashMap::new();
        memories.insert("static".to_string(), "value".to_string());

        let mut memory = SimpleMemory::new(memories);

        // Try to save context (should do nothing)
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "test".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "response".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        // Memory should be unchanged
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars.get("static"), Some(&"value".to_string()));
        assert_eq!(vars.get("input"), None);
        assert_eq!(vars.get("output"), None);
    }

    #[tokio::test]
    async fn test_clear_does_nothing() {
        let mut memories = HashMap::new();
        memories.insert("permanent".to_string(), "data".to_string());

        let mut memory = SimpleMemory::new(memories);

        memory.clear().await.unwrap();

        // Memory should still be there (vault-like)
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars.get("permanent"), Some(&"data".to_string()));
    }

    #[tokio::test]
    async fn test_empty_simple_memory() {
        let memory = SimpleMemory::empty();

        assert!(memory.memories.is_empty());
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert!(vars.is_empty());
    }

    #[tokio::test]
    async fn test_insert_memory() {
        let mut memory = SimpleMemory::empty();
        memory.insert("key1".to_string(), "value1".to_string());
        memory.insert("key2".to_string(), "value2".to_string());

        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert_eq!(vars.len(), 2);
        assert_eq!(vars.get("key1"), Some(&"value1".to_string()));
        assert_eq!(vars.get("key2"), Some(&"value2".to_string()));
    }

    #[tokio::test]
    async fn test_multiple_save_attempts() {
        let mut memories = HashMap::new();
        memories.insert("static".to_string(), "original".to_string());

        let mut memory = SimpleMemory::new(memories);

        // Multiple save_context calls (all should be no-ops)
        let mut inputs = HashMap::new();
        inputs.insert("input1".to_string(), "test1".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output1".to_string(), "response1".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();
        memory.save_context(&inputs, &outputs).await.unwrap();
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Memory should still be unchanged after multiple save attempts
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars.get("static"), Some(&"original".to_string()));
    }

    #[tokio::test]
    async fn test_multiple_clear_attempts() {
        let mut memories = HashMap::new();
        memories.insert("permanent".to_string(), "data".to_string());

        let mut memory = SimpleMemory::new(memories);

        // Multiple clear calls (all should be no-ops)
        memory.clear().await.unwrap();
        memory.clear().await.unwrap();
        memory.clear().await.unwrap();

        // Memory should still be there (vault-like)
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars.get("permanent"), Some(&"data".to_string()));
    }

    #[tokio::test]
    async fn test_unicode_and_special_characters() {
        let mut memories = HashMap::new();
        memories.insert("chinese".to_string(), "你好世界".to_string());
        memories.insert("russian".to_string(), "Здравствуй мир".to_string());
        memories.insert("arabic".to_string(), "مرحبا بالعالم".to_string());
        memories.insert("emoji".to_string(), "🌍👋🎉".to_string());
        memories.insert("special".to_string(), r#"!@#$%^&*(){}[]|\"'<>"#.to_string());

        let memory = SimpleMemory::new(memories);

        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert_eq!(vars.len(), 5);
        assert_eq!(vars.get("chinese"), Some(&"你好世界".to_string()));
        assert_eq!(vars.get("russian"), Some(&"Здравствуй мир".to_string()));
        assert_eq!(vars.get("arabic"), Some(&"مرحبا بالعالم".to_string()));
        assert_eq!(vars.get("emoji"), Some(&"🌍👋🎉".to_string()));
        assert_eq!(
            vars.get("special"),
            Some(&r#"!@#$%^&*(){}[]|\"'<>"#.to_string())
        );
    }

    #[tokio::test]
    async fn test_large_number_of_memories() {
        let mut memories = HashMap::new();
        for i in 0..100 {
            memories.insert(format!("key_{}", i), format!("value_{}", i));
        }

        let memory = SimpleMemory::new(memories);

        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert_eq!(vars.len(), 100);
        assert_eq!(vars.get("key_0"), Some(&"value_0".to_string()));
        assert_eq!(vars.get("key_50"), Some(&"value_50".to_string()));
        assert_eq!(vars.get("key_99"), Some(&"value_99".to_string()));

        let keys = memory.memory_variables();
        assert_eq!(keys.len(), 100);
    }

    #[tokio::test]
    async fn test_default_trait() {
        let memory = SimpleMemory::default();

        assert!(memory.memories.is_empty());
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert!(vars.is_empty());

        let keys = memory.memory_variables();
        assert!(keys.is_empty());
    }

    #[tokio::test]
    async fn test_clone_functionality() {
        let mut memories = HashMap::new();
        memories.insert("shared".to_string(), "data".to_string());

        let memory1 = SimpleMemory::new(memories);
        let mut memory2 = memory1.clone();

        // memory2 is an independent copy
        memory2.insert("new_key".to_string(), "new_value".to_string());

        let vars1 = memory1
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();
        let vars2 = memory2
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();

        // memory1 should only have original data
        assert_eq!(vars1.len(), 1);
        assert_eq!(vars1.get("shared"), Some(&"data".to_string()));
        assert_eq!(vars1.get("new_key"), None);

        // memory2 should have both original and new data
        assert_eq!(vars2.len(), 2);
        assert_eq!(vars2.get("shared"), Some(&"data".to_string()));
        assert_eq!(vars2.get("new_key"), Some(&"new_value".to_string()));
    }
}
