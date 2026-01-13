//! Language model abstractions for DashFlow
//!
//! This module provides the core traits and types for working with language models,
//! including chat models, LLMs, and embeddings.
//!
//! # Key Types
//!
//! - [`ChatModel`]: Trait for chat models that take messages as input
//! - [`ChatGeneration`]: Single chat generation output from a model
//! - [`ChatGenerationChunk`]: Streamable chunk of a chat generation
//! - [`ChatResult`]: Result containing one or more chat generations
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::core::language_models::ChatModel;
//! use dashflow::core::messages::HumanMessage;
//!
//! async fn example(model: &dyn ChatModel) -> Result<()> {
//!     let messages = vec![HumanMessage::new("Hello!").into()];
//!     let result = model.generate(&messages, None, None, None, None).await?;
//!     println!("Response: {}", result.generations[0].message.content());
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use futures::stream::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;

use crate::core::callbacks::CallbackManager;
use crate::core::error::{Error, Result};
use crate::core::messages::{AIMessage, AIMessageChunk, BaseMessage};

// ============================================================================
// Submodules
// ============================================================================

pub mod bind_tools;
pub mod builder;
pub mod context;
pub mod structured;
pub mod traced;

#[cfg(test)]
mod tests;

// Re-exports
pub use builder::{ChatModelBuildExt, GenerateBuilder, GenerateOptions};
pub use context::*;

// ============================================================================
// Tool Binding Types
// ============================================================================

/// Definition of a tool that can be called by the model.
///
/// This represents a tool (function) that the language model can choose to call
/// during generation. Tools enable models to interact with external systems,
/// perform calculations, search databases, and more.
///
/// # Example
///
/// ```rust
/// use dashflow::core::language_models::ToolDefinition;
/// use serde_json::json;
///
/// let tool = ToolDefinition {
///     name: "calculator".to_string(),
///     description: "Performs arithmetic calculations".to_string(),
///     parameters: json!({
///         "type": "object",
///         "properties": {
///             "expression": {
///                 "type": "string",
///                 "description": "Mathematical expression to evaluate"
///             }
///         },
///         "required": ["expression"]
///     }),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Name of the tool (must be unique within a request)
    pub name: String,

    /// Human-readable description of what the tool does
    pub description: String,

    /// JSON Schema describing the tool's parameters
    ///
    /// Should be a valid JSON Schema object with "type": "object"
    /// and properties describing the tool's inputs.
    pub parameters: serde_json::Value,
}

/// How the model should choose which tool(s) to call.
///
/// This enum controls whether and how the model selects tools to invoke
/// during generation. Different providers may have different levels of
/// support for each option.
///
/// # Examples
///
/// ```rust
/// use dashflow::core::language_models::ToolChoice;
///
/// // Let the model decide
/// let choice = ToolChoice::Auto;
///
/// // Force the model to call a specific tool
/// let choice = ToolChoice::Specific("search".to_string());
///
/// // Require at least one tool call
/// let choice = ToolChoice::Required;
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ToolChoice {
    /// Let the model decide whether to call a tool (default)
    ///
    /// The model can choose to call zero or more tools, or respond directly.
    #[default]
    Auto,

    /// Don't call any tools
    ///
    /// The model will not call any tools and will only generate text.
    None,

    /// Must call at least one tool
    ///
    /// Maps to "required" in `OpenAI` API, "any" in some other providers.
    /// Forces the model to call at least one of the available tools.
    Required,

    /// Call a specific tool by name
    ///
    /// Forces the model to call the specified tool. The tool name must
    /// match one of the provided tool definitions.
    Specific(String),
}

// ============================================================================
// Chat Model Types
// ============================================================================

/// A single chat generation output from a language model.
///
/// This represents one possible completion from the model. Models may return multiple
/// generations if `n > 1` is specified.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatGeneration {
    /// The message output by the chat model (usually an `AIMessage`)
    pub message: BaseMessage,

    /// Additional generation information (scores, token counts, etc.)
    pub generation_info: Option<HashMap<String, serde_json::Value>>,
}

impl ChatGeneration {
    /// Create a new `ChatGeneration` from a message
    #[must_use]
    pub fn new(message: BaseMessage) -> Self {
        Self {
            message,
            generation_info: None,
        }
    }

    /// Create a new `ChatGeneration` with generation info
    #[must_use]
    pub fn with_info(
        message: BaseMessage,
        generation_info: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            message,
            generation_info: Some(generation_info),
        }
    }

    /// Get the text content of the message
    #[must_use]
    pub fn text(&self) -> String {
        self.message.content().as_text()
    }
}

/// A streamable chunk of a chat generation.
///
/// These chunks can be concatenated together to form a complete message.
/// Used in streaming responses from chat models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatGenerationChunk {
    /// The message chunk (usually an `AIMessageChunk`)
    pub message: AIMessageChunk,

    /// Additional generation information for this chunk
    pub generation_info: Option<HashMap<String, serde_json::Value>>,
}

impl ChatGenerationChunk {
    /// Create a new `ChatGenerationChunk` from a message chunk
    #[must_use]
    pub fn new(message: AIMessageChunk) -> Self {
        Self {
            message,
            generation_info: None,
        }
    }

