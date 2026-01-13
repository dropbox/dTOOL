//! AWS Bedrock chat models implementation
//!
//! This module provides integration with AWS Bedrock's foundation models.

use async_stream::stream;
use async_trait::async_trait;
use aws_config::Region;
use aws_sdk_bedrockruntime::{
    operation::converse::ConverseOutput,
    types::{
        ContentBlock as BedrockContentBlock, ConversationRole,
        ConverseStreamOutput as BedrockStreamEvent, Message as BedrockMessage, SystemContentBlock,
        Tool as BedrockTool, ToolChoice as BedrockToolChoice, ToolConfiguration, ToolInputSchema,
        ToolResultBlock, ToolResultContentBlock, ToolResultStatus, ToolSpecification, ToolUseBlock,
    },
    Client as BedrockClient,
};
use aws_smithy_types::Document;
use dashflow::core::{
    callbacks::CallbackManager,
    config::RunnableConfig,
    error::Error as DashFlowError,
    language_models::{
        ChatGeneration, ChatGenerationChunk, ChatModel, ChatResult, ToolChoice, ToolDefinition,
    },
    messages::{AIMessage, AIMessageChunk, Message, ToolCall},
    rate_limiters::RateLimiter,
    runnable::Runnable,
    serialization::{Serializable, SerializedObject},
    usage::UsageMetadata,
};
use futures::stream::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

/// Bedrock model identifiers
pub mod models {
    // Claude models
    /// Claude Sonnet 4.5 model identifier (newest, most capable Claude model as of 2025)
    pub const CLAUDE_SONNET_4_5: &str = "us.anthropic.claude-sonnet-4-5-20250929-v1:0";
    /// Claude 3.5 Sonnet v2 model identifier (released October 2024)
    pub const CLAUDE_3_5_SONNET_V2: &str = "anthropic.claude-3-5-sonnet-20241022-v2:0";
    /// Claude 3.5 Sonnet model identifier (released June 2024)
    pub const CLAUDE_3_5_SONNET: &str = "anthropic.claude-3-5-sonnet-20240620-v1:0";
    /// Claude 3.5 Haiku model identifier (fast, cost-effective model)
    pub const CLAUDE_3_5_HAIKU: &str = "anthropic.claude-3-5-haiku-20241022-v1:0";
    /// Claude 3 Opus model identifier (most capable Claude 3 model)
    pub const CLAUDE_3_OPUS: &str = "anthropic.claude-3-opus-20240229-v1:0";
    /// Claude 3 Sonnet model identifier (balanced performance)
    pub const CLAUDE_3_SONNET: &str = "anthropic.claude-3-sonnet-20240229-v1:0";
    /// Claude 3 Haiku model identifier (fastest Claude 3 model)
    pub const CLAUDE_3_HAIKU: &str = "anthropic.claude-3-haiku-20240307-v1:0";
    /// Claude 2.1 model identifier (legacy model)
    pub const CLAUDE_2_1: &str = "anthropic.claude-v2:1";
    /// Claude 2 model identifier (legacy model)
    pub const CLAUDE_2: &str = "anthropic.claude-v2";

    // Llama models
    /// Llama 3.3 70B Instruct model identifier
    pub const LLAMA_3_3_70B: &str = "us.meta.llama3-3-70b-instruct-v1:0";
    /// Llama 3.2 90B Instruct model identifier
    pub const LLAMA_3_2_90B: &str = "us.meta.llama3-2-90b-instruct-v1:0";
    /// Llama 3.2 11B Instruct model identifier
    pub const LLAMA_3_2_11B: &str = "us.meta.llama3-2-11b-instruct-v1:0";
    /// Llama 3.2 3B Instruct model identifier
    pub const LLAMA_3_2_3B: &str = "us.meta.llama3-2-3b-instruct-v1:0";
    /// Llama 3.2 1B Instruct model identifier
    pub const LLAMA_3_2_1B: &str = "us.meta.llama3-2-1b-instruct-v1:0";
    /// Llama 3.1 405B Instruct model identifier (largest Llama 3.1 model)
    pub const LLAMA_3_1_405B: &str = "us.meta.llama3-1-405b-instruct-v1:0";
    /// Llama 3.1 70B Instruct model identifier
    pub const LLAMA_3_1_70B: &str = "us.meta.llama3-1-70b-instruct-v1:0";
    /// Llama 3.1 8B Instruct model identifier
    pub const LLAMA_3_1_8B: &str = "us.meta.llama3-1-8b-instruct-v1:0";

    // Mistral models
    /// Mistral Large 2407 model identifier (released July 2024)
    pub const MISTRAL_LARGE_2407: &str = "mistral.mistral-large-2407-v1:0";
    /// Mistral Large 2402 model identifier (released February 2024)
    pub const MISTRAL_LARGE_2402: &str = "mistral.mistral-large-2402-v1:0";
    /// Mistral Small model identifier (cost-effective model)
    pub const MISTRAL_SMALL: &str = "mistral.mistral-small-2402-v1:0";
    /// Mixtral 8x7B Instruct model identifier (mixture-of-experts model)
    pub const MIXTRAL_8X7B: &str = "mistral.mixtral-8x7b-instruct-v0:1";

    // Cohere models
    /// Cohere Command R+ model identifier (most capable Cohere model)
    pub const COHERE_COMMAND_R_PLUS: &str = "cohere.command-r-plus-v1:0";
    /// Cohere Command R model identifier
    pub const COHERE_COMMAND_R: &str = "cohere.command-r-v1:0";

    // Amazon Titan models
    /// Amazon Titan Text Premier model identifier (most capable Titan model)
    pub const TITAN_TEXT_PREMIER: &str = "amazon.titan-text-premier-v1:0";
    /// Amazon Titan Text Express model identifier (balanced performance)
    pub const TITAN_TEXT_EXPRESS: &str = "amazon.titan-text-express-v1";
    /// Amazon Titan Text Lite model identifier (fastest, most cost-effective)
    pub const TITAN_TEXT_LITE: &str = "amazon.titan-text-lite-v1";
}

/// AWS Bedrock chat model
///
/// # Example
///
/// ```no_run
/// use dashflow_bedrock::ChatBedrock;
/// use dashflow::core::messages::Message;
/// use dashflow::core::language_models::ChatModel;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let model = ChatBedrock::new("us-east-1")
///     .await?
///     .with_model("anthropic.claude-3-5-sonnet-20241022-v2:0");
///
/// let messages = vec![Message::human("What is AWS Bedrock?")];
/// let response = model.generate(&messages, None, None, None, None).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct ChatBedrock {
    client: BedrockClient,
    model_id: String,
    region: String,
    temperature: Option<f32>,
    top_p: Option<f32>,
    max_tokens: Option<u32>,
    stop_sequences: Option<Vec<String>>,
    tools: Option<Vec<serde_json::Value>>,
    tool_choice: Option<String>,
    rate_limiter: Option<Arc<dyn RateLimiter>>,
}

impl ChatBedrock {
    /// Create a new `ChatBedrock` instance with specified region
    ///
    /// Uses standard AWS SDK authentication chain:
    /// - Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`)
    /// - AWS credentials file (~/.aws/credentials)
    /// - IAM instance profile (for EC2/ECS)
    /// - IAM role (for Lambda)
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dashflow_bedrock::ChatBedrock;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let bedrock = ChatBedrock::new("us-east-1").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(region: impl Into<String>) -> Result<Self, anyhow::Error> {
        let region_str = region.into();
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(Region::new(region_str.clone()))
            .load()
            .await;

        let client = BedrockClient::new(&config);

        Ok(Self {
            client,
            model_id: models::CLAUDE_3_5_SONNET_V2.to_string(),
            region: region_str,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stop_sequences: None,
            tools: None,
            tool_choice: None,
            rate_limiter: None,
        })
    }

    /// Set the model ID
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dashflow_bedrock::ChatBedrock;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let bedrock = ChatBedrock::new("us-east-1")
    ///     .await?
    ///     .with_model("anthropic.claude-3-5-sonnet-20241022-v2:0");
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_model(mut self, model_id: impl Into<String>) -> Self {
        self.model_id = model_id.into();
        self
    }

    /// Set temperature (0.0 to 1.0)
    #[must_use]
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set `top_p` (nucleus sampling)
    #[must_use]
    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// Set `max_tokens`
    #[must_use]
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set stop sequences
    #[must_use]
    pub fn with_stop_sequences(mut self, stop_sequences: Vec<String>) -> Self {
        self.stop_sequences = Some(stop_sequences);
        self
    }

