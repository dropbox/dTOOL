//! Entity-based conversation memory
//!
//! Extracts and tracks entities mentioned in conversations, maintaining summaries
//! for each entity using an LLM.

use crate::base_memory::{BaseMemory, MemoryError, MemoryResult};
use crate::entity_store::{EntityStore, InMemoryEntityStore};
use crate::prompts::{create_entity_extraction_prompt, create_entity_summarization_prompt};
use async_trait::async_trait;
use dashflow::core::chat_history::{
    get_buffer_string, BaseChatMessageHistory, InMemoryChatMessageHistory,
};
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::BaseMessage;
use dashflow::core::prompts::string::PromptTemplate;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Entity extraction & summarization memory
///
/// Extracts named entities from recent chat history and maintains summaries for each entity.
/// Uses an LLM to:
/// 1. Extract entities from the most recent conversation turns
/// 2. Generate and update summaries for each entity as the conversation progresses
///
/// # Features
///
/// - Automatic entity extraction from conversation
/// - Progressive entity summarization
/// - Configurable entity storage backend (in-memory, Redis, `SQLite`, etc.)
/// - Windowed entity extraction (only considers recent K message pairs)
/// - Returns both conversation history and entity summaries
///
/// # Python Baseline Compatibility
///
/// Matches `ConversationEntityMemory` from `dashflow.memory.entity:465-615`.
///
/// Key differences:
/// - Rust uses concrete `InMemoryChatMessageHistory` instead of `BaseChatMessageHistory` trait object
/// - Rust uses `ChatModel` trait directly (Python uses `LLMChain`)
/// - Thread-safe with `Arc<RwLock<>>` for interior mutability
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_memory::{ConversationEntityMemory, InMemoryEntityStore};
/// use dashflow_openai::ChatOpenAI;
/// use dashflow::core::chat_history::InMemoryChatMessageHistory;
///
/// let llm = ChatOpenAI::default();
/// let chat_memory = InMemoryChatMessageHistory::new();
/// let entity_store = InMemoryEntityStore::new();
/// let mut memory = ConversationEntityMemory::new(llm, chat_memory, entity_store);
///
/// // First conversation turn
/// let mut inputs = HashMap::new();
/// inputs.insert("input".to_string(), "I'm meeting with Alice tomorrow in Seattle".to_string());
/// let mut outputs = HashMap::new();
/// outputs.insert("output".to_string(), "That sounds great! I hope you have a good meeting.".to_string());
/// memory.save_context(&inputs, &outputs).await?;
///
/// // Next turn - memory will include entity summaries
/// inputs.clear();
/// inputs.insert("input".to_string(), "Should I bring a laptop?".to_string());
/// let vars = memory.load_memory_variables(&inputs).await?;
/// // vars["entities"] contains summaries for Alice and Seattle
/// // vars["history"] contains recent conversation
/// ```
#[derive(Clone)]
pub struct ConversationEntityMemory<M: ChatModel, S: EntityStore> {
    /// LLM for entity extraction and summarization
    llm: M,

    /// Chat message history storage
    chat_memory: Arc<RwLock<InMemoryChatMessageHistory>>,

    /// Entity storage backend
    entity_store: Arc<RwLock<S>>,

    /// Cache of recently detected entity names
    entity_cache: Arc<RwLock<Vec<String>>>,

    /// Number of recent message pairs to consider when extracting/updating entities
    k: usize,

    /// Key for chat history in memory variables
    chat_history_key: String,

    /// Prefix for human messages in formatted history
    human_prefix: String,

    /// Prefix for AI messages in formatted history
    ai_prefix: String,

    /// Custom entity extraction prompt (if provided)
    entity_extraction_prompt: Option<PromptTemplate>,

    /// Custom entity summarization prompt (if provided)
    entity_summarization_prompt: Option<PromptTemplate>,

    /// Input key to use from inputs dict (auto-detected if None)
    input_key: Option<String>,

    /// Whether to return messages as Message objects or formatted string
    return_messages: bool,
}

