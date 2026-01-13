//! Conversation buffer memory for storing full conversation history
//!
//! This module provides basic conversation memory that stores the complete
//! conversation history without any processing or truncation.
//!
//! # Python Baseline Compatibility
//!
//! Based on `dashflow_classic.memory.buffer:21-92` (`ConversationBufferMemory`)
//! from Python `DashFlow`.

use crate::base_memory::{BaseMemory, MemoryError, MemoryResult};
use async_trait::async_trait;
use dashflow::core::chat_history::BaseChatMessageHistory;
use dashflow::core::messages::Message;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A basic memory implementation that stores the full conversation history.
///
/// This is the simplest form of memory - it stores all messages without
/// truncation, summarization, or other processing. For long conversations,
/// consider using `ConversationBufferWindowMemory` (windowed) or
/// `ConversationSummaryMemory` (summarized) to avoid context window limits.
///
/// # Behavior
///
/// - Stores all messages indefinitely until `clear()` is called
/// - Returns either formatted string or list of messages based on `return_messages`
/// - Can use any chat message history backend (in-memory, Redis, `MongoDB`, etc.)
///
/// # Python Baseline Compatibility
///
/// Matches `ConversationBufferMemory` from `dashflow_classic.memory.buffer:21-92`.
///
/// Key differences:
/// - Rust uses `Arc<RwLock<_>>` for thread-safe shared state
/// - Rust is async-first (Python has sync+async variants)
/// - Rust uses Result types for error handling
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_memory::ConversationBufferMemory;
/// use dashflow::core::chat_history::InMemoryChatMessageHistory;
///
/// // Create memory with in-memory backend
/// let chat_history = InMemoryChatMessageHistory::new();
/// let memory = ConversationBufferMemory::new(chat_history)
///     .with_memory_key("chat_history")
///     .with_return_messages(false);
///
/// // Save a conversation turn
/// let mut inputs = HashMap::new();
/// inputs.insert("input".to_string(), "Hello!".to_string());
/// let mut outputs = HashMap::new();
/// outputs.insert("output".to_string(), "Hi there!".to_string());
///
/// memory.save_context(&inputs, &outputs).await?;
///
/// // Load memory variables
/// let vars = memory.load_memory_variables(&HashMap::new()).await?;
/// // vars["chat_history"] contains the formatted conversation
/// ```
#[derive(Clone)]
pub struct ConversationBufferMemory<H: BaseChatMessageHistory> {
    /// Backend storage for chat messages
    chat_memory: Arc<RwLock<H>>,

    /// Key name for the memory variable (default: "history")
    memory_key: String,

    /// Whether to return messages as list or formatted string
    return_messages: bool,

    /// Prefix for human messages in formatted string (default: "Human")
    human_prefix: String,

    /// Prefix for AI messages in formatted string (default: "AI")
    ai_prefix: String,

    /// Input key to use from chain inputs (None = auto-detect)
    input_key: Option<String>,

    /// Output key to use from chain outputs (None = auto-detect)
    output_key: Option<String>,
}

impl<H: BaseChatMessageHistory> ConversationBufferMemory<H> {
    /// Create a new conversation buffer memory.
    ///
    /// # Arguments
    ///
    /// * `chat_memory` - Backend for storing chat messages
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow_memory::ConversationBufferMemory;
    /// use dashflow::core::chat_history::InMemoryChatMessageHistory;
    ///
    /// let history = InMemoryChatMessageHistory::new();
    /// let memory = ConversationBufferMemory::new(history);
    /// ```
    pub fn new(chat_memory: H) -> Self {
        Self {
            chat_memory: Arc::new(RwLock::new(chat_memory)),
            memory_key: "history".to_string(),
            return_messages: false,
            human_prefix: "Human".to_string(),
            ai_prefix: "AI".to_string(),
            input_key: None,
            output_key: None,
        }
    }

    /// Set the memory key name.
    ///
    /// This is the dictionary key under which the memory will be stored
    /// when loading memory variables.
    ///
    /// Default: "history"
    pub fn with_memory_key(mut self, key: impl Into<String>) -> Self {
        self.memory_key = key.into();
        self
    }

    /// Set whether to return messages as a list vs formatted string.
    ///
    /// - `true`: Return as `Vec<Message>` (useful for chat models)
    /// - `false`: Return as formatted string like "Human: ...\nAI: ..."
    ///
    /// Default: `false`
    #[must_use]
    pub fn with_return_messages(mut self, return_messages: bool) -> Self {
        self.return_messages = return_messages;
        self
    }

