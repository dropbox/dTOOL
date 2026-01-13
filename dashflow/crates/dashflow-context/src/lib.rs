//! # DashFlow Context Management
//!
//! Context window management for LLM applications. Provides token counting,
//! truncation strategies, and budget tracking with reserved response space.
//!
//! ## Features
//!
//! - **Token counting**: Uses tiktoken-rs for accurate token counts
//! - **Truncation strategies**: Drop-oldest, sliding-window, keep-first-and-last
//! - **Budget tracking**: Reserve tokens for response generation
//! - **Model-specific limits**: Auto-detect context window sizes
//!
//! ## Example
//!
//! ```
//! use dashflow_context::{ContextManager, TruncationStrategy};
//! use dashflow::core::messages::Message;
//!
//! let manager = ContextManager::builder()
//!     .model("gpt-4o")
//!     .reserve_tokens(4000)
//!     .truncation(TruncationStrategy::DropOldest)
//!     .build();
//!
//! // Count tokens in text
//! let count = manager.count_tokens("Hello, world!");
//!
//! // Check if messages fit within budget
//! let messages = vec![
//!     Message::system("You are helpful"),
//!     Message::human("Hello!"),
//! ];
//! let result = manager.fit(&messages);
//! println!("Messages: {}, Tokens: {}", result.messages.len(), result.token_count);
//! ```

use dashflow::core::messages::{ContentBlock, IntoLlmMessage, Message, MessageContent};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;
use thiserror::Error;
use tiktoken_rs::{get_bpe_from_model, CoreBPE};

/// Errors that can occur during context management
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ContextError {
    /// Unknown model name
    #[error("Unknown model: {0}")]
    UnknownModel(String),

    /// Token counting failed
    #[error("Failed to count tokens: {0}")]
    TokenCountError(String),

    /// Message is too large for context window
    #[error("Single message exceeds context limit: {0} tokens > {1} available")]
    MessageTooLarge(usize, usize),

    /// Reserved space exceeds context limit
    #[error("Reserved tokens ({0}) exceed context limit ({1})")]
    ReservedExceedsLimit(usize, usize),
}

/// Truncation strategy for managing context overflow
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TruncationStrategy {
    /// Drop oldest messages first (keeps most recent context)
    #[default]
    DropOldest,

    /// Sliding window - keep last N tokens worth of messages
    SlidingWindow,

    /// Keep first (system) and last messages, drop middle
    KeepFirstAndLast,
}

/// Model context limits (in tokens)
#[derive(Debug, Clone, Copy)]
pub struct ModelLimits {
    /// Maximum context window size
    pub context_window: usize,
    /// Maximum output tokens (if different from context)
    pub max_output: Option<usize>,
}

impl ModelLimits {
    /// Create new model limits
    #[must_use]
    pub const fn new(context_window: usize) -> Self {
        Self {
            context_window,
            max_output: None,
        }
    }

    /// Create model limits with custom output limit
    #[must_use]
    pub const fn with_output(context_window: usize, max_output: usize) -> Self {
        Self {
            context_window,
            max_output: Some(max_output),
        }
    }
}

/// Get model limits for known models
fn get_model_limits() -> &'static HashMap<&'static str, ModelLimits> {
    static LIMITS: OnceLock<HashMap<&'static str, ModelLimits>> = OnceLock::new();
    LIMITS.get_or_init(|| {
        let mut m = HashMap::new();

        // OpenAI models
        m.insert("gpt-4o", ModelLimits::with_output(128_000, 16_384));
        m.insert("gpt-4o-mini", ModelLimits::with_output(128_000, 16_384));
        m.insert("gpt-4-turbo", ModelLimits::with_output(128_000, 4_096));
        m.insert(
            "gpt-4-turbo-preview",
            ModelLimits::with_output(128_000, 4_096),
        );
        m.insert("gpt-4", ModelLimits::with_output(8_192, 4_096));
        m.insert("gpt-4-32k", ModelLimits::with_output(32_768, 4_096));
        m.insert("gpt-3.5-turbo", ModelLimits::with_output(16_385, 4_096));
        m.insert("gpt-3.5-turbo-16k", ModelLimits::with_output(16_385, 4_096));
        m.insert("o1", ModelLimits::with_output(200_000, 100_000));
        m.insert("o1-mini", ModelLimits::with_output(128_000, 65_536));
        m.insert("o1-preview", ModelLimits::with_output(128_000, 32_768));
        m.insert("o3-mini", ModelLimits::with_output(200_000, 100_000));

        // Anthropic models
        m.insert("claude-3-opus", ModelLimits::with_output(200_000, 4_096));
        m.insert("claude-3-sonnet", ModelLimits::with_output(200_000, 4_096));
        m.insert("claude-3-haiku", ModelLimits::with_output(200_000, 4_096));
        m.insert(
            "claude-3-5-sonnet",
            ModelLimits::with_output(200_000, 8_192),
        );
        m.insert("claude-3-5-haiku", ModelLimits::with_output(200_000, 8_192));
        m.insert("claude-opus-4", ModelLimits::with_output(200_000, 32_000));
        m.insert("claude-sonnet-4", ModelLimits::with_output(200_000, 64_000));

        // Google models
        m.insert("gemini-1.5-pro", ModelLimits::with_output(2_097_152, 8_192));
        m.insert(
            "gemini-1.5-flash",
            ModelLimits::with_output(1_048_576, 8_192),
        );
        m.insert(
            "gemini-2.0-flash",
            ModelLimits::with_output(1_048_576, 8_192),
        );

        // Mistral models
        m.insert("mistral-large", ModelLimits::with_output(128_000, 8_192));
        m.insert("mistral-medium", ModelLimits::with_output(32_000, 8_192));
        m.insert("mistral-small", ModelLimits::with_output(32_000, 8_192));
        m.insert("codestral", ModelLimits::with_output(32_000, 8_192));

        // DeepSeek models
        m.insert("deepseek-chat", ModelLimits::with_output(64_000, 8_000));
        m.insert("deepseek-coder", ModelLimits::with_output(64_000, 8_000));

        m
    })
}