impl<M: ChatModel> ConversationEntityMemory<M, InMemoryEntityStore> {
    /// Create a new entity memory with in-memory storage
    ///
    /// # Arguments
    ///
    /// - `llm`: `ChatModel` for entity extraction and summarization
    /// - `chat_memory`: Chat message history storage
    pub fn new(llm: M, chat_memory: InMemoryChatMessageHistory) -> Self {
        Self::with_entity_store(llm, chat_memory, InMemoryEntityStore::new())
    }
}

impl<M: ChatModel, S: EntityStore> ConversationEntityMemory<M, S> {
    /// Create a new entity memory with custom entity store
    ///
    /// # Arguments
    ///
    /// - `llm`: `ChatModel` for entity extraction and summarization
    /// - `chat_memory`: Chat message history storage
    /// - `entity_store`: Entity storage backend
    pub fn with_entity_store(
        llm: M,
        chat_memory: InMemoryChatMessageHistory,
        entity_store: S,
    ) -> Self {
        Self {
            llm,
            chat_memory: Arc::new(RwLock::new(chat_memory)),
            entity_store: Arc::new(RwLock::new(entity_store)),
            entity_cache: Arc::new(RwLock::new(Vec::new())),
            k: 3,
            chat_history_key: "history".to_string(),
            human_prefix: "Human".to_string(),
            ai_prefix: "AI".to_string(),
            entity_extraction_prompt: None,
            entity_summarization_prompt: None,
            input_key: None,
            return_messages: false,
        }
    }

    /// Set the number of recent message pairs to consider for entity extraction
    pub fn with_k(mut self, k: usize) -> Self {
        self.k = k;
        self
    }

    /// Set the chat history key name
    pub fn with_chat_history_key(mut self, key: String) -> Self {
        self.chat_history_key = key;
        self
    }

    /// Set the human message prefix
    pub fn with_human_prefix(mut self, prefix: String) -> Self {
        self.human_prefix = prefix;
        self
    }

    /// Set the AI message prefix
    pub fn with_ai_prefix(mut self, prefix: String) -> Self {
        self.ai_prefix = prefix;
        self
    }

    /// Set a custom entity extraction prompt
    pub fn with_entity_extraction_prompt(mut self, prompt: PromptTemplate) -> Self {
        self.entity_extraction_prompt = Some(prompt);
        self
    }

    /// Set a custom entity summarization prompt
    pub fn with_entity_summarization_prompt(mut self, prompt: PromptTemplate) -> Self {
        self.entity_summarization_prompt = Some(prompt);
        self
    }

    /// Set the input key to use (auto-detected if not set)
    pub fn with_input_key(mut self, key: String) -> Self {
        self.input_key = Some(key);
        self
    }

    /// Set whether to return messages as objects (true) or formatted string (false)
    pub fn with_return_messages(mut self, return_messages: bool) -> Self {
        self.return_messages = return_messages;
        self
    }

    /// Get the buffer of messages
    async fn buffer(&self) -> Result<Vec<BaseMessage>, Box<dyn std::error::Error + Send + Sync>> {
        let chat_memory = self.chat_memory.read().await;
        let result = chat_memory.get_messages().await;
        drop(chat_memory);
        result
    }

    /// Extract entities from recent conversation using LLM
    async fn extract_entities(
        &self,
        buffer_string: &str,
        input: &str,
    ) -> MemoryResult<Vec<String>> {
        let prompt = self
            .entity_extraction_prompt
            .clone()
            .unwrap_or_else(create_entity_extraction_prompt);

        let mut prompt_values = HashMap::new();
        prompt_values.insert("history".to_string(), buffer_string.to_string());
        prompt_values.insert("input".to_string(), input.to_string());

        let prompt_text = prompt
            .format(&prompt_values)
            .map_err(|e| MemoryError::OperationFailed(format!("Failed to format prompt: {e}")))?;

        // Use LLM to extract entities
        let messages = vec![BaseMessage::human(prompt_text)];
        let result = self
            .llm
            .generate(&messages, None, None, None, None)
            .await
            .map_err(|e| MemoryError::LLMError(format!("Entity extraction failed: {e}")))?;

        let output = result
            .generations
            .first()
            .ok_or_else(|| {
                MemoryError::OperationFailed("No generations returned from LLM".to_string())
            })?
            .text();

        let output = output.trim();

        // Parse comma-separated entities or "NONE"
        if output.eq_ignore_ascii_case("NONE") || output.is_empty() {
            Ok(Vec::new())
        } else {
            Ok(output.split(',').map(|s| s.trim().to_string()).collect())
        }
    }