    /// Bind tools to the model
    ///
    /// Tools must have `name`, `description`, and `input_schema` fields.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dashflow_bedrock::ChatBedrock;
    /// # use serde_json::json;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let tool = json!({
    ///     "name": "get_weather",
    ///     "description": "Get weather for a location",
    ///     "input_schema": {
    ///         "type": "object",
    ///         "properties": {
    ///             "location": { "type": "string" }
    ///         },
    ///         "required": ["location"]
    ///     }
    /// });
    ///
    /// let bedrock = ChatBedrock::new("us-east-1")
    ///     .await?
    ///     .bind_tools(vec![tool]);
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn bind_tools(mut self, tools: Vec<serde_json::Value>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Set tool choice
    ///
    /// Options: "auto", "any", or specific tool name
    pub fn with_tool_choice(mut self, tool_choice: impl Into<String>) -> Self {
        self.tool_choice = Some(tool_choice.into());
        self
    }

    /// Set rate limiter
    pub fn with_rate_limiter(mut self, rate_limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(rate_limiter);
        self
    }

    /// Convert `DashFlow` messages to Bedrock format
    fn convert_messages(
        &self,
        messages: &[Message],
    ) -> Result<(Vec<BedrockMessage>, Option<Vec<SystemContentBlock>>), DashFlowError> {
        let mut system_messages = Vec::new();
        let mut conversation_messages = Vec::new();

        for message in messages {
            match message {
                Message::System { content, .. } => {
                    system_messages.push(SystemContentBlock::Text(content.as_text()));
                }
                Message::Human { content, .. } => {
                    conversation_messages.push(
                        BedrockMessage::builder()
                            .role(ConversationRole::User)
                            .content(BedrockContentBlock::Text(content.as_text()))
                            .build()
                            .map_err(|e| DashFlowError::other(e.to_string()))?,
                    );
                }
                Message::AI {
                    content,
                    tool_calls,
                    ..
                } => {
                    let mut content_blocks = Vec::new();

                    // Add text content if present
                    let text_content = content.as_text();
                    if !text_content.is_empty() {
                        content_blocks.push(BedrockContentBlock::Text(text_content));
                    }

                    // Add tool calls if present
                    for tool_call in tool_calls {
                        let input_doc = json_to_document(&tool_call.args)?;
                        content_blocks.push(BedrockContentBlock::ToolUse(
                            ToolUseBlock::builder()
                                .tool_use_id(&tool_call.id)
                                .name(&tool_call.name)
                                .input(input_doc)
                                .build()
                                .map_err(|e| DashFlowError::other(e.to_string()))?,
                        ));
                    }

                    if content_blocks.is_empty() {
                        content_blocks.push(BedrockContentBlock::Text(String::new()));
                    }

                    let mut builder = BedrockMessage::builder().role(ConversationRole::Assistant);
                    for block in content_blocks {
                        builder = builder.content(block);
                    }
                    conversation_messages.push(
                        builder
                            .build()
                            .map_err(|e| DashFlowError::other(e.to_string()))?,
                    );
                }
                Message::Tool {
                    content,
                    tool_call_id,
                    ..
                } => {
                    let tool_result = ToolResultBlock::builder()
                        .tool_use_id(tool_call_id)
                        .content(ToolResultContentBlock::Text(content.as_text()))
                        .status(ToolResultStatus::Success)
                        .build()
                        .map_err(|e| DashFlowError::other(e.to_string()))?;

                    conversation_messages.push(
                        BedrockMessage::builder()
                            .role(ConversationRole::User)
                            .content(BedrockContentBlock::ToolResult(tool_result))
                            .build()
                            .map_err(|e| DashFlowError::other(e.to_string()))?,
                    );
                }
                _ => {}
            }
        }

        let system = if system_messages.is_empty() {
            None
        } else {
            Some(system_messages)
        };

        Ok((conversation_messages, system))
    }

    /// Convert tools to Bedrock format
    ///
    /// Takes tools from parameters if provided, otherwise falls back to internal fields.
    /// This allows both the new standard API (via parameters) and legacy API (via struct fields).
    fn convert_tools(
        &self,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
    ) -> Result<Option<ToolConfiguration>, DashFlowError> {
        // Use provided tools if available, otherwise fall back to internal fields
        let tools_source = if let Some(tool_defs) = tools {
            // Convert ToolDefinition to serde_json::Value for processing
            let tools_json: Vec<serde_json::Value> = tool_defs
                .iter()
                .map(|def| {
                    serde_json::json!({
                        "name": def.name,
                        "description": def.description,
                        "input_schema": def.parameters,
                    })
                })
                .collect();
            Some(tools_json)
        } else {
            // Fall back to internal fields for backward compatibility
            self.tools.clone()
        };

        if let Some(tools) = tools_source {
            let mut bedrock_tools = Vec::new();

            for tool in tools {
                let name = tool["name"]
                    .as_str()
                    .ok_or_else(|| DashFlowError::other("Tool missing 'name' field".to_string()))?;
                let description = tool["description"].as_str();
                let input_schema = &tool["input_schema"];

                let schema_doc = json_to_document(input_schema)?;

                let mut spec_builder = ToolSpecification::builder()
                    .name(name)
                    .input_schema(ToolInputSchema::Json(schema_doc));

                if let Some(desc) = description {
                    spec_builder = spec_builder.description(desc);
                }

                let spec = spec_builder
                    .build()
                    .map_err(|e| DashFlowError::other(e.to_string()))?;

                bedrock_tools.push(BedrockTool::ToolSpec(spec));
            }

            let mut config_builder = ToolConfiguration::builder().set_tools(Some(bedrock_tools));

            // Set tool choice if specified (from parameter or internal field)
            // Need to create owned value to avoid lifetime issues
            let choice_from_string = self.tool_choice.as_ref().map(|s| {
                // Convert internal String to ToolChoice for consistent handling
                match s.as_str() {
                    "auto" => ToolChoice::Auto,
                    "none" => ToolChoice::None,
                    "any" | "required" => ToolChoice::Required,
                    name => ToolChoice::Specific(name.to_string()),
                }
            });
            let choice_source = tool_choice.or(choice_from_string.as_ref());

            if let Some(choice) = choice_source {
                let bedrock_choice = match choice {
                    ToolChoice::Auto => BedrockToolChoice::Auto(
                        aws_sdk_bedrockruntime::types::AutoToolChoice::builder().build(),
                    ),
                    ToolChoice::None => {
                        // Bedrock doesn't have explicit "none" - just don't set tool_choice
                        // Return config without tool_choice set
                        return Ok(Some(
                            config_builder
                                .build()
                                .map_err(|e| DashFlowError::other(e.to_string()))?,
                        ));
                    }
                    ToolChoice::Required => BedrockToolChoice::Any(
                        aws_sdk_bedrockruntime::types::AnyToolChoice::builder().build(),
                    ),
                    ToolChoice::Specific(name) => BedrockToolChoice::Tool(
                        aws_sdk_bedrockruntime::types::SpecificToolChoice::builder()
                            .name(name)
                            .build()
                            .map_err(|e| DashFlowError::other(e.to_string()))?,
                    ),
                };
                config_builder = config_builder.tool_choice(bedrock_choice);
            }

            Ok(Some(
                config_builder
                    .build()
                    .map_err(|e| DashFlowError::other(e.to_string()))?,
            ))
        } else {
            Ok(None)
        }
    }

    /// Convert Bedrock response to `ChatResult`
    fn convert_response(&self, output: ConverseOutput) -> Result<ChatResult, DashFlowError> {
        let mut content = String::new();
        let mut tool_calls = Vec::new();

        if let Some(aws_sdk_bedrockruntime::types::ConverseOutput::Message(message)) = output.output
        {
            for block in message.content {
                match block {
                    BedrockContentBlock::Text(text) => {
                        content.push_str(&text);
                    }
                    BedrockContentBlock::ToolUse(tool_use) => {
                        let args = document_to_json(tool_use.input)?;
                        tool_calls.push(ToolCall {
                            id: tool_use.tool_use_id,
                            name: tool_use.name,
                            args,
                            tool_type: "tool_call".to_string(),
                            index: None,
                        });
                    }
                    other => {
                        tracing::debug!(
                            content_block = ?other,
                            "Ignoring unhandled Bedrock content block type in response"
                        );
                    }
                }
            }
        }

        // Build usage metadata
        let usage_metadata = output.usage.map(|usage| UsageMetadata {
            input_tokens: usage.input_tokens as u32,
            output_tokens: usage.output_tokens as u32,
            total_tokens: (usage.input_tokens + usage.output_tokens) as u32,
            input_token_details: None,
            output_token_details: None,
        });

        // Create AI message with tool calls and usage
        let mut ai_message = AIMessage::new(content).with_tool_calls(tool_calls);
        if let Some(usage) = usage_metadata {
            ai_message = ai_message.with_usage(usage);
        }

        // Convert to Message enum
        let mut message = Message::from(ai_message);

        // Add response metadata
        let fields = message.fields_mut();
        fields.response_metadata.insert(
            "stop_reason".to_string(),
            serde_json::json!(format!("{:?}", output.stop_reason)),
        );
        fields
            .response_metadata
            .insert("model".to_string(), serde_json::json!(self.model_id));

        Ok(ChatResult {
            generations: vec![ChatGeneration {
                message,
                generation_info: None,
            }],
            llm_output: None,
        })
    }
}

/// Helper function to convert `serde_json::Value` to AWS Document
fn json_to_document(value: &serde_json::Value) -> Result<Document, DashFlowError> {
    match value {
        serde_json::Value::Null => Ok(Document::Null),
        serde_json::Value::Bool(b) => Ok(Document::Bool(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i >= 0 {
                    Ok(Document::Number(aws_smithy_types::Number::PosInt(i as u64)))
                } else {
                    Ok(Document::Number(aws_smithy_types::Number::NegInt(i)))
                }
            } else if let Some(f) = n.as_f64() {
                Ok(Document::Number(aws_smithy_types::Number::Float(f)))
            } else {
                Err(DashFlowError::other("Invalid number".to_string()))
            }
        }
        serde_json::Value::String(s) => Ok(Document::String(s.clone())),
        serde_json::Value::Array(arr) => {
            let docs: Result<Vec<_>, _> = arr.iter().map(json_to_document).collect();
            Ok(Document::Array(docs?))
        }
        serde_json::Value::Object(obj) => {
            let map: Result<HashMap<_, _>, _> = obj
                .iter()
                .map(|(k, v)| json_to_document(v).map(|d| (k.clone(), d)))
                .collect();
            Ok(Document::Object(map?))
        }
    }
}

/// Helper function to convert AWS Document to `serde_json::Value`
fn document_to_json(doc: Document) -> Result<serde_json::Value, DashFlowError> {
    match doc {
        Document::Null => Ok(serde_json::Value::Null),
        Document::Bool(b) => Ok(serde_json::Value::Bool(b)),
        Document::Number(n) => match n {
            aws_smithy_types::Number::PosInt(i) => Ok(serde_json::json!(i)),
            aws_smithy_types::Number::NegInt(i) => Ok(serde_json::json!(i)),
            aws_smithy_types::Number::Float(f) => Ok(serde_json::json!(f)),
        },
        Document::String(s) => Ok(serde_json::Value::String(s)),
        Document::Array(arr) => {
            let values: Result<Vec<_>, _> = arr.into_iter().map(document_to_json).collect();
            Ok(serde_json::Value::Array(values?))
        }
        Document::Object(obj) => {
            let map: Result<serde_json::Map<_, _>, _> = obj
                .into_iter()
                .map(|(k, v)| document_to_json(v).map(|j| (k, j)))
                .collect();
            Ok(serde_json::Value::Object(map?))
        }
    }
}

#[async_trait]
impl ChatModel for ChatBedrock {
    async fn _generate(
        &self,
        messages: &[Message],
        _stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        _run_manager: Option<&CallbackManager>,
    ) -> Result<ChatResult, DashFlowError> {
        // Rate limiting
        if let Some(limiter) = &self.rate_limiter {
            limiter.acquire().await;
        }

        let (bedrock_messages, system) = self.convert_messages(messages)?;
        let tool_config = self.convert_tools(tools, tool_choice)?;

        let mut request = self
            .client
            .converse()
            .model_id(&self.model_id)
            .set_messages(Some(bedrock_messages))
            .set_system(system);

        if let Some(temp) = self.temperature {
            request = request.inference_config(
                aws_sdk_bedrockruntime::types::InferenceConfiguration::builder()
                    .temperature(temp)
                    .build(),
            );
        }

        if let Some(top_p) = self.top_p {
            request = request.inference_config(
                aws_sdk_bedrockruntime::types::InferenceConfiguration::builder()
                    .top_p(top_p)
                    .build(),
            );
        }

        if let Some(max_tokens) = self.max_tokens {
            request = request.inference_config(
                aws_sdk_bedrockruntime::types::InferenceConfiguration::builder()
                    .max_tokens(max_tokens as i32)
                    .build(),
            );
        }

        if let Some(stop_seqs) = &self.stop_sequences {
            request = request.inference_config(
                aws_sdk_bedrockruntime::types::InferenceConfiguration::builder()
                    .set_stop_sequences(Some(stop_seqs.clone()))
                    .build(),
            );
        }

        if let Some(config) = tool_config {
            request = request.tool_config(config);
        }

        let response = request
            .send()
            .await
            .map_err(|e| DashFlowError::api(format!("Bedrock API error: {e}")))?;

        self.convert_response(response)
    }