/// Resolve model name to canonical form and get limits
fn resolve_model(model: &str) -> Option<(&'static str, ModelLimits)> {
    let limits = get_model_limits();

    // Direct match - need to find the static key
    for (&name, &limit) in limits.iter() {
        if name == model {
            return Some((name, limit));
        }
    }

    // Prefix matching for versioned model names
    let model_lower = model.to_lowercase();
    for (&name, &limit) in limits.iter() {
        if model_lower.starts_with(name) || model_lower.contains(name) {
            return Some((name, limit));
        }
    }

    // Fallback patterns
    if model_lower.contains("gpt-4o") {
        return Some(("gpt-4o", *limits.get("gpt-4o")?));
    }
    if model_lower.contains("gpt-4") {
        return Some(("gpt-4", *limits.get("gpt-4")?));
    }
    if model_lower.contains("gpt-3.5") {
        return Some(("gpt-3.5-turbo", *limits.get("gpt-3.5-turbo")?));
    }
    if model_lower.contains("claude-3-opus") || model_lower.contains("claude-3-opus") {
        return Some(("claude-3-opus", *limits.get("claude-3-opus")?));
    }
    if model_lower.contains("claude-3-5-sonnet") || model_lower.contains("claude-3.5-sonnet") {
        return Some(("claude-3-5-sonnet", *limits.get("claude-3-5-sonnet")?));
    }
    if model_lower.contains("claude") {
        return Some(("claude-3-sonnet", *limits.get("claude-3-sonnet")?));
    }
    if model_lower.contains("gemini") {
        return Some(("gemini-1.5-pro", *limits.get("gemini-1.5-pro")?));
    }

    None
}

/// Get tiktoken encoder for a model
fn get_encoder(model: &str) -> Option<CoreBPE> {
    // Try direct model name first
    if let Ok(bpe) = get_bpe_from_model(model) {
        return Some(bpe);
    }

    // Try common model prefixes
    let model_lower = model.to_lowercase();
    if model_lower.contains("gpt-4") || model_lower.contains("gpt4") {
        return get_bpe_from_model("gpt-4").ok();
    }
    if model_lower.contains("gpt-3.5") || model_lower.contains("gpt3") {
        return get_bpe_from_model("gpt-3.5-turbo").ok();
    }

    // Fallback to cl100k_base for unknown models (reasonable default)
    tiktoken_rs::cl100k_base().ok()
}

/// Result of fitting messages into context
#[derive(Debug, Clone)]
pub struct FitResult {
    /// Messages that fit within the budget
    pub messages: Vec<Message>,
    /// Total tokens used
    pub token_count: usize,
    /// Number of messages dropped
    pub messages_dropped: usize,
    /// Tokens available for response
    pub tokens_remaining: usize,
}

/// Builder for `ContextManager`
#[derive(Debug, Clone)]
pub struct ContextManagerBuilder {
    model: Option<String>,
    context_limit: Option<usize>,
    reserve_tokens: usize,
    truncation: TruncationStrategy,
    tokens_per_message: usize,
}

impl Default for ContextManagerBuilder {
    fn default() -> Self {
        Self {
            model: None,
            context_limit: None,
            reserve_tokens: 4000,
            truncation: TruncationStrategy::DropOldest,
            tokens_per_message: 4, // OpenAI format overhead
        }
    }
}

impl ContextManagerBuilder {
    /// Set the model name (auto-detects context limits)
    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set explicit context limit (overrides model detection)
    #[must_use]
    pub fn context_limit(mut self, limit: usize) -> Self {
        self.context_limit = Some(limit);
        self
    }

    /// Set tokens to reserve for response
    #[must_use]
    pub fn reserve_tokens(mut self, tokens: usize) -> Self {
        self.reserve_tokens = tokens;
        self
    }

    /// Set truncation strategy
    #[must_use]
    pub fn truncation(mut self, strategy: TruncationStrategy) -> Self {
        self.truncation = strategy;
        self
    }

    /// Set tokens per message overhead
    #[must_use]
    pub fn tokens_per_message(mut self, tokens: usize) -> Self {
        self.tokens_per_message = tokens;
        self
    }

    /// Build the `ContextManager`
    #[must_use]
    pub fn build(self) -> ContextManager {
        let model = self.model.unwrap_or_else(|| "gpt-4".to_string());

        // Resolve model limits
        let (resolved_model, limits) =
            resolve_model(&model).unwrap_or(("gpt-4", ModelLimits::new(8192)));

        // Use explicit limit if provided, otherwise model default
        let context_limit = self.context_limit.unwrap_or(limits.context_window);

        // Get encoder
        let encoder = get_encoder(resolved_model);

        ContextManager {
            model: model.clone(),
            resolved_model: resolved_model.to_string(),
            context_limit,
            reserve_tokens: self.reserve_tokens,
            truncation: self.truncation,
            tokens_per_message: self.tokens_per_message,
            encoder,
            limits,
        }
    }
}

/// Context window manager for LLM applications
///
/// Manages token budgets, counting, and truncation for chat messages.
pub struct ContextManager {
    /// Original model name provided
    model: String,
    /// Resolved model name (for tokenizer selection)
    resolved_model: String,
    /// Maximum context window size
    context_limit: usize,
    /// Tokens reserved for response
    reserve_tokens: usize,
    /// Truncation strategy
    truncation: TruncationStrategy,
    /// Tokens per message overhead
    tokens_per_message: usize,
    /// Tokenizer (optional - falls back to estimation)
    encoder: Option<CoreBPE>,
    /// Model limits
    limits: ModelLimits,
}

impl std::fmt::Debug for ContextManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContextManager")
            .field("model", &self.model)
            .field("resolved_model", &self.resolved_model)
            .field("context_limit", &self.context_limit)
            .field("reserve_tokens", &self.reserve_tokens)
            .field("truncation", &self.truncation)
            .field("tokens_per_message", &self.tokens_per_message)
            .field("encoder", &self.encoder.is_some())
            .field("limits", &self.limits)
            .finish()
    }
}

impl ContextManager {
    /// Create a new builder
    #[must_use]
    pub fn builder() -> ContextManagerBuilder {
        ContextManagerBuilder::default()
    }

    /// Create a context manager for a specific model
    #[must_use]
    pub fn for_model(model: impl Into<String>) -> Self {
        Self::builder().model(model).build()
    }

    /// Get the model name
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Get the context limit
    #[must_use]
    pub fn context_limit(&self) -> usize {
        self.context_limit
    }

    /// Get reserved tokens
    #[must_use]
    pub fn reserve_tokens(&self) -> usize {
        self.reserve_tokens
    }

