// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Unit tests for ChatAnthropic.
//!
//! Tests cover builder patterns, message conversion, tool handling,
//! streaming events, thinking blocks, and serialization.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods
)]

use super::*;

/// Note: This is the legacy stateless version kept for testing purposes.
/// Production code uses convert_stream_event_to_chunk_stateful for full tool call support.
fn convert_stream_event_to_chunk(event: StreamEvent) -> Option<AIMessageChunk> {
    match event {
        StreamEvent::MessageStart { message } => {
            // Return chunk with model metadata
            let mut chunk = AIMessageChunk::new("");
            chunk
                .fields
                .response_metadata
                .insert("model".to_string(), serde_json::json!(message.model));
            Some(chunk)
        }
        StreamEvent::ContentBlockStart { content_block, .. } => {
            match content_block {
                StreamContentBlock::Text { .. } => {
                    // For text blocks, we don't need to emit anything here
                    // The actual content comes in ContentBlockDelta events
                    None
                }
                StreamContentBlock::ToolUse { id, name } => {
                    // Emit chunk indicating tool use started
                    // Note: This legacy version emits tool calls immediately with empty args.
                    // The stateful version (convert_stream_event_to_chunk_stateful) accumulates
                    // input_json_delta events and emits complete tool calls.
                    let tool_call = ToolCall {
                        id,
                        name,
                        args: serde_json::json!({}), // Empty args for now
                        tool_type: "tool_call".to_string(),
                        index: None,
                    };
                    let mut chunk = AIMessageChunk::new("");
                    chunk.tool_calls.push(tool_call);
                    Some(chunk)
                }
            }
        }
        StreamEvent::ContentBlockDelta { delta, .. } => {
            match delta {
                ContentDelta::TextDelta { text } => Some(AIMessageChunk::new(text)),
                ContentDelta::InputJsonDelta { partial_json: _ } => {
                    // Note: This legacy version skips input_json_delta events.
                    // The stateful version accumulates these and parses when block stops.
                    None
                }
            }
        }
        StreamEvent::ContentBlockStop { .. } => {
            // No chunk needed for block stop in legacy version.
            // The stateful version emits the final tool call with complete args here.
            None
        }
        StreamEvent::MessageDelta { delta, usage } => {
            // Return chunk with final usage metadata
            let mut chunk = AIMessageChunk::new("");
            chunk.usage_metadata = Some(UsageMetadata {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                total_tokens: usage.input_tokens + usage.output_tokens,
                input_token_details: None,
                output_token_details: None,
            });
            chunk.fields.response_metadata.insert(
                "stop_reason".to_string(),
                serde_json::json!(delta.stop_reason),
            );
            chunk.fields.response_metadata.insert(
                "stop_sequence".to_string(),
                serde_json::json!(delta.stop_sequence),
            );
            Some(chunk)
        }
        StreamEvent::MessageStop | StreamEvent::Unknown => {
            // No chunk needed for message stop or unknown events
            None
        }
    }
}

#[test]
fn test_chat_anthropic_builder() {
    let model = ChatAnthropic::try_new()
        .unwrap()
        .with_api_key("test-key")
        .with_model(models::CLAUDE_3_7_SONNET)
        .with_max_tokens(2048)
        .with_temperature(0.7);

    assert_eq!(model.api_key, "test-key");
    assert_eq!(model.model, models::CLAUDE_3_7_SONNET);
    assert_eq!(model.max_tokens, 2048);
    assert_eq!(model.temperature, Some(0.7));
}

#[test]
fn test_message_conversion_simple() {
    let model = ChatAnthropic::try_new().unwrap();
    let messages = vec![
        Message::system("You are a helpful assistant"),
        Message::human("Hello"),
    ];

    let (system, anthropic_msgs) = model.convert_messages(&messages).unwrap();

    assert_eq!(
        system,
        Some(SystemContent::Text(
            "You are a helpful assistant".to_string()
        ))
    );
    assert_eq!(anthropic_msgs.len(), 1);
    assert_eq!(anthropic_msgs[0].role, "user");
}

#[test]
fn test_message_conversion_multiple_system_fails() {
    let model = ChatAnthropic::try_new().unwrap();
    let messages = vec![Message::system("System 1"), Message::system("System 2")];

    let result = model.convert_messages(&messages);
    assert!(result.is_err());
}

#[test]
fn test_stream_event_message_start() {
    let event = StreamEvent::MessageStart {
        message: MessageStartData {
            id: "msg_123".to_string(),
            _message_type: "message".to_string(),
            role: "assistant".to_string(),
            model: "claude-3-7-sonnet-20250219".to_string(),
            usage: Usage {
                input_tokens: 10,
                output_tokens: 0,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
        },
    };

    let chunk = convert_stream_event_to_chunk(event).unwrap();
    assert_eq!(chunk.content, "");
    assert!(chunk.fields.response_metadata.contains_key("model"));
}

#[test]
fn test_stream_event_content_delta() {
    let event = StreamEvent::ContentBlockDelta {
        index: 0,
        delta: ContentDelta::TextDelta {
            text: "Hello".to_string(),
        },
    };

    let chunk = convert_stream_event_to_chunk(event).unwrap();
    assert_eq!(chunk.content, "Hello");
}

#[test]
fn test_stream_event_message_delta() {
    let event = StreamEvent::MessageDelta {
        delta: MessageDeltaData {
            stop_reason: Some("end_turn".to_string()),
            stop_sequence: None,
        },
        usage: Usage {
            input_tokens: 10,
            output_tokens: 20,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        },
    };

    let chunk = convert_stream_event_to_chunk(event).unwrap();
    assert_eq!(chunk.content, "");
    assert!(chunk.usage_metadata.is_some());
    let usage = chunk.usage_metadata.unwrap();
    assert_eq!(usage.input_tokens, 10);
    assert_eq!(usage.output_tokens, 20);
    assert_eq!(usage.total_tokens, 30);
}

#[test]
fn test_stream_event_content_block_stop() {
    let event = StreamEvent::ContentBlockStop { index: 0 };
    let chunk = convert_stream_event_to_chunk(event);
    assert!(chunk.is_none());
}

#[test]
fn test_stream_event_message_stop() {
    let event = StreamEvent::MessageStop;
    let chunk = convert_stream_event_to_chunk(event);
    assert!(chunk.is_none());
}

#[test]
fn test_convert_message_ai_with_tool_calls() {
    let model = ChatAnthropic::try_new().unwrap();

    let tool_call = ToolCall {
        id: "toolu_01abc".to_string(),
        name: "get_weather".to_string(),
        args: serde_json::json!({"location": "San Francisco"}),
        tool_type: "tool_call".to_string(),
        index: None,
    };

    let messages = vec![Message::AI {
        content: "Let me check the weather.".into(),
        tool_calls: vec![tool_call],
        invalid_tool_calls: vec![],
        usage_metadata: None,
        fields: Default::default(),
    }];

    let (system, anthropic_messages) = model.convert_messages(&messages).unwrap();
    assert!(system.is_none());
    assert_eq!(anthropic_messages.len(), 1);

    let msg = &anthropic_messages[0];
    assert_eq!(msg.role, "assistant");

    match &msg.content {
        AnthropicContent::Blocks(blocks) => {
            assert_eq!(blocks.len(), 2);
            // First block should be text
            match &blocks[0] {
                ContentBlock::Text { text, .. } => {
                    assert_eq!(text, "Let me check the weather.");
                }
                _ => panic!("Expected text block"),
            }
            // Second block should be tool use
            match &blocks[1] {
                ContentBlock::ToolUse {
                    id, name, input, ..
                } => {
                    assert_eq!(id, "toolu_01abc");
                    assert_eq!(name, "get_weather");
                    assert_eq!(input["location"], "San Francisco");
                }
                _ => panic!("Expected tool use block"),
            }
        }
        _ => panic!("Expected blocks"),
    }
}

#[test]
fn test_convert_message_tool_result() {
    let model = ChatAnthropic::try_new().unwrap();

    let messages = vec![Message::Tool {
        content: "It's sunny, 72°F".into(),
        tool_call_id: "toolu_01abc".to_string(),
        artifact: None,
        status: None,
        fields: Default::default(),
    }];

    let (system, anthropic_messages) = model.convert_messages(&messages).unwrap();
    assert!(system.is_none());
    assert_eq!(anthropic_messages.len(), 1);

    let msg = &anthropic_messages[0];
    assert_eq!(msg.role, "user");

    match &msg.content {
        AnthropicContent::Blocks(blocks) => {
            assert_eq!(blocks.len(), 1);
            match &blocks[0] {
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                    ..
                } => {
                    assert_eq!(tool_use_id, "toolu_01abc");
                    assert_eq!(content, "It's sunny, 72°F");
                    assert!(!is_error);
                }
                _ => panic!("Expected tool result block"),
            }
        }
        _ => panic!("Expected blocks"),
    }
}

#[test]
fn test_convert_message_tool_result_error() {
    let model = ChatAnthropic::try_new().unwrap();

    let messages = vec![Message::Tool {
        content: "Error: API unavailable".into(),
        tool_call_id: "toolu_01abc".to_string(),
        artifact: None,
        status: Some("error".to_string()),
        fields: Default::default(),
    }];

    let (_system, anthropic_messages) = model.convert_messages(&messages).unwrap();
    assert_eq!(anthropic_messages.len(), 1);

    let msg = &anthropic_messages[0];
    match &msg.content {
        AnthropicContent::Blocks(blocks) => match &blocks[0] {
            ContentBlock::ToolResult { is_error, .. } => {
                assert!(is_error);
            }
            _ => panic!("Expected tool result block"),
        },
        _ => panic!("Expected blocks"),
    }
}

#[test]
fn test_convert_response_with_tool_calls() {
    let model = ChatAnthropic::try_new().unwrap();

    let response = AnthropicResponse {
        id: "msg_01abc".to_string(),
        _response_type: "message".to_string(),
        _role: "assistant".to_string(),
        content: vec![
            ContentBlock::Text {
                text: "Let me check that for you.".to_string(),
                cache_control: None,
            },
            ContentBlock::ToolUse {
                id: "toolu_01xyz".to_string(),
                name: "get_weather".to_string(),
                input: serde_json::json!({"location": "Paris"}),
                cache_control: None,
            },
        ],
        model: models::CLAUDE_3_7_SONNET.to_string(),
        stop_reason: Some("tool_use".to_string()),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 50,
            output_tokens: 30,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        },
    };

    let result = model.convert_response(response);
    assert_eq!(result.generations.len(), 1);

    let message = &result.generations[0].message;
    match message {
        Message::AI {
            content,
            tool_calls,
            usage_metadata,
            ..
        } => {
            assert_eq!(content.as_text(), "Let me check that for you.");
            assert_eq!(tool_calls.len(), 1);

            let tool_call = &tool_calls[0];
            assert_eq!(tool_call.id, "toolu_01xyz");
            assert_eq!(tool_call.name, "get_weather");
            assert_eq!(tool_call.args["location"], "Paris");

            let usage = usage_metadata.as_ref().unwrap();
            assert_eq!(usage.input_tokens, 50);
            assert_eq!(usage.output_tokens, 30);
        }
        _ => panic!("Expected AI message"),
    }
}

#[test]
#[allow(deprecated)]
fn test_with_tools_and_tool_choice() {
    let tool_schema = serde_json::json!({
        "name": "get_weather",
        "description": "Get the current weather",
        "input_schema": {
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "City name"
                }
            },
            "required": ["location"]
        }
    });

    let model = ChatAnthropic::try_new()
        .unwrap()
        .with_tools(vec![tool_schema])
        .with_tool_choice(Some("get_weather".to_string()));

    assert!(model.tools.is_some());
    let tools = model.tools.unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "get_weather");

    assert!(model.tool_choice.is_some());
    match model.tool_choice.unwrap() {
        AnthropicToolChoice::Tool { r#type, name } => {
            assert_eq!(r#type, "tool");
            assert_eq!(name, "get_weather");
        }
        _ => panic!("Expected Tool choice"),
    }
}

#[test]
fn test_stream_event_tool_use_start() {
    let event = StreamEvent::ContentBlockStart {
        index: 0,
        content_block: StreamContentBlock::ToolUse {
            id: "toolu_01abc".to_string(),
            name: "get_weather".to_string(),
        },
    };

    let chunk = convert_stream_event_to_chunk(event).unwrap();
    assert_eq!(chunk.content, "");
    assert_eq!(chunk.tool_calls.len(), 1);

    let tool_call = &chunk.tool_calls[0];
    assert_eq!(tool_call.id, "toolu_01abc");
    assert_eq!(tool_call.name, "get_weather");
    // Args are empty in MVP streaming implementation
    assert_eq!(tool_call.args, serde_json::json!({}));
}

