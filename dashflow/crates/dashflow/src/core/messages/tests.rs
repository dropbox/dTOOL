// Import everything from parent module (messages/mod.rs)
use super::*;

use super::{
    convert_to_messages, default_text_splitter, merge_message_runs, message_chunk_to_message,
    message_from_dict, message_to_dict, messages_from_dict, messages_to_dict,
};
use crate::test_prelude::*;

#[test]
fn test_message_constructors() {
    let human = Message::human("Hello");
    assert!(human.is_human());
    assert_eq!(human.as_text(), "Hello");

    let ai = Message::ai("Hi there");
    assert!(ai.is_ai());
    assert_eq!(ai.as_text(), "Hi there");

    let system = Message::system("You are helpful");
    assert!(system.is_system());
    assert_eq!(system.as_text(), "You are helpful");
}

#[test]
fn test_message_content() {
    let content = MessageContent::Text("test".to_string());
    assert_eq!(content.as_text(), "test");
    assert!(!content.is_empty());

    let blocks = MessageContent::Blocks(vec![
        ContentBlock::Text {
            text: "Hello".to_string(),
        },
        ContentBlock::Text {
            text: "World".to_string(),
        },
    ]);
    assert_eq!(blocks.as_text(), "Hello\nWorld");
}

#[test]
fn test_message_serialization() {
    let msg = Message::human("test message");
    let json = serde_json::to_string(&msg).unwrap();
    let deserialized: Message = serde_json::from_str(&json).unwrap();

    assert_eq!(msg, deserialized);
    assert_eq!(deserialized.as_text(), "test message");
}

#[test]
fn test_ai_message_with_tool_calls() {
    let tool_call = ToolCall {
        id: "call_123".to_string(),
        name: "calculator".to_string(),
        args: serde_json::json!({"operation": "add", "a": 5, "b": 3}),
        tool_type: "tool_call".to_string(),
        index: None,
    };

    let msg = Message::AI {
        content: MessageContent::Text("Let me calculate that".to_string()),
        tool_calls: vec![tool_call],
        invalid_tool_calls: vec![],
        usage_metadata: Some(UsageMetadata::new(10, 20)),
        fields: BaseMessageFields::default(),
    };

    if let Message::AI { tool_calls, .. } = &msg {
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "calculator");
    } else {
        panic!("Expected AI message");
    }
}

#[test]
fn test_content_block_serialization() {
    let block = ContentBlock::Text {
        text: "Hello".to_string(),
    };
    let json = serde_json::to_string(&block).unwrap();
    let deserialized: ContentBlock = serde_json::from_str(&json).unwrap();
    assert_eq!(block, deserialized);

    let tool_block = ContentBlock::ToolUse {
        id: "call_1".to_string(),
        name: "search".to_string(),
        input: serde_json::json!({"query": "test"}),
    };
    let json = serde_json::to_string(&tool_block).unwrap();
    let deserialized: ContentBlock = serde_json::from_str(&json).unwrap();
    assert_eq!(tool_block, deserialized);
}

#[test]
fn test_usage_metadata_in_message() {
    let usage = UsageMetadata::new(100, 50);
    let msg = Message::AI {
        content: MessageContent::Text("response".to_string()),
        tool_calls: vec![],
        invalid_tool_calls: vec![],
        usage_metadata: Some(usage.clone()),
        fields: BaseMessageFields::default(),
    };

    if let Message::AI {
        usage_metadata: Some(u),
        ..
    } = msg
    {
        assert_eq!(u.input_tokens, 100);
        assert_eq!(u.output_tokens, 50);
        assert_eq!(u.total_tokens, 150);
    } else {
        panic!("Expected AI message with usage metadata");
    }
}

// Tests for filter_messages()

#[test]
fn test_filter_messages_by_name() {
    let messages = vec![
        Message::system("you're a good assistant."),
        Message::human("what's your name").with_name("example_user"),
        Message::ai("steve-o").with_name("example_assistant"),
        Message::human("what's your favorite color"),
        Message::ai("silicon blue"),
    ];

    let filtered = filter_messages(
        messages,
        Some(&["example_user".to_string(), "example_assistant".to_string()]),
        None,
        None,
        None,
        None,
        None,
        None,
    );

    assert_eq!(filtered.len(), 2);
    assert_eq!(filtered[0].fields().name, Some("example_user".to_string()));
    assert_eq!(
        filtered[1].fields().name,
        Some("example_assistant".to_string())
    );
}

#[test]
fn test_filter_messages_by_type() {
    let messages = vec![
        Message::system("you're a good assistant."),
        Message::human("what's your name"),
        Message::ai("steve-o"),
        Message::human("what's your favorite color"),
    ];

    let filtered = filter_messages(
        messages,
        None,
        None,
        Some(&["system".into(), "ai".into()]),
        None,
        None,
        None,
        None,
    );

    assert_eq!(filtered.len(), 2);
    assert!(filtered[0].is_system());
    assert!(filtered[1].is_ai());
}

#[test]
fn test_filter_messages_exclude_by_type() {
    let messages = vec![
        Message::system("you're a good assistant."),
        Message::human("what's your name"),
        Message::ai("steve-o"),
        Message::human("what's your favorite color"),
    ];

    let filtered = filter_messages(
        messages,
        None,
        None,
        None,
        Some(&["system".into()]),
        None,
        None,
        None,
    );

    assert_eq!(filtered.len(), 3);
    assert!(filtered[0].is_human());
    assert!(filtered[1].is_ai());
    assert!(filtered[2].is_human());
}

#[test]
fn test_filter_messages_by_id() {
    let mut msg1 = Message::human("hello");
    msg1.fields_mut().id = Some("foo".to_string());

    let mut msg2 = Message::ai("hi there");
    msg2.fields_mut().id = Some("bar".to_string());

    let mut msg3 = Message::human("how are you");
    msg3.fields_mut().id = Some("baz".to_string());

    let messages = vec![msg1, msg2, msg3];

    let filtered = filter_messages(
        messages,
        None,
        None,
        None,
        None,
        Some(&["foo".to_string(), "baz".to_string()]),
        None,
        None,
    );

    assert_eq!(filtered.len(), 2);
    assert_eq!(filtered[0].fields().id, Some("foo".to_string()));
    assert_eq!(filtered[1].fields().id, Some("baz".to_string()));
}

