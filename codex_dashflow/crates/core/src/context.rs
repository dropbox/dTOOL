//! Context utilities for managing LLM context windows
//!
//! This module provides application-specific utilities that complement
//! DashFlow's context management. For token counting and message truncation,
//! use `dashflow_context::ContextManager` directly.
//!
//! ## What's here vs DashFlow
//!
//! **Use DashFlow directly:**
//! - Token counting: `ContextManager::count_tokens()`, `count_message_tokens()`
//! - Message truncation: `ContextManager::fit()` with `TruncationStrategy`
//! - Model limits: `ContextManager::context_limit()`, `available_tokens()`
//!
//! **This module provides:**
//! - `TruncationPolicy`: Bytes vs Tokens policy enum (application-specific)
//! - `truncate_text()`: Text truncation preserving start/end (not in DashFlow)
//! - `TruncationConfig`: Application-specific config struct

use crate::state::Message;
use dashflow_context::TruncationStrategy;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Re-export DashFlow's context manager for convenience
pub use dashflow_context::ContextManager;

/// Global context manager singleton for default model (GPT-4)
static DEFAULT_CONTEXT_MANAGER: OnceLock<ContextManager> = OnceLock::new();

/// Get the default context manager (lazily initialized)
fn default_context_manager() -> &'static ContextManager {
    DEFAULT_CONTEXT_MANAGER.get_or_init(|| ContextManager::for_model("gpt-4"))
}

/// Default context window budget in tokens
pub const DEFAULT_CONTEXT_BUDGET: usize = 6000;

/// Approximate bytes per token for rough conversions
const APPROX_BYTES_PER_TOKEN: usize = 4;

/// Truncation policy specifying how to truncate content
///
/// This is application-specific: DashFlow's `TruncationStrategy` handles
/// message-level truncation, while this handles text content limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TruncationPolicy {
    /// Truncate by byte count
    Bytes(usize),
    /// Truncate by approximate token count
    Tokens(usize),
}

impl Default for TruncationPolicy {
    fn default() -> Self {
        Self::Tokens(DEFAULT_CONTEXT_BUDGET)
    }
}

impl TruncationPolicy {
    /// Create a new byte-based truncation policy
    pub fn bytes(limit: usize) -> Self {
        Self::Bytes(limit)
    }

    /// Create a new token-based truncation policy
    pub fn tokens(limit: usize) -> Self {
        Self::Tokens(limit)
    }

    /// Scale the underlying budget by `multiplier`, rounding up
    pub fn scale(self, multiplier: f64) -> Self {
        match self {
            TruncationPolicy::Bytes(bytes) => {
                TruncationPolicy::Bytes((bytes as f64 * multiplier).ceil() as usize)
            }
            TruncationPolicy::Tokens(tokens) => {
                TruncationPolicy::Tokens((tokens as f64 * multiplier).ceil() as usize)
            }
        }
    }

    /// Returns the token budget derived from this policy
    pub fn token_budget(&self) -> usize {
        match self {
            TruncationPolicy::Bytes(bytes) => bytes_to_tokens(*bytes),
            TruncationPolicy::Tokens(tokens) => *tokens,
        }
    }

    /// Returns the byte budget derived from this policy
    pub fn byte_budget(&self) -> usize {
        match self {
            TruncationPolicy::Bytes(bytes) => *bytes,
            TruncationPolicy::Tokens(tokens) => tokens_to_bytes(*tokens),
        }
    }
}

/// Convert bytes to approximate token count
fn bytes_to_tokens(bytes: usize) -> usize {
    bytes.saturating_add(APPROX_BYTES_PER_TOKEN.saturating_sub(1)) / APPROX_BYTES_PER_TOKEN
}

/// Convert tokens to approximate byte count
fn tokens_to_bytes(tokens: usize) -> usize {
    tokens.saturating_mul(APPROX_BYTES_PER_TOKEN)
}

