//! Combined memory implementation for merging multiple memory types
//!
//! This module provides `CombinedMemory`, which allows combining multiple memory
//! implementations into a single unified memory. This enables hybrid approaches
//! like using both entity tracking and conversation summarization together.

use crate::{BaseMemory, MemoryError, MemoryResult};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};

/// Memory that combines multiple memory implementations.
///
/// `CombinedMemory` allows using multiple memory types together, such as combining
/// entity memory with conversation summarization, or recent history with vector
/// search. Each sub-memory contributes its own memory variables, and all receive
/// the same `save_context` calls.
///
/// # Validation
///
/// - Memory variables must not overlap across sub-memories
/// - Each memory must provide unique variable names
///
/// # Python Baseline Compatibility
///
/// Matches `CombinedMemory` from `dashflow.memory.combined:10-86`.
///
/// Key implementation details:
/// - Validates no overlapping `memory_variables` during construction
/// - Collects `memory_variables` from all sub-memories
/// - Merges `load_memory_variables` results from all sub-memories
/// - Forwards `save_context` to all sub-memories
/// - Forwards clear to all sub-memories
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_memory::{CombinedMemory, ConversationSummaryMemory, ConversationEntityMemory};
///
/// // Create multiple memory types
/// let summary_memory = ConversationSummaryMemory::new(llm1, chat_history1);
/// let entity_memory = ConversationEntityMemory::new(llm2, chat_history2, entity_store);
///
/// // Combine them
/// let combined = CombinedMemory::new(vec![
///     Box::new(summary_memory),
///     Box::new(entity_memory),
/// ])?;
///
/// // Now save_context updates both memories
/// combined.save_context(&inputs, &outputs).await?;
///
/// // And load_memory_variables returns variables from both
/// let vars = combined.load_memory_variables(&inputs).await?;
/// // vars contains both "history" (from summary) and "entities" (from entity memory)
/// ```
#[derive(Default)]
pub struct CombinedMemory {
    /// List of sub-memories to combine.
    ///
    /// Each memory must provide unique `memory_variables` (no overlap allowed).
    memories: Vec<Box<dyn BaseMemory>>,
}

impl CombinedMemory {
    /// Create a new `CombinedMemory` with the given sub-memories.
    ///
    /// # Arguments
    ///
    /// * `memories` - Vector of memory implementations to combine
    ///
    /// # Returns
    ///
    /// New `CombinedMemory` instance, or error if memory variables overlap
    ///
    /// # Errors
    ///
    /// Returns error if any `memory_variables` are repeated across memories
    pub fn new(memories: Vec<Box<dyn BaseMemory>>) -> MemoryResult<Self> {
        // Validate no overlapping memory variables
        Self::validate_no_overlap(&memories)?;

        Ok(Self { memories })
    }

    /// Validate that no memory variables overlap across sub-memories.
    fn validate_no_overlap(memories: &[Box<dyn BaseMemory>]) -> MemoryResult<()> {
        let mut all_variables = HashSet::new();

        for memory in memories {
            let vars = memory.memory_variables();
            let overlap: Vec<_> = vars
                .iter()
                .filter(|v| all_variables.contains(*v))
                .cloned()
                .collect();

            if !overlap.is_empty() {
                return Err(MemoryError::InvalidConfiguration(format!(
                    "The same variables {overlap:?} are found in multiple memory objects, \
                     which is not allowed by CombinedMemory."
                )));
            }

            all_variables.extend(vars);
        }

        Ok(())
    }

    /// Add a memory to the combined memory.
    ///
    /// # Arguments
    ///
    /// * `memory` - Memory implementation to add
    ///
    /// # Errors
    ///
    /// Returns error if the new memory's variables overlap with existing ones
    pub fn add_memory(&mut self, memory: Box<dyn BaseMemory>) -> MemoryResult<()> {
        // Check for overlap with existing memories
        let new_vars: HashSet<_> = memory.memory_variables().into_iter().collect();
        let existing_vars: HashSet<_> = self
            .memories
            .iter()
            .flat_map(|m| m.memory_variables())
            .collect();

        let overlap: Vec<_> = new_vars.intersection(&existing_vars).cloned().collect();

        if !overlap.is_empty() {
            return Err(MemoryError::InvalidConfiguration(format!(
                "The same variables {overlap:?} are found in multiple memory objects, \
                 which is not allowed by CombinedMemory."
            )));
        }

        self.memories.push(memory);
        Ok(())
    }
}

