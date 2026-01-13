//! Conversation Chain - Basic conversation with memory
//!
//! This module provides a chain for maintaining conversations with context from
//! memory. The chain combines a language model with memory to maintain conversation
//! state across multiple turns.
//!
//! # Python Baseline Compatibility
//!
//! Based on `dashflow_classic.chains.conversation.base:14-150` (`ConversationChain`)
//! from Python `DashFlow`.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_chains::ConversationChain;
//! use dashflow::core::chat_history::InMemoryChatMessageHistory;
//! use dashflow_memory::ConversationBufferMemory;
//! use std::sync::Arc;
//!
//! // Create conversation chain with default memory
//! let llm = Arc::new(/* your LLM */);
//! let chat_history = InMemoryChatMessageHistory::new();
//! let memory = ConversationBufferMemory::new(chat_history);
//! let chain = ConversationChain::new(llm, memory);
//!
//! // Run conversation
//! let response = chain.run("Hello, my name is Alice!").await?;
//! // Second turn will remember the first
//! let response2 = chain.run("What is my name?").await?;
//! ```

use dashflow::core::error::Result;
use dashflow::core::language_models::{ChatModel, LLM};
use dashflow::core::prompts::PromptTemplate;
use dashflow_memory::BaseMemory;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Default conversation prompt template.
///
/// This is a friendly conversation prompt that includes the conversation history.
/// The AI is instructed to be talkative and honest about what it doesn't know.
///
/// Input variables: `history` (from memory) and `input` (from user)
const DEFAULT_TEMPLATE: &str = r"The following is a friendly conversation between a human and an AI. The AI is talkative and provides lots of specific details from its context. If the AI does not know the answer to a question, it truthfully says it does not know.

Current conversation:
{history}
Human: {input}
AI:";

/// Chain that carries on a conversation and loads context from memory.
///
/// This chain combines a language model with memory to maintain conversation
/// context across multiple turns. It's the simplest way to build a chatbot
/// with memory in `DashFlow`.
///
/// # Architecture
///
/// 1. User provides input
/// 2. Memory loads conversation history
/// 3. Prompt template formats history + input
/// 4. LLM generates response
/// 5. Memory saves input + response for next turn
///
/// # Default Configuration
///
/// - **Prompt**: Friendly conversation prompt with {history} and {input}
/// - **Memory**: `ConversationBufferMemory` (stores full history)
/// - **Input Key**: "input"
/// - **Output Key**: "response"
///
/// # Python Baseline Compatibility
///
/// Matches `ConversationChain` from `dashflow_classic.chains.conversation.base:14-150`.
///
/// Key differences:
/// - Rust uses `Arc<RwLock<_>>` for thread-safe mutable memory
/// - Rust doesn't support inheritance, so `ConversationChain` contains an LLM
///   rather than extending `LLMChain`
/// - Rust uses Result types for error handling
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_chains::ConversationChain;
/// use dashflow::core::chat_history::InMemoryChatMessageHistory;
/// use dashflow_memory::ConversationBufferMemory;
/// use dashflow_openai::OpenAI;
/// use std::sync::Arc;
///
/// // Create chain with default settings
/// let llm = Arc::new(OpenAI::default());
/// let chat_history = InMemoryChatMessageHistory::new();
/// let memory = ConversationBufferMemory::new(chat_history);
/// let chain = ConversationChain::new(llm, memory);
///
/// // First turn
/// let response1 = chain.run("Hi, I'm Alice!").await?;
/// println!("{}", response1);
///
/// // Second turn - chain remembers previous context
/// let response2 = chain.run("What's my name?").await?;
/// println!("{}", response2); // Should reference "Alice"
/// ```
pub struct ConversationChain<M, Mem: BaseMemory> {
    /// The language model to use for generation
    model: Arc<M>,

    /// Memory for storing conversation history
    memory: Arc<RwLock<Mem>>,

    /// Prompt template (default: friendly conversation prompt)
    prompt: PromptTemplate,

    /// Input key name (default: "input")
    input_key: String,

    /// Output key name (default: "response")
    output_key: String,
}

