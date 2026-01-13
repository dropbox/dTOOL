//! Context Limit Types and Validation for Language Models
//!
//! This module provides token counting and context limit validation for language models,
//! supporting model-specific limits and various handling policies.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;
use tiktoken_rs::{get_bpe_from_model, CoreBPE};

use crate::core::error::{Error, Result};
use crate::core::messages::BaseMessage;

// ============================================================================
// Model Limits
// ============================================================================

/// Model context window limits (in tokens)
#[derive(Debug, Clone, Copy)]
pub struct ModelLimits {
    /// Maximum context window size (input + output)
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

/// Look up model limits by name (supports fuzzy matching for versioned names)
#[must_use]
pub fn lookup_model_limits(model: &str) -> Option<ModelLimits> {
    let limits = get_model_limits();

    // Direct match
    if let Some(&limit) = limits.get(model) {
        return Some(limit);
    }

    // Check specific patterns first (order matters - more specific patterns first)
    let model_lower = model.to_lowercase();

    // OpenAI models (check gpt-4o before gpt-4 to avoid false match)
    if model_lower.contains("gpt-4o") {
        return limits.get("gpt-4o").copied();
    }
    if model_lower.contains("gpt-4-turbo") {
        return limits.get("gpt-4-turbo").copied();
    }
    if model_lower.contains("gpt-4-32k") {
        return limits.get("gpt-4-32k").copied();
    }
    if model_lower.contains("gpt-4") {
        return limits.get("gpt-4").copied();
    }
    if model_lower.contains("gpt-3.5") {
        return limits.get("gpt-3.5-turbo").copied();
    }
    if model_lower.contains("o1-mini") {
        return limits.get("o1-mini").copied();
    }
    if model_lower.contains("o1-preview") {
        return limits.get("o1-preview").copied();
    }
    if model_lower.starts_with("o1") {
        return limits.get("o1").copied();
    }
    if model_lower.starts_with("o3") {
        return limits.get("o3-mini").copied();
    }

    // Anthropic models (check more specific patterns first)
    if model_lower.contains("claude-3-5-sonnet") || model_lower.contains("claude-3.5-sonnet") {
        return limits.get("claude-3-5-sonnet").copied();
    }
    if model_lower.contains("claude-3-5-haiku") || model_lower.contains("claude-3.5-haiku") {
        return limits.get("claude-3-5-haiku").copied();
    }
    if model_lower.contains("claude-opus-4") || model_lower.contains("claude-4-opus") {
        return limits.get("claude-opus-4").copied();
    }
    if model_lower.contains("claude-sonnet-4") || model_lower.contains("claude-4-sonnet") {
        return limits.get("claude-sonnet-4").copied();
    }
    if model_lower.contains("claude-3-opus") {
        return limits.get("claude-3-opus").copied();
    }
    if model_lower.contains("claude-3-sonnet") {
        return limits.get("claude-3-sonnet").copied();
    }
    if model_lower.contains("claude-3-haiku") {
        return limits.get("claude-3-haiku").copied();
    }
    if model_lower.contains("claude") {
        return limits.get("claude-3-sonnet").copied();
    }

    // Google models
    if model_lower.contains("gemini-2") {
        return limits.get("gemini-2.0-flash").copied();
    }
    if model_lower.contains("gemini-1.5-flash") {
        return limits.get("gemini-1.5-flash").copied();
    }
    if model_lower.contains("gemini") {
        return limits.get("gemini-1.5-pro").copied();
    }

    // Mistral models
    if model_lower.contains("mistral-large") {
        return limits.get("mistral-large").copied();
    }
    if model_lower.contains("mistral-medium") {
        return limits.get("mistral-medium").copied();
    }
    if model_lower.contains("mistral-small") {
        return limits.get("mistral-small").copied();
    }
    if model_lower.contains("codestral") {
        return limits.get("codestral").copied();
    }

    // DeepSeek models
    if model_lower.contains("deepseek-coder") {
        return limits.get("deepseek-coder").copied();
    }
    if model_lower.contains("deepseek") {
        return limits.get("deepseek-chat").copied();
    }

    None
}

// ============================================================================
// Context Limit Policy
// ============================================================================

/// Policy for handling context limit violations
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ContextLimitPolicy {
    /// Don't check context limits (default for backwards compatibility)
    #[default]
    None,
    /// Log a warning but continue with the request
    Warn,
    /// Return an error if context limit is exceeded
    Error,
}

// ============================================================================
// Token Counting
// ============================================================================

/// Get tiktoken encoder for a model (with fallback)
fn get_encoder_for_model(model: &str) -> Option<CoreBPE> {
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

    // Fallback to cl100k_base for unknown models (reasonable default for modern models)
    tiktoken_rs::cl100k_base().ok()
}

/// Count tokens in text using tiktoken
///
/// Falls back to character-based estimation (~4 chars per token) if tiktoken fails.
#[must_use]
pub fn count_tokens(text: &str, model: Option<&str>) -> usize {
    if let Some(model_name) = model {
        if let Some(encoder) = get_encoder_for_model(model_name) {
            return encoder.encode_with_special_tokens(text).len();
        }
    }
    // Fallback: ~4 characters per token (conservative estimate)
    text.len().div_ceil(4)
}

/// Count tokens in messages
///
/// Accounts for message formatting overhead (~4 tokens per message for role/separators).
#[must_use]
pub fn count_messages_tokens(messages: &[BaseMessage], model: Option<&str>) -> usize {
    const TOKENS_PER_MESSAGE: usize = 4; // role + separators overhead

    messages
        .iter()
        .map(|msg| {
            let content_tokens = count_tokens(&msg.content().as_text(), model);
            content_tokens + TOKENS_PER_MESSAGE
        })
        .sum()
}

/// Validate that messages fit within model context limits
///
/// Returns `Ok(token_count)` if messages fit, or `Err` if limit is exceeded
/// and policy is `Error`.
pub fn validate_context_limit(
    messages: &[BaseMessage],
    model: Option<&str>,
    explicit_limit: Option<usize>,
    reserve_tokens: usize,
    policy: ContextLimitPolicy,
) -> Result<usize> {
    if policy == ContextLimitPolicy::None {
        return Ok(0); // Skip validation
    }

    let token_count = count_messages_tokens(messages, model);

    // Determine the context limit to use
    let context_limit = explicit_limit.or_else(|| {
        model
            .and_then(lookup_model_limits)
            .map(|l| l.context_window)
    });

    let Some(limit) = context_limit else {
        // No limit known - can't validate
        return Ok(token_count);
    };

    // Available budget = context limit - reserved tokens for response
    let available = limit.saturating_sub(reserve_tokens);

    if token_count > available {
        let message = format!(
            "Input tokens ({}) exceed context limit ({} - {} reserved = {} available) for model {}",
            token_count,
            limit,
            reserve_tokens,
            available,
            model.unwrap_or("unknown")
        );

        match policy {
            ContextLimitPolicy::None => unreachable!(),
            ContextLimitPolicy::Warn => {
                tracing::warn!("{}", message);
                Ok(token_count)
            }
            ContextLimitPolicy::Error => Err(Error::ContextLimitExceeded {
                token_count,
                limit: available,
                model: model.unwrap_or("unknown").to_string(),
            }),
        }
    } else {
        Ok(token_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::messages::{AIMessage, HumanMessage};

    // ============================================================================
    // ModelLimits Tests
    // ============================================================================

    #[test]
    fn model_limits_new_creates_with_context_window() {
        let limits = ModelLimits::new(8192);
        assert_eq!(limits.context_window, 8192);
        assert!(limits.max_output.is_none());
    }

    #[test]
    fn model_limits_with_output_creates_with_both_limits() {
        let limits = ModelLimits::with_output(128_000, 16_384);
        assert_eq!(limits.context_window, 128_000);
        assert_eq!(limits.max_output, Some(16_384));
    }

    #[test]
    fn model_limits_clone_works() {
        let limits = ModelLimits::with_output(100_000, 8000);
        let cloned = limits;
        assert_eq!(limits.context_window, cloned.context_window);
        assert_eq!(limits.max_output, cloned.max_output);
    }

    #[test]
    fn model_limits_debug_format() {
        let limits = ModelLimits::with_output(128_000, 16_384);
        let debug = format!("{:?}", limits);
        assert!(debug.contains("128000"));
        assert!(debug.contains("16384"));
    }

    // ============================================================================
    // lookup_model_limits Tests - OpenAI Models
    // ============================================================================

    #[test]
    fn lookup_model_limits_gpt4o_direct() {
        let limits = lookup_model_limits("gpt-4o").unwrap();
        assert_eq!(limits.context_window, 128_000);
        assert_eq!(limits.max_output, Some(16_384));
    }

    #[test]
    fn lookup_model_limits_gpt4o_mini() {
        let limits = lookup_model_limits("gpt-4o-mini").unwrap();
        assert_eq!(limits.context_window, 128_000);
    }

    #[test]
    fn lookup_model_limits_gpt4_turbo() {
        let limits = lookup_model_limits("gpt-4-turbo").unwrap();
        assert_eq!(limits.context_window, 128_000);
        assert_eq!(limits.max_output, Some(4_096));
    }

    #[test]
    fn lookup_model_limits_gpt4_base() {
        let limits = lookup_model_limits("gpt-4").unwrap();
        assert_eq!(limits.context_window, 8_192);
    }

    #[test]
    fn lookup_model_limits_gpt4_32k() {
        let limits = lookup_model_limits("gpt-4-32k").unwrap();
        assert_eq!(limits.context_window, 32_768);
    }

    #[test]
    fn lookup_model_limits_gpt35_turbo() {
        let limits = lookup_model_limits("gpt-3.5-turbo").unwrap();
        assert_eq!(limits.context_window, 16_385);
    }

    #[test]
    fn lookup_model_limits_o1() {
        let limits = lookup_model_limits("o1").unwrap();
        assert_eq!(limits.context_window, 200_000);
        assert_eq!(limits.max_output, Some(100_000));
    }

    #[test]
    fn lookup_model_limits_o1_mini() {
        let limits = lookup_model_limits("o1-mini").unwrap();
        assert_eq!(limits.context_window, 128_000);
    }

    #[test]
    fn lookup_model_limits_o1_preview() {
        let limits = lookup_model_limits("o1-preview").unwrap();
        assert_eq!(limits.context_window, 128_000);
    }

    #[test]
    fn lookup_model_limits_o3_mini() {
        let limits = lookup_model_limits("o3-mini").unwrap();
        assert_eq!(limits.context_window, 200_000);
    }

    // ============================================================================
    // lookup_model_limits Tests - Anthropic Models
    // ============================================================================

    #[test]
    fn lookup_model_limits_claude3_opus() {
        let limits = lookup_model_limits("claude-3-opus").unwrap();
        assert_eq!(limits.context_window, 200_000);
        assert_eq!(limits.max_output, Some(4_096));
    }

    #[test]
    fn lookup_model_limits_claude3_sonnet() {
        let limits = lookup_model_limits("claude-3-sonnet").unwrap();
        assert_eq!(limits.context_window, 200_000);
    }

    #[test]
    fn lookup_model_limits_claude3_haiku() {
        let limits = lookup_model_limits("claude-3-haiku").unwrap();
        assert_eq!(limits.context_window, 200_000);
    }

    #[test]
    fn lookup_model_limits_claude35_sonnet() {
        let limits = lookup_model_limits("claude-3-5-sonnet").unwrap();
        assert_eq!(limits.context_window, 200_000);
        assert_eq!(limits.max_output, Some(8_192));
    }

    #[test]
    fn lookup_model_limits_claude35_haiku() {
        let limits = lookup_model_limits("claude-3-5-haiku").unwrap();
        assert_eq!(limits.context_window, 200_000);
    }

    #[test]
    fn lookup_model_limits_claude_opus_4() {
        let limits = lookup_model_limits("claude-opus-4").unwrap();
        assert_eq!(limits.context_window, 200_000);
        assert_eq!(limits.max_output, Some(32_000));
    }

    #[test]
    fn lookup_model_limits_claude_sonnet_4() {
        let limits = lookup_model_limits("claude-sonnet-4").unwrap();
        assert_eq!(limits.context_window, 200_000);
        assert_eq!(limits.max_output, Some(64_000));
    }

    // ============================================================================
    // lookup_model_limits Tests - Google Models
    // ============================================================================

    #[test]
    fn lookup_model_limits_gemini_15_pro() {
        let limits = lookup_model_limits("gemini-1.5-pro").unwrap();
        assert_eq!(limits.context_window, 2_097_152);
        assert_eq!(limits.max_output, Some(8_192));
    }

    #[test]
    fn lookup_model_limits_gemini_15_flash() {
        let limits = lookup_model_limits("gemini-1.5-flash").unwrap();
        assert_eq!(limits.context_window, 1_048_576);
    }

    #[test]
    fn lookup_model_limits_gemini_20_flash() {
        let limits = lookup_model_limits("gemini-2.0-flash").unwrap();
        assert_eq!(limits.context_window, 1_048_576);
    }

    // ============================================================================
    // lookup_model_limits Tests - Mistral Models
    // ============================================================================

    #[test]
    fn lookup_model_limits_mistral_large() {
        let limits = lookup_model_limits("mistral-large").unwrap();
        assert_eq!(limits.context_window, 128_000);
    }

    #[test]
    fn lookup_model_limits_mistral_medium() {
        let limits = lookup_model_limits("mistral-medium").unwrap();
        assert_eq!(limits.context_window, 32_000);
    }

    #[test]
    fn lookup_model_limits_mistral_small() {
        let limits = lookup_model_limits("mistral-small").unwrap();
        assert_eq!(limits.context_window, 32_000);
    }

    #[test]
    fn lookup_model_limits_codestral() {
        let limits = lookup_model_limits("codestral").unwrap();
        assert_eq!(limits.context_window, 32_000);
    }

    // ============================================================================
    // lookup_model_limits Tests - DeepSeek Models
    // ============================================================================

    #[test]
    fn lookup_model_limits_deepseek_chat() {
        let limits = lookup_model_limits("deepseek-chat").unwrap();
        assert_eq!(limits.context_window, 64_000);
        assert_eq!(limits.max_output, Some(8_000));
    }

    #[test]
    fn lookup_model_limits_deepseek_coder() {
        let limits = lookup_model_limits("deepseek-coder").unwrap();
        assert_eq!(limits.context_window, 64_000);
    }

    // ============================================================================
    // lookup_model_limits Tests - Fuzzy Matching
    // ============================================================================

    #[test]
    fn lookup_model_limits_fuzzy_gpt4o_versioned() {
        // Should match gpt-4o even with version suffix
        let limits = lookup_model_limits("gpt-4o-2024-05-13").unwrap();
        assert_eq!(limits.context_window, 128_000);
    }

    #[test]
    fn lookup_model_limits_fuzzy_gpt4_turbo_versioned() {
        let limits = lookup_model_limits("gpt-4-turbo-2024-04-09").unwrap();
        assert_eq!(limits.context_window, 128_000);
    }

    #[test]
    fn lookup_model_limits_fuzzy_claude_35_sonnet_alt_format() {
        // Should match with alternate dot format
        let limits = lookup_model_limits("claude-3.5-sonnet-20241022").unwrap();
        assert_eq!(limits.context_window, 200_000);
    }

    #[test]
    fn lookup_model_limits_fuzzy_claude_opus_4_alt_format() {
        // Should match claude-4-opus format
        let limits = lookup_model_limits("claude-4-opus").unwrap();
        assert_eq!(limits.context_window, 200_000);
    }

    #[test]
    fn lookup_model_limits_fuzzy_claude_sonnet_4_alt_format() {
        // Should match claude-4-sonnet format
        let limits = lookup_model_limits("claude-4-sonnet").unwrap();
        assert_eq!(limits.context_window, 200_000);
    }

    #[test]
    fn lookup_model_limits_fuzzy_gemini_generic() {
        // Generic gemini should match gemini-1.5-pro
        let limits = lookup_model_limits("gemini-pro-vision").unwrap();
        assert_eq!(limits.context_window, 2_097_152);
    }

    #[test]
    fn lookup_model_limits_fuzzy_gemini_2() {
        let limits = lookup_model_limits("gemini-2.0-pro").unwrap();
        assert_eq!(limits.context_window, 1_048_576);
    }

    #[test]
    fn lookup_model_limits_fuzzy_deepseek_generic() {
        let limits = lookup_model_limits("deepseek-v3").unwrap();
        assert_eq!(limits.context_window, 64_000);
    }

    #[test]
    fn lookup_model_limits_fuzzy_o3_family() {
        let limits = lookup_model_limits("o3-something").unwrap();
        assert_eq!(limits.context_window, 200_000);
    }

    #[test]
    fn lookup_model_limits_fuzzy_claude_generic() {
        // Generic claude should default to claude-3-sonnet
        let limits = lookup_model_limits("claude-instant").unwrap();
        assert_eq!(limits.context_window, 200_000);
    }

    #[test]
    fn lookup_model_limits_unknown_model_returns_none() {
        assert!(lookup_model_limits("unknown-model-xyz").is_none());
    }

    #[test]
    fn lookup_model_limits_case_insensitive() {
        let limits = lookup_model_limits("GPT-4O").unwrap();
        assert_eq!(limits.context_window, 128_000);
    }

    // ============================================================================
    // ContextLimitPolicy Tests
    // ============================================================================

    #[test]
    fn context_limit_policy_default_is_none() {
        let policy = ContextLimitPolicy::default();
        assert_eq!(policy, ContextLimitPolicy::None);
    }

    #[test]
    fn context_limit_policy_equality() {
        assert_eq!(ContextLimitPolicy::None, ContextLimitPolicy::None);
        assert_eq!(ContextLimitPolicy::Warn, ContextLimitPolicy::Warn);
        assert_eq!(ContextLimitPolicy::Error, ContextLimitPolicy::Error);
        assert_ne!(ContextLimitPolicy::None, ContextLimitPolicy::Warn);
        assert_ne!(ContextLimitPolicy::Warn, ContextLimitPolicy::Error);
    }

    #[test]
    fn context_limit_policy_clone() {
        let policy = ContextLimitPolicy::Error;
        let cloned = policy;
        assert_eq!(policy, cloned);
    }

    #[test]
    fn context_limit_policy_debug() {
        let debug = format!("{:?}", ContextLimitPolicy::Warn);
        assert!(debug.contains("Warn"));
    }

    #[test]
    fn context_limit_policy_serde_roundtrip() {
        let policy = ContextLimitPolicy::Warn;
        let json = serde_json::to_string(&policy).unwrap();
        assert_eq!(json, "\"warn\"");
        let parsed: ContextLimitPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, policy);
    }

    #[test]
    fn context_limit_policy_serde_all_variants() {
        assert_eq!(
            serde_json::to_string(&ContextLimitPolicy::None).unwrap(),
            "\"none\""
        );
        assert_eq!(
            serde_json::to_string(&ContextLimitPolicy::Warn).unwrap(),
            "\"warn\""
        );
        assert_eq!(
            serde_json::to_string(&ContextLimitPolicy::Error).unwrap(),
            "\"error\""
        );
    }

    // ============================================================================
    // count_tokens Tests
    // ============================================================================

    #[test]
    fn count_tokens_empty_string() {
        let count = count_tokens("", None);
        assert_eq!(count, 0);
    }

    #[test]
    fn count_tokens_short_text_fallback() {
        // Without model, uses ~4 chars per token fallback
        let count = count_tokens("Hello", None);
        assert!(count > 0);
        assert!(count <= 2); // "Hello" is 5 chars, so 1-2 tokens with fallback
    }

    #[test]
    fn count_tokens_longer_text() {
        let text = "The quick brown fox jumps over the lazy dog.";
        let count = count_tokens(text, None);
        assert!(count > 0);
        assert!(count <= 15); // Reasonable estimate for this sentence
    }

    #[test]
    fn count_tokens_with_model_gpt4() {
        let count = count_tokens("Hello, world!", Some("gpt-4"));
        assert!(count > 0);
        assert!(count <= 5); // Tiktoken should give accurate count
    }

    #[test]
    fn count_tokens_with_model_gpt35() {
        let count = count_tokens("Hello, world!", Some("gpt-3.5-turbo"));
        assert!(count > 0);
    }

    #[test]
    fn count_tokens_unicode_text() {
        let count = count_tokens("こんにちは世界", None);
        assert!(count > 0);
    }

    #[test]
    fn count_tokens_special_characters() {
        let count = count_tokens("!@#$%^&*()", None);
        assert!(count > 0);
    }

    #[test]
    fn count_tokens_whitespace_only() {
        let count = count_tokens("   \t\n  ", None);
        // Whitespace should still produce some tokens
        assert!(count > 0);
    }

    #[test]
    fn count_tokens_unknown_model_uses_fallback() {
        // Unknown model should still work via fallback
        let count = count_tokens("Hello", Some("unknown-model-xyz"));
        assert!(count > 0);
    }

    // ============================================================================
    // count_messages_tokens Tests
    // ============================================================================

    #[test]
    fn count_messages_tokens_empty_list() {
        let messages: Vec<BaseMessage> = vec![];
        let count = count_messages_tokens(&messages, None);
        assert_eq!(count, 0);
    }

    #[test]
    fn count_messages_tokens_single_message() {
        let messages = vec![HumanMessage::new("Hello").into()];
        let count = count_messages_tokens(&messages, None);
        // Should include content tokens + ~4 overhead per message
        assert!(count >= 4);
    }

    #[test]
    fn count_messages_tokens_multiple_messages() {
        let messages = vec![
            AIMessage::new("You are a helpful assistant.").into(),
            HumanMessage::new("What is 2+2?").into(),
        ];
        let count = count_messages_tokens(&messages, None);
        // Should include both messages with overhead
        assert!(count >= 8); // At least 4 overhead per message
    }

    #[test]
    fn count_messages_tokens_with_model() {
        let messages = vec![HumanMessage::new("Hello, world!").into()];
        let count = count_messages_tokens(&messages, Some("gpt-4"));
        assert!(count > 0);
    }

    #[test]
    fn count_messages_tokens_long_conversation() {
        let messages: Vec<BaseMessage> = (0..10)
            .map(|i| HumanMessage::new(format!("Message number {}", i)).into())
            .collect();
        let count = count_messages_tokens(&messages, None);
        // 10 messages * ~4 overhead = 40 minimum
        assert!(count >= 40);
    }

    // ============================================================================
    // validate_context_limit Tests
    // ============================================================================

    #[test]
    fn validate_context_limit_policy_none_skips_validation() {
        let messages = vec![HumanMessage::new("Hello").into()];
        let result = validate_context_limit(&messages, None, None, 0, ContextLimitPolicy::None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0); // Returns 0 when skipped
    }

    #[test]
    fn validate_context_limit_within_limit_ok() {
        let messages = vec![HumanMessage::new("Hello").into()];
        let result = validate_context_limit(
            &messages,
            Some("gpt-4"), // 8192 context
            None,
            0,
            ContextLimitPolicy::Error,
        );
        assert!(result.is_ok());
        let count = result.unwrap();
        assert!(count > 0);
    }

    #[test]
    fn validate_context_limit_explicit_limit_used() {
        let messages = vec![HumanMessage::new("Hello").into()];
        let result = validate_context_limit(
            &messages,
            None,
            Some(1000), // Explicit limit
            0,
            ContextLimitPolicy::Error,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn validate_context_limit_exceeds_with_error_policy() {
        let messages = vec![HumanMessage::new("This is a test message that has some content").into()];
        let result = validate_context_limit(
            &messages,
            None,
            Some(5), // Very small limit
            0,
            ContextLimitPolicy::Error,
        );
        assert!(result.is_err());
    }

    #[test]
    fn validate_context_limit_exceeds_with_warn_policy() {
        let messages = vec![HumanMessage::new("This is a test message that has some content").into()];
        let result = validate_context_limit(
            &messages,
            None,
            Some(5), // Very small limit
            0,
            ContextLimitPolicy::Warn,
        );
        // Warn policy returns Ok even when exceeded
        assert!(result.is_ok());
    }

    #[test]
    fn validate_context_limit_reserve_tokens_respected() {
        let messages = vec![HumanMessage::new("Hi").into()];
        // Set limit to 10, reserve 8, so only 2 available
        let result = validate_context_limit(
            &messages,
            None,
            Some(10),
            8, // Reserve most of the context
            ContextLimitPolicy::Error,
        );
        // Should fail because available is too small
        assert!(result.is_err());
    }

    #[test]
    fn validate_context_limit_no_known_limit_returns_count() {
        let messages = vec![HumanMessage::new("Hello").into()];
        let result = validate_context_limit(
            &messages,
            Some("unknown-model"), // No known limits
            None,                  // No explicit limit
            0,
            ContextLimitPolicy::Error,
        );
        // When no limit is known, just return the count
        assert!(result.is_ok());
        assert!(result.unwrap() > 0);
    }

    #[test]
    fn validate_context_limit_error_contains_details() {
        let messages = vec![HumanMessage::new("Test message content here").into()];
        let result = validate_context_limit(
            &messages,
            Some("gpt-4"),
            Some(5), // Very small limit
            0,
            ContextLimitPolicy::Error,
        );
        match result {
            Err(Error::ContextLimitExceeded {
                token_count,
                limit,
                model,
            }) => {
                assert!(token_count > 0);
                assert_eq!(limit, 5);
                assert_eq!(model, "gpt-4");
            }
            _ => panic!("Expected ContextLimitExceeded error"),
        }
    }

    #[test]
    fn validate_context_limit_empty_messages() {
        let messages: Vec<BaseMessage> = vec![];
        let result = validate_context_limit(
            &messages,
            Some("gpt-4"),
            None,
            0,
            ContextLimitPolicy::Error,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn validate_context_limit_reserve_tokens_saturating() {
        let messages = vec![HumanMessage::new("Hello").into()];
        // Reserve more than the limit - should saturate to 0 available
        let result = validate_context_limit(
            &messages,
            None,
            Some(10),
            100, // Reserve more than limit
            ContextLimitPolicy::Error,
        );
        // Should fail because available saturates to 0
        assert!(result.is_err());
    }

    #[test]
    fn validate_context_limit_model_from_lookup() {
        let messages = vec![HumanMessage::new("Hello").into()];
        // Use a model with known limits but no explicit limit
        let result = validate_context_limit(
            &messages,
            Some("gpt-4o"), // 128k context
            None,           // Use model lookup
            0,
            ContextLimitPolicy::Error,
        );
        assert!(result.is_ok());
    }

    // ============================================================================
    // Edge Case Tests
    // ============================================================================

    #[test]
    fn get_model_limits_static_initialized_once() {
        // Call multiple times to verify OnceLock behavior
        let limits1 = lookup_model_limits("gpt-4");
        let limits2 = lookup_model_limits("gpt-4");
        assert_eq!(limits1.unwrap().context_window, limits2.unwrap().context_window);
    }

    #[test]
    fn count_tokens_very_long_text() {
        let long_text = "word ".repeat(10000);
        let count = count_tokens(&long_text, None);
        assert!(count > 1000); // Should have many tokens
    }

    #[test]
    fn validate_context_limit_integration_gpt4o() {
        // Real-world scenario with gpt-4o limits
        let messages: Vec<BaseMessage> = (0..100)
            .map(|i| HumanMessage::new(format!("Message {}", i)).into())
            .collect();
        let result = validate_context_limit(
            &messages,
            Some("gpt-4o"),
            None,
            1000, // Reserve tokens for response
            ContextLimitPolicy::Error,
        );
        // 100 short messages should fit in 128k context
        assert!(result.is_ok());
    }
}