    /// Set the prefix for human messages in formatted output.
    ///
    /// Only used when `return_messages` is `false`.
    ///
    /// Default: "Human"
    pub fn with_human_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.human_prefix = prefix.into();
        self
    }

    /// Set the prefix for AI messages in formatted output.
    ///
    /// Only used when `return_messages` is `false`.
    ///
    /// Default: "AI"
    pub fn with_ai_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.ai_prefix = prefix.into();
        self
    }

    /// Set the input key to extract from chain inputs.
    ///
    /// If `None`, will auto-detect the input key (requires exactly one input).
    ///
    /// Default: `None` (auto-detect)
    #[must_use]
    pub fn with_input_key(mut self, key: Option<String>) -> Self {
        self.input_key = key;
        self
    }

    /// Set the output key to extract from chain outputs.
    ///
    /// If `None`, will auto-detect the output key (requires exactly one output).
    ///
    /// Default: `None` (auto-detect)
    #[must_use]
    pub fn with_output_key(mut self, key: Option<String>) -> Self {
        self.output_key = key;
        self
    }

    /// Get the buffer as a formatted string.
    ///
    /// Formats all messages as "Human: ...\nAI: ..." style text.
    async fn buffer_as_string(&self) -> MemoryResult<String> {
        let history = self.chat_memory.read().await;
        let messages = history
            .get_messages()
            .await
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))?;

        Ok(format_messages_as_string(
            &messages,
            &self.human_prefix,
            &self.ai_prefix,
        ))
    }

    /// Get the buffer as a list of messages.
    async fn buffer_as_messages(&self) -> MemoryResult<Vec<Message>> {
        let history = self.chat_memory.read().await;
        history
            .get_messages()
            .await
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))
    }

    /// Get the prompt input key from inputs.
    ///
    /// Either uses the configured `input_key` or auto-detects it.
    fn get_prompt_input_key<'a>(
        &'a self,
        inputs: &'a HashMap<String, String>,
    ) -> MemoryResult<&'a str> {
        if let Some(ref key) = self.input_key {
            if !inputs.contains_key(key) {
                return Err(MemoryError::InvalidConfiguration(format!(
                    "Configured input_key '{key}' not found in inputs"
                )));
            }
            Ok(key)
        } else {
            // Auto-detect: find the key that's not a memory variable
            let memory_vars: std::collections::HashSet<_> =
                self.memory_variables().into_iter().collect();
            let non_memory_keys: Vec<_> = inputs
                .keys()
                .filter(|k| !memory_vars.contains(k.as_str()))
                .collect();

            if non_memory_keys.is_empty() {
                return Err(MemoryError::InvalidConfiguration(
                    "No input keys found (all keys are memory variables)".to_string(),
                ));
            }
            if non_memory_keys.len() > 1 {
                return Err(MemoryError::InvalidConfiguration(format!(
                    "Multiple input keys found: {non_memory_keys:?}. Please specify input_key explicitly"
                )));
            }

            Ok(non_memory_keys[0])
        }
    }

    /// Get the prompt output key from outputs.
    ///
    /// Either uses the configured `output_key` or auto-detects it.
    fn get_prompt_output_key<'a>(
        &'a self,
        outputs: &'a HashMap<String, String>,
    ) -> MemoryResult<&'a str> {
        if let Some(ref key) = self.output_key {
            if !outputs.contains_key(key) {
                return Err(MemoryError::InvalidConfiguration(format!(
                    "Configured output_key '{key}' not found in outputs"
                )));
            }
            Ok(key)
        } else {
            // Auto-detect: should have exactly one output
            if outputs.is_empty() {
                return Err(MemoryError::InvalidConfiguration(
                    "No output keys found".to_string(),
                ));
            }
            if outputs.len() > 1 {
                return Err(MemoryError::InvalidConfiguration(format!(
                    "Multiple output keys found: {:?}. Please specify output_key explicitly",
                    outputs.keys().collect::<Vec<_>>()
                )));
            }

            // SAFETY: len() == 1 check above guarantees .next() returns Some
            #[allow(clippy::unwrap_used)]
            Ok(outputs.keys().next().unwrap())
        }
    }
}

#[async_trait]
impl<H: BaseChatMessageHistory> BaseMemory for ConversationBufferMemory<H> {
    fn memory_variables(&self) -> Vec<String> {
        vec![self.memory_key.clone()]
    }