#[async_trait]
impl BaseMemory for CombinedMemory {
    /// All the memory variables from all sub-memories.
    ///
    /// Collected from all linked memories.
    fn memory_variables(&self) -> Vec<String> {
        self.memories
            .iter()
            .flat_map(|memory| memory.memory_variables())
            .collect()
    }

    /// Load all memory variables from all sub-memories.
    ///
    /// Merges the results from all sub-memories. If any variable name is
    /// repeated, returns an error (this should not happen if validation
    /// worked correctly during construction).
    async fn load_memory_variables(
        &self,
        inputs: &HashMap<String, String>,
    ) -> MemoryResult<HashMap<String, String>> {
        let mut memory_data = HashMap::new();

        // Collect vars from all sub-memories
        for memory in &self.memories {
            let data = memory.load_memory_variables(inputs).await?;
            for (key, value) in data {
                if memory_data.contains_key(&key) {
                    return Err(MemoryError::InvalidConfiguration(format!(
                        "The variable {key} is repeated in the CombinedMemory."
                    )));
                }
                memory_data.insert(key, value);
            }
        }

        Ok(memory_data)
    }

    /// Save context to all sub-memories.
    ///
    /// Forwards the `save_context` call to each sub-memory.
    async fn save_context(
        &mut self,
        inputs: &HashMap<String, String>,
        outputs: &HashMap<String, String>,
    ) -> MemoryResult<()> {
        // Save context for all sub-memories
        for memory in &mut self.memories {
            memory.save_context(inputs, outputs).await?;
        }

        Ok(())
    }

    /// Clear all sub-memories.
    ///
    /// Forwards the clear call to each sub-memory.
    async fn clear(&mut self) -> MemoryResult<()> {
        for memory in &mut self.memories {
            memory.clear().await?;
        }

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // Mock memory for testing
    #[derive(Default)]
    struct MockMemory {
        variable_name: String,
        data: HashMap<String, String>,
    }

    impl MockMemory {
        fn new(variable_name: impl Into<String>) -> Self {
            Self {
                variable_name: variable_name.into(),
                data: HashMap::new(),
            }
        }
    }

    #[async_trait]
    impl BaseMemory for MockMemory {
        fn memory_variables(&self) -> Vec<String> {
            vec![self.variable_name.clone()]
        }

        async fn load_memory_variables(
            &self,
            _inputs: &HashMap<String, String>,
        ) -> MemoryResult<HashMap<String, String>> {
            Ok(self.data.clone())
        }

        async fn save_context(
            &mut self,
            inputs: &HashMap<String, String>,
            outputs: &HashMap<String, String>,
        ) -> MemoryResult<()> {
            // Store concatenation of inputs and outputs
            let mut content = String::new();
            for (k, v) in inputs {
                content.push_str(&format!("{}:{} ", k, v));
            }
            for (k, v) in outputs {
                content.push_str(&format!("{}:{} ", k, v));
            }
            self.data
                .insert(self.variable_name.clone(), content.trim().to_string());
            Ok(())
        }

        async fn clear(&mut self) -> MemoryResult<()> {
            self.data.clear();
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_combined_memory_basic() {
        let memory1 = Box::new(MockMemory::new("memory1"));
        let memory2 = Box::new(MockMemory::new("memory2"));

        let mut combined = CombinedMemory::new(vec![memory1, memory2]).unwrap();

        // Check memory variables
        let vars = combined.memory_variables();
        assert_eq!(vars.len(), 2);
        assert!(vars.contains(&"memory1".to_string()));
        assert!(vars.contains(&"memory2".to_string()));

        // Save context
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "hello".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "hi there".to_string());

        combined.save_context(&inputs, &outputs).await.unwrap();

        // Load memory variables
        let loaded = combined
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(
            loaded.get("memory1").unwrap(),
            "input:hello output:hi there"
        );
        assert_eq!(
            loaded.get("memory2").unwrap(),
            "input:hello output:hi there"
        );
    }

    #[tokio::test]
    async fn test_combined_memory_clear() {
        let memory1 = Box::new(MockMemory::new("memory1"));
        let memory2 = Box::new(MockMemory::new("memory2"));

        let mut combined = CombinedMemory::new(vec![memory1, memory2]).unwrap();

        // Save context
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "hello".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "hi".to_string());

        combined.save_context(&inputs, &outputs).await.unwrap();

        // Clear
        combined.clear().await.unwrap();

        // Load should be empty (MockMemory returns empty HashMap after clear)
        let loaded = combined
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_combined_memory_overlapping_variables() {
        let memory1 = Box::new(MockMemory::new("history"));
        let memory2 = Box::new(MockMemory::new("history")); // Same variable name

        let result = CombinedMemory::new(vec![memory1, memory2]);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("same variables"));
        }
    }