    /// Get available tokens (context limit minus reserved)
    #[must_use]
    pub fn available_tokens(&self) -> usize {
        self.context_limit.saturating_sub(self.reserve_tokens)
    }

    /// Get model limits
    #[must_use]
    pub fn limits(&self) -> ModelLimits {
        self.limits
    }

    /// Count tokens in text
    #[must_use]
    pub fn count_tokens(&self, text: &str) -> usize {
        if let Some(ref encoder) = self.encoder {
            encoder.encode_with_special_tokens(text).len()
        } else {
            // Fallback estimation: ~4 characters per token
            text.len().div_ceil(4)
        }
    }

    /// Count tokens in a message
    #[must_use]
    pub fn count_message_tokens(&self, message: &Message) -> usize {
        let content_tokens = match message {
            Message::Human { content, .. }
            | Message::AI { content, .. }
            | Message::System { content, .. }
            | Message::Tool { content, .. }
            | Message::Function { content, .. } => self.count_content_tokens(content),
        };

        // Add overhead for message structure
        content_tokens + self.tokens_per_message
    }

    /// Count tokens in message content
    fn count_content_tokens(&self, content: &MessageContent) -> usize {
        match content {
            MessageContent::Text(text) => self.count_tokens(text),
            MessageContent::Blocks(blocks) => {
                blocks.iter().map(|b| self.count_block_tokens(b)).sum()
            }
        }
    }

    /// Count tokens in a content block
    fn count_block_tokens(&self, block: &ContentBlock) -> usize {
        match block {
            ContentBlock::Text { text } => self.count_tokens(text),
            ContentBlock::Image { .. } => {
                // Images have fixed token cost (high/low detail)
                // High detail: ~765 tokens, Low: ~85 tokens
                // Default to high estimate
                765
            }
            ContentBlock::ToolUse { name, input, .. } => {
                self.count_tokens(name) + self.count_tokens(&input.to_string())
            }
            ContentBlock::ToolResult { content, .. } => self.count_tokens(content),
            ContentBlock::Reasoning { reasoning } => self.count_tokens(reasoning),
            ContentBlock::Thinking { thinking, .. } => self.count_tokens(thinking),
            ContentBlock::RedactedThinking { data } => self.count_tokens(data),
        }
    }

    /// Count total tokens in a list of messages
    #[must_use]
    pub fn count_messages_tokens(&self, messages: &[Message]) -> usize {
        messages
            .iter()
            .map(|m| self.count_message_tokens(m))
            .sum::<usize>()
            + 3 // Base overhead for message array
    }

    /// Fit messages into context budget, applying truncation if needed
    ///
    /// Returns the messages that fit and metadata about the operation.
    #[must_use]
    pub fn fit(&self, messages: &[Message]) -> FitResult {
        let available = self.available_tokens();
        let total_tokens = self.count_messages_tokens(messages);

        // If messages fit, return as-is
        if total_tokens <= available {
            return FitResult {
                messages: messages.to_vec(),
                token_count: total_tokens,
                messages_dropped: 0,
                tokens_remaining: available.saturating_sub(total_tokens),
            };
        }

        // Apply truncation strategy
        match self.truncation {
            TruncationStrategy::DropOldest => self.truncate_drop_oldest(messages, available),
            TruncationStrategy::SlidingWindow => self.truncate_sliding_window(messages, available),
            TruncationStrategy::KeepFirstAndLast => {
                self.truncate_keep_first_and_last(messages, available)
            }
        }
    }

    /// Drop oldest messages first (keeps system message and most recent)
    fn truncate_drop_oldest(&self, messages: &[Message], available: usize) -> FitResult {
        if messages.is_empty() {
            return FitResult {
                messages: vec![],
                token_count: 0,
                messages_dropped: 0,
                tokens_remaining: available,
            };
        }

        // Always keep system message if first
        let (system_msg, rest) = if matches!(messages.first(), Some(Message::System { .. })) {
            (Some(&messages[0]), &messages[1..])
        } else {
            (None, messages)
        };

        let mut result = Vec::new();
        let mut tokens_used = 3; // Base overhead

        // Add system message first
        if let Some(sys) = system_msg {
            let sys_tokens = self.count_message_tokens(sys);
            result.push(sys.clone());
            tokens_used += sys_tokens;
        }

        // Add messages from newest to oldest until budget exhausted
        let mut messages_to_add: Vec<&Message> = Vec::new();
        let mut pending_tokens = 0;

        for msg in rest.iter().rev() {
            let msg_tokens = self.count_message_tokens(msg);
            if tokens_used + pending_tokens + msg_tokens <= available {
                pending_tokens += msg_tokens;
                messages_to_add.push(msg);
            } else {
                break;
            }
        }

        // Reverse to restore chronological order
        messages_to_add.reverse();
        for msg in messages_to_add {
            result.push(msg.clone());
        }

        tokens_used += pending_tokens;
        let messages_dropped = messages.len() - result.len();

        FitResult {
            messages: result,
            token_count: tokens_used,
            messages_dropped,
            tokens_remaining: available.saturating_sub(tokens_used),
        }
    }

    /// Sliding window - keep last N tokens worth of messages
    fn truncate_sliding_window(&self, messages: &[Message], available: usize) -> FitResult {
        // Same as drop_oldest for now
        self.truncate_drop_oldest(messages, available)
    }

    /// Keep first (system) and last messages, drop middle
    fn truncate_keep_first_and_last(&self, messages: &[Message], available: usize) -> FitResult {
        if messages.len() <= 2 {
            return self.truncate_drop_oldest(messages, available);
        }

        let first = &messages[0];
        let last = &messages[messages.len() - 1];

        let first_tokens = self.count_message_tokens(first);
        let last_tokens = self.count_message_tokens(last);
        let base_tokens = first_tokens + last_tokens + 3;

        // If first + last don't fit, fall back to drop_oldest
        if base_tokens > available {
            return self.truncate_drop_oldest(messages, available);
        }

        let mut result = vec![first.clone()];
        let mut tokens_used = first_tokens + 3;

        // Try to add messages from the end working backwards
        let middle = &messages[1..messages.len() - 1];
        let mut middle_to_add: Vec<&Message> = Vec::new();
        let mut middle_tokens = 0;

        for msg in middle.iter().rev() {
            let msg_tokens = self.count_message_tokens(msg);
            if tokens_used + middle_tokens + msg_tokens + last_tokens <= available {
                middle_tokens += msg_tokens;
                middle_to_add.push(msg);
            } else {
                break;
            }
        }

        // Add middle messages in chronological order
        middle_to_add.reverse();
        for msg in middle_to_add {
            result.push(msg.clone());
            tokens_used += self.count_message_tokens(msg);
        }

        // Add last message
        result.push(last.clone());
        tokens_used += last_tokens;

        let messages_dropped = messages.len() - result.len();

        FitResult {
            messages: result,
            token_count: tokens_used,
            messages_dropped,
            tokens_remaining: available.saturating_sub(tokens_used),
        }
    }