/// Approximate token count using byte-based heuristic
///
/// For accurate tiktoken-based counting, use `ContextManager::count_tokens()`.
pub fn approx_token_count(text: &str) -> usize {
    bytes_to_tokens(text.len())
}

/// Approximate bytes needed for a given token count
pub fn approx_bytes_for_tokens(tokens: usize) -> usize {
    tokens_to_bytes(tokens)
}

/// Count tokens for a message list using DashFlow's ContextManager
///
/// Uses the IntoLlmMessage trait implementation from state.rs
pub fn messages_token_count(messages: &[Message]) -> usize {
    default_context_manager().count_llm_messages_tokens(messages)
}

/// Truncate text preserving beginning and end portions
///
/// Returns the truncated text with a marker showing how much was removed.
/// This is NOT a DashFlow wrapper - DashFlow provides message-level truncation,
/// not text-level truncation with start/end preservation.
pub fn truncate_text(content: &str, policy: TruncationPolicy) -> String {
    let max_bytes = policy.byte_budget();

    if content.len() <= max_bytes {
        return content.to_string();
    }

    if max_bytes == 0 {
        let removed = removed_units_for_policy(policy, content.len());
        return format_truncation_marker(policy, removed);
    }

    // Split budget between beginning and end
    let left_budget = max_bytes / 2;
    let right_budget = max_bytes - left_budget;

    let (removed_chars, left, right) = split_string(content, left_budget, right_budget);

    let removed = removed_units_for_policy(policy, content.len().saturating_sub(max_bytes));
    let marker = format_truncation_marker(policy, removed.max(removed_chars as u64));

    assemble_truncated_output(left, right, &marker)
}

/// Truncate text with a formatted header showing original line count
pub fn formatted_truncate_text(content: &str, policy: TruncationPolicy) -> String {
    if content.len() <= policy.byte_budget() {
        return content.to_string();
    }
    let total_lines = content.lines().count();
    let result = truncate_text(content, policy);
    format!("Total output lines: {total_lines}\n\n{result}")
}

/// Split a string preserving UTF-8 boundaries
fn split_string(s: &str, beginning_bytes: usize, end_bytes: usize) -> (usize, &str, &str) {
    if s.is_empty() {
        return (0, "", "");
    }

    let len = s.len();
    let tail_start_target = len.saturating_sub(end_bytes);
    let mut prefix_end = 0usize;
    let mut suffix_start = len;
    let mut removed_chars = 0usize;
    let mut suffix_started = false;

    for (idx, ch) in s.char_indices() {
        let char_end = idx + ch.len_utf8();
        if char_end <= beginning_bytes {
            prefix_end = char_end;
            continue;
        }

        if idx >= tail_start_target {
            if !suffix_started {
                suffix_start = idx;
                suffix_started = true;
            }
            continue;
        }

        removed_chars = removed_chars.saturating_add(1);
    }

    if suffix_start < prefix_end {
        suffix_start = prefix_end;
    }

    let before = &s[..prefix_end];
    let after = &s[suffix_start..];

    (removed_chars, before, after)
}

fn format_truncation_marker(policy: TruncationPolicy, removed_count: u64) -> String {
    match policy {
        TruncationPolicy::Tokens(_) => format!("…{removed_count} tokens truncated…"),
        TruncationPolicy::Bytes(_) => format!("…{removed_count} chars truncated…"),
    }
}

fn removed_units_for_policy(policy: TruncationPolicy, removed_bytes: usize) -> u64 {
    match policy {
        TruncationPolicy::Tokens(_) => bytes_to_tokens(removed_bytes) as u64,
        TruncationPolicy::Bytes(_) => removed_bytes as u64,
    }
}

fn assemble_truncated_output(prefix: &str, suffix: &str, marker: &str) -> String {
    let mut out = String::with_capacity(prefix.len() + marker.len() + suffix.len() + 1);
    out.push_str(prefix);
    out.push_str(marker);
    out.push_str(suffix);
    out
}

