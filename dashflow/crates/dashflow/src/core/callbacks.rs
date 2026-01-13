// Allow clippy warnings for callback manager
// - clone_on_ref_ptr: CallbackManager clones Arc<dyn CallbackHandler> for parallel execution
#![allow(clippy::clone_on_ref_ptr)]

//! Callback system for observability and debugging
//!
//! This module provides the callback infrastructure for tracking execution
//! of DashFlow components. Callbacks are used for logging, observability,
//! debugging, and integration with tracing systems like `LangSmith`.
//!
//! # Overview
//!
//! - [`CallbackHandler`] - Trait for implementing custom callbacks
//! - [`CallbackManager`] - Manages multiple callback handlers
//! - [`ConsoleCallbackHandler`] - Logs events to console
//! - [`FileCallbackHandler`] - Logs events to file
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::core::callbacks::{CallbackManager, ConsoleCallbackHandler};
//! use dashflow::core::language_models::ChatModel;
//! use dashflow::core::messages::Message;
//! use dashflow_openai::ChatOpenAI;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create callback manager with console handler
//! let console_handler = ConsoleCallbackHandler::new();
//! let callbacks = CallbackManager::new()
//!     .with_handler(console_handler);
//!
//! // Use with chat model
//! let chat = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");
//! let messages = vec![Message::human("Hello!")];
//!
//! // Callbacks will log start/end events
//! let result = chat.generate(&messages, Some(&callbacks)).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # `LangSmith` Integration
//!
//! For production observability, use [`DashFlowTracer`](crate::core::tracers::DashFlowTracer):
//!
//! ```rust,ignore
//! use dashflow::core::callbacks::CallbackManager;
//! use dashflow::core::tracers::DashFlowTracer;
//!
//! let tracer = DashFlowTracer::new("my-project")?;
//! let callbacks = CallbackManager::new().with_handler(tracer);
//!
//! // All operations will be traced to LangSmith
//! let result = chat.generate(&messages, Some(&callbacks)).await?;
//! ```

use async_trait::async_trait;
use std::collections::HashMap;
use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::core::config::RunnableConfig;
use crate::core::error::Result;
use crate::core::messages::Message;

/// Callback event types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallbackEvent {
    /// Chain started
    ChainStart,
    /// Chain ended
    ChainEnd,
    /// Chain error
    ChainError,
    /// LLM started
    LlmStart,
    /// LLM ended
    LlmEnd,
    /// LLM error
    LlmError,
    /// LLM new token
    LlmNewToken,
    /// Chat model started
    ChatModelStart,
    /// Tool started
    ToolStart,
    /// Tool ended
    ToolEnd,
    /// Tool error
    ToolError,
    /// Retriever started
    RetrieverStart,
    /// Retriever ended
    RetrieverEnd,
    /// Retriever error
    RetrieverError,
    /// Text output
    Text,
    /// Retry event
    Retry,
    /// Custom event
    Custom(String),
}

/// Callback handler trait.
///
/// Implement this trait to create custom callback handlers that can observe
/// execution of DashFlow components.
#[async_trait]
pub trait CallbackHandler: Send + Sync {
    /// Called when a chain starts running.
    async fn on_chain_start(
        &self,
        serialized: &HashMap<String, serde_json::Value>,
        inputs: &HashMap<String, serde_json::Value>,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        tags: &[String],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let _ = (serialized, inputs, run_id, parent_run_id, tags, metadata);
        Ok(())
    }

    /// Called when a chain ends running.
    async fn on_chain_end(
        &self,
        outputs: &HashMap<String, serde_json::Value>,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        let _ = (outputs, run_id, parent_run_id);
        Ok(())
    }

    /// Called when a chain errors.
    async fn on_chain_error(
        &self,
        error: &str,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        let _ = (error, run_id, parent_run_id);
        Ok(())
    }

    /// Called when an LLM starts running.
    async fn on_llm_start(
        &self,
        serialized: &HashMap<String, serde_json::Value>,
        prompts: &[String],
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        tags: &[String],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let _ = (serialized, prompts, run_id, parent_run_id, tags, metadata);
        Ok(())
    }

    /// Called when a chat model starts running.
    async fn on_chat_model_start(
        &self,
        serialized: &HashMap<String, serde_json::Value>,
        messages: &[Vec<Message>],
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        tags: &[String],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        // Default: fall back to on_llm_start with empty prompts
        let _ = (serialized, messages, run_id, parent_run_id, tags, metadata);
        self.on_llm_start(serialized, &[], run_id, parent_run_id, tags, metadata)
            .await
    }

    /// Called when an LLM generates a new token (streaming).
    async fn on_llm_new_token(
        &self,
        token: &str,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        let _ = (token, run_id, parent_run_id);
        Ok(())
    }

    /// Called when an LLM ends running.
    async fn on_llm_end(
        &self,
        response: &HashMap<String, serde_json::Value>,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        let _ = (response, run_id, parent_run_id);
        Ok(())
    }

    /// Called when an LLM errors.
    async fn on_llm_error(
        &self,
        error: &str,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        let _ = (error, run_id, parent_run_id);
        Ok(())
    }

    /// Called when a tool starts running.
    async fn on_tool_start(
        &self,
        serialized: &HashMap<String, serde_json::Value>,
        input_str: &str,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        tags: &[String],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let _ = (serialized, input_str, run_id, parent_run_id, tags, metadata);
        Ok(())
    }

    /// Called when a tool ends running.
    async fn on_tool_end(
        &self,
        output: &str,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        let _ = (output, run_id, parent_run_id);
        Ok(())
    }

