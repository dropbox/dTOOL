//! Token-buffered conversation memory implementation
//!
//! This module provides `ConversationTokenBufferMemory`, a memory type that
//! maintains conversation history while enforcing a maximum token limit. When
//! the conversation exceeds the token limit, older messages are pruned.
//!
//! # Python Baseline
//!
//! Matches `ConversationTokenBufferMemory` from
//! `dashflow.memory.token_buffer:19-75`.

use async_trait::async_trait;
use dashflow::core::chat_history::{
    get_buffer_string, BaseChatMessageHistory, InMemoryChatMessageHistory,
};
use dashflow::core::messages::Message;
use std::collections::HashMap;
use std::sync::Arc;
use tiktoken_rs::CoreBPE;

use crate::base_memory::{BaseMemory, MemoryError, MemoryResult};

/// Conversation memory with token-based buffer limit.
///
/// Keeps only the most recent messages in the conversation under the constraint
/// that the total number of tokens does not exceed a specified limit. When the
/// limit is exceeded, older messages are pruned from the beginning until the
/// conversation fits within the token budget.
///
/// This is useful for:
/// - Managing context window limits for LLMs
/// - Controlling API costs by limiting token usage
/// - Maintaining recent context while pruning old messages
///
/// # Token Counting
///
/// Uses tiktoken (`OpenAI`'s tokenizer) to count tokens. The model encoding
/// can be specified, defaulting to "`cl100k_base`" (used by GPT-3.5/GPT-4).
///
/// # Python Baseline
///
/// Matches `ConversationTokenBufferMemory` from
/// `dashflow.memory.token_buffer:19-75`.
///
/// Key differences:
/// - Rust uses explicit `tokio::sync::RwLock` (Python uses standard dict)
/// - Rust requires explicit async/await
/// - Rust uses tiktoken-rs crate (Python uses tiktoken)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_memory::ConversationTokenBufferMemory;
/// use dashflow::core::chat_history::InMemoryChatMessageHistory;
///
/// let chat_history = InMemoryChatMessageHistory::new();
/// let memory = ConversationTokenBufferMemory::new(
///     chat_history,
///     2000,  // max 2000 tokens
///     "history",
///     "Human",
///     "AI",
/// )?;
///
/// // Save conversation - old messages auto-pruned when limit exceeded
/// memory.save_context(
///     &[("input", "Hello!")],
///     &[("output", "Hi there!")],
/// ).await?;
/// ```
pub struct ConversationTokenBufferMemory {
    /// The underlying chat message history storage
    chat_memory: Arc<InMemoryChatMessageHistory>,

    /// Maximum number of tokens to keep in memory
    max_token_limit: usize,

    /// Key to use for storing history in chain inputs
    memory_key: String,

    /// Whether to return messages as structured Message objects (true)
    /// or as a formatted string (false)
    return_messages: bool,

    /// Optional input key to extract from chain inputs
    input_key: Option<String>,

    /// Optional output key to extract from chain outputs
    output_key: Option<String>,

    /// Tiktoken encoder for counting tokens
    tokenizer: Arc<CoreBPE>,
}

impl ConversationTokenBufferMemory {
    /// Create a new `ConversationTokenBufferMemory`.
    ///
    /// Uses `cl100k_base` encoding (GPT-3.5/GPT-4 tokenizer).
    ///
    /// # Arguments
    ///
    /// * `chat_memory` - Chat message history backend
    /// * `max_token_limit` - Maximum tokens to keep in memory
    /// * `memory_key` - Key for storing history in chain inputs (default: "history")
    ///
    /// # Returns
    ///
    /// Result containing the new memory instance or error if tokenizer fails to load
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow_memory::ConversationTokenBufferMemory;
    /// use dashflow::core::chat_history::InMemoryChatMessageHistory;
    ///
    /// let memory = ConversationTokenBufferMemory::new(
    ///     InMemoryChatMessageHistory::new(),
    ///     2000,
    ///     "history",
    /// )?;
    /// ```
    pub fn new(
        chat_memory: InMemoryChatMessageHistory,
        max_token_limit: usize,
        memory_key: impl Into<String>,
    ) -> MemoryResult<Self> {
        // Use cl100k_base encoding (GPT-3.5/GPT-4)
        let tokenizer = tiktoken_rs::cl100k_base().map_err(|e| {
            MemoryError::InvalidConfiguration(format!("Failed to load tokenizer: {e}"))
        })?;

        Ok(Self {
            chat_memory: Arc::new(chat_memory),
            max_token_limit,
            memory_key: memory_key.into(),
            return_messages: false,
            input_key: None,
            output_key: None,
            tokenizer: Arc::new(tokenizer),
        })
    }