/// Configuration for context truncation
#[derive(Debug, Clone)]
pub struct TruncationConfig {
    /// Maximum tokens for the context window
    pub max_context_tokens: usize,
    /// Reserved tokens for the response
    pub reserved_response_tokens: usize,
    /// Policy for truncating individual tool outputs
    pub tool_output_policy: TruncationPolicy,
}

impl Default for TruncationConfig {
    fn default() -> Self {
        Self {
            max_context_tokens: 128_000, // GPT-4-turbo context window
            reserved_response_tokens: 4_096,
            tool_output_policy: TruncationPolicy::Tokens(8_000),
        }
    }
}

impl TruncationConfig {
    /// Create a config for a specific model based on its context window size
    ///
    /// This addresses Audit #40: Model-specific context limits.
    /// Returns appropriate truncation config based on known model context windows.
    pub fn for_model(model: &str) -> Self {
        let lower = model.to_lowercase();

        // Claude models
        if lower.contains("claude-3-opus") {
            // Claude 3 Opus: 200k context
            Self {
                max_context_tokens: 200_000,
                reserved_response_tokens: 4_096,
                tool_output_policy: TruncationPolicy::Tokens(16_000),
            }
        } else if lower.contains("claude-3-5-sonnet")
            || lower.contains("claude-3.5-sonnet")
            || lower.contains("claude-3-sonnet")
        {
            // Claude 3/3.5 Sonnet: 200k context
            Self {
                max_context_tokens: 200_000,
                reserved_response_tokens: 4_096,
                tool_output_policy: TruncationPolicy::Tokens(16_000),
            }
        } else if lower.contains("claude-3-haiku") {
            // Claude 3 Haiku: 200k context
            Self {
                max_context_tokens: 200_000,
                reserved_response_tokens: 4_096,
                tool_output_policy: TruncationPolicy::Tokens(16_000),
            }
        } else if lower.contains("claude") {
            // Other Claude models - assume 100k for safety
            Self {
                max_context_tokens: 100_000,
                reserved_response_tokens: 4_096,
                tool_output_policy: TruncationPolicy::Tokens(8_000),
            }
        }
        // GPT-4 variants
        else if lower.contains("gpt-4-turbo")
            || lower.contains("gpt-4-1106")
            || lower.contains("gpt-4-0125")
        {
            // GPT-4 Turbo: 128k context
            Self {
                max_context_tokens: 128_000,
                reserved_response_tokens: 4_096,
                tool_output_policy: TruncationPolicy::Tokens(16_000),
            }
        } else if lower.contains("gpt-4o") {
            // GPT-4o: 128k context
            Self {
                max_context_tokens: 128_000,
                reserved_response_tokens: 4_096,
                tool_output_policy: TruncationPolicy::Tokens(16_000),
            }
        } else if lower.contains("gpt-4-32k") {
            // GPT-4 32k: 32k context
            Self {
                max_context_tokens: 32_000,
                reserved_response_tokens: 4_096,
                tool_output_policy: TruncationPolicy::Tokens(8_000),
            }
        } else if lower.contains("gpt-4") {
            // Original GPT-4: 8k context
            Self {
                max_context_tokens: 8_000,
                reserved_response_tokens: 2_048,
                tool_output_policy: TruncationPolicy::Tokens(2_000),
            }
        }
        // GPT-3.5 variants
        else if lower.contains("gpt-3.5-turbo-16k") {
            // GPT-3.5 Turbo 16k
            Self {
                max_context_tokens: 16_000,
                reserved_response_tokens: 2_048,
                tool_output_policy: TruncationPolicy::Tokens(4_000),
            }
        } else if lower.contains("gpt-3.5") {
            // GPT-3.5 Turbo: 4k-16k (assume 4k for safety)
            Self {
                max_context_tokens: 4_096,
                reserved_response_tokens: 1_024,
                tool_output_policy: TruncationPolicy::Tokens(1_000),
            }
        }
        // O1 models
        else if lower.contains("o1-preview") || lower.contains("o1-mini") {
            // O1 models: 128k context
            Self {
                max_context_tokens: 128_000,
                reserved_response_tokens: 32_768, // O1 can generate longer outputs
                tool_output_policy: TruncationPolicy::Tokens(16_000),
            }
        }
        // Default: assume moderate context for unknown models
        else {
            tracing::debug!(
                model = model,
                "Unknown model, using default 32k context limit"
            );
            Self {
                max_context_tokens: 32_000,
                reserved_response_tokens: 4_096,
                tool_output_policy: TruncationPolicy::Tokens(8_000),
            }
        }
    }