    #[tokio::test]
    async fn test_combined_memory_add_memory() {
        let memory1 = Box::new(MockMemory::new("memory1"));
        let mut combined = CombinedMemory::new(vec![memory1]).unwrap();

        // Add another memory
        let memory2 = Box::new(MockMemory::new("memory2"));
        combined.add_memory(memory2).unwrap();

        // Check memory variables
        let vars = combined.memory_variables();
        assert_eq!(vars.len(), 2);

        // Try to add overlapping memory
        let memory3 = Box::new(MockMemory::new("memory1")); // Overlaps with memory1
        let result = combined.add_memory(memory3);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_combined_memory_empty() {
        let combined = CombinedMemory::new(vec![]).unwrap();

        // Should work but return empty results
        let vars = combined.memory_variables();
        assert!(vars.is_empty());

        let loaded = combined
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();
        assert!(loaded.is_empty());
    }

    #[tokio::test]
    async fn test_combined_memory_three_memories() {
        let memory1 = Box::new(MockMemory::new("summary"));
        let memory2 = Box::new(MockMemory::new("entities"));
        let memory3 = Box::new(MockMemory::new("vector_context"));

        let mut combined = CombinedMemory::new(vec![memory1, memory2, memory3]).unwrap();

        // Check memory variables
        let vars = combined.memory_variables();
        assert_eq!(vars.len(), 3);
        assert!(vars.contains(&"summary".to_string()));
        assert!(vars.contains(&"entities".to_string()));
        assert!(vars.contains(&"vector_context".to_string()));

        // Save context
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "test".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "response".to_string());

        combined.save_context(&inputs, &outputs).await.unwrap();

        // Load memory variables
        let loaded = combined
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();
        assert_eq!(loaded.len(), 3);
        assert!(loaded.contains_key("summary"));
        assert!(loaded.contains_key("entities"));
        assert!(loaded.contains_key("vector_context"));
    }