    /// Create with default settings (`cl100k_base` encoding, "history" key).
    ///
    /// # Arguments
    ///
    /// * `chat_memory` - Chat message history backend
    /// * `max_token_limit` - Maximum tokens to keep in memory
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow_memory::ConversationTokenBufferMemory;
    /// use dashflow::core::chat_history::InMemoryChatMessageHistory;
    ///
    /// let memory = ConversationTokenBufferMemory::with_defaults(
    ///     InMemoryChatMessageHistory::new(),
    ///     2000,
    /// )?;
    /// ```
    pub fn with_defaults(
        chat_memory: InMemoryChatMessageHistory,
        max_token_limit: usize,
    ) -> MemoryResult<Self> {
        Self::new(chat_memory, max_token_limit, "history")
    }

    /// Set whether to return messages as structured objects.
    ///
    /// If true, returns `Vec<Message>`. If false, returns formatted string.
    #[must_use]
    pub fn with_return_messages(mut self, return_messages: bool) -> Self {
        self.return_messages = return_messages;
        self
    }

    /// Set the input key for extracting user input from chain inputs.
    pub fn with_input_key(mut self, input_key: impl Into<String>) -> Self {
        self.input_key = Some(input_key.into());
        self
    }

    /// Set the output key for extracting AI output from chain outputs.
    pub fn with_output_key(mut self, output_key: impl Into<String>) -> Self {
        self.output_key = Some(output_key.into());
        self
    }

    /// Get the current buffer as a string.
    async fn buffer_as_str(&self) -> MemoryResult<String> {
        let messages = self
            .chat_memory
            .get_messages()
            .await
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))?;
        Ok(get_buffer_string(&messages))
    }

    /// Get the current buffer as messages.
    async fn buffer_as_messages(&self) -> MemoryResult<Vec<Message>> {
        self.chat_memory
            .get_messages()
            .await
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))
    }

    /// Count tokens in the current message buffer.
    ///
    /// Converts messages to string format and counts tokens using tiktoken.
    async fn count_tokens(&self) -> MemoryResult<usize> {
        let buffer_str = self.buffer_as_str().await?;
        let tokens = self.tokenizer.encode_with_special_tokens(&buffer_str);
        Ok(tokens.len())
    }

    /// Prune old messages if token count exceeds limit.
    ///
    /// Removes messages from the beginning of the conversation until the
    /// token count is under the limit.
    async fn prune_if_needed(&self) -> MemoryResult<()> {
        let mut current_token_count = self.count_tokens().await?;

        if current_token_count <= self.max_token_limit {
            return Ok(());
        }

        // Need to prune - remove messages from the beginning
        let mut messages = self
            .chat_memory
            .get_messages()
            .await
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))?;

        while current_token_count > self.max_token_limit && !messages.is_empty() {
            // Remove oldest message
            messages.remove(0);

            // Recalculate token count
            let buffer_str = get_buffer_string(&messages);
            let tokens = self.tokenizer.encode_with_special_tokens(&buffer_str);
            current_token_count = tokens.len();
        }

        // Clear and re-add pruned messages
        self.chat_memory
            .clear()
            .await
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))?;
        self.chat_memory
            .add_messages(&messages)
            .await
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))?;

        Ok(())
    }

    /// Extract input/output from chain dictionaries.
    ///
    /// Matches the logic from Python's `BaseChatMemory`._`get_input_output()`.
    fn get_input_output<'a>(
        &self,
        inputs: &'a HashMap<String, String>,
        outputs: &'a HashMap<String, String>,
        memory_variables: &[String],
    ) -> MemoryResult<(&'a str, &'a str)> {
        // Determine input key
        let input_key = if let Some(ref key) = self.input_key {
            key
        } else {
            // Find first key in inputs that's not a memory variable
            inputs
                .keys()
                .find(|k| !memory_variables.contains(k))
                .ok_or_else(|| {
                    MemoryError::InvalidConfiguration(
                        "No valid input key found in inputs".to_string(),
                    )
                })?
        };

        // Determine output key
        let output_key = if let Some(ref key) = self.output_key {
            key
        } else if outputs.len() == 1 {
            // SAFETY: len() == 1 guarantees .next() returns Some
            #[allow(clippy::unwrap_used)]
            outputs.keys().next().unwrap()
        } else if outputs.contains_key("output") {
            "output"
        } else {
            return Err(MemoryError::InvalidConfiguration(format!(
                "Got multiple output keys: {:?}, cannot determine which to store in memory. \
                 Please set the 'output_key' explicitly.",
                outputs.keys().collect::<Vec<_>>()
            )));
        };

        let input = inputs
            .get(input_key)
            .ok_or_else(|| {
                MemoryError::InvalidConfiguration(format!("Input key '{input_key}' not found"))
            })?
            .as_str();

        let output = outputs
            .get(output_key)
            .ok_or_else(|| {
                MemoryError::InvalidConfiguration(format!("Output key '{output_key}' not found"))
            })?
            .as_str();

        Ok((input, output))
    }
}