    /// Create a config for models with smaller context windows
    pub fn small_context() -> Self {
        Self {
            max_context_tokens: 8_000,
            reserved_response_tokens: 1_024,
            tool_output_policy: TruncationPolicy::Tokens(2_000),
        }
    }

    /// Create a config for models with large context windows
    pub fn large_context() -> Self {
        Self {
            max_context_tokens: 128_000,
            reserved_response_tokens: 4_096,
            tool_output_policy: TruncationPolicy::Tokens(16_000),
        }
    }

    /// Calculate available tokens for messages
    pub fn available_message_tokens(&self) -> usize {
        self.max_context_tokens
            .saturating_sub(self.reserved_response_tokens)
    }

    /// Truncate messages to fit within the configured budget
    ///
    /// Uses DashFlow's ContextManager with DropOldest strategy internally.
    /// For model-specific tokenization, use `truncate_messages_for_model`.
    pub fn truncate_messages(&self, messages: &[Message]) -> Vec<Message> {
        self.truncate_messages_for_model(messages, "gpt-4")
    }

    /// Truncate messages for a specific model
    ///
    /// Uses DashFlow's ContextManager with model-specific tokenization.
    /// This addresses Audit #40: Model-specific context limits.
    pub fn truncate_messages_for_model(&self, messages: &[Message], model: &str) -> Vec<Message> {
        let manager = ContextManager::builder()
            .model(model)
            .context_limit(self.max_context_tokens)
            .reserve_tokens(self.reserved_response_tokens)
            .truncation(TruncationStrategy::DropOldest)
            .build();

        // Convert our Message type to DashFlow messages for truncation
        // Since Message implements IntoLlmMessage, we can check if truncation is needed
        let available = manager.available_tokens();
        let total_tokens = manager.count_llm_messages_tokens(messages);

        if total_tokens <= available {
            return messages.to_vec();
        }

        // Need to truncate - drop oldest while keeping system message
        let mut result = Vec::with_capacity(messages.len());

        // Always keep system message if present
        let (system_msg, rest) = if !messages.is_empty()
            && matches!(messages[0].role, crate::state::MessageRole::System)
        {
            (Some(&messages[0]), &messages[1..])
        } else {
            (None, messages)
        };

        let system_tokens = system_msg.map_or(0, |m| manager.count_llm_message_tokens(m));
        let remaining_budget = available.saturating_sub(system_tokens);

        if let Some(sys) = system_msg {
            result.push(sys.clone());
        }

        // Add truncation notice placeholder budget
        let truncation_notice_tokens = 20;
        let content_budget = remaining_budget.saturating_sub(truncation_notice_tokens);

        // Keep most recent messages working backwards
        let mut kept: Vec<&Message> = Vec::new();
        let mut used_tokens = 0;

        for msg in rest.iter().rev() {
            let msg_tokens = manager.count_llm_message_tokens(msg);
            if used_tokens + msg_tokens <= content_budget {
                kept.push(msg);
                used_tokens += msg_tokens;
            } else {
                break;
            }
        }

        // Add truncation notice if messages were dropped
        let dropped_count = rest.len() - kept.len();
        if dropped_count > 0 {
            result.push(Message::system(format!(
                "[Context truncated: {} earlier messages omitted to fit context window]",
                dropped_count
            )));
        }

        // Reverse to restore chronological order
        for msg in kept.into_iter().rev() {
            result.push(msg.clone());
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncation_policy_default() {
        let policy = TruncationPolicy::default();
        assert_eq!(policy, TruncationPolicy::Tokens(DEFAULT_CONTEXT_BUDGET));
    }

    #[test]
    fn test_truncation_policy_bytes_constructor() {
        let policy = TruncationPolicy::bytes(1000);
        assert_eq!(policy, TruncationPolicy::Bytes(1000));
    }

    #[test]
    fn test_truncation_policy_tokens_constructor() {
        let policy = TruncationPolicy::tokens(500);
        assert_eq!(policy, TruncationPolicy::Tokens(500));
    }

    #[test]
    fn test_truncation_policy_token_budget() {
        assert_eq!(TruncationPolicy::Tokens(100).token_budget(), 100);
        assert_eq!(TruncationPolicy::Bytes(400).token_budget(), 100);
    }

    #[test]
    fn test_truncation_policy_byte_budget() {
        assert_eq!(TruncationPolicy::Tokens(100).byte_budget(), 400);
        assert_eq!(TruncationPolicy::Bytes(400).byte_budget(), 400);
    }

    #[test]
    fn test_truncation_policy_scale() {
        assert_eq!(
            TruncationPolicy::Tokens(100).scale(0.5),
            TruncationPolicy::Tokens(50)
        );
        assert_eq!(
            TruncationPolicy::Bytes(100).scale(2.0),
            TruncationPolicy::Bytes(200)
        );
    }

    #[test]
    fn test_approx_token_count() {
        assert_eq!(approx_token_count(""), 0);
        assert_eq!(approx_token_count("a"), 1);
        assert_eq!(approx_token_count("abcd"), 1);
        assert_eq!(approx_token_count("abcde"), 2);
    }

    #[test]
    fn test_truncate_text_under_limit() {
        let content = "short text";
        let result = truncate_text(content, TruncationPolicy::Bytes(100));
        assert_eq!(result, content);
    }

    #[test]
    fn test_truncate_text_over_limit() {
        let content = "this is a much longer text that should be truncated";
        let result = truncate_text(content, TruncationPolicy::Bytes(20));
        assert!(result.len() < content.len());
        assert!(result.contains("truncated"));
    }

    #[test]
    fn test_truncate_text_preserves_utf8() {
        let content = "😀😀😀😀😀😀😀😀😀😀";
        let result = truncate_text(content, TruncationPolicy::Bytes(20));
        // Should not panic and should produce valid UTF-8
        assert!(result.is_ascii() || result.chars().count() > 0);
    }

    #[test]
    fn test_truncate_text_zero_budget() {
        let content = "some text";
        let result = truncate_text(content, TruncationPolicy::Bytes(0));
        assert!(result.contains("truncated"));
        assert!(!result.contains("some"));
    }

    #[test]
    fn test_truncation_config_default() {
        let config = TruncationConfig::default();
        assert_eq!(config.max_context_tokens, 128_000);
        assert_eq!(config.reserved_response_tokens, 4_096);
    }

    #[test]
    fn test_truncation_config_available_tokens() {
        let config = TruncationConfig {
            max_context_tokens: 10_000,
            reserved_response_tokens: 2_000,
            tool_output_policy: TruncationPolicy::Tokens(1_000),
        };
        assert_eq!(config.available_message_tokens(), 8_000);
    }

    #[test]
    fn test_split_string_basic() {
        let (removed, left, right) = split_string("hello world", 5, 5);
        assert_eq!(left, "hello");
        assert_eq!(right, "world");
        assert_eq!(removed, 1);
    }

    #[test]
    fn test_split_string_empty() {
        let (removed, left, right) = split_string("", 5, 5);
        assert_eq!(removed, 0);
        assert_eq!(left, "");
        assert_eq!(right, "");
    }

    // Tests for TruncationConfig::for_model (Audit #40)

    #[test]
    fn test_truncation_config_for_gpt4_turbo() {
        let config = TruncationConfig::for_model("gpt-4-turbo");
        assert_eq!(config.max_context_tokens, 128_000);
        assert_eq!(config.reserved_response_tokens, 4_096);
    }

    #[test]
    fn test_truncation_config_for_gpt4_turbo_variants() {
        let config = TruncationConfig::for_model("gpt-4-1106-preview");
        assert_eq!(config.max_context_tokens, 128_000);

        let config = TruncationConfig::for_model("gpt-4-0125-preview");
        assert_eq!(config.max_context_tokens, 128_000);
    }

    #[test]
    fn test_truncation_config_for_gpt4o() {
        let config = TruncationConfig::for_model("gpt-4o");
        assert_eq!(config.max_context_tokens, 128_000);

        let config = TruncationConfig::for_model("gpt-4o-mini");
        assert_eq!(config.max_context_tokens, 128_000);
    }

    #[test]
    fn test_truncation_config_for_gpt4_32k() {
        let config = TruncationConfig::for_model("gpt-4-32k");
        assert_eq!(config.max_context_tokens, 32_000);
    }

    #[test]
    fn test_truncation_config_for_gpt4_original() {
        let config = TruncationConfig::for_model("gpt-4");
        assert_eq!(config.max_context_tokens, 8_000);
        assert_eq!(config.reserved_response_tokens, 2_048);
    }

    #[test]
    fn test_truncation_config_for_gpt35() {
        let config = TruncationConfig::for_model("gpt-3.5-turbo");
        assert_eq!(config.max_context_tokens, 4_096);
        assert_eq!(config.reserved_response_tokens, 1_024);
    }

    #[test]
    fn test_truncation_config_for_gpt35_16k() {
        let config = TruncationConfig::for_model("gpt-3.5-turbo-16k");
        assert_eq!(config.max_context_tokens, 16_000);
    }

    #[test]
    fn test_truncation_config_for_claude_opus() {
        let config = TruncationConfig::for_model("claude-3-opus-20240229");
        assert_eq!(config.max_context_tokens, 200_000);
    }

    #[test]
    fn test_truncation_config_for_claude_sonnet() {
        let config = TruncationConfig::for_model("claude-3-sonnet-20240229");
        assert_eq!(config.max_context_tokens, 200_000);

        let config = TruncationConfig::for_model("claude-3-5-sonnet-20240620");
        assert_eq!(config.max_context_tokens, 200_000);

        let config = TruncationConfig::for_model("claude-3.5-sonnet");
        assert_eq!(config.max_context_tokens, 200_000);
    }

    #[test]
    fn test_truncation_config_for_claude_haiku() {
        let config = TruncationConfig::for_model("claude-3-haiku-20240307");
        assert_eq!(config.max_context_tokens, 200_000);
    }

    #[test]
    fn test_truncation_config_for_o1_models() {
        let config = TruncationConfig::for_model("o1-preview");
        assert_eq!(config.max_context_tokens, 128_000);
        assert_eq!(config.reserved_response_tokens, 32_768);

        let config = TruncationConfig::for_model("o1-mini");
        assert_eq!(config.max_context_tokens, 128_000);
    }

    #[test]
    fn test_truncation_config_for_unknown_model() {
        let config = TruncationConfig::for_model("unknown-model-xyz");
        // Should use conservative default
        assert_eq!(config.max_context_tokens, 32_000);
    }

    #[test]
    fn test_truncation_config_case_insensitive() {
        let config1 = TruncationConfig::for_model("GPT-4-TURBO");
        let config2 = TruncationConfig::for_model("gpt-4-turbo");
        assert_eq!(config1.max_context_tokens, config2.max_context_tokens);
    }
}
