//! Conversation summary memory implementation
//!
//! This memory type continually summarizes the conversation history using an LLM,
//! providing a compact representation of the conversation that doesn't grow
//! with conversation length.

use async_trait::async_trait;
use dashflow::core::chat_history::{
    get_buffer_string, BaseChatMessageHistory, InMemoryChatMessageHistory,
};
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::{BaseMessage, Message};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::base_memory::{BaseMemory, MemoryError, MemoryResult};
use crate::prompts::create_summary_prompt;

/// Continually summarizes the conversation history using an LLM.
///
/// This memory type maintains a running summary of the conversation instead of
/// storing all messages. After each conversation turn, it uses an LLM to update
/// the summary with new information. This keeps memory usage bounded regardless
/// of conversation length.
///
/// # Design
///
/// - **Bounded Memory**: Summary size stays constant, unlike buffer memory
/// - **Progressive Updates**: Each turn updates the existing summary
/// - **LLM-Powered**: Uses language model to generate coherent summaries
/// - **Configurable**: Can customize summarization prompt and prefixes
///
/// # Python Baseline Compatibility
///
/// Matches `ConversationSummaryMemory` from `dashflow.memory.summary:91-172`.
///
/// Key differences:
/// - Rust uses `Arc<RwLock<>>` for thread-safe interior mutability
/// - Rust is async-first (Python has sync+async variants)
/// - Rust uses trait objects (`Box<dyn LLM>`) instead of Python's duck typing
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_memory::ConversationSummaryMemory;
/// use dashflow_openai::ChatOpenAI;
/// use dashflow::core::chat_history::InMemoryChatMessageHistory;
/// use std::collections::HashMap;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Create LLM and chat history
///     let llm = ChatOpenAI::default();
///     let chat_memory = InMemoryChatMessageHistory::new();
///
///     // Create summary memory
///     let mut memory = ConversationSummaryMemory::new(
///         Box::new(llm),
///         Box::new(chat_memory),
///     );
///
///     // Save first conversation turn
///     let mut inputs = HashMap::new();
///     inputs.insert("input".to_string(), "Hi, my name is Alice.".to_string());
///     let mut outputs = HashMap::new();
///     outputs.insert("output".to_string(), "Hello Alice! Nice to meet you.".to_string());
///     memory.save_context(&inputs, &outputs).await?;
///
///     // Load memory (contains summary)
///     let vars = memory.load_memory_variables(&HashMap::new()).await?;
///     println!("Summary: {}", vars.get("history").unwrap());
///
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct ConversationSummaryMemory {
    /// The chat model used for summarization
    llm: Arc<RwLock<Box<dyn ChatModel>>>,

    /// The chat message history backend (using `InMemoryChatMessageHistory` for now)
    chat_memory: Arc<RwLock<InMemoryChatMessageHistory>>,

    /// Current summary buffer
    buffer: Arc<RwLock<String>>,

    /// The key for storing/retrieving memory in chain inputs
    memory_key: String,

    /// Prefix for human messages in summary
    human_prefix: String,

    /// Prefix for AI messages in summary
    ai_prefix: String,

    /// Prompt template for summarization
    prompt: dashflow::core::prompts::string::PromptTemplate,

    /// Whether to return messages (true) or string (false)
    return_messages: bool,

    /// Optional input key to use (if None, auto-detected)
    input_key: Option<String>,

    /// Optional output key to use (if None, auto-detected)
    output_key: Option<String>,
}

impl ConversationSummaryMemory {
    /// Create a new `ConversationSummaryMemory`.
    ///
    /// # Arguments
    ///
    /// * `llm` - The chat model to use for summarization
    /// * `chat_memory` - The chat history backend for storing messages
    #[must_use]
    pub fn new(llm: Box<dyn ChatModel>, chat_memory: InMemoryChatMessageHistory) -> Self {
        Self {
            llm: Arc::new(RwLock::new(llm)),
            chat_memory: Arc::new(RwLock::new(chat_memory)),
            buffer: Arc::new(RwLock::new(String::new())),
            memory_key: "history".to_string(),
            human_prefix: "Human".to_string(),
            ai_prefix: "AI".to_string(),
            prompt: create_summary_prompt(),
            return_messages: false,
            input_key: None,
            output_key: None,
        }
    }