#[test]
fn test_filter_messages_exclude_by_id() {
    let mut msg1 = Message::human("hello");
    msg1.fields_mut().id = Some("foo".to_string());

    let mut msg2 = Message::ai("hi there");
    msg2.fields_mut().id = Some("bar".to_string());

    let mut msg3 = Message::human("how are you");
    msg3.fields_mut().id = Some("baz".to_string());

    let messages = vec![msg1, msg2, msg3];

    let filtered = filter_messages(
        messages,
        None,
        None,
        None,
        None,
        None,
        Some(&["bar".to_string()]),
        None,
    );

    assert_eq!(filtered.len(), 2);
    assert_eq!(filtered[0].fields().id, Some("foo".to_string()));
    assert_eq!(filtered[1].fields().id, Some("baz".to_string()));
}

#[test]
fn test_filter_messages_combined_include_exclude() {
    let mut msg1 = Message::system("you're a good assistant.");
    msg1.fields_mut().id = Some("sys1".to_string());

    let mut msg2 = Message::human("what's your name");
    msg2.fields_mut().id = Some("foo".to_string());
    msg2.fields_mut().name = Some("example_user".to_string());

    let mut msg3 = Message::ai("steve-o");
    msg3.fields_mut().id = Some("bar".to_string());
    msg3.fields_mut().name = Some("example_assistant".to_string());

    let mut msg4 = Message::human("what's your favorite color");
    msg4.fields_mut().id = Some("baz".to_string());

    let mut msg5 = Message::ai("silicon blue");
    msg5.fields_mut().id = Some("blah".to_string());

    let messages = vec![msg1, msg2, msg3, msg4, msg5];

    let filtered = filter_messages(
        messages,
        Some(&["example_user".to_string(), "example_assistant".to_string()]),
        None,
        Some(&["system".into()]),
        None,
        None,
        Some(&["bar".to_string()]),
        None,
    );

    // Should include: system message (by type) and human message (by name)
    // Should exclude: AI message with id="bar"
    assert_eq!(filtered.len(), 2);
    assert!(filtered[0].is_system());
    assert!(filtered[1].is_human());
    assert_eq!(filtered[1].fields().name, Some("example_user".to_string()));
}

#[test]
fn test_filter_messages_exclude_all_tool_calls() {
    let ai_with_tools = Message::AI {
        content: MessageContent::Text("Let me search for that".to_string()),
        tool_calls: vec![ToolCall {
            id: "call_1".to_string(),
            name: "search".to_string(),
            args: serde_json::json!({"query": "test"}),
            tool_type: "tool_call".to_string(),
            index: None,
        }],
        invalid_tool_calls: vec![],
        usage_metadata: None,
        fields: BaseMessageFields::default(),
    };

    let ai_without_tools = Message::ai("I found something");
    let tool_msg = Message::tool("search results", "call_1");

    let messages = vec![ai_with_tools, ai_without_tools, tool_msg];

    let filtered = filter_messages(
        messages,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(ExcludeToolCalls::All),
    );

    // Should only include AI message without tool calls
    assert_eq!(filtered.len(), 1);
    assert!(filtered[0].is_ai());
    if let Message::AI { tool_calls, .. } = &filtered[0] {
        assert!(tool_calls.is_empty());
    }
}

#[test]
fn test_filter_messages_exclude_specific_tool_calls() {
    let ai_msg = Message::AI {
        content: MessageContent::Text("Using tools".to_string()),
        tool_calls: vec![
            ToolCall {
                id: "call_1".to_string(),
                name: "search".to_string(),
                args: serde_json::json!({"query": "test"}),
                tool_type: "tool_call".to_string(),
                index: None,
            },
            ToolCall {
                id: "call_2".to_string(),
                name: "calculator".to_string(),
                args: serde_json::json!({"expr": "2+2"}),
                tool_type: "tool_call".to_string(),
                index: None,
            },
        ],
        invalid_tool_calls: vec![],
        usage_metadata: None,
        fields: BaseMessageFields::default(),
    };

    let tool_msg1 = Message::tool("search results", "call_1");
    let tool_msg2 = Message::tool("4", "call_2");

    let messages = vec![ai_msg, tool_msg1, tool_msg2];

    let filtered = filter_messages(
        messages,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(ExcludeToolCalls::Ids(vec!["call_1".to_string()])),
    );

    // Should exclude tool_msg1 and filter call_1 from ai_msg
    assert_eq!(filtered.len(), 2);

    // First message should be AI with only call_2
    if let Message::AI { tool_calls, .. } = &filtered[0] {
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_2");
    } else {
        panic!("Expected AI message");
    }

    // Second message should be tool_msg2
    if let Message::Tool { tool_call_id, .. } = &filtered[1] {
        assert_eq!(tool_call_id, "call_2");
    } else {
        panic!("Expected Tool message");
    }
}