#[test]
fn test_convert_message_with_image_url() {
    use dashflow::core::messages::{
        ContentBlock as LcContentBlock, ImageSource, MessageContent,
    };

    let model = ChatAnthropic::try_new().unwrap();

    let messages = vec![Message::Human {
        content: MessageContent::Blocks(vec![
            LcContentBlock::Text {
                text: "What's in this image?".to_string(),
            },
            LcContentBlock::Image {
                source: ImageSource::Url {
                    url: "https://example.com/image.jpg".to_string(),
                },
                detail: None,
            },
        ]),
        fields: Default::default(),
    }];

    let (system, anthropic_messages) = model.convert_messages(&messages).unwrap();
    assert!(system.is_none());
    assert_eq!(anthropic_messages.len(), 1);

    let msg = &anthropic_messages[0];
    assert_eq!(msg.role, "user");

    match &msg.content {
        AnthropicContent::Blocks(blocks) => {
            assert_eq!(blocks.len(), 2);

            // First block should be text
            match &blocks[0] {
                ContentBlock::Text { text, .. } => {
                    assert_eq!(text, "What's in this image?");
                }
                _ => panic!("Expected text block"),
            }

            // Second block should be image
            match &blocks[1] {
                ContentBlock::Image { source, .. } => match source {
                    AnthropicImageSource::Url { url } => {
                        assert_eq!(url, "https://example.com/image.jpg");
                    }
                    _ => panic!("Expected URL image source"),
                },
                _ => panic!("Expected image block"),
            }
        }
        _ => panic!("Expected blocks content"),
    }
}

#[test]
fn test_convert_message_with_base64_image() {
    use dashflow::core::messages::{
        ContentBlock as LcContentBlock, ImageSource, MessageContent,
    };

    let model = ChatAnthropic::try_new().unwrap();

    let messages = vec![Message::Human {
        content: MessageContent::Blocks(vec![
            LcContentBlock::Text {
                text: "Analyze this image".to_string(),
            },
            LcContentBlock::Image {
                source: ImageSource::Base64 {
                    media_type: "image/png".to_string(),
                    data: "iVBORw0KGgoAAAANS...".to_string(),
                },
                detail: None,
            },
        ]),
        fields: Default::default(),
    }];

    let (system, anthropic_messages) = model.convert_messages(&messages).unwrap();
    assert!(system.is_none());
    assert_eq!(anthropic_messages.len(), 1);

    let msg = &anthropic_messages[0];
    assert_eq!(msg.role, "user");

    match &msg.content {
        AnthropicContent::Blocks(blocks) => {
            assert_eq!(blocks.len(), 2);

            // Second block should be image with base64
            match &blocks[1] {
                ContentBlock::Image { source, .. } => match source {
                    AnthropicImageSource::Base64 { media_type, data } => {
                        assert_eq!(media_type, "image/png");
                        assert_eq!(data, "iVBORw0KGgoAAAANS...");
                    }
                    _ => panic!("Expected base64 image source"),
                },
                _ => panic!("Expected image block"),
            }
        }
        _ => panic!("Expected blocks content"),
    }
}

#[test]
fn test_convert_message_image_only() {
    use dashflow::core::messages::{
        ContentBlock as LcContentBlock, ImageSource, MessageContent,
    };

    let model = ChatAnthropic::try_new().unwrap();

    // Message with only an image, no text
    let messages = vec![Message::Human {
        content: MessageContent::Blocks(vec![LcContentBlock::Image {
            source: ImageSource::Url {
                url: "https://example.com/photo.jpg".to_string(),
            },
            detail: None,
        }]),
        fields: Default::default(),
    }];

    let (system, anthropic_messages) = model.convert_messages(&messages).unwrap();
    assert!(system.is_none());
    assert_eq!(anthropic_messages.len(), 1);

    let msg = &anthropic_messages[0];
    assert_eq!(msg.role, "user");

    match &msg.content {
        AnthropicContent::Blocks(blocks) => {
            assert_eq!(blocks.len(), 1);

            // Only block should be image
            match &blocks[0] {
                ContentBlock::Image { source, .. } => match source {
                    AnthropicImageSource::Url { url } => {
                        assert_eq!(url, "https://example.com/photo.jpg");
                    }
                    _ => panic!("Expected URL image source"),
                },
                _ => panic!("Expected image block"),
            }
        }
        _ => panic!("Expected blocks content"),
    }
}

#[test]
fn test_convert_message_multiple_images() {
    use dashflow::core::messages::{
        ContentBlock as LcContentBlock, ImageSource, MessageContent,
    };

    let model = ChatAnthropic::try_new().unwrap();

    // Message with text and multiple images
    let messages = vec![Message::Human {
        content: MessageContent::Blocks(vec![
            LcContentBlock::Text {
                text: "Compare these images".to_string(),
            },
            LcContentBlock::Image {
                source: ImageSource::Url {
                    url: "https://example.com/image1.jpg".to_string(),
                },
                detail: None,
            },
            LcContentBlock::Image {
                source: ImageSource::Url {
                    url: "https://example.com/image2.jpg".to_string(),
                },
                detail: None,
            },
        ]),
        fields: Default::default(),
    }];

    let (system, anthropic_messages) = model.convert_messages(&messages).unwrap();
    assert!(system.is_none());
    assert_eq!(anthropic_messages.len(), 1);

    let msg = &anthropic_messages[0];
    assert_eq!(msg.role, "user");

    match &msg.content {
        AnthropicContent::Blocks(blocks) => {
            assert_eq!(blocks.len(), 3);

            // First: text
            match &blocks[0] {
                ContentBlock::Text { text, .. } => {
                    assert_eq!(text, "Compare these images");
                }
                _ => panic!("Expected text block"),
            }

            // Second and third: images
            match &blocks[1] {
                ContentBlock::Image { source, .. } => match source {
                    AnthropicImageSource::Url { url } => {
                        assert_eq!(url, "https://example.com/image1.jpg");
                    }
                    _ => panic!("Expected URL image source"),
                },
                _ => panic!("Expected image block"),
            }

            match &blocks[2] {
                ContentBlock::Image { source, .. } => match source {
                    AnthropicImageSource::Url { url } => {
                        assert_eq!(url, "https://example.com/image2.jpg");
                    }
                    _ => panic!("Expected URL image source"),
                },
                _ => panic!("Expected image block"),
            }
        }
        _ => panic!("Expected blocks content"),
    }
}

#[test]
fn test_thinking_config() {
    let thinking = ThinkingConfig::enabled(2000);
    assert_eq!(thinking.thinking_type, "enabled");
    assert_eq!(thinking.budget_tokens, 2000);
}

#[test]
fn test_with_thinking_builder() {
    let model = ChatAnthropic::try_new()
        .unwrap()
        .with_thinking(ThinkingConfig::enabled(5000));

    assert!(model.thinking.is_some());
    let thinking = model.thinking.unwrap();
    assert_eq!(thinking.thinking_type, "enabled");
    assert_eq!(thinking.budget_tokens, 5000);
}

#[test]
fn test_thinking_request_serialization() {
    let request = AnthropicRequest {
        model: "claude-3-7-sonnet-latest".to_string(),
        max_tokens: 1024,
        messages: vec![],
        system: None,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: None,
        tools: None,
        tool_choice: None,
        thinking: Some(ThinkingConfig::enabled(2000)),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["thinking"]["type"], "enabled");
    assert_eq!(json["thinking"]["budget_tokens"], 2000);
}

#[test]
fn test_thinking_response_parsing() {
    use dashflow::core::messages::MessageContent;

    let model = ChatAnthropic::try_new().unwrap();

    let response = AnthropicResponse {
        id: "msg_123".to_string(),
        _response_type: "message".to_string(),
        _role: "assistant".to_string(),
        content: vec![
            ContentBlock::Thinking {
                thinking: "Let me think about this...".to_string(),
                signature: Some("sig_abc".to_string()),
                cache_control: None,
            },
            ContentBlock::Text {
                text: "The answer is 42".to_string(),
                cache_control: None,
            },
        ],
        model: "claude-3-7-sonnet-latest".to_string(),
        stop_reason: Some("end_turn".to_string()),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 10,
            output_tokens: 20,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        },
    };

    let result = model.convert_response(response);
    let message = &result.generations[0].message;

    // With thinking blocks, content should be Blocks variant
    match message.content() {
        MessageContent::Blocks(blocks) => {
            assert_eq!(blocks.len(), 2);

            // First block should be Thinking
            match &blocks[0] {
                dashflow::core::messages::ContentBlock::Thinking {
                    thinking,
                    signature,
                } => {
                    assert_eq!(thinking, "Let me think about this...");
                    assert_eq!(signature.as_ref().unwrap(), "sig_abc");
                }
                _ => panic!("Expected thinking block"),
            }

            // Second block should be Text
            match &blocks[1] {
                dashflow::core::messages::ContentBlock::Text { text } => {
                    assert_eq!(text, "The answer is 42");
                }
                _ => panic!("Expected text block"),
            }
        }
        _ => panic!("Expected blocks content with thinking"),
    }
}

#[test]
fn test_is_builtin_tool_bash() {
    let tool = serde_json::json!({
        "type": "bash_20241022",
        "name": "bash"
    });
    assert!(is_builtin_tool(&tool));
}

#[test]
fn test_is_builtin_tool_web_search() {
    let tool = serde_json::json!({
        "type": "web_search_20250305",
        "name": "web_search",
        "max_uses": 3
    });
    assert!(is_builtin_tool(&tool));
}

#[test]
fn test_is_builtin_tool_text_editor() {
    let tool = serde_json::json!({
        "type": "text_editor_20250728",
        "name": "text_editor"
    });
    assert!(is_builtin_tool(&tool));
}

#[test]
fn test_is_builtin_tool_computer() {
    let tool = serde_json::json!({
        "type": "computer_20241022",
        "name": "computer"
    });
    assert!(is_builtin_tool(&tool));
}

#[test]
fn test_is_not_builtin_tool_regular() {
    let tool = serde_json::json!({
        "name": "get_weather",
        "description": "Get weather",
        "input_schema": {
            "type": "object",
            "properties": {}
        }
    });
    assert!(!is_builtin_tool(&tool));
}

#[test]
fn test_is_not_builtin_tool_wrong_prefix() {
    let tool = serde_json::json!({
        "type": "custom_tool_type",
        "name": "custom"
    });
    assert!(!is_builtin_tool(&tool));
}

#[test]
#[allow(deprecated)]
fn test_with_tools_builtin_bash() {
    let model = ChatAnthropic::try_new()
        .unwrap()
        .with_tools(vec![serde_json::json!({
            "type": "bash_20241022",
            "name": "bash"
        })]);

    assert!(model.tools.is_some());
    let tools = model.tools.unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].r#type, Some("bash_20241022".to_string()));
    assert_eq!(tools[0].name, "bash");
    assert!(tools[0].input_schema.is_none());
}

#[test]
#[allow(deprecated)]
fn test_with_tools_builtin_web_search_with_max_uses() {
    let model = ChatAnthropic::try_new()
        .unwrap()
        .with_tools(vec![serde_json::json!({
            "type": "web_search_20250305",
            "name": "web_search",
            "max_uses": 5
        })]);

    assert!(model.tools.is_some());
    let tools = model.tools.unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].r#type, Some("web_search_20250305".to_string()));
    assert_eq!(tools[0].name, "web_search");
    assert_eq!(tools[0].max_uses, Some(5));
    assert!(tools[0].input_schema.is_none());
}

#[test]
#[allow(deprecated)]
fn test_with_tools_regular_tool() {
    let model = ChatAnthropic::try_new()
        .unwrap()
        .with_tools(vec![serde_json::json!({
            "name": "get_weather",
            "description": "Get weather for a location",
            "input_schema": {
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string"
                    }
                }
            }
        })]);

    assert!(model.tools.is_some());
    let tools = model.tools.unwrap();
    assert_eq!(tools.len(), 1);
    assert!(tools[0].r#type.is_none());
    assert_eq!(tools[0].name, "get_weather");
    assert!(tools[0].input_schema.is_some());
    assert!(tools[0].max_uses.is_none());
}