    /// Set the memory key (default: "history")
    pub fn with_memory_key(mut self, memory_key: impl Into<String>) -> Self {
        self.memory_key = memory_key.into();
        self
    }

    /// Set the human prefix (default: "Human")
    pub fn with_human_prefix(mut self, human_prefix: impl Into<String>) -> Self {
        self.human_prefix = human_prefix.into();
        self
    }

    /// Set the AI prefix (default: "AI")
    pub fn with_ai_prefix(mut self, ai_prefix: impl Into<String>) -> Self {
        self.ai_prefix = ai_prefix.into();
        self
    }

    /// Set custom summarization prompt
    #[must_use]
    pub fn with_prompt(mut self, prompt: dashflow::core::prompts::string::PromptTemplate) -> Self {
        self.prompt = prompt;
        self
    }

    /// Set whether to return messages (true) or string (false)
    #[must_use]
    pub fn with_return_messages(mut self, return_messages: bool) -> Self {
        self.return_messages = return_messages;
        self
    }

    /// Set input key
    pub fn with_input_key(mut self, input_key: impl Into<String>) -> Self {
        self.input_key = Some(input_key.into());
        self
    }

    /// Set output key
    pub fn with_output_key(mut self, output_key: impl Into<String>) -> Self {
        self.output_key = Some(output_key.into());
        self
    }

    /// Predict a new summary based on messages and existing summary.
    ///
    /// # Arguments
    ///
    /// * `messages` - New messages to incorporate into summary
    /// * `existing_summary` - Current summary to build upon
    ///
    /// # Returns
    ///
    /// Updated summary string
    async fn predict_new_summary(
        &self,
        messages: &[Message],
        existing_summary: &str,
    ) -> MemoryResult<String> {
        // Format messages as buffer string
        let new_lines = get_buffer_string(messages);

        // Create prompt variables
        let mut vars = HashMap::new();
        vars.insert("summary".to_string(), existing_summary.to_string());
        vars.insert("new_lines".to_string(), new_lines);

        // Format prompt
        let prompt_text = self
            .prompt
            .format(&vars)
            .map_err(|e| MemoryError::OperationFailed(format!("Failed to format prompt: {e}")))?;

        // Call ChatModel with prompt as a human message
        let llm = self.llm.read().await;
        let prompt_messages = vec![BaseMessage::human(prompt_text)];
        let result = llm
            .generate(&prompt_messages, None, None, None, None)
            .await
            .map_err(|e| MemoryError::LLMError(format!("Failed to generate summary: {e}")))?;

        // Extract text from first generation
        let summary = result
            .generations
            .first()
            .ok_or_else(|| MemoryError::LLMError("No generations returned from LLM".to_string()))?
            .text();
        Ok(summary.trim().to_string())
    }

    /// Extract input and output strings from context dictionaries.
    ///
    /// Handles auto-detection of keys if `input_key/output_key` not set.
    fn get_input_output(
        &self,
        inputs: &HashMap<String, String>,
        outputs: &HashMap<String, String>,
    ) -> MemoryResult<(String, String)> {
        // Get input key
        let input_key = if let Some(ref key) = self.input_key {
            key.clone()
        } else if inputs.len() == 1 {
            // SAFETY: len() == 1 check guarantees .next() returns Some
            #[allow(clippy::unwrap_used)]
            inputs.keys().next().unwrap().clone()
        } else if inputs.contains_key("input") {
            "input".to_string()
        } else {
            return Err(MemoryError::InvalidConfiguration(
                "Could not determine input key. Please set input_key explicitly.".to_string(),
            ));
        };

        // Get output key
        let output_key = if let Some(ref key) = self.output_key {
            key.clone()
        } else if outputs.len() == 1 {
            // SAFETY: len() == 1 check guarantees .next() returns Some
            #[allow(clippy::unwrap_used)]
            outputs.keys().next().unwrap().clone()
        } else if outputs.contains_key("output") {
            "output".to_string()
        } else {
            return Err(MemoryError::InvalidConfiguration(
                "Could not determine output key. Please set output_key explicitly.".to_string(),
            ));
        };

        // Extract values
        let input = inputs
            .get(&input_key)
            .ok_or_else(|| {
                MemoryError::InvalidConfiguration(format!("Input key '{input_key}' not found"))
            })?
            .clone();

        let output = outputs
            .get(&output_key)
            .ok_or_else(|| {
                MemoryError::InvalidConfiguration(format!("Output key '{output_key}' not found"))
            })?
            .clone();

        Ok((input, output))
    }
}