    /// Called when a tool errors.
    async fn on_tool_error(
        &self,
        error: &str,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        let _ = (error, run_id, parent_run_id);
        Ok(())
    }

    /// Called when a retriever starts running.
    async fn on_retriever_start(
        &self,
        serialized: &HashMap<String, serde_json::Value>,
        query: &str,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        tags: &[String],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let _ = (serialized, query, run_id, parent_run_id, tags, metadata);
        Ok(())
    }

    /// Called when a retriever ends running.
    async fn on_retriever_end(
        &self,
        documents: &[serde_json::Value],
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        let _ = (documents, run_id, parent_run_id);
        Ok(())
    }

    /// Called when a retriever errors.
    async fn on_retriever_error(
        &self,
        error: &str,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        let _ = (error, run_id, parent_run_id);
        Ok(())
    }

    /// Called on arbitrary text.
    async fn on_text(&self, text: &str, run_id: Uuid, parent_run_id: Option<Uuid>) -> Result<()> {
        let _ = (text, run_id, parent_run_id);
        Ok(())
    }

    /// Called on a retry event.
    async fn on_retry(&self, run_id: Uuid, parent_run_id: Option<Uuid>) -> Result<()> {
        let _ = (run_id, parent_run_id);
        Ok(())
    }

    /// Called on a custom event.
    async fn on_custom_event(
        &self,
        name: &str,
        data: &serde_json::Value,
        run_id: Uuid,
        tags: &[String],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let _ = (name, data, run_id, tags, metadata);
        Ok(())
    }

    /// Whether to ignore LLM callbacks.
    fn ignore_llm(&self) -> bool {
        false
    }

    /// Whether to ignore chain callbacks.
    fn ignore_chain(&self) -> bool {
        false
    }

    /// Whether to ignore tool callbacks.
    fn ignore_tool(&self) -> bool {
        false
    }

    /// Whether to ignore retriever callbacks.
    fn ignore_retriever(&self) -> bool {
        false
    }

    /// Whether to ignore chat model callbacks.
    fn ignore_chat_model(&self) -> bool {
        false
    }

    /// Whether to ignore retry callbacks.
    fn ignore_retry(&self) -> bool {
        false
    }

    /// Whether to ignore custom events.
    fn ignore_custom_event(&self) -> bool {
        false
    }

    /// Whether to raise errors or continue on callback errors.
    fn raise_error(&self) -> bool {
        false
    }
}

/// Null callback handler that does nothing.
///
/// This is useful for disabling callbacks without removing callback support.
#[derive(Debug, Clone, Default)]
pub struct NullCallbackHandler;

#[async_trait]
impl CallbackHandler for NullCallbackHandler {
    // All methods use default implementations (no-ops)
}

/// Console callback handler that prints to stdout.
///
/// This handler prints execution events to the console, useful for debugging
/// and monitoring chain execution.
#[derive(Debug, Clone)]
pub struct ConsoleCallbackHandler {
    /// Whether to use colored output (ANSI codes).
    colored: bool,
}

impl ConsoleCallbackHandler {
    /// Create a new console callback handler.
    #[must_use]
    pub const fn new(colored: bool) -> Self {
        Self { colored }
    }

    fn print(&self, msg: &str) {
        println!("{msg}");
    }

    fn print_with_color(&self, msg: &str, bold: bool) {
        if self.colored && bold {
            println!("\x1b[1m{msg}\x1b[0m");
        } else {
            println!("{msg}");
        }
    }
}

impl Default for ConsoleCallbackHandler {
    fn default() -> Self {
        Self::new(true)
    }
}

#[async_trait]
impl CallbackHandler for ConsoleCallbackHandler {
    async fn on_chain_start(
        &self,
        serialized: &HashMap<String, serde_json::Value>,
        _inputs: &HashMap<String, serde_json::Value>,
        _run_id: Uuid,
        _parent_run_id: Option<Uuid>,
        _tags: &[String],
        _metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let name = serialized
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown>");
        self.print_with_color(&format!("\n> Entering new {name} chain..."), true);
        Ok(())
    }

    async fn on_chain_end(
        &self,
        _outputs: &HashMap<String, serde_json::Value>,
        _run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.print_with_color("\n> Finished chain.", true);
        Ok(())
    }

    async fn on_chain_error(
        &self,
        error: &str,
        _run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.print(&format!("\n> Chain error: {error}"));
        Ok(())
    }

    async fn on_llm_new_token(
        &self,
        token: &str,
        _run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        let token = token.to_string();
        tokio::task::spawn_blocking(move || {
            print!("{token}");
            std::io::stdout().flush().ok();
        })
        .await
        .map_err(|e| std::io::Error::other(format!("stdout write task panicked: {e}")))?;
        Ok(())
    }

    async fn on_text(&self, text: &str, _run_id: Uuid, _parent_run_id: Option<Uuid>) -> Result<()> {
        self.print(text);
        Ok(())
    }

    async fn on_tool_end(
        &self,
        output: &str,
        _run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.print(&format!("Tool output: {output}"));
        Ok(())
    }
}

/// File callback handler that writes to a file.
///
/// This handler writes execution events to a file, useful for logging
/// and audit trails.
#[derive(Debug, Clone)]
pub struct FileCallbackHandler {
    /// File handle (wrapped in `Arc<Mutex>` for thread-safety).
    file: Arc<Mutex<Option<std::fs::File>>>,
}