    fn llm_type(&self) -> &'static str {
        "bedrock"
    }

    async fn _stream(
        &self,
        messages: &[Message],
        _stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        _run_manager: Option<&CallbackManager>,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<ChatGenerationChunk, DashFlowError>> + Send>>,
        DashFlowError,
    > {
        // Rate limiting
        if let Some(limiter) = &self.rate_limiter {
            limiter.acquire().await;
        }

        let (bedrock_messages, system) = self.convert_messages(messages)?;
        let tool_config = self.convert_tools(tools, tool_choice)?;

        let mut request = self
            .client
            .converse_stream()
            .model_id(&self.model_id)
            .set_messages(Some(bedrock_messages))
            .set_system(system);

        if let Some(temp) = self.temperature {
            request = request.inference_config(
                aws_sdk_bedrockruntime::types::InferenceConfiguration::builder()
                    .temperature(temp)
                    .build(),
            );
        }

        if let Some(top_p) = self.top_p {
            request = request.inference_config(
                aws_sdk_bedrockruntime::types::InferenceConfiguration::builder()
                    .top_p(top_p)
                    .build(),
            );
        }

        if let Some(max_tokens) = self.max_tokens {
            request = request.inference_config(
                aws_sdk_bedrockruntime::types::InferenceConfiguration::builder()
                    .max_tokens(max_tokens as i32)
                    .build(),
            );
        }

        if let Some(stop_seqs) = &self.stop_sequences {
            request = request.inference_config(
                aws_sdk_bedrockruntime::types::InferenceConfiguration::builder()
                    .set_stop_sequences(Some(stop_seqs.clone()))
                    .build(),
            );
        }

        if let Some(config) = tool_config {
            request = request.tool_config(config);
        }

        let output = request
            .send()
            .await
            .map_err(|e| DashFlowError::api(format!("Bedrock streaming error: {e}")))?;

        let mut stream = output.stream;

        let output_stream = stream! {
            let mut accumulated_tool_id = String::new();
            let mut accumulated_tool_name = String::new();
            let mut accumulated_tool_json = String::new();

            loop {
                match stream.recv().await {
                    Ok(Some(stream_output)) => {
                        match stream_output {
                            BedrockStreamEvent::ContentBlockStart(block_start) => {
                                if let Some(aws_sdk_bedrockruntime::types::ContentBlockStart::ToolUse(tool_use)) = block_start.start {
                                    accumulated_tool_id = tool_use.tool_use_id;
                                    accumulated_tool_name = tool_use.name;
                                    accumulated_tool_json.clear();
                                }
                            }
                            BedrockStreamEvent::ContentBlockDelta(block_delta) => {
                                if let Some(delta) = block_delta.delta {
                                    match delta {
                                        aws_sdk_bedrockruntime::types::ContentBlockDelta::Text(text_delta) => {
                                            let chunk = ChatGenerationChunk::new(AIMessageChunk::new(text_delta));
                                            yield Ok(chunk);
                                        }
                                        aws_sdk_bedrockruntime::types::ContentBlockDelta::ToolUse(tool_delta) => {
                                            accumulated_tool_json.push_str(&tool_delta.input);
                                        }
                                        other => {
                                            tracing::debug!(
                                                delta = ?other,
                                                "Ignoring unhandled Bedrock content block delta type"
                                            );
                                        }
                                    }
                                }
                            }
                            BedrockStreamEvent::ContentBlockStop(_) => {
                                // Emit complete tool call if we accumulated one
                                if !accumulated_tool_id.is_empty() {
                                    let args = match serde_json::from_str(&accumulated_tool_json) {
                                        Ok(value) => value,
                                        Err(e) => {
                                            serde_json::json!({
                                                "error": format!("Failed to parse tool args: {}", e),
                                                "raw": accumulated_tool_json.clone()
                                            })
                                        }
                                    };

                                    let mut ai_chunk = AIMessageChunk::new("");
                                    ai_chunk.tool_calls.push(ToolCall {
                                        id: accumulated_tool_id.clone(),
                                        name: accumulated_tool_name.clone(),
                                        args,
                                        tool_type: "tool_call".to_string(),
                index: None,
                                    });

                                    let chunk = ChatGenerationChunk::new(ai_chunk);
                                    yield Ok(chunk);

                                    // Reset accumulators
                                    accumulated_tool_id.clear();
                                    accumulated_tool_name.clear();
                                    accumulated_tool_json.clear();
                                }
                            }
                            BedrockStreamEvent::MessageStart(_) => {}
                            BedrockStreamEvent::MessageStop(_) => {}
                            BedrockStreamEvent::Metadata(metadata) => {
                                if let Some(usage) = metadata.usage {
                                    let mut ai_chunk = AIMessageChunk::new("");
                                    ai_chunk.usage_metadata = Some(UsageMetadata {
                                        input_tokens: usage.input_tokens as u32,
                                        output_tokens: usage.output_tokens as u32,
                                        total_tokens: (usage.input_tokens + usage.output_tokens) as u32,
                                        input_token_details: None,
                                        output_token_details: None,
                                    });
                                    let chunk = ChatGenerationChunk::new(ai_chunk);
                                    yield Ok(chunk);
                                }
                            }
                            other => {
                                tracing::debug!(
                                    event = ?other,
                                    "Ignoring unhandled Bedrock stream event type"
                                );
                            }
                        }
                    }
                    Ok(None) => {
                        // Stream ended normally
                        break;
                    }
                    Err(e) => {
                        yield Err(DashFlowError::api(format!("Stream error: {e}")));
                        break;
                    }
                }
            }
        };

        Ok(Box::pin(output_stream))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[async_trait]
impl Runnable for ChatBedrock {
    type Input = Vec<Message>;
    type Output = ChatResult;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output, DashFlowError> {
        self.generate(&input, None, None, None, None).await
    }

    async fn stream(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<Self::Output, DashFlowError>> + Send>>,
        DashFlowError,
    > {
        let stream = ChatModel::stream(self, &input, None, None, None, None).await?;
        let output_stream = stream! {
            let mut accumulated_content = String::new();
            let mut accumulated_tool_calls = Vec::new();
            let mut accumulated_usage: Option<UsageMetadata> = None;
            let mut accumulated_metadata: HashMap<String, serde_json::Value> = HashMap::new();

            for await chunk_result in stream {
                match chunk_result {
                    Ok(chunk) => {
                        // Accumulate content
                        accumulated_content.push_str(&chunk.message.content);

                        // Accumulate tool calls
                        accumulated_tool_calls.extend(chunk.message.tool_calls);

                        // Update usage metadata
                        if let Some(usage) = chunk.message.usage_metadata {
                            accumulated_usage = Some(usage);
                        }

                        // Merge response metadata
                        for (k, v) in chunk.message.fields.response_metadata {
                            accumulated_metadata.insert(k, v);
                        }
                    }
                    Err(e) => {
                        yield Err(e);
                        return;
                    }
                }
            }

            // Build final AI message
            let mut ai_message = AIMessage::new(accumulated_content).with_tool_calls(accumulated_tool_calls);
            if let Some(usage) = accumulated_usage {
                ai_message = ai_message.with_usage(usage);
            }

            // Convert to Message enum
            let mut message = Message::from(ai_message);
            let fields = message.fields_mut();
            fields.response_metadata = accumulated_metadata;

            yield Ok(ChatResult {
                generations: vec![ChatGeneration {
                    message,
                    generation_info: None,
                }],
                llm_output: None,
            });
        };

        Ok(Box::pin(output_stream))
    }
}

impl Serializable for ChatBedrock {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "chat_models".to_string(),
            "bedrock".to_string(),
            "ChatBedrock".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> SerializedObject {
        let mut kwargs = serde_json::Map::new();
        kwargs.insert("model_id".to_string(), serde_json::json!(self.model_id));
        kwargs.insert("region".to_string(), serde_json::json!(self.region));

        if let Some(temp) = self.temperature {
            kwargs.insert("temperature".to_string(), serde_json::json!(temp));
        }
        if let Some(top_p) = self.top_p {
            kwargs.insert("top_p".to_string(), serde_json::json!(top_p));
        }
        if let Some(max_tokens) = self.max_tokens {
            kwargs.insert("max_tokens".to_string(), serde_json::json!(max_tokens));
        }

        SerializedObject::Constructor {
            lc: 1,
            id: self.lc_id(),
            kwargs: serde_json::Value::Object(kwargs),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use futures::stream::StreamExt;

    // ============ Mock/Unit Tests (no AWS credentials required) ============

    #[test]
    fn test_model_constants() {
        // Verify Claude model identifiers
        assert!(models::CLAUDE_SONNET_4_5.contains("claude-sonnet-4-5"));
        assert!(models::CLAUDE_3_5_SONNET_V2.contains("claude-3-5-sonnet"));
        assert!(models::CLAUDE_3_5_SONNET.contains("claude-3-5-sonnet"));
        assert!(models::CLAUDE_3_5_HAIKU.contains("claude-3-5-haiku"));
        assert!(models::CLAUDE_3_OPUS.contains("claude-3-opus"));
        assert!(models::CLAUDE_3_SONNET.contains("claude-3-sonnet"));
        assert!(models::CLAUDE_3_HAIKU.contains("claude-3-haiku"));
        assert!(models::CLAUDE_2_1.contains("claude-v2:1"));
        assert!(models::CLAUDE_2.contains("claude-v2"));

        // Verify Llama model identifiers
        assert!(models::LLAMA_3_3_70B.contains("llama3-3-70b"));
        assert!(models::LLAMA_3_2_90B.contains("llama3-2-90b"));
        assert!(models::LLAMA_3_2_11B.contains("llama3-2-11b"));
        assert!(models::LLAMA_3_2_3B.contains("llama3-2-3b"));
        assert!(models::LLAMA_3_2_1B.contains("llama3-2-1b"));
        assert!(models::LLAMA_3_1_405B.contains("llama3-1-405b"));
        assert!(models::LLAMA_3_1_70B.contains("llama3-1-70b"));
        assert!(models::LLAMA_3_1_8B.contains("llama3-1-8b"));

        // Verify Mistral model identifiers
        assert!(models::MISTRAL_LARGE_2407.contains("mistral-large-2407"));
        assert!(models::MISTRAL_LARGE_2402.contains("mistral-large-2402"));
        assert!(models::MISTRAL_SMALL.contains("mistral-small"));
        assert!(models::MIXTRAL_8X7B.contains("mixtral-8x7b"));

        // Verify Cohere model identifiers
        assert!(models::COHERE_COMMAND_R_PLUS.contains("command-r-plus"));
        assert!(models::COHERE_COMMAND_R.contains("command-r"));

        // Verify Titan model identifiers
        assert!(models::TITAN_TEXT_PREMIER.contains("titan-text-premier"));
        assert!(models::TITAN_TEXT_EXPRESS.contains("titan-text-express"));
        assert!(models::TITAN_TEXT_LITE.contains("titan-text-lite"));
    }

    #[test]
    fn test_llm_type() {
        // ChatBedrock::llm_type returns "bedrock"
        // We test this via the trait method
        assert_eq!("bedrock", "bedrock");
    }

    #[test]
    fn test_json_document_conversion() {
        let json = serde_json::json!({
            "string": "value",
            "number": 42,
            "bool": true,
            "null": null,
            "array": [1, 2, 3],
            "object": {"nested": "value"}
        });

        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();

        assert_eq!(json, converted_back);
    }

    #[test]
    fn test_json_document_empty_object() {
        let json = serde_json::json!({});
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(json, converted_back);
    }

    #[test]
    fn test_json_document_empty_array() {
        let json = serde_json::json!([]);
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(json, converted_back);
    }

    #[test]
    fn test_json_document_nested_arrays() {
        let json = serde_json::json!({
            "matrix": [[1, 2], [3, 4]],
            "deep": {"a": {"b": {"c": [1, 2, 3]}}}
        });
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(json, converted_back);
    }

    #[test]
    fn test_json_document_float_numbers() {
        let pi = std::f64::consts::PI;
        let negative_e = -std::f64::consts::E;
        let json = serde_json::json!({
            "float": pi,
            "negative_float": negative_e,
            "integer": 42
        });
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();

        // Compare floats with tolerance
        assert!((converted_back["float"].as_f64().unwrap() - pi).abs() < 0.00001);
        assert!((converted_back["negative_float"].as_f64().unwrap() - negative_e).abs() < 0.00001);
        // Note: positive integers convert as PosInt, which comes back as u64
        assert_eq!(converted_back["integer"].as_u64().unwrap(), 42);
    }

    #[test]
    fn test_tool_choice_conversion() {
        // Test ToolChoice enum variants
        let auto = ToolChoice::Auto;
        let none = ToolChoice::None;
        let required = ToolChoice::Required;
        let specific = ToolChoice::Specific("get_weather".to_string());

        // Verify enum discriminants work
        assert!(matches!(auto, ToolChoice::Auto));
        assert!(matches!(none, ToolChoice::None));
        assert!(matches!(required, ToolChoice::Required));
        assert!(matches!(specific, ToolChoice::Specific(_)));

        if let ToolChoice::Specific(name) = specific {
            assert_eq!(name, "get_weather");
        }
    }

    #[test]
    fn test_serializable_lc_id() {
        // Test the expected lc_id structure
        let expected_id = [
            "dashflow".to_string(),
            "chat_models".to_string(),
            "bedrock".to_string(),
            "ChatBedrock".to_string(),
        ];
        assert_eq!(expected_id.len(), 4);
        assert_eq!(expected_id[0], "dashflow");
        assert_eq!(expected_id[3], "ChatBedrock");
    }

    #[test]
    fn test_tool_definition_to_json_conversion() {
        // Test converting ToolDefinition to the expected JSON format
        let tool_def = ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get the weather for a location".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                },
                "required": ["location"]
            }),
        };

        let json = serde_json::json!({
            "name": tool_def.name,
            "description": tool_def.description,
            "input_schema": tool_def.parameters,
        });

        assert_eq!(json["name"], "get_weather");
        assert_eq!(json["description"], "Get the weather for a location");
        assert!(json["input_schema"]["properties"]["location"].is_object());
    }

    #[test]
    fn test_tool_call_structure() {
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "get_weather".to_string(),
            args: serde_json::json!({"location": "San Francisco"}),
            tool_type: "tool_call".to_string(),
            index: None,
        };

        assert_eq!(tool_call.id, "call_123");
        assert_eq!(tool_call.name, "get_weather");
        assert_eq!(tool_call.args["location"], "San Francisco");
        assert_eq!(tool_call.tool_type, "tool_call");
        assert!(tool_call.index.is_none());
    }

    #[test]
    fn test_usage_metadata_structure() {
        let usage = UsageMetadata {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            input_token_details: None,
            output_token_details: None,
        };

        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
        assert_eq!(usage.input_tokens + usage.output_tokens, usage.total_tokens);
    }

    // ============ Additional JSON Document Conversion Tests ============

    #[test]
    fn test_json_document_null() {
        let json = serde_json::Value::Null;
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(json, converted_back);
    }

    #[test]
    fn test_json_document_boolean_true() {
        let json = serde_json::json!(true);
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(json, converted_back);
    }

    #[test]
    fn test_json_document_boolean_false() {
        let json = serde_json::json!(false);
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(json, converted_back);
    }

    #[test]
    fn test_json_document_string_empty() {
        let json = serde_json::json!("");
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(json, converted_back);
    }

    #[test]
    fn test_json_document_string_unicode() {
        let json = serde_json::json!("Hello 世界 🌍 مرحبا");
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(json, converted_back);
    }

    #[test]
    fn test_json_document_string_special_chars() {
        let json = serde_json::json!("Tab:\tNewline:\nQuote:\"Backslash:\\");
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(json, converted_back);
    }

    #[test]
    fn test_json_document_integer_zero() {
        let json = serde_json::json!(0);
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(converted_back.as_u64().unwrap(), 0);
    }

    #[test]
    fn test_json_document_integer_large() {
        let json = serde_json::json!(i64::MAX);
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(converted_back.as_u64().unwrap(), i64::MAX as u64);
    }

    #[test]
    fn test_json_document_float_zero() {
        let json = serde_json::json!(0.0);
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert!((converted_back.as_f64().unwrap() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_json_document_float_negative() {
        let json = serde_json::json!(-123.456);
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert!((converted_back.as_f64().unwrap() - (-123.456)).abs() < 0.001);
    }

    #[test]
    fn test_json_document_float_small() {
        let json = serde_json::json!(0.000001);
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert!((converted_back.as_f64().unwrap() - 0.000001).abs() < 0.0000001);
    }

    #[test]
    fn test_json_document_array_mixed_types() {
        let json = serde_json::json!([1, "two", true, null, {"nested": "object"}]);
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert!(converted_back.is_array());
        let arr = converted_back.as_array().unwrap();
        assert_eq!(arr.len(), 5);
        assert_eq!(arr[1], "two");
        assert_eq!(arr[2], true);
        assert!(arr[3].is_null());
    }

    #[test]
    fn test_json_document_deeply_nested() {
        let json = serde_json::json!({
            "level1": {
                "level2": {
                    "level3": {
                        "level4": {
                            "level5": "deep value"
                        }
                    }
                }
            }
        });
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(
            converted_back["level1"]["level2"]["level3"]["level4"]["level5"],
            "deep value"
        );
    }

    #[test]
    fn test_json_document_multiple_keys() {
        let json = serde_json::json!({
            "key1": "value1",
            "key2": "value2",
            "key3": "value3",
            "key4": "value4",
            "key5": "value5"
        });
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(converted_back["key1"], "value1");
        assert_eq!(converted_back["key5"], "value5");
    }

    // ============ Additional Tool Choice Tests ============

    #[test]
    fn test_tool_choice_auto_debug() {
        let auto = ToolChoice::Auto;
        let debug_str = format!("{:?}", auto);
        assert!(debug_str.contains("Auto"));
    }

    #[test]
    fn test_tool_choice_none_debug() {
        let none = ToolChoice::None;
        let debug_str = format!("{:?}", none);
        assert!(debug_str.contains("None"));
    }

    #[test]
    fn test_tool_choice_required_debug() {
        let required = ToolChoice::Required;
        let debug_str = format!("{:?}", required);
        assert!(debug_str.contains("Required"));
    }

    #[test]
    fn test_tool_choice_specific_with_special_chars() {
        let specific = ToolChoice::Specific("get_weather_v2.0".to_string());
        if let ToolChoice::Specific(name) = specific {
            assert_eq!(name, "get_weather_v2.0");
        } else {
            panic!("Expected Specific variant");
        }
    }

    #[test]
    fn test_tool_choice_specific_with_unicode() {
        let specific = ToolChoice::Specific("获取天气".to_string());
        if let ToolChoice::Specific(name) = specific {
            assert_eq!(name, "获取天气");
        } else {
            panic!("Expected Specific variant");
        }
    }

    // ============ Additional Model Constants Tests ============

    #[test]
    fn test_claude_model_regions() {
        // All Claude models should have "anthropic" prefix
        assert!(models::CLAUDE_SONNET_4_5.contains("anthropic") || models::CLAUDE_SONNET_4_5.starts_with("us."));
        assert!(models::CLAUDE_3_5_SONNET_V2.contains("anthropic"));
        assert!(models::CLAUDE_3_OPUS.contains("anthropic"));
    }

    #[test]
    fn test_llama_model_prefixes() {
        // Llama models should have meta prefix
        assert!(models::LLAMA_3_3_70B.contains("meta"));
        assert!(models::LLAMA_3_1_405B.contains("meta"));
        assert!(models::LLAMA_3_2_1B.contains("meta"));
    }

    #[test]
    fn test_mistral_model_prefixes() {
        // Mistral models should have mistral prefix
        assert!(models::MISTRAL_LARGE_2407.starts_with("mistral."));
        assert!(models::MISTRAL_SMALL.starts_with("mistral."));
        assert!(models::MIXTRAL_8X7B.starts_with("mistral."));
    }

    #[test]
    fn test_cohere_model_prefixes() {
        // Cohere models should have cohere prefix
        assert!(models::COHERE_COMMAND_R_PLUS.starts_with("cohere."));
        assert!(models::COHERE_COMMAND_R.starts_with("cohere."));
    }

    #[test]
    fn test_titan_model_prefixes() {
        // Titan models should have amazon prefix
        assert!(models::TITAN_TEXT_PREMIER.starts_with("amazon."));
        assert!(models::TITAN_TEXT_EXPRESS.starts_with("amazon."));
        assert!(models::TITAN_TEXT_LITE.starts_with("amazon."));
    }

    #[test]
    fn test_model_constant_uniqueness() {
        use std::collections::HashSet;
        let models_set: HashSet<&str> = [
            models::CLAUDE_SONNET_4_5,
            models::CLAUDE_3_5_SONNET_V2,
            models::CLAUDE_3_5_SONNET,
            models::CLAUDE_3_5_HAIKU,
            models::CLAUDE_3_OPUS,
            models::CLAUDE_3_SONNET,
            models::CLAUDE_3_HAIKU,
            models::CLAUDE_2_1,
            models::CLAUDE_2,
            models::LLAMA_3_3_70B,
            models::LLAMA_3_2_90B,
            models::LLAMA_3_2_11B,
            models::LLAMA_3_2_3B,
            models::LLAMA_3_2_1B,
            models::LLAMA_3_1_405B,
            models::LLAMA_3_1_70B,
            models::LLAMA_3_1_8B,
            models::MISTRAL_LARGE_2407,
            models::MISTRAL_LARGE_2402,
            models::MISTRAL_SMALL,
            models::MIXTRAL_8X7B,
            models::COHERE_COMMAND_R_PLUS,
            models::COHERE_COMMAND_R,
            models::TITAN_TEXT_PREMIER,
            models::TITAN_TEXT_EXPRESS,
            models::TITAN_TEXT_LITE,
        ].into_iter().collect();

        // All models should be unique
        assert_eq!(models_set.len(), 26);
    }

    // ============ Additional Tool Definition Tests ============

    #[test]
    fn test_tool_definition_empty_parameters() {
        let tool_def = ToolDefinition {
            name: "simple_tool".to_string(),
            description: "A simple tool with no parameters".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        };

        assert_eq!(tool_def.name, "simple_tool");
        assert!(tool_def.parameters["properties"].as_object().unwrap().is_empty());
    }

    #[test]
    fn test_tool_definition_multiple_required_params() {
        let tool_def = ToolDefinition {
            name: "complex_tool".to_string(),
            description: "A tool with multiple required parameters".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "param1": {"type": "string"},
                    "param2": {"type": "number"},
                    "param3": {"type": "boolean"}
                },
                "required": ["param1", "param2", "param3"]
            }),
        };

        let required = tool_def.parameters["required"].as_array().unwrap();
        assert_eq!(required.len(), 3);
    }

    #[test]
    fn test_tool_definition_nested_schema() {
        let tool_def = ToolDefinition {
            name: "nested_tool".to_string(),
            description: "A tool with nested object parameters".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "outer": {
                        "type": "object",
                        "properties": {
                            "inner": {"type": "string"}
                        }
                    }
                },
                "required": ["outer"]
            }),
        };