    /// Create a new `ChatGenerationChunk` with generation info
    #[must_use]
    pub fn with_info(
        message: AIMessageChunk,
        generation_info: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            message,
            generation_info: Some(generation_info),
        }
    }

    /// Merge two chunks together
    pub fn merge(&mut self, other: ChatGenerationChunk) {
        self.message = self.message.merge(other.message);

        // Merge generation_info if both exist
        if let Some(info) = &mut self.generation_info {
            if let Some(other_info) = other.generation_info {
                info.extend(other_info);
            }
        } else if other.generation_info.is_some() {
            self.generation_info = other.generation_info;
        }
    }
}

impl From<AIMessageChunk> for ChatGenerationChunk {
    fn from(message: AIMessageChunk) -> Self {
        Self::new(message)
    }
}

/// Result from a chat model call with a single prompt.
///
/// Contains one or more candidate generations. Multiple generations are produced
/// when `n > 1` is specified in the model parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResult {
    /// List of chat generations (candidates)
    pub generations: Vec<ChatGeneration>,

    /// Provider-specific output (usage stats, model version, etc.)
    pub llm_output: Option<HashMap<String, serde_json::Value>>,
}

impl ChatResult {
    /// Create a new `ChatResult` with a single generation
    #[must_use]
    pub fn new(generation: ChatGeneration) -> Self {
        Self {
            generations: vec![generation],
            llm_output: None,
        }
    }

    /// Create a new `ChatResult` with multiple generations
    #[must_use]
    pub fn with_generations(generations: Vec<ChatGeneration>) -> Self {
        Self {
            generations,
            llm_output: None,
        }
    }

    /// Create a new `ChatResult` with generations and LLM output
    #[must_use]
    pub fn with_llm_output(
        generations: Vec<ChatGeneration>,
        llm_output: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            generations,
            llm_output: Some(llm_output),
        }
    }

    /// Get the message from the first generation
    ///
    /// This is a convenience method for the common case of extracting the message
    /// from a single-generation response. Returns `None` if there are no generations.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use dashflow::core::language_models::{ChatResult, ChatGeneration};
    /// # use dashflow::core::messages::Message;
    /// let result = ChatResult::new(ChatGeneration {
    ///     message: Message::ai("Hello!"),
    ///     generation_info: None,
    /// });
    /// let message = result.message().unwrap();
    /// assert_eq!(message.as_text(), "Hello!");
    /// ```
    #[must_use]
    pub fn message(&self) -> Option<&crate::core::messages::Message> {
        self.generations.first().map(|gen| &gen.message)
    }

    /// Get a clone of the message from the first generation
    ///
    /// This is a convenience method that combines `message()` with `.cloned()`.
    /// Returns `None` if there are no generations.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use dashflow::core::language_models::{ChatResult, ChatGeneration};
    /// # use dashflow::core::messages::Message;
    /// let result = ChatResult::new(ChatGeneration {
    ///     message: Message::ai("Hello!"),
    ///     generation_info: None,
    /// });
    /// let message = result.message_cloned().unwrap();
    /// assert_eq!(message.as_text(), "Hello!");
    /// ```
    #[must_use]
    pub fn message_cloned(&self) -> Option<crate::core::messages::Message> {
        self.message().cloned()
    }
}

// ============================================================================
// LLM Types (Text Completion Models)
// ============================================================================

/// A single text generation output from an LLM (text completion model).
///
/// This is for text completion LLMs that take a string prompt and return a string,
/// as opposed to chat models that work with structured messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Generation {
    /// The generated text output
    pub text: String,

    /// Additional generation information (scores, token counts, finish reason, etc.)
    pub generation_info: Option<HashMap<String, serde_json::Value>>,
}

impl Generation {
    /// Create a new Generation from text
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            generation_info: None,
        }
    }

    /// Create a new Generation with generation info
    pub fn with_info(
        text: impl Into<String>,
        generation_info: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            text: text.into(),
            generation_info: Some(generation_info),
        }
    }
}

/// A streamable chunk of a text generation.
///
/// These chunks can be concatenated together to form a complete generation.
/// Used in streaming responses from LLMs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GenerationChunk {
    /// The text chunk
    pub text: String,

    /// Additional generation information for this chunk
    pub generation_info: Option<HashMap<String, serde_json::Value>>,
}

impl GenerationChunk {
    /// Create a new `GenerationChunk` from text
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            generation_info: None,
        }
    }

    /// Create a new `GenerationChunk` with generation info
    pub fn with_info(
        text: impl Into<String>,
        generation_info: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            text: text.into(),
            generation_info: Some(generation_info),
        }
    }

    /// Merge two chunks together (concatenate text)
    pub fn merge(&mut self, other: GenerationChunk) {
        self.text.push_str(&other.text);

        // Merge generation_info if both exist
        if let Some(info) = &mut self.generation_info {
            if let Some(other_info) = other.generation_info {
                info.extend(other_info);
            }
        } else if other.generation_info.is_some() {
            self.generation_info = other.generation_info;
        }
    }
}

/// Result from an LLM call containing multiple generations.
///
/// The outer vector represents different prompts, the inner vector represents
/// multiple candidate generations for each prompt (when n > 1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResult {
    /// Generated outputs - generations\[`prompt_idx`\]\[`candidate_idx`\]
    pub generations: Vec<Vec<Generation>>,

    /// Provider-specific output (usage stats, model version, etc.)
    pub llm_output: Option<HashMap<String, serde_json::Value>>,
}