    async fn load_memory_variables(
        &self,
        _inputs: &HashMap<String, String>,
    ) -> MemoryResult<HashMap<String, String>> {
        let buffer_value = if self.return_messages {
            // Return serialized messages
            let messages = self.buffer_as_messages().await?;
            serde_json::to_string(&messages).map_err(MemoryError::SerializationError)?
        } else {
            // Return formatted string
            self.buffer_as_string().await?
        };

        let mut vars = HashMap::new();
        vars.insert(self.memory_key.clone(), buffer_value);
        Ok(vars)
    }

    async fn save_context(
        &mut self,
        inputs: &HashMap<String, String>,
        outputs: &HashMap<String, String>,
    ) -> MemoryResult<()> {
        // Extract input and output values
        let input_key = self.get_prompt_input_key(inputs)?;
        let output_key = self.get_prompt_output_key(outputs)?;

        let input_value = inputs
            .get(input_key)
            .ok_or_else(|| MemoryError::InvalidConfiguration("Input key not found".to_string()))?;
        let output_value = outputs
            .get(output_key)
            .ok_or_else(|| MemoryError::InvalidConfiguration("Output key not found".to_string()))?;

        // Create and add messages
        let human_msg = Message::human(input_value.clone());
        let ai_msg = Message::ai(output_value.clone());

        let history = self.chat_memory.write().await;
        history
            .add_messages(&[human_msg, ai_msg])
            .await
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))?;

        Ok(())
    }

    async fn clear(&mut self) -> MemoryResult<()> {
        let history = self.chat_memory.write().await;
        history
            .clear()
            .await
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))
    }
}