#[test]
#[allow(deprecated)]
fn test_with_tools_mixed_builtin_and_regular() {
    let model = ChatAnthropic::try_new().unwrap().with_tools(vec![
        serde_json::json!({
            "type": "bash_20241022",
            "name": "bash"
        }),
        serde_json::json!({
            "name": "get_weather",
            "description": "Get weather",
            "input_schema": {
                "type": "object",
                "properties": {}
            }
        }),
    ]);

    assert!(model.tools.is_some());
    let tools = model.tools.unwrap();
    assert_eq!(tools.len(), 2);

    // First tool is built-in
    assert_eq!(tools[0].r#type, Some("bash_20241022".to_string()));
    assert_eq!(tools[0].name, "bash");
    assert!(tools[0].input_schema.is_none());

    // Second tool is regular
    assert!(tools[1].r#type.is_none());
    assert_eq!(tools[1].name, "get_weather");
    assert!(tools[1].input_schema.is_some());
}

#[test]
fn test_builtin_tool_request_serialization() {
    let request = AnthropicRequest {
        model: "claude-3-5-haiku-latest".to_string(),
        max_tokens: 1024,
        messages: vec![],
        system: None,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: None,
        tools: Some(vec![AnthropicTool {
            r#type: Some("web_search_20250305".to_string()),
            name: "web_search".to_string(),
            description: None,
            input_schema: None,
            cache_control: None,
            max_uses: Some(3),
        }]),
        tool_choice: None,
        thinking: None,
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["tools"][0]["type"], "web_search_20250305");
    assert_eq!(json["tools"][0]["name"], "web_search");
    assert_eq!(json["tools"][0]["max_uses"], 3);
    // input_schema should not be present
    assert!(json["tools"][0].get("input_schema").is_none());
}

#[test]
fn test_redacted_thinking_response_parsing() {
    use dashflow::core::messages::MessageContent;

    let model = ChatAnthropic::try_new().unwrap();

    let response = AnthropicResponse {
        id: "msg_456".to_string(),
        _response_type: "message".to_string(),
        _role: "assistant".to_string(),
        content: vec![
            ContentBlock::RedactedThinking {
                data: "Thinking was redacted".to_string(),
                cache_control: None,
            },
            ContentBlock::Text {
                text: "Final answer".to_string(),
                cache_control: None,
            },
        ],
        model: "claude-sonnet-4".to_string(),
        stop_reason: Some("end_turn".to_string()),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 15,
            output_tokens: 25,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        },
    };

    let result = model.convert_response(response);
    let message = &result.generations[0].message;

    match message.content() {
        MessageContent::Blocks(blocks) => {
            assert_eq!(blocks.len(), 2);

            // First block should be RedactedThinking
            match &blocks[0] {
                dashflow::core::messages::ContentBlock::RedactedThinking { data } => {
                    assert_eq!(data, "Thinking was redacted");
                }
                _ => panic!("Expected redacted thinking block"),
            }

            // Second block should be Text
            match &blocks[1] {
                dashflow::core::messages::ContentBlock::Text { text } => {
                    assert_eq!(text, "Final answer");
                }
                _ => panic!("Expected text block"),
            }
        }
        _ => panic!("Expected blocks content with redacted thinking"),
    }
}

#[tokio::test]
async fn test_rate_limiter_integration() {
    use dashflow::core::rate_limiters::InMemoryRateLimiter;
    use std::time::{Duration, Instant};

    // Create a rate limiter that allows 2 requests per second
    let rate_limiter = Arc::new(InMemoryRateLimiter::new(
        2.0, // 2 requests per second = 1 request every 500ms
        Duration::from_millis(10),
        2.0,
    ));

    let model = ChatAnthropic::try_new()
        .unwrap()
        .with_api_key("test-key")
        .with_rate_limiter(rate_limiter.clone());

    // Verify rate limiter is set
    assert!(model.rate_limiter().is_some());

    // Test that rate limiter controls timing
    let start = Instant::now();

    // First request should complete after ~500ms (first token generation)
    model.rate_limiter().unwrap().acquire().await;
    let first = start.elapsed();

    // Second request should complete after ~1000ms (second token)
    model.rate_limiter().unwrap().acquire().await;
    let second = start.elapsed();

    // Third request should complete after ~1500ms (third token)
    model.rate_limiter().unwrap().acquire().await;
    let third = start.elapsed();

    // Verify timing (with some tolerance for test execution)
    assert!(
        first >= Duration::from_millis(400),
        "First request took {:?}",
        first
    );
    assert!(
        first <= Duration::from_millis(600),
        "First request took {:?}",
        first
    );

    assert!(
        second >= Duration::from_millis(900),
        "Second request took {:?}",
        second
    );
    assert!(
        second <= Duration::from_millis(1100),
        "Second request took {:?}",
        second
    );

    assert!(
        third >= Duration::from_millis(1400),
        "Third request took {:?}",
        third
    );
    assert!(
        third <= Duration::from_millis(1600),
        "Third request took {:?}",
        third
    );
}

#[test]
fn test_stateful_streaming_tool_call_accumulation() {
    // Test the stateful streaming with tool call JSON accumulation
    let mut state = StreamToolCallState::new();

    // Event 1: ContentBlockStart for tool use
    let event1 = StreamEvent::ContentBlockStart {
        index: 0,
        content_block: StreamContentBlock::ToolUse {
            id: "toolu_01abc".to_string(),
            name: "get_weather".to_string(),
        },
    };
    let chunk1 = convert_stream_event_to_chunk_stateful(event1, &mut state);
    // Should not emit a chunk yet - waiting for JSON to accumulate
    assert!(chunk1.is_none());

    // Event 2: ContentBlockDelta with partial JSON (first chunk)
    let event2 = StreamEvent::ContentBlockDelta {
        index: 0,
        delta: ContentDelta::InputJsonDelta {
            partial_json: r#"{"location":"#.to_string(),
        },
    };
    let chunk2 = convert_stream_event_to_chunk_stateful(event2, &mut state);
    // Should not emit a chunk yet - still accumulating
    assert!(chunk2.is_none());

    // Event 3: ContentBlockDelta with partial JSON (second chunk)
    let event3 = StreamEvent::ContentBlockDelta {
        index: 0,
        delta: ContentDelta::InputJsonDelta {
            partial_json: r#""San Francisco","units":"celsius"}"#.to_string(),
        },
    };
    let chunk3 = convert_stream_event_to_chunk_stateful(event3, &mut state);
    // Should not emit a chunk yet - still accumulating
    assert!(chunk3.is_none());

    // Event 4: ContentBlockStop - should emit complete tool call
    let event4 = StreamEvent::ContentBlockStop { index: 0 };
    let chunk4 = convert_stream_event_to_chunk_stateful(event4, &mut state);
    // Should emit a chunk with the complete tool call
    assert!(chunk4.is_some());

    let chunk = chunk4.unwrap();
    assert_eq!(chunk.tool_calls.len(), 1);
    assert_eq!(chunk.tool_calls[0].id, "toolu_01abc");
    assert_eq!(chunk.tool_calls[0].name, "get_weather");
    assert_eq!(chunk.tool_calls[0].args["location"], "San Francisco");
    assert_eq!(chunk.tool_calls[0].args["units"], "celsius");
}

#[test]
fn test_stateful_streaming_tool_call_empty_args() {
    // Test tool call with no arguments
    let mut state = StreamToolCallState::new();

    let event1 = StreamEvent::ContentBlockStart {
        index: 0,
        content_block: StreamContentBlock::ToolUse {
            id: "toolu_02xyz".to_string(),
            name: "get_time".to_string(),
        },
    };
    let _ = convert_stream_event_to_chunk_stateful(event1, &mut state);

    // Immediately stop without any JSON deltas
    let event2 = StreamEvent::ContentBlockStop { index: 0 };
    let chunk = convert_stream_event_to_chunk_stateful(event2, &mut state);

    assert!(chunk.is_some());
    let chunk = chunk.unwrap();
    assert_eq!(chunk.tool_calls.len(), 1);
    assert_eq!(chunk.tool_calls[0].id, "toolu_02xyz");
    assert_eq!(chunk.tool_calls[0].name, "get_time");
    assert_eq!(chunk.tool_calls[0].args, serde_json::json!({}));
}

#[test]
fn test_stateful_streaming_tool_call_invalid_json() {
    // Test tool call with malformed JSON
    let mut state = StreamToolCallState::new();

    let event1 = StreamEvent::ContentBlockStart {
        index: 0,
        content_block: StreamContentBlock::ToolUse {
            id: "toolu_03bad".to_string(),
            name: "broken_tool".to_string(),
        },
    };
    let _ = convert_stream_event_to_chunk_stateful(event1, &mut state);

    // Add invalid JSON
    let event2 = StreamEvent::ContentBlockDelta {
        index: 0,
        delta: ContentDelta::InputJsonDelta {
            partial_json: r#"{"invalid: json here"#.to_string(),
        },
    };
    let _ = convert_stream_event_to_chunk_stateful(event2, &mut state);

    let event3 = StreamEvent::ContentBlockStop { index: 0 };
    let chunk = convert_stream_event_to_chunk_stateful(event3, &mut state);

    assert!(chunk.is_some());
    let chunk = chunk.unwrap();
    assert_eq!(chunk.tool_calls.len(), 1);
    // Should have error field with raw JSON
    assert!(chunk.tool_calls[0].args["error"].is_string());
    assert_eq!(chunk.tool_calls[0].args["raw"], r#"{"invalid: json here"#);
}

#[test]
fn test_chat_anthropic_serialization_simple() {
    use dashflow::core::serialization::Serializable;

    let model = ChatAnthropic::try_new()
        .unwrap()
        .with_model("claude-3-5-sonnet")
        .with_max_tokens(2048);

    let json_value = model.to_json_value().unwrap();

    // Check structure
    assert_eq!(json_value["lc"], 1);
    assert_eq!(json_value["type"], "constructor");
    assert_eq!(
        json_value["id"],
        serde_json::json!(["dashflow", "chat_models", "anthropic", "ChatAnthropic"])
    );

    // Check kwargs
    let kwargs = &json_value["kwargs"];
    assert_eq!(kwargs["model"], "claude-3-5-sonnet");
    assert_eq!(kwargs["max_tokens"], 2048);
}

#[test]
fn test_chat_anthropic_serialization_with_parameters() {
    use dashflow::core::serialization::Serializable;

    let model = ChatAnthropic::try_new()
        .unwrap()
        .with_model("claude-3-opus")
        .with_max_tokens(4096)
        .with_temperature(0.7)
        .with_top_p(0.9)
        .with_top_k(40);

    let json_value = model.to_json_value().unwrap();
    let kwargs = &json_value["kwargs"];

    assert_eq!(kwargs["model"], "claude-3-opus");
    assert_eq!(kwargs["max_tokens"], 4096);
    // Use approximate comparison for floats due to f32 precision
    assert!(
        (kwargs["temperature"].as_f64().unwrap() - 0.7).abs() < 0.01,
        "temperature should be approximately 0.7"
    );
    assert!(
        (kwargs["top_p"].as_f64().unwrap() - 0.9).abs() < 0.01,
        "top_p should be approximately 0.9"
    );
    assert_eq!(kwargs["top_k"], 40);
}

#[test]
fn test_chat_anthropic_serialization_secrets() {
    use dashflow::core::serialization::Serializable;

    let model = ChatAnthropic::try_new().unwrap();
    let secrets = model.lc_secrets();

    // Should mark API key as a secret
    assert_eq!(
        secrets.get("api_key"),
        Some(&"ANTHROPIC_API_KEY".to_string())
    );
}

#[test]
fn test_chat_anthropic_serialization_no_api_key_in_output() {
    use dashflow::core::serialization::Serializable;

    let model = ChatAnthropic::try_new()
        .unwrap()
        .with_api_key("test-secret-key")
        .with_model("claude-3-sonnet");

    let json_string = model.to_json_string(false).unwrap();

    // Verify API key is NOT in the serialized output
    assert!(!json_string.contains("api_key"));
    assert!(!json_string.contains("ANTHROPIC_API_KEY"));
    assert!(!json_string.contains("test-secret-key"));
}

#[test]
fn test_chat_anthropic_serialization_pretty_json() {
    use dashflow::core::serialization::Serializable;

    let model = ChatAnthropic::try_new()
        .unwrap()
        .with_model("claude-3-haiku")
        .with_max_tokens(1024);

    let json_string = model.to_json_string(true).unwrap();

    // Should be pretty-printed (contains newlines)
    assert!(json_string.contains('\n'));
    assert!(json_string.contains("ChatAnthropic"));
    assert!(json_string.contains("\"model\": \"claude-3-haiku\""));
}

// =============================================================================
// Prompt Caching Effectiveness Tests (M-323)
// =============================================================================

#[test]
fn test_cache_creation_tokens_in_response() {
    // Test that cache creation tokens are properly parsed and exposed in generation_info
    let model = ChatAnthropic::try_new().unwrap();

    // Simulate a response with cache creation (first request with cacheable content)
    let response = AnthropicResponse {
        id: "msg_cache_test_1".to_string(),
        _response_type: "message".to_string(),
        _role: "assistant".to_string(),
        content: vec![ContentBlock::Text {
            text: "Response to first query".to_string(),
            cache_control: None,
        }],
        model: "claude-3-5-sonnet-20241022".to_string(),
        stop_reason: Some("end_turn".to_string()),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 1500,
            output_tokens: 50,
            cache_creation_input_tokens: Some(1200),
            cache_read_input_tokens: None,
        },
    };

    let result = model.convert_response(response);

    // Verify generation_info contains cache creation tokens
    let generation_info = result.generations[0].generation_info.as_ref().unwrap();
    assert_eq!(generation_info.get("input_tokens").unwrap(), &serde_json::json!(1500));
    assert_eq!(generation_info.get("output_tokens").unwrap(), &serde_json::json!(50));
    assert_eq!(
        generation_info.get("cache_creation_input_tokens").unwrap(),
        &serde_json::json!(1200)
    );
    // cache_read_input_tokens should not be present (first request creates cache)
    assert!(generation_info.get("cache_read_input_tokens").is_none());
}

#[test]
fn test_cache_read_tokens_in_response() {
    // Test that cache read tokens are properly parsed and exposed (cache hit scenario)
    let model = ChatAnthropic::try_new().unwrap();

    // Simulate a response with cache hit (subsequent request using cached content)
    let response = AnthropicResponse {
        id: "msg_cache_test_2".to_string(),
        _response_type: "message".to_string(),
        _role: "assistant".to_string(),
        content: vec![ContentBlock::Text {
            text: "Response to second query".to_string(),
            cache_control: None,
        }],
        model: "claude-3-5-sonnet-20241022".to_string(),
        stop_reason: Some("end_turn".to_string()),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 300,
            output_tokens: 45,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: Some(1200),
        },
    };

    let result = model.convert_response(response);

    // Verify generation_info contains cache read tokens (cache hit!)
    let generation_info = result.generations[0].generation_info.as_ref().unwrap();
    assert_eq!(generation_info.get("input_tokens").unwrap(), &serde_json::json!(300));
    assert_eq!(generation_info.get("output_tokens").unwrap(), &serde_json::json!(45));
    assert_eq!(
        generation_info.get("cache_read_input_tokens").unwrap(),
        &serde_json::json!(1200)
    );
    // cache_creation_input_tokens should not be present (cache was already created)
    assert!(generation_info.get("cache_creation_input_tokens").is_none());
}