#[async_trait]
impl BaseMemory for ConversationTokenBufferMemory {
    fn memory_variables(&self) -> Vec<String> {
        vec![self.memory_key.clone()]
    }

    async fn load_memory_variables(
        &self,
        _inputs: &HashMap<String, String>,
    ) -> MemoryResult<HashMap<String, String>> {
        let mut result = HashMap::new();

        if self.return_messages {
            // Return messages as JSON string (since we can't return Vec<Message> in HashMap<String, String>)
            let messages = self.buffer_as_messages().await?;
            let messages_json =
                serde_json::to_string(&messages).map_err(MemoryError::SerializationError)?;
            result.insert(self.memory_key.clone(), messages_json);
        } else {
            // Return as formatted string
            let buffer = self.buffer_as_str().await?;
            result.insert(self.memory_key.clone(), buffer);
        }

        Ok(result)
    }

    async fn save_context(
        &mut self,
        inputs: &HashMap<String, String>,
        outputs: &HashMap<String, String>,
    ) -> MemoryResult<()> {
        let memory_vars = self.memory_variables();
        let (input, output) = self.get_input_output(inputs, outputs, &memory_vars)?;

        // Add messages to chat history
        self.chat_memory
            .add_user_message(input)
            .await
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))?;
        self.chat_memory
            .add_ai_message(output)
            .await
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))?;

        // Prune buffer if needed
        self.prune_if_needed().await?;

        Ok(())
    }

    async fn clear(&mut self) -> MemoryResult<()> {
        self.chat_memory
            .clear()
            .await
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_token_buffer_memory_basic() {
        let chat_history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationTokenBufferMemory::with_defaults(chat_history, 2000).unwrap();

        // Save a conversation
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello!".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi there!".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        // Load memory
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history = vars.get("history").unwrap();
        assert!(history.contains("Hello!"));
        assert!(history.contains("Hi there!"));
    }

    #[tokio::test]
    async fn test_token_limit_pruning() {
        let chat_history = InMemoryChatMessageHistory::new();
        // Very small token limit to force pruning
        let mut memory = ConversationTokenBufferMemory::with_defaults(chat_history, 50).unwrap();

        // Add multiple messages
        for i in 0..10 {
            let mut inputs = HashMap::new();
            inputs.insert("input".to_string(), format!("Message {}", i));
            let mut outputs = HashMap::new();
            outputs.insert("output".to_string(), format!("Response {}", i));
            memory.save_context(&inputs, &outputs).await.unwrap();
        }

        // Check that old messages were pruned
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history = vars.get("history").unwrap();

        // Should have recent messages but not the earliest ones
        assert!(history.contains("Message 9"));
        assert!(history.contains("Response 9"));
        // Due to small token limit, earliest messages should be gone
        assert!(!history.contains("Message 0"));
    }

    #[tokio::test]
    async fn test_return_messages_mode() {
        let chat_history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationTokenBufferMemory::with_defaults(chat_history, 2000)
            .unwrap()
            .with_return_messages(true);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Test message".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Test response".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history_json = vars.get("history").unwrap();

        // Should be JSON array of messages
        let messages: Vec<Message> = serde_json::from_str(history_json).unwrap();
        assert_eq!(messages.len(), 2);
    }

    #[tokio::test]
    async fn test_custom_keys() {
        let chat_history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationTokenBufferMemory::new(chat_history, 2000, "chat_history")
            .unwrap()
            .with_input_key("user_input")
            .with_output_key("assistant_output");

        let mut inputs = HashMap::new();
        inputs.insert("user_input".to_string(), "Custom input".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("assistant_output".to_string(), "Custom output".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history = vars.get("chat_history").unwrap();
        assert!(history.contains("Custom input"));
        assert!(history.contains("Custom output"));
    }

    #[tokio::test]
    async fn test_clear_memory() {
        let chat_history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationTokenBufferMemory::with_defaults(chat_history, 2000).unwrap();

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Test".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Response".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();
        memory.clear().await.unwrap();

        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history = vars.get("history").unwrap();
        assert!(history.is_empty());
    }

    #[tokio::test]
    async fn test_memory_variables() {
        let chat_history = InMemoryChatMessageHistory::new();
        let memory = ConversationTokenBufferMemory::new(chat_history, 2000, "custom_key").unwrap();

        let vars = memory.memory_variables();
        assert_eq!(vars, vec!["custom_key".to_string()]);
    }

    #[tokio::test]
    async fn test_empty_token_buffer() {
        let chat_history = InMemoryChatMessageHistory::new();
        let memory = ConversationTokenBufferMemory::with_defaults(chat_history, 2000).unwrap();

        // Load memory without any messages
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history = vars.get("history").unwrap();
        assert!(history.is_empty());

        // Verify token count is 0
        let token_count = memory.count_tokens().await.unwrap();
        assert_eq!(token_count, 0);
    }

    #[tokio::test]
    async fn test_single_message_under_limit() {
        let chat_history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationTokenBufferMemory::with_defaults(chat_history, 2000).unwrap();

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        // Verify message is stored
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history = vars.get("history").unwrap();
        assert!(history.contains("Hello"));
        assert!(history.contains("Hi"));

        // Verify token count is well under limit
        let token_count = memory.count_tokens().await.unwrap();
        assert!(token_count < 2000);
        assert!(token_count > 0);
    }

    #[tokio::test]
    async fn test_exact_token_limit() {
        let chat_history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationTokenBufferMemory::with_defaults(chat_history, 100).unwrap();

        // Add a single message
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Short".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "OK".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        let initial_count = memory.count_tokens().await.unwrap();
        assert!(initial_count <= 100);

        // Add another message, still under limit
        inputs.insert("input".to_string(), "Another".to_string());
        outputs.insert("output".to_string(), "Yes".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        let final_count = memory.count_tokens().await.unwrap();
        assert!(final_count <= 100);

        // Verify both messages present (no pruning)
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history = vars.get("history").unwrap();
        assert!(history.contains("Short"));
        assert!(history.contains("Another"));
    }

    #[tokio::test]
    async fn test_token_count_accuracy() {
        let chat_history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationTokenBufferMemory::with_defaults(chat_history, 5000).unwrap();

        // Add known message
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "The quick brown fox".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "jumps over the lazy dog".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        let token_count = memory.count_tokens().await.unwrap();
        // Token count should be reasonable (typically 10-15 tokens for this phrase)
        assert!(token_count > 5);
        assert!(token_count < 30);
    }

    #[tokio::test]
    async fn test_unicode_token_counting() {
        let chat_history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationTokenBufferMemory::with_defaults(chat_history, 1000).unwrap();

        // Test with Chinese characters
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "你好世界".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "مرحبا".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        // Verify unicode messages are stored correctly
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history = vars.get("history").unwrap();
        assert!(history.contains("你好世界"));
        assert!(history.contains("مرحبا"));

        // Test with emojis
        inputs.insert("input".to_string(), "Hello 🌍🚀".to_string());
        outputs.insert("output".to_string(), "Hi 👋😊".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history = vars.get("history").unwrap();
        assert!(history.contains("🌍"));
        assert!(history.contains("👋"));

        // Verify token count is positive
        let token_count = memory.count_tokens().await.unwrap();
        assert!(token_count > 0);
    }

    #[tokio::test]
    async fn test_very_long_single_message() {
        let chat_history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationTokenBufferMemory::with_defaults(chat_history, 50).unwrap();

        // Create a very long message that exceeds token limit
        let long_input = "word ".repeat(100); // ~500 tokens
        let long_output = "response ".repeat(100); // ~500 tokens

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), long_input);
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), long_output);

        memory.save_context(&inputs, &outputs).await.unwrap();

        // After pruning, token count should be under or at limit
        // Note: If a single message pair exceeds limit, ALL messages are pruned
        let token_count = memory.count_tokens().await.unwrap();
        assert!(token_count <= 50);

        // Add a shorter message that fits
        inputs.insert("input".to_string(), "Hi".to_string());
        outputs.insert("output".to_string(), "Hello".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // This shorter message should be retained
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history = vars.get("history").unwrap();
        assert!(history.contains("Hi"));
        assert!(history.contains("Hello"));
    }

    #[tokio::test]
    async fn test_multiple_output_keys_ambiguous() {
        let chat_history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationTokenBufferMemory::with_defaults(chat_history, 2000).unwrap();

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Test".to_string());

        // Multiple output keys without output_key set
        let mut outputs = HashMap::new();
        outputs.insert("result1".to_string(), "Response 1".to_string());
        outputs.insert("result2".to_string(), "Response 2".to_string());

        let result = memory.save_context(&inputs, &outputs).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(
            matches!(
                &err,
                MemoryError::InvalidConfiguration(msg) if msg.contains("multiple output keys")
            ),
            "Expected InvalidConfiguration error mentioning multiple output keys, got {err:?}"
        );
    }

    #[tokio::test]
    async fn test_missing_input_key() {
        let chat_history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationTokenBufferMemory::with_defaults(chat_history, 2000)
            .unwrap()
            .with_input_key("custom_input");

        let mut inputs = HashMap::new();
        inputs.insert("wrong_key".to_string(), "Test".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Response".to_string());

        let result = memory.save_context(&inputs, &outputs).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(
            matches!(
                &err,
                MemoryError::InvalidConfiguration(msg)
                    if msg.contains("custom_input") && msg.contains("not found")
            ),
            "Expected InvalidConfiguration error for missing input key, got {err:?}"
        );
    }

    #[tokio::test]
    async fn test_gradual_overflow() {
        let chat_history = InMemoryChatMessageHistory::new();
        let mut memory = ConversationTokenBufferMemory::with_defaults(chat_history, 60).unwrap();

        // Add messages one by one and track when pruning occurs
        for i in 0..10 {
            let mut inputs = HashMap::new();
            inputs.insert("input".to_string(), format!("Msg {}", i));
            let mut outputs = HashMap::new();
            outputs.insert("output".to_string(), format!("Resp {}", i));

            memory.save_context(&inputs, &outputs).await.unwrap();

            let token_count = memory.count_tokens().await.unwrap();
            assert!(
                token_count <= 60,
                "Token count {} exceeds limit 60 at iteration {}",
                token_count,
                i
            );
        }

        // Verify newest messages remain
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let history = vars.get("history").unwrap();
        assert!(history.contains("Msg 9"));
        assert!(history.contains("Resp 9"));

        // With a small token limit (60) and 10 iterations, earliest messages should be pruned
        // Token count for "Human: Msg 0\nAI: Resp 0" is roughly 12-15 tokens
        // With limit 60, we can fit ~4 message pairs, so messages 0-5 should be gone
        assert!(!history.contains("Msg 0"), "Message 0 should be pruned");
        assert!(!history.contains("Msg 1"), "Message 1 should be pruned");
    }
}