#[test]
fn test_filter_messages_exclude_tool_calls_with_content_blocks() {
    let ai_msg = Message::AI {
        content: MessageContent::Blocks(vec![
            ContentBlock::Text {
                text: "Let me help".to_string(),
            },
            ContentBlock::ToolUse {
                id: "call_1".to_string(),
                name: "search".to_string(),
                input: serde_json::json!({"query": "test"}),
            },
            ContentBlock::ToolUse {
                id: "call_2".to_string(),
                name: "calculator".to_string(),
                input: serde_json::json!({"expr": "2+2"}),
            },
        ]),
        tool_calls: vec![
            ToolCall {
                id: "call_1".to_string(),
                name: "search".to_string(),
                args: serde_json::json!({"query": "test"}),
                tool_type: "tool_call".to_string(),
                index: None,
            },
            ToolCall {
                id: "call_2".to_string(),
                name: "calculator".to_string(),
                args: serde_json::json!({"expr": "2+2"}),
                tool_type: "tool_call".to_string(),
                index: None,
            },
        ],
        invalid_tool_calls: vec![],
        usage_metadata: None,
        fields: BaseMessageFields::default(),
    };

    let messages = vec![ai_msg];

    let filtered = filter_messages(
        messages,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(ExcludeToolCalls::Ids(vec!["call_1".to_string()])),
    );

    assert_eq!(filtered.len(), 1);

    // Check that call_1 is removed from both tool_calls and content blocks
    if let Message::AI {
        tool_calls,
        content,
        ..
    } = &filtered[0]
    {
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_2");

        if let MessageContent::Blocks(blocks) = content {
            assert_eq!(blocks.len(), 2); // Text + ToolUse for call_2
            match &blocks[1] {
                ContentBlock::ToolUse { id, .. } => {
                    assert_eq!(id, "call_2");
                }
                _ => panic!("Expected ToolUse block"),
            }
        } else {
            panic!("Expected content blocks");
        }
    } else {
        panic!("Expected AI message");
    }
}

#[test]
fn test_filter_messages_no_criteria_returns_all() {
    let messages = vec![
        Message::system("system"),
        Message::human("human"),
        Message::ai("ai"),
    ];

    let filtered = filter_messages(messages.clone(), None, None, None, None, None, None, None);

    assert_eq!(filtered.len(), 3);
}

#[test]
fn test_filter_messages_empty_input() {
    let messages: Vec<Message> = vec![];
    let filtered = filter_messages(messages, None, None, None, None, None, None, None);
    assert_eq!(filtered.len(), 0);
}

// ============================================================================
// trim_messages tests
// ============================================================================

// Simple token counter for testing (counts 10 tokens per message)
fn simple_token_counter(msgs: &[Message]) -> usize {
    msgs.len() * 10
}

// Content-aware token counter for testing partial message trimming
// Counts 5 tokens per line (split by \n) or 5 tokens per block
fn content_token_counter(msgs: &[Message]) -> usize {
    msgs.iter()
        .map(|msg| {
            let content = msg.content();
            match content {
                MessageContent::Text(text) => {
                    // Count 5 tokens per line (lines are split by \n)
                    // Use the same logic as default_text_splitter for consistency
                    if text.is_empty() {
                        0
                    } else {
                        let splits = default_text_splitter(text);
                        splits.len() * 5
                    }
                }
                MessageContent::Blocks(blocks) => {
                    // Count 5 tokens per block
                    blocks.len() * 5
                }
            }
        })
        .sum()
}

// Helper function for basic trim_messages calls without all parameters
fn trim_messages_basic(
    messages: Vec<Message>,
    max_tokens: usize,
    strategy: TrimStrategy,
    end_on: Option<&[MessageTypeFilter]>,
) -> std::result::Result<Vec<Message>, TrimError> {
    trim_messages(
        messages,
        max_tokens,
        simple_token_counter,
        strategy,
        false, // allow_partial
        None,  // text_splitter
        false, // include_system
        None,  // start_on
        end_on,
    )
}

#[test]
fn test_trim_messages_all_fit() {
    // All messages fit within token limit
    let messages = vec![
        Message::system("You are a helpful assistant"),
        Message::human("Hello!"),
        Message::ai("Hi there!"),
    ];

    let result = trim_messages_basic(
        messages.clone(),
        100, // Large enough for all messages
        TrimStrategy::Last,
        None,
    )
    .unwrap();

    assert_eq!(result.len(), 3);
}

#[test]
fn test_trim_messages_trim_first() {
    // Keep first N tokens
    let messages = vec![
        Message::system("You are a helpful assistant"),
        Message::human("Hello!"),
        Message::ai("Hi there!"),
        Message::human("How are you?"),
        Message::ai("I'm good!"),
    ];

    let result = trim_messages_basic(
        messages,
        25, // 2-3 messages max
        TrimStrategy::First,
        None,
    )
    .unwrap();

    // Should keep first 2 messages (20 tokens)
    assert_eq!(result.len(), 2);
    assert!(result[0].is_system());
    assert!(result[1].is_human());
}

#[test]
fn test_trim_messages_trim_last() {
    // Keep last N tokens
    let messages = vec![
        Message::system("You are a helpful assistant"),
        Message::human("Hello!"),
        Message::ai("Hi there!"),
        Message::human("How are you?"),
        Message::ai("I'm good!"),
    ];

    let result = trim_messages_basic(
        messages,
        25, // 2-3 messages max
        TrimStrategy::Last,
        None,
    )
    .unwrap();

    // Should keep last 2 messages (20 tokens)
    assert_eq!(result.len(), 2);
    assert!(result[0].is_human());
    assert!(result[1].is_ai());
}

#[test]
fn test_trim_messages_empty() {
    let messages: Vec<Message> = vec![];

    let result = trim_messages_basic(messages, 100, TrimStrategy::Last, None).unwrap();

    assert_eq!(result.len(), 0);
}

#[test]
fn test_trim_messages_zero_tokens() {
    let messages = vec![Message::human("Hello!"), Message::ai("Hi there!")];

    let result = trim_messages_basic(
        messages,
        0, // No tokens allowed
        TrimStrategy::Last,
        None,
    )
    .unwrap();

    assert_eq!(result.len(), 0);
}

#[test]
fn test_trim_messages_end_on_human() {
    // End on human message (remove messages after last human)
    let messages = vec![
        Message::human("Hello!"),
        Message::ai("Hi there!"),
        Message::human("How are you?"),
        Message::ai("I'm good!"),
        Message::ai("Anything else?"),
    ];

    let result = trim_messages_basic(
        messages,
        100, // Large enough for all
        TrimStrategy::First,
        Some(&[MessageTypeFilter::String("human".to_string())]),
    )
    .unwrap();

    // Should end on last human message (index 2)
    assert_eq!(result.len(), 3);
    assert!(result[2].is_human());
}