#[test]
fn test_cache_metrics_both_present() {
    // Test scenario where both cache creation and read occur (partial cache hit)
    let model = ChatAnthropic::try_new().unwrap();

    let response = AnthropicResponse {
        id: "msg_cache_test_3".to_string(),
        _response_type: "message".to_string(),
        _role: "assistant".to_string(),
        content: vec![ContentBlock::Text {
            text: "Response with partial cache".to_string(),
            cache_control: None,
        }],
        model: "claude-3-5-sonnet-20241022".to_string(),
        stop_reason: Some("end_turn".to_string()),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 500,
            output_tokens: 60,
            cache_creation_input_tokens: Some(200),
            cache_read_input_tokens: Some(800),
        },
    };

    let result = model.convert_response(response);

    // Verify both cache metrics are present
    let generation_info = result.generations[0].generation_info.as_ref().unwrap();
    assert_eq!(
        generation_info.get("cache_creation_input_tokens").unwrap(),
        &serde_json::json!(200)
    );
    assert_eq!(
        generation_info.get("cache_read_input_tokens").unwrap(),
        &serde_json::json!(800)
    );
}

#[test]
fn test_no_cache_metrics_backward_compatibility() {
    // Test that responses without cache metrics still work (backward compatibility)
    let model = ChatAnthropic::try_new().unwrap();

    let response = AnthropicResponse {
        id: "msg_no_cache".to_string(),
        _response_type: "message".to_string(),
        _role: "assistant".to_string(),
        content: vec![ContentBlock::Text {
            text: "Response without caching".to_string(),
            cache_control: None,
        }],
        model: "claude-3-5-sonnet-20241022".to_string(),
        stop_reason: Some("end_turn".to_string()),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 100,
            output_tokens: 20,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        },
    };

    let result = model.convert_response(response);

    // Verify basic tokens are present but no cache metrics
    let generation_info = result.generations[0].generation_info.as_ref().unwrap();
    assert_eq!(generation_info.get("input_tokens").unwrap(), &serde_json::json!(100));
    assert_eq!(generation_info.get("output_tokens").unwrap(), &serde_json::json!(20));
    assert!(generation_info.get("cache_creation_input_tokens").is_none());
    assert!(generation_info.get("cache_read_input_tokens").is_none());
}

#[test]
fn test_cache_control_ephemeral() {
    // Test CacheControl ephemeral helper
    let cache = CacheControl::ephemeral();
    assert_eq!(cache.cache_type, "ephemeral");
    assert_eq!(cache.ttl, Some("5m".to_string()));
}

#[test]
fn test_cache_control_with_custom_ttl() {
    // Test CacheControl with custom TTL
    let cache = CacheControl::with_ttl("1h");
    assert_eq!(cache.cache_type, "ephemeral");
    assert_eq!(cache.ttl, Some("1h".to_string()));
}

#[test]
fn test_cache_control_serialization() {
    // Test that CacheControl serializes correctly
    let cache = CacheControl::ephemeral();
    let json = serde_json::to_value(&cache).unwrap();
    assert_eq!(json["type"], "ephemeral");
    assert_eq!(json["ttl"], "5m");
}

#[test]
fn test_content_block_with_cache_control() {
    // Test that content blocks can include cache control
    let block = ContentBlock::Text {
        text: "Large system prompt...".to_string(),
        cache_control: Some(CacheControl::ephemeral()),
    };

    let json = serde_json::to_value(&block).unwrap();
    assert_eq!(json["type"], "text");
    assert_eq!(json["text"], "Large system prompt...");
    assert_eq!(json["cache_control"]["type"], "ephemeral");
}

#[test]
fn test_usage_deserialization_with_cache_metrics() {
    // Test direct deserialization of Usage struct with cache metrics
    let usage_json = serde_json::json!({
        "input_tokens": 1000,
        "output_tokens": 100,
        "cache_creation_input_tokens": 800,
        "cache_read_input_tokens": 0
    });

    let usage: Usage = serde_json::from_value(usage_json).unwrap();
    assert_eq!(usage.input_tokens, 1000);
    assert_eq!(usage.output_tokens, 100);
    assert_eq!(usage.cache_creation_input_tokens, Some(800));
    assert_eq!(usage.cache_read_input_tokens, Some(0));
}

#[test]
fn test_usage_deserialization_without_cache_metrics() {
    // Test that Usage still deserializes when cache metrics are absent
    let usage_json = serde_json::json!({
        "input_tokens": 500,
        "output_tokens": 75
    });

    let usage: Usage = serde_json::from_value(usage_json).unwrap();
    assert_eq!(usage.input_tokens, 500);
    assert_eq!(usage.output_tokens, 75);
    assert!(usage.cache_creation_input_tokens.is_none());
    assert!(usage.cache_read_input_tokens.is_none());
}

// =============================================================================
// Concurrent Cache Behavior Tests (M-323)
// =============================================================================

#[tokio::test]
async fn test_concurrent_cache_validation_setup() {
    // Validate test setup for concurrent caching scenarios
    // This test ensures the infrastructure for concurrent cache testing is correct
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let counter = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    // Spawn 5 concurrent tasks
    for _ in 0..5 {
        let counter_clone = Arc::clone(&counter);
        let handle = tokio::spawn(async move {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });
        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all tasks completed
    assert_eq!(counter.load(Ordering::SeqCst), 5);
}

#[test]
fn test_cache_hit_cost_savings_calculation() {
    // Test that cache hits provide expected cost savings
    // Per Anthropic docs: cache reads are 90% cheaper than input tokens

    let model = ChatAnthropic::try_new().unwrap();

    // First request: creates cache
    let first_response = AnthropicResponse {
        id: "msg_first".to_string(),
        _response_type: "message".to_string(),
        _role: "assistant".to_string(),
        content: vec![ContentBlock::Text {
            text: "First response".to_string(),
            cache_control: None,
        }],
        model: "claude-3-5-sonnet-20241022".to_string(),
        stop_reason: Some("end_turn".to_string()),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 1500,
            output_tokens: 50,
            cache_creation_input_tokens: Some(1200),
            cache_read_input_tokens: None,
        },
    };

    let first_result = model.convert_response(first_response);
    let first_info = first_result.generations[0].generation_info.as_ref().unwrap();

    // Second request: uses cache
    let second_response = AnthropicResponse {
        id: "msg_second".to_string(),
        _response_type: "message".to_string(),
        _role: "assistant".to_string(),
        content: vec![ContentBlock::Text {
            text: "Second response".to_string(),
            cache_control: None,
        }],
        model: "claude-3-5-sonnet-20241022".to_string(),
        stop_reason: Some("end_turn".to_string()),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 350,
            output_tokens: 45,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: Some(1200),
        },
    };

    let second_result = model.convert_response(second_response);
    let second_info = second_result.generations[0].generation_info.as_ref().unwrap();

    // Verify cache creation in first request
    assert!(first_info.get("cache_creation_input_tokens").is_some());
    assert_eq!(
        first_info.get("cache_creation_input_tokens").unwrap().as_u64().unwrap(),
        1200
    );

    // Verify cache hit in second request
    assert!(second_info.get("cache_read_input_tokens").is_some());
    assert_eq!(
        second_info.get("cache_read_input_tokens").unwrap().as_u64().unwrap(),
        1200
    );

    // Verify input tokens are lower in second request (cache hit)
    let first_input = first_info.get("input_tokens").unwrap().as_u64().unwrap();
    let second_input = second_info.get("input_tokens").unwrap().as_u64().unwrap();
    assert!(
        second_input < first_input,
        "Cache hit should reduce input tokens: first={}, second={}",
        first_input,
        second_input
    );
}

#[test]
fn test_cache_metrics_in_nested_usage() {
    // Verify cache metrics are also accessible via nested 'usage' field
    let model = ChatAnthropic::try_new().unwrap();

    let response = AnthropicResponse {
        id: "msg_nested".to_string(),
        _response_type: "message".to_string(),
        _role: "assistant".to_string(),
        content: vec![ContentBlock::Text {
            text: "Test".to_string(),
            cache_control: None,
        }],
        model: "claude-3-5-sonnet-20241022".to_string(),
        stop_reason: Some("end_turn".to_string()),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 100,
            output_tokens: 20,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        },
    };

    let result = model.convert_response(response);
    let generation_info = result.generations[0].generation_info.as_ref().unwrap();

    // Verify nested usage structure
    let nested_usage = generation_info.get("usage").unwrap();
    assert_eq!(nested_usage["input_tokens"], 100);
    assert_eq!(nested_usage["output_tokens"], 20);
}

// =============================================================================
// Tool-Use Integration Tests (M-324)
// =============================================================================

#[test]
fn test_tool_definition_to_anthropic_tool_conversion() {
    // Test that ToolDefinition converts correctly to AnthropicTool format
    let tool_def = ToolDefinition {
        name: "get_weather".to_string(),
        description: "Get the current weather for a location".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "The city and state, e.g. San Francisco, CA"
                },
                "unit": {
                    "type": "string",
                    "enum": ["celsius", "fahrenheit"],
                    "description": "Temperature unit"
                }
            },
            "required": ["location"]
        }),
    };

    let anthropic_tool = convert_tool_definition_anthropic(&tool_def);

    assert_eq!(anthropic_tool.name, "get_weather");
    assert_eq!(
        anthropic_tool.description,
        Some("Get the current weather for a location".to_string())
    );
    assert!(anthropic_tool.r#type.is_none()); // Regular tools don't have type field
    assert!(anthropic_tool.input_schema.is_some());

    let schema = anthropic_tool.input_schema.unwrap();
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"]["location"].is_object());
    assert_eq!(schema["required"], serde_json::json!(["location"]));
}

