//! Conversation compaction for reducing context size
//!
//! When conversations become too long, this module provides functionality to:
//! - Summarize older messages using an LLM
//! - Rebuild conversation history with the summary
//! - Preserve recent user messages within token limits
//!
//! Compaction helps maintain performance by keeping the conversation context
//! within model limits while preserving important context.

use crate::context::{approx_token_count, truncate_text, TruncationPolicy};
use crate::state::{Message, MessageRole};
use serde::{Deserialize, Serialize};

/// Default prompt for summarizing conversations.
pub const SUMMARIZATION_PROMPT: &str = r#"Please provide a concise summary of the conversation so far.
Focus on:
- Key topics discussed
- Important decisions made
- Current state of any tasks
- Relevant context for continuing the conversation

Keep the summary brief but informative."#;

/// Prefix added to summary messages to identify them.
pub const SUMMARY_PREFIX: &str = "[Previous conversation summary]";

/// Maximum tokens for user messages in compacted history.
const COMPACT_USER_MESSAGE_MAX_TOKENS: usize = 20_000;

/// Configuration for compaction behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactConfig {
    /// Maximum tokens to allocate for preserved user messages
    pub user_message_max_tokens: usize,
    /// Custom summarization prompt (None = use default)
    pub summarization_prompt: Option<String>,
    /// Whether to emit warnings about compaction
    pub emit_warnings: bool,
}

impl Default for CompactConfig {
    fn default() -> Self {
        Self {
            user_message_max_tokens: COMPACT_USER_MESSAGE_MAX_TOKENS,
            summarization_prompt: None,
            emit_warnings: true,
        }
    }
}

impl CompactConfig {
    /// Create a config with a custom user message token limit.
    pub fn with_user_message_max_tokens(mut self, tokens: usize) -> Self {
        self.user_message_max_tokens = tokens;
        self
    }

    /// Set a custom summarization prompt.
    pub fn with_summarization_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.summarization_prompt = Some(prompt.into());
        self
    }

    /// Get the summarization prompt to use.
    pub fn get_summarization_prompt(&self) -> &str {
        self.summarization_prompt
            .as_deref()
            .unwrap_or(SUMMARIZATION_PROMPT)
    }
}

/// Result of a compaction operation.
#[derive(Debug, Clone)]
pub struct CompactionResult {
    /// The new compacted conversation history
    pub history: Vec<Message>,
    /// The summary text that was generated
    pub summary: String,
    /// Original message count before compaction
    pub original_count: usize,
    /// New message count after compaction
    pub compacted_count: usize,
    /// Estimated tokens saved
    pub tokens_saved: usize,
}

/// Collect user messages from conversation history.
///
/// Filters out summary messages and system messages.
pub fn collect_user_messages(messages: &[Message]) -> Vec<String> {
    messages
        .iter()
        .filter_map(|msg| {
            if msg.role != MessageRole::User {
                return None;
            }
            if is_summary_message(&msg.content) {
                return None;
            }
            Some(msg.content.clone())
        })
        .collect()
}

/// Check if a message is a summary message (from previous compaction).
pub fn is_summary_message(message: &str) -> bool {
    message.starts_with(SUMMARY_PREFIX) || message.starts_with(&format!("{SUMMARY_PREFIX}\n"))
}

/// Build a compacted history from an initial context, user messages, and summary.
///
/// This function:
/// 1. Starts with the initial context (system messages, etc.)
/// 2. Adds recent user messages (within token limits, newest first)
/// 3. Appends the summary as the final message
///
/// # Arguments
/// * `initial_context` - Messages to preserve at the start (system prompt, etc.)
/// * `user_messages` - User messages from the conversation
/// * `summary_text` - The summary of the conversation
pub fn build_compacted_history(
    initial_context: Vec<Message>,
    user_messages: &[String],
    summary_text: &str,
) -> Vec<Message> {
    build_compacted_history_with_limit(
        initial_context,
        user_messages,
        summary_text,
        COMPACT_USER_MESSAGE_MAX_TOKENS,
    )
}