impl LLMResult {
    /// Create a new `LLMResult` with a single generation
    #[must_use]
    pub fn new(generation: Generation) -> Self {
        Self {
            generations: vec![vec![generation]],
            llm_output: None,
        }
    }

    /// Create a new `LLMResult` with multiple generations for a single prompt
    #[must_use]
    pub fn with_generations(generations: Vec<Generation>) -> Self {
        Self {
            generations: vec![generations],
            llm_output: None,
        }
    }

    /// Create a new `LLMResult` for multiple prompts
    #[must_use]
    pub fn with_prompts(generations: Vec<Vec<Generation>>) -> Self {
        Self {
            generations,
            llm_output: None,
        }
    }

    /// Create a new `LLMResult` with LLM output metadata
    #[must_use]
    pub fn with_llm_output(
        generations: Vec<Vec<Generation>>,
        llm_output: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            generations,
            llm_output: Some(llm_output),
        }
    }
}

// ============================================================================
// LLM Trait (Text Completion Models)
// ============================================================================

/// Trait for text completion LLMs (string in, string out).
///
/// This trait is for text completion LLMs that work with plain text prompts
/// rather than structured messages. For chat models, use the `ChatModel` trait.
///
/// # Implementation Requirements
///
/// Implementors must provide:
/// - `_generate`: Core method to generate completions
/// - `llm_type`: Unique identifier for the model type
///
/// Optional methods for enhanced functionality:
/// - `_stream`: For streaming responses
///
/// # Example Implementation
///
/// ```rust,ignore
/// struct MyLLM { /* fields */ }
///
/// #[async_trait]
/// impl LLM for MyLLM {
///     async fn _generate(
///         &self,
///         prompts: &[String],
///         stop: Option<&[String]>,
///         run_manager: Option<&CallbackManager>,
///     ) -> Result<LLMResult> {
///         // Call API, return result
///         todo!()
///     }
///
///     fn llm_type(&self) -> &str {
///         "my_llm"
///     }
/// }
/// ```
#[async_trait]
pub trait LLM: Send + Sync {
    /// Generate text completions from prompts.
    ///
    /// This is the core generation method that must be implemented by all LLMs.
    ///
    /// # Arguments
    ///
    /// * `prompts` - Input prompts to generate from
    /// * `stop` - Optional stop sequences to halt generation
    /// * `run_manager` - Optional callback manager for this run
    ///
    /// # Returns
    ///
    /// An `LLMResult` containing generations for each prompt
    async fn _generate(
        &self,
        prompts: &[String],
        stop: Option<&[String]>,
        run_manager: Option<&CallbackManager>,
    ) -> Result<LLMResult>;

    /// Stream text generation chunks.
    ///
    /// Default implementation returns `NotImplemented` error. Override to enable streaming.
    ///
    /// # Arguments
    ///
    /// * `prompt` - Input prompt to generate from
    /// * `stop` - Optional stop sequences to halt generation
    /// * `run_manager` - Optional callback manager for this run
    ///
    /// # Returns
    ///
    /// An async stream of `GenerationChunk`s
    async fn _stream(
        &self,
        _prompt: &str,
        _stop: Option<&[String]>,
        _run_manager: Option<&CallbackManager>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<GenerationChunk>> + Send>>> {
        Err(Error::NotImplemented(
            "Streaming not implemented for this LLM".to_string(),
        ))
    }

    /// Get the type of language model
    fn llm_type(&self) -> &str;

    /// Public interface to generate completions from prompts
    ///
    /// This method wraps `_generate` and handles configuration and callbacks.
    async fn generate(
        &self,
        prompts: &[String],
        stop: Option<&[String]>,
        run_manager: Option<&CallbackManager>,
    ) -> Result<LLMResult> {
        self._generate(prompts, stop, run_manager).await
    }

    /// Public interface to stream completions from a prompt
    ///
    /// This method wraps `_stream` and handles configuration and callbacks.
    async fn stream(
        &self,
        prompt: &str,
        stop: Option<&[String]>,
        run_manager: Option<&CallbackManager>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<GenerationChunk>> + Send>>> {
        self._stream(prompt, stop, run_manager).await
    }
}

// ============================================================================
// Reinforcement Learning API
// ============================================================================

/// Example for reinforcement learning fine-tuning.
///
/// Represents a single training example with prompt, completion, and reward signal.
/// Used by `ChatModel::reinforce()` to perform RL-based fine-tuning.
///
/// # Example
///
/// ```rust
/// use dashflow::core::language_models::ReinforceExample;
/// use dashflow::core::messages::{HumanMessage, BaseMessage};
///
/// let example = ReinforceExample {
///     prompt: vec![HumanMessage::new("What is 2+2?").into()],
///     completion: "The answer is 4.".to_string(),
///     reward: 1.0,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReinforceExample {
    /// Input messages (prompt) for the training example
    pub prompt: Vec<BaseMessage>,

    /// Model's completion (output) for this prompt
    pub completion: String,

    /// Reward signal for this example (typically -1.0 to 1.0)
    ///
    /// Higher rewards indicate better completions that should be reinforced.
    /// Lower/negative rewards indicate poor completions to discourage.
    pub reward: f64,
}

/// Configuration for reinforcement learning fine-tuning.
///
/// Controls hyperparameters for RL-based model optimization.
///
/// # Example
///
/// ```rust
/// use dashflow::core::language_models::ReinforceConfig;
///
/// let config = ReinforceConfig {
///     learning_rate: 1e-5,
///     batch_size: 16,
///     num_epochs: 3,
///     max_steps: None,
///     warmup_steps: 100,
///     gradient_accumulation_steps: 1,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReinforceConfig {
    /// Learning rate for optimization (default: 1e-5)
    pub learning_rate: f64,