#[test]
fn test_trim_messages_end_on_ai() {
    let messages = vec![
        Message::human("Hello!"),
        Message::ai("Hi there!"),
        Message::human("How are you?"),
        Message::ai("I'm good!"),
        Message::human("Great!"),
    ];

    let result = trim_messages_basic(
        messages,
        100, // Large enough for all
        TrimStrategy::First,
        Some(&[MessageTypeFilter::Type(MessageType::AI)]),
    )
    .unwrap();

    // Should end on last AI message (index 3)
    assert_eq!(result.len(), 4);
    assert!(result[3].is_ai());
}

#[test]
fn test_trim_messages_end_on_with_trim() {
    // Combine token trimming with end_on filtering
    let messages = vec![
        Message::human("msg1"),
        Message::ai("msg2"),
        Message::human("msg3"),
        Message::ai("msg4"),
        Message::human("msg5"),
        Message::ai("msg6"),
    ];

    let result = trim_messages_basic(
        messages,
        35, // 3-4 messages
        TrimStrategy::First,
        Some(&[MessageTypeFilter::String("human".to_string())]),
    )
    .unwrap();

    // Should take first 3 messages, then trim to last human
    assert_eq!(result.len(), 3);
    assert!(result[2].is_human());
}

#[test]
fn test_default_text_splitter_empty() {
    let result = default_text_splitter("");
    assert_eq!(result, Vec::<String>::new());
}

#[test]
fn test_default_text_splitter_no_newlines() {
    let result = default_text_splitter("hello world");
    assert_eq!(result, vec!["hello world"]);
}

#[test]
fn test_default_text_splitter_with_newlines() {
    let result = default_text_splitter("line1\nline2\nline3");
    assert_eq!(result, vec!["line1\n", "line2\n", "line3"]);
    // Verify can be rejoined
    assert_eq!(result.join(""), "line1\nline2\nline3");
}

#[test]
fn test_default_text_splitter_trailing_newline() {
    // Empty strings from trailing newlines are filtered out
    let result = default_text_splitter("line1\nline2\n");
    assert_eq!(result, vec!["line1\n", "line2\n"]);
    assert_eq!(result.join(""), "line1\nline2\n");
}

// Tests for include_system parameter

#[test]
fn test_trim_messages_include_system_basic() {
    // Test that system message is preserved with include_system=True
    let messages = vec![
        Message::system("You are a helpful assistant"), // 10 tokens
        Message::human("Hello!"),                       // 10 tokens
        Message::ai("Hi there!"),                       // 10 tokens
        Message::human("How are you?"),                 // 10 tokens
        Message::ai("I'm good!"),                       // 10 tokens
    ];

    let result = trim_messages(
        messages,
        20, // Budget: 20 tokens = system(10) + 1 more message(10)
        simple_token_counter,
        TrimStrategy::Last,
        false, // allow_partial
        None,  // text_splitter
        true,  // include_system - preserve system message
        None,  // start_on
        None,  // end_on
    )
    .unwrap();

    // Should have: system (10) + last message (10) = 2 messages, 20 tokens
    assert_eq!(result.len(), 2);
    assert!(result[0].is_system());
    assert_eq!(result[0].as_text(), "You are a helpful assistant");
    assert!(result[1].is_ai());
    assert_eq!(result[1].as_text(), "I'm good!");
}

#[test]
fn test_trim_messages_include_system_empty_messages() {
    // Test include_system with only system message
    let messages = vec![Message::system("You are a helpful assistant")]; // 10 tokens

    let result = trim_messages(
        messages,
        20, // More tokens than system message
        simple_token_counter,
        TrimStrategy::Last,
        false,
        None,
        true, // include_system
        None,
        None,
    )
    .unwrap();

    // Should have just system message
    assert_eq!(result.len(), 1);
    assert!(result[0].is_system());
}

#[test]
fn test_trim_messages_include_system_no_system_message() {
    // Test include_system when there's no system message
    let messages = vec![
        Message::human("Hello!"),       // 10 tokens
        Message::ai("Hi there!"),       // 10 tokens
        Message::human("How are you?"), // 10 tokens
    ];

    let result = trim_messages(
        messages,
        15, // Budget: 15 tokens = 1 message (10 tokens each)
        simple_token_counter,
        TrimStrategy::Last,
        false,
        None,
        true, // include_system (but no system message exists)
        None,
        None,
    )
    .unwrap();

    // Should just trim normally (no system message to preserve) - last 1 message fits
    assert_eq!(result.len(), 1);
    assert!(result[0].is_human());
    assert_eq!(result[0].as_text(), "How are you?");
}

#[test]
fn test_trim_messages_include_system_exceeds_budget() {
    // Test include_system when system message exceeds token budget
    let messages = vec![
        Message::system("You are a very helpful and knowledgeable assistant"), // 25 tokens
        Message::human("Hello!"),                                              // 5 tokens
    ];

    let result = trim_messages(
        messages,
        10, // Less than system message tokens
        simple_token_counter,
        TrimStrategy::Last,
        false,
        None,
        true, // include_system
        None,
        None,
    )
    .unwrap();

    // Should still include system message even though it exceeds budget
    assert_eq!(result.len(), 1);
    assert!(result[0].is_system());
}

#[test]
fn test_trim_messages_include_system_with_first_strategy_errors() {
    // Test that include_system with First strategy returns error
    let messages = vec![
        Message::system("You are a helpful assistant"),
        Message::human("Hello!"),
    ];

    let result = trim_messages(
        messages,
        20,
        simple_token_counter,
        TrimStrategy::First, // First strategy not compatible with include_system
        false,
        None,
        true, // include_system
        None,
        None,
    );

    assert!(result.is_err());
    if let Err(TrimError::InvalidParameters(msg)) = result {
        assert!(msg.contains("include_system"));
        assert!(msg.contains("Last"));
    } else {
        panic!("Expected InvalidParameters error");
    }
}