/// Build compacted history with a custom token limit for user messages.
pub fn build_compacted_history_with_limit(
    mut history: Vec<Message>,
    user_messages: &[String],
    summary_text: &str,
    max_tokens: usize,
) -> Vec<Message> {
    // Select user messages from newest to oldest, within token budget
    let mut selected_messages: Vec<String> = Vec::new();
    if max_tokens > 0 {
        let mut remaining = max_tokens;
        for message in user_messages.iter().rev() {
            if remaining == 0 {
                break;
            }
            let tokens = approx_token_count(message);
            if tokens <= remaining {
                selected_messages.push(message.clone());
                remaining = remaining.saturating_sub(tokens);
            } else {
                // Truncate the message to fit
                let truncated = truncate_text(message, TruncationPolicy::Tokens(remaining));
                selected_messages.push(truncated);
                break;
            }
        }
        // Reverse to restore chronological order
        selected_messages.reverse();
    }

    // Add selected user messages to history
    for message in &selected_messages {
        history.push(Message {
            role: MessageRole::User,
            content: message.clone(),
            tool_call_id: None,
            tool_calls: vec![],
        });
    }

    // Add the summary as the final message
    let summary_text = if summary_text.is_empty() {
        "(no summary available)".to_string()
    } else {
        format!("{SUMMARY_PREFIX}\n{summary_text}")
    };

    history.push(Message {
        role: MessageRole::User,
        content: summary_text,
        tool_call_id: None,
        tool_calls: vec![],
    });

    history
}

/// Calculate the number of tokens in a message list.
pub fn estimate_history_tokens(messages: &[Message]) -> usize {
    messages
        .iter()
        .map(|msg| approx_token_count(&msg.content))
        .sum()
}

/// Determine if compaction is needed based on token count and threshold.
pub fn should_compact(messages: &[Message], token_threshold: usize) -> bool {
    if token_threshold == 0 {
        return false;
    }
    estimate_history_tokens(messages) > token_threshold
}