    /// Batch size for training (default: 16)
    pub batch_size: usize,

    /// Number of training epochs (default: 3)
    pub num_epochs: usize,

    /// Maximum training steps (overrides epochs if specified)
    pub max_steps: Option<usize>,

    /// Number of warmup steps for learning rate scheduler (default: 100)
    pub warmup_steps: usize,

    /// Number of gradient accumulation steps (default: 1)
    ///
    /// Effective batch size is `batch_size * gradient_accumulation_steps`
    pub gradient_accumulation_steps: usize,
}

impl Default for ReinforceConfig {
    fn default() -> Self {
        Self {
            learning_rate: 1e-5,
            batch_size: 16,
            num_epochs: 3,
            max_steps: None,
            warmup_steps: 100,
            gradient_accumulation_steps: 1,
        }
    }
}

/// Status of a reinforcement learning fine-tuning job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReinforceJobStatus {
    /// Job is queued and waiting to start
    Queued,

    /// Job is currently running
    Running,

    /// Job completed successfully
    Succeeded,

    /// Job failed with error
    Failed {
        /// The error message describing why the job failed.
        error: String,
    },

    /// Job was cancelled
    Cancelled,
}

/// Handle for a reinforcement learning fine-tuning job.
///
/// Represents an asynchronous fine-tuning job that may be running on a remote service.
/// Use `poll_status()` to check job progress or `wait_for_completion()` to block until done.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::language_models::{ChatModel, ReinforceExample, ReinforceConfig};
///
/// async fn train_model(model: &dyn ChatModel, examples: Vec<ReinforceExample>) {
///     let config = ReinforceConfig::default();
///     let job = model.reinforce(examples, config).await.unwrap();
///
///     println!("Job ID: {}", job.job_id);
///
///     // Poll for completion
///     loop {
///         let status = job.poll_status().await.unwrap();
///         match status {
///             ReinforceJobStatus::Succeeded => {
///                 println!("Training complete!");
///                 break;
///             }
///             ReinforceJobStatus::Failed { error } => {
///                 eprintln!("Training failed: {}", error);
///                 break;
///             }
///             _ => {
///                 tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
///             }
///         }
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ReinforceJob {
    /// Unique identifier for this training job
    pub job_id: String,

    /// Initial status when job was created
    pub status: ReinforceJobStatus,

    /// Provider-specific metadata (e.g., fine-tune ID, model name)
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ReinforceJob {
    /// Create a new reinforcement learning job.
    ///
    /// # Arguments
    ///
    /// * `job_id` - Unique identifier for this job
    /// * `status` - Initial job status
    pub fn new(job_id: String, status: ReinforceJobStatus) -> Self {
        Self {
            job_id,
            status,
            metadata: HashMap::new(),
        }
    }

    /// Create a new job with metadata.
    pub fn with_metadata(
        job_id: String,
        status: ReinforceJobStatus,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            job_id,
            status,
            metadata,
        }
    }
}

// ============================================================================
// ChatModel Trait
// ============================================================================

/// Trait for chat models that generate responses to messages.
///
/// Chat models take a list of messages as input and produce `AIMessage` responses.
/// They support streaming, tool calling, and structured outputs.
///
/// # Implementation Requirements
///
/// Implementors must provide:
/// - `_generate`: Core method to generate a response
/// - `llm_type`: Unique identifier for the model type
///
/// Optional methods for enhanced functionality:
/// - `_stream`: For streaming responses
/// - `_agenerate`: For native async generation (defaults to sync in executor)
/// - `_astream`: For native async streaming (defaults to sync in executor)
///
/// # Example Implementation
///
/// ```rust,ignore
/// struct MyChat { /* fields */ }
///
/// #[async_trait]
/// impl ChatModel for MyChat {
///     async fn _generate(
///         &self,
///         messages: &[BaseMessage],
///         stop: Option<&[String]>,
///         run_manager: Option<&CallbackManager>,
///     ) -> Result<ChatResult> {
///         // Call API, return result
///         todo!()
///     }
///
///     fn llm_type(&self) -> &str {
///         "my_chat_model"
///     }
/// }
/// ```
#[async_trait]
pub trait ChatModel: Send + Sync {
    /// Generate a chat result from messages.
    ///
    /// **IMPORTANT: Application code should use [`dashflow::generate()`] instead of calling
    /// this method directly.** Direct calls bypass DashFlow's graph infrastructure and miss:
    /// - ExecutionTrace collection for optimizers
    /// - Streaming events for live progress
    /// - Introspection capabilities
    /// - Metrics collection (tokens, cost, latency)
    /// - A/B testing support
    ///
    /// This method is intended for trait implementors. Applications should use:
    /// ```rust,ignore
    /// use dashflow::generate;
    /// let result = generate(model, &messages).await?;
    /// ```
    ///
    /// # Arguments
    ///
    /// * `messages` - Input messages to generate from
    /// * `stop` - Optional stop sequences to halt generation
    /// * `tools` - Optional tool definitions for function calling
    /// * `tool_choice` - Optional specification of which tool(s) to call
    /// * `run_manager` - Optional callback manager for this run
    ///
    /// # Returns
    ///
    /// A `ChatResult` containing one or more generations
    ///
    /// # Tool Calling
    ///
    /// If `tools` are provided and the model supports tool calling, the model
    /// may return a response with tool calls in the `AIMessage`'s `tool_calls` field.
    /// The `tool_choice` parameter controls whether and which tools can be called.
    ///
    /// Not all models support tool calling. Models that don't support tools should
    /// ignore the `tools` and `tool_choice` parameters or return an error.
    ///
    /// # Internal Method
    ///
    /// This method is prefixed with `_` to indicate it's an internal implementation detail.
    /// Application code should use `dashflow::generate()` instead of calling this directly.
    #[doc(hidden)]
    async fn _generate(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        run_manager: Option<&CallbackManager>,
    ) -> Result<ChatResult>;