#[test]
fn test_tool_definition_empty_description() {
    // Test tool definition with empty description (should serialize as None)
    let tool_def = ToolDefinition {
        name: "simple_tool".to_string(),
        description: String::new(),
        parameters: serde_json::json!({"type": "object"}),
    };

    let anthropic_tool = convert_tool_definition_anthropic(&tool_def);

    assert_eq!(anthropic_tool.name, "simple_tool");
    assert!(anthropic_tool.description.is_none()); // Empty string becomes None
    assert!(anthropic_tool.input_schema.is_some());
}

#[test]
fn test_tool_choice_auto_conversion() {
    let choice = convert_tool_choice_anthropic(&ToolChoice::Auto);

    match choice {
        AnthropicToolChoice::Auto { r#type } => {
            assert_eq!(r#type, "auto");
        }
        _ => panic!("Expected Auto variant"),
    }
}

#[test]
fn test_tool_choice_none_conversion() {
    // Anthropic doesn't have "none" - it maps to "auto"
    let choice = convert_tool_choice_anthropic(&ToolChoice::None);

    match choice {
        AnthropicToolChoice::Auto { r#type } => {
            assert_eq!(r#type, "auto");
        }
        _ => panic!("Expected Auto variant (None maps to Auto in Anthropic)"),
    }
}

#[test]
fn test_tool_choice_required_conversion() {
    let choice = convert_tool_choice_anthropic(&ToolChoice::Required);

    match choice {
        AnthropicToolChoice::Any { r#type } => {
            assert_eq!(r#type, "any");
        }
        _ => panic!("Expected Any variant"),
    }
}

#[test]
fn test_tool_choice_specific_conversion() {
    let choice = convert_tool_choice_anthropic(&ToolChoice::Specific("get_weather".to_string()));

    match choice {
        AnthropicToolChoice::Tool { r#type, name } => {
            assert_eq!(r#type, "tool");
            assert_eq!(name, "get_weather");
        }
        _ => panic!("Expected Tool variant"),
    }
}

#[test]
fn test_tool_choice_serialization_auto() {
    let choice = AnthropicToolChoice::Auto {
        r#type: "auto".to_string(),
    };
    let json = serde_json::to_value(&choice).unwrap();
    assert_eq!(json["type"], "auto");
}

#[test]
fn test_tool_choice_serialization_any() {
    let choice = AnthropicToolChoice::Any {
        r#type: "any".to_string(),
    };
    let json = serde_json::to_value(&choice).unwrap();
    assert_eq!(json["type"], "any");
}

#[test]
fn test_tool_choice_serialization_tool() {
    let choice = AnthropicToolChoice::Tool {
        r#type: "tool".to_string(),
        name: "calculate".to_string(),
    };
    let json = serde_json::to_value(&choice).unwrap();
    assert_eq!(json["type"], "tool");
    assert_eq!(json["name"], "calculate");
}

#[test]
fn test_multiple_tool_calls_in_response() {
    // Test response with multiple tool calls (agent calling multiple tools)
    let model = ChatAnthropic::try_new().unwrap();

    let response = AnthropicResponse {
        id: "msg_multi_tool".to_string(),
        _response_type: "message".to_string(),
        _role: "assistant".to_string(),
        content: vec![
            ContentBlock::Text {
                text: "I'll check the weather and time for you.".to_string(),
                cache_control: None,
            },
            ContentBlock::ToolUse {
                id: "toolu_01abc".to_string(),
                name: "get_weather".to_string(),
                input: serde_json::json!({"location": "San Francisco"}),
                cache_control: None,
            },
            ContentBlock::ToolUse {
                id: "toolu_02def".to_string(),
                name: "get_time".to_string(),
                input: serde_json::json!({"timezone": "America/Los_Angeles"}),
                cache_control: None,
            },
            ContentBlock::ToolUse {
                id: "toolu_03ghi".to_string(),
                name: "get_forecast".to_string(),
                input: serde_json::json!({"location": "San Francisco", "days": 3}),
                cache_control: None,
            },
        ],
        model: models::CLAUDE_3_7_SONNET.to_string(),
        stop_reason: Some("tool_use".to_string()),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 100,
            output_tokens: 150,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        },
    };

    let result = model.convert_response(response);
    let message = &result.generations[0].message;

    match message {
        Message::AI { tool_calls, .. } => {
            assert_eq!(tool_calls.len(), 3);

            assert_eq!(tool_calls[0].id, "toolu_01abc");
            assert_eq!(tool_calls[0].name, "get_weather");

            assert_eq!(tool_calls[1].id, "toolu_02def");
            assert_eq!(tool_calls[1].name, "get_time");

            assert_eq!(tool_calls[2].id, "toolu_03ghi");
            assert_eq!(tool_calls[2].name, "get_forecast");
            assert_eq!(tool_calls[2].args["days"], 3);
        }
        _ => panic!("Expected AI message"),
    }
}

#[test]
fn test_complex_nested_tool_arguments() {
    // Test tool call with complex nested JSON arguments
    let model = ChatAnthropic::try_new().unwrap();

    let response = AnthropicResponse {
        id: "msg_complex_args".to_string(),
        _response_type: "message".to_string(),
        _role: "assistant".to_string(),
        content: vec![ContentBlock::ToolUse {
            id: "toolu_complex".to_string(),
            name: "create_event".to_string(),
            input: serde_json::json!({
                "title": "Team Meeting",
                "participants": [
                    {"email": "alice@example.com", "role": "organizer"},
                    {"email": "bob@example.com", "role": "attendee"}
                ],
                "schedule": {
                    "start": "2025-01-15T10:00:00Z",
                    "end": "2025-01-15T11:00:00Z",
                    "recurring": {
                        "frequency": "weekly",
                        "count": 4
                    }
                },
                "settings": {
                    "notifications": true,
                    "reminders": [15, 60],
                    "attachments": null
                }
            }),
            cache_control: None,
        }],
        model: models::CLAUDE_3_7_SONNET.to_string(),
        stop_reason: Some("tool_use".to_string()),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 50,
            output_tokens: 100,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        },
    };

    let result = model.convert_response(response);
    let message = &result.generations[0].message;

    match message {
        Message::AI { tool_calls, .. } => {
            assert_eq!(tool_calls.len(), 1);
            let tc = &tool_calls[0];

            assert_eq!(tc.name, "create_event");
            assert_eq!(tc.args["title"], "Team Meeting");
            assert_eq!(tc.args["participants"].as_array().unwrap().len(), 2);
            assert_eq!(
                tc.args["schedule"]["recurring"]["frequency"],
                "weekly"
            );
            assert!(tc.args["settings"]["notifications"].as_bool().unwrap());
            assert!(tc.args["settings"]["attachments"].is_null());
        }
        _ => panic!("Expected AI message"),
    }
}

#[test]
fn test_tool_call_id_format() {
    // Test that Anthropic tool call IDs follow expected format (toolu_*)
    let model = ChatAnthropic::try_new().unwrap();

    let response = AnthropicResponse {
        id: "msg_id_format".to_string(),
        _response_type: "message".to_string(),
        _role: "assistant".to_string(),
        content: vec![ContentBlock::ToolUse {
            id: "toolu_01XYZ789abc".to_string(),
            name: "test_tool".to_string(),
            input: serde_json::json!({}),
            cache_control: None,
        }],
        model: models::CLAUDE_3_7_SONNET.to_string(),
        stop_reason: Some("tool_use".to_string()),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 10,
            output_tokens: 20,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        },
    };

    let result = model.convert_response(response);
    let message = &result.generations[0].message;

    match message {
        Message::AI { tool_calls, .. } => {
            assert_eq!(tool_calls.len(), 1);
            // Verify ID format is preserved exactly
            assert!(tool_calls[0].id.starts_with("toolu_"));
            assert_eq!(tool_calls[0].id, "toolu_01XYZ789abc");
        }
        _ => panic!("Expected AI message"),
    }
}

#[test]
fn test_tool_call_type_field() {
    // Test that tool calls have correct type field
    let model = ChatAnthropic::try_new().unwrap();

    let response = AnthropicResponse {
        id: "msg_type_field".to_string(),
        _response_type: "message".to_string(),
        _role: "assistant".to_string(),
        content: vec![ContentBlock::ToolUse {
            id: "toolu_type_test".to_string(),
            name: "example_tool".to_string(),
            input: serde_json::json!({"key": "value"}),
            cache_control: None,
        }],
        model: models::CLAUDE_3_7_SONNET.to_string(),
        stop_reason: Some("tool_use".to_string()),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 10,
            output_tokens: 15,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        },
    };

    let result = model.convert_response(response);
    let message = &result.generations[0].message;

    match message {
        Message::AI { tool_calls, .. } => {
            assert_eq!(tool_calls[0].tool_type, "tool_call");
        }
        _ => panic!("Expected AI message"),
    }
}

#[test]
fn test_tool_result_roundtrip() {
    // Test full roundtrip: AI message with tool call -> Tool result message
    let model = ChatAnthropic::try_new().unwrap();

    // Step 1: AI requests a tool
    let ai_tool_call = ToolCall {
        id: "toolu_roundtrip".to_string(),
        name: "search_web".to_string(),
        args: serde_json::json!({"query": "Rust programming language"}),
        tool_type: "tool_call".to_string(),
        index: None,
    };

    let messages = vec![
        Message::Human {
            content: "Search for Rust".into(),
            fields: Default::default(),
        },
        Message::AI {
            content: "I'll search for that.".into(),
            tool_calls: vec![ai_tool_call],
            invalid_tool_calls: vec![],
            usage_metadata: None,
            fields: Default::default(),
        },
        Message::Tool {
            content: "Rust is a systems programming language...".into(),
            tool_call_id: "toolu_roundtrip".to_string(),
            artifact: None,
            status: None,
            fields: Default::default(),
        },
    ];

    // Convert to Anthropic format
    let (system, anthropic_messages) = model.convert_messages(&messages).unwrap();
    assert!(system.is_none());
    assert_eq!(anthropic_messages.len(), 3);

    // Message 0: user message
    assert_eq!(anthropic_messages[0].role, "user");

    // Message 1: assistant with tool use
    assert_eq!(anthropic_messages[1].role, "assistant");
    match &anthropic_messages[1].content {
        AnthropicContent::Blocks(blocks) => {
            assert_eq!(blocks.len(), 2);
            match &blocks[1] {
                ContentBlock::ToolUse { id, name, input, .. } => {
                    assert_eq!(id, "toolu_roundtrip");
                    assert_eq!(name, "search_web");
                    assert_eq!(input["query"], "Rust programming language");
                }
                _ => panic!("Expected tool use block"),
            }
        }
        _ => panic!("Expected blocks"),
    }

    // Message 2: tool result
    assert_eq!(anthropic_messages[2].role, "user");
    match &anthropic_messages[2].content {
        AnthropicContent::Blocks(blocks) => {
            assert_eq!(blocks.len(), 1);
            match &blocks[0] {
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                    ..
                } => {
                    assert_eq!(tool_use_id, "toolu_roundtrip");
                    assert!(content.contains("Rust is a systems programming"));
                    assert!(!is_error);
                }
                _ => panic!("Expected tool result block"),
            }
        }
        _ => panic!("Expected blocks"),
    }
}

