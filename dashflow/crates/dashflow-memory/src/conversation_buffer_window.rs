//! Conversation buffer window memory for storing recent conversation history
//!
//! This module provides windowed conversation memory that only keeps the last K
//! conversation turns, automatically dropping older messages.
//!
//! # Python Baseline Compatibility
//!
//! Based on `dashflow_classic.memory.buffer_window:18-63` (`ConversationBufferWindowMemory`)
//! from Python `DashFlow`.

use crate::base_memory::{BaseMemory, MemoryError, MemoryResult};
use async_trait::async_trait;
use dashflow::core::chat_history::BaseChatMessageHistory;
use dashflow::core::messages::Message;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Memory that keeps track of the last K turns of a conversation.
///
/// This memory implementation maintains a sliding window over the conversation
/// history, keeping only the most recent K turns. When the buffer exceeds K turns,
/// the oldest messages are automatically dropped.
///
/// # What is a "turn"?
///
/// A turn consists of one human message + one AI message (2 messages total).
/// So `k=5` means we keep the last 5 human messages and 5 AI messages (10 messages total).
///
/// # Use Cases
///
/// - Long conversations where full history would exceed context window
/// - Applications where only recent context is relevant
/// - Memory-constrained environments
///
/// # Python Baseline Compatibility
///
/// Matches `ConversationBufferWindowMemory` from `dashflow_classic.memory.buffer_window:18-63`.
///
/// Key differences:
/// - Rust uses `Arc<RwLock<_>>` for thread-safe shared state
/// - Rust is async-first (Python has sync+async variants)
/// - Rust uses Result types for error handling
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_memory::ConversationBufferWindowMemory;
/// use dashflow::core::chat_history::InMemoryChatMessageHistory;
///
/// // Keep only the last 3 conversation turns (6 messages)
/// let history = InMemoryChatMessageHistory::new();
/// let memory = ConversationBufferWindowMemory::new(history)
///     .with_k(3)
///     .with_memory_key("chat_history");
///
/// // As you add more turns, old ones are automatically dropped
/// // after exceeding k=3 turns
/// ```
#[derive(Clone)]
pub struct ConversationBufferWindowMemory<H: BaseChatMessageHistory> {
    /// Backend storage for chat messages
    chat_memory: Arc<RwLock<H>>,

    /// Number of conversation turns to keep (default: 5)
    ///
    /// A turn = 1 human message + 1 AI message, so k=5 means 10 messages total.
    k: usize,

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

impl<H: BaseChatMessageHistory> ConversationBufferWindowMemory<H> {
    /// Create a new conversation buffer window memory.
    ///
    /// # Arguments
    ///
    /// * `chat_memory` - Backend for storing chat messages
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow_memory::ConversationBufferWindowMemory;
    /// use dashflow::core::chat_history::InMemoryChatMessageHistory;
    ///
    /// let history = InMemoryChatMessageHistory::new();
    /// let memory = ConversationBufferWindowMemory::new(history);
    /// ```
    pub fn new(chat_memory: H) -> Self {
        Self {
            chat_memory: Arc::new(RwLock::new(chat_memory)),
            k: 5,
            memory_key: "history".to_string(),
            return_messages: false,
            human_prefix: "Human".to_string(),
            ai_prefix: "AI".to_string(),
            input_key: None,
            output_key: None,
        }
    }

    /// Set the number of conversation turns to keep.
    ///
    /// A turn = 1 human message + 1 AI message.
    /// So `k=5` keeps the last 5 human + 5 AI messages = 10 messages total.
    ///
    /// Default: 5
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches the `k` parameter in Python (line 28).
    #[must_use]
    pub fn with_k(mut self, k: usize) -> Self {
        self.k = k;
        self
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

    /// Get the windowed buffer as a formatted string.
    ///
    /// Returns only the last `k*2` messages (k turns).
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `buffer_as_str` property in Python (line 37-44).
    async fn buffer_as_string(&self) -> MemoryResult<String> {
        let messages = self.buffer_as_messages().await?;

        Ok(format_messages_as_string(
            &messages,
            &self.human_prefix,
            &self.ai_prefix,
        ))
    }

    /// Get the windowed buffer as a list of messages.
    ///
    /// Returns only the last `k*2` messages (k turns).
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `buffer_as_messages` property in Python (line 46-49).
    async fn buffer_as_messages(&self) -> MemoryResult<Vec<Message>> {
        let history = self.chat_memory.read().await;
        let all_messages = history
            .get_messages()
            .await
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))?;