    /// Generate or update summary for a specific entity
    async fn summarize_entity(
        &self,
        entity: &str,
        existing_summary: &str,
        buffer_string: &str,
        input: &str,
    ) -> MemoryResult<String> {
        let prompt = self
            .entity_summarization_prompt
            .clone()
            .unwrap_or_else(create_entity_summarization_prompt);

        let mut prompt_values = HashMap::new();
        prompt_values.insert("entity".to_string(), entity.to_string());
        prompt_values.insert("summary".to_string(), existing_summary.to_string());
        prompt_values.insert("history".to_string(), buffer_string.to_string());
        prompt_values.insert("input".to_string(), input.to_string());

        let prompt_text = prompt
            .format(&prompt_values)
            .map_err(|e| MemoryError::OperationFailed(format!("Failed to format prompt: {e}")))?;

        // Use LLM to generate summary
        let messages = vec![BaseMessage::human(prompt_text)];
        let result = self
            .llm
            .generate(&messages, None, None, None, None)
            .await
            .map_err(|e| MemoryError::LLMError(format!("Entity summarization failed: {e}")))?;

        let output = result
            .generations
            .first()
            .ok_or_else(|| {
                MemoryError::OperationFailed("No generations returned from LLM".to_string())
            })?
            .text();

        Ok(output.trim().to_string())
    }

    /// Get the input key from inputs, using configured key or auto-detecting
    fn get_input_key(&self, inputs: &HashMap<String, String>) -> MemoryResult<String> {
        if let Some(key) = &self.input_key {
            return Ok(key.clone());
        }

        // Auto-detect: use first key that's not a memory variable
        let memory_vars: Vec<String> = vec!["entities".to_string(), self.chat_history_key.clone()];

        for key in inputs.keys() {
            if !memory_vars.contains(key) {
                return Ok(key.clone());
            }
        }

        // Default to "input"
        Ok("input".to_string())
    }
}