    /// Check if messages fit within the available budget
    #[must_use]
    pub fn fits(&self, messages: &[Message]) -> bool {
        self.count_messages_tokens(messages) <= self.available_tokens()
    }

    /// Get context usage as a percentage (0.0 to 1.0)
    #[must_use]
    pub fn usage_ratio(&self, messages: &[Message]) -> f64 {
        let tokens = self.count_messages_tokens(messages);
        tokens as f64 / self.context_limit as f64
    }

    // ===== Generic IntoLlmMessage API =====
    //
    // These methods accept any type implementing IntoLlmMessage,
    // enabling token counting for custom message types without conversion.

    /// Count tokens for any message type implementing [`IntoLlmMessage`]
    ///
    /// This is the generic version of [`Self::count_message_tokens`] that works with
    /// custom message types. Applications can implement `IntoLlmMessage` for their
    /// own message types and use this method directly.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow_context::ContextManager;
    /// use dashflow::core::messages::{IntoLlmMessage, ToolCall};
    ///
    /// struct MyMessage {
    ///     role: String,
    ///     content: String,
    /// }
    ///
    /// impl IntoLlmMessage for MyMessage {
    ///     fn role(&self) -> &str { &self.role }
    ///     fn content(&self) -> &str { &self.content }
    ///     fn tool_calls(&self) -> Option<&[ToolCall]> { None }
    ///     fn tool_call_id(&self) -> Option<&str> { None }
    /// }
    ///
    /// let manager = ContextManager::for_model("gpt-4o");
    /// let msg = MyMessage { role: "user".to_string(), content: "Hello!".to_string() };
    /// let tokens = manager.count_llm_message_tokens(&msg);
    /// ```
    #[must_use]
    pub fn count_llm_message_tokens<M: IntoLlmMessage>(&self, message: &M) -> usize {
        let content_tokens = self.count_tokens(message.content());

        // Add tokens for tool calls if present
        let tool_tokens = if let Some(tool_calls) = message.tool_calls() {
            tool_calls
                .iter()
                .map(|tc| self.count_tokens(&tc.name) + self.count_tokens(&tc.args.to_string()))
                .sum()
        } else {
            0
        };

        // Add overhead for message structure
        content_tokens + tool_tokens + self.tokens_per_message
    }

    /// Count tokens for a slice of messages implementing [`IntoLlmMessage`]
    ///
    /// This is the generic version of [`Self::count_messages_tokens`] that works with
    /// custom message types.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow_context::ContextManager;
    /// use dashflow::core::messages::{IntoLlmMessage, ToolCall};
    ///
    /// struct MyMessage {
    ///     role: String,
    ///     content: String,
    /// }
    ///
    /// impl IntoLlmMessage for MyMessage {
    ///     fn role(&self) -> &str { &self.role }
    ///     fn content(&self) -> &str { &self.content }
    ///     fn tool_calls(&self) -> Option<&[ToolCall]> { None }
    ///     fn tool_call_id(&self) -> Option<&str> { None }
    /// }
    ///
    /// let manager = ContextManager::for_model("gpt-4o");
    /// let messages = vec![
    ///     MyMessage { role: "system".to_string(), content: "You are helpful".to_string() },
    ///     MyMessage { role: "user".to_string(), content: "Hello!".to_string() },
    /// ];
    /// let total_tokens = manager.count_llm_messages_tokens(&messages);
    /// ```
    #[must_use]
    pub fn count_llm_messages_tokens<M: IntoLlmMessage>(&self, messages: &[M]) -> usize {
        messages
            .iter()
            .map(|m| self.count_llm_message_tokens(m))
            .sum::<usize>()
            + 3 // Base overhead for message array
    }