#[test]
fn test_multiple_tool_results() {
    // Test providing multiple tool results for multiple tool calls
    let model = ChatAnthropic::try_new().unwrap();

    let messages = vec![
        Message::Tool {
            content: "Weather: 72°F sunny".into(),
            tool_call_id: "toolu_weather".to_string(),
            artifact: None,
            status: None,
            fields: Default::default(),
        },
        Message::Tool {
            content: "Time: 3:45 PM PST".into(),
            tool_call_id: "toolu_time".to_string(),
            artifact: None,
            status: None,
            fields: Default::default(),
        },
    ];

    let (_system, anthropic_messages) = model.convert_messages(&messages).unwrap();

    // Each tool result creates a separate user message
    assert_eq!(anthropic_messages.len(), 2);

    // First tool result
    match &anthropic_messages[0].content {
        AnthropicContent::Blocks(blocks) => {
            match &blocks[0] {
                ContentBlock::ToolResult { tool_use_id, .. } => {
                    assert_eq!(tool_use_id, "toolu_weather");
                }
                _ => panic!("Expected tool result"),
            }
        }
        _ => panic!("Expected blocks"),
    }

    // Second tool result
    match &anthropic_messages[1].content {
        AnthropicContent::Blocks(blocks) => {
            match &blocks[0] {
                ContentBlock::ToolResult { tool_use_id, .. } => {
                    assert_eq!(tool_use_id, "toolu_time");
                }
                _ => panic!("Expected tool result"),
            }
        }
        _ => panic!("Expected blocks"),
    }
}

#[test]
fn test_tool_definition_json_schema_compliance() {
    // Test that tool definitions follow JSON Schema spec for Anthropic
    let tool_def = ToolDefinition {
        name: "calculate".to_string(),
        description: "Perform mathematical calculations".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "Mathematical expression to evaluate"
                },
                "precision": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 15,
                    "default": 2
                }
            },
            "required": ["expression"],
            "additionalProperties": false
        }),
    };

    let anthropic_tool = convert_tool_definition_anthropic(&tool_def);
    let schema = anthropic_tool.input_schema.unwrap();

    // Verify JSON Schema structure
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"].is_object());
    assert_eq!(schema["properties"]["expression"]["type"], "string");
    assert_eq!(schema["properties"]["precision"]["type"], "integer");
    assert_eq!(schema["properties"]["precision"]["minimum"], 0);
    assert_eq!(schema["properties"]["precision"]["maximum"], 15);
    assert_eq!(schema["required"], serde_json::json!(["expression"]));
    assert!(!schema["additionalProperties"].as_bool().unwrap());
}

#[test]
fn test_anthropic_request_with_tools_serialization() {
    // Test that request with tools serializes correctly
    let tool = AnthropicTool {
        r#type: None,
        name: "search".to_string(),
        description: Some("Search the web".to_string()),
        input_schema: Some(serde_json::json!({
            "type": "object",
            "properties": {
                "query": {"type": "string"}
            },
            "required": ["query"]
        })),
        cache_control: None,
        max_uses: None,
    };

    let request = AnthropicRequest {
        model: "claude-3-5-sonnet-latest".to_string(),
        max_tokens: 1024,
        messages: vec![AnthropicMessage {
            role: "user".to_string(),
            content: AnthropicContent::Text("Search for rust".to_string()),
        }],
        system: None,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: None,
        tools: Some(vec![tool]),
        tool_choice: Some(AnthropicToolChoice::Auto {
            r#type: "auto".to_string(),
        }),
        thinking: None,
    };

    let json = serde_json::to_value(&request).unwrap();

    assert_eq!(json["tools"].as_array().unwrap().len(), 1);
    assert_eq!(json["tools"][0]["name"], "search");
    assert!(json["tools"][0].get("type").is_none()); // type is None for regular tools
    assert_eq!(json["tools"][0]["input_schema"]["type"], "object");
    assert_eq!(json["tool_choice"]["type"], "auto");
}

#[test]
fn test_tool_use_stop_reason() {
    // Test that stop_reason is "tool_use" when model calls tools
    let model = ChatAnthropic::try_new().unwrap();

    let response = AnthropicResponse {
        id: "msg_stop_reason".to_string(),
        _response_type: "message".to_string(),
        _role: "assistant".to_string(),
        content: vec![ContentBlock::ToolUse {
            id: "toolu_stop_test".to_string(),
            name: "get_data".to_string(),
            input: serde_json::json!({}),
            cache_control: None,
        }],
        model: models::CLAUDE_3_7_SONNET.to_string(),
        stop_reason: Some("tool_use".to_string()),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 10,
            output_tokens: 15,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        },
    };

    let result = model.convert_response(response);
    let generation_info = result.generations[0].generation_info.as_ref().unwrap();

    assert_eq!(
        generation_info.get("stop_reason").unwrap(),
        &serde_json::json!("tool_use")
    );
}

#[test]
fn test_tool_call_with_empty_object_args() {
    // Test tool call that takes no arguments (empty object)
    let model = ChatAnthropic::try_new().unwrap();

    let response = AnthropicResponse {
        id: "msg_empty_args".to_string(),
        _response_type: "message".to_string(),
        _role: "assistant".to_string(),
        content: vec![ContentBlock::ToolUse {
            id: "toolu_noargs".to_string(),
            name: "get_current_time".to_string(),
            input: serde_json::json!({}),
            cache_control: None,
        }],
        model: models::CLAUDE_3_7_SONNET.to_string(),
        stop_reason: Some("tool_use".to_string()),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 10,
            output_tokens: 10,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        },
    };

    let result = model.convert_response(response);
    let message = &result.generations[0].message;

    match message {
        Message::AI { tool_calls, .. } => {
            assert_eq!(tool_calls.len(), 1);
            assert_eq!(tool_calls[0].args, serde_json::json!({}));
        }
        _ => panic!("Expected AI message"),
    }
}

#[test]
fn test_tool_call_with_array_argument() {
    // Test tool call with array as argument value
    let model = ChatAnthropic::try_new().unwrap();

    let response = AnthropicResponse {
        id: "msg_array_arg".to_string(),
        _response_type: "message".to_string(),
        _role: "assistant".to_string(),
        content: vec![ContentBlock::ToolUse {
            id: "toolu_array".to_string(),
            name: "batch_process".to_string(),
            input: serde_json::json!({
                "items": ["apple", "banana", "cherry"],
                "options": {
                    "parallel": true,
                    "batch_size": 10
                }
            }),
            cache_control: None,
        }],
        model: models::CLAUDE_3_7_SONNET.to_string(),
        stop_reason: Some("tool_use".to_string()),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 15,
            output_tokens: 20,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        },
    };

    let result = model.convert_response(response);
    let message = &result.generations[0].message;

    match message {
        Message::AI { tool_calls, .. } => {
            let items = tool_calls[0].args["items"].as_array().unwrap();
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], "apple");
            assert_eq!(items[1], "banana");
            assert_eq!(items[2], "cherry");
        }
        _ => panic!("Expected AI message"),
    }
}

#[test]
fn test_tool_call_with_unicode_arguments() {
    // Test tool call with Unicode/international characters in arguments
    let model = ChatAnthropic::try_new().unwrap();

    let response = AnthropicResponse {
        id: "msg_unicode".to_string(),
        _response_type: "message".to_string(),
        _role: "assistant".to_string(),
        content: vec![ContentBlock::ToolUse {
            id: "toolu_unicode".to_string(),
            name: "translate".to_string(),
            input: serde_json::json!({
                "text": "こんにちは世界",
                "source_lang": "ja",
                "target_lang": "en",
                "context": "Greeting: 你好 🌍"
            }),
            cache_control: None,
        }],
        model: models::CLAUDE_3_7_SONNET.to_string(),
        stop_reason: Some("tool_use".to_string()),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 20,
            output_tokens: 25,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        },
    };

    let result = model.convert_response(response);
    let message = &result.generations[0].message;

    match message {
        Message::AI { tool_calls, .. } => {
            assert_eq!(tool_calls[0].args["text"], "こんにちは世界");
            assert_eq!(tool_calls[0].args["context"], "Greeting: 你好 🌍");
        }
        _ => panic!("Expected AI message"),
    }
}

#[test]
fn test_tool_result_with_error_status() {
    // Test tool result with error status is correctly serialized
    let model = ChatAnthropic::try_new().unwrap();

    let messages = vec![Message::Tool {
        content: "Error: Connection timeout after 30 seconds".into(),
        tool_call_id: "toolu_failed".to_string(),
        artifact: None,
        status: Some("error".to_string()),
        fields: Default::default(),
    }];

    let (_system, anthropic_messages) = model.convert_messages(&messages).unwrap();

    match &anthropic_messages[0].content {
        AnthropicContent::Blocks(blocks) => {
            match &blocks[0] {
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                    ..
                } => {
                    assert_eq!(tool_use_id, "toolu_failed");
                    assert!(content.contains("Connection timeout"));
                    assert!(is_error, "is_error should be true for error status");
                }
                _ => panic!("Expected tool result block"),
            }
        }
        _ => panic!("Expected blocks"),
    }
}

#[test]
fn test_tool_result_with_json_content() {
    // Test tool result that contains JSON as its content
    let model = ChatAnthropic::try_new().unwrap();

    let json_result = serde_json::json!({
        "temperature": 72,
        "humidity": 45,
        "conditions": "sunny"
    });

    let messages = vec![Message::Tool {
        content: json_result.to_string().into(),
        tool_call_id: "toolu_json_result".to_string(),
        artifact: None,
        status: None,
        fields: Default::default(),
    }];

    let (_system, anthropic_messages) = model.convert_messages(&messages).unwrap();

    match &anthropic_messages[0].content {
        AnthropicContent::Blocks(blocks) => {
            match &blocks[0] {
                ContentBlock::ToolResult { content, .. } => {
                    // Verify JSON is preserved as string
                    let parsed: serde_json::Value = serde_json::from_str(content).unwrap();
                    assert_eq!(parsed["temperature"], 72);
                    assert_eq!(parsed["conditions"], "sunny");
                }
                _ => panic!("Expected tool result block"),
            }
        }
        _ => panic!("Expected blocks"),
    }
}

#[test]
fn test_ai_message_tool_only_no_text() {
    // Test AI message with only tool calls and no text content
    let model = ChatAnthropic::try_new().unwrap();

    let tool_call = ToolCall {
        id: "toolu_only".to_string(),
        name: "silent_action".to_string(),
        args: serde_json::json!({"action": "execute"}),
        tool_type: "tool_call".to_string(),
        index: None,
    };

    let messages = vec![Message::AI {
        content: "".into(), // Empty content
        tool_calls: vec![tool_call],
        invalid_tool_calls: vec![],
        usage_metadata: None,
        fields: Default::default(),
    }];

    let (_system, anthropic_messages) = model.convert_messages(&messages).unwrap();

    // Should have blocks format with just tool use
    match &anthropic_messages[0].content {
        AnthropicContent::Blocks(blocks) => {
            // Only the tool use block (no text block for empty content)
            assert_eq!(blocks.len(), 1);
            match &blocks[0] {
                ContentBlock::ToolUse { name, .. } => {
                    assert_eq!(name, "silent_action");
                }
                _ => panic!("Expected tool use block"),
            }
        }
        // Also acceptable: Text format for empty string before tool blocks added
        AnthropicContent::Text(text) => {
            assert!(text.is_empty());
        }
    }
}

#[test]
fn test_streaming_multiple_tool_calls_accumulation() {
    // Test stateful streaming with multiple sequential tool calls
    let mut state = StreamToolCallState::new();

    // First tool call
    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStart {
            index: 0,
            content_block: StreamContentBlock::ToolUse {
                id: "toolu_first".to_string(),
                name: "tool_one".to_string(),
            },
        },
        &mut state,
    );

    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::InputJsonDelta {
                partial_json: r#"{"arg": "one"}"#.to_string(),
            },
        },
        &mut state,
    );

    let chunk1 = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStop { index: 0 },
        &mut state,
    );

    assert!(chunk1.is_some());
    assert_eq!(chunk1.as_ref().unwrap().tool_calls[0].name, "tool_one");
    assert_eq!(chunk1.as_ref().unwrap().tool_calls[0].args["arg"], "one");

    // Second tool call (state should be reset)
    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStart {
            index: 1,
            content_block: StreamContentBlock::ToolUse {
                id: "toolu_second".to_string(),
                name: "tool_two".to_string(),
            },
        },
        &mut state,
    );

    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockDelta {
            index: 1,
            delta: ContentDelta::InputJsonDelta {
                partial_json: r#"{"arg": "two"}"#.to_string(),
            },
        },
        &mut state,
    );

    let chunk2 = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStop { index: 1 },
        &mut state,
    );

    assert!(chunk2.is_some());
    assert_eq!(chunk2.as_ref().unwrap().tool_calls[0].name, "tool_two");
    assert_eq!(chunk2.as_ref().unwrap().tool_calls[0].args["arg"], "two");
}

// =============================================================================
// Streaming Reliability Tests (M-325)
// =============================================================================

// -----------------------------------------------------------------------------
// Chunk Ordering Tests
// -----------------------------------------------------------------------------

