// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Tests for OpenAI chat model implementation.

#![allow(clippy::unwrap_used, clippy::panic)]

#[allow(deprecated)] // Tests use deprecated with_tools() method
use super::*;

#[test]
fn test_chat_openai_builder() {
    let model = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-4")
        .with_temperature(0.7)
        .with_max_tokens(1000)
        .with_top_p(0.9);

    assert_eq!(model.model, "gpt-4");
    assert_eq!(model.temperature, Some(0.7));
    assert_eq!(model.max_tokens, Some(1000));
    assert_eq!(model.top_p, Some(0.9));
}

#[test]
fn test_message_conversion_system() {
    let msg = Message::system("You are a helpful assistant");
    let converted = convert_message(&msg).unwrap();

    match converted {
        ChatCompletionRequestMessage::System(_) => {}
        _ => panic!("Expected system message"),
    }
}

#[test]
fn test_message_conversion_human() {
    let msg = Message::human("Hello");
    let converted = convert_message(&msg).unwrap();

    match converted {
        ChatCompletionRequestMessage::User(_) => {}
        _ => panic!("Expected user message"),
    }
}

#[test]
fn test_message_conversion_ai() {
    let msg = Message::ai("Hi there!");
    let converted = convert_message(&msg).unwrap();

    match converted {
        ChatCompletionRequestMessage::Assistant(_) => {}
        _ => panic!("Expected assistant message"),
    }
}

#[test]
fn test_message_conversion_ai_with_tool_calls() {
    let tool_call = ToolCall {
        id: "call_123".to_string(),
        name: "get_weather".to_string(),
        args: serde_json::json!({"location": "San Francisco"}),
        tool_type: "tool_call".to_string(),
        index: None,
    };

    let msg = Message::AI {
        content: "Let me check the weather.".into(),
        tool_calls: vec![tool_call],
        invalid_tool_calls: vec![],
        usage_metadata: None,
        fields: Default::default(),
    };

    let converted = convert_message(&msg).unwrap();

    match converted {
        ChatCompletionRequestMessage::Assistant(assistant_msg) => {
            assert!(assistant_msg.tool_calls.is_some());
            let tool_calls = assistant_msg.tool_calls.unwrap();
            assert_eq!(tool_calls.len(), 1);
            assert_eq!(tool_calls[0].id, "call_123");
            assert_eq!(tool_calls[0].function.name, "get_weather");
        }
        _ => panic!("Expected assistant message"),
    }
}

#[test]
fn test_message_conversion_tool() {
    let msg = Message::Tool {
        content: "The weather is sunny, 72°F".into(),
        tool_call_id: "call_123".to_string(),
        artifact: None,
        status: None,
        fields: Default::default(),
    };

    let converted = convert_message(&msg).unwrap();

    match converted {
        ChatCompletionRequestMessage::Tool(tool_msg) => {
            assert_eq!(tool_msg.tool_call_id, "call_123");
        }
        _ => panic!("Expected tool message"),
    }
}

#[test]
#[allow(deprecated)]
fn test_with_tools_builder() {
    let tool = serde_json::json!({
        "name": "get_weather",
        "description": "Get current weather for a location",
        "parameters": {
            "type": "object",
            "properties": {
                "location": {"type": "string"}
            },
            "required": ["location"]
        }
    });

    let model = ChatOpenAI::with_config(Default::default()).with_tools(vec![tool]);

    assert!(model.tools.is_some());
    let tools = model.tools.unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].function.name, "get_weather");
    assert_eq!(
        tools[0].function.description,
        Some("Get current weather for a location".to_string())
    );
    assert!(tools[0].function.parameters.is_some());
}

#[test]
#[allow(deprecated)]
fn test_with_tools_multiple() {
    let tools = vec![
        serde_json::json!({"name": "get_weather", "description": "Get weather"}),
        serde_json::json!({"name": "search_web", "description": "Search the web"}),
    ];

    let model = ChatOpenAI::with_config(Default::default()).with_tools(tools);

    assert!(model.tools.is_some());
    let bound_tools = model.tools.unwrap();
    assert_eq!(bound_tools.len(), 2);
    assert_eq!(bound_tools[0].function.name, "get_weather");
    assert_eq!(bound_tools[1].function.name, "search_web");
}