    /// Check if messages implementing [`IntoLlmMessage`] fit within the available budget
    #[must_use]
    pub fn fits_llm_messages<M: IntoLlmMessage>(&self, messages: &[M]) -> bool {
        self.count_llm_messages_tokens(messages) <= self.available_tokens()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== ContextError Tests =====

    #[test]
    fn test_context_error_unknown_model() {
        let err = ContextError::UnknownModel("test-model".to_string());
        assert!(err.to_string().contains("Unknown model"));
        assert!(err.to_string().contains("test-model"));
    }

    #[test]
    fn test_context_error_token_count_error() {
        let err = ContextError::TokenCountError("failed to encode".to_string());
        assert!(err.to_string().contains("Failed to count tokens"));
        assert!(err.to_string().contains("failed to encode"));
    }

    #[test]
    fn test_context_error_message_too_large() {
        let err = ContextError::MessageTooLarge(5000, 4000);
        assert!(err.to_string().contains("5000"));
        assert!(err.to_string().contains("4000"));
        assert!(err.to_string().contains("exceeds context limit"));
    }

    #[test]
    fn test_context_error_reserved_exceeds_limit() {
        let err = ContextError::ReservedExceedsLimit(10000, 8000);
        assert!(err.to_string().contains("10000"));
        assert!(err.to_string().contains("8000"));
        assert!(err.to_string().contains("exceed"));
    }

    // ===== TruncationStrategy Tests =====

    #[test]
    fn test_truncation_strategy_default() {
        let strategy = TruncationStrategy::default();
        assert_eq!(strategy, TruncationStrategy::DropOldest);
    }

    #[test]
    fn test_truncation_strategy_serde() {
        // Serialize
        let drop_oldest = TruncationStrategy::DropOldest;
        let json = serde_json::to_string(&drop_oldest).unwrap();
        assert_eq!(json, "\"drop_oldest\"");

        let sliding = TruncationStrategy::SlidingWindow;
        let json = serde_json::to_string(&sliding).unwrap();
        assert_eq!(json, "\"sliding_window\"");

        let keep_fl = TruncationStrategy::KeepFirstAndLast;
        let json = serde_json::to_string(&keep_fl).unwrap();
        assert_eq!(json, "\"keep_first_and_last\"");

        // Deserialize
        let parsed: TruncationStrategy = serde_json::from_str("\"drop_oldest\"").unwrap();
        assert_eq!(parsed, TruncationStrategy::DropOldest);

        let parsed: TruncationStrategy = serde_json::from_str("\"sliding_window\"").unwrap();
        assert_eq!(parsed, TruncationStrategy::SlidingWindow);

        let parsed: TruncationStrategy = serde_json::from_str("\"keep_first_and_last\"").unwrap();
        assert_eq!(parsed, TruncationStrategy::KeepFirstAndLast);
    }

    #[test]
    fn test_truncation_strategy_clone_copy() {
        let strategy = TruncationStrategy::SlidingWindow;
        let cloned = strategy.clone();
        let copied = strategy;
        assert_eq!(cloned, copied);
        assert_eq!(copied, TruncationStrategy::SlidingWindow);
    }

    // ===== ModelLimits Tests =====

    #[test]
    fn test_model_limits_new() {
        let limits = ModelLimits::new(8192);
        assert_eq!(limits.context_window, 8192);
        assert_eq!(limits.max_output, None);
    }

    #[test]
    fn test_model_limits_with_output() {
        let limits = ModelLimits::with_output(128_000, 16_384);
        assert_eq!(limits.context_window, 128_000);
        assert_eq!(limits.max_output, Some(16_384));
    }

    #[test]
    fn test_model_limits_debug() {
        let limits = ModelLimits::with_output(100, 50);
        let debug = format!("{:?}", limits);
        assert!(debug.contains("100"));
        assert!(debug.contains("50"));
    }

    #[test]
    fn test_model_limits_clone_copy() {
        let limits = ModelLimits::with_output(1000, 500);
        let cloned = limits;
        assert_eq!(cloned.context_window, 1000);
        assert_eq!(cloned.max_output, Some(500));
    }

    // ===== Model Detection Tests =====

    #[test]
    fn test_model_detection_openai() {
        // GPT-4o
        let manager = ContextManager::for_model("gpt-4o");
        assert_eq!(manager.context_limit(), 128_000);

        let manager = ContextManager::for_model("gpt-4o-mini");
        assert_eq!(manager.context_limit(), 128_000);

        // GPT-4 Turbo
        let manager = ContextManager::for_model("gpt-4-turbo");
        assert_eq!(manager.context_limit(), 128_000);

        // GPT-4 base
        let manager = ContextManager::for_model("gpt-4");
        assert_eq!(manager.context_limit(), 8192);

        let manager = ContextManager::for_model("gpt-4-32k");
        assert_eq!(manager.context_limit(), 32_768);

        // GPT-3.5
        let manager = ContextManager::for_model("gpt-3.5-turbo");
        assert_eq!(manager.context_limit(), 16_385);

        // o1 models
        let manager = ContextManager::for_model("o1");
        assert_eq!(manager.context_limit(), 200_000);

        let manager = ContextManager::for_model("o1-mini");
        assert_eq!(manager.context_limit(), 128_000);
    }

    #[test]
    fn test_model_detection_anthropic() {
        let manager = ContextManager::for_model("claude-3-opus");
        assert_eq!(manager.context_limit(), 200_000);

        let manager = ContextManager::for_model("claude-3-sonnet");
        assert_eq!(manager.context_limit(), 200_000);

        let manager = ContextManager::for_model("claude-3-haiku");
        assert_eq!(manager.context_limit(), 200_000);

        let manager = ContextManager::for_model("claude-3-5-sonnet");
        assert_eq!(manager.context_limit(), 200_000);

        let manager = ContextManager::for_model("claude-opus-4");
        assert_eq!(manager.context_limit(), 200_000);
    }

    #[test]
    fn test_model_detection_google() {
        let manager = ContextManager::for_model("gemini-1.5-pro");
        assert_eq!(manager.context_limit(), 2_097_152);

        let manager = ContextManager::for_model("gemini-1.5-flash");
        assert_eq!(manager.context_limit(), 1_048_576);

        let manager = ContextManager::for_model("gemini-2.0-flash");
        assert_eq!(manager.context_limit(), 1_048_576);
    }

    #[test]
    fn test_model_detection_mistral() {
        let manager = ContextManager::for_model("mistral-large");
        assert_eq!(manager.context_limit(), 128_000);

        let manager = ContextManager::for_model("mistral-medium");
        assert_eq!(manager.context_limit(), 32_000);

        let manager = ContextManager::for_model("codestral");
        assert_eq!(manager.context_limit(), 32_000);
    }

    #[test]
    fn test_model_detection_deepseek() {
        let manager = ContextManager::for_model("deepseek-chat");
        assert_eq!(manager.context_limit(), 64_000);

        let manager = ContextManager::for_model("deepseek-coder");
        assert_eq!(manager.context_limit(), 64_000);
    }

    #[test]
    fn test_model_detection_contains_matching() {
        // Model names containing known patterns should match
        // Note: prefix matching is case-insensitive and uses contains()

        // Contains "gpt-4" so matches gpt-4 (not gpt-4o since version suffix breaks exact prefix)
        let manager = ContextManager::for_model("gpt-4-0613");
        assert_eq!(manager.context_limit(), 8192); // Matches gpt-4

        // Contains "claude-3-5-sonnet" - direct match
        let manager = ContextManager::for_model("claude-3-5-sonnet");
        assert_eq!(manager.context_limit(), 200_000);

        // Contains "claude" pattern
        let manager = ContextManager::for_model("anthropic-claude-model");
        assert_eq!(manager.context_limit(), 200_000); // Falls back to claude-3-sonnet
    }

    #[test]
    fn test_model_detection_fallback_to_gpt4() {
        // Unknown model should fallback to gpt-4 defaults
        let manager = ContextManager::for_model("unknown-model-xyz");
        assert_eq!(manager.context_limit(), 8192); // gpt-4 default
    }

    // ===== ContextManagerBuilder Tests =====

    #[test]
    fn test_builder_defaults() {
        let manager = ContextManager::builder().model("gpt-4").build();

        assert_eq!(manager.model(), "gpt-4");
        assert_eq!(manager.reserve_tokens(), 4000);
    }

    #[test]
    fn test_builder_all_options() {
        let manager = ContextManager::builder()
            .model("gpt-4")
            .context_limit(50_000)
            .reserve_tokens(8000)
            .truncation(TruncationStrategy::KeepFirstAndLast)
            .tokens_per_message(8)
            .build();

        assert_eq!(manager.model(), "gpt-4");
        assert_eq!(manager.context_limit(), 50_000);
        assert_eq!(manager.reserve_tokens(), 8000);
        assert_eq!(manager.available_tokens(), 42_000);
    }

    #[test]
    fn test_builder_default_model() {
        // Build without specifying model - should default to gpt-4
        let manager = ContextManager::builder().build();
        assert_eq!(manager.model(), "gpt-4");
    }

    #[test]
    fn test_builder_clone() {
        let builder = ContextManager::builder()
            .model("gpt-4o")
            .reserve_tokens(5000);
        let cloned = builder.clone();
        let manager = cloned.build();
        assert_eq!(manager.reserve_tokens(), 5000);
    }

    #[test]
    fn test_explicit_context_limit() {
        let manager = ContextManager::builder()
            .model("gpt-4")
            .context_limit(50_000)
            .build();

        assert_eq!(manager.context_limit(), 50_000);
    }

    // ===== ContextManager Methods Tests =====

    #[test]
    fn test_context_manager_for_model() {
        let manager = ContextManager::for_model("gpt-4o");
        assert_eq!(manager.model(), "gpt-4o");
        assert_eq!(manager.context_limit(), 128_000);
    }

    #[test]
    fn test_context_manager_limits() {
        let manager = ContextManager::for_model("gpt-4o");
        let limits = manager.limits();
        assert_eq!(limits.context_window, 128_000);
        assert_eq!(limits.max_output, Some(16_384));
    }

    #[test]
    fn test_context_manager_debug() {
        let manager = ContextManager::for_model("gpt-4");
        let debug = format!("{:?}", manager);
        assert!(debug.contains("ContextManager"));
        assert!(debug.contains("gpt-4"));
    }

    // ===== Token Counting Tests =====

    #[test]
    fn test_token_counting() {
        let manager = ContextManager::for_model("gpt-4");

        // Basic text
        let count = manager.count_tokens("Hello, world!");
        assert!(count > 0);
        assert!(count < 10); // Should be ~4 tokens
    }

    #[test]
    fn test_token_counting_empty_string() {
        let manager = ContextManager::for_model("gpt-4");
        let count = manager.count_tokens("");
        assert_eq!(count, 0);
    }

    #[test]
    fn test_token_counting_long_text() {
        let manager = ContextManager::for_model("gpt-4");
        let long_text = "word ".repeat(1000);
        let count = manager.count_tokens(&long_text);
        // Each "word " is roughly 1-2 tokens
        assert!(count > 500);
        assert!(count < 3000);
    }

    #[test]
    fn test_token_counting_unicode() {
        let manager = ContextManager::for_model("gpt-4");
        let unicode_text = "Hello 你好 مرحبا 🎉";
        let count = manager.count_tokens(unicode_text);
        assert!(count > 0);
    }

    #[test]
    fn test_token_counting_special_characters() {
        let manager = ContextManager::for_model("gpt-4");
        let special = "!@#$%^&*()_+-=[]{}|;':\",./<>?";
        let count = manager.count_tokens(special);
        assert!(count > 0);
    }

    #[test]
    fn test_token_counting_whitespace() {
        let manager = ContextManager::for_model("gpt-4");
        let whitespace = "   \t\n\r   ";
        let count = manager.count_tokens(whitespace);
        assert!(count > 0);
    }

    #[test]
    fn test_message_token_counting() {
        let manager = ContextManager::for_model("gpt-4");

        let msg = Message::human("Hello, world!");
        let count = manager.count_message_tokens(&msg);

        // Content tokens + overhead
        assert!(count > 0);
    }

    #[test]
    fn test_message_token_counting_all_types() {
        let manager = ContextManager::for_model("gpt-4");

        let human = Message::human("User message");
        let ai = Message::ai("AI response");
        let system = Message::system("System prompt");

        let human_count = manager.count_message_tokens(&human);
        let ai_count = manager.count_message_tokens(&ai);
        let system_count = manager.count_message_tokens(&system);

        assert!(human_count > 0);
        assert!(ai_count > 0);
        assert!(system_count > 0);
    }

    #[test]
    fn test_count_messages_tokens() {
        let manager = ContextManager::for_model("gpt-4");

        let messages = vec![
            Message::system("You are helpful"),
            Message::human("Hello"),
            Message::ai("Hi there!"),
        ];

        let total = manager.count_messages_tokens(&messages);
        let individual: usize = messages
            .iter()
            .map(|m| manager.count_message_tokens(m))
            .sum();

        // Total should include base overhead (3 tokens)
        assert!(total >= individual);
        assert_eq!(total, individual + 3);
    }

    #[test]
    fn test_count_messages_tokens_empty() {
        let manager = ContextManager::for_model("gpt-4");
        let total = manager.count_messages_tokens(&[]);
        assert_eq!(total, 3); // Just base overhead
    }

    // ===== ContentBlock Token Counting Tests =====

    #[test]
    fn test_count_block_tokens_text() {
        let manager = ContextManager::for_model("gpt-4");
        let block = ContentBlock::Text {
            text: "Hello world".to_string(),
        };
        let count = manager.count_block_tokens(&block);
        assert!(count > 0);
        assert!(count < 10);
    }

    #[test]
    fn test_count_block_tokens_image() {
        let manager = ContextManager::for_model("gpt-4");
        let block = ContentBlock::Image {
            source: dashflow::core::messages::ImageSource::Base64 {
                media_type: "image/png".to_string(),
                data: "base64data".to_string(),
            },
            detail: None,
        };
        let count = manager.count_block_tokens(&block);
        // Images have fixed cost of 765 tokens (high detail estimate)
        assert_eq!(count, 765);
    }

    #[test]
    fn test_count_block_tokens_tool_use() {
        let manager = ContextManager::for_model("gpt-4");
        let block = ContentBlock::ToolUse {
            id: "tool_123".to_string(),
            name: "calculator".to_string(),
            input: serde_json::json!({"expression": "2+2"}),
        };
        let count = manager.count_block_tokens(&block);
        assert!(count > 0);
    }

    #[test]
    fn test_count_block_tokens_tool_result() {
        let manager = ContextManager::for_model("gpt-4");
        let block = ContentBlock::ToolResult {
            tool_use_id: "tool_123".to_string(),
            content: "The result is 4".to_string(),
            is_error: false,
        };
        let count = manager.count_block_tokens(&block);
        assert!(count > 0);
    }

    #[test]
    fn test_count_block_tokens_reasoning() {
        let manager = ContextManager::for_model("gpt-4");
        let block = ContentBlock::Reasoning {
            reasoning: "Let me think about this carefully...".to_string(),
        };
        let count = manager.count_block_tokens(&block);
        assert!(count > 0);
    }

    #[test]
    fn test_count_block_tokens_thinking() {
        let manager = ContextManager::for_model("gpt-4");
        let block = ContentBlock::Thinking {
            thinking: "Analyzing the problem...".to_string(),
            signature: Some("sig123".to_string()),
        };
        let count = manager.count_block_tokens(&block);
        assert!(count > 0);
    }

    #[test]
    fn test_count_block_tokens_redacted_thinking() {
        let manager = ContextManager::for_model("gpt-4");
        let block = ContentBlock::RedactedThinking {
            data: "encrypted_data_here".to_string(),
        };
        let count = manager.count_block_tokens(&block);
        assert!(count > 0);
    }

    // ===== Fit Tests =====

    #[test]
    fn test_fit_messages_no_truncation() {
        let manager = ContextManager::builder()
            .model("gpt-4o")
            .reserve_tokens(1000)
            .build();

        let messages = vec![
            Message::system("You are helpful"),
            Message::human("Hello!"),
            Message::ai("Hi there!"),
        ];

        let result = manager.fit(&messages);

        assert_eq!(result.messages.len(), 3);
        assert_eq!(result.messages_dropped, 0);
        assert!(result.token_count > 0);
    }

    #[test]
    fn test_fit_messages_with_truncation() {
        let manager = ContextManager::builder()
            .model("gpt-4")
            .context_limit(50) // Very small limit to force truncation
            .reserve_tokens(10)
            .build();

        let messages = vec![
            Message::system("You are a helpful assistant that provides detailed responses about various topics."),
            Message::human("Tell me a long story about the history of computing from ancient times to modern era."),
            Message::ai("Computing began with the invention of the abacus thousands of years ago in ancient civilizations."),
            Message::human("Please continue the story with more details about the development."),
            Message::ai("As we moved into the 20th century, electronic computers emerged and changed everything."),
        ];

        let result = manager.fit(&messages);

        // Should truncate some messages due to very tight limit
        assert!(
            result.messages.len() < messages.len()
                || result.token_count <= manager.available_tokens(),
            "Expected truncation or fit: {} messages, {} tokens, {} available",
            result.messages.len(),
            result.token_count,
            manager.available_tokens()
        );
        assert!(result.token_count <= manager.available_tokens());
    }

    #[test]
    fn test_fit_result_fields() {
        let manager = ContextManager::builder()
            .model("gpt-4o")
            .context_limit(10000)
            .reserve_tokens(1000)
            .build();

        let messages = vec![Message::human("Hello!")];
        let result = manager.fit(&messages);

        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages_dropped, 0);
        assert!(result.token_count > 0);
        assert!(result.tokens_remaining > 0);
        assert!(result.tokens_remaining < 10000);
    }