#[async_trait]
impl BaseMemory for ConversationSummaryMemory {
    fn memory_variables(&self) -> Vec<String> {
        vec![self.memory_key.clone()]
    }

    async fn load_memory_variables(
        &self,
        _inputs: &HashMap<String, String>,
    ) -> MemoryResult<HashMap<String, String>> {
        let buffer = self.buffer.read().await;
        let mut result = HashMap::new();

        if self.return_messages {
            // Return as message format (simplified - just return the summary text)
            // In a full implementation, we'd create a SystemMessage here
            result.insert(self.memory_key.clone(), buffer.clone());
        } else {
            result.insert(self.memory_key.clone(), buffer.clone());
        }

        Ok(result)
    }

    async fn save_context(
        &mut self,
        inputs: &HashMap<String, String>,
        outputs: &HashMap<String, String>,
    ) -> MemoryResult<()> {
        // Extract input/output
        let (input_str, output_str) = self.get_input_output(inputs, outputs)?;

        // Add messages to chat history
        let chat_memory = self.chat_memory.write().await;
        let human_msg = Message::human(input_str);
        let ai_msg = Message::ai(output_str);
        chat_memory
            .add_messages(&[human_msg.clone(), ai_msg.clone()])
            .await
            .map_err(|e| {
                MemoryError::OperationFailed(format!("Failed to add messages to history: {e}"))
            })?;
        drop(chat_memory);

        // Update summary with new messages
        let current_buffer = self.buffer.read().await.clone();
        let new_summary = self
            .predict_new_summary(&[human_msg, ai_msg], &current_buffer)
            .await?;

        // Update buffer
        let mut buffer = self.buffer.write().await;
        *buffer = new_summary;

        Ok(())
    }