#[test]
#[allow(deprecated)]
fn test_with_tools_empty() {
    let model = ChatOpenAI::with_config(Default::default()).with_tools(vec![]);
    assert!(model.tools.is_none());
}

#[test]
#[allow(deprecated)]
fn test_with_tools_invalid_schema() {
    // Tool without name should be filtered out
    let tools = vec![
        serde_json::json!({"description": "No name"}),
        serde_json::json!({"name": "valid_tool"}),
    ];

    let model = ChatOpenAI::with_config(Default::default()).with_tools(tools);

    assert!(model.tools.is_some());
    let bound_tools = model.tools.unwrap();
    assert_eq!(bound_tools.len(), 1);
    assert_eq!(bound_tools[0].function.name, "valid_tool");
}

#[test]
fn test_with_tool_choice_none() {
    let model =
        ChatOpenAI::with_config(Default::default()).with_tool_choice(Some("none".to_string()));

    assert!(model.tool_choice.is_some());
    match model.tool_choice.unwrap() {
        ChatCompletionToolChoiceOption::None => {}
        _ => panic!("Expected None tool choice"),
    }
}

#[test]
fn test_with_tool_choice_auto() {
    let model =
        ChatOpenAI::with_config(Default::default()).with_tool_choice(Some("auto".to_string()));

    assert!(model.tool_choice.is_some());
    match model.tool_choice.unwrap() {
        ChatCompletionToolChoiceOption::Auto => {}
        _ => panic!("Expected Auto tool choice"),
    }
}

#[test]
fn test_with_tool_choice_required() {
    let model = ChatOpenAI::with_config(Default::default())
        .with_tool_choice(Some("required".to_string()));

    assert!(model.tool_choice.is_some());
    match model.tool_choice.unwrap() {
        ChatCompletionToolChoiceOption::Required => {}
        _ => panic!("Expected Required tool choice"),
    }
}

#[test]
fn test_with_tool_choice_named() {
    let model = ChatOpenAI::with_config(Default::default())
        .with_tool_choice(Some("get_weather".to_string()));

    assert!(model.tool_choice.is_some());
    match model.tool_choice.unwrap() {
        ChatCompletionToolChoiceOption::Named(choice) => {
            assert_eq!(choice.function.name, "get_weather");
        }
        _ => panic!("Expected Named tool choice"),
    }
}

#[test]
fn test_with_tool_choice_unset() {
    let model = ChatOpenAI::with_config(Default::default()).with_tool_choice(None);
    assert!(model.tool_choice.is_none());
}

#[test]
#[allow(deprecated)]
fn test_tools_and_tool_choice_together() {
    let tool = serde_json::json!({
        "name": "calculate",
        "description": "Perform calculation"
    });

    let model = ChatOpenAI::with_config(Default::default())
        .with_tools(vec![tool])
        .with_tool_choice(Some("required".to_string()));

    assert!(model.tools.is_some());
    assert!(model.tool_choice.is_some());
}

#[test]
fn test_with_json_mode() {
    let model = ChatOpenAI::with_config(Default::default()).with_json_mode();

    assert!(model.response_format.is_some());
    match model.response_format.unwrap() {
        ResponseFormat::JsonObject => {}
        _ => panic!("Expected JsonObject response format"),
    }
}

#[test]
fn test_with_structured_output() {
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "age": {"type": "number"}
        },
        "required": ["name", "age"]
    });

    let model = ChatOpenAI::with_config(Default::default()).with_structured_output(
        "user_info",
        schema.clone(),
        Some("User information format".to_string()),
        true,
    );

    assert!(model.response_format.is_some());
    match model.response_format.unwrap() {
        ResponseFormat::JsonSchema { json_schema } => {
            assert_eq!(json_schema.name, "user_info");
            assert_eq!(
                json_schema.description,
                Some("User information format".to_string())
            );
            assert_eq!(json_schema.strict, Some(true));
            assert_eq!(json_schema.schema, Some(schema));
        }
        _ => panic!("Expected JsonSchema response format"),
    }
}