    // ===== Fits Check Tests =====

    #[test]
    fn test_fits_check() {
        let manager = ContextManager::builder()
            .model("gpt-4")
            .context_limit(30) // Very tight limit
            .reserve_tokens(5)
            .build();

        let short_messages = vec![Message::human("Hi")];
        assert!(manager.fits(&short_messages));

        // Make a message that is definitely too long for 25 available tokens
        let long_text = "word ".repeat(100); // ~100+ tokens
        let long_messages = vec![Message::human(long_text)];
        assert!(!manager.fits(&long_messages));
    }

    #[test]
    fn test_fits_empty_messages() {
        let manager = ContextManager::for_model("gpt-4");
        assert!(manager.fits(&[]));
    }

    // ===== Usage Ratio Tests =====

    #[test]
    fn test_usage_ratio() {
        let manager = ContextManager::builder()
            .model("gpt-4")
            .context_limit(1000)
            .reserve_tokens(0)
            .build();

        let messages = vec![Message::human("Hello")];
        let ratio = manager.usage_ratio(&messages);

        assert!(ratio > 0.0);
        assert!(ratio < 1.0);
    }

    #[test]
    fn test_usage_ratio_empty() {
        let manager = ContextManager::builder()
            .model("gpt-4")
            .context_limit(1000)
            .reserve_tokens(0)
            .build();

        let ratio = manager.usage_ratio(&[]);
        // Should be just base overhead / limit
        assert!(ratio > 0.0);
        assert!(ratio < 0.01); // Very small
    }