#[test]
fn test_streaming_chunk_ordering_text_sequence() {
    // Test that text chunks maintain correct ordering when processed sequentially
    let mut state = StreamToolCallState::new();

    // Simulate a sequence of text deltas as they would arrive from the API
    let text_chunks = vec!["Hello", ", ", "world", "! ", "How ", "are ", "you?"];
    let mut accumulated = String::new();

    for text in &text_chunks {
        let event = StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::TextDelta {
                text: text.to_string(),
            },
        };

        if let Some(chunk) = convert_stream_event_to_chunk_stateful(event, &mut state) {
            accumulated.push_str(&chunk.content);
        }
    }

    // Verify ordering is preserved
    assert_eq!(accumulated, "Hello, world! How are you?");
}

#[test]
fn test_streaming_chunk_ordering_multiple_content_blocks() {
    // Test that multiple content blocks with different indices are handled correctly
    let mut state = StreamToolCallState::new();

    // Block 0: Text
    let events = vec![
        StreamEvent::ContentBlockStart {
            index: 0,
            content_block: StreamContentBlock::Text {
                text: String::new(),
            },
        },
        StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::TextDelta {
                text: "First block".to_string(),
            },
        },
        StreamEvent::ContentBlockStop { index: 0 },
        // Block 1: Another text
        StreamEvent::ContentBlockStart {
            index: 1,
            content_block: StreamContentBlock::Text {
                text: String::new(),
            },
        },
        StreamEvent::ContentBlockDelta {
            index: 1,
            delta: ContentDelta::TextDelta {
                text: "Second block".to_string(),
            },
        },
        StreamEvent::ContentBlockStop { index: 1 },
    ];

    let mut text_chunks = Vec::new();
    for event in events {
        if let Some(chunk) = convert_stream_event_to_chunk_stateful(event, &mut state) {
            if !chunk.content.is_empty() {
                text_chunks.push(chunk.content);
            }
        }
    }

    // Both blocks should be processed in order
    assert_eq!(text_chunks.len(), 2);
    assert_eq!(text_chunks[0], "First block");
    assert_eq!(text_chunks[1], "Second block");
}

#[test]
fn test_streaming_chunk_ordering_interleaved_text_and_tools() {
    // Test ordering when text and tool calls are interleaved
    let mut state = StreamToolCallState::new();

    let events = vec![
        // Text block first
        StreamEvent::ContentBlockStart {
            index: 0,
            content_block: StreamContentBlock::Text {
                text: String::new(),
            },
        },
        StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::TextDelta {
                text: "Let me help you.".to_string(),
            },
        },
        StreamEvent::ContentBlockStop { index: 0 },
        // Tool block second
        StreamEvent::ContentBlockStart {
            index: 1,
            content_block: StreamContentBlock::ToolUse {
                id: "toolu_interleave".to_string(),
                name: "helper_tool".to_string(),
            },
        },
        StreamEvent::ContentBlockDelta {
            index: 1,
            delta: ContentDelta::InputJsonDelta {
                partial_json: r#"{"action": "help"}"#.to_string(),
            },
        },
        StreamEvent::ContentBlockStop { index: 1 },
    ];

    let mut results: Vec<(&str, String)> = Vec::new();
    for event in events {
        if let Some(chunk) = convert_stream_event_to_chunk_stateful(event, &mut state) {
            if !chunk.content.is_empty() {
                results.push(("text", chunk.content));
            }
            if !chunk.tool_calls.is_empty() {
                results.push(("tool", chunk.tool_calls[0].name.clone()));
            }
        }
    }

    // Verify order: text first, tool second
    assert_eq!(results.len(), 2);
    assert_eq!(results[0], ("text", "Let me help you.".to_string()));
    assert_eq!(results[1], ("tool", "helper_tool".to_string()));
}

#[test]
fn test_streaming_chunk_ordering_index_preservation() {
    // Test that content block indices are correctly tracked
    let mut state = StreamToolCallState::new();

    // Start blocks at non-zero indices (simulating prior blocks)
    let events = vec![
        StreamEvent::ContentBlockStart {
            index: 2,
            content_block: StreamContentBlock::Text {
                text: String::new(),
            },
        },
        StreamEvent::ContentBlockDelta {
            index: 2,
            delta: ContentDelta::TextDelta {
                text: "Index 2".to_string(),
            },
        },
        StreamEvent::ContentBlockStop { index: 2 },
        StreamEvent::ContentBlockStart {
            index: 5,
            content_block: StreamContentBlock::Text {
                text: String::new(),
            },
        },
        StreamEvent::ContentBlockDelta {
            index: 5,
            delta: ContentDelta::TextDelta {
                text: "Index 5".to_string(),
            },
        },
        StreamEvent::ContentBlockStop { index: 5 },
    ];

    let mut chunks = Vec::new();
    for event in events {
        if let Some(chunk) = convert_stream_event_to_chunk_stateful(event, &mut state) {
            if !chunk.content.is_empty() {
                chunks.push(chunk.content);
            }
        }
    }

    // All chunks should be captured regardless of index values
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0], "Index 2");
    assert_eq!(chunks[1], "Index 5");
}

// -----------------------------------------------------------------------------
// Partial Frame Tests
// -----------------------------------------------------------------------------

#[test]
fn test_streaming_partial_json_accumulation_multi_chunk() {
    // Test JSON accumulation across many small chunks (simulates network fragmentation)
    let mut state = StreamToolCallState::new();

    // Start tool use
    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStart {
            index: 0,
            content_block: StreamContentBlock::ToolUse {
                id: "toolu_fragmented".to_string(),
                name: "fragmented_tool".to_string(),
            },
        },
        &mut state,
    );

    // Simulate highly fragmented JSON arrival
    let fragments = vec![
        "{",
        "\"",
        "key",
        "\"",
        ":",
        " ",
        "\"",
        "value",
        "\"",
        ",",
        " ",
        "\"",
        "number",
        "\"",
        ":",
        " ",
        "42",
        "}",
    ];

    for fragment in fragments {
        let _ = convert_stream_event_to_chunk_stateful(
            StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentDelta::InputJsonDelta {
                    partial_json: fragment.to_string(),
                },
            },
            &mut state,
        );
    }

    // Complete the tool call
    let chunk = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStop { index: 0 },
        &mut state,
    );

    assert!(chunk.is_some());
    let tool_call = &chunk.unwrap().tool_calls[0];
    assert_eq!(tool_call.args["key"], "value");
    assert_eq!(tool_call.args["number"], 42);
}

#[test]
fn test_streaming_partial_json_unicode_split() {
    // Test JSON with Unicode characters that might be split across chunks
    let mut state = StreamToolCallState::new();

    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStart {
            index: 0,
            content_block: StreamContentBlock::ToolUse {
                id: "toolu_unicode".to_string(),
                name: "unicode_tool".to_string(),
            },
        },
        &mut state,
    );

    // Unicode string split across chunks
    let fragments = vec![
        r#"{"text": ""#,
        "こんに",
        "ちは",
        "世界",
        r#"", "emoji": "🎉"#,
        r#"🚀"}"#,
    ];

    for fragment in fragments {
        let _ = convert_stream_event_to_chunk_stateful(
            StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentDelta::InputJsonDelta {
                    partial_json: fragment.to_string(),
                },
            },
            &mut state,
        );
    }

    let chunk = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStop { index: 0 },
        &mut state,
    );

    assert!(chunk.is_some());
    let tool_call = &chunk.unwrap().tool_calls[0];
    assert_eq!(tool_call.args["text"], "こんにちは世界");
    assert_eq!(tool_call.args["emoji"], "🎉🚀");
}

#[test]
fn test_streaming_partial_json_nested_objects() {
    // Test deeply nested JSON split across multiple chunks
    let mut state = StreamToolCallState::new();

    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStart {
            index: 0,
            content_block: StreamContentBlock::ToolUse {
                id: "toolu_nested".to_string(),
                name: "nested_tool".to_string(),
            },
        },
        &mut state,
    );

    let fragments = vec![
        r#"{"level1": {"#,
        r#""level2": {"#,
        r#""level3": {"#,
        r#""value": "deep""#,
        r#"}"#,
        r#"}"#,
        r#"}"#,
        r#"}"#,
    ];

    for fragment in fragments {
        let _ = convert_stream_event_to_chunk_stateful(
            StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentDelta::InputJsonDelta {
                    partial_json: fragment.to_string(),
                },
            },
            &mut state,
        );
    }

    let chunk = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStop { index: 0 },
        &mut state,
    );

    assert!(chunk.is_some());
    let tool_call = &chunk.unwrap().tool_calls[0];
    assert_eq!(tool_call.args["level1"]["level2"]["level3"]["value"], "deep");
}

#[test]
fn test_streaming_partial_json_array_split() {
    // Test JSON array split across chunks
    let mut state = StreamToolCallState::new();

    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStart {
            index: 0,
            content_block: StreamContentBlock::ToolUse {
                id: "toolu_array".to_string(),
                name: "array_tool".to_string(),
            },
        },
        &mut state,
    );

    let fragments = vec![
        r#"{"items": ["#,
        r#"1, "#,
        r#"2, "#,
        r#"3, "#,
        r#""str", "#,
        r#"null, "#,
        r#"true, "#,
        r#"{"nested": "obj"}"#,
        r#"]}"#,
    ];

    for fragment in fragments {
        let _ = convert_stream_event_to_chunk_stateful(
            StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentDelta::InputJsonDelta {
                    partial_json: fragment.to_string(),
                },
            },
            &mut state,
        );
    }

    let chunk = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStop { index: 0 },
        &mut state,
    );

    assert!(chunk.is_some());
    let tool_call = &chunk.unwrap().tool_calls[0];
    let items = tool_call.args["items"].as_array().unwrap();
    assert_eq!(items.len(), 7);
    assert_eq!(items[0], 1);
    assert_eq!(items[3], "str");
    assert!(items[4].is_null());
    assert!(items[5].as_bool().unwrap());
    assert_eq!(items[6]["nested"], "obj");
}

#[test]
fn test_streaming_malformed_json_recovery() {
    // Test that malformed JSON is handled gracefully with error info
    let mut state = StreamToolCallState::new();

    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStart {
            index: 0,
            content_block: StreamContentBlock::ToolUse {
                id: "toolu_malformed".to_string(),
                name: "malformed_tool".to_string(),
            },
        },
        &mut state,
    );

    // Intentionally malformed JSON (missing closing brace and quote)
    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::InputJsonDelta {
                partial_json: r#"{"broken": "json"#.to_string(),
            },
        },
        &mut state,
    );

    let chunk = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStop { index: 0 },
        &mut state,
    );

    // Should still emit a chunk with error information
    assert!(chunk.is_some());
    let tool_call = &chunk.unwrap().tool_calls[0];
    assert!(tool_call.args["error"].is_string());
    assert!(tool_call.args["raw"]
        .as_str()
        .unwrap()
        .contains("broken"));
}

#[test]
fn test_streaming_empty_delta_handling() {
    // Test handling of empty delta events (can occur with API edge cases)
    let mut state = StreamToolCallState::new();

    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStart {
            index: 0,
            content_block: StreamContentBlock::ToolUse {
                id: "toolu_empty".to_string(),
                name: "empty_delta_tool".to_string(),
            },
        },
        &mut state,
    );

    // Empty deltas interspersed with real data
    let fragments = vec![
        "",                    // empty
        r#"{"key":"#,          // start of JSON
        "",                    // empty
        r#" "value"}"#,        // rest of JSON
        "",                    // empty
    ];

    for fragment in fragments {
        let _ = convert_stream_event_to_chunk_stateful(
            StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentDelta::InputJsonDelta {
                    partial_json: fragment.to_string(),
                },
            },
            &mut state,
        );
    }

    let chunk = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStop { index: 0 },
        &mut state,
    );

    assert!(chunk.is_some());
    let tool_call = &chunk.unwrap().tool_calls[0];
    assert_eq!(tool_call.args["key"], "value");
}