#[test]
fn test_with_structured_output_no_description() {
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "value": {"type": "string"}
        }
    });

    let model = ChatOpenAI::with_config(Default::default()).with_structured_output(
        "simple_output",
        schema.clone(),
        None,
        false,
    );

    assert!(model.response_format.is_some());
    match model.response_format.unwrap() {
        ResponseFormat::JsonSchema { json_schema } => {
            assert_eq!(json_schema.name, "simple_output");
            assert_eq!(json_schema.description, None);
            assert_eq!(json_schema.strict, Some(false));
            assert_eq!(json_schema.schema, Some(schema));
        }
        _ => panic!("Expected JsonSchema response format"),
    }
}

#[test]
#[allow(deprecated)]
fn test_json_mode_and_tools_incompatible() {
    // OpenAI API doesn't allow tools and response_format together in most cases
    // But at the builder level, we allow it (API will error if incompatible)
    let tool = serde_json::json!({"name": "test_tool"});

    let model = ChatOpenAI::with_config(Default::default())
        .with_tools(vec![tool])
        .with_json_mode();

    assert!(model.tools.is_some());
    assert!(model.response_format.is_some());
}

#[test]
fn test_response_format_unset_by_default() {
    let model = ChatOpenAI::with_config(Default::default());
    assert!(model.response_format.is_none());
}

#[test]
fn test_structured_output_with_complex_schema() {
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "users": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "email": {"type": "string", "format": "email"},
                        "roles": {
                            "type": "array",
                            "items": {"type": "string"}
                        }
                    },
                    "required": ["name", "email"]
                }
            },
            "metadata": {
                "type": "object",
                "properties": {
                    "total": {"type": "number"},
                    "page": {"type": "number"}
                }
            }
        },
        "required": ["users"],
        "additionalProperties": false
    });

    let model = ChatOpenAI::with_config(Default::default()).with_structured_output(
        "user_list_response",
        schema,
        Some("Paginated user list with metadata".to_string()),
        true,
    );

    assert!(model.response_format.is_some());
    match model.response_format.unwrap() {
        ResponseFormat::JsonSchema { json_schema } => {
            assert_eq!(json_schema.name, "user_list_response");
            assert_eq!(json_schema.strict, Some(true));
            // Verify the schema is preserved correctly
            assert!(json_schema.schema.is_some());
            let stored_schema = json_schema.schema.unwrap();
            assert_eq!(stored_schema.get("type").unwrap(), "object");
            assert!(stored_schema
                .get("properties")
                .unwrap()
                .get("users")
                .is_some());
        }
        _ => panic!("Expected JsonSchema response format"),
    }
}

#[test]
fn test_message_conversion_human_with_image_url() {
    use dashflow::core::messages::{ContentBlock, ImageSource, MessageContent};

    let msg = Message::Human {
        content: MessageContent::Blocks(vec![
            ContentBlock::Text {
                text: "What's in this image?".to_string(),
            },
            ContentBlock::Image {
                source: ImageSource::Url {
                    url: "https://example.com/image.jpg".to_string(),
                },
                detail: None,
            },
        ]),
        fields: Default::default(),
    };

    let converted = convert_message(&msg).unwrap();

    match converted {
        ChatCompletionRequestMessage::User(user_msg) => {
            match user_msg.content {
                ChatCompletionRequestUserMessageContent::Array(parts) => {
                    assert_eq!(parts.len(), 2);

                    // First part: text
                    match &parts[0] {
                        ChatCompletionRequestUserMessageContentPart::Text(text_part) => {
                            assert_eq!(text_part.text, "What's in this image?");
                        }
                        _ => panic!("Expected text part"),
                    }

                    // Second part: image
                    match &parts[1] {
                        ChatCompletionRequestUserMessageContentPart::ImageUrl(img_part) => {
                            assert_eq!(img_part.image_url.url, "https://example.com/image.jpg");
                            assert!(img_part.image_url.detail.is_none());
                        }
                        _ => panic!("Expected image part"),
                    }
                }
                _ => panic!("Expected array content"),
            }
        }
        _ => panic!("Expected user message"),
    }
}