    #[test]
    fn test_usage_ratio_full() {
        let manager = ContextManager::builder()
            .model("gpt-4")
            .context_limit(50)
            .reserve_tokens(0)
            .build();

        let long_text = "word ".repeat(100);
        let messages = vec![Message::human(long_text)];
        let ratio = manager.usage_ratio(&messages);

        // Should exceed 1.0 since messages don't fit
        assert!(ratio > 1.0);
    }

    // ===== Truncation Strategy Tests =====

    #[test]
    fn test_truncation_strategies() {
        let messages = vec![
            Message::system("System"),
            Message::human("First"),
            Message::ai("Response 1"),
            Message::human("Second"),
            Message::ai("Response 2"),
            Message::human("Third"),
        ];

        // Test drop_oldest
        let manager = ContextManager::builder()
            .model("gpt-4")
            .context_limit(80)
            .reserve_tokens(10)
            .truncation(TruncationStrategy::DropOldest)
            .build();

        let result = manager.fit(&messages);
        // Should keep system message and most recent
        assert!(matches!(
            result.messages.first(),
            Some(Message::System { .. })
        ));

        // Test keep_first_and_last
        let manager = ContextManager::builder()
            .model("gpt-4")
            .context_limit(80)
            .reserve_tokens(10)
            .truncation(TruncationStrategy::KeepFirstAndLast)
            .build();

        let result = manager.fit(&messages);
        // Should keep first and last
        if result.messages.len() >= 2 {
            assert!(matches!(
                result.messages.first(),
                Some(Message::System { .. })
            ));
        }
    }