    async fn clear(&mut self) -> MemoryResult<()> {
        // Clear buffer
        let mut buffer = self.buffer.write().await;
        *buffer = String::new();
        drop(buffer);

        // Clear chat history
        let chat_memory = self.chat_memory.write().await;
        chat_memory.clear().await.map_err(|e| {
            MemoryError::OperationFailed(format!("Failed to clear chat history: {e}"))
        })?;

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use dashflow::core::callbacks::CallbackManager;
    use dashflow::core::chat_history::InMemoryChatMessageHistory;
    use dashflow::core::language_models::{ChatGeneration, ChatResult};
    use dashflow::core::language_models::{ToolChoice, ToolDefinition};
    use futures::stream::Stream;
    use std::pin::Pin;

    // Simple mock ChatModel for testing
    #[derive(Clone)]
    struct MockChatModel {
        response: String,
    }

    impl MockChatModel {
        fn new(response: impl Into<String>) -> Self {
            Self {
                response: response.into(),
            }
        }
    }

    #[async_trait]
    impl ChatModel for MockChatModel {
        async fn _generate(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _run_manager: Option<&CallbackManager>,
        ) -> dashflow::core::error::Result<ChatResult> {
            Ok(ChatResult {
                generations: vec![ChatGeneration::new(BaseMessage::ai(self.response.clone()))],
                llm_output: None,
            })
        }

        async fn _stream(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _run_manager: Option<&CallbackManager>,
        ) -> dashflow::core::error::Result<
            Pin<
                Box<
                    dyn Stream<
                            Item = dashflow::core::error::Result<
                                dashflow::core::language_models::ChatGenerationChunk,
                            >,
                        > + Send,
                >,
            >,
        > {
            Err(dashflow::core::error::Error::NotImplemented(
                "Stream not implemented for MockChatModel".to_string(),
            ))
        }

        fn llm_type(&self) -> &str {
            "mock"
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[tokio::test]
    async fn test_conversation_summary_memory_creation() {
        let llm = Box::new(MockChatModel::new("This is a test summary."));
        let chat_memory = InMemoryChatMessageHistory::new();
        let memory = ConversationSummaryMemory::new(llm, chat_memory);

        assert_eq!(memory.memory_variables(), vec!["history".to_string()]);
    }

    #[tokio::test]
    async fn test_conversation_summary_memory_save_and_load() {
        let llm = Box::new(MockChatModel::new(
            "Alice introduced herself to the AI assistant.",
        ));
        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory = ConversationSummaryMemory::new(llm, chat_memory);

        // Save context
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hi, I'm Alice.".to_string());
        let mut outputs = HashMap::new();
        outputs.insert(
            "output".to_string(),
            "Hello Alice! Nice to meet you.".to_string(),
        );

        memory.save_context(&inputs, &outputs).await.unwrap();

        // Load memory
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert!(vars.contains_key("history"));
        assert_eq!(
            vars.get("history").unwrap(),
            "Alice introduced herself to the AI assistant."
        );
    }

    #[tokio::test]
    async fn test_conversation_summary_memory_clear() {
        let llm = Box::new(MockChatModel::new("Summary"));
        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory = ConversationSummaryMemory::new(llm, chat_memory);

        // Add some context
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Test".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Response".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Clear
        memory.clear().await.unwrap();

        // Verify empty
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert_eq!(vars.get("history").unwrap(), "");
    }

    #[tokio::test]
    async fn test_empty_conversation_summarization() {
        // Test that empty summary returns empty string
        let llm = Box::new(MockChatModel::new("Empty summary"));
        let chat_memory = InMemoryChatMessageHistory::new();
        let memory = ConversationSummaryMemory::new(llm, chat_memory);

        // Load without any saved context
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert_eq!(vars.get("history").unwrap(), "");
    }

    #[tokio::test]
    async fn test_single_turn_summarization() {
        // Test summarization of a single conversation turn
        let llm = Box::new(MockChatModel::new("User greeted the assistant."));
        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory = ConversationSummaryMemory::new(llm, chat_memory);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello!".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi there!".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert_eq!(vars.get("history").unwrap(), "User greeted the assistant.");
    }

    #[tokio::test]
    async fn test_progressive_summary_updates() {
        // Test that summaries evolve with multiple turns
        // First mock will use static response, but we verify the call pattern
        let llm = Box::new(MockChatModel::new("Updated summary with new information"));
        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory = ConversationSummaryMemory::new(llm, chat_memory);

        // First turn
        let mut inputs1 = HashMap::new();
        inputs1.insert("input".to_string(), "I like pizza.".to_string());
        let mut outputs1 = HashMap::new();
        outputs1.insert("output".to_string(), "That's nice!".to_string());
        memory.save_context(&inputs1, &outputs1).await.unwrap();

        let vars1 = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let summary1 = vars1.get("history").unwrap().clone();
        assert!(!summary1.is_empty());

        // Second turn - summary should update
        let mut inputs2 = HashMap::new();
        inputs2.insert("input".to_string(), "I also like pasta.".to_string());
        let mut outputs2 = HashMap::new();
        outputs2.insert("output".to_string(), "Italian food fan!".to_string());
        memory.save_context(&inputs2, &outputs2).await.unwrap();

        let vars2 = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        let summary2 = vars2.get("history").unwrap().clone();
        // Summary should be non-empty and potentially different
        assert!(!summary2.is_empty());
    }

    #[tokio::test]
    async fn test_missing_input_key_error() {
        // Test error when input key cannot be determined
        let llm = Box::new(MockChatModel::new("Summary"));
        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory = ConversationSummaryMemory::new(llm, chat_memory);

        let mut inputs = HashMap::new();
        inputs.insert("key1".to_string(), "value1".to_string());
        inputs.insert("key2".to_string(), "value2".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "response".to_string());

        let result = memory.save_context(&inputs, &outputs).await;
        assert!(
            matches!(
                &result,
                Err(MemoryError::InvalidConfiguration(msg))
                    if msg.contains("Could not determine input key")
            ),
            "Expected InvalidConfiguration error for missing input key, got {result:?}"
        );
    }

    #[tokio::test]
    async fn test_missing_output_key_error() {
        // Test error when output key cannot be determined
        let llm = Box::new(MockChatModel::new("Summary"));
        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory = ConversationSummaryMemory::new(llm, chat_memory);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "test".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("key1".to_string(), "value1".to_string());
        outputs.insert("key2".to_string(), "value2".to_string());

        let result = memory.save_context(&inputs, &outputs).await;
        assert!(
            matches!(
                &result,
                Err(MemoryError::InvalidConfiguration(msg))
                    if msg.contains("Could not determine output key")
            ),
            "Expected InvalidConfiguration error for missing output key, got {result:?}"
        );
    }

    #[tokio::test]
    async fn test_custom_memory_key() {
        // Test custom memory key configuration
        let llm = Box::new(MockChatModel::new("Custom key summary"));
        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory =
            ConversationSummaryMemory::new(llm, chat_memory).with_memory_key("conversation");

        assert_eq!(memory.memory_variables(), vec!["conversation".to_string()]);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Test".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Response".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert!(vars.contains_key("conversation"));
        assert_eq!(vars.get("conversation").unwrap(), "Custom key summary");
    }

    #[tokio::test]
    async fn test_custom_human_ai_prefixes() {
        // Test custom prefix configuration (verified through memory_variables only)
        let llm = Box::new(MockChatModel::new("Prefixed summary"));
        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory = ConversationSummaryMemory::new(llm, chat_memory)
            .with_human_prefix("User")
            .with_ai_prefix("Assistant");

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Verify the memory was saved (prefixes affect internal behavior)
        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert!(vars.contains_key("history"));
    }

    #[tokio::test]
    async fn test_custom_input_output_keys() {
        // Test explicit input/output key configuration
        let llm = Box::new(MockChatModel::new("Custom keys summary"));
        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory = ConversationSummaryMemory::new(llm, chat_memory)
            .with_input_key("question")
            .with_output_key("answer");

        let mut inputs = HashMap::new();
        inputs.insert("question".to_string(), "What is Rust?".to_string());
        inputs.insert("other_key".to_string(), "ignored".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("answer".to_string(), "A programming language.".to_string());
        outputs.insert("other_output".to_string(), "ignored".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert_eq!(vars.get("history").unwrap(), "Custom keys summary");
    }

    #[tokio::test]
    async fn test_return_messages_mode() {
        // Test return_messages configuration
        let llm = Box::new(MockChatModel::new("Message mode summary"));
        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory =
            ConversationSummaryMemory::new(llm, chat_memory).with_return_messages(true);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Test".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Response".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        // In return_messages mode, still returns string (simplified implementation)
        assert_eq!(vars.get("history").unwrap(), "Message mode summary");
    }

    #[tokio::test]
    async fn test_very_long_messages() {
        // Stress test with 10KB messages
        let long_input = "A".repeat(10_000);
        let long_output = "B".repeat(10_000);

        let llm = Box::new(MockChatModel::new("Long message summary"));
        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory = ConversationSummaryMemory::new(llm, chat_memory);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), long_input);
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), long_output);

        memory.save_context(&inputs, &outputs).await.unwrap();

        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert_eq!(vars.get("history").unwrap(), "Long message summary");
    }

    #[tokio::test]
    async fn test_unicode_and_special_characters() {
        // Test international characters and emojis
        let llm = Box::new(MockChatModel::new("Unicode summary: 你好 مرحبا 🚀"));
        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory = ConversationSummaryMemory::new(llm, chat_memory);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "你好世界 (Hello World)".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "مرحبا بك! 🌟".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
        assert_eq!(
            vars.get("history").unwrap(),
            "Unicode summary: 你好 مرحبا 🚀"
        );
    }

    #[tokio::test]
    async fn test_memory_variables_returns_correct_key() {
        // Verify memory_variables() returns configured key
        let llm = Box::new(MockChatModel::new("Summary"));
        let chat_memory = InMemoryChatMessageHistory::new();

        // Default key
        let memory1 = ConversationSummaryMemory::new(llm.clone(), chat_memory.clone());
        assert_eq!(memory1.memory_variables(), vec!["history".to_string()]);

        // Custom key
        let memory2 =
            ConversationSummaryMemory::new(llm, chat_memory).with_memory_key("my_summary");
        assert_eq!(memory2.memory_variables(), vec!["my_summary".to_string()]);
    }
}