#[test]
fn test_message_conversion_human_with_base64_image() {
    use dashflow::core::messages::{ContentBlock, ImageSource, MessageContent};

    let msg = Message::Human {
        content: MessageContent::Blocks(vec![
            ContentBlock::Text {
                text: "Analyze this image".to_string(),
            },
            ContentBlock::Image {
                source: ImageSource::Base64 {
                    media_type: "image/png".to_string(),
                    data: "iVBORw0KGgoAAAANS...".to_string(),
                },
                detail: Some(dashflow::core::messages::ImageDetail::High),
            },
        ]),
        fields: Default::default(),
    };

    let converted = convert_message(&msg).unwrap();

    match converted {
        ChatCompletionRequestMessage::User(user_msg) => {
            match user_msg.content {
                ChatCompletionRequestUserMessageContent::Array(parts) => {
                    assert_eq!(parts.len(), 2);

                    // Second part: base64 image
                    match &parts[1] {
                        ChatCompletionRequestUserMessageContentPart::ImageUrl(img_part) => {
                            assert_eq!(
                                img_part.image_url.url,
                                "data:image/png;base64,iVBORw0KGgoAAAANS..."
                            );
                            assert_eq!(
                                img_part.image_url.detail,
                                Some(OpenAIImageDetail::High)
                            );
                        }
                        _ => panic!("Expected image part"),
                    }
                }
                _ => panic!("Expected array content"),
            }
        }
        _ => panic!("Expected user message"),
    }
}

#[test]
fn test_message_conversion_human_image_only() {
    use dashflow::core::messages::{ContentBlock, ImageSource, MessageContent};

    let msg = Message::Human {
        content: MessageContent::Blocks(vec![ContentBlock::Image {
            source: ImageSource::Url {
                url: "https://example.com/photo.jpg".to_string(),
            },
            detail: Some(dashflow::core::messages::ImageDetail::Auto),
        }]),
        fields: Default::default(),
    };

    let converted = convert_message(&msg).unwrap();

    match converted {
        ChatCompletionRequestMessage::User(user_msg) => match user_msg.content {
            ChatCompletionRequestUserMessageContent::Array(parts) => {
                assert_eq!(parts.len(), 1);

                match &parts[0] {
                    ChatCompletionRequestUserMessageContentPart::ImageUrl(img_part) => {
                        assert_eq!(img_part.image_url.url, "https://example.com/photo.jpg");
                        assert_eq!(img_part.image_url.detail, Some(OpenAIImageDetail::Auto));
                    }
                    _ => panic!("Expected image part"),
                }
            }
            _ => panic!("Expected array content"),
        },
        _ => panic!("Expected user message"),
    }
}

#[test]
fn test_message_conversion_human_multiple_images() {
    use dashflow::core::messages::{ContentBlock, ImageSource, MessageContent};

    let msg = Message::Human {
        content: MessageContent::Blocks(vec![
            ContentBlock::Text {
                text: "Compare these images".to_string(),
            },
            ContentBlock::Image {
                source: ImageSource::Url {
                    url: "https://example.com/image1.jpg".to_string(),
                },
                detail: Some(dashflow::core::messages::ImageDetail::Low),
            },
            ContentBlock::Image {
                source: ImageSource::Url {
                    url: "https://example.com/image2.jpg".to_string(),
                },
                detail: Some(dashflow::core::messages::ImageDetail::Low),
            },
        ]),
        fields: Default::default(),
    };

    let converted = convert_message(&msg).unwrap();

    match converted {
        ChatCompletionRequestMessage::User(user_msg) => {
            match user_msg.content {
                ChatCompletionRequestUserMessageContent::Array(parts) => {
                    assert_eq!(parts.len(), 3);

                    // First: text
                    match &parts[0] {
                        ChatCompletionRequestUserMessageContentPart::Text(text_part) => {
                            assert_eq!(text_part.text, "Compare these images");
                        }
                        _ => panic!("Expected text part"),
                    }

                    // Second: first image
                    match &parts[1] {
                        ChatCompletionRequestUserMessageContentPart::ImageUrl(img_part) => {
                            assert_eq!(
                                img_part.image_url.url,
                                "https://example.com/image1.jpg"
                            );
                            assert_eq!(img_part.image_url.detail, Some(OpenAIImageDetail::Low));
                        }
                        _ => panic!("Expected image part"),
                    }

                    // Third: second image
                    match &parts[2] {
                        ChatCompletionRequestUserMessageContentPart::ImageUrl(img_part) => {
                            assert_eq!(
                                img_part.image_url.url,
                                "https://example.com/image2.jpg"
                            );
                            assert_eq!(img_part.image_url.detail, Some(OpenAIImageDetail::Low));
                        }
                        _ => panic!("Expected image part"),
                    }
                }
                _ => panic!("Expected array content"),
            }
        }
        _ => panic!("Expected user message"),
    }
}