    /// Stream chat generation chunks.
    ///
    /// Default implementation returns `NotImplemented` error. Override to enable streaming.
    ///
    /// # Arguments
    ///
    /// * `messages` - Input messages to generate from
    /// * `stop` - Optional stop sequences to halt generation
    /// * `tools` - Optional tool definitions for function calling
    /// * `tool_choice` - Optional specification of which tool(s) to call
    /// * `run_manager` - Optional callback manager for this run
    ///
    /// # Returns
    ///
    /// An async stream of `ChatGenerationChunk`s
    ///
    /// # Internal Method
    ///
    /// This method is prefixed with `_` to indicate it's an internal implementation detail.
    /// Application code should use `dashflow::stream()` instead of calling this directly.
    #[doc(hidden)]
    async fn _stream(
        &self,
        _messages: &[BaseMessage],
        _stop: Option<&[String]>,
        _tools: Option<&[ToolDefinition]>,
        _tool_choice: Option<&ToolChoice>,
        _run_manager: Option<&CallbackManager>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatGenerationChunk>> + Send>>> {
        Err(Error::NotImplemented(
            "Streaming not implemented for this model".to_string(),
        ))
    }

    /// Get the type of language model.
    ///
    /// Used for logging and identification.
    fn llm_type(&self) -> &str;

    /// Get the model name for this chat model.
    ///
    /// Returns the specific model identifier (e.g., "gpt-4o", "claude-3-5-sonnet")
    /// used for context limit lookups and token counting. Override this method
    /// to enable automatic context limit validation.
    ///
    /// # Default Implementation
    ///
    /// Returns `None`, which disables context limit validation unless an explicit
    /// limit is provided via configuration.
    fn model_name(&self) -> Option<&str> {
        None
    }

    /// Get the context limit policy for this model.
    ///
    /// Controls how context limit violations are handled during `generate()` and
    /// `stream()` calls. Override to change the default behavior.
    ///
    /// # Default Implementation
    ///
    /// Returns `ContextLimitPolicy::None` (no validation) for backwards compatibility.
    fn context_limit_policy(&self) -> ContextLimitPolicy {
        ContextLimitPolicy::None
    }

    /// Get the number of tokens to reserve for the response.
    ///
    /// When validating context limits, this many tokens are reserved for the
    /// model's response, reducing the available budget for input messages.
    ///
    /// # Default Implementation
    ///
    /// Returns 4096, a conservative default that works for most models.
    fn reserve_tokens(&self) -> usize {
        4096
    }

    /// Get identifying parameters for the model.
    ///
    /// Used for tracing and caching. Returns model configuration that uniquely
    /// identifies this model instance.
    fn identifying_params(&self) -> HashMap<String, serde_json::Value> {
        HashMap::new()
    }

    /// Get the rate limiter for this model, if any.
    ///
    /// Rate limiters control the rate at which requests are made to the model's API.
    /// Override this method to provide rate limiting for your model.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use dashflow::core::rate_limiters::{InMemoryRateLimiter, RateLimiter};
    /// use std::sync::Arc;
    ///
    /// fn rate_limiter(&self) -> Option<Arc<dyn RateLimiter>> {
    ///     self.rate_limiter.clone()
    /// }
    /// ```
    fn rate_limiter(&self) -> Option<std::sync::Arc<dyn crate::core::rate_limiters::RateLimiter>> {
        None
    }

    /// Downcast to Any for type-specific operations.
    ///
    /// This method enables downcasting `Arc<dyn ChatModel>` to concrete types
    /// when needed for type-specific functionality.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let model: Arc<dyn ChatModel> = Arc::new(ChatOpenAI::with_config(Default::default()));
    /// if let Some(openai_model) = model.as_any().downcast_ref::<ChatOpenAI>() {
    ///     // Use ChatOpenAI-specific methods
    /// }
    /// ```
    fn as_any(&self) -> &dyn std::any::Any;

    /// Generate a response from messages (public API).
    ///
    /// This is the main entry point for generating chat completions.
    /// If a rate limiter is configured, this method will wait for permission before proceeding.
    ///
    /// # Arguments
    ///
    /// * `messages` - Input messages to generate from
    /// * `stop` - Optional stop sequences to halt generation
    /// * `tools` - Optional tool definitions for function calling
    /// * `tool_choice` - Optional specification of which tool(s) to call
    /// * `config` - Optional configuration including callbacks for tracing
    ///
    /// # Tool Calling
    ///
    /// Models that support tool calling (`OpenAI`, Anthropic, etc.) can accept tool
    /// definitions and return responses with tool calls. Check the model's documentation
    /// for specific tool calling capabilities.
    async fn generate(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        config: Option<&crate::core::config::RunnableConfig>,
    ) -> Result<ChatResult> {
        // Validate context limits before making the request
        let policy = self.context_limit_policy();
        if policy != ContextLimitPolicy::None {
            validate_context_limit(
                messages,
                self.model_name(),
                None, // Use model's default limit
                self.reserve_tokens(),
                policy,
            )?;
        }

        // Acquire rate limiter permission if configured
        if let Some(limiter) = self.rate_limiter() {
            limiter.acquire().await;
        }

        // Extract callbacks from config
        let callbacks = config.and_then(|c| c.callbacks.as_ref());

        self._generate(messages, stop, tools, tool_choice, callbacks)
            .await
    }

    /// Stream a response from messages (public API).
    ///
    /// Returns a stream of message chunks that can be concatenated.
    /// If a rate limiter is configured, this method will wait for permission before proceeding.
    ///
    /// # Arguments
    ///
    /// * `messages` - Input messages to generate from
    /// * `stop` - Optional stop sequences to halt generation
    /// * `tools` - Optional tool definitions for function calling
    /// * `tool_choice` - Optional specification of which tool(s) to call
    /// * `config` - Optional configuration including callbacks for tracing
    async fn stream(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        config: Option<&crate::core::config::RunnableConfig>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatGenerationChunk>> + Send>>> {
        // Validate context limits before making the request
        let policy = self.context_limit_policy();
        if policy != ContextLimitPolicy::None {
            validate_context_limit(
                messages,
                self.model_name(),
                None, // Use model's default limit
                self.reserve_tokens(),
                policy,
            )?;
        }

        // Acquire rate limiter permission if configured
        if let Some(limiter) = self.rate_limiter() {
            limiter.acquire().await;
        }

        // Extract callbacks from config
        let callbacks = config.and_then(|c| c.callbacks.as_ref());

        self._stream(messages, stop, tools, tool_choice, callbacks)
            .await
    }

    /// Perform reinforcement learning fine-tuning on this model.
    ///
    /// This method submits training examples with reward signals to fine-tune
    /// the model using reinforcement learning. The implementation is provider-specific:
    ///
    /// - **OpenAI**: May use fine-tuning API with reward-weighted examples
    /// - **Local models**: May integrate with training frameworks (e.g., trl, RLHF)
    /// - **Cloud services**: May submit to provider's RL training service
    ///
    /// # Arguments
    ///
    /// * `examples` - Training examples with prompts, completions, and reward signals
    /// * `config` - Hyperparameters for RL training (learning rate, batch size, etc.)
    ///
    /// # Returns
    ///
    /// A `ReinforceJob` handle for tracking training progress. Use `poll_status()`
    /// to check job status or implement polling logic to wait for completion.
    ///
    /// # Errors
    ///
    /// Returns `Error::NotImplemented` if the model doesn't support RL fine-tuning.
    /// Returns provider-specific errors for training failures.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::core::language_models::{ChatModel, ReinforceExample, ReinforceConfig};
    /// use dashflow::core::messages::HumanMessage;
    ///
    /// async fn train_with_rl(model: &dyn ChatModel) -> Result<()> {
    ///     let examples = vec![
    ///         ReinforceExample {
    ///             prompt: vec![HumanMessage::new("Solve: 2+2").into()],
    ///             completion: "4".to_string(),
    ///             reward: 1.0,  // Correct answer
    ///         },
    ///         ReinforceExample {
    ///             prompt: vec![HumanMessage::new("Solve: 3+3").into()],
    ///             completion: "7".to_string(),
    ///             reward: -1.0,  // Incorrect answer
    ///         },
    ///     ];
    ///
    ///     let config = ReinforceConfig::default();
    ///     let job = model.reinforce(examples, config).await?;
    ///
    ///     println!("Training job submitted: {}", job.job_id);
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Provider Implementation Notes
    ///
    /// Implementors should:
    /// 1. Validate that examples have valid reward signals
    /// 2. Submit training job to provider's API or training service
    /// 3. Return job handle with unique ID and initial status
    /// 4. Store provider-specific metadata in job.metadata
    ///
    /// Default implementation returns `NotImplemented` error.
    async fn reinforce(
        &self,
        _examples: Vec<ReinforceExample>,
        _config: ReinforceConfig,
    ) -> Result<ReinforceJob> {
        Err(Error::NotImplemented(
            "Reinforcement learning fine-tuning not implemented for this model".to_string(),
        ))
    }
}

// ============================================================================
// Arc<dyn ChatModel> Implementation
// ============================================================================

/// Implementation of `ChatModel` for `Arc<dyn ChatModel>`.
///
/// This allows using `Arc<dyn ChatModel>` anywhere a `ChatModel` is expected,
/// enabling provider-agnostic code to use features like `bind_tools()`.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::language_models::{ChatModel, ChatModelToolBindingExt};
/// use std::sync::Arc;
///
/// let model: Arc<dyn ChatModel> = create_llm(...).await?;
/// let model_with_tools = model.bind_tools(tools, None);
/// ```
#[async_trait]
impl ChatModel for std::sync::Arc<dyn ChatModel> {
    async fn _generate(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        run_manager: Option<&CallbackManager>,
    ) -> Result<ChatResult> {
        (**self)
            ._generate(messages, stop, tools, tool_choice, run_manager)
            .await
    }

    async fn _stream(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        run_manager: Option<&CallbackManager>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatGenerationChunk>> + Send>>> {
        (**self)
            ._stream(messages, stop, tools, tool_choice, run_manager)
            .await
    }

    fn llm_type(&self) -> &str {
        (**self).llm_type()
    }

    fn model_name(&self) -> Option<&str> {
        (**self).model_name()
    }

    fn context_limit_policy(&self) -> ContextLimitPolicy {
        (**self).context_limit_policy()
    }

    fn reserve_tokens(&self) -> usize {
        (**self).reserve_tokens()
    }

    fn identifying_params(&self) -> HashMap<String, serde_json::Value> {
        (**self).identifying_params()
    }

    fn rate_limiter(&self) -> Option<std::sync::Arc<dyn crate::core::rate_limiters::RateLimiter>> {
        (**self).rate_limiter()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn reinforce(
        &self,
        examples: Vec<ReinforceExample>,
        config: ReinforceConfig,
    ) -> Result<ReinforceJob> {
        (**self).reinforce(examples, config).await
    }
}

/// Fake chat model for testing.
///
/// Returns predefined responses and optionally streams them as chunks.
#[derive(Debug, Clone)]
pub struct FakeChatModel {
    /// Responses to return for each generate call
    pub responses: Vec<String>,

    /// Current response index
    response_index: std::sync::Arc<std::sync::Mutex<usize>>,

    /// Whether to support streaming
    pub supports_streaming: bool,
}

impl FakeChatModel {
    /// Create a new `FakeChatModel` with given responses
    #[must_use]
    pub fn new(responses: Vec<String>) -> Self {
        Self {
            responses,
            response_index: std::sync::Arc::new(std::sync::Mutex::new(0)),
            supports_streaming: false,
        }
    }

    /// Create a `FakeChatModel` that supports streaming
    #[must_use]
    pub fn with_streaming(mut self) -> Self {
        self.supports_streaming = true;
        self
    }

    fn get_next_response(&self) -> String {
        let mut idx = self
            .response_index
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let response = self
            .responses
            .get(*idx)
            .cloned()
            .unwrap_or_else(|| "Default response".to_string());
        *idx = (*idx + 1) % self.responses.len().max(1);
        response
    }
}

#[async_trait]
impl ChatModel for FakeChatModel {
    async fn _generate(
        &self,
        _messages: &[BaseMessage],
        _stop: Option<&[String]>,
        _tools: Option<&[ToolDefinition]>,
        _tool_choice: Option<&ToolChoice>,
        _run_manager: Option<&CallbackManager>,
    ) -> Result<ChatResult> {
        let response = self.get_next_response();
        let message = AIMessage::new(response);
        Ok(ChatResult::new(ChatGeneration::new(message.into())))
    }

    async fn _stream(
        &self,
        _messages: &[BaseMessage],
        _stop: Option<&[String]>,
        _tools: Option<&[ToolDefinition]>,
        _tool_choice: Option<&ToolChoice>,
        _run_manager: Option<&CallbackManager>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatGenerationChunk>> + Send>>> {
        if !self.supports_streaming {
            return Err(Error::NotImplemented(
                "Streaming not enabled for this FakeChatModel".to_string(),
            ));
        }

        let response = self.get_next_response();
        // Split response into chunks (by words for simplicity)
        let chunks: Vec<String> = response
            .split_whitespace()
            .map(|s| format!("{s} "))
            .collect();

        Ok(Box::pin(futures::stream::iter(chunks.into_iter().map(
            |chunk| Ok(ChatGenerationChunk::new(AIMessageChunk::new(chunk))),
        ))))
    }

    fn llm_type(&self) -> &'static str {
        "fake_chat_model"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ============================================================================
// Fake LLM for Testing
// ============================================================================

/// Fake LLM for testing text completion functionality.
///
/// Returns predefined responses and optionally streams them as chunks.
#[derive(Debug, Clone)]
pub struct FakeLLM {
    /// Responses to return for each generate call
    pub responses: Vec<String>,

    /// Current response index
    response_index: std::sync::Arc<std::sync::Mutex<usize>>,

    /// Whether to support streaming
    pub supports_streaming: bool,
}

impl FakeLLM {
    /// Create a new `FakeLLM` with given responses
    #[must_use]
    pub fn new(responses: Vec<String>) -> Self {
        Self {
            responses,
            response_index: std::sync::Arc::new(std::sync::Mutex::new(0)),
            supports_streaming: false,
        }
    }

    /// Create a `FakeLLM` that supports streaming
    #[must_use]
    pub fn with_streaming(mut self) -> Self {
        self.supports_streaming = true;
        self
    }

    fn get_next_response(&self) -> String {
        let mut idx = self
            .response_index
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let response = self
            .responses
            .get(*idx)
            .cloned()
            .unwrap_or_else(|| "Default LLM response".to_string());
        *idx = (*idx + 1) % self.responses.len().max(1);
        response
    }
}

#[async_trait]
impl LLM for FakeLLM {
    async fn _generate(
        &self,
        prompts: &[String],
        _stop: Option<&[String]>,
        _run_manager: Option<&CallbackManager>,
    ) -> Result<LLMResult> {
        let mut all_generations = Vec::new();

        for _prompt in prompts {
            let response = self.get_next_response();
            all_generations.push(vec![Generation::new(response)]);
        }

        Ok(LLMResult::with_prompts(all_generations))
    }

    async fn _stream(
        &self,
        _prompt: &str,
        _stop: Option<&[String]>,
        _run_manager: Option<&CallbackManager>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<GenerationChunk>> + Send>>> {
        if !self.supports_streaming {
            return Err(Error::NotImplemented(
                "Streaming not enabled for this FakeLLM".to_string(),
            ));
        }

        let response = self.get_next_response();
        // Split response into chunks (by words for simplicity)
        let chunks: Vec<String> = response
            .split_whitespace()
            .map(|s| format!("{s} "))
            .collect();

        Ok(Box::pin(futures::stream::iter(
            chunks
                .into_iter()
                .map(|chunk| Ok(GenerationChunk::new(chunk))),
        )))
    }

    fn llm_type(&self) -> &'static str {
        "fake_llm"
    }
}

// ============================================================================
// ChatModel to LLM Adapter
// ============================================================================

/// Adapter that converts a `ChatModel` to an LLM.
///
/// This adapter allows using any `ChatModel` implementation in contexts that
/// require an LLM trait. It converts string prompts to `HumanMessage` and
/// extracts text content from the chat model's response.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::language_models::{ChatModelToLLM, LLM};
/// use dashflow_openai::ChatOpenAI;
///
/// let chat_model = ChatOpenAI::default();
/// let llm = ChatModelToLLM::new(chat_model);
///
/// // Now use it as an LLM
/// let result = llm.generate(&["Hello!".to_string()], None, None).await?;
/// ```
pub struct ChatModelToLLM<C: ChatModel> {
    chat_model: C,
}

impl<C: ChatModel> ChatModelToLLM<C> {
    /// Create a new adapter from a `ChatModel`.
    pub fn new(chat_model: C) -> Self {
        Self { chat_model }
    }

    /// Get a reference to the underlying `ChatModel`.
    pub fn inner(&self) -> &C {
        &self.chat_model
    }

    /// Get a mutable reference to the underlying `ChatModel`.
    pub fn inner_mut(&mut self) -> &mut C {
        &mut self.chat_model
    }

    /// Consume the adapter and return the underlying `ChatModel`.
    #[must_use]
    pub fn into_inner(self) -> C {
        self.chat_model
    }
}

#[async_trait]
impl<C: ChatModel> LLM for ChatModelToLLM<C> {
    async fn _generate(
        &self,
        prompts: &[String],
        stop: Option<&[String]>,
        run_manager: Option<&CallbackManager>,
    ) -> Result<LLMResult> {
        // Convert each prompt string to a message
        let mut all_generations = Vec::new();

        for prompt in prompts {
            let messages = vec![BaseMessage::human(prompt.clone())];

            // Create config with callbacks if provided
            let config = run_manager.map(|rm| crate::core::config::RunnableConfig {
                callbacks: Some(rm.clone()),
                ..Default::default()
            });

            let chat_result = self
                .chat_model
                .generate(&messages, stop, None, None, config.as_ref())
                .await?;

            // Extract text from first generation's message using text() method
            let generation = if let Some(chat_gen) = chat_result.generations.first() {
                Generation {
                    text: chat_gen.text(),
                    generation_info: chat_gen.generation_info.clone(),
                }
            } else {
                Generation::new(String::new())
            };

            all_generations.push(vec![generation]);
        }

        Ok(LLMResult {
            generations: all_generations,
            llm_output: None,
        })
    }

    async fn _stream(
        &self,
        prompt: &str,
        stop: Option<&[String]>,
        run_manager: Option<&CallbackManager>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<GenerationChunk>> + Send>>> {
        let messages = vec![BaseMessage::human(prompt.to_string())];

        // Create config with callbacks if provided
        let config = run_manager.map(|rm| crate::core::config::RunnableConfig {
            callbacks: Some(rm.clone()),
            ..Default::default()
        });

        let chat_stream = self
            .chat_model
            .stream(&messages, stop, None, None, config.as_ref())
            .await?;

        // Convert ChatGenerationChunk stream to GenerationChunk stream
        let generation_stream = chat_stream.map(|result| {
            result.map(|chat_chunk| GenerationChunk {
                text: chat_chunk.message.content,
                generation_info: chat_chunk.generation_info,
            })
        });

        Ok(Box::pin(generation_stream))
    }

    fn llm_type(&self) -> &str {
        self.chat_model.llm_type()
    }
}