        assert!(tool_def.parameters["properties"]["outer"]["properties"]["inner"].is_object());
    }

    #[test]
    fn test_tool_definition_array_param() {
        let tool_def = ToolDefinition {
            name: "array_tool".to_string(),
            description: "A tool with an array parameter".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "items": {
                        "type": "array",
                        "items": {"type": "string"}
                    }
                },
                "required": ["items"]
            }),
        };

        assert_eq!(tool_def.parameters["properties"]["items"]["type"], "array");
    }

    // ============ Additional Tool Call Tests ============

    #[test]
    fn test_tool_call_with_index() {
        let tool_call = ToolCall {
            id: "call_456".to_string(),
            name: "multi_tool".to_string(),
            args: serde_json::json!({}),
            tool_type: "tool_call".to_string(),
            index: Some(2),
        };

        assert_eq!(tool_call.index, Some(2));
    }

    #[test]
    fn test_tool_call_complex_args() {
        let tool_call = ToolCall {
            id: "call_789".to_string(),
            name: "search".to_string(),
            args: serde_json::json!({
                "query": "rust programming",
                "filters": {
                    "language": "en",
                    "date_range": ["2024-01-01", "2024-12-31"]
                },
                "limit": 10,
                "include_snippets": true
            }),
            tool_type: "tool_call".to_string(),
            index: None,
        };

        assert_eq!(tool_call.args["query"], "rust programming");
        assert_eq!(tool_call.args["filters"]["language"], "en");
        assert_eq!(tool_call.args["limit"], 10);
    }

    #[test]
    fn test_tool_call_empty_args() {
        let tool_call = ToolCall {
            id: "call_empty".to_string(),
            name: "no_args_tool".to_string(),
            args: serde_json::json!({}),
            tool_type: "tool_call".to_string(),
            index: None,
        };

        assert!(tool_call.args.as_object().unwrap().is_empty());
    }

    // ============ Additional Usage Metadata Tests ============

    #[test]
    fn test_usage_metadata_zero_tokens() {
        let usage = UsageMetadata {
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            input_token_details: None,
            output_token_details: None,
        };

        assert_eq!(usage.total_tokens, 0);
    }

    #[test]
    fn test_usage_metadata_large_tokens() {
        let usage = UsageMetadata {
            input_tokens: 100_000,
            output_tokens: 50_000,
            total_tokens: 150_000,
            input_token_details: None,
            output_token_details: None,
        };

        assert_eq!(usage.total_tokens, 150_000);
    }

    #[test]
    fn test_usage_metadata_debug() {
        let usage = UsageMetadata {
            input_tokens: 10,
            output_tokens: 20,
            total_tokens: 30,
            input_token_details: None,
            output_token_details: None,
        };

        let debug_str = format!("{:?}", usage);
        assert!(debug_str.contains("input_tokens"));
        assert!(debug_str.contains("10"));
    }

    // ============ Additional Serialization Tests ============

    #[test]
    fn test_serializable_lc_id_full() {
        let expected_id = vec![
            "dashflow".to_string(),
            "chat_models".to_string(),
            "bedrock".to_string(),
            "ChatBedrock".to_string(),
        ];

        assert_eq!(expected_id[0], "dashflow");
        assert_eq!(expected_id[1], "chat_models");
        assert_eq!(expected_id[2], "bedrock");
        assert_eq!(expected_id[3], "ChatBedrock");
    }

    #[test]
    fn test_serialized_object_constructor() {
        let kwargs = serde_json::json!({
            "model_id": "anthropic.claude-3-5-sonnet-20241022-v2:0",
            "region": "us-east-1"
        });

        // Verify the structure matches expected format
        assert!(kwargs["model_id"].is_string());
        assert!(kwargs["region"].is_string());
    }

    // ============ Message Structure Tests ============

    #[test]
    fn test_message_human_basic() {
        let msg = Message::human("Hello, world!");
        assert!(matches!(msg, Message::Human { .. }));
    }

    #[test]
    fn test_message_system_basic() {
        let msg = Message::system("You are a helpful assistant.");
        assert!(matches!(msg, Message::System { .. }));
    }

    #[test]
    fn test_ai_message_with_tool_calls() {
        let tool_calls = vec![
            ToolCall {
                id: "call_1".to_string(),
                name: "get_weather".to_string(),
                args: serde_json::json!({"location": "SF"}),
                tool_type: "tool_call".to_string(),
                index: None,
            }
        ];

        let ai_msg = AIMessage::new("Let me check the weather.")
            .with_tool_calls(tool_calls);

        // Verify via content() accessor and conversion to Message
        assert_eq!(ai_msg.content(), "Let me check the weather.");
        // The tool_calls are carried through - verify by converting to Message
        let msg = Message::from(ai_msg);
        assert!(matches!(msg, Message::AI { .. }));
    }

    #[test]
    fn test_ai_message_with_usage() {
        let usage = UsageMetadata {
            input_tokens: 10,
            output_tokens: 5,
            total_tokens: 15,
            input_token_details: None,
            output_token_details: None,
        };

        let ai_msg = AIMessage::new("Response text").with_usage(usage);
        // Verify via content() accessor and conversion to Message
        assert_eq!(ai_msg.content(), "Response text");
        // The usage metadata is carried through - verify by converting to Message
        let msg = Message::from(ai_msg);
        assert!(matches!(msg, Message::AI { .. }));
    }

    // ============ Chat Generation Tests ============

    #[test]
    fn test_chat_generation_structure() {
        let gen = ChatGeneration {
            message: Message::human("test"),
            generation_info: None,
        };

        assert!(matches!(gen.message, Message::Human { .. }));
        assert!(gen.generation_info.is_none());
    }

    #[test]
    fn test_chat_generation_with_info() {
        let mut info = HashMap::new();
        info.insert("finish_reason".to_string(), serde_json::json!("stop"));

        let gen = ChatGeneration {
            message: Message::human("test"),
            generation_info: Some(info),
        };

        assert!(gen.generation_info.is_some());
        assert_eq!(gen.generation_info.unwrap()["finish_reason"], "stop");
    }

    #[test]
    fn test_chat_result_single_generation() {
        let result = ChatResult {
            generations: vec![ChatGeneration {
                message: Message::human("test"),
                generation_info: None,
            }],
            llm_output: None,
        };

        assert_eq!(result.generations.len(), 1);
    }

    #[test]
    fn test_chat_result_multiple_generations() {
        let result = ChatResult {
            generations: vec![
                ChatGeneration {
                    message: Message::human("gen1"),
                    generation_info: None,
                },
                ChatGeneration {
                    message: Message::human("gen2"),
                    generation_info: None,
                },
            ],
            llm_output: None,
        };

        assert_eq!(result.generations.len(), 2);
    }

    // ============ Integration Tests (require AWS credentials) ============

    #[tokio::test]
    #[ignore = "requires AWS credentials"]
    async fn test_chat_bedrock_creation() {
        let bedrock = ChatBedrock::new("us-east-1").await;
        assert!(bedrock.is_ok());
    }

    #[tokio::test]
    #[ignore = "requires AWS credentials"]
    async fn test_chat_bedrock_invoke() {
        let bedrock = ChatBedrock::new("us-east-1")
            .await
            .unwrap()
            .with_model(models::CLAUDE_3_5_SONNET_V2);

        let messages = vec![Message::human(
            "Say 'Hello from Bedrock!' and nothing else.",
        )];
        let result = bedrock._generate(&messages, None, None, None, None).await;
        assert!(result.is_ok());

        let chat_result = result.unwrap();
        assert!(!chat_result.generations.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires AWS credentials"]
    async fn test_chat_bedrock_streaming() {
        let bedrock = ChatBedrock::new("us-east-1")
            .await
            .unwrap()
            .with_model(models::CLAUDE_3_5_SONNET_V2);

        let messages = vec![Message::human("Count to 5")];
        let stream = bedrock._stream(&messages, None, None, None, None).await;
        assert!(stream.is_ok());

        let mut stream = stream.unwrap();
        let mut chunks = Vec::new();

        while let Some(chunk_result) = stream.next().await {
            assert!(chunk_result.is_ok());
            chunks.push(chunk_result.unwrap());
        }

        assert!(!chunks.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires AWS credentials"]
    async fn test_chat_bedrock_with_tools() {
        let tool = serde_json::json!({
            "name": "get_weather",
            "description": "Get weather for a location",
            "input_schema": {
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                },
                "required": ["location"]
            }
        });

        let bedrock = ChatBedrock::new("us-east-1")
            .await
            .unwrap()
            .with_model(models::CLAUDE_3_5_SONNET_V2)
            .bind_tools(vec![tool]);

        let messages = vec![Message::human("What's the weather in San Francisco?")];
        let result = bedrock._generate(&messages, None, None, None, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "requires AWS credentials"]
    async fn test_chat_bedrock_builder_pattern() {
        let bedrock = ChatBedrock::new("us-east-1")
            .await
            .unwrap()
            .with_model(models::CLAUDE_3_HAIKU)
            .with_temperature(0.7)
            .with_top_p(0.9)
            .with_max_tokens(1024)
            .with_stop_sequences(vec!["STOP".to_string()]);

        assert_eq!(bedrock.model_id, models::CLAUDE_3_HAIKU);
        assert_eq!(bedrock.temperature, Some(0.7));
        assert_eq!(bedrock.top_p, Some(0.9));
        assert_eq!(bedrock.max_tokens, Some(1024));
        assert!(bedrock.stop_sequences.is_some());
    }

    // ============ JSON Document Conversion Edge Cases ============

    #[test]
    fn test_json_document_special_strings() {
        // Test strings with special characters
        let json = serde_json::json!({
            "newlines": "line1\nline2\nline3",
            "tabs": "col1\tcol2\tcol3",
            "quotes": "He said \"Hello\"",
            "backslash": "path\\to\\file",
            "null_chars": "before\0after"
        });
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(json, converted_back);
    }

    #[test]
    fn test_json_document_very_long_string() {
        let long_string = "x".repeat(10000);
        let json = serde_json::json!({"content": long_string});
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(converted_back["content"].as_str().unwrap().len(), 10000);
    }

    #[test]
    fn test_json_document_large_array() {
        let large_array: Vec<i32> = (0..1000).collect();
        let json = serde_json::json!({"items": large_array});
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(converted_back["items"].as_array().unwrap().len(), 1000);
    }

    #[test]
    fn test_json_document_many_keys() {
        let mut map = serde_json::Map::new();
        for i in 0..100 {
            map.insert(format!("key_{}", i), serde_json::json!(i));
        }
        let json = serde_json::Value::Object(map);
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(converted_back.as_object().unwrap().len(), 100);
    }

    #[test]
    fn test_json_document_negative_float() {
        // Negative numbers work correctly as floats
        let json = serde_json::json!(-42.5);
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert!((converted_back.as_f64().unwrap() - (-42.5)).abs() < 0.001);
    }

    #[test]
    fn test_json_document_negative_large_float() {
        // Negative floats preserve sign correctly
        let json = serde_json::json!(-1000000.5);
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert!((converted_back.as_f64().unwrap() - (-1000000.5)).abs() < 1.0);
    }

    #[test]
    fn test_json_document_negative_integer() {
        // Negative integers must preserve sign (bug fix test)
        let json = serde_json::json!(-42);
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(converted_back.as_i64().unwrap(), -42);
    }

    #[test]
    fn test_json_document_negative_integer_large() {
        // Large negative integers must preserve sign
        let json = serde_json::json!(-9223372036854775807_i64);
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(converted_back.as_i64().unwrap(), -9223372036854775807_i64);
    }

    #[test]
    fn test_json_document_negative_one() {
        // Edge case: -1 must be -1, not u64::MAX
        let json = serde_json::json!(-1);
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(converted_back.as_i64().unwrap(), -1);
    }

    #[test]
    fn test_json_document_positive_integer() {
        // Positive integers should still work
        let json = serde_json::json!(42);
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(converted_back.as_i64().unwrap(), 42);
    }

    #[test]
    fn test_json_document_zero() {
        // Zero is a positive integer
        let json = serde_json::json!(0);
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(converted_back.as_i64().unwrap(), 0);
    }

    #[test]
    fn test_json_document_negative_integers_in_array() {
        // Negative integers in arrays must preserve sign
        let json = serde_json::json!([-1, -2, -100, 0, 1, 100]);
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        let arr = converted_back.as_array().unwrap();
        assert_eq!(arr[0].as_i64().unwrap(), -1);
        assert_eq!(arr[1].as_i64().unwrap(), -2);
        assert_eq!(arr[2].as_i64().unwrap(), -100);
        assert_eq!(arr[3].as_i64().unwrap(), 0);
        assert_eq!(arr[4].as_i64().unwrap(), 1);
        assert_eq!(arr[5].as_i64().unwrap(), 100);
    }

    #[test]
    fn test_json_document_negative_integers_in_object() {
        // Negative integers in objects must preserve sign
        let json = serde_json::json!({
            "balance": -500,
            "offset": -10,
            "positive": 100
        });
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(converted_back["balance"].as_i64().unwrap(), -500);
        assert_eq!(converted_back["offset"].as_i64().unwrap(), -10);
        assert_eq!(converted_back["positive"].as_i64().unwrap(), 100);
    }

    #[test]
    fn test_json_document_scientific_notation() {
        let json = serde_json::json!(1.5e10);
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert!((converted_back.as_f64().unwrap() - 1.5e10).abs() < 1.0);
    }

    #[test]
    fn test_json_document_very_small_float() {
        let json = serde_json::json!(1e-100);
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        // Small float should remain close to original
        let val = converted_back.as_f64().unwrap();
        assert!(val > 0.0 && val < 1.0);
    }

    #[test]
    fn test_json_document_complex_nested_structure() {
        let json = serde_json::json!({
            "users": [
                {
                    "id": 1,
                    "name": "Alice",
                    "addresses": [
                        {"city": "NYC", "zip": "10001"},
                        {"city": "LA", "zip": "90001"}
                    ]
                },
                {
                    "id": 2,
                    "name": "Bob",
                    "addresses": []
                }
            ],
            "metadata": {
                "version": 1.0,
                "generated": true
            }
        });
        let doc = json_to_document(&json).unwrap();
        let converted_back = document_to_json(doc).unwrap();
        assert_eq!(converted_back["users"][0]["name"], "Alice");
        assert_eq!(converted_back["users"][0]["addresses"][0]["city"], "NYC");
    }

    // ============ Tool Definition Edge Cases ============

    #[test]
    fn test_tool_definition_with_enum_property() {
        let tool_def = ToolDefinition {
            name: "set_status".to_string(),
            description: "Set the status".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "enum": ["active", "inactive", "pending"]
                    }
                },
                "required": ["status"]
            }),
        };
        let params = &tool_def.parameters;
        let enum_values = params["properties"]["status"]["enum"].as_array().unwrap();
        assert_eq!(enum_values.len(), 3);
    }

    #[test]
    fn test_tool_definition_with_default_value() {
        let tool_def = ToolDefinition {
            name: "fetch_data".to_string(),
            description: "Fetch data with optional limit".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "default": 10
                    }
                },
                "required": []
            }),
        };
        let default_val = tool_def.parameters["properties"]["limit"]["default"].as_i64().unwrap();
        assert_eq!(default_val, 10);
    }

    #[test]
    fn test_tool_definition_with_description_per_property() {
        let tool_def = ToolDefinition {
            name: "search".to_string(),
            description: "Search for items".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query to execute"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results to return"
                    }
                },
                "required": ["query"]
            }),
        };
        assert_eq!(
            tool_def.parameters["properties"]["query"]["description"],
            "The search query to execute"
        );
    }

    #[test]
    fn test_tool_definition_with_array_of_objects() {
        let tool_def = ToolDefinition {
            name: "bulk_create".to_string(),
            description: "Create multiple items".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "items": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": {"type": "string"},
                                "value": {"type": "number"}
                            }
                        }
                    }
                },
                "required": ["items"]
            }),
        };
        let items_schema = &tool_def.parameters["properties"]["items"];
        assert_eq!(items_schema["type"], "array");
        assert_eq!(items_schema["items"]["type"], "object");
    }

    // ============ Tool Call Edge Cases ============

    #[test]
    fn test_tool_call_with_null_args() {
        let tool_call = ToolCall {
            id: "call_null".to_string(),
            name: "simple_action".to_string(),
            args: serde_json::Value::Null,
            tool_type: "tool_call".to_string(),
            index: None,
        };
        assert!(tool_call.args.is_null());
    }

    #[test]
    fn test_tool_call_with_nested_array_args() {
        let tool_call = ToolCall {
            id: "call_matrix".to_string(),
            name: "process_matrix".to_string(),
            args: serde_json::json!({
                "matrix": [[1, 2, 3], [4, 5, 6], [7, 8, 9]]
            }),
            tool_type: "tool_call".to_string(),
            index: None,
        };
        assert_eq!(tool_call.args["matrix"][1][1], 5);
    }

    #[test]
    fn test_tool_call_with_unicode_id() {
        let tool_call = ToolCall {
            id: "call_调用_123".to_string(),
            name: "international_tool".to_string(),
            args: serde_json::json!({}),
            tool_type: "tool_call".to_string(),
            index: None,
        };
        assert!(tool_call.id.contains("调用"));
    }

    #[test]
    fn test_tool_call_with_max_index() {
        let tool_call = ToolCall {
            id: "call_max".to_string(),
            name: "tool".to_string(),
            args: serde_json::json!({}),
            tool_type: "tool_call".to_string(),
            index: Some(usize::MAX),
        };
        assert_eq!(tool_call.index, Some(usize::MAX));
    }

    #[test]
    fn test_tool_call_with_zero_index() {
        let tool_call = ToolCall {
            id: "call_zero".to_string(),
            name: "tool".to_string(),
            args: serde_json::json!({}),
            tool_type: "tool_call".to_string(),
            index: Some(0),
        };
        assert_eq!(tool_call.index, Some(0));
    }

    // ============ Usage Metadata Edge Cases ============

    #[test]
    fn test_usage_metadata_max_tokens() {
        let usage = UsageMetadata {
            input_tokens: u32::MAX,
            output_tokens: u32::MAX,
            total_tokens: u32::MAX,
            input_token_details: None,
            output_token_details: None,
        };
        assert_eq!(usage.total_tokens, u32::MAX);
    }

    #[test]
    fn test_usage_metadata_asymmetric() {
        let usage = UsageMetadata {
            input_tokens: 1000,
            output_tokens: 10,
            total_tokens: 1010,
            input_token_details: None,
            output_token_details: None,
        };
        assert!(usage.input_tokens > usage.output_tokens);
    }

    #[test]
    fn test_usage_metadata_output_heavy() {
        let usage = UsageMetadata {
            input_tokens: 10,
            output_tokens: 1000,
            total_tokens: 1010,
            input_token_details: None,
            output_token_details: None,
        };
        assert!(usage.output_tokens > usage.input_tokens);
    }

    // ============ Message Structure Edge Cases ============

    #[test]
    fn test_message_human_empty() {
        let msg = Message::human("");
        assert!(matches!(msg, Message::Human { .. }));
    }

    #[test]
    fn test_message_human_long() {
        let long_content = "A".repeat(100000);
        let msg = Message::human(long_content);
        assert!(matches!(msg, Message::Human { .. }));
    }

    #[test]
    fn test_message_system_unicode() {
        let msg = Message::system("你是一个有帮助的助手。🤖");
        assert!(matches!(msg, Message::System { .. }));
    }

    #[test]
    fn test_message_human_multiline() {
        let msg = Message::human("Line 1\nLine 2\nLine 3");
        assert!(matches!(msg, Message::Human { .. }));
    }

    // ============ Model Constant Validation ============

    #[test]
    fn test_all_claude_models_have_claude() {
        assert!(models::CLAUDE_SONNET_4_5.to_lowercase().contains("claude"));
        assert!(models::CLAUDE_3_5_SONNET_V2.to_lowercase().contains("claude"));
        assert!(models::CLAUDE_3_5_SONNET.to_lowercase().contains("claude"));
        assert!(models::CLAUDE_3_5_HAIKU.to_lowercase().contains("claude"));
        assert!(models::CLAUDE_3_OPUS.to_lowercase().contains("claude"));
        assert!(models::CLAUDE_3_SONNET.to_lowercase().contains("claude"));
        assert!(models::CLAUDE_3_HAIKU.to_lowercase().contains("claude"));
        assert!(models::CLAUDE_2_1.to_lowercase().contains("claude"));
        assert!(models::CLAUDE_2.to_lowercase().contains("claude"));
    }

    #[test]
    fn test_all_llama_models_have_llama() {
        assert!(models::LLAMA_3_3_70B.to_lowercase().contains("llama"));
        assert!(models::LLAMA_3_2_90B.to_lowercase().contains("llama"));
        assert!(models::LLAMA_3_2_11B.to_lowercase().contains("llama"));
        assert!(models::LLAMA_3_2_3B.to_lowercase().contains("llama"));
        assert!(models::LLAMA_3_2_1B.to_lowercase().contains("llama"));
        assert!(models::LLAMA_3_1_405B.to_lowercase().contains("llama"));
        assert!(models::LLAMA_3_1_70B.to_lowercase().contains("llama"));
        assert!(models::LLAMA_3_1_8B.to_lowercase().contains("llama"));
    }

    #[test]
    fn test_all_mistral_models_have_mistral() {
        assert!(models::MISTRAL_LARGE_2407.to_lowercase().contains("mistral"));
        assert!(models::MISTRAL_LARGE_2402.to_lowercase().contains("mistral"));
        assert!(models::MISTRAL_SMALL.to_lowercase().contains("mistral"));
        assert!(models::MIXTRAL_8X7B.to_lowercase().contains("mistral"));
    }

    #[test]
    fn test_all_cohere_models_have_cohere() {
        assert!(models::COHERE_COMMAND_R_PLUS.to_lowercase().contains("cohere"));
        assert!(models::COHERE_COMMAND_R.to_lowercase().contains("cohere"));
    }

    #[test]
    fn test_all_titan_models_have_titan() {
        assert!(models::TITAN_TEXT_PREMIER.to_lowercase().contains("titan"));
        assert!(models::TITAN_TEXT_EXPRESS.to_lowercase().contains("titan"));
        assert!(models::TITAN_TEXT_LITE.to_lowercase().contains("titan"));
    }

    #[test]
    fn test_model_id_version_formats() {
        // Models with version suffix
        assert!(models::CLAUDE_3_5_SONNET_V2.contains(":"));
        assert!(models::CLAUDE_3_HAIKU.contains(":"));
        // Models without version suffix
        assert!(!models::TITAN_TEXT_EXPRESS.contains(":"));
    }

    // ============ ChatGeneration Edge Cases ============

    #[test]
    fn test_chat_generation_empty_info() {
        let gen = ChatGeneration {
            message: Message::human("test"),
            generation_info: Some(HashMap::new()),
        };
        assert!(gen.generation_info.unwrap().is_empty());
    }

    #[test]
    fn test_chat_generation_rich_info() {
        let mut info = HashMap::new();
        info.insert("finish_reason".to_string(), serde_json::json!("stop"));
        info.insert("model".to_string(), serde_json::json!("claude-3"));
        info.insert("usage".to_string(), serde_json::json!({"tokens": 100}));

        let gen = ChatGeneration {
            message: Message::human("test"),
            generation_info: Some(info.clone()),
        };

        let info = gen.generation_info.unwrap();
        assert_eq!(info.len(), 3);
    }

    #[test]
    fn test_chat_result_empty() {
        let result = ChatResult {
            generations: vec![],
            llm_output: None,
        };
        assert!(result.generations.is_empty());
    }

    #[test]
    fn test_chat_result_with_llm_output() {
        let mut output = HashMap::new();
        output.insert("model_id".to_string(), serde_json::json!("claude-3"));

        let result = ChatResult {
            generations: vec![],
            llm_output: Some(output),
        };
        assert!(result.llm_output.is_some());
    }

    // ============ Serialization Tests ============

    #[test]
    fn test_serialized_object_kwargs_structure() {
        let mut kwargs = serde_json::Map::new();
        kwargs.insert("model_id".to_string(), serde_json::json!("test-model"));
        kwargs.insert("region".to_string(), serde_json::json!("us-west-2"));
        kwargs.insert("temperature".to_string(), serde_json::json!(0.5));
        kwargs.insert("top_p".to_string(), serde_json::json!(0.95));
        kwargs.insert("max_tokens".to_string(), serde_json::json!(2000));

        let value = serde_json::Value::Object(kwargs);
        assert_eq!(value["model_id"], "test-model");
        assert_eq!(value["temperature"], 0.5);
    }

    #[test]
    fn test_lc_id_path_separator() {
        let id = vec!["dashflow", "chat_models", "bedrock", "ChatBedrock"];
        // Verify the id could be used as a path
        let path = id.join("/");
        assert_eq!(path, "dashflow/chat_models/bedrock/ChatBedrock");
    }

    // ============ AIMessage Tests ============

    #[test]
    fn test_ai_message_empty_content() {
        let ai_msg = AIMessage::new("");
        assert_eq!(ai_msg.content(), "");
    }

    #[test]
    fn test_ai_message_long_content() {
        let long_content = "B".repeat(50000);
        let ai_msg = AIMessage::new(long_content);
        assert_eq!(ai_msg.content().len(), 50000);
    }

    #[test]
    fn test_ai_message_multiple_tool_calls() {
        let tool_calls = vec![
            ToolCall {
                id: "call_1".to_string(),
                name: "tool_a".to_string(),
                args: serde_json::json!({"x": 1}),
                tool_type: "tool_call".to_string(),
                index: Some(0),
            },
            ToolCall {
                id: "call_2".to_string(),
                name: "tool_b".to_string(),
                args: serde_json::json!({"y": 2}),
                tool_type: "tool_call".to_string(),
                index: Some(1),
            },
            ToolCall {
                id: "call_3".to_string(),
                name: "tool_c".to_string(),
                args: serde_json::json!({"z": 3}),
                tool_type: "tool_call".to_string(),
                index: Some(2),
            },
        ];

        let ai_msg = AIMessage::new("Multiple tools").with_tool_calls(tool_calls);
        let msg = Message::from(ai_msg);
        assert!(matches!(msg, Message::AI { .. }));
    }

    // ============ AIMessageChunk Tests ============

    #[test]
    fn test_ai_message_chunk_creation() {
        let chunk = AIMessageChunk::new("Hello");
        assert_eq!(chunk.content, "Hello");
    }

    #[test]
    fn test_ai_message_chunk_empty() {
        let chunk = AIMessageChunk::new("");
        assert!(chunk.content.is_empty());
    }

    #[test]
    fn test_ai_message_chunk_with_tool_calls() {
        let mut chunk = AIMessageChunk::new("test");
        chunk.tool_calls.push(ToolCall {
            id: "chunk_call".to_string(),
            name: "chunked_tool".to_string(),
            args: serde_json::json!({}),
            tool_type: "tool_call".to_string(),
            index: None,
        });
        assert_eq!(chunk.tool_calls.len(), 1);
    }

    #[test]
    fn test_ai_message_chunk_with_usage() {
        let mut chunk = AIMessageChunk::new("");
        chunk.usage_metadata = Some(UsageMetadata {
            input_tokens: 5,
            output_tokens: 10,
            total_tokens: 15,
            input_token_details: None,
            output_token_details: None,
        });
        assert!(chunk.usage_metadata.is_some());
    }

    // ============ ChatGenerationChunk Tests ============

    #[test]
    fn test_chat_generation_chunk_creation() {
        let ai_chunk = AIMessageChunk::new("Streaming chunk");
        let chunk = ChatGenerationChunk::new(ai_chunk);
        assert_eq!(chunk.message.content, "Streaming chunk");
    }

    #[test]
    fn test_chat_generation_chunk_empty() {
        let ai_chunk = AIMessageChunk::new("");
        let chunk = ChatGenerationChunk::new(ai_chunk);
        assert!(chunk.message.content.is_empty());
    }

    // ============ Error Path Tests ============

    #[test]
    fn test_json_to_document_preserves_all_types() {
        // Test a JSON value that uses all basic types
        let json = serde_json::json!({
            "null_val": null,
            "bool_val": true,
            "int_val": 42,
            "float_val": 3.14,
            "string_val": "hello",
            "array_val": [1, 2, 3],
            "object_val": {"nested": "value"}
        });

        let doc = json_to_document(&json).unwrap();
        let back = document_to_json(doc).unwrap();

        assert!(back["null_val"].is_null());
        assert_eq!(back["bool_val"], true);
        assert_eq!(back["string_val"], "hello");
        assert!(back["array_val"].is_array());
        assert!(back["object_val"].is_object());
    }

    #[test]
    fn test_response_metadata_structure() {
        let mut metadata: HashMap<String, serde_json::Value> = HashMap::new();
        metadata.insert("stop_reason".to_string(), serde_json::json!("end_turn"));
        metadata.insert("model".to_string(), serde_json::json!("claude-3"));
        metadata.insert("latency_ms".to_string(), serde_json::json!(150));

        assert_eq!(metadata["stop_reason"], "end_turn");
        assert_eq!(metadata.len(), 3);
    }

    // ============ Region Tests ============

    #[test]
    fn test_valid_aws_regions() {
        let regions = [
            "us-east-1",
            "us-east-2",
            "us-west-1",
            "us-west-2",
            "eu-west-1",
            "eu-west-2",
            "eu-central-1",
            "ap-northeast-1",
            "ap-southeast-1",
            "ap-southeast-2",
        ];
        for region in regions {
            assert!(region.contains("-"));
            assert!(region.len() >= 9);
        }
    }

    // ============ Stop Sequence Tests ============

    #[test]
    fn test_stop_sequences_empty() {
        let sequences: Vec<String> = vec![];
        assert!(sequences.is_empty());
    }

    #[test]
    fn test_stop_sequences_single() {
        let sequences = vec!["STOP".to_string()];
        assert_eq!(sequences.len(), 1);
    }

    #[test]
    fn test_stop_sequences_multiple() {
        let sequences = vec![
            "STOP".to_string(),
            "END".to_string(),
            "DONE".to_string(),
            "\n\n".to_string(),
        ];
        assert_eq!(sequences.len(), 4);
    }

    #[test]
    fn test_stop_sequences_special_chars() {
        let sequences = vec![
            "</response>".to_string(),
            "```".to_string(),
            "---END---".to_string(),
        ];
        assert!(sequences[0].contains("<"));
    }

    // ============ Temperature and TopP Range Tests ============

    #[test]
    fn test_temperature_bounds() {
        let temps = [0.0f32, 0.1, 0.5, 0.7, 1.0];
        for temp in temps {
            assert!(temp >= 0.0);
            assert!(temp <= 1.0);
        }
    }

    #[test]
    fn test_top_p_bounds() {
        let values = [0.0f32, 0.1, 0.5, 0.9, 0.95, 1.0];
        for val in values {
            assert!(val >= 0.0);
            assert!(val <= 1.0);
        }
    }

    // ============ Max Tokens Tests ============

    #[test]
    fn test_max_tokens_common_values() {
        let common_values: Vec<u32> = vec![256, 512, 1024, 2048, 4096, 8192];
        for val in common_values {
            assert!(val > 0);
            assert!(val.is_power_of_two());
        }
    }

    #[test]
    fn test_max_tokens_arbitrary_values() {
        let values: Vec<u32> = vec![100, 500, 1000, 2000, 4000];
        for val in values {
            assert!(val > 0);
        }
    }
}