impl<M, Mem: BaseMemory> ConversationChain<M, Mem> {
    /// Create a new conversation chain with a model and memory.
    ///
    /// Uses default prompt, `input_key="input`", and `output_key="response`".
    ///
    /// # Arguments
    ///
    /// * `model` - Language model to use for generation
    /// * `memory` - Memory for storing conversation history
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow_chains::ConversationChain;
    /// use dashflow_memory::ConversationBufferMemory;
    /// use dashflow::core::chat_history::InMemoryChatMessageHistory;
    ///
    /// let llm = Arc::new(/* your LLM */);
    /// let chat_history = InMemoryChatMessageHistory::new();
    /// let memory = ConversationBufferMemory::new(chat_history);
    /// let chain = ConversationChain::new(llm, memory).await?;
    /// ```
    pub async fn new(model: Arc<M>, memory: Mem) -> Result<Self> {
        let prompt = PromptTemplate::from_template(DEFAULT_TEMPLATE)?;
        let input_key = "input".to_string();
        let output_key = "response".to_string();
        let memory = Arc::new(RwLock::new(memory));

        // Validate that memory keys don't overlap with input key
        let memory_vars = memory.read().await.memory_variables();
        if memory_vars.contains(&input_key) {
            return Err(dashflow::core::error::Error::Other(format!(
                "The input key {input_key} was also found in the memory keys ({memory_vars:?}) - \
                 please provide keys that don't overlap."
            )));
        }

        Ok(Self {
            model,
            memory,
            prompt,
            input_key,
            output_key,
        })
    }

    /// Create a conversation chain with a custom prompt template.
    ///
    /// The prompt must have variables that match `memory_variables` + `input_key`.
    ///
    /// # Arguments
    ///
    /// * `model` - Language model to use
    /// * `memory` - Memory for conversation history
    /// * `prompt` - Custom prompt template
    ///
    /// # Errors
    ///
    /// Returns error if prompt variables don't match memory variables + input key.
    pub async fn with_prompt(model: Arc<M>, memory: Mem, prompt: PromptTemplate) -> Result<Self> {
        let input_key = "input".to_string();
        let output_key = "response".to_string();
        let memory = Arc::new(RwLock::new(memory));

        // Validate prompt variables match memory + input
        let memory_vars = memory.read().await.memory_variables();
        if memory_vars.contains(&input_key) {
            return Err(dashflow::core::error::Error::Other(format!(
                "The input key {input_key} was also found in the memory keys ({memory_vars:?}) - \
                 please provide keys that don't overlap."
            )));
        }

        let mut expected_vars = memory_vars.clone();
        expected_vars.push(input_key.clone());
        expected_vars.sort();

        let mut prompt_vars = prompt.input_variables.clone();
        prompt_vars.sort();

        if expected_vars != prompt_vars {
            return Err(dashflow::core::error::Error::Other(format!(
                "Got unexpected prompt input variables. The prompt expects {prompt_vars:?}, \
                 but got {memory_vars:?} as inputs from memory, and {input_key} as the normal input key."
            )));
        }

        Ok(Self {
            model,
            memory,
            prompt,
            input_key,
            output_key,
        })
    }

    /// Get the memory reference.
    #[must_use]
    pub fn memory(&self) -> &Arc<RwLock<Mem>> {
        &self.memory
    }

    /// Get the prompt template.
    #[must_use]
    pub fn prompt(&self) -> &PromptTemplate {
        &self.prompt
    }

    /// Get the model reference.
    #[must_use]
    pub fn model(&self) -> &M {
        &self.model
    }

    /// Clear the conversation memory.
    pub async fn clear(&self) -> Result<()> {
        self.memory
            .write()
            .await
            .clear()
            .await
            .map_err(|e| dashflow::core::error::Error::Other(e.to_string()))
    }
}

impl<M: LLM, Mem: BaseMemory> ConversationChain<M, Mem> {
    /// Run the conversation chain with a single input string.
    ///
    /// This is the simplest way to use the chain - just provide the user's
    /// input as a string. The chain will:
    /// 1. Load conversation history from memory
    /// 2. Format the prompt with history + input
    /// 3. Call the LLM
    /// 4. Save the turn to memory
    /// 5. Return the response
    ///
    /// # Arguments
    ///
    /// * `input` - User's input message
    ///
    /// # Returns
    ///
    /// The AI's response as a string
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let response = chain.run("Hello!").await?;
    /// println!("{}", response);
    /// ```
    pub async fn run(&self, input: &str) -> Result<String> {
        // Load memory variables
        let mut inputs = HashMap::new();
        inputs.insert(self.input_key.clone(), input.to_string());

        let memory_vars = self
            .memory
            .read()
            .await
            .load_memory_variables(&inputs)
            .await
            .map_err(|e| dashflow::core::error::Error::Other(e.to_string()))?;

        // Combine memory vars with input
        let mut all_inputs = memory_vars.clone();
        all_inputs.insert(self.input_key.clone(), input.to_string());

        // Format prompt
        let formatted = self.prompt.format(&all_inputs)?;

        // Call LLM
        let result = self.model.generate(&[formatted], None, None).await?;

        // Extract response
        let response = result
            .generations
            .first()
            .and_then(|g| g.first())
            .map(|gen| gen.text.clone())
            .ok_or_else(|| {
                dashflow::core::error::Error::Other("No generation returned from LLM".to_string())
            })?;

        // Save context to memory
        let mut outputs = HashMap::new();
        outputs.insert(self.output_key.clone(), response.clone());

        self.memory
            .write()
            .await
            .save_context(&inputs, &outputs)
            .await
            .map_err(|e| dashflow::core::error::Error::Other(e.to_string()))?;

        Ok(response)
    }