#[test]
fn test_message_conversion_human_text_only_optimized() {
    use dashflow::core::messages::{ContentBlock, MessageContent};

    // Single text block should be optimized to text format, not array
    let msg = Message::Human {
        content: MessageContent::Blocks(vec![ContentBlock::Text {
            text: "Hello".to_string(),
        }]),
        fields: Default::default(),
    };

    let converted = convert_message(&msg).unwrap();

    match converted {
        ChatCompletionRequestMessage::User(user_msg) => match user_msg.content {
            ChatCompletionRequestUserMessageContent::Text(text) => {
                assert_eq!(text, "Hello");
            }
            _ => panic!("Expected text content, not array (should be optimized)"),
        },
        _ => panic!("Expected user message"),
    }
}

#[test]
fn test_image_detail_conversions() {
    use dashflow::core::messages::{ImageDetail, ImageSource};

    // Test all ImageDetail variants
    let low_image = ImageSource::Url {
        url: "https://example.com/image.jpg".to_string(),
    };
    let converted = convert_image_source(&low_image, Some(ImageDetail::Low));
    assert_eq!(converted.detail, Some(OpenAIImageDetail::Low));

    let converted = convert_image_source(&low_image, Some(ImageDetail::High));
    assert_eq!(converted.detail, Some(OpenAIImageDetail::High));

    let converted = convert_image_source(&low_image, Some(ImageDetail::Auto));
    assert_eq!(converted.detail, Some(OpenAIImageDetail::Auto));

    let converted = convert_image_source(&low_image, None);
    assert!(converted.detail.is_none());
}

// Azure-specific tests
#[test]
fn test_azure_chat_openai_builder() {
    let model = AzureChatOpenAI::with_config(Default::default())
        .with_deployment("my-gpt4-deployment")
        .with_api_version("2024-05-01-preview")
        .with_model("gpt-4")
        .with_temperature(0.7)
        .with_max_tokens(1000);

    assert_eq!(
        model.deployment_name,
        Some("my-gpt4-deployment".to_string())
    );
    assert_eq!(model.api_version, Some("2024-05-01-preview".to_string()));
    assert_eq!(model.model, "gpt-4");
    assert_eq!(model.temperature, Some(0.7));
    assert_eq!(model.max_tokens, Some(1000));
}

#[test]
fn test_azure_default_model() {
    let model = AzureChatOpenAI::with_config(Default::default());
    assert_eq!(model.model, "gpt-35-turbo");
}

#[test]
#[allow(deprecated)]
fn test_azure_with_tools() {
    let tool = serde_json::json!({
        "name": "get_weather",
        "description": "Get current weather",
        "parameters": {
            "type": "object",
            "properties": {
                "location": {"type": "string"}
            }
        }
    });

    let model = AzureChatOpenAI::with_config(Default::default()).with_tools(vec![tool]);

    assert!(model.tools.is_some());
    let tools = model.tools.unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].function.name, "get_weather");
}

#[test]
fn test_azure_with_json_mode() {
    let model = AzureChatOpenAI::with_config(Default::default()).with_json_mode();

    assert!(model.response_format.is_some());
    match model.response_format.unwrap() {
        ResponseFormat::JsonObject => {}
        _ => panic!("Expected JsonObject response format"),
    }
}