/// Extract text content from messages, joining with newlines.
pub fn messages_to_text(messages: &[Message]) -> String {
    messages
        .iter()
        .map(|msg| {
            let role = match msg.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool => "tool",
            };
            format!("[{}]: {}", role, msg.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Create a compaction prompt that includes the conversation context.
pub fn create_compaction_prompt(messages: &[Message], custom_prompt: Option<&str>) -> String {
    let prompt = custom_prompt.unwrap_or(SUMMARIZATION_PROMPT);
    let context = messages_to_text(messages);
    format!("{prompt}\n\n---\n\nConversation to summarize:\n\n{context}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_user_message(content: &str) -> Message {
        Message {
            role: MessageRole::User,
            content: content.to_string(),
            tool_call_id: None,
            tool_calls: vec![],
        }
    }

    fn make_assistant_message(content: &str) -> Message {
        Message {
            role: MessageRole::Assistant,
            content: content.to_string(),
            tool_call_id: None,
            tool_calls: vec![],
        }
    }

    #[test]
    fn test_collect_user_messages() {
        let messages = vec![
            make_user_message("first user"),
            make_assistant_message("assistant response"),
            make_user_message("second user"),
        ];

        let collected = collect_user_messages(&messages);
        assert_eq!(collected, vec!["first user", "second user"]);
    }

    #[test]
    fn test_collect_user_messages_filters_summaries() {
        let messages = vec![
            make_user_message("[Previous conversation summary]\nOld summary"),
            make_user_message("real message"),
        ];

        let collected = collect_user_messages(&messages);
        assert_eq!(collected, vec!["real message"]);
    }

    #[test]
    fn test_is_summary_message() {
        assert!(is_summary_message("[Previous conversation summary]"));
        assert!(is_summary_message(
            "[Previous conversation summary]\nContent"
        ));
        assert!(!is_summary_message("Regular message"));
    }

    #[test]
    fn test_build_compacted_history_basic() {
        let initial = vec![Message {
            role: MessageRole::System,
            content: "System prompt".to_string(),
            tool_call_id: None,
            tool_calls: vec![],
        }];
        let user_messages = vec!["user msg 1".to_string(), "user msg 2".to_string()];
        let summary = "This is a summary";

        let result = build_compacted_history(initial, &user_messages, summary);

        // Should have: system + 2 user messages + summary
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].role, MessageRole::System);
        assert_eq!(result[1].content, "user msg 1");
        assert_eq!(result[2].content, "user msg 2");
        assert!(result[3].content.contains("summary"));
    }

    #[test]
    fn test_build_compacted_history_with_truncation() {
        let initial = vec![];
        // Create a long message
        let long_message = "word ".repeat(5000);
        let user_messages = vec![long_message];
        let summary = "Summary";

        // Use a small token limit
        let result = build_compacted_history_with_limit(initial, &user_messages, summary, 100);

        // Should have truncated user message + summary
        assert_eq!(result.len(), 2);
        // The message should be truncated
        assert!(result[0].content.contains("truncated"));
    }

    #[test]
    fn test_build_compacted_history_empty_summary() {
        let initial = vec![];
        let user_messages = vec!["message".to_string()];
        let summary = "";

        let result = build_compacted_history(initial, &user_messages, summary);

        // Summary should be replaced with default text
        let last = result.last().unwrap();
        assert!(last.content.contains("no summary available"));
    }

    #[test]
    fn test_should_compact() {
        let messages = vec![
            make_user_message("short"),
            make_assistant_message("response"),
        ];

        // Low threshold should trigger compaction
        assert!(should_compact(&messages, 1));

        // High threshold should not trigger
        assert!(!should_compact(&messages, 10000));

        // Zero threshold never triggers
        assert!(!should_compact(&messages, 0));
    }

    #[test]
    fn test_estimate_history_tokens() {
        let messages = vec![
            make_user_message("hello world"),
            make_assistant_message("hi there"),
        ];

        let tokens = estimate_history_tokens(&messages);
        assert!(tokens > 0);
    }

    #[test]
    fn test_messages_to_text() {
        let messages = vec![
            make_user_message("user content"),
            make_assistant_message("assistant content"),
        ];

        let text = messages_to_text(&messages);
        assert!(text.contains("[user]"));
        assert!(text.contains("user content"));
        assert!(text.contains("[assistant]"));
        assert!(text.contains("assistant content"));
    }

    #[test]
    fn test_compact_config_default() {
        let config = CompactConfig::default();
        assert_eq!(config.user_message_max_tokens, 20_000);
        assert!(config.emit_warnings);
        assert_eq!(config.get_summarization_prompt(), SUMMARIZATION_PROMPT);
    }

    #[test]
    fn test_compact_config_custom_prompt() {
        let config = CompactConfig::default().with_summarization_prompt("Custom prompt");
        assert_eq!(config.get_summarization_prompt(), "Custom prompt");
    }

    #[test]
    fn test_create_compaction_prompt() {
        let messages = vec![make_user_message("Hello")];
        let prompt = create_compaction_prompt(&messages, None);

        assert!(prompt.contains(SUMMARIZATION_PROMPT));
        assert!(prompt.contains("[user]: Hello"));
    }

    // === Additional tests for comprehensive coverage ===

    // CompactConfig tests
    #[test]
    fn test_compact_config_with_user_message_max_tokens() {
        let config = CompactConfig::default().with_user_message_max_tokens(5000);
        assert_eq!(config.user_message_max_tokens, 5000);
    }

    #[test]
    fn test_compact_config_builder_chaining() {
        let config = CompactConfig::default()
            .with_user_message_max_tokens(1000)
            .with_summarization_prompt("Custom");
        assert_eq!(config.user_message_max_tokens, 1000);
        assert_eq!(config.get_summarization_prompt(), "Custom");
    }

    #[test]
    fn test_compact_config_debug() {
        let config = CompactConfig::default();
        let debug = format!("{:?}", config);
        assert!(debug.contains("CompactConfig"));
        assert!(debug.contains("user_message_max_tokens"));
    }

    #[test]
    fn test_compact_config_clone() {
        let config = CompactConfig::default().with_summarization_prompt("Test");
        let cloned = config.clone();
        assert_eq!(cloned.get_summarization_prompt(), "Test");
        assert_eq!(
            cloned.user_message_max_tokens,
            config.user_message_max_tokens
        );
    }

    #[test]
    fn test_compact_config_serde_roundtrip() {
        let config = CompactConfig {
            user_message_max_tokens: 5000,
            summarization_prompt: Some("Custom prompt".to_string()),
            emit_warnings: false,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: CompactConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.user_message_max_tokens, 5000);
        assert_eq!(
            parsed.summarization_prompt,
            Some("Custom prompt".to_string())
        );
        assert!(!parsed.emit_warnings);
    }

    #[test]
    fn test_compact_config_serde_with_none_prompt() {
        let config = CompactConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: CompactConfig = serde_json::from_str(&json).unwrap();
        assert!(parsed.summarization_prompt.is_none());
    }

    // CompactionResult tests
    #[test]
    fn test_compaction_result_debug() {
        let result = CompactionResult {
            history: vec![make_user_message("test")],
            summary: "Summary".to_string(),
            original_count: 10,
            compacted_count: 3,
            tokens_saved: 500,
        };
        let debug = format!("{:?}", result);
        assert!(debug.contains("CompactionResult"));
        assert!(debug.contains("original_count"));
        assert!(debug.contains("tokens_saved"));
    }

    #[test]
    fn test_compaction_result_clone() {
        let result = CompactionResult {
            history: vec![make_user_message("test")],
            summary: "Summary".to_string(),
            original_count: 10,
            compacted_count: 3,
            tokens_saved: 500,
        };
        let cloned = result.clone();
        assert_eq!(cloned.original_count, 10);
        assert_eq!(cloned.compacted_count, 3);
        assert_eq!(cloned.tokens_saved, 500);
        assert_eq!(cloned.summary, "Summary");
    }

    #[test]
    fn test_compaction_result_fields() {
        let result = CompactionResult {
            history: vec![],
            summary: String::new(),
            original_count: 0,
            compacted_count: 0,
            tokens_saved: 0,
        };
        assert!(result.history.is_empty());
        assert!(result.summary.is_empty());
        assert_eq!(result.original_count, 0);
    }

    // collect_user_messages tests
    #[test]
    fn test_collect_user_messages_empty() {
        let messages: Vec<Message> = vec![];
        let collected = collect_user_messages(&messages);
        assert!(collected.is_empty());
    }

    #[test]
    fn test_collect_user_messages_only_assistant() {
        let messages = vec![
            make_assistant_message("response 1"),
            make_assistant_message("response 2"),
        ];
        let collected = collect_user_messages(&messages);
        assert!(collected.is_empty());
    }

    #[test]
    fn test_collect_user_messages_mixed_roles() {
        let messages = vec![
            Message {
                role: MessageRole::System,
                content: "system".to_string(),
                tool_call_id: None,
                tool_calls: vec![],
            },
            make_user_message("user1"),
            make_assistant_message("assistant1"),
            Message {
                role: MessageRole::Tool,
                content: "tool result".to_string(),
                tool_call_id: Some("id".to_string()),
                tool_calls: vec![],
            },
            make_user_message("user2"),
        ];
        let collected = collect_user_messages(&messages);
        assert_eq!(collected, vec!["user1", "user2"]);
    }

    // is_summary_message tests
    #[test]
    fn test_is_summary_message_exact_prefix() {
        assert!(is_summary_message(SUMMARY_PREFIX));
    }

    #[test]
    fn test_is_summary_message_with_content_no_newline() {
        let msg = format!("{}more content", SUMMARY_PREFIX);
        // Should be true - starts_with handles this
        assert!(is_summary_message(&msg));
    }

    #[test]
    fn test_is_summary_message_false_cases() {
        assert!(!is_summary_message(""));
        assert!(!is_summary_message("Hello world"));
        assert!(!is_summary_message("Previous conversation summary")); // Missing brackets
        assert!(!is_summary_message(" [Previous conversation summary]")); // Leading space
    }

    // build_compacted_history_with_limit edge cases
    #[test]
    fn test_build_compacted_history_with_zero_limit() {
        let initial = vec![];
        let user_messages = vec!["message1".to_string(), "message2".to_string()];
        let summary = "Summary";

        let result = build_compacted_history_with_limit(initial, &user_messages, summary, 0);

        // Should only have the summary, no user messages
        assert_eq!(result.len(), 1);
        assert!(result[0].content.contains(SUMMARY_PREFIX));
    }

    #[test]
    fn test_build_compacted_history_preserves_initial_context() {
        let initial = vec![
            Message {
                role: MessageRole::System,
                content: "System prompt 1".to_string(),
                tool_call_id: None,
                tool_calls: vec![],
            },
            Message {
                role: MessageRole::System,
                content: "System prompt 2".to_string(),
                tool_call_id: None,
                tool_calls: vec![],
            },
        ];
        let user_messages = vec!["user".to_string()];
        let summary = "Summary";

        let result = build_compacted_history(initial, &user_messages, summary);

        assert_eq!(result.len(), 4);
        assert_eq!(result[0].content, "System prompt 1");
        assert_eq!(result[1].content, "System prompt 2");
    }

    #[test]
    fn test_build_compacted_history_newest_messages_preserved() {
        let initial = vec![];
        // 100 short messages that fit within token limit
        let user_messages: Vec<String> = (1..=100).map(|i| format!("message {}", i)).collect();
        let summary = "Summary";

        let result = build_compacted_history_with_limit(initial, &user_messages, summary, 1000);

        // Messages should be newest first (in chronological order after reversal)
        // The algorithm takes from newest, then reverses
        let last_user_msg_idx = result.len() - 2; // Before summary
        assert!(result[last_user_msg_idx].content.contains("100"));
    }

    #[test]
    fn test_build_compacted_history_empty_user_messages() {
        let initial = vec![];
        let user_messages: Vec<String> = vec![];
        let summary = "Summary";

        let result = build_compacted_history(initial, &user_messages, summary);

        // Should only have summary
        assert_eq!(result.len(), 1);
        assert!(result[0].content.contains("Summary"));
    }

    // messages_to_text tests
    #[test]
    fn test_messages_to_text_all_roles() {
        let messages = vec![
            Message {
                role: MessageRole::System,
                content: "system content".to_string(),
                tool_call_id: None,
                tool_calls: vec![],
            },
            make_user_message("user content"),
            make_assistant_message("assistant content"),
            Message {
                role: MessageRole::Tool,
                content: "tool content".to_string(),
                tool_call_id: Some("id".to_string()),
                tool_calls: vec![],
            },
        ];

        let text = messages_to_text(&messages);
        assert!(text.contains("[system]: system content"));
        assert!(text.contains("[user]: user content"));
        assert!(text.contains("[assistant]: assistant content"));
        assert!(text.contains("[tool]: tool content"));
    }

    #[test]
    fn test_messages_to_text_empty() {
        let messages: Vec<Message> = vec![];
        let text = messages_to_text(&messages);
        assert!(text.is_empty());
    }

    #[test]
    fn test_messages_to_text_single_message() {
        let messages = vec![make_user_message("single")];
        let text = messages_to_text(&messages);
        assert_eq!(text, "[user]: single");
    }

    #[test]
    fn test_messages_to_text_separator() {
        let messages = vec![make_user_message("first"), make_assistant_message("second")];
        let text = messages_to_text(&messages);
        // Should be separated by double newline
        assert!(text.contains("\n\n"));
    }

    // estimate_history_tokens tests
    #[test]
    fn test_estimate_history_tokens_empty() {
        let messages: Vec<Message> = vec![];
        let tokens = estimate_history_tokens(&messages);
        assert_eq!(tokens, 0);
    }

    #[test]
    fn test_estimate_history_tokens_long_message() {
        let long_content = "word ".repeat(1000);
        let messages = vec![make_user_message(&long_content)];
        let tokens = estimate_history_tokens(&messages);
        // Should be substantial for 1000 words
        assert!(tokens > 500);
    }

    #[test]
    fn test_estimate_history_tokens_multiple_messages() {
        let messages = vec![
            make_user_message("hello world"),
            make_assistant_message("hi there friend"),
            make_user_message("more content here"),
        ];
        let single_tokens = estimate_history_tokens(&[make_user_message("hello world")]);
        let total_tokens = estimate_history_tokens(&messages);
        assert!(total_tokens > single_tokens);
    }

    // should_compact tests
    #[test]
    fn test_should_compact_empty_messages() {
        let messages: Vec<Message> = vec![];
        assert!(!should_compact(&messages, 100));
    }

    #[test]
    fn test_should_compact_exact_threshold() {
        // Use a longer message to ensure token count > 1
        let messages = vec![make_user_message(
            "This is a longer message with multiple words to ensure token count is substantial",
        )];
        let tokens = estimate_history_tokens(&messages);
        assert!(
            tokens > 1,
            "Token count should be greater than 1 for this test"
        );
        // At exact threshold, should not compact (tokens <= threshold)
        assert!(!should_compact(&messages, tokens));
        // Just below threshold, should compact (tokens > threshold)
        assert!(should_compact(&messages, tokens - 1));
    }

    // create_compaction_prompt tests
    #[test]
    fn test_create_compaction_prompt_custom() {
        let messages = vec![make_user_message("Hello")];
        let custom = "My custom prompt";
        let prompt = create_compaction_prompt(&messages, Some(custom));

        assert!(prompt.contains(custom));
        assert!(!prompt.contains(SUMMARIZATION_PROMPT));
        assert!(prompt.contains("[user]: Hello"));
    }

    #[test]
    fn test_create_compaction_prompt_empty_messages() {
        let messages: Vec<Message> = vec![];
        let prompt = create_compaction_prompt(&messages, None);

        assert!(prompt.contains(SUMMARIZATION_PROMPT));
        assert!(prompt.contains("---"));
    }

    #[test]
    fn test_create_compaction_prompt_format() {
        let messages = vec![
            make_user_message("question"),
            make_assistant_message("answer"),
        ];
        let prompt = create_compaction_prompt(&messages, None);

        // Should have structure: prompt + separator + context
        assert!(prompt.contains(SUMMARIZATION_PROMPT));
        assert!(prompt.contains("---"));
        assert!(prompt.contains("Conversation to summarize:"));
        assert!(prompt.contains("[user]: question"));
        assert!(prompt.contains("[assistant]: answer"));
    }

    // Constants tests
    #[test]
    fn test_summarization_prompt_content() {
        assert!(SUMMARIZATION_PROMPT.contains("summary"));
        assert!(SUMMARIZATION_PROMPT.contains("Key topics"));
        assert!(SUMMARIZATION_PROMPT.contains("Important decisions"));
    }

    #[test]
    fn test_summary_prefix_format() {
        assert!(SUMMARY_PREFIX.starts_with('['));
        assert!(SUMMARY_PREFIX.ends_with(']'));
        assert!(SUMMARY_PREFIX.contains("summary"));
    }

    #[test]
    fn test_compact_user_message_max_tokens_default() {
        let config = CompactConfig::default();
        assert_eq!(
            config.user_message_max_tokens,
            COMPACT_USER_MESSAGE_MAX_TOKENS
        );
        assert_eq!(COMPACT_USER_MESSAGE_MAX_TOKENS, 20_000);
    }
}