        // Keep only last k*2 messages (k turns)
        let max_messages = if self.k > 0 { self.k * 2 } else { 0 };
        let start_idx = all_messages.len().saturating_sub(max_messages);

        Ok(all_messages[start_idx..].to_vec())
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
impl<H: BaseChatMessageHistory> BaseMemory for ConversationBufferWindowMemory<H> {
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
        // The windowing happens automatically on read via buffer_as_messages()
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
    async fn test_conversation_buffer_window_memory_basic() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferWindowMemory::new(history).with_k(2);

        // Add 3 turns (exceeds k=2)
        for i in 1..=3 {
            let mut inputs = HashMap::new();
            inputs.insert("input".to_string(), format!("Message {}", i));
            let mut outputs = HashMap::new();
            outputs.insert("output".to_string(), format!("Response {}", i));

            memory.save_context(&inputs, &outputs).await.unwrap();
        }

        // Should only keep last 2 turns (4 messages)
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history_str = vars.get("history").unwrap();

        // Should NOT contain first turn
        assert!(!history_str.contains("Message 1"));
        assert!(!history_str.contains("Response 1"));

        // Should contain last 2 turns
        assert!(history_str.contains("Message 2"));
        assert!(history_str.contains("Response 2"));
        assert!(history_str.contains("Message 3"));
        assert!(history_str.contains("Response 3"));
    }

    #[tokio::test]
    async fn test_conversation_buffer_window_memory_k_zero() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferWindowMemory::new(history).with_k(0);

        // Add a turn
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Should have empty history (k=0)
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history_str = vars.get("history").unwrap();
        assert!(history_str.is_empty());
    }

    #[tokio::test]
    async fn test_conversation_buffer_window_memory_under_limit() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferWindowMemory::new(history).with_k(5);

        // Add only 2 turns (under k=5)
        for i in 1..=2 {
            let mut inputs = HashMap::new();
            inputs.insert("input".to_string(), format!("Message {}", i));
            let mut outputs = HashMap::new();
            outputs.insert("output".to_string(), format!("Response {}", i));

            memory.save_context(&inputs, &outputs).await.unwrap();
        }

        // Should keep all messages
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history_str = vars.get("history").unwrap();