#[test]
fn test_azure_with_structured_output() {
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "age": {"type": "number"}
        }
    });

    let model = AzureChatOpenAI::with_config(Default::default()).with_structured_output(
        "user_info",
        schema,
        Some("User info format".to_string()),
        true,
    );

    assert!(model.response_format.is_some());
    match model.response_format.unwrap() {
        ResponseFormat::JsonSchema { json_schema } => {
            assert_eq!(json_schema.name, "user_info");
            assert_eq!(json_schema.strict, Some(true));
        }
        _ => panic!("Expected JsonSchema response format"),
    }
}

#[test]
fn test_azure_llm_type() {
    let model = AzureChatOpenAI::with_config(Default::default());
    assert_eq!(model.llm_type(), "azure-openai-chat");
}

#[test]
fn test_azure_identifying_params() {
    let model = AzureChatOpenAI::with_config(Default::default())
        .with_deployment("my-deployment")
        .with_api_version("2024-05-01-preview")
        .with_model("gpt-4")
        .with_temperature(0.7);

    let params = model.identifying_params();

    assert_eq!(params.get("model"), Some(&serde_json::json!("gpt-4")));
    assert_eq!(
        params.get("deployment"),
        Some(&serde_json::json!("my-deployment"))
    );
    assert_eq!(
        params.get("api_version"),
        Some(&serde_json::json!("2024-05-01-preview"))
    );
    // Check temperature is present and is a number (floating point comparison can have precision issues)
    assert!(params.contains_key("temperature"));
    assert!(params.get("temperature").unwrap().is_f64());
}

#[test]
fn test_azure_with_tool_choice() {
    let model = AzureChatOpenAI::with_config(Default::default())
        .with_tool_choice(Some("required".to_string()));

    assert!(model.tool_choice.is_some());
    match model.tool_choice.unwrap() {
        ChatCompletionToolChoiceOption::Required => {}
        _ => panic!("Expected Required tool choice"),
    }
}

#[test]
fn test_azure_builder_pattern() {
    // Test that builder pattern works correctly
    let model = AzureChatOpenAI::with_config(Default::default())
        .with_deployment("test-deployment")
        .with_api_version("2024-05-01-preview")
        .with_azure_endpoint("https://test-resource.openai.azure.com")
        .with_model("gpt-4")
        .with_temperature(0.5)
        .with_max_tokens(2000)
        .with_top_p(0.95)
        .with_frequency_penalty(0.1)
        .with_presence_penalty(0.2)
        .with_n(1);

    assert_eq!(model.deployment_name, Some("test-deployment".to_string()));
    assert_eq!(model.api_version, Some("2024-05-01-preview".to_string()));
    assert_eq!(model.model, "gpt-4");
    assert_eq!(model.temperature, Some(0.5));
    assert_eq!(model.max_tokens, Some(2000));
    assert_eq!(model.top_p, Some(0.95));
    assert_eq!(model.frequency_penalty, Some(0.1));
    assert_eq!(model.presence_penalty, Some(0.2));
    assert_eq!(model.n, Some(1));
}

#[test]
fn test_azure_chat_openai_serialization_simple() {
    use dashflow::core::serialization::Serializable;

    let model = AzureChatOpenAI::with_config(Default::default())
        .with_deployment("my-gpt4-deployment")
        .with_model("gpt-4");

    let json_value = model.to_json_value().unwrap();

    // Check structure
    assert_eq!(json_value["lc"], 1);
    assert_eq!(json_value["type"], "constructor");
    assert_eq!(
        json_value["id"],
        serde_json::json!(["dashflow", "chat_models", "azure_openai", "AzureChatOpenAI"])
    );

    // Check kwargs
    let kwargs = &json_value["kwargs"];
    assert_eq!(kwargs["model"], "gpt-4");
    assert_eq!(kwargs["deployment_name"], "my-gpt4-deployment");
}