impl FileCallbackHandler {
    /// Create a new file callback handler.
    ///
    /// # Arguments
    ///
    /// * `filepath` - Path to the file to write to
    /// * `append` - If true, append to existing file; if false, truncate
    pub fn new(filepath: impl Into<PathBuf>, append: bool) -> Result<Self> {
        let filepath = filepath.into();
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(&filepath)?;

        Ok(Self {
            file: Arc::new(Mutex::new(Some(file))),
        })
    }

    async fn write(&self, msg: &str) -> Result<()> {
        let msg = msg.to_string();

        let mut file_guard = self.file.lock().await;
        let Some(file) = file_guard.take() else {
            return Ok(());
        };
        drop(file_guard);

        let (file, io_result) = tokio::task::spawn_blocking(move || {
            let mut file = file;
            let result = (|| -> std::io::Result<()> {
                writeln!(file, "{msg}")?;
                file.flush()?;
                Ok(())
            })();
            (file, result)
        })
        .await
        .map_err(|e| std::io::Error::other(format!("file write task panicked: {e}")))?;

        let mut file_guard = self.file.lock().await;
        *file_guard = Some(file);
        io_result.map_err(crate::core::error::Error::Io)?;
        Ok(())
    }

    /// Close the file.
    pub async fn close(&self) {
        let mut file_guard = self.file.lock().await;
        *file_guard = None;
    }
}

#[async_trait]
impl CallbackHandler for FileCallbackHandler {
    async fn on_chain_start(
        &self,
        serialized: &HashMap<String, serde_json::Value>,
        _inputs: &HashMap<String, serde_json::Value>,
        _run_id: Uuid,
        _parent_run_id: Option<Uuid>,
        _tags: &[String],
        _metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let name = serialized
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown>");
        self.write(&format!("\n> Entering new {name} chain..."))
            .await
    }

    async fn on_chain_end(
        &self,
        _outputs: &HashMap<String, serde_json::Value>,
        _run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.write("\n> Finished chain.").await
    }

    async fn on_chain_error(
        &self,
        error: &str,
        _run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.write(&format!("\n> Chain error: {error}")).await
    }

    async fn on_llm_new_token(
        &self,
        token: &str,
        _run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.write(token).await
    }

    async fn on_text(&self, text: &str, _run_id: Uuid, _parent_run_id: Option<Uuid>) -> Result<()> {
        self.write(text).await
    }

    async fn on_tool_end(
        &self,
        output: &str,
        _run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.write(&format!("Tool output: {output}")).await
    }
}

/// Callback manager that coordinates multiple callback handlers.
///
/// The manager executes callbacks in order, handling errors according to
/// the `raise_error` setting of each handler.
#[derive(Clone)]
pub struct CallbackManager {
    handlers: Vec<Arc<dyn CallbackHandler>>,
}

impl CallbackManager {
    /// Create a new callback manager with no handlers.
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    /// Create a callback manager with the given handlers.
    #[must_use]
    pub fn with_handlers(handlers: Vec<Arc<dyn CallbackHandler>>) -> Self {
        Self { handlers }
    }

    /// Add a callback handler to the manager.
    pub fn add_handler(&mut self, handler: Arc<dyn CallbackHandler>) {
        self.handlers.push(handler);
    }

    /// Get the number of handlers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    /// Check if there are no handlers.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }

    /// Execute a callback on all handlers.
    ///
    /// If a handler has `raise_error()` set to true and returns an error,
    /// the error is propagated. Otherwise, errors are logged but execution
    /// continues.
    async fn execute<F, Fut>(&self, f: F) -> Result<()>
    where
        F: Fn(Arc<dyn CallbackHandler>) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        for handler in &self.handlers {
            let result = f(handler.clone()).await;
            if let Err(e) = result {
                if handler.raise_error() {
                    return Err(e);
                }
                // Log error but continue
                tracing::warn!(error = %e, "Callback error (ignored)");
            }
        }
        Ok(())
    }

    /// Called when a chain starts running.
    pub async fn on_chain_start(
        &self,
        serialized: &HashMap<String, serde_json::Value>,
        inputs: &HashMap<String, serde_json::Value>,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        tags: &[String],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let serialized = serialized.clone();
        let inputs = inputs.clone();
        let tags = tags.to_vec();
        let metadata = metadata.clone();

        self.execute(move |handler| {
            let serialized = serialized.clone();
            let inputs = inputs.clone();
            let tags = tags.clone();
            let metadata = metadata.clone();
            async move {
                if handler.ignore_chain() {
                    Ok(())
                } else {
                    handler
                        .on_chain_start(
                            &serialized,
                            &inputs,
                            run_id,
                            parent_run_id,
                            &tags,
                            &metadata,
                        )
                        .await
                }
            }
        })
        .await
    }

    /// Called when a chain ends running.
    pub async fn on_chain_end(
        &self,
        outputs: &HashMap<String, serde_json::Value>,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        let outputs = outputs.clone();

        self.execute(move |handler| {
            let outputs = outputs.clone();
            async move {
                if handler.ignore_chain() {
                    Ok(())
                } else {
                    handler.on_chain_end(&outputs, run_id, parent_run_id).await
                }
            }
        })
        .await
    }

    /// Called when a chain errors.
    pub async fn on_chain_error(
        &self,
        error: &str,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        let error = error.to_string();

        self.execute(move |handler| {
            let error = error.clone();
            async move {
                if handler.ignore_chain() {
                    Ok(())
                } else {
                    handler.on_chain_error(&error, run_id, parent_run_id).await
                }
            }
        })
        .await
    }

    /// Called when an LLM starts running.
    pub async fn on_llm_start(
        &self,
        serialized: &HashMap<String, serde_json::Value>,
        prompts: &[String],
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        tags: &[String],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let serialized = serialized.clone();
        let prompts = prompts.to_vec();
        let tags = tags.to_vec();
        let metadata = metadata.clone();

        self.execute(move |handler| {
            let serialized = serialized.clone();
            let prompts = prompts.clone();
            let tags = tags.clone();
            let metadata = metadata.clone();
            async move {
                if handler.ignore_llm() {
                    Ok(())
                } else {
                    handler
                        .on_llm_start(
                            &serialized,
                            &prompts,
                            run_id,
                            parent_run_id,
                            &tags,
                            &metadata,
                        )
                        .await
                }
            }
        })
        .await
    }

    /// Called when a chat model starts running.
    pub async fn on_chat_model_start(
        &self,
        serialized: &HashMap<String, serde_json::Value>,
        messages: &[Vec<Message>],
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        tags: &[String],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let serialized = serialized.clone();
        let messages = messages.to_vec();
        let tags = tags.to_vec();
        let metadata = metadata.clone();

        self.execute(move |handler| {
            let serialized = serialized.clone();
            let messages = messages.clone();
            let tags = tags.clone();
            let metadata = metadata.clone();
            async move {
                if !handler.ignore_chat_model() && !handler.ignore_llm() {
                    handler
                        .on_chat_model_start(
                            &serialized,
                            &messages,
                            run_id,
                            parent_run_id,
                            &tags,
                            &metadata,
                        )
                        .await
                } else {
                    Ok(())
                }
            }
        })
        .await
    }

    /// Called when an LLM generates a new token.
    pub async fn on_llm_new_token(
        &self,
        token: &str,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        let token = token.to_string();

        self.execute(move |handler| {
            let token = token.clone();
            async move {
                if handler.ignore_llm() {
                    Ok(())
                } else {
                    handler
                        .on_llm_new_token(&token, run_id, parent_run_id)
                        .await
                }
            }
        })
        .await
    }

    /// Called when an LLM ends running.
    pub async fn on_llm_end(
        &self,
        response: &HashMap<String, serde_json::Value>,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        let response = response.clone();

        self.execute(move |handler| {
            let response = response.clone();
            async move {
                if handler.ignore_llm() {
                    Ok(())
                } else {
                    handler.on_llm_end(&response, run_id, parent_run_id).await
                }
            }
        })
        .await
    }

    /// Called when an LLM errors.
    pub async fn on_llm_error(
        &self,
        error: &str,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        let error = error.to_string();

        self.execute(move |handler| {
            let error = error.clone();
            async move {
                if handler.ignore_llm() {
                    Ok(())
                } else {
                    handler.on_llm_error(&error, run_id, parent_run_id).await
                }
            }
        })
        .await
    }

    /// Called when arbitrary text is output.
    pub async fn on_text(
        &self,
        text: &str,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        let text = text.to_string();

        self.execute(move |handler| {
            let text = text.clone();
            async move { handler.on_text(&text, run_id, parent_run_id).await }
        })
        .await
    }

    /// Called on a retry event.
    pub async fn on_retry(&self, run_id: Uuid, parent_run_id: Option<Uuid>) -> Result<()> {
        self.execute(move |handler| async move {
            if handler.ignore_retry() {
                Ok(())
            } else {
                handler.on_retry(run_id, parent_run_id).await
            }
        })
        .await
    }
}

impl Default for CallbackManager {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for CallbackManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CallbackManager")
            .field("handlers_count", &self.handlers.len())
            .finish()
    }
}

/// Execution context that combines `RunnableConfig` with callbacks.
///
/// Since callbacks contain trait objects and cannot be serialized,
/// they are kept separate from `RunnableConfig`. This struct provides
/// a convenient way to pass both together during execution.
#[derive(Clone)]
pub struct ExecutionContext {
    /// The configuration for this execution.
    pub config: RunnableConfig,
    /// The callback manager (optional).
    pub callbacks: Option<CallbackManager>,
}

impl ExecutionContext {
    /// Create a new execution context with the given config.
    #[must_use]
    pub const fn new(config: RunnableConfig) -> Self {
        Self {
            config,
            callbacks: None,
        }
    }

    /// Create a new execution context with config and callbacks.
    #[must_use]
    pub const fn with_callbacks(config: RunnableConfig, callbacks: CallbackManager) -> Self {
        Self {
            config,
            callbacks: Some(callbacks),
        }
    }

    /// Add a callback handler to this context.
    #[must_use]
    pub fn add_handler(mut self, handler: Arc<dyn CallbackHandler>) -> Self {
        if let Some(ref mut callbacks) = self.callbacks {
            callbacks.add_handler(handler);
        } else {
            self.callbacks = Some(CallbackManager::with_handlers(vec![handler]));
        }
        self
    }

    /// Get the run ID, generating one if needed.
    pub fn ensure_run_id(&mut self) -> Uuid {
        self.config.ensure_run_id()
    }

    /// Get the run ID if it exists.
    #[must_use]
    pub const fn run_id(&self) -> Option<Uuid> {
        self.config.run_id
    }

    /// Get a reference to the callbacks.
    #[must_use]
    pub const fn callbacks(&self) -> Option<&CallbackManager> {
        self.callbacks.as_ref()
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new(RunnableConfig::default())
    }
}

impl From<RunnableConfig> for ExecutionContext {
    fn from(config: RunnableConfig) -> Self {
        Self::new(config)
    }
}