    #[tokio::test]
    async fn test_combined_memory_multiple_saves() {
        let memory1 = Box::new(MockMemory::new("memory1"));
        let memory2 = Box::new(MockMemory::new("memory2"));

        let mut combined = CombinedMemory::new(vec![memory1, memory2]).unwrap();

        // First save
        let mut inputs1 = HashMap::new();
        inputs1.insert("input".to_string(), "hello".to_string());
        let mut outputs1 = HashMap::new();
        outputs1.insert("output".to_string(), "hi".to_string());
        combined.save_context(&inputs1, &outputs1).await.unwrap();

        // Second save (MockMemory overwrites, but should not error)
        let mut inputs2 = HashMap::new();
        inputs2.insert("input".to_string(), "goodbye".to_string());
        let mut outputs2 = HashMap::new();
        outputs2.insert("output".to_string(), "bye".to_string());
        combined.save_context(&inputs2, &outputs2).await.unwrap();

        // Load should have most recent save
        let loaded = combined
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.get("memory1").unwrap(), "input:goodbye output:bye");
        assert_eq!(loaded.get("memory2").unwrap(), "input:goodbye output:bye");
    }

    #[tokio::test]
    async fn test_combined_memory_single_memory() {
        let memory = Box::new(MockMemory::new("history"));
        let mut combined = CombinedMemory::new(vec![memory]).unwrap();

        // Should work with just one memory
        let vars = combined.memory_variables();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0], "history");

        // Save and load
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "test".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "result".to_string());

        combined.save_context(&inputs, &outputs).await.unwrap();

        let loaded = combined
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.get("history").unwrap(), "input:test output:result");
    }

    #[tokio::test]
    async fn test_combined_memory_add_then_use() {
        let memory1 = Box::new(MockMemory::new("memory1"));
        let mut combined = CombinedMemory::new(vec![memory1]).unwrap();

        // Add second memory
        let memory2 = Box::new(MockMemory::new("memory2"));
        combined.add_memory(memory2).unwrap();

        // Now save context to both
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "added".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "dynamically".to_string());

        combined.save_context(&inputs, &outputs).await.unwrap();

        // Both memories should have the data
        let loaded = combined
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(
            loaded.get("memory1").unwrap(),
            "input:added output:dynamically"
        );
        assert_eq!(
            loaded.get("memory2").unwrap(),
            "input:added output:dynamically"
        );
    }

    #[tokio::test]
    async fn test_combined_memory_empty_save() {
        let memory1 = Box::new(MockMemory::new("memory1"));
        let memory2 = Box::new(MockMemory::new("memory2"));

        let mut combined = CombinedMemory::new(vec![memory1, memory2]).unwrap();

        // Save with empty inputs/outputs
        let inputs = HashMap::new();
        let outputs = HashMap::new();

        combined.save_context(&inputs, &outputs).await.unwrap();

        // Load should still work (but MockMemory will have empty string)
        let loaded = combined
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.get("memory1").unwrap(), "");
        assert_eq!(loaded.get("memory2").unwrap(), "");
    }

    #[tokio::test]
    async fn test_combined_memory_many_memories() {
        // Test with 5 memories to verify scalability
        let mut memories: Vec<Box<dyn BaseMemory>> = Vec::new();
        for i in 0..5 {
            memories.push(Box::new(MockMemory::new(format!("memory{}", i))));
        }

        let mut combined = CombinedMemory::new(memories).unwrap();

        // Check all variables present
        let vars = combined.memory_variables();
        assert_eq!(vars.len(), 5);
        for i in 0..5 {
            assert!(vars.contains(&format!("memory{}", i)));
        }

        // Save context
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "scale".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "test".to_string());

        combined.save_context(&inputs, &outputs).await.unwrap();

        // Load all
        let loaded = combined
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();
        assert_eq!(loaded.len(), 5);
        for i in 0..5 {
            assert_eq!(
                loaded.get(&format!("memory{}", i)).unwrap(),
                "input:scale output:test"
            );
        }

        // Clear all
        combined.clear().await.unwrap();
        let loaded_after_clear = combined
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();
        assert!(loaded_after_clear.is_empty());
    }

    // Mock memory that fails on save_context for error testing
    struct FailingSaveMemory {
        variable_name: String,
    }

    impl FailingSaveMemory {
        fn new(variable_name: impl Into<String>) -> Self {
            Self {
                variable_name: variable_name.into(),
            }
        }
    }

    #[async_trait]
    impl BaseMemory for FailingSaveMemory {
        fn memory_variables(&self) -> Vec<String> {
            vec![self.variable_name.clone()]
        }

        async fn load_memory_variables(
            &self,
            _inputs: &HashMap<String, String>,
        ) -> MemoryResult<HashMap<String, String>> {
            Ok(HashMap::new())
        }

        async fn save_context(
            &mut self,
            _inputs: &HashMap<String, String>,
            _outputs: &HashMap<String, String>,
        ) -> MemoryResult<()> {
            Err(MemoryError::InvalidConfiguration(
                "Simulated save failure".to_string(),
            ))
        }

        async fn clear(&mut self) -> MemoryResult<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_combined_memory_save_error_propagation() {
        let memory1 = Box::new(MockMemory::new("memory1"));
        let memory2 = Box::new(FailingSaveMemory::new("memory2"));

        let mut combined = CombinedMemory::new(vec![memory1, memory2]).unwrap();

        // Save should fail due to memory2
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "test".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "fail".to_string());

        let result = combined.save_context(&inputs, &outputs).await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("save failure"));
        }
    }

    // Mock memory that fails on load_memory_variables for error testing
    struct FailingLoadMemory {
        variable_name: String,
    }

    impl FailingLoadMemory {
        fn new(variable_name: impl Into<String>) -> Self {
            Self {
                variable_name: variable_name.into(),
            }
        }
    }

    #[async_trait]
    impl BaseMemory for FailingLoadMemory {
        fn memory_variables(&self) -> Vec<String> {
            vec![self.variable_name.clone()]
        }

        async fn load_memory_variables(
            &self,
            _inputs: &HashMap<String, String>,
        ) -> MemoryResult<HashMap<String, String>> {
            Err(MemoryError::InvalidConfiguration(
                "Simulated load failure".to_string(),
            ))
        }

        async fn save_context(
            &mut self,
            _inputs: &HashMap<String, String>,
            _outputs: &HashMap<String, String>,
        ) -> MemoryResult<()> {
            Ok(())
        }

        async fn clear(&mut self) -> MemoryResult<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_combined_memory_load_error_propagation() {
        let memory1 = Box::new(MockMemory::new("memory1"));
        let memory2 = Box::new(FailingLoadMemory::new("memory2"));

        let mut combined = CombinedMemory::new(vec![memory1, memory2]).unwrap();

        // Save should work
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "test".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "ok".to_string());

        combined.save_context(&inputs, &outputs).await.unwrap();

        // Load should fail due to memory2
        let result = combined.load_memory_variables(&HashMap::new()).await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("load failure"));
        }
    }
}