// Tests for start_on parameter

#[test]
fn test_trim_messages_start_on_human() {
    // Test start_on human - trim messages before first human
    let messages = vec![
        Message::system("You are a helpful assistant"), // 10 tokens
        Message::ai("Hi!"),                             // 5 tokens
        Message::human("Hello!"),                       // 5 tokens
        Message::ai("How can I help?"),                 // 10 tokens
        Message::human("Tell me a joke"),               // 10 tokens
    ];

    let result = trim_messages(
        messages,
        100, // Large enough for all
        simple_token_counter,
        TrimStrategy::Last,
        false,
        None,
        false,
        Some(&[MessageTypeFilter::String("human".to_string())]), // start_on human
        None,
    )
    .unwrap();

    // Should start from first human message (index 2)
    assert_eq!(result.len(), 3);
    assert!(result[0].is_human());
    assert_eq!(result[0].as_text(), "Hello!");
    assert!(result[1].is_ai());
    assert!(result[2].is_human());
}

#[test]
fn test_trim_messages_start_on_ai() {
    // Test start_on ai
    let messages = vec![
        Message::system("You are a helpful assistant"),
        Message::human("Hello!"),
        Message::ai("Hi there!"),
        Message::human("How are you?"),
        Message::ai("I'm good!"),
    ];

    let result = trim_messages(
        messages,
        100,
        simple_token_counter,
        TrimStrategy::Last,
        false,
        None,
        false,
        Some(&[MessageTypeFilter::Type(MessageType::AI)]), // start_on AI
        None,
    )
    .unwrap();

    // Should start from first AI message (index 2)
    assert_eq!(result.len(), 3);
    assert!(result[0].is_ai());
    assert_eq!(result[0].as_text(), "Hi there!");
    assert!(result[1].is_human());
    assert!(result[2].is_ai());
}

#[test]
fn test_trim_messages_start_on_not_found() {
    // Test start_on when message type not found
    let messages = vec![Message::human("Hello!"), Message::human("How are you?")];

    let result = trim_messages(
        messages,
        100,
        simple_token_counter,
        TrimStrategy::Last,
        false,
        None,
        false,
        Some(&[MessageTypeFilter::Type(MessageType::AI)]), // start_on AI (none exist)
        None,
    )
    .unwrap();

    // Should return empty when start_on type not found
    assert_eq!(result.len(), 0);
}

#[test]
fn test_trim_messages_start_on_with_include_system() {
    // Test start_on combined with include_system
    let messages = vec![
        Message::system("You are a helpful assistant"), // 10 tokens
        Message::ai("Hi!"),                             // 10 tokens
        Message::human("Hello!"),                       // 10 tokens
        Message::ai("How can I help?"),                 // 10 tokens
        Message::human("Tell me a joke"),               // 10 tokens
    ];

    let result = trim_messages(
        messages,
        30, // Budget: 30 tokens
        simple_token_counter,
        TrimStrategy::Last,
        false,
        None,
        true,                                                    // include_system
        Some(&[MessageTypeFilter::String("human".to_string())]), // start_on human
        None,
    )
    .unwrap();

    // Python behavior: system (10) + last human before budget (10) = 2 messages, 20 tokens
    // (start_on filters to messages from first human onwards, then takes last messages within budget)
    assert_eq!(result.len(), 2);
    assert!(result[0].is_system());
    assert_eq!(result[0].as_text(), "You are a helpful assistant");
    assert!(result[1].is_human());
    assert_eq!(result[1].as_text(), "Tell me a joke");
}

#[test]
fn test_trim_messages_start_on_with_first_strategy_errors() {
    // Test that start_on with First strategy returns error
    let messages = vec![Message::human("Hello!"), Message::ai("Hi!")];

    let result = trim_messages(
        messages,
        20,
        simple_token_counter,
        TrimStrategy::First, // First strategy not compatible with start_on
        false,
        None,
        false,
        Some(&[MessageTypeFilter::String("human".to_string())]),
        None,
    );

    assert!(result.is_err());
    if let Err(TrimError::InvalidParameters(msg)) = result {
        assert!(msg.contains("start_on"));
        assert!(msg.contains("Last"));
    } else {
        panic!("Expected InvalidParameters error");
    }
}

// Tests for partial message trimming (text splitting)

#[test]
fn test_trim_messages_partial_text_first_strategy() {
    // Test partial text trimming with First strategy
    let messages = vec![
        Message::human("line1\nline2\nline3\nline4\nline5"), // 25 tokens (5 lines)
        Message::ai("response"),                             // 5 tokens
    ];

    let result = trim_messages(
        messages,
        15, // Enough for partial first message (3 lines)
        content_token_counter,
        TrimStrategy::First,
        true, // allow_partial
        None, // use default text splitter
        false,
        None,
        None,
    )
    .unwrap();

    // Should have partial first message (first 3 lines = 15 tokens)
    assert_eq!(result.len(), 1);
    assert!(result[0].is_human());
    assert_eq!(result[0].as_text(), "line1\nline2\nline3\n");
}

#[test]
fn test_trim_messages_partial_text_last_strategy() {
    // Test partial text trimming with Last strategy
    let messages = vec![
        Message::human("ignored"),                        // 5 tokens
        Message::ai("line1\nline2\nline3\nline4\nline5"), // 25 tokens (5 lines)
    ];

    let result = trim_messages(
        messages,
        15, // Enough for partial last message (3 lines)
        content_token_counter,
        TrimStrategy::Last,
        true, // allow_partial
        None, // use default text splitter
        false,
        None,
        None,
    )
    .unwrap();

    // Should have partial last message (last 3 lines = 15 tokens)
    assert_eq!(result.len(), 1);
    assert!(result[0].is_ai());
    assert_eq!(result[0].as_text(), "line3\nline4\nline5");
}