/// Format messages as a string with prefixes.
///
/// Converts a list of messages into a string like:
/// ```text
/// Human: Hello
/// AI: Hi there!
/// Human: How are you?
/// AI: I'm doing well, thanks!
/// ```
///
/// # Python Baseline Compatibility
///
/// Matches `get_buffer_string()` from `dashflow_core.messages.utils:39-75`.
fn format_messages_as_string(messages: &[Message], human_prefix: &str, ai_prefix: &str) -> String {
    let mut lines = Vec::new();

    for msg in messages {
        let line = match msg {
            Message::Human { content, .. } => format!("{}: {}", human_prefix, content.as_text()),
            Message::AI { content, .. } => format!("{}: {}", ai_prefix, content.as_text()),
            Message::System { content, .. } => format!("System: {}", content.as_text()),
            Message::Function { content, .. } => format!("Function: {}", content.as_text()),
            Message::Tool { content, .. } => format!("Tool: {}", content.as_text()),
        };

        lines.push(line);
    }

    lines.join("\n")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use dashflow::core::chat_history::InMemoryChatMessageHistory;

    #[tokio::test]
    async fn test_conversation_buffer_memory_basic() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferMemory::new(history);

        // Save first conversation turn
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello!".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi there!".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        // Load and verify
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert_eq!(vars.len(), 1);
        assert!(vars.contains_key("history"));

        let history_str = vars.get("history").unwrap();
        assert!(history_str.contains("Hello!"));
        assert!(history_str.contains("Hi there!"));
    }

    #[tokio::test]
    async fn test_conversation_buffer_memory_formatted_string() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferMemory::new(history)
            .with_return_messages(false)
            .with_human_prefix("User")
            .with_ai_prefix("Assistant")
            .with_input_key(Some("question".to_string()))
            .with_output_key(Some("answer".to_string()));

        let mut inputs = HashMap::new();
        inputs.insert("question".to_string(), "What is Rust?".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("answer".to_string(), "A systems language".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history_str = vars.get("history").unwrap();

        assert!(history_str.contains("User: What is Rust?"));
        assert!(history_str.contains("Assistant: A systems language"));
    }

    #[tokio::test]
    async fn test_conversation_buffer_memory_multiple_turns() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferMemory::new(history);

        // Turn 1
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hi".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hello".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Turn 2
        inputs.clear();
        inputs.insert("input".to_string(), "How are you?".to_string());
        outputs.clear();
        outputs.insert("output".to_string(), "I'm good!".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Verify both turns are stored
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history_str = vars.get("history").unwrap();

        assert!(history_str.contains("Hi"));
        assert!(history_str.contains("Hello"));
        assert!(history_str.contains("How are you?"));
        assert!(history_str.contains("I'm good!"));
    }

    #[tokio::test]
    async fn test_conversation_buffer_memory_clear() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferMemory::new(history);

        // Add some messages
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Test".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Response".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Verify messages exist
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert!(!vars.get("history").unwrap().is_empty());

        // Clear and verify empty
        memory.clear().await.unwrap();
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert!(vars.get("history").unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_conversation_buffer_memory_custom_key() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferMemory::new(history).with_memory_key("chat_history");

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Test".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Response".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert!(vars.contains_key("chat_history"));
        assert!(!vars.contains_key("history"));
    }

    #[tokio::test]
    async fn test_format_messages_as_string() {
        let messages = vec![
            Message::human("Hello"),
            Message::ai("Hi there"),
            Message::human("How are you?"),
            Message::ai("I'm good!"),
        ];

        let formatted = format_messages_as_string(&messages, "Human", "AI");

        assert_eq!(
            formatted,
            "Human: Hello\nAI: Hi there\nHuman: How are you?\nAI: I'm good!"
        );
    }

    // ========================================================================
    // Edge Case Tests
    // ========================================================================

    #[tokio::test]
    async fn test_conversation_buffer_memory_empty_inputs() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferMemory::new(history);

        // Empty inputs and outputs should error (no input key found)
        let inputs = HashMap::new();
        let outputs = HashMap::new();

        let result = memory.save_context(&inputs, &outputs).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_conversation_buffer_memory_missing_input_key() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory =
            ConversationBufferMemory::new(history).with_input_key(Some("question".to_string()));

        // Inputs missing the expected key
        let mut inputs = HashMap::new();
        inputs.insert("wrong_key".to_string(), "Hello".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi".to_string());

        let result = memory.save_context(&inputs, &outputs).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_conversation_buffer_memory_missing_output_key() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory =
            ConversationBufferMemory::new(history).with_output_key(Some("answer".to_string()));

        // Outputs missing the expected key
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("wrong_key".to_string(), "Hi".to_string());

        let result = memory.save_context(&inputs, &outputs).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_conversation_buffer_memory_multiple_input_keys_no_spec() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferMemory::new(history);

        // Multiple input keys without specifying which to use - should error
        let mut inputs = HashMap::new();
        inputs.insert("input1".to_string(), "Hello 1".to_string());
        inputs.insert("input2".to_string(), "Hello 2".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi".to_string());

        let result = memory.save_context(&inputs, &outputs).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_conversation_buffer_memory_multiple_output_keys_no_spec() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferMemory::new(history);

        // Multiple output keys without specifying which to use - should error
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output1".to_string(), "Hi 1".to_string());
        outputs.insert("output2".to_string(), "Hi 2".to_string());

        let result = memory.save_context(&inputs, &outputs).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_conversation_buffer_memory_return_messages_mode() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferMemory::new(history).with_return_messages(true);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi there".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        // In return_messages mode, should return JSON-serialized messages
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history_value = vars.get("history").unwrap();

        // Should contain serialized message structure (JSON array)
        assert!(history_value.starts_with('['));
        assert!(history_value.ends_with(']'));
        assert!(history_value.contains("Hello"));
        assert!(history_value.contains("Hi there"));
    }

    #[tokio::test]
    async fn test_conversation_buffer_memory_empty_history_load() {
        let history = InMemoryChatMessageHistory::new();
        let memory = ConversationBufferMemory::new(history);

        // Load from empty memory should return empty string
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars.get("history").unwrap(), "");
    }

    #[tokio::test]
    async fn test_conversation_buffer_memory_special_characters() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferMemory::new(history);

        // Test with special characters and unicode
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello! 你好 🚀".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi! مرحبا 🌟".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history_str = vars.get("history").unwrap();

        assert!(history_str.contains("Hello! 你好 🚀"));
        assert!(history_str.contains("Hi! مرحبا 🌟"));
    }

    #[tokio::test]
    async fn test_conversation_buffer_memory_very_long_message() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferMemory::new(history);

        // Test with very long message (10KB)
        let long_input = "x".repeat(10_000);
        let long_output = "y".repeat(10_000);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), long_input.clone());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), long_output.clone());

        memory.save_context(&inputs, &outputs).await.unwrap();

        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history_str = vars.get("history").unwrap();

        assert!(history_str.contains(&long_input));
        assert!(history_str.contains(&long_output));
    }

    #[tokio::test]
    async fn test_conversation_buffer_memory_variables_returns_keys() {
        let history = InMemoryChatMessageHistory::new();
        let memory = ConversationBufferMemory::new(history).with_memory_key("chat");

        let vars = memory.memory_variables();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0], "chat");
    }
}