        assert!(history_str.contains("Message 1"));
        assert!(history_str.contains("Response 1"));
        assert!(history_str.contains("Message 2"));
        assert!(history_str.contains("Response 2"));
    }

    #[tokio::test]
    async fn test_conversation_buffer_window_memory_exact_limit() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferWindowMemory::new(history).with_k(2);

        // Add exactly k=2 turns
        for i in 1..=2 {
            let mut inputs = HashMap::new();
            inputs.insert("input".to_string(), format!("Message {}", i));
            let mut outputs = HashMap::new();
            outputs.insert("output".to_string(), format!("Response {}", i));

            memory.save_context(&inputs, &outputs).await.unwrap();
        }

        // Should keep all messages
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history_str = vars.get("history").unwrap();

        assert!(history_str.contains("Message 1"));
        assert!(history_str.contains("Response 1"));
        assert!(history_str.contains("Message 2"));
        assert!(history_str.contains("Response 2"));
    }

    #[tokio::test]
    async fn test_conversation_buffer_window_memory_clear() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferWindowMemory::new(history).with_k(3);

        // Add messages
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Test".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Response".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Clear and verify
        memory.clear().await.unwrap();
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert!(vars.get("history").unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_conversation_buffer_window_memory_custom_prefixes() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferWindowMemory::new(history)
            .with_k(2)
            .with_human_prefix("User")
            .with_ai_prefix("Bot");

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history_str = vars.get("history").unwrap();

        assert!(history_str.contains("User: Hello"));
        assert!(history_str.contains("Bot: Hi"));
    }

    // ========================================================================
    // Boundary Condition Tests
    // ========================================================================

    #[tokio::test]
    async fn test_conversation_buffer_window_memory_k_one() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferWindowMemory::new(history).with_k(1);

        // Add 3 turns
        for i in 1..=3 {
            let mut inputs = HashMap::new();
            inputs.insert("input".to_string(), format!("Message {}", i));
            let mut outputs = HashMap::new();
            outputs.insert("output".to_string(), format!("Response {}", i));
            memory.save_context(&inputs, &outputs).await.unwrap();
        }

        // Should only keep last 1 turn (2 messages)
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history_str = vars.get("history").unwrap();

        assert!(!history_str.contains("Message 1"));
        assert!(!history_str.contains("Message 2"));
        assert!(history_str.contains("Message 3"));
        assert!(history_str.contains("Response 3"));
    }

    #[tokio::test]
    async fn test_conversation_buffer_window_memory_very_large_k() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferWindowMemory::new(history).with_k(1000);

        // Add 5 turns (well under k=1000)
        for i in 1..=5 {
            let mut inputs = HashMap::new();
            inputs.insert("input".to_string(), format!("Message {}", i));
            let mut outputs = HashMap::new();
            outputs.insert("output".to_string(), format!("Response {}", i));
            memory.save_context(&inputs, &outputs).await.unwrap();
        }

        // Should keep all messages
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history_str = vars.get("history").unwrap();

        for i in 1..=5 {
            assert!(history_str.contains(&format!("Message {}", i)));
            assert!(history_str.contains(&format!("Response {}", i)));
        }
    }

    #[tokio::test]
    async fn test_conversation_buffer_window_memory_sliding_window() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferWindowMemory::new(history).with_k(2);

        // Add turns one by one and verify sliding window behavior
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Turn 1".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Response 1".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // After turn 1: should see turn 1
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert!(vars.get("history").unwrap().contains("Turn 1"));

        // Add turn 2
        inputs.clear();
        inputs.insert("input".to_string(), "Turn 2".to_string());
        outputs.clear();
        outputs.insert("output".to_string(), "Response 2".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // After turn 2: should see turns 1 and 2
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history_str = vars.get("history").unwrap();
        assert!(history_str.contains("Turn 1"));
        assert!(history_str.contains("Turn 2"));

        // Add turn 3 (exceeds k=2)
        inputs.clear();
        inputs.insert("input".to_string(), "Turn 3".to_string());
        outputs.clear();
        outputs.insert("output".to_string(), "Response 3".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // After turn 3: should only see turns 2 and 3 (turn 1 dropped)
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history_str = vars.get("history").unwrap();
        assert!(!history_str.contains("Turn 1"));
        assert!(history_str.contains("Turn 2"));
        assert!(history_str.contains("Turn 3"));
    }

    #[tokio::test]
    async fn test_conversation_buffer_window_memory_return_messages_mode() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferWindowMemory::new(history)
            .with_k(2)
            .with_return_messages(true);

        // Add 3 turns
        for i in 1..=3 {
            let mut inputs = HashMap::new();
            inputs.insert("input".to_string(), format!("Message {}", i));
            let mut outputs = HashMap::new();
            outputs.insert("output".to_string(), format!("Response {}", i));
            memory.save_context(&inputs, &outputs).await.unwrap();
        }

        // Should return JSON array of messages
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history_value = vars.get("history").unwrap();

        assert!(history_value.starts_with('['));
        assert!(history_value.ends_with(']'));
        // Should only contain last 2 turns
        assert!(!history_value.contains("Message 1"));
        assert!(history_value.contains("Message 2"));
        assert!(history_value.contains("Message 3"));
    }

    #[tokio::test]
    async fn test_conversation_buffer_window_memory_empty_load() {
        let history = InMemoryChatMessageHistory::new();
        let memory = ConversationBufferWindowMemory::new(history).with_k(3);

        // Load from empty memory
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert_eq!(vars.get("history").unwrap(), "");
    }

    #[tokio::test]
    async fn test_conversation_buffer_window_memory_custom_memory_key() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferWindowMemory::new(history)
            .with_k(2)
            .with_memory_key("conversation");

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Test".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Response".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert!(vars.contains_key("conversation"));
        assert!(!vars.contains_key("history"));
    }

    #[tokio::test]
    async fn test_conversation_buffer_window_memory_window_at_boundary() {
        let history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationBufferWindowMemory::new(history).with_k(3);

        // Add exactly k+1 turns to test boundary
        for i in 1..=4 {
            let mut inputs = HashMap::new();
            inputs.insert("input".to_string(), format!("Turn {}", i));
            let mut outputs = HashMap::new();
            outputs.insert("output".to_string(), format!("Resp {}", i));
            memory.save_context(&inputs, &outputs).await.unwrap();
        }

        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history_str = vars.get("history").unwrap();

        // First turn should be dropped
        assert!(!history_str.contains("Turn 1"));
        // Last 3 turns should remain
        assert!(history_str.contains("Turn 2"));
        assert!(history_str.contains("Turn 3"));
        assert!(history_str.contains("Turn 4"));
    }

    #[tokio::test]
    async fn test_conversation_buffer_window_memory_memory_variables() {
        let history = InMemoryChatMessageHistory::new();
        let memory = ConversationBufferWindowMemory::new(history)
            .with_k(2)
            .with_memory_key("window");

        let vars = memory.memory_variables();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0], "window");
    }
}