impl fmt::Debug for ExecutionContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExecutionContext")
            .field("config", &self.config)
            .field("has_callbacks", &self.callbacks.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{ExecutionContext, FileCallbackHandler};
    use crate::test_prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::Mutex;

    #[derive(Debug)]
    struct CountingHandler {
        chain_starts: Arc<AtomicUsize>,
        chain_ends: Arc<AtomicUsize>,
        chain_errors: Arc<AtomicUsize>,
    }

    impl CountingHandler {
        fn new() -> Self {
            Self {
                chain_starts: Arc::new(AtomicUsize::new(0)),
                chain_ends: Arc::new(AtomicUsize::new(0)),
                chain_errors: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn get_counts(&self) -> (usize, usize, usize) {
            (
                self.chain_starts.load(Ordering::SeqCst),
                self.chain_ends.load(Ordering::SeqCst),
                self.chain_errors.load(Ordering::SeqCst),
            )
        }
    }

    #[async_trait]
    impl CallbackHandler for CountingHandler {
        async fn on_chain_start(
            &self,
            _serialized: &HashMap<String, serde_json::Value>,
            _inputs: &HashMap<String, serde_json::Value>,
            _run_id: Uuid,
            _parent_run_id: Option<Uuid>,
            _tags: &[String],
            _metadata: &HashMap<String, serde_json::Value>,
        ) -> Result<()> {
            self.chain_starts.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn on_chain_end(
            &self,
            _outputs: &HashMap<String, serde_json::Value>,
            _run_id: Uuid,
            _parent_run_id: Option<Uuid>,
        ) -> Result<()> {
            self.chain_ends.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn on_chain_error(
            &self,
            _error: &str,
            _run_id: Uuid,
            _parent_run_id: Option<Uuid>,
        ) -> Result<()> {
            self.chain_errors.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_null_handler() {
        let handler = NullCallbackHandler;
        let run_id = Uuid::new_v4();

        // Should not panic or error
        handler
            .on_chain_start(
                &HashMap::new(),
                &HashMap::new(),
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();
        handler
            .on_chain_end(&HashMap::new(), run_id, None)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_console_handler() {
        let handler = ConsoleCallbackHandler::new(false);
        let run_id = Uuid::new_v4();

        let mut serialized = HashMap::new();
        serialized.insert(
            "name".to_string(),
            serde_json::Value::String("test_chain".to_string()),
        );

        // Should print but not error
        handler
            .on_chain_start(
                &serialized,
                &HashMap::new(),
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();
        handler
            .on_chain_end(&HashMap::new(), run_id, None)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_callback_manager() {
        let handler1 = Arc::new(CountingHandler::new());
        let handler2 = Arc::new(CountingHandler::new());

        let manager = CallbackManager::with_handlers(vec![
            handler1.clone() as Arc<dyn CallbackHandler>,
            handler2.clone() as Arc<dyn CallbackHandler>,
        ]);

        let run_id = Uuid::new_v4();

        manager
            .on_chain_start(
                &HashMap::new(),
                &HashMap::new(),
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();

        manager
            .on_chain_end(&HashMap::new(), run_id, None)
            .await
            .unwrap();

        manager
            .on_chain_error("test error", run_id, None)
            .await
            .unwrap();

        // Both handlers should have received all callbacks
        assert_eq!(handler1.get_counts(), (1, 1, 1));
        assert_eq!(handler2.get_counts(), (1, 1, 1));
    }

    #[tokio::test]
    async fn test_callback_ordering() {
        use std::sync::Mutex;

        #[derive(Debug)]
        struct OrderTracker {
            order: Arc<Mutex<Vec<String>>>,
            name: String,
        }

        #[async_trait]
        impl CallbackHandler for OrderTracker {
            async fn on_chain_start(
                &self,
                _serialized: &HashMap<String, serde_json::Value>,
                _inputs: &HashMap<String, serde_json::Value>,
                _run_id: Uuid,
                _parent_run_id: Option<Uuid>,
                _tags: &[String],
                _metadata: &HashMap<String, serde_json::Value>,
            ) -> Result<()> {
                self.order
                    .lock()
                    .unwrap()
                    .push(format!("{}_start", self.name));
                Ok(())
            }
        }

        let order = Arc::new(Mutex::new(Vec::new()));
        let h1 = Arc::new(OrderTracker {
            order: order.clone(),
            name: "h1".to_string(),
        });
        let h2 = Arc::new(OrderTracker {
            order: order.clone(),
            name: "h2".to_string(),
        });
        let h3 = Arc::new(OrderTracker {
            order: order.clone(),
            name: "h3".to_string(),
        });

        let manager = CallbackManager::with_handlers(vec![
            h1 as Arc<dyn CallbackHandler>,
            h2 as Arc<dyn CallbackHandler>,
            h3 as Arc<dyn CallbackHandler>,
        ]);

        manager
            .on_chain_start(
                &HashMap::new(),
                &HashMap::new(),
                Uuid::new_v4(),
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();

        let final_order = order.lock().unwrap();
        assert_eq!(*final_order, vec!["h1_start", "h2_start", "h3_start"]);
    }

    #[tokio::test]
    async fn test_file_handler() {
        use std::fs;
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        let handler = FileCallbackHandler::new(&path, false).unwrap();
        let run_id = Uuid::new_v4();

        let mut serialized = HashMap::new();
        serialized.insert(
            "name".to_string(),
            serde_json::Value::String("test_chain".to_string()),
        );

        handler
            .on_chain_start(
                &serialized,
                &HashMap::new(),
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();
        handler.on_text("test message", run_id, None).await.unwrap();
        handler
            .on_chain_end(&HashMap::new(), run_id, None)
            .await
            .unwrap();

        handler.close().await;

        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("test_chain"));
        assert!(contents.contains("test message"));
        assert!(contents.contains("Finished chain"));
    }

    #[tokio::test]
    async fn test_execution_context() {
        let config = RunnableConfig::new()
            .with_tag("test")
            .with_run_name("test_run");

        let mut ctx = ExecutionContext::new(config.clone());
        assert!(ctx.callbacks.is_none());
        assert_eq!(ctx.config.tags, vec!["test"]);

        // Add a handler
        let handler = Arc::new(NullCallbackHandler) as Arc<dyn CallbackHandler>;
        ctx = ctx.add_handler(handler);
        assert!(ctx.callbacks.is_some());
        assert_eq!(ctx.callbacks.as_ref().unwrap().len(), 1);

        // Ensure run ID
        let run_id = ctx.ensure_run_id();
        assert_eq!(ctx.run_id(), Some(run_id));
    }

    #[tokio::test]
    async fn test_execution_context_with_callbacks() {
        let config = RunnableConfig::new();
        let handler = Arc::new(ConsoleCallbackHandler::default()) as Arc<dyn CallbackHandler>;
        let manager = CallbackManager::with_handlers(vec![handler]);

        let ctx = ExecutionContext::with_callbacks(config, manager);
        assert!(ctx.callbacks.is_some());
        assert_eq!(ctx.callbacks().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_error_propagation() {
        #[derive(Debug)]
        struct ErrorHandler {
            should_error: bool,
            should_raise: bool,
        }

        #[async_trait]
        impl CallbackHandler for ErrorHandler {
            async fn on_chain_start(
                &self,
                _serialized: &HashMap<String, serde_json::Value>,
                _inputs: &HashMap<String, serde_json::Value>,
                _run_id: Uuid,
                _parent_run_id: Option<Uuid>,
                _tags: &[String],
                _metadata: &HashMap<String, serde_json::Value>,
            ) -> Result<()> {
                if self.should_error {
                    Err(crate::core::error::Error::api("test error"))
                } else {
                    Ok(())
                }
            }

            fn raise_error(&self) -> bool {
                self.should_raise
            }
        }

        // Handler that errors but doesn't raise - should not propagate
        let handler1 = Arc::new(ErrorHandler {
            should_error: true,
            should_raise: false,
        });
        let manager1 = CallbackManager::with_handlers(vec![handler1 as Arc<dyn CallbackHandler>]);

        let result = manager1
            .on_chain_start(
                &HashMap::new(),
                &HashMap::new(),
                Uuid::new_v4(),
                None,
                &[],
                &HashMap::new(),
            )
            .await;
        assert!(result.is_ok());

        // Handler that errors and raises - should propagate
        let handler2 = Arc::new(ErrorHandler {
            should_error: true,
            should_raise: true,
        });
        let manager2 = CallbackManager::with_handlers(vec![handler2 as Arc<dyn CallbackHandler>]);

        let result = manager2
            .on_chain_start(
                &HashMap::new(),
                &HashMap::new(),
                Uuid::new_v4(),
                None,
                &[],
                &HashMap::new(),
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_callback_handler_tool_callbacks() {
        // Test that CallbackHandler trait has tool callback methods
        #[derive(Debug)]
        struct ToolTrackingHandler {
            tool_starts: Arc<AtomicUsize>,
            tool_ends: Arc<AtomicUsize>,
            tool_errors: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl CallbackHandler for ToolTrackingHandler {
            async fn on_tool_start(
                &self,
                _serialized: &HashMap<String, serde_json::Value>,
                _input: &str,
                _run_id: Uuid,
                _parent_run_id: Option<Uuid>,
                _tags: &[String],
                _metadata: &HashMap<String, serde_json::Value>,
            ) -> Result<()> {
                self.tool_starts.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }

            async fn on_tool_end(
                &self,
                _output: &str,
                _run_id: Uuid,
                _parent_run_id: Option<Uuid>,
            ) -> Result<()> {
                self.tool_ends.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }

            async fn on_tool_error(
                &self,
                _error: &str,
                _run_id: Uuid,
                _parent_run_id: Option<Uuid>,
            ) -> Result<()> {
                self.tool_errors.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }

        let handler = ToolTrackingHandler {
            tool_starts: Arc::new(AtomicUsize::new(0)),
            tool_ends: Arc::new(AtomicUsize::new(0)),
            tool_errors: Arc::new(AtomicUsize::new(0)),
        };

        let run_id = Uuid::new_v4();

        // Test tool callbacks directly on handler
        handler
            .on_tool_start(
                &HashMap::new(),
                "test input",
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();
        assert_eq!(handler.tool_starts.load(Ordering::SeqCst), 1);

        handler
            .on_tool_end("test output", run_id, None)
            .await
            .unwrap();
        assert_eq!(handler.tool_ends.load(Ordering::SeqCst), 1);

        handler
            .on_tool_error("test error", run_id, None)
            .await
            .unwrap();
        assert_eq!(handler.tool_errors.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_callback_handler_retriever_callbacks() {
        // Test that CallbackHandler trait has retriever callback methods
        #[derive(Debug)]
        struct RetrieverTrackingHandler {
            retriever_starts: Arc<AtomicUsize>,
            retriever_ends: Arc<AtomicUsize>,
            retriever_errors: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl CallbackHandler for RetrieverTrackingHandler {
            async fn on_retriever_start(
                &self,
                _serialized: &HashMap<String, serde_json::Value>,
                _query: &str,
                _run_id: Uuid,
                _parent_run_id: Option<Uuid>,
                _tags: &[String],
                _metadata: &HashMap<String, serde_json::Value>,
            ) -> Result<()> {
                self.retriever_starts.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }

            async fn on_retriever_end(
                &self,
                _documents: &[serde_json::Value],
                _run_id: Uuid,
                _parent_run_id: Option<Uuid>,
            ) -> Result<()> {
                self.retriever_ends.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }

            async fn on_retriever_error(
                &self,
                _error: &str,
                _run_id: Uuid,
                _parent_run_id: Option<Uuid>,
            ) -> Result<()> {
                self.retriever_errors.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }

        let handler = RetrieverTrackingHandler {
            retriever_starts: Arc::new(AtomicUsize::new(0)),
            retriever_ends: Arc::new(AtomicUsize::new(0)),
            retriever_errors: Arc::new(AtomicUsize::new(0)),
        };

        let run_id = Uuid::new_v4();

        // Test retriever callbacks directly on handler
        handler
            .on_retriever_start(
                &HashMap::new(),
                "test query",
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();
        assert_eq!(handler.retriever_starts.load(Ordering::SeqCst), 1);

        let docs = vec![serde_json::json!({"content": "test"})];
        handler.on_retriever_end(&docs, run_id, None).await.unwrap();
        assert_eq!(handler.retriever_ends.load(Ordering::SeqCst), 1);

        handler
            .on_retriever_error("test error", run_id, None)
            .await
            .unwrap();
        assert_eq!(handler.retriever_errors.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_callback_manager_llm_new_token() {
        #[derive(Debug)]
        struct TokenTrackingHandler {
            tokens: Arc<Mutex<Vec<String>>>,
        }

        #[async_trait]
        impl CallbackHandler for TokenTrackingHandler {
            async fn on_llm_new_token(
                &self,
                token: &str,
                _run_id: Uuid,
                _parent_run_id: Option<Uuid>,
            ) -> Result<()> {
                self.tokens.lock().await.push(token.to_string());
                Ok(())
            }
        }

        let handler = Arc::new(TokenTrackingHandler {
            tokens: Arc::new(Mutex::new(Vec::new())),
        });

        let manager =
            CallbackManager::with_handlers(vec![handler.clone() as Arc<dyn CallbackHandler>]);
        let run_id = Uuid::new_v4();

        // Stream some tokens
        manager
            .on_llm_new_token("Hello", run_id, None)
            .await
            .unwrap();
        manager.on_llm_new_token(" ", run_id, None).await.unwrap();
        manager
            .on_llm_new_token("world", run_id, None)
            .await
            .unwrap();

        let tokens = handler.tokens.lock().await;
        assert_eq!(*tokens, vec!["Hello", " ", "world"]);
    }

    #[tokio::test]
    async fn test_callback_manager_on_text() {
        #[derive(Debug)]
        struct TextTrackingHandler {
            texts: Arc<Mutex<Vec<String>>>,
        }

        #[async_trait]
        impl CallbackHandler for TextTrackingHandler {
            async fn on_text(
                &self,
                text: &str,
                _run_id: Uuid,
                _parent_run_id: Option<Uuid>,
            ) -> Result<()> {
                self.texts.lock().await.push(text.to_string());
                Ok(())
            }
        }

        let handler = Arc::new(TextTrackingHandler {
            texts: Arc::new(Mutex::new(Vec::new())),
        });

        let manager =
            CallbackManager::with_handlers(vec![handler.clone() as Arc<dyn CallbackHandler>]);
        let run_id = Uuid::new_v4();

        manager.on_text("First text", run_id, None).await.unwrap();
        manager.on_text("Second text", run_id, None).await.unwrap();

        let texts = handler.texts.lock().await;
        assert_eq!(*texts, vec!["First text", "Second text"]);
    }

    #[tokio::test]
    async fn test_callback_manager_on_retry() {
        #[derive(Debug)]
        struct RetryTrackingHandler {
            retry_count: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl CallbackHandler for RetryTrackingHandler {
            async fn on_retry(&self, _run_id: Uuid, _parent_run_id: Option<Uuid>) -> Result<()> {
                self.retry_count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }

        let handler = Arc::new(RetryTrackingHandler {
            retry_count: Arc::new(AtomicUsize::new(0)),
        });

        let manager =
            CallbackManager::with_handlers(vec![handler.clone() as Arc<dyn CallbackHandler>]);
        let run_id = Uuid::new_v4();

        manager.on_retry(run_id, None).await.unwrap();
        manager.on_retry(run_id, None).await.unwrap();
        manager.on_retry(run_id, None).await.unwrap();

        assert_eq!(handler.retry_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_null_callback_handler() {
        let handler = NullCallbackHandler;
        let run_id = Uuid::new_v4();

        // All methods should be no-ops
        handler
            .on_chain_start(
                &HashMap::new(),
                &HashMap::new(),
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();
        handler
            .on_chain_end(&HashMap::new(), run_id, None)
            .await
            .unwrap();
        handler.on_chain_error("error", run_id, None).await.unwrap();
        handler
            .on_llm_start(&HashMap::new(), &[], run_id, None, &[], &HashMap::new())
            .await
            .unwrap();
        handler
            .on_llm_end(&HashMap::new(), run_id, None)
            .await
            .unwrap();
        handler.on_llm_error("error", run_id, None).await.unwrap();
        handler
            .on_llm_new_token("token", run_id, None)
            .await
            .unwrap();

        // Verify ignore flags default to false
        assert!(!handler.ignore_llm());
        assert!(!handler.ignore_chain());
        assert!(!handler.ignore_tool());
        assert!(!handler.ignore_retriever());
        assert!(!handler.raise_error());
    }

    #[tokio::test]
    async fn test_console_callback_handler_colored_output() {
        // Test both colored and non-colored variants
        let colored_handler = ConsoleCallbackHandler::new(true);
        let plain_handler = ConsoleCallbackHandler::new(false);

        assert!(colored_handler.colored);
        assert!(!plain_handler.colored);

        // Test default
        let default_handler = ConsoleCallbackHandler::default();
        assert!(default_handler.colored);
    }

    #[tokio::test]
    async fn test_console_callback_handler_chain_events() {
        let handler = ConsoleCallbackHandler::new(false);
        let run_id = Uuid::new_v4();

        let mut serialized = HashMap::new();
        serialized.insert("name".to_string(), serde_json::json!("TestChain"));

        // Should not panic
        handler
            .on_chain_start(
                &serialized,
                &HashMap::new(),
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();

        handler
            .on_chain_end(&HashMap::new(), run_id, None)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_callback_manager_ignore_flags() {
        #[derive(Debug)]
        struct SelectiveHandler {
            chain_count: Arc<AtomicUsize>,
            llm_count: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl CallbackHandler for SelectiveHandler {
            async fn on_chain_start(
                &self,
                _serialized: &HashMap<String, serde_json::Value>,
                _inputs: &HashMap<String, serde_json::Value>,
                _run_id: Uuid,
                _parent_run_id: Option<Uuid>,
                _tags: &[String],
                _metadata: &HashMap<String, serde_json::Value>,
            ) -> Result<()> {
                self.chain_count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }

            async fn on_llm_start(
                &self,
                _serialized: &HashMap<String, serde_json::Value>,
                _prompts: &[String],
                _run_id: Uuid,
                _parent_run_id: Option<Uuid>,
                _tags: &[String],
                _metadata: &HashMap<String, serde_json::Value>,
            ) -> Result<()> {
                self.llm_count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }

            // Ignore chains but not LLMs
            fn ignore_chain(&self) -> bool {
                true
            }
        }

        let handler = Arc::new(SelectiveHandler {
            chain_count: Arc::new(AtomicUsize::new(0)),
            llm_count: Arc::new(AtomicUsize::new(0)),
        });

        let manager =
            CallbackManager::with_handlers(vec![handler.clone() as Arc<dyn CallbackHandler>]);
        let run_id = Uuid::new_v4();

        // Call both chain and LLM callbacks
        manager
            .on_chain_start(
                &HashMap::new(),
                &HashMap::new(),
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();

        manager
            .on_llm_start(&HashMap::new(), &[], run_id, None, &[], &HashMap::new())
            .await
            .unwrap();

        // Only LLM callback should have been invoked (chain was ignored)
        assert_eq!(handler.chain_count.load(Ordering::SeqCst), 0);
        assert_eq!(handler.llm_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_callback_event_types() {
        // Test enum variants
        assert_eq!(CallbackEvent::ChainStart, CallbackEvent::ChainStart);
        assert_eq!(CallbackEvent::ChainEnd, CallbackEvent::ChainEnd);
        assert_eq!(CallbackEvent::LlmStart, CallbackEvent::LlmStart);
        assert_eq!(CallbackEvent::ToolStart, CallbackEvent::ToolStart);
        assert_eq!(
            CallbackEvent::Custom("test".to_string()),
            CallbackEvent::Custom("test".to_string())
        );

        // Test Debug trait
        let event = CallbackEvent::ChainStart;
        assert!(format!("{:?}", event).contains("ChainStart"));
    }

    #[tokio::test]
    async fn test_callback_manager_chat_model_start() {
        #[derive(Debug)]
        struct ChatModelTrackingHandler {
            chat_model_count: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl CallbackHandler for ChatModelTrackingHandler {
            async fn on_chat_model_start(
                &self,
                _serialized: &HashMap<String, serde_json::Value>,
                _messages: &[Vec<Message>],
                _run_id: Uuid,
                _parent_run_id: Option<Uuid>,
                _tags: &[String],
                _metadata: &HashMap<String, serde_json::Value>,
            ) -> Result<()> {
                self.chat_model_count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }

        let handler = Arc::new(ChatModelTrackingHandler {
            chat_model_count: Arc::new(AtomicUsize::new(0)),
        });

        let manager =
            CallbackManager::with_handlers(vec![handler.clone() as Arc<dyn CallbackHandler>]);
        let run_id = Uuid::new_v4();

        let messages = vec![vec![Message::human("test")]];
        manager
            .on_chat_model_start(
                &HashMap::new(),
                &messages,
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();

        assert_eq!(handler.chat_model_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_callback_manager_debug_format() {
        let manager = CallbackManager::new();
        let debug_str = format!("{:?}", manager);
        assert!(debug_str.contains("CallbackManager"));
        assert!(debug_str.contains("handlers_count"));
    }

    #[tokio::test]
    async fn test_execution_context_debug_format() {
        let config = RunnableConfig::new();
        let ctx = ExecutionContext::new(config);
        let debug_str = format!("{:?}", ctx);
        assert!(debug_str.contains("ExecutionContext"));
        assert!(debug_str.contains("has_callbacks"));
    }
}