#[test]
fn test_trim_messages_partial_text_custom_splitter() {
    // Test partial text trimming with custom text splitter
    // Splitter preserves spaces by adding them back (similar to default_text_splitter with newlines)
    fn word_splitter(text: &str) -> Vec<String> {
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.is_empty() {
            return vec![];
        }

        // Add spaces back to all but the last word (preserves separators)
        let mut result: Vec<String> = words.iter().map(|s| s.to_string()).collect();
        for i in 0..result.len() - 1 {
            result[i].push(' ');
        }
        result
    }

    // Custom token counter that counts words (5 tokens per word)
    // Uses the same word_splitter for consistency
    fn word_token_counter(msgs: &[Message]) -> usize {
        msgs.iter()
            .map(|msg| {
                let text = msg.as_text();
                if text.is_empty() {
                    0
                } else {
                    word_splitter(&text).len() * 5
                }
            })
            .sum()
    }

    let messages = vec![
        Message::human("one two three four five"), // 25 tokens (5 words)
    ];

    let result = trim_messages(
        messages,
        15, // Enough for 3 words
        word_token_counter,
        TrimStrategy::First,
        true,                // allow_partial
        Some(word_splitter), // custom splitter
        false,
        None,
        None,
    )
    .unwrap();

    // Should have partial message with first 3 words (spaces preserved)
    assert_eq!(result.len(), 1);
    assert!(result[0].is_human());
    assert_eq!(result[0].as_text(), "one two three ");
}

#[test]
fn test_trim_messages_partial_text_multiple_messages() {
    // Test partial trimming with multiple messages
    let messages = vec![
        Message::human("msg1"),                       // 5 tokens (1 line)
        Message::ai("msg2"),                          // 5 tokens (1 line)
        Message::human("line1\nline2\nline3\nline4"), // 20 tokens (4 lines)
    ];

    let result = trim_messages(
        messages,
        25, // msg1 (5) + msg2 (5) + partial msg3 (15 = 3 lines)
        content_token_counter,
        TrimStrategy::First,
        true, // allow_partial
        None,
        false,
        None,
        None,
    )
    .unwrap();

    // Should have all 3 messages, with last one partial
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].as_text(), "msg1");
    assert_eq!(result[1].as_text(), "msg2");
    assert_eq!(result[2].as_text(), "line1\nline2\nline3\n");
}

#[test]
fn test_trim_messages_partial_text_with_end_on() {
    // Test partial trimming with end_on parameter
    let messages = vec![
        Message::human("line1\nline2\nline3\nline4\nline5"), // 25 tokens
        Message::ai("response"),                             // 5 tokens
        Message::human("final"),                             // 5 tokens
    ];

    let result = trim_messages(
        messages,
        20, // Enough for partial first message (4 lines)
        content_token_counter,
        TrimStrategy::First,
        true, // allow_partial
        None,
        false,
        None,
        Some(&[MessageTypeFilter::String("human".to_string())]), // end_on human
    )
    .unwrap();

    // Should have partial first message, then filter to end on human
    // So we get the partial human message only
    assert_eq!(result.len(), 1);
    assert!(result[0].is_human());
    assert_eq!(result[0].as_text(), "line1\nline2\nline3\nline4\n");
}

#[test]
fn test_trim_messages_partial_disabled() {
    // Test that partial trimming is disabled when allow_partial=false
    let messages = vec![
        Message::human("line1\nline2\nline3\nline4\nline5"), // 25 tokens
        Message::ai("response"),                             // 5 tokens
    ];

    let result = trim_messages(
        messages,
        15, // Not enough for full first message
        content_token_counter,
        TrimStrategy::First,
        false, // allow_partial=false
        None,
        false,
        None,
        None,
    )
    .unwrap();

    // Should return empty (no complete messages fit)
    assert_eq!(result.len(), 0);
}

// Tests for partial message trimming (content blocks)

#[test]
fn test_trim_messages_partial_blocks_first_strategy() {
    // Test partial content block trimming with First strategy
    let messages = vec![
        Message::human_with_blocks(vec![
            ContentBlock::Text {
                text: "block1".to_string(),
            }, // 5 tokens
            ContentBlock::Text {
                text: "block2".to_string(),
            }, // 5 tokens
            ContentBlock::Text {
                text: "block3".to_string(),
            }, // 5 tokens
            ContentBlock::Text {
                text: "block4".to_string(),
            }, // 5 tokens
        ]),
        Message::ai("response"), // 5 tokens
    ];

    let result = trim_messages(
        messages,
        15, // Enough for first 3 blocks
        content_token_counter,
        TrimStrategy::First,
        true, // allow_partial
        None,
        false,
        None,
        None,
    )
    .unwrap();

    // Should have partial message with first 3 blocks
    assert_eq!(result.len(), 1);
    assert!(result[0].is_human());
    if let MessageContent::Blocks(blocks) = result[0].content() {
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].as_text(), "block1");
        assert_eq!(blocks[1].as_text(), "block2");
        assert_eq!(blocks[2].as_text(), "block3");
    } else {
        panic!("Expected Blocks content");
    }
}

#[test]
fn test_trim_messages_partial_blocks_last_strategy() {
    // Test partial content block trimming with Last strategy
    let messages = vec![
        Message::ai("ignored"), // 5 tokens
        Message::human_with_blocks(vec![
            ContentBlock::Text {
                text: "block1".to_string(),
            }, // 5 tokens
            ContentBlock::Text {
                text: "block2".to_string(),
            }, // 5 tokens
            ContentBlock::Text {
                text: "block3".to_string(),
            }, // 5 tokens
            ContentBlock::Text {
                text: "block4".to_string(),
            }, // 5 tokens
        ]),
    ];

    let result = trim_messages(
        messages,
        15, // Enough for last 3 blocks
        content_token_counter,
        TrimStrategy::Last,
        true, // allow_partial
        None,
        false,
        None,
        None,
    )
    .unwrap();

    // Should have partial message with last 3 blocks
    assert_eq!(result.len(), 1);
    assert!(result[0].is_human());
    if let MessageContent::Blocks(blocks) = result[0].content() {
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].as_text(), "block2");
        assert_eq!(blocks[1].as_text(), "block3");
        assert_eq!(blocks[2].as_text(), "block4");
    } else {
        panic!("Expected Blocks content");
    }
}