    /// Run the chain with explicit input/output keys.
    ///
    /// This provides more control over the input/output structure.
    ///
    /// # Arguments
    ///
    /// * `inputs` - Input key-value pairs (must contain `input_key`)
    ///
    /// # Returns
    ///
    /// Output key-value pairs (contains `output_key`)
    pub async fn invoke(
        &self,
        inputs: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>> {
        // Get input
        let input = inputs
            .get(&self.input_key)
            .ok_or_else(|| {
                dashflow::core::error::Error::Other(format!(
                    "Input key '{}' not found in inputs",
                    self.input_key
                ))
            })?
            .clone();

        // Load memory variables
        let memory_vars = self
            .memory
            .read()
            .await
            .load_memory_variables(inputs)
            .await
            .map_err(|e| dashflow::core::error::Error::Other(e.to_string()))?;

        // Combine memory vars with input
        let mut all_inputs = memory_vars.clone();
        all_inputs.insert(self.input_key.clone(), input.clone());

        // Format prompt
        let formatted = self.prompt.format(&all_inputs)?;

        // Call LLM
        let result = self.model.generate(&[formatted], None, None).await?;

        // Extract response
        let response = result
            .generations
            .first()
            .and_then(|g| g.first())
            .map(|gen| gen.text.clone())
            .ok_or_else(|| {
                dashflow::core::error::Error::Other("No generation returned from LLM".to_string())
            })?;

        // Save context to memory
        let mut outputs = HashMap::new();
        outputs.insert(self.output_key.clone(), response.clone());

        self.memory
            .write()
            .await
            .save_context(inputs, &outputs)
            .await
            .map_err(|e| dashflow::core::error::Error::Other(e.to_string()))?;

        Ok(outputs)
    }
}

impl<M: ChatModel, Mem: BaseMemory> ConversationChain<M, Mem> {
    /// Run the conversation chain with a chat model.
    ///
    /// Similar to `run()` but uses a chat model instead of an LLM.
    /// The prompt is formatted and sent as a human message.
    ///
    /// # Arguments
    ///
    /// * `input` - User's input message
    ///
    /// # Returns
    ///
    /// The AI's response as a string
    pub async fn run_chat(&self, input: &str) -> Result<String> {
        // Load memory variables
        let mut inputs = HashMap::new();
        inputs.insert(self.input_key.clone(), input.to_string());

        let memory_vars = self
            .memory
            .read()
            .await
            .load_memory_variables(&inputs)
            .await
            .map_err(|e| dashflow::core::error::Error::Other(e.to_string()))?;

        // Combine memory vars with input
        let mut all_inputs = memory_vars.clone();
        all_inputs.insert(self.input_key.clone(), input.to_string());

        // Format prompt
        let formatted = self.prompt.format(&all_inputs)?;

        // Convert to chat message
        use dashflow::core::messages::HumanMessage;
        let messages = vec![HumanMessage::new(formatted).into()];

        // Call chat model
        let result = self
            .model
            .generate(&messages, None, None, None, None)
            .await?;

        // Extract response
        let response = result
            .generations
            .first()
            .map(dashflow::core::language_models::ChatGeneration::text)
            .unwrap_or_default();

        // Save context to memory
        let mut outputs = HashMap::new();
        outputs.insert(self.output_key.clone(), response.clone());

        self.memory
            .write()
            .await
            .save_context(&inputs, &outputs)
            .await
            .map_err(|e| dashflow::core::error::Error::Other(e.to_string()))?;

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use dashflow::core::chat_history::InMemoryChatMessageHistory;
    use dashflow::core::language_models::{ChatGeneration, ChatResult, Generation, LLMResult};
    use dashflow::core::language_models::{ToolChoice, ToolDefinition};
    use dashflow::core::messages::{AIMessage, BaseMessage};
    use dashflow_memory::ConversationBufferMemory;
    use std::collections::HashMap;

    // Mock LLM for testing
    struct MockLLM;

    #[async_trait]
    impl LLM for MockLLM {
        async fn _generate(
            &self,
            prompts: &[String],
            _stop: Option<&[String]>,
            _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
        ) -> Result<LLMResult> {
            let generations: Vec<Vec<Generation>> = prompts
                .iter()
                .map(|_prompt| vec![Generation::new("Mock response".to_string())])
                .collect();

            Ok(LLMResult::with_prompts(generations))
        }

        fn llm_type(&self) -> &str {
            "mock"
        }
    }

    // Mock ChatModel for testing
    struct MockChatModel;

    #[async_trait]
    impl ChatModel for MockChatModel {
        async fn _generate(
            &self,
            messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
        ) -> Result<ChatResult> {
            let content = messages
                .iter()
                .map(|m| m.content().as_text())
                .collect::<Vec<_>>()
                .join(" ");

            let generation = ChatGeneration::new(
                AIMessage::new(format!("Mock response to: {}", content)).into(),
            );

            Ok(ChatResult::new(generation))
        }

        fn llm_type(&self) -> &str {
            "mock"
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[tokio::test]
    async fn test_conversation_chain_basic() {
        let llm = Arc::new(MockLLM);
        let chat_history = InMemoryChatMessageHistory::new();
        let memory = ConversationBufferMemory::new(chat_history);
        let chain = ConversationChain::new(llm, memory).await.unwrap();

        let response = chain.run("Hello!").await.unwrap();
        assert_eq!(response, "Mock response");

        // Check that memory was saved
        let memory_vars = chain
            .memory()
            .read()
            .await
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();

        let history = memory_vars.get("history").unwrap();
        assert!(history.contains("Hello!"));
        assert!(history.contains("Mock response"));
    }

    #[tokio::test]
    async fn test_conversation_chain_multiple_turns() {
        let llm = Arc::new(MockLLM);
        let chat_history = InMemoryChatMessageHistory::new();
        let memory = ConversationBufferMemory::new(chat_history);
        let chain = ConversationChain::new(llm, memory).await.unwrap();

        // First turn
        let _response1 = chain.run("My name is Alice").await.unwrap();

        // Second turn
        let _response2 = chain.run("What is my name?").await.unwrap();

        // Check that both turns are in memory
        let memory_vars = chain
            .memory()
            .read()
            .await
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();

        let history = memory_vars.get("history").unwrap();
        assert!(history.contains("Alice"));
        assert!(history.contains("What is my name"));
    }

    #[tokio::test]
    async fn test_conversation_chain_clear() {
        let llm = Arc::new(MockLLM);
        let chat_history = InMemoryChatMessageHistory::new();
        let memory = ConversationBufferMemory::new(chat_history);
        let chain = ConversationChain::new(llm, memory).await.unwrap();

        // Add some conversation
        let _response = chain.run("Hello!").await.unwrap();

        // Clear memory
        chain.clear().await.unwrap();

        // Check that memory is empty
        let memory_vars = chain
            .memory()
            .read()
            .await
            .load_memory_variables(&HashMap::new())
            .await
            .unwrap();

        let history = memory_vars.get("history").unwrap();
        assert!(history.is_empty());
    }

    #[tokio::test]
    async fn test_conversation_chain_custom_prompt() {
        let llm = Arc::new(MockLLM);
        let chat_history = InMemoryChatMessageHistory::new();
        let memory = ConversationBufferMemory::new(chat_history);
        let prompt =
            PromptTemplate::from_template("History: {history}\nUser: {input}\nBot:").unwrap();

        let chain = ConversationChain::with_prompt(llm, memory, prompt)
            .await
            .unwrap();

        let response = chain.run("Test").await.unwrap();
        assert_eq!(response, "Mock response");
    }

    #[tokio::test]
    async fn test_conversation_chain_validation_overlap() {
        let llm = Arc::new(MockLLM);
        let chat_history = InMemoryChatMessageHistory::new();

        // Create memory with "input" as memory key (should conflict)
        let memory = ConversationBufferMemory::new(chat_history).with_memory_key("input");

        let result = ConversationChain::new(llm, memory).await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("was also found in the memory keys"));
        }
    }

    #[tokio::test]
    async fn test_conversation_chain_validation_prompt_mismatch() {
        let llm = Arc::new(MockLLM);
        let chat_history = InMemoryChatMessageHistory::new();
        let memory = ConversationBufferMemory::new(chat_history);

        // Prompt with wrong variables
        let prompt = PromptTemplate::from_template("Wrong: {wrong}").unwrap();

        let result = ConversationChain::with_prompt(llm, memory, prompt).await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("unexpected prompt input variables"));
        }
    }

    #[tokio::test]
    async fn test_conversation_chain_invoke() {
        let llm = Arc::new(MockLLM);
        let chat_history = InMemoryChatMessageHistory::new();
        let memory = ConversationBufferMemory::new(chat_history);
        let chain = ConversationChain::new(llm, memory).await.unwrap();

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello!".to_string());

        let outputs = chain.invoke(&inputs).await.unwrap();
        assert_eq!(outputs.get("response").unwrap(), "Mock response");
    }

    #[tokio::test]
    async fn test_conversation_chain_chat_model() {
        let model = Arc::new(MockChatModel);
        let chat_history = InMemoryChatMessageHistory::new();
        let memory = ConversationBufferMemory::new(chat_history);
        let chain = ConversationChain::new(model, memory).await.unwrap();

        let response = chain.run_chat("Hello!").await.unwrap();
        assert!(response.contains("Mock response"));
    }
}