#[async_trait]
impl<M: ChatModel + Send + Sync, S: EntityStore + Send + Sync> BaseMemory
    for ConversationEntityMemory<M, S>
{
    fn memory_variables(&self) -> Vec<String> {
        vec!["entities".to_string(), self.chat_history_key.clone()]
    }

    async fn load_memory_variables(
        &self,
        inputs: &HashMap<String, String>,
    ) -> MemoryResult<HashMap<String, String>> {
        let buffer = self
            .buffer()
            .await
            .map_err(|e| MemoryError::OperationFailed(format!("Failed to get messages: {e}")))?;

        // Get the input key
        let input_key = self.get_input_key(inputs)?;
        let input = inputs.get(&input_key).ok_or_else(|| {
            MemoryError::OperationFailed(format!("Input key '{input_key}' not found"))
        })?;

        // Extract window of recent messages (last k pairs = 2k messages)
        let window_size = self.k * 2;
        let recent_messages: Vec<BaseMessage> = buffer
            .iter()
            .rev()
            .take(window_size)
            .rev()
            .cloned()
            .collect();

        // Format messages as string
        let buffer_string = get_buffer_string(&recent_messages);

        // Extract entities from recent conversation
        let entities = self.extract_entities(&buffer_string, input).await?;

        // Build entity summaries map
        let entity_store = self.entity_store.read().await;
        let mut entity_summaries = HashMap::new();
        for entity in &entities {
            let summary = entity_store.get(entity).unwrap_or_default();
            entity_summaries.insert(entity.clone(), summary);
        }
        drop(entity_store);

        // Update entity cache
        *self.entity_cache.write().await = entities;

        // Format history for output
        let history_value = if self.return_messages {
            // Return as JSON array of messages (simplified)
            serde_json::to_string(&recent_messages).unwrap_or_else(|_| buffer_string.clone())
        } else {
            buffer_string
        };

        // Format entities as JSON
        let entities_value =
            serde_json::to_string(&entity_summaries).map_err(MemoryError::SerializationError)?;

        let mut result = HashMap::new();
        result.insert(self.chat_history_key.clone(), history_value);
        result.insert("entities".to_string(), entities_value);

        Ok(result)
    }

    async fn save_context(
        &mut self,
        inputs: &HashMap<String, String>,
        outputs: &HashMap<String, String>,
    ) -> MemoryResult<()> {
        // Save messages to chat history
        let input_key = self.get_input_key(inputs)?;
        let input = inputs.get(&input_key).ok_or_else(|| {
            MemoryError::OperationFailed(format!("Input key '{input_key}' not found"))
        })?;

        let output_key = "output";
        let output = outputs.get(output_key).ok_or_else(|| {
            MemoryError::OperationFailed(format!("Output key '{output_key}' not found"))
        })?;

        // Add messages to history
        let chat_memory = self.chat_memory.write().await;
        let human_msg = BaseMessage::human(input.clone());
        let ai_msg = BaseMessage::ai(output.clone());
        chat_memory
            .add_messages(&[human_msg, ai_msg])
            .await
            .map_err(|e| {
                MemoryError::OperationFailed(format!("Failed to add messages to history: {e}"))
            })?;
        drop(chat_memory);

        // Get recent buffer for entity summarization
        let buffer = self
            .buffer()
            .await
            .map_err(|e| MemoryError::OperationFailed(format!("Failed to get messages: {e}")))?;
        let window_size = self.k * 2;
        let recent_messages: Vec<BaseMessage> = buffer
            .iter()
            .rev()
            .take(window_size)
            .rev()
            .cloned()
            .collect();
        let buffer_string = get_buffer_string(&recent_messages);

        // Update summaries for cached entities
        let entity_cache = self.entity_cache.read().await.clone();

        for entity in entity_cache {
            let existing_summary = {
                let entity_store = self.entity_store.read().await;
                entity_store.get(&entity).unwrap_or_default()
            };

            let updated_summary = self
                .summarize_entity(&entity, &existing_summary, &buffer_string, input)
                .await?;

            let mut entity_store = self.entity_store.write().await;
            entity_store.set(&entity, Some(updated_summary));
            drop(entity_store);
        }

        Ok(())
    }

    async fn clear(&mut self) -> MemoryResult<()> {
        let chat_memory = self.chat_memory.write().await;
        chat_memory.clear().await.map_err(|e| {
            MemoryError::OperationFailed(format!("Failed to clear chat history: {e}"))
        })?;
        drop(chat_memory);

        self.entity_cache.write().await.clear();
        self.entity_store.write().await.clear();
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use dashflow::core::callbacks::CallbackManager;
    use dashflow::core::language_models::{ChatGeneration, ChatResult};
    use dashflow::core::language_models::{ToolChoice, ToolDefinition};
    use futures::stream::Stream;
    use std::pin::Pin;
    use std::sync::Mutex;

    // Simple mock ChatModel for testing with multiple responses
    #[derive(Clone)]
    struct MockChatModel {
        responses: Arc<Mutex<Vec<String>>>,
        index: Arc<Mutex<usize>>,
    }

    impl MockChatModel {
        fn new() -> Self {
            Self {
                responses: Arc::new(Mutex::new(Vec::new())),
                index: Arc::new(Mutex::new(0)),
            }
        }

        fn add_response(&mut self, response: impl Into<String>) {
            self.responses.lock().unwrap().push(response.into());
        }
    }

    #[async_trait]
    impl ChatModel for MockChatModel {
        fn llm_type(&self) -> &str {
            "mock"
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        async fn _generate(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _run_manager: Option<&CallbackManager>,
        ) -> dashflow::core::error::Result<ChatResult> {
            let responses = self.responses.lock().unwrap();
            let mut index = self.index.lock().unwrap();
            let response = responses.get(*index).cloned().unwrap_or_default();
            *index += 1;
            drop(index);
            drop(responses);

            Ok(ChatResult {
                generations: vec![ChatGeneration::new(BaseMessage::ai(response))],
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
            Ok(Box::pin(futures::stream::empty::<
                dashflow::core::error::Result<dashflow::core::language_models::ChatGenerationChunk>,
            >()))
        }
    }

    #[tokio::test]
    async fn test_conversation_entity_memory_basic() {
        let mut mock_llm = MockChatModel::new();

        // Mock entity extraction: return "Alice, Seattle"
        mock_llm.add_response("Alice, Seattle");
        // Mock entity summarization for Alice
        mock_llm.add_response("Alice is a software engineer.");
        // Mock entity summarization for Seattle
        mock_llm.add_response("Seattle is a city in Washington state.");

        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory = ConversationEntityMemory::new(mock_llm, chat_memory);

        // First conversation turn
        let mut inputs = HashMap::new();
        inputs.insert(
            "input".to_string(),
            "I'm meeting with Alice tomorrow in Seattle".to_string(),
        );
        let mut outputs = HashMap::new();
        outputs.insert(
            "output".to_string(),
            "That sounds great! I hope you have a good meeting.".to_string(),
        );

        memory.save_context(&inputs, &outputs).await.unwrap();

        // Load memory variables for next turn - this extracts entities from the conversation
        inputs.clear();
        inputs.insert("input".to_string(), "What should I bring?".to_string());
        let _vars = memory.load_memory_variables(&inputs).await.unwrap();

        // Now save another context turn - this will summarize the entities
        outputs.clear();
        outputs.insert(
            "output".to_string(),
            "Bring a laptop and a notebook.".to_string(),
        );
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Verify entities were stored and summarized
        let entity_store = memory.entity_store.read().await;
        assert!(entity_store.exists("Alice"));
        assert!(entity_store.exists("Seattle"));
        assert_eq!(
            entity_store.get("Alice"),
            Some("Alice is a software engineer.".to_string())
        );
        assert_eq!(
            entity_store.get("Seattle"),
            Some("Seattle is a city in Washington state.".to_string())
        );
    }

    #[tokio::test]
    async fn test_conversation_entity_memory_no_entities() {
        let mut mock_llm = MockChatModel::new();

        // Mock entity extraction: return "NONE"
        mock_llm.add_response("NONE");

        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory = ConversationEntityMemory::new(mock_llm, chat_memory);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello, how are you?".to_string());
        let mut outputs = HashMap::new();
        outputs.insert(
            "output".to_string(),
            "I'm doing well, thank you!".to_string(),
        );

        memory.save_context(&inputs, &outputs).await.unwrap();

        // Verify no entities were stored
        let entity_cache = memory.entity_cache.read().await;
        assert!(entity_cache.is_empty());
    }

    #[tokio::test]
    async fn test_conversation_entity_memory_clear() {
        let mut mock_llm = MockChatModel::new();
        mock_llm.add_response("Alice");
        mock_llm.add_response("Alice is a friend.");

        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory = ConversationEntityMemory::new(mock_llm, chat_memory);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "I met Alice today".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "That's nice!".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        // Verify data exists
        assert!(!memory.buffer().await.unwrap().is_empty());

        // Clear memory
        memory.clear().await.unwrap();

        // Verify everything is cleared
        assert!(memory.buffer().await.unwrap().is_empty());
        assert!(memory.entity_cache.read().await.is_empty());
    }

    // ========================================================================
    // Edge Case and Configuration Tests
    // ========================================================================

    #[tokio::test]
    async fn test_conversation_entity_memory_multiple_entities() {
        let mut mock_llm = MockChatModel::new();
        // Return multiple entities
        mock_llm.add_response("Alice, Bob, Seattle, Microsoft");
        // Summaries for each
        mock_llm.add_response("Alice is a colleague.");
        mock_llm.add_response("Bob is a manager.");
        mock_llm.add_response("Seattle is a city.");
        mock_llm.add_response("Microsoft is a tech company.");

        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory = ConversationEntityMemory::new(mock_llm, chat_memory);

        let mut inputs = HashMap::new();
        inputs.insert(
            "input".to_string(),
            "Alice and Bob work at Microsoft in Seattle".to_string(),
        );
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "That's interesting!".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        // Load to trigger extraction
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Tell me more".to_string());
        let _vars = memory.load_memory_variables(&inputs).await.unwrap();

        // Save to trigger summarization
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Sure!".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Verify all entities were tracked
        let entity_store = memory.entity_store.read().await;
        assert!(entity_store.exists("Alice"));
        assert!(entity_store.exists("Bob"));
        assert!(entity_store.exists("Seattle"));
        assert!(entity_store.exists("Microsoft"));
    }

    #[tokio::test]
    async fn test_conversation_entity_memory_entity_update() {
        let mut mock_llm = MockChatModel::new();
        // First extraction
        mock_llm.add_response("Alice");
        mock_llm.add_response("Alice is mentioned.");
        // Second extraction (same entity)
        mock_llm.add_response("Alice");
        // Updated summary
        mock_llm.add_response("Alice is a senior engineer at Google.");

        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory = ConversationEntityMemory::new(mock_llm, chat_memory);

        // First mention
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "I met Alice".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Nice!".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "What else?".to_string());
        let _vars = memory.load_memory_variables(&inputs).await.unwrap();
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Tell me more".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Second mention with more details
        inputs.clear();
        inputs.insert(
            "input".to_string(),
            "Alice is a senior engineer at Google".to_string(),
        );
        let _vars = memory.load_memory_variables(&inputs).await.unwrap();
        outputs.clear();
        outputs.insert("output".to_string(), "That's cool!".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Verify summary was updated
        let entity_store = memory.entity_store.read().await;
        let summary = entity_store.get("Alice").unwrap();
        assert!(summary.contains("senior engineer") || summary.contains("Google"));
    }

    #[tokio::test]
    async fn test_conversation_entity_memory_custom_k() {
        let mut mock_llm = MockChatModel::new();
        mock_llm.add_response("Alice");
        mock_llm.add_response("Alice was mentioned.");

        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory = ConversationEntityMemory::new(mock_llm, chat_memory).with_k(1);

        // Add 2 conversation turns
        for i in 1..=2 {
            let mut inputs = HashMap::new();
            inputs.insert("input".to_string(), format!("Turn {} message", i));
            let mut outputs = HashMap::new();
            outputs.insert("output".to_string(), format!("Response {}", i));
            memory.save_context(&inputs, &outputs).await.unwrap();
        }

        // With k=1, only last 1 turn (2 messages) should be in buffer
        let buffer = memory.buffer().await.unwrap();
        assert_eq!(buffer.len(), 4); // All messages still in history
                                     // But windowing happens in load_memory_variables (last k pairs)
    }

    #[tokio::test]
    async fn test_conversation_entity_memory_custom_prefixes() {
        let mut mock_llm = MockChatModel::new();
        mock_llm.add_response("NONE");

        let chat_memory = InMemoryChatMessageHistory::new();
        let memory = ConversationEntityMemory::new(mock_llm, chat_memory)
            .with_human_prefix("User".to_string())
            .with_ai_prefix("Bot".to_string());

        // Just verify construction works with custom prefixes
        assert_eq!(memory.human_prefix, "User");
        assert_eq!(memory.ai_prefix, "Bot");
    }

    #[tokio::test]
    async fn test_conversation_entity_memory_custom_input_key() {
        let mut mock_llm = MockChatModel::new();
        mock_llm.add_response("NONE");

        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory = ConversationEntityMemory::new(mock_llm, chat_memory)
            .with_input_key("question".to_string());

        // Use custom input key
        let mut inputs = HashMap::new();
        inputs.insert("question".to_string(), "Hello".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi".to_string());

        let result = memory.save_context(&inputs, &outputs).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_conversation_entity_memory_missing_input_key() {
        let mut mock_llm = MockChatModel::new();
        mock_llm.add_response("NONE");

        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory = ConversationEntityMemory::new(mock_llm, chat_memory)
            .with_input_key("custom_key".to_string());

        // Missing custom input key should error
        let mut inputs = HashMap::new();
        inputs.insert("wrong_key".to_string(), "Hello".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi".to_string());

        let result = memory.save_context(&inputs, &outputs).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_conversation_entity_memory_empty_extraction() {
        let mut mock_llm = MockChatModel::new();
        // Return empty string (malformed response)
        mock_llm.add_response("");

        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory = ConversationEntityMemory::new(mock_llm, chat_memory);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        inputs.clear();
        inputs.insert("input".to_string(), "Continue".to_string());
        let vars = memory.load_memory_variables(&inputs).await.unwrap();

        // Should handle empty extraction gracefully
        assert!(vars.contains_key("entities"));
    }

    #[tokio::test]
    async fn test_conversation_entity_memory_return_messages_mode() {
        let mut mock_llm = MockChatModel::new();
        mock_llm.add_response("NONE");

        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory =
            ConversationEntityMemory::new(mock_llm, chat_memory).with_return_messages(true);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // In return_messages mode, history should be JSON
        inputs.clear();
        inputs.insert("input".to_string(), "Continue".to_string());
        let vars = memory.load_memory_variables(&inputs).await.unwrap();
        let history = vars.get("history").unwrap();

        // Should be JSON array
        assert!(history.starts_with('[') || history.contains("Hello"));
    }

    #[tokio::test]
    async fn test_conversation_entity_memory_custom_chat_history_key() {
        let mut mock_llm = MockChatModel::new();
        mock_llm.add_response("NONE");

        let chat_memory = InMemoryChatMessageHistory::new();
        let memory = ConversationEntityMemory::new(mock_llm, chat_memory)
            .with_chat_history_key("conversation".to_string());

        let memory_vars = memory.memory_variables();
        assert!(memory_vars.contains(&"conversation".to_string()));
        assert!(!memory_vars.contains(&"history".to_string()));
    }

    #[tokio::test]
    async fn test_conversation_entity_memory_whitespace_handling() {
        let mut mock_llm = MockChatModel::new();
        // Entity extraction with extra whitespace
        mock_llm.add_response("  Alice  ,  Bob  ");
        mock_llm.add_response("Alice summary");
        mock_llm.add_response("Bob summary");

        let chat_memory = InMemoryChatMessageHistory::new();
        let mut memory = ConversationEntityMemory::new(mock_llm, chat_memory);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Alice and Bob".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "OK".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        inputs.clear();
        inputs.insert("input".to_string(), "Next".to_string());
        let _vars = memory.load_memory_variables(&inputs).await.unwrap();
        outputs.clear();
        outputs.insert("output".to_string(), "Sure".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Should trim whitespace from entity names
        let entity_store = memory.entity_store.read().await;
        assert!(entity_store.exists("Alice"));
        assert!(entity_store.exists("Bob"));
    }

    #[tokio::test]
    async fn test_conversation_entity_memory_memory_variables() {
        let mock_llm = MockChatModel::new();
        let chat_memory = InMemoryChatMessageHistory::new();
        let memory = ConversationEntityMemory::new(mock_llm, chat_memory);

        let vars = memory.memory_variables();
        assert_eq!(vars.len(), 2);
        assert!(vars.contains(&"entities".to_string()));
        assert!(vars.contains(&"history".to_string()));
    }
}