#[test]
fn test_trim_messages_partial_blocks_mixed_types() {
    // Test partial blocks with mixed content types (text + tool use)
    let messages = vec![Message::ai_with_blocks(vec![
        ContentBlock::Text {
            text: "Let me help".to_string(),
        }, // 10 tokens
        ContentBlock::ToolUse {
            id: "call_1".to_string(),
            name: "search".to_string(),
            input: serde_json::json!({"query": "test"}), // 5 tokens
        },
        ContentBlock::Text {
            text: "Here's the result".to_string(),
        }, // 10 tokens
    ])];

    let result = trim_messages(
        messages,
        10, // Enough for first 2 blocks: text (5 tokens per line = 5) + tool use (1 block = 5)
        content_token_counter,
        TrimStrategy::First,
        true, // allow_partial
        None,
        false,
        None,
        None,
    )
    .unwrap();

    // Should have partial message with first 2 blocks
    assert_eq!(result.len(), 1);
    if let MessageContent::Blocks(blocks) = result[0].content() {
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].as_text(), "Let me help");
        if let ContentBlock::ToolUse { name, .. } = &blocks[1] {
            assert_eq!(name, "search");
        } else {
            panic!("Expected ToolUse block");
        }
    } else {
        panic!("Expected Blocks content");
    }
}

#[test]
fn test_trim_messages_partial_blocks_single_block_too_large() {
    // Test when single block exceeds token budget
    let messages = vec![Message::human_with_blocks(vec![
        ContentBlock::Text {
            text: "block1".to_string(),
        }, // 5 tokens (1 block)
    ])];

    let result = trim_messages(
        messages,
        3, // Not enough for single block (need 5 tokens)
        content_token_counter,
        TrimStrategy::First,
        true, // allow_partial
        None,
        false,
        None,
        None,
    )
    .unwrap();

    // Should return empty (single block doesn't fit, can't be partially trimmed)
    assert_eq!(result.len(), 0);
}

// Tests for error cases (parameter validation)

#[test]
fn test_trim_messages_error_include_system_with_first() {
    let messages = vec![Message::system("test"), Message::human("hello")];

    let result = trim_messages(
        messages,
        20,
        simple_token_counter,
        TrimStrategy::First,
        false,
        None,
        true, // include_system with First strategy
        None,
        None,
    );

    assert!(result.is_err());
    assert!(matches!(result, Err(TrimError::InvalidParameters(_))));
}

#[test]
fn test_trim_messages_error_start_on_with_first() {
    let messages = vec![Message::human("hello"), Message::ai("hi")];

    let result = trim_messages(
        messages,
        20,
        simple_token_counter,
        TrimStrategy::First,
        false,
        None,
        false,
        Some(&[MessageTypeFilter::String("human".to_string())]), // start_on with First
        None,
    );

    assert!(result.is_err());
    assert!(matches!(result, Err(TrimError::InvalidParameters(_))));
}

#[test]
fn test_trim_messages_no_error_with_valid_params() {
    // Test that valid parameter combinations don't error
    let messages = vec![
        Message::system("You are helpful"),
        Message::human("Hello!"),
        Message::ai("Hi!"),
    ];

    // Last strategy with include_system and start_on is valid
    let result = trim_messages(
        messages,
        100,
        simple_token_counter,
        TrimStrategy::Last,
        false,
        None,
        true,                                                    // include_system
        Some(&[MessageTypeFilter::String("human".to_string())]), // start_on
        None,
    );

    assert!(result.is_ok());
}

#[test]
fn test_get_buffer_string() {
    let messages = vec![
        Message::human("Hi, how are you?"),
        Message::ai("Good, how are you?"),
    ];
    let result = get_buffer_string(&messages, "Human", "AI").unwrap();
    assert_eq!(result, "Human: Hi, how are you?\nAI: Good, how are you?");
}

#[test]
fn test_get_buffer_string_custom_prefixes() {
    let messages = vec![
        Message::human("Hello"),
        Message::ai("Hi there"),
        Message::system("Be helpful"),
    ];
    let result = get_buffer_string(&messages, "User", "Assistant").unwrap();
    assert_eq!(
        result,
        "User: Hello\nAssistant: Hi there\nSystem: Be helpful"
    );
}

#[test]
fn test_merge_message_runs_same_type() {
    let messages = vec![
        Message::system("You're a good assistant."),
        Message::human("What's your favorite color?"),
        Message::human("Wait, your favorite food?"),
        Message::ai("My favorite color is blue"),
    ];

    let merged = merge_message_runs(messages, "\n");
    assert_eq!(merged.len(), 3); // System, Human (merged), AI

    // Check the merged human message
    if let Message::Human { content, .. } = &merged[1] {
        assert_eq!(
            content.as_text(),
            "What's your favorite color?\nWait, your favorite food?"
        );
    } else {
        panic!("Expected Human message");
    }
}

#[test]
fn test_merge_message_runs_tool_not_merged() {
    let messages = vec![
        Message::tool("Result 1", "call1"),
        Message::tool("Result 2", "call2"),
    ];

    let merged = merge_message_runs(messages, "\n");
    assert_eq!(merged.len(), 2); // Tool messages should NOT be merged
}

#[test]
fn test_merge_message_runs_custom_separator() {
    let messages = vec![Message::human("First"), Message::human("Second")];

    let merged = merge_message_runs(messages, " | ");
    assert_eq!(merged.len(), 1);

    if let Message::Human { content, .. } = &merged[0] {
        assert_eq!(content.as_text(), "First | Second");
    } else {
        panic!("Expected Human message");
    }
}