    #[test]
    fn test_truncation_sliding_window() {
        let manager = ContextManager::builder()
            .model("gpt-4")
            .context_limit(60)
            .reserve_tokens(10)
            .truncation(TruncationStrategy::SlidingWindow)
            .build();

        let messages = vec![
            Message::human("First message"),
            Message::ai("First response"),
            Message::human("Second message"),
            Message::ai("Second response"),
        ];

        let result = manager.fit(&messages);
        // Sliding window currently behaves same as drop_oldest
        assert!(result.token_count <= manager.available_tokens());
    }

    #[test]
    fn test_truncation_no_system_message() {
        let manager = ContextManager::builder()
            .model("gpt-4")
            .context_limit(50)
            .reserve_tokens(10)
            .truncation(TruncationStrategy::DropOldest)
            .build();

        // Messages without system message
        let messages = vec![
            Message::human("First"),
            Message::ai("Response"),
            Message::human("Second"),
        ];

        let result = manager.fit(&messages);
        assert!(result.token_count <= manager.available_tokens());
    }

    #[test]
    fn test_truncation_two_messages() {
        let manager = ContextManager::builder()
            .model("gpt-4")
            .context_limit(100)
            .reserve_tokens(10)
            .truncation(TruncationStrategy::KeepFirstAndLast)
            .build();

        let messages = vec![Message::system("System"), Message::human("User")];

        let result = manager.fit(&messages);
        // With only 2 messages, should fall back to drop_oldest
        assert!(result.messages.len() <= 2);
    }

    // ===== Empty Messages Tests =====

    #[test]
    fn test_empty_messages() {
        let manager = ContextManager::for_model("gpt-4");

        let result = manager.fit(&[]);
        assert_eq!(result.messages.len(), 0);
        assert_eq!(result.messages_dropped, 0);
    }

    // ===== Available Tokens Tests =====

    #[test]
    fn test_available_tokens() {
        let manager = ContextManager::builder()
            .model("gpt-4")
            .context_limit(1000)
            .reserve_tokens(200)
            .build();

        assert_eq!(manager.available_tokens(), 800);
    }

    #[test]
    fn test_available_tokens_saturating() {
        let manager = ContextManager::builder()
            .model("gpt-4")
            .context_limit(100)
            .reserve_tokens(200) // More reserved than available
            .build();

        // Should saturate to 0, not underflow
        assert_eq!(manager.available_tokens(), 0);
    }

    // ===== Generic IntoLlmMessage API Tests =====

    // Helper struct for testing generic API
    struct TestMessage {
        role: String,
        content: String,
    }

    impl dashflow::core::messages::IntoLlmMessage for TestMessage {
        fn role(&self) -> &str {
            &self.role
        }
        fn content(&self) -> &str {
            &self.content
        }
        fn tool_calls(&self) -> Option<&[dashflow::core::messages::ToolCall]> {
            None
        }
        fn tool_call_id(&self) -> Option<&str> {
            None
        }
    }

    #[test]
    fn test_count_llm_message_tokens() {
        let manager = ContextManager::for_model("gpt-4");
        let msg = TestMessage {
            role: "user".to_string(),
            content: "Hello, world!".to_string(),
        };
        let count = manager.count_llm_message_tokens(&msg);
        assert!(count > 0);
    }

    #[test]
    fn test_count_llm_messages_tokens() {
        let manager = ContextManager::for_model("gpt-4");
        let messages = vec![
            TestMessage {
                role: "system".to_string(),
                content: "You are helpful".to_string(),
            },
            TestMessage {
                role: "user".to_string(),
                content: "Hello!".to_string(),
            },
        ];
        let count = manager.count_llm_messages_tokens(&messages);
        assert!(count > 0);
        // Should include base overhead
        assert!(count > messages.len());
    }

    #[test]
    fn test_count_llm_messages_tokens_empty() {
        let manager = ContextManager::for_model("gpt-4");
        let messages: Vec<TestMessage> = vec![];
        let count = manager.count_llm_messages_tokens(&messages);
        assert_eq!(count, 3); // Just base overhead
    }

    #[test]
    fn test_fits_llm_messages() {
        let manager = ContextManager::builder()
            .model("gpt-4")
            .context_limit(100) // Very small limit
            .reserve_tokens(10)
            .build();

        let short_messages = vec![TestMessage {
            role: "user".to_string(),
            content: "Hi".to_string(),
        }];
        assert!(manager.fits_llm_messages(&short_messages));

        // Make content definitely too long (5000 "word " = ~5000+ tokens)
        let long_content = "word ".repeat(5000);
        let long_messages = vec![TestMessage {
            role: "user".to_string(),
            content: long_content,
        }];
        assert!(!manager.fits_llm_messages(&long_messages));
    }

    // ===== Edge Cases =====

    #[test]
    fn test_very_large_context_limit() {
        let manager = ContextManager::builder()
            .model("gemini-1.5-pro")
            .build();

        // Gemini has 2M+ context
        assert_eq!(manager.context_limit(), 2_097_152);
        assert!(manager.available_tokens() > 2_000_000);
    }

    #[test]
    fn test_zero_reserve_tokens() {
        let manager = ContextManager::builder()
            .model("gpt-4")
            .context_limit(1000)
            .reserve_tokens(0)
            .build();

        assert_eq!(manager.available_tokens(), 1000);
    }

    #[test]
    fn test_single_message_truncation() {
        let manager = ContextManager::builder()
            .model("gpt-4")
            .context_limit(20)
            .reserve_tokens(5)
            .truncation(TruncationStrategy::DropOldest)
            .build();

        let messages = vec![Message::system("Short")];
        let result = manager.fit(&messages);

        // Single message should fit if small enough
        if manager.count_messages_tokens(&messages) <= manager.available_tokens() {
            assert_eq!(result.messages.len(), 1);
        }
    }

    #[test]
    fn test_model_with_output_limit() {
        let manager = ContextManager::for_model("gpt-4o");
        let limits = manager.limits();
        assert_eq!(limits.max_output, Some(16_384));
    }

    #[test]
    fn test_model_unknown_fallback_limits() {
        // Unknown model falls back to ModelLimits::new(8192) with no output limit
        let manager = ContextManager::for_model("completely-unknown-xyz-model");
        let limits = manager.limits();
        assert_eq!(limits.context_window, 8192);
        // Fallback uses ModelLimits::new() which has no explicit output limit
        assert_eq!(limits.max_output, None);
    }
}