#[test]
fn test_azure_chat_openai_serialization_with_parameters() {
    use dashflow::core::serialization::Serializable;

    let model = AzureChatOpenAI::with_config(Default::default())
        .with_deployment("my-deployment")
        .with_api_version("2024-05-01-preview")
        .with_model("gpt-4")
        .with_temperature(0.7)
        .with_max_tokens(1000)
        .with_top_p(0.9);

    let json_value = model.to_json_value().unwrap();
    let kwargs = &json_value["kwargs"];

    assert_eq!(kwargs["model"], "gpt-4");
    assert_eq!(kwargs["deployment_name"], "my-deployment");
    assert_eq!(kwargs["api_version"], "2024-05-01-preview");

    // Use approximate comparison for floats due to f32 precision
    assert!(
        (kwargs["temperature"].as_f64().unwrap() - 0.7).abs() < 0.01,
        "temperature should be approximately 0.7"
    );
    assert_eq!(kwargs["max_tokens"], 1000);
    assert!(
        (kwargs["top_p"].as_f64().unwrap() - 0.9).abs() < 0.01,
        "top_p should be approximately 0.9"
    );
}

#[test]
fn test_azure_chat_openai_lc_secrets() {
    use dashflow::core::serialization::Serializable;

    let model = AzureChatOpenAI::with_config(Default::default());
    let secrets = model.lc_secrets();

    assert_eq!(
        secrets.get("api_key"),
        Some(&"AZURE_OPENAI_API_KEY".to_string())
    );
    assert_eq!(
        secrets.get("azure_endpoint"),
        Some(&"AZURE_OPENAI_ENDPOINT".to_string())
    );
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

    let model = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-3.5-turbo")
        .with_rate_limiter(rate_limiter);

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
fn test_chat_openai_serialization_simple() {
    use dashflow::core::serialization::Serializable;

    let model = ChatOpenAI::with_config(Default::default()).with_model("gpt-4");

    let json_value = model.to_json_value().unwrap();

    // Check structure
    assert_eq!(json_value["lc"], 1);
    assert_eq!(json_value["type"], "constructor");
    assert_eq!(
        json_value["id"],
        serde_json::json!(["dashflow", "chat_models", "openai", "ChatOpenAI"])
    );

    // Check kwargs
    let kwargs = &json_value["kwargs"];
    assert_eq!(kwargs["model"], "gpt-4");
}

#[test]
fn test_chat_openai_serialization_with_parameters() {
    use dashflow::core::serialization::Serializable;

    let model = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-4")
        .with_temperature(0.7)
        .with_max_tokens(1000)
        .with_top_p(0.9);

    let json_value = model.to_json_value().unwrap();
    let kwargs = &json_value["kwargs"];

    assert_eq!(kwargs["model"], "gpt-4");
    // Use approximate comparison for floats due to f32 precision
    assert!(
        (kwargs["temperature"].as_f64().unwrap() - 0.7).abs() < 0.01,
        "temperature should be approximately 0.7"
    );
    assert_eq!(kwargs["max_tokens"], 1000);
    assert!(
        (kwargs["top_p"].as_f64().unwrap() - 0.9).abs() < 0.01,
        "top_p should be approximately 0.9"
    );
}

#[test]
fn test_chat_openai_serialization_secrets() {
    use dashflow::core::serialization::Serializable;

    let model = ChatOpenAI::with_config(Default::default());
    let secrets = model.lc_secrets();

    // Should mark API key as a secret
    assert_eq!(secrets.get("api_key"), Some(&"OPENAI_API_KEY".to_string()));
}

#[test]
fn test_chat_openai_serialization_no_api_key_in_output() {
    use dashflow::core::serialization::Serializable;

    let model = ChatOpenAI::with_config(Default::default()).with_model("gpt-4");

    let json_string = model.to_json_string(false).unwrap();

    // Verify API key is NOT in the serialized output
    assert!(!json_string.contains("api_key"));
    assert!(!json_string.contains("OPENAI_API_KEY"));
    assert!(!json_string.contains("sk-")); // OpenAI key prefix
}

#[test]
fn test_chat_openai_serialization_pretty_json() {
    use dashflow::core::serialization::Serializable;

    let model = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-4")
        .with_temperature(0.7);

    let json_string = model.to_json_string(true).unwrap();

    // Should be pretty-printed (contains newlines)
    assert!(json_string.contains('\n'));
    assert!(json_string.contains("ChatOpenAI"));
    assert!(json_string.contains("\"model\": \"gpt-4\""));
}