#[test]
fn test_convert_to_messages_string() {
    let messages = vec![MessageLike::from("Hello")];
    let converted = convert_to_messages(messages).unwrap();
    assert_eq!(converted.len(), 1);
    assert!(matches!(converted[0], Message::Human { .. }));
    assert_eq!(converted[0].as_text(), "Hello");
}

#[test]
fn test_convert_to_messages_tuple() {
    let messages = vec![
        MessageLike::from(("human", "Hello")),
        MessageLike::from(("ai", "Hi there")),
    ];
    let converted = convert_to_messages(messages).unwrap();
    assert_eq!(converted.len(), 2);
    assert!(matches!(converted[0], Message::Human { .. }));
    assert!(matches!(converted[1], Message::AI { .. }));
}

#[test]
fn test_convert_to_messages_dict() {
    use serde_json::json;

    let messages = vec![MessageLike::from(json!({
        "role": "human",
        "content": "Hello world"
    }))];

    let converted = convert_to_messages(messages).unwrap();
    assert_eq!(converted.len(), 1);
    assert!(matches!(converted[0], Message::Human { .. }));
    assert_eq!(converted[0].as_text(), "Hello world");
}

#[test]
fn test_convert_to_messages_mixed() {
    use serde_json::json;

    let messages = vec![
        MessageLike::from("Hello"),
        MessageLike::from(("ai", "Hi")),
        MessageLike::from(Message::system("Be helpful")),
        MessageLike::from(json!({"role": "human", "content": "Thanks"})),
    ];

    let converted = convert_to_messages(messages).unwrap();
    assert_eq!(converted.len(), 4);
    assert!(matches!(converted[0], Message::Human { .. }));
    assert!(matches!(converted[1], Message::AI { .. }));
    assert!(matches!(converted[2], Message::System { .. }));
    assert!(matches!(converted[3], Message::Human { .. }));
}

#[test]
fn test_message_to_dict() {
    let msg = Message::human("Hello");
    let dict = message_to_dict(&msg).unwrap();

    // Check structure: {"type": "...", "data": {...}}
    assert!(dict.is_object());
    assert_eq!(dict["type"], "human");
    assert!(dict["data"].is_object());

    // Check data contains message content
    let data = &dict["data"];
    assert_eq!(data["type"], "human");
}

#[test]
fn test_messages_to_dict() {
    let messages = vec![
        Message::human("Hello"),
        Message::ai("Hi there"),
        Message::system("Be helpful"),
    ];

    let dicts = messages_to_dict(&messages).unwrap();
    assert_eq!(dicts.len(), 3);

    assert_eq!(dicts[0]["type"], "human");
    assert_eq!(dicts[1]["type"], "ai");
    assert_eq!(dicts[2]["type"], "system");
}

#[test]
fn test_message_from_dict() {
    use serde_json::json;

    // Create a dict in Rust serialization format (matches message_to_dict output)
    let dict = json!({
        "type": "human",
        "data": {
            "content": "Hello world",  // MessageContent::Text serializes as a string
            "additional_kwargs": {},
            "response_metadata": {},
            "type": "human",
            "name": null,
            "id": null
        }
    });

    let msg = message_from_dict(&dict).unwrap();
    assert!(msg.is_human());
    assert_eq!(msg.as_text(), "Hello world");
}

#[test]
fn test_messages_from_dict() {
    use serde_json::json;

    let dicts = vec![
        json!({
            "type": "human",
            "data": {
                "content": "Hello",  // String format for MessageContent::Text
                "additional_kwargs": {},
                "response_metadata": {},
                "type": "human",
                "name": null,
                "id": null
            }
        }),
        json!({
            "type": "ai",
            "data": {
                "content": "Hi there",  // String format for MessageContent::Text
                "additional_kwargs": {},
                "response_metadata": {},
                "type": "ai",
                "name": null,
                "id": null,
                "tool_calls": [],
                "invalid_tool_calls": [],
                "usage_metadata": null
            }
        }),
    ];

    let messages = messages_from_dict(&dicts).unwrap();
    assert_eq!(messages.len(), 2);
    assert!(messages[0].is_human());
    assert_eq!(messages[0].as_text(), "Hello");
    assert!(messages[1].is_ai());
    assert_eq!(messages[1].as_text(), "Hi there");
}

#[test]
fn test_message_serialization_roundtrip() {
    // Test that we can serialize and deserialize messages
    let original = vec![
        Message::human("Hello"),
        Message::ai("Hi there"),
        Message::system("You are helpful"),
        Message::tool("Tool result", "call_123"),
    ];

    // Serialize to dicts
    let dicts = messages_to_dict(&original).unwrap();

    // Deserialize back
    let restored = messages_from_dict(&dicts).unwrap();

    assert_eq!(restored.len(), original.len());
    assert_eq!(restored[0].as_text(), "Hello");
    assert_eq!(restored[1].as_text(), "Hi there");
    assert_eq!(restored[2].as_text(), "You are helpful");
    assert_eq!(restored[3].as_text(), "Tool result");
}

#[test]
fn test_message_chunk_to_message() {
    let chunk = AIMessageChunk::new("Hello world");
    let msg = message_chunk_to_message(chunk);

    assert!(msg.is_ai());
    assert_eq!(msg.as_text(), "Hello world");
}

#[test]
fn test_message_chunk_to_message_with_tool_calls() {
    let tool_call = ToolCall {
        id: "call_1".to_string(),
        name: "search".to_string(),
        args: serde_json::json!({"query": "test"}),
        tool_type: "tool_call".to_string(),
        index: None,
    };

    let mut chunk = AIMessageChunk::new("Using search tool");
    chunk.tool_calls.push(tool_call);

    let msg = message_chunk_to_message(chunk);

    if let Message::AI { tool_calls, .. } = msg {
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "search");
    } else {
        panic!("Expected AI message");
    }
}