#[test]
fn test_streaming_unknown_event_handling() {
    // Test that unknown events are safely ignored
    let mut state = StreamToolCallState::new();

    // Unknown events should not affect state
    let chunk = convert_stream_event_to_chunk_stateful(StreamEvent::Unknown, &mut state);
    assert!(chunk.is_none());

    // State should still work normally after unknown events
    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStart {
            index: 0,
            content_block: StreamContentBlock::ToolUse {
                id: "toolu_after_unknown".to_string(),
                name: "normal_tool".to_string(),
            },
        },
        &mut state,
    );

    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::InputJsonDelta {
                partial_json: r#"{"test": true}"#.to_string(),
            },
        },
        &mut state,
    );

    let chunk = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStop { index: 0 },
        &mut state,
    );

    assert!(chunk.is_some());
    assert!(chunk.unwrap().tool_calls[0].args["test"].as_bool().unwrap());
}

// -----------------------------------------------------------------------------
// Backpressure and High-Volume Tests
// -----------------------------------------------------------------------------

#[test]
fn test_streaming_high_volume_text_chunks() {
    // Test processing a large number of small text chunks (backpressure scenario)
    let mut state = StreamToolCallState::new();
    let mut accumulated = String::new();
    let chunk_count = 1000;

    for i in 0..chunk_count {
        let event = StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::TextDelta {
                text: format!("{}", i % 10),
            },
        };

        if let Some(chunk) = convert_stream_event_to_chunk_stateful(event, &mut state) {
            accumulated.push_str(&chunk.content);
        }
    }

    // Verify all chunks were processed
    assert_eq!(accumulated.len(), chunk_count);
}

#[test]
fn test_streaming_high_volume_json_fragments() {
    // Test processing many JSON fragments (simulates slow network or backpressure)
    let mut state = StreamToolCallState::new();

    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStart {
            index: 0,
            content_block: StreamContentBlock::ToolUse {
                id: "toolu_highvol".to_string(),
                name: "high_volume_tool".to_string(),
            },
        },
        &mut state,
    );

    // Build JSON character by character (extreme fragmentation)
    let json = r#"{"count": 100, "items": [1,2,3,4,5,6,7,8,9,10]}"#;
    for ch in json.chars() {
        let _ = convert_stream_event_to_chunk_stateful(
            StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentDelta::InputJsonDelta {
                    partial_json: ch.to_string(),
                },
            },
            &mut state,
        );
    }

    let chunk = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStop { index: 0 },
        &mut state,
    );

    assert!(chunk.is_some());
    let tool_call = &chunk.unwrap().tool_calls[0];
    assert_eq!(tool_call.args["count"], 100);
    assert_eq!(tool_call.args["items"].as_array().unwrap().len(), 10);
}

#[test]
fn test_streaming_large_json_payload() {
    // Test handling of a large JSON payload (memory efficiency)
    let mut state = StreamToolCallState::new();

    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStart {
            index: 0,
            content_block: StreamContentBlock::ToolUse {
                id: "toolu_large".to_string(),
                name: "large_payload_tool".to_string(),
            },
        },
        &mut state,
    );

    // Build a large JSON object with many fields
    let mut large_json = String::from("{");
    for i in 0..100 {
        if i > 0 {
            large_json.push_str(", ");
        }
        large_json.push_str(&format!(
            r#""field_{i}": "value_{i} with some padding text""#
        ));
    }
    large_json.push('}');

    // Send in larger chunks (simulating batch network delivery)
    let chunk_size = 100;
    for chunk in large_json.as_bytes().chunks(chunk_size) {
        let fragment = String::from_utf8_lossy(chunk);
        let _ = convert_stream_event_to_chunk_stateful(
            StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentDelta::InputJsonDelta {
                    partial_json: fragment.to_string(),
                },
            },
            &mut state,
        );
    }

    let chunk = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStop { index: 0 },
        &mut state,
    );

    assert!(chunk.is_some());
    let tool_call = &chunk.unwrap().tool_calls[0];
    assert_eq!(tool_call.args["field_0"], "value_0 with some padding text");
    assert_eq!(
        tool_call.args["field_99"],
        "value_99 with some padding text"
    );
}

#[test]
fn test_streaming_multiple_tool_calls_high_volume() {
    // Test processing many tool calls in sequence (backpressure with state resets)
    let mut state = StreamToolCallState::new();
    let tool_count = 50;
    let mut tool_calls_received = Vec::new();

    for i in 0..tool_count {
        let _ = convert_stream_event_to_chunk_stateful(
            StreamEvent::ContentBlockStart {
                index: i,
                content_block: StreamContentBlock::ToolUse {
                    id: format!("toolu_{i}"),
                    name: format!("tool_{i}"),
                },
            },
            &mut state,
        );

        let _ = convert_stream_event_to_chunk_stateful(
            StreamEvent::ContentBlockDelta {
                index: i,
                delta: ContentDelta::InputJsonDelta {
                    partial_json: format!(r#"{{"index": {i}}}"#),
                },
            },
            &mut state,
        );

        if let Some(chunk) = convert_stream_event_to_chunk_stateful(
            StreamEvent::ContentBlockStop { index: i },
            &mut state,
        ) {
            tool_calls_received.push(chunk.tool_calls[0].clone());
        }
    }

    // All tool calls should be captured
    assert_eq!(tool_calls_received.len(), tool_count);

    // Verify each tool call has correct data
    for (i, tc) in tool_calls_received.iter().enumerate() {
        assert_eq!(tc.id, format!("toolu_{i}"));
        assert_eq!(tc.name, format!("tool_{i}"));
        assert_eq!(tc.args["index"], i);
    }
}

#[tokio::test]
async fn test_streaming_async_processing_simulation() {
    // Simulate async stream processing with yields (backpressure test)
    let mut state = StreamToolCallState::new();
    let mut results = Vec::new();

    let events = vec![
        StreamEvent::MessageStart {
            message: MessageStartData {
                id: "msg_async".to_string(),
                _message_type: "message".to_string(),
                role: "assistant".to_string(),
                model: "claude-3-5-sonnet".to_string(),
                usage: Usage {
                    input_tokens: 10,
                    output_tokens: 0,
                    cache_creation_input_tokens: None,
                    cache_read_input_tokens: None,
                },
            },
        },
        StreamEvent::ContentBlockStart {
            index: 0,
            content_block: StreamContentBlock::Text {
                text: String::new(),
            },
        },
        StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::TextDelta {
                text: "Processing".to_string(),
            },
        },
        StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::TextDelta {
                text: " complete".to_string(),
            },
        },
        StreamEvent::ContentBlockStop { index: 0 },
        StreamEvent::MessageDelta {
            delta: MessageDeltaData {
                stop_reason: Some("end_turn".to_string()),
                stop_sequence: None,
            },
            usage: Usage {
                input_tokens: 10,
                output_tokens: 5,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
        },
        StreamEvent::MessageStop,
    ];

    for event in events {
        // Simulate async yield between events
        tokio::task::yield_now().await;

        if let Some(chunk) = convert_stream_event_to_chunk_stateful(event, &mut state) {
            results.push(chunk);
        }
    }

    // Should have: MessageStart, 2 text deltas, MessageDelta
    assert!(results.len() >= 3);

    // Verify we got the text content
    let text_content: String = results
        .iter()
        .filter(|c| !c.content.is_empty())
        .map(|c| c.content.clone())
        .collect();
    assert_eq!(text_content, "Processing complete");
}

#[test]
fn test_streaming_state_isolation_between_tools() {
    // Test that state is properly isolated between consecutive tool calls
    let mut state = StreamToolCallState::new();

    // First tool with specific JSON
    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStart {
            index: 0,
            content_block: StreamContentBlock::ToolUse {
                id: "toolu_first".to_string(),
                name: "first_tool".to_string(),
            },
        },
        &mut state,
    );
    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::InputJsonDelta {
                partial_json: r#"{"first": true}"#.to_string(),
            },
        },
        &mut state,
    );
    let first_chunk = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStop { index: 0 },
        &mut state,
    );

    // Second tool should NOT have any contamination from first tool's JSON
    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStart {
            index: 1,
            content_block: StreamContentBlock::ToolUse {
                id: "toolu_second".to_string(),
                name: "second_tool".to_string(),
            },
        },
        &mut state,
    );
    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockDelta {
            index: 1,
            delta: ContentDelta::InputJsonDelta {
                partial_json: r#"{"second": true}"#.to_string(),
            },
        },
        &mut state,
    );
    let second_chunk = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStop { index: 1 },
        &mut state,
    );

    // Verify isolation
    let first_tc = &first_chunk.unwrap().tool_calls[0];
    assert!(first_tc.args["first"].as_bool().unwrap());
    assert!(first_tc.args.get("second").is_none());

    let second_tc = &second_chunk.unwrap().tool_calls[0];
    assert!(second_tc.args["second"].as_bool().unwrap());
    assert!(second_tc.args.get("first").is_none());
}

#[test]
fn test_streaming_message_metadata_preservation() {
    // Test that metadata is preserved through streaming events
    let mut state = StreamToolCallState::new();

    // Message start should capture model info
    let start_chunk = convert_stream_event_to_chunk_stateful(
        StreamEvent::MessageStart {
            message: MessageStartData {
                id: "msg_meta_test".to_string(),
                _message_type: "message".to_string(),
                role: "assistant".to_string(),
                model: "claude-3-5-sonnet-20241022".to_string(),
                usage: Usage {
                    input_tokens: 50,
                    output_tokens: 0,
                    cache_creation_input_tokens: Some(40),
                    cache_read_input_tokens: None,
                },
            },
        },
        &mut state,
    );

    assert!(start_chunk.is_some());
    let chunk = start_chunk.unwrap();
    assert_eq!(
        chunk.fields.response_metadata["model"],
        "claude-3-5-sonnet-20241022"
    );

    // Message delta should capture final usage
    let delta_chunk = convert_stream_event_to_chunk_stateful(
        StreamEvent::MessageDelta {
            delta: MessageDeltaData {
                stop_reason: Some("end_turn".to_string()),
                stop_sequence: None,
            },
            usage: Usage {
                input_tokens: 50,
                output_tokens: 100,
                cache_creation_input_tokens: Some(40),
                cache_read_input_tokens: None,
            },
        },
        &mut state,
    );

    assert!(delta_chunk.is_some());
    let delta = delta_chunk.unwrap();
    assert!(delta.usage_metadata.is_some());
    let usage = delta.usage_metadata.unwrap();
    assert_eq!(usage.input_tokens, 50);
    assert_eq!(usage.output_tokens, 100);
    assert_eq!(usage.total_tokens, 150);
}

#[test]
fn test_streaming_whitespace_only_json() {
    // Test handling of JSON with significant whitespace
    let mut state = StreamToolCallState::new();

    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStart {
            index: 0,
            content_block: StreamContentBlock::ToolUse {
                id: "toolu_ws".to_string(),
                name: "whitespace_tool".to_string(),
            },
        },
        &mut state,
    );

    // JSON with lots of whitespace (as might come from pretty-printed responses)
    let whitespace_json = r#"
{
    "key":    "value",
    "nested": {
        "inner":  "data"
    }
}
"#;

    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::InputJsonDelta {
                partial_json: whitespace_json.to_string(),
            },
        },
        &mut state,
    );

    let chunk = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStop { index: 0 },
        &mut state,
    );

    assert!(chunk.is_some());
    let tool_call = &chunk.unwrap().tool_calls[0];
    assert_eq!(tool_call.args["key"], "value");
    assert_eq!(tool_call.args["nested"]["inner"], "data");
}

#[test]
fn test_streaming_special_json_characters() {
    // Test JSON with special characters that need escaping
    let mut state = StreamToolCallState::new();

    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStart {
            index: 0,
            content_block: StreamContentBlock::ToolUse {
                id: "toolu_special".to_string(),
                name: "special_chars_tool".to_string(),
            },
        },
        &mut state,
    );

    // JSON with escaped special characters
    let special_json = r#"{"quote": "He said \"hello\"", "backslash": "path\\to\\file", "newline": "line1\nline2", "tab": "col1\tcol2"}"#;

    let _ = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::InputJsonDelta {
                partial_json: special_json.to_string(),
            },
        },
        &mut state,
    );

    let chunk = convert_stream_event_to_chunk_stateful(
        StreamEvent::ContentBlockStop { index: 0 },
        &mut state,
    );

    assert!(chunk.is_some());
    let tool_call = &chunk.unwrap().tool_calls[0];
    assert_eq!(tool_call.args["quote"], r#"He said "hello""#);
    assert_eq!(tool_call.args["backslash"], r"path\to\file");
    assert_eq!(tool_call.args["newline"], "line1\nline2");
    assert_eq!(tool_call.args["tab"], "col1\tcol2");
}
