// Mistral AI chat model implementation

use async_stream::stream;
use async_trait::async_trait;
use dashflow::core::{
    callbacks::CallbackManager,
    error::{Error, Result},
    language_models::{
        ChatGeneration, ChatGenerationChunk, ChatModel, ChatResult, ToolChoice, ToolDefinition,
    },
    messages::{AIMessageChunk, BaseMessage, Message, ToolCall},
    rate_limiters::RateLimiter,
    retry::{with_retry, RetryPolicy},
    usage::UsageMetadata,
};
use futures::Stream;
use futures::StreamExt;
use mistralai_client::v1::{
    chat::{ChatMessage as MistralChatMessage, ChatMessageRole, ChatParams},
    client::Client as MistralClient,
    common::ResponseUsage,
    constants::Model as MistralModel,
    tool::{
        Tool as MistralTool, ToolCall as MistralToolCall, ToolCallFunction,
        ToolChoice as MistralToolChoice, ToolFunctionParameter, ToolFunctionParameterType,
    },
};
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;

/// Mistral AI chat model configuration and client
///
/// Mistral AI provides powerful open-source and proprietary language models
/// optimized for various tasks including chat, code generation, and reasoning.
///
/// # Example
/// ```no_run
/// use dashflow_mistral::ChatMistralAI;
/// use dashflow::core::language_models::ChatModel;
/// use dashflow::core::messages::Message;
///
/// #[tokio::main]
/// async fn main() {
///     let model = ChatMistralAI::new()
///         .with_model("mistral-small-latest")
///         .with_temperature(0.7);
///
///     let messages = vec![Message::human("Hello!")];
///     let result = model.generate(&messages, None, None, None, None).await.unwrap();
///     println!("{:?}", result);
/// }
/// ```
///
/// # Available Models
/// - `mistral-small-latest` (default) - Fast and efficient for simple tasks
/// - `mistral-medium-latest` - Balanced performance and capability
/// - `mistral-large-latest` - Most capable model for complex tasks
/// - `codestral-latest` - Specialized for code generation
/// - `open-mistral-7b` - Open-source 7B parameter model
/// - `open-mixtral-8x7b` - Open-source mixture of experts model
/// - `open-mixtral-8x22b` - Larger mixture of experts model
#[derive(Clone, Debug)]
pub struct ChatMistralAI {
    /// Mistral client
    client: Arc<MistralClient>,

    /// Model name (e.g., "mistral-small-latest", "mistral-large-latest")
    model: String,

    /// Sampling temperature (0.0 to 1.0)
    temperature: Option<f32>,

    /// Maximum tokens to generate
    max_tokens: Option<u32>,

    /// Top-p sampling parameter
    top_p: Option<f32>,

    /// Random seed for reproducibility
    random_seed: Option<i32>,

    /// Safe mode flag (filters unsafe content)
    safe_mode: Option<bool>,

    /// Retry policy for API calls
    retry_policy: RetryPolicy,

    /// Optional rate limiter to control request rate
    rate_limiter: Option<Arc<dyn RateLimiter>>,

    /// Optional tools available for the model to call
    tools: Option<Vec<Value>>,

    /// Tool choice strategy ("auto", "none", or specific tool name)
    tool_choice: Option<String>,
}

impl ChatMistralAI {
    /// Create a new `ChatMistralAI` instance with default settings
    ///
    /// Reads the Mistral API key from the `MISTRAL_API_KEY` environment variable.
    ///
    /// # Panics
    /// Panics if the `MISTRAL_API_KEY` environment variable is not set.
    #[deprecated(
        since = "1.0.1",
        note = "Use `dashflow_mistral::build_chat_model(&config)` for config-driven instantiation"
    )]
    #[must_use]
    pub fn new() -> Self {
        let client = MistralClient::new(None, None, None, None)
            .expect("Failed to create Mistral client. Ensure MISTRAL_API_KEY is set.");

        Self {
            client: Arc::new(client),
            model: "mistral-small-latest".to_string(),
            temperature: Some(0.7),
            max_tokens: None,
            top_p: Some(1.0),
            random_seed: None,
            safe_mode: None,
            retry_policy: RetryPolicy::default(),
            rate_limiter: None,
            tools: None,
            tool_choice: None,
        }
    }

    /// Create a new `ChatMistralAI` instance with explicit API key
    pub fn with_api_key(api_key: impl Into<String>) -> Result<Self> {
        let api_key = api_key.into();
        let client = MistralClient::new(Some(api_key), None, None, None)
            .map_err(|e| Error::api(format!("Failed to create Mistral client: {e}")))?;

        Ok(Self {
            client: Arc::new(client),
            model: "mistral-small-latest".to_string(),
            temperature: Some(0.7),
            max_tokens: None,
            top_p: Some(1.0),
            random_seed: None,
            safe_mode: None,
            retry_policy: RetryPolicy::default(),
            rate_limiter: None,
            tools: None,
            tool_choice: None,
        })
    }

    /// Set the model name
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the sampling temperature (0.0 to 1.0)
    #[must_use]
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set the maximum number of tokens to generate
    #[must_use]
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set the top-p sampling parameter (0.0 to 1.0)
    #[must_use]
    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// Set a random seed for reproducible generation
    #[must_use]
    pub fn with_random_seed(mut self, seed: i32) -> Self {
        self.random_seed = Some(seed);
        self
    }

    /// Enable or disable safe mode (content filtering)
    #[must_use]
    pub fn with_safe_mode(mut self, safe_mode: bool) -> Self {
        self.safe_mode = Some(safe_mode);
        self
    }

    /// Set a custom retry policy
    #[must_use]
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// Set a rate limiter
    #[must_use]
    pub fn with_rate_limiter(mut self, limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Set tools available for the model to call
    ///
    /// Tools should be JSON objects following the `OpenAI` function calling format.
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_mistral::ChatMistralAI;
    /// use serde_json::json;
    ///
    /// let calculator_tool = json!({
    ///     "type": "function",
    ///     "function": {
    ///         "name": "calculator",
    ///         "description": "Performs basic arithmetic",
    ///         "parameters": {
    ///             "type": "object",
    ///             "properties": {
    ///                 "operation": {"type": "string"},
    ///                 "a": {"type": "number"},
    ///                 "b": {"type": "number"}
    ///             },
    ///             "required": ["operation", "a", "b"]
    ///         }
    ///     }
    ///});
    ///
    /// let model = ChatMistralAI::new()
    ///     .with_tools(vec![calculator_tool]);
    /// ```
    #[deprecated(
        since = "1.9.0",
        note = "Use bind_tools() from ChatModelToolBindingExt trait instead. \
                bind_tools() is type-safe and works consistently across all providers. \
                Example: `use dashflow::core::language_models::ChatModelToolBindingExt; \
                model.bind_tools(vec![Arc::new(tool)], None)`"
    )]
    #[must_use]
    pub fn with_tools(mut self, tools: Vec<Value>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Set tool choice strategy
    ///
    /// Options:
    /// - "auto" - Let the model decide when to use tools (default)
    /// - "none" - Disable tool use
    /// - specific tool name - Force use of a specific tool
    #[must_use]
    pub fn with_tool_choice(mut self, tool_choice: impl Into<String>) -> Self {
        self.tool_choice = Some(tool_choice.into());
        self
    }

    /// Create a `ChatMistralAI` instance from a configuration
    ///
    /// This method constructs a `ChatMistralAI` model from a `ChatModelConfig::Mistral` variant,
    /// resolving environment variables for API keys and applying all configuration parameters.
    ///
    /// # Arguments
    ///
    /// * `config` - A reference to a `ChatModelConfig` (must be Mistral variant)
    ///
    /// # Returns
    ///
    /// Returns a `Result<Self>` with the constructed `ChatMistralAI` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The config is not a Mistral variant
    /// - API key environment variable cannot be resolved
    pub fn from_config(
        config: &dashflow::core::config_loader::ChatModelConfig,
    ) -> dashflow::core::error::Result<Self> {
        use dashflow::core::config_loader::ChatModelConfig;

        match config {
            ChatModelConfig::Mistral {
                model,
                api_key,
                temperature,
            } => {
                // Resolve the API key
                let resolved_api_key = api_key.resolve()?;

                // Create the ChatMistralAI instance with the API key
                let mut chat_model = Self::with_api_key(&resolved_api_key)?.with_model(model);

                // Apply optional parameters
                if let Some(temp) = temperature {
                    chat_model = chat_model.with_temperature(*temp);
                }

                Ok(chat_model)
            }
            _ => Err(dashflow::core::error::Error::Configuration(format!(
                "Expected Mistral config, got {} config",
                config.provider()
            ))),
        }
    }

    /// Convert the model string to Mistral SDK's Model enum
    fn get_mistral_model(&self) -> MistralModel {
        // Map common model names to SDK enum
        match self.model.as_str() {
            "open-mistral-7b" => MistralModel::OpenMistral7b,
            "open-mixtral-8x7b" => MistralModel::OpenMixtral8x7b,
            "open-mixtral-8x22b" => MistralModel::OpenMixtral8x22b,
            "open-mistral-nemo" => MistralModel::OpenMistralNemo,
            "mistral-tiny" => MistralModel::MistralTiny,
            "mistral-small-latest" | "mistral-small" => MistralModel::MistralSmallLatest,
            "mistral-medium-latest" | "mistral-medium" => MistralModel::MistralMediumLatest,
            "mistral-large-latest" | "mistral-large" => MistralModel::MistralLargeLatest,
            "codestral-latest" | "codestral" => MistralModel::CodestralLatest,
            "codestral-mamba" => MistralModel::CodestralMamba,
            // Default to MistralSmallLatest for unknown models
            _ => {
                tracing::warn!(
                    model = %self.model,
                    "Unknown model, defaulting to mistral-small-latest"
                );
                MistralModel::MistralSmallLatest
            }
        }
    }

    /// Build `ChatParams` with parameter overrides
    ///
    /// Parameters passed to this method take precedence over struct fields.
    /// This allows agents to pass `tools/tool_choice` without modifying the model struct.
    fn build_chat_params_with_overrides(
        &self,
        _stop: Option<&[String]>,
        tools_override: Option<&[ToolDefinition]>,
        tool_choice_override: Option<&ToolChoice>,
    ) -> Result<ChatParams> {
        // Parameter precedence: method parameter > struct field
        let tools = if let Some(tool_defs) = tools_override {
            // Use parameter tools (convert from ToolDefinition to MistralTool)
            let mistral_tools: Vec<MistralTool> = tool_defs
                .iter()
                .map(convert_tool_definition)
                .collect::<Result<Vec<_>>>()?;
            Some(mistral_tools)
        } else if let Some(json_tools) = &self.tools {
            // Fall back to struct field tools (already in JSON format)
            Some(convert_json_tools_to_mistral(json_tools)?)
        } else {
            None
        };

        // Parameter precedence: method parameter > struct field
        let tool_choice = if let Some(tc) = tool_choice_override {
            // Use parameter tool_choice
            Some(convert_tool_choice(tc))
        } else if let Some(choice_str) = &self.tool_choice {
            // Fall back to struct field tool_choice
            Some(convert_tool_choice_to_mistral(choice_str)?)
        } else {
            None
        };

        // Note: Mistral API doesn't support stop sequences parameter
        // Ignoring _stop parameter as it's not supported by the Mistral API

        Ok(ChatParams {
            temperature: self.temperature.unwrap_or(0.7),
            max_tokens: self.max_tokens,
            top_p: self.top_p.unwrap_or(1.0),
            random_seed: self.random_seed.map(|s| s as u32),
            safe_prompt: self.safe_mode.unwrap_or(false),
            tools,
            tool_choice,
            ..Default::default()
        })
    }

    /// Build ChatParams from configuration
    ///
    /// NOTE: Prefer `build_chat_params_with_overrides` for agent integration.
    /// This method is kept for backward compatibility.
    #[cfg(test)]
    fn build_chat_params(&self) -> Result<ChatParams> {
        // Convert JSON tools to mistralai-client Tool format
        let tools = if let Some(json_tools) = &self.tools {
            Some(convert_json_tools_to_mistral(json_tools)?)
        } else {
            None
        };

        // Convert tool_choice string to MistralToolChoice enum
        let tool_choice = if let Some(choice_str) = &self.tool_choice {
            Some(convert_tool_choice_to_mistral(choice_str)?)
        } else {
            None
        };

        Ok(ChatParams {
            temperature: self.temperature.unwrap_or(0.7),
            max_tokens: self.max_tokens,
            top_p: self.top_p.unwrap_or(1.0),
            random_seed: self.random_seed.map(|s| s as u32),
            safe_prompt: self.safe_mode.unwrap_or(false),
            tools,
            tool_choice,
            ..Default::default()
        })
    }
}

#[allow(deprecated, clippy::disallowed_methods)]
impl Default for ChatMistralAI {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert `ToolChoice` from dashflow::core to mistralai-client `ToolChoice` enum
///
/// Maps dashflow::core's generic `ToolChoice` enum to Mistral-specific format:
/// - Auto → Auto (model decides)
/// - None → None (no tools)
/// - Required → Any (must call at least one tool)
/// - Specific(name) → Not supported by Mistral API, falls back to Auto
fn convert_tool_choice(choice: &ToolChoice) -> MistralToolChoice {
    match choice {
        ToolChoice::Auto => MistralToolChoice::Auto,
        ToolChoice::None => MistralToolChoice::None,
        ToolChoice::Required => MistralToolChoice::Any,
        ToolChoice::Specific(_) => {
            // Mistral doesn't support forcing a specific tool, fall back to Auto
            tracing::warn!("Mistral API doesn't support forcing specific tool, using Auto");
            MistralToolChoice::Auto
        }
    }
}

/// Convert `ToolDefinition` from dashflow::core to mistralai-client Tool format
///
/// Mistral's tool format is similar to `OpenAI`'s:
/// - name: Function name
/// - description: What the function does
/// - parameters: JSON Schema defining the function's parameters
fn convert_tool_definition(tool_def: &ToolDefinition) -> Result<MistralTool> {
    let name = tool_def.name.clone();
    let description = tool_def.description.clone();

    // Extract parameters from JSON Schema
    let parameters_obj = tool_def.parameters.as_object().ok_or_else(|| {
        Error::invalid_input(format!("Tool '{name}' parameters must be a JSON object"))
    })?;

    // Extract properties from the JSON Schema
    let properties = parameters_obj
        .get("properties")
        .and_then(|v| v.as_object())
        .ok_or_else(|| {
            Error::invalid_input(format!(
                "Tool '{name}' missing 'properties' in parameters schema"
            ))
        })?;

    // Convert each property to ToolFunctionParameter
    // Note: mistralai-client library has limited type support
    let tool_params: Vec<ToolFunctionParameter> = properties
        .iter()
        .map(|(param_name, param_def)| {
            let param_desc = param_def
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // For now, only support string type (mistralai-client limitation)
            ToolFunctionParameter::new(
                param_name.clone(),
                param_desc,
                ToolFunctionParameterType::String,
            )
        })
        .collect();

    Ok(MistralTool::new(name, description, tool_params))
}

/// Convert `tool_choice` string to mistralai-client `ToolChoice` enum
fn convert_tool_choice_to_mistral(choice: &str) -> Result<MistralToolChoice> {
    match choice.to_lowercase().as_str() {
        "auto" => Ok(MistralToolChoice::Auto),
        "none" => Ok(MistralToolChoice::None),
        "any" => Ok(MistralToolChoice::Any),
        _ => Err(Error::invalid_input(format!(
            "Invalid tool_choice '{choice}'. Valid options: 'auto', 'none', 'any'"
        ))),
    }
}

/// Convert JSON tool definitions to mistralai-client Tool format
///
/// Expects OpenAI-compatible JSON format:
/// ```json
/// {
///   "type": "function",
///   "function": {
///     "name": "function_name",
///     "description": "Function description",
///     "parameters": {
///       "type": "object",
///       "properties": {
///         "param1": {"type": "string", "description": "Param description"},
///         ...
///       },
///       "required": ["param1", ...]
///     }
///   }
/// }
/// ```
fn convert_json_tools_to_mistral(json_tools: &[Value]) -> Result<Vec<MistralTool>> {
    json_tools
        .iter()
        .enumerate()
        .map(|(idx, tool_json)| {
            // Extract function object
            let function = tool_json.get("function").ok_or_else(|| {
                Error::invalid_input(format!("Tool at index {idx} missing 'function' field"))
            })?;

            // Extract function name
            let name = function
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    Error::invalid_input(format!(
                        "Tool at index {idx} missing 'function.name' field"
                    ))
                })?
                .to_string();

            // Extract function description
            let description = function
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Extract parameters
            let parameters = function.get("parameters").and_then(|v| v.as_object());

            let tool_params = if let Some(params) = parameters {
                // Extract properties
                let properties = params
                    .get("properties")
                    .and_then(|v| v.as_object())
                    .ok_or_else(|| {
                        Error::invalid_input(format!(
                            "Tool '{name}' missing 'parameters.properties' field"
                        ))
                    })?;

                // Convert each property to ToolFunctionParameter
                properties
                    .iter()
                    .map(|(param_name, param_def)| {
                        let param_desc = param_def
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        // For now, only support string type (mistralai-client limitation)
                        // The library only defines ToolFunctionParameterType::String
                        ToolFunctionParameter::new(
                            param_name.clone(),
                            param_desc,
                            ToolFunctionParameterType::String,
                        )
                    })
                    .collect()
            } else {
                Vec::new()
            };

            Ok(MistralTool::new(name, description, tool_params))
        })
        .collect()
}

/// Convert `DashFlow` message to Mistral message format
fn convert_message_to_mistral(message: &BaseMessage) -> Result<MistralChatMessage> {
    match message {
        Message::Human { content, .. } => Ok(MistralChatMessage {
            role: ChatMessageRole::User,
            content: content.as_text().clone(),
            tool_calls: None,
        }),
        Message::AI {
            content,
            tool_calls,
            ..
        } => {
            // Convert DashFlow tool calls to Mistral format
            let mistral_tool_calls = if tool_calls.is_empty() {
                None
            } else {
                Some(
                    tool_calls
                        .iter()
                        .map(|tc| MistralToolCall {
                            function: ToolCallFunction {
                                name: tc.name.clone(),
                                // Mistral expects arguments as a JSON string
                                arguments: serde_json::to_string(&tc.args).unwrap_or_default(),
                            },
                        })
                        .collect(),
                )
            };

            Ok(MistralChatMessage {
                role: ChatMessageRole::Assistant,
                content: content.as_text().clone(),
                tool_calls: mistral_tool_calls,
            })
        }
        Message::System { content, .. } => Ok(MistralChatMessage {
            role: ChatMessageRole::System,
            content: content.as_text().clone(),
            tool_calls: None,
        }),
        Message::Tool { content, .. } => Ok(MistralChatMessage {
            role: ChatMessageRole::Tool,
            content: content.as_text().clone(),
            tool_calls: None,
        }),
        _ => Err(Error::invalid_input(format!(
            "Unsupported message type: {message:?}"
        ))),
    }
}

/// Convert Mistral response message to `DashFlow` `Message::AI`
fn convert_mistral_message_to_message(
    message: &MistralChatMessage,
    usage: Option<&ResponseUsage>,
) -> Result<Message> {
    let content = message.content.clone();

    // Convert usage metadata if available
    let usage_metadata = usage.map(|u| UsageMetadata {
        input_tokens: u.prompt_tokens,
        output_tokens: u.completion_tokens,
        total_tokens: u.total_tokens,
        input_token_details: None,
        output_token_details: None,
    });

    // Convert Mistral tool calls to DashFlow format
    let tool_calls = if let Some(mistral_calls) = &message.tool_calls {
        mistral_calls
            .iter()
            .enumerate()
            .map(|(idx, tc)| {
                // Parse arguments JSON string to Value
                let args = serde_json::from_str(&tc.function.arguments).unwrap_or_else(|e| {
                    // If parsing fails, wrap the raw string in an error object
                    serde_json::json!({
                        "error": format!("Failed to parse arguments: {}", e),
                        "raw": tc.function.arguments
                    })
                });

                ToolCall {
                    // Mistral doesn't provide IDs in tool calls, generate one
                    id: format!("call_{idx}"),
                    name: tc.function.name.clone(),
                    args,
                    tool_type: "tool_call".to_string(),
                    index: None,
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(Message::AI {
        content: content.into(),
        tool_calls,
        invalid_tool_calls: Vec::new(),
        usage_metadata,
        fields: Default::default(),
    })
}

#[async_trait]
impl ChatModel for ChatMistralAI {
    async fn _generate(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        run_manager: Option<&CallbackManager>,
    ) -> Result<ChatResult> {
        // Generate run ID for tracing
        let run_id = Uuid::new_v4();

        // Call on_llm_start callback
        if let Some(manager) = run_manager {
            let serialized = self.identifying_params();
            let prompts: Vec<String> = messages
                .iter()
                .map(dashflow::core::messages::Message::as_text)
                .collect();

            manager
                .on_llm_start(
                    &serialized,
                    &prompts,
                    run_id,
                    None,            // parent_run_id
                    &[],             // tags
                    &HashMap::new(), // metadata
                )
                .await?;
        }

        // Apply rate limiting if configured
        if let Some(limiter) = &self.rate_limiter {
            limiter.acquire().await;
        }

        // Convert messages to Mistral format
        let mistral_messages: Vec<MistralChatMessage> = messages
            .iter()
            .map(convert_message_to_mistral)
            .collect::<Result<Vec<_>>>()?;

        // Build request parameters with tool override support
        // Parameter values take precedence over struct fields
        let params = self.build_chat_params_with_overrides(stop, tools, tool_choice)?;
        let model = self.get_mistral_model();

        // Make API call with retry
        let response_result = with_retry(&self.retry_policy, || async {
            self.client
                .chat(
                    model.clone(),
                    mistral_messages.clone(),
                    Some(params.clone()),
                )
                .map_err(|e| Error::api(format!("Mistral API error: {e}")))
        })
        .await;

        // Handle error callback
        let response = match response_result {
            Ok(resp) => resp,
            Err(e) => {
                if let Some(manager) = run_manager {
                    if let Err(cb_err) = manager.on_llm_error(&e.to_string(), run_id, None).await {
                        tracing::warn!("Failed to send LLM error callback: {}", cb_err);
                    }
                }
                return Err(e);
            }
        };

        // Convert response to ChatResult
        let generations: Vec<ChatGeneration> = response
            .choices
            .iter()
            .map(|choice| {
                let message =
                    convert_mistral_message_to_message(&choice.message, Some(&response.usage))?;

                Ok(ChatGeneration {
                    message,
                    generation_info: Some({
                        let mut info = HashMap::new();
                        info.insert(
                            "finish_reason".to_string(),
                            Value::String(format!("{:?}", choice.finish_reason)),
                        );
                        info
                    }),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let mut llm_output = HashMap::new();
        llm_output.insert(
            "model".to_string(),
            Value::String(format!("{:?}", response.model)),
        );
        llm_output.insert(
            "usage".to_string(),
            serde_json::to_value(&response.usage).unwrap_or_default(),
        );

        let chat_result = ChatResult {
            generations,
            llm_output: Some(llm_output),
        };

        // Call on_llm_end callback
        if let Some(manager) = run_manager {
            let mut outputs = HashMap::new();
            outputs.insert(
                "generations".to_string(),
                serde_json::to_value(&chat_result.generations)?,
            );
            manager.on_llm_end(&outputs, run_id, None).await?;
        }

        Ok(chat_result)
    }

    async fn _stream(
        &self,
        messages: &[BaseMessage],
        stop: Option<&[String]>,
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
        _run_manager: Option<&CallbackManager>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatGenerationChunk>> + Send>>> {
        // Apply rate limiting if configured
        if let Some(limiter) = &self.rate_limiter {
            limiter.acquire().await;
        }

        // Convert messages to Mistral format
        let mistral_messages: Vec<MistralChatMessage> = messages
            .iter()
            .map(convert_message_to_mistral)
            .collect::<Result<Vec<_>>>()?;

        // Build request parameters with tool override support
        // Parameter values take precedence over struct fields
        let params = self.build_chat_params_with_overrides(stop, tools, tool_choice)?;
        let model = self.get_mistral_model();

        // Clone client for use in stream
        let client = Arc::clone(&self.client);

        // Create stream
        let stream = stream! {
            match client.chat_stream(model, mistral_messages, Some(params)).await {
                Ok(mistral_stream) => {
                    let mut pinned_stream = Box::pin(mistral_stream);
                    while let Some(chunk_result) = pinned_stream.next().await {
                        match chunk_result {
                            Ok(chunks) => {
                                for chunk in chunks {
                                    if let Some(choice) = chunk.choices.first() {
                                        let content = &choice.delta.content;
                                        let chunk_msg = AIMessageChunk::new(content.clone());

                                        yield Ok(ChatGenerationChunk {
                                            message: chunk_msg,
                                            generation_info: None,
                                        });
                                    }
                                }
                            }
                            Err(e) => {
                                yield Err(Error::api(format!("Streaming error: {e}")));
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    yield Err(Error::api(format!("Failed to create stream: {e}")));
                }
            }
        };

        Ok(Box::pin(stream))
    }

    fn llm_type(&self) -> &str {
        &self.model
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
#[allow(
    deprecated,
    clippy::disallowed_methods,
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used
)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires MISTRAL_API_KEY"]
    fn test_chat_mistral_creation() {
        let model = ChatMistralAI::new();
        assert_eq!(model.model, "mistral-small-latest");
        assert_eq!(model.temperature, Some(0.7));
    }

    #[test]
    #[ignore = "requires MISTRAL_API_KEY"]
    fn test_chat_mistral_with_custom_settings() {
        let model = ChatMistralAI::new()
            .with_model("mistral-large-latest")
            .with_temperature(0.9)
            .with_max_tokens(500)
            .with_random_seed(42);

        assert_eq!(model.model, "mistral-large-latest");
        assert_eq!(model.temperature, Some(0.9));
        assert_eq!(model.max_tokens, Some(500));
        assert_eq!(model.random_seed, Some(42));
    }

    #[test]
    fn test_message_conversion() {
        let human_msg = Message::human("Hello");
        let mistral_msg = convert_message_to_mistral(&human_msg).unwrap();
        assert!(matches!(mistral_msg.role, ChatMessageRole::User));
        assert_eq!(mistral_msg.content, "Hello");
    }

    #[test]
    #[ignore = "requires MISTRAL_API_KEY"]
    fn test_model_enum_conversion() {
        let model = ChatMistralAI::new().with_model("open-mistral-7b");
        let mistral_model = model.get_mistral_model();
        assert!(matches!(mistral_model, MistralModel::OpenMistral7b));
    }

    #[test]
    #[ignore = "requires MISTRAL_API_KEY"]
    fn test_llm_type() {
        let model = ChatMistralAI::new().with_model("mistral-large-latest");
        assert_eq!(model.llm_type(), "mistral-large-latest");
    }

    #[test]
    fn test_ai_message_with_tool_calls_conversion() {
        // Test converting an AI message with tool calls to Mistral format
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "get_weather".to_string(),
            args: serde_json::json!({
                "location": "San Francisco",
                "units": "celsius"
            }),
            tool_type: "tool_call".to_string(),
            index: None,
        };

        let ai_msg = Message::AI {
            content: "Let me check the weather for you.".into(),
            tool_calls: vec![tool_call],
            invalid_tool_calls: Vec::new(),
            usage_metadata: None,
            fields: Default::default(),
        };

        let mistral_msg = convert_message_to_mistral(&ai_msg).unwrap();
        assert!(matches!(mistral_msg.role, ChatMessageRole::Assistant));
        assert_eq!(mistral_msg.content, "Let me check the weather for you.");
        assert!(mistral_msg.tool_calls.is_some());

        let tool_calls = mistral_msg.tool_calls.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].function.name, "get_weather");

        // Parse the arguments back to verify correct serialization
        let parsed_args: serde_json::Value =
            serde_json::from_str(&tool_calls[0].function.arguments).unwrap();
        assert_eq!(parsed_args["location"], "San Francisco");
        assert_eq!(parsed_args["units"], "celsius");
    }

    #[test]
    fn test_mistral_response_with_tool_calls_conversion() {
        // Test converting Mistral response with tool calls to DashFlow format
        let mistral_msg = MistralChatMessage {
            role: ChatMessageRole::Assistant,
            content: "I'll help you with that.".to_string(),
            tool_calls: Some(vec![MistralToolCall {
                function: ToolCallFunction {
                    name: "calculator".to_string(),
                    arguments: r#"{"operation":"add","x":5,"y":3}"#.to_string(),
                },
            }]),
        };

        let usage = ResponseUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        };

        let dashflow_msg = convert_mistral_message_to_message(&mistral_msg, Some(&usage)).unwrap();

        match dashflow_msg {
            Message::AI {
                content,
                tool_calls,
                usage_metadata,
                ..
            } => {
                assert_eq!(content.as_text(), "I'll help you with that.");
                assert_eq!(tool_calls.len(), 1);
                assert_eq!(tool_calls[0].name, "calculator");
                assert_eq!(tool_calls[0].args["operation"], "add");
                assert_eq!(tool_calls[0].args["x"], 5);
                assert_eq!(tool_calls[0].args["y"], 3);

                let usage_meta = usage_metadata.unwrap();
                assert_eq!(usage_meta.input_tokens, 10);
                assert_eq!(usage_meta.output_tokens, 20);
                assert_eq!(usage_meta.total_tokens, 30);
            }
            _ => panic!("Expected AI message"),
        }
    }

    #[test]
    fn test_ai_message_without_tool_calls_conversion() {
        // Test that AI messages without tool calls still work correctly
        let ai_msg = Message::AI {
            content: "Just a regular response.".into(),
            tool_calls: Vec::new(),
            invalid_tool_calls: Vec::new(),
            usage_metadata: None,
            fields: Default::default(),
        };

        let mistral_msg = convert_message_to_mistral(&ai_msg).unwrap();
        assert!(matches!(mistral_msg.role, ChatMessageRole::Assistant));
        assert_eq!(mistral_msg.content, "Just a regular response.");
        assert!(mistral_msg.tool_calls.is_none());
    }

    #[test]
    fn test_convert_tool_choice_to_mistral() {
        // Test valid tool choice conversions
        assert!(matches!(
            convert_tool_choice_to_mistral("auto").unwrap(),
            MistralToolChoice::Auto
        ));
        assert!(matches!(
            convert_tool_choice_to_mistral("none").unwrap(),
            MistralToolChoice::None
        ));
        assert!(matches!(
            convert_tool_choice_to_mistral("any").unwrap(),
            MistralToolChoice::Any
        ));

        // Test case insensitivity
        assert!(matches!(
            convert_tool_choice_to_mistral("AUTO").unwrap(),
            MistralToolChoice::Auto
        ));
        assert!(matches!(
            convert_tool_choice_to_mistral("None").unwrap(),
            MistralToolChoice::None
        ));

        // Test invalid choice
        assert!(convert_tool_choice_to_mistral("invalid").is_err());
        assert!(convert_tool_choice_to_mistral("required").is_err()); // "required" is not a valid Mistral option
    }

    #[test]
    fn test_convert_json_tools_to_mistral() {
        // Test converting a simple tool definition
        let json_tools = vec![serde_json::json!({
            "type": "function",
            "function": {
                "name": "calculator",
                "description": "Performs arithmetic operations",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "operation": {
                            "type": "string",
                            "description": "The operation to perform"
                        },
                        "a": {
                            "type": "string",
                            "description": "First number"
                        },
                        "b": {
                            "type": "string",
                            "description": "Second number"
                        }
                    },
                    "required": ["operation", "a", "b"]
                }
            }
        })];

        let mistral_tools = convert_json_tools_to_mistral(&json_tools).unwrap();
        assert_eq!(mistral_tools.len(), 1);
        // Tool structure is correct if no panic occurs
    }

    #[test]
    fn test_convert_json_tools_missing_function() {
        // Test error handling for missing function field
        let json_tools = vec![serde_json::json!({
            "type": "function"
        })];

        let result = convert_json_tools_to_mistral(&json_tools);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("missing 'function' field"));
    }

    #[test]
    fn test_convert_json_tools_missing_name() {
        // Test error handling for missing name field
        let json_tools = vec![serde_json::json!({
            "type": "function",
            "function": {
                "description": "A tool without a name"
            }
        })];

        let result = convert_json_tools_to_mistral(&json_tools);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("missing 'function.name' field"));
    }

    #[test]
    #[ignore = "requires MISTRAL_API_KEY"]
    #[allow(deprecated)] // Testing deprecated with_tools() method
    fn test_build_chat_params_with_tools() {
        let calculator_tool = serde_json::json!({
            "type": "function",
            "function": {
                "name": "calculator",
                "description": "Performs arithmetic",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "operation": {"type": "string", "description": "Operation"}
                    }
                }
            }
        });

        let model = ChatMistralAI::new()
            .with_tools(vec![calculator_tool])
            .with_tool_choice("auto");

        let params = model.build_chat_params().unwrap();
        assert!(params.tools.is_some());
        assert!(params.tool_choice.is_some());
        assert!(matches!(
            params.tool_choice.unwrap(),
            MistralToolChoice::Auto
        ));
    }

    // ============================================
    // Builder method tests (no API key required)
    // ============================================

    #[test]
    fn test_with_api_key_builder() {
        let model = ChatMistralAI::with_api_key("test-api-key-12345").unwrap();
        // Default model should be set
        assert_eq!(model.model, "mistral-small-latest");
        // Default temperature
        assert_eq!(model.temperature, Some(0.7));
    }

    #[test]
    fn test_with_model_builder() {
        let model = ChatMistralAI::with_api_key("test-key")
            .unwrap()
            .with_model("mistral-large-latest");
        assert_eq!(model.model, "mistral-large-latest");
    }

    #[test]
    fn test_with_model_various_models() {
        // Test all supported model name variations
        let models = vec![
            ("open-mistral-7b", "open-mistral-7b"),
            ("open-mixtral-8x7b", "open-mixtral-8x7b"),
            ("open-mixtral-8x22b", "open-mixtral-8x22b"),
            ("open-mistral-nemo", "open-mistral-nemo"),
            ("mistral-tiny", "mistral-tiny"),
            ("mistral-small-latest", "mistral-small-latest"),
            ("mistral-small", "mistral-small"),
            ("mistral-medium-latest", "mistral-medium-latest"),
            ("mistral-medium", "mistral-medium"),
            ("mistral-large-latest", "mistral-large-latest"),
            ("mistral-large", "mistral-large"),
            ("codestral-latest", "codestral-latest"),
            ("codestral", "codestral"),
            ("codestral-mamba", "codestral-mamba"),
        ];

        for (input, expected) in models {
            let model = ChatMistralAI::with_api_key("key")
                .unwrap()
                .with_model(input);
            assert_eq!(model.model, expected);
        }
    }

    #[test]
    fn test_with_temperature_builder() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_temperature(0.5);
        assert_eq!(model.temperature, Some(0.5));
    }

    #[test]
    fn test_with_temperature_zero() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_temperature(0.0);
        assert_eq!(model.temperature, Some(0.0));
    }

    #[test]
    fn test_with_temperature_one() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_temperature(1.0);
        assert_eq!(model.temperature, Some(1.0));
    }

    #[test]
    fn test_with_max_tokens_builder() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_max_tokens(1000);
        assert_eq!(model.max_tokens, Some(1000));
    }

    #[test]
    fn test_with_max_tokens_small() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_max_tokens(1);
        assert_eq!(model.max_tokens, Some(1));
    }

    #[test]
    fn test_with_max_tokens_large() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_max_tokens(100_000);
        assert_eq!(model.max_tokens, Some(100_000));
    }

    #[test]
    fn test_with_top_p_builder() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_top_p(0.95);
        assert_eq!(model.top_p, Some(0.95));
    }

    #[test]
    fn test_with_top_p_zero() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_top_p(0.0);
        assert_eq!(model.top_p, Some(0.0));
    }

    #[test]
    fn test_with_random_seed_builder() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_random_seed(42);
        assert_eq!(model.random_seed, Some(42));
    }

    #[test]
    fn test_with_random_seed_zero() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_random_seed(0);
        assert_eq!(model.random_seed, Some(0));
    }

    #[test]
    fn test_with_random_seed_negative() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_random_seed(-100);
        assert_eq!(model.random_seed, Some(-100));
    }

    #[test]
    fn test_with_safe_mode_enabled() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_safe_mode(true);
        assert_eq!(model.safe_mode, Some(true));
    }

    #[test]
    fn test_with_safe_mode_disabled() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_safe_mode(false);
        assert_eq!(model.safe_mode, Some(false));
    }

    #[test]
    fn test_with_retry_policy_exponential() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_retry_policy(RetryPolicy::exponential(5));
        // RetryPolicy doesn't implement PartialEq, so we just verify it doesn't panic
        assert_eq!(model.model, "mistral-small-latest");
    }

    #[test]
    fn test_with_retry_policy_fixed() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_retry_policy(RetryPolicy::fixed(3, 1000));
        assert_eq!(model.model, "mistral-small-latest");
    }

    #[test]
    fn test_with_rate_limiter() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_rate_limiter(rate_limiter);

        assert!(model.rate_limiter.is_some());
    }

    #[test]
    fn test_with_tool_choice_auto() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_tool_choice("auto");
        assert_eq!(model.tool_choice, Some("auto".to_string()));
    }

    #[test]
    fn test_with_tool_choice_none() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_tool_choice("none");
        assert_eq!(model.tool_choice, Some("none".to_string()));
    }

    #[test]
    fn test_with_tool_choice_any() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_tool_choice("any");
        assert_eq!(model.tool_choice, Some("any".to_string()));
    }

    // ============================================
    // Builder chaining tests
    // ============================================

    #[test]
    fn test_builder_chain_all_params() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("mistral-large-latest")
            .with_temperature(0.8)
            .with_max_tokens(500)
            .with_top_p(0.9)
            .with_random_seed(123)
            .with_safe_mode(true)
            .with_tool_choice("auto");

        assert_eq!(model.model, "mistral-large-latest");
        assert_eq!(model.temperature, Some(0.8));
        assert_eq!(model.max_tokens, Some(500));
        assert_eq!(model.top_p, Some(0.9));
        assert_eq!(model.random_seed, Some(123));
        assert_eq!(model.safe_mode, Some(true));
        assert_eq!(model.tool_choice, Some("auto".to_string()));
    }

    #[test]
    fn test_builder_chain_order_independence() {
        // Build with different orders, should get same results
        let model1 = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_temperature(0.5)
            .with_model("mistral-large")
            .with_max_tokens(100);

        let model2 = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_max_tokens(100)
            .with_temperature(0.5)
            .with_model("mistral-large");

        assert_eq!(model1.model, model2.model);
        assert_eq!(model1.temperature, model2.temperature);
        assert_eq!(model1.max_tokens, model2.max_tokens);
    }

    #[test]
    fn test_builder_overwrite_values() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_temperature(0.5)
            .with_temperature(0.9);
        assert_eq!(model.temperature, Some(0.9));
    }

    // ============================================
    // Debug implementation tests
    // ============================================

    #[test]
    fn test_debug_impl_exists() {
        let model = ChatMistralAI::with_api_key("secret-key").unwrap();
        let debug_str = format!("{:?}", model);
        assert!(debug_str.contains("ChatMistralAI"));
    }

    #[test]
    fn test_debug_shows_model() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("mistral-large");
        let debug_str = format!("{:?}", model);
        assert!(debug_str.contains("mistral-large"));
    }

    // ============================================
    // Clone tests
    // ============================================

    #[test]
    fn test_clone_basic() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("mistral-large")
            .with_temperature(0.5);

        let cloned = model.clone();
        assert_eq!(cloned.model, "mistral-large");
        assert_eq!(cloned.temperature, Some(0.5));
    }

    #[test]
    fn test_clone_with_rate_limiter() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_rate_limiter(rate_limiter);

        let cloned = model.clone();
        assert!(cloned.rate_limiter.is_some());
    }

    #[test]
    fn test_clone_independence() {
        let model1 = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_temperature(0.5);

        let mut model2 = model1.clone();
        model2.temperature = Some(0.9);

        // Original should be unchanged
        assert_eq!(model1.temperature, Some(0.5));
        assert_eq!(model2.temperature, Some(0.9));
    }

    // ============================================
    // ChatModel trait tests
    // ============================================

    #[test]
    fn test_llm_type_returns_model_name() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("custom-model");
        assert_eq!(model.llm_type(), "custom-model");
    }

    #[test]
    fn test_as_any() {
        let model = ChatMistralAI::with_api_key("key").unwrap();
        let any_ref = model.as_any();
        assert!(any_ref.downcast_ref::<ChatMistralAI>().is_some());
    }

    // ============================================
    // Model enum conversion tests (no API call)
    // ============================================

    #[test]
    fn test_get_mistral_model_open_mistral_7b() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("open-mistral-7b");
        let mistral_model = model.get_mistral_model();
        assert!(matches!(mistral_model, MistralModel::OpenMistral7b));
    }

    #[test]
    fn test_get_mistral_model_open_mixtral_8x7b() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("open-mixtral-8x7b");
        let mistral_model = model.get_mistral_model();
        assert!(matches!(mistral_model, MistralModel::OpenMixtral8x7b));
    }

    #[test]
    fn test_get_mistral_model_open_mixtral_8x22b() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("open-mixtral-8x22b");
        let mistral_model = model.get_mistral_model();
        assert!(matches!(mistral_model, MistralModel::OpenMixtral8x22b));
    }

    #[test]
    fn test_get_mistral_model_open_mistral_nemo() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("open-mistral-nemo");
        let mistral_model = model.get_mistral_model();
        assert!(matches!(mistral_model, MistralModel::OpenMistralNemo));
    }

    #[test]
    fn test_get_mistral_model_mistral_tiny() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("mistral-tiny");
        let mistral_model = model.get_mistral_model();
        assert!(matches!(mistral_model, MistralModel::MistralTiny));
    }

    #[test]
    fn test_get_mistral_model_mistral_small() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("mistral-small");
        let mistral_model = model.get_mistral_model();
        assert!(matches!(mistral_model, MistralModel::MistralSmallLatest));
    }

    #[test]
    fn test_get_mistral_model_mistral_small_latest() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("mistral-small-latest");
        let mistral_model = model.get_mistral_model();
        assert!(matches!(mistral_model, MistralModel::MistralSmallLatest));
    }

    #[test]
    fn test_get_mistral_model_mistral_medium() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("mistral-medium");
        let mistral_model = model.get_mistral_model();
        assert!(matches!(mistral_model, MistralModel::MistralMediumLatest));
    }

    #[test]
    fn test_get_mistral_model_mistral_medium_latest() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("mistral-medium-latest");
        let mistral_model = model.get_mistral_model();
        assert!(matches!(mistral_model, MistralModel::MistralMediumLatest));
    }

    #[test]
    fn test_get_mistral_model_mistral_large() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("mistral-large");
        let mistral_model = model.get_mistral_model();
        assert!(matches!(mistral_model, MistralModel::MistralLargeLatest));
    }

    #[test]
    fn test_get_mistral_model_mistral_large_latest() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("mistral-large-latest");
        let mistral_model = model.get_mistral_model();
        assert!(matches!(mistral_model, MistralModel::MistralLargeLatest));
    }

    #[test]
    fn test_get_mistral_model_codestral() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("codestral");
        let mistral_model = model.get_mistral_model();
        assert!(matches!(mistral_model, MistralModel::CodestralLatest));
    }

    #[test]
    fn test_get_mistral_model_codestral_latest() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("codestral-latest");
        let mistral_model = model.get_mistral_model();
        assert!(matches!(mistral_model, MistralModel::CodestralLatest));
    }

    #[test]
    fn test_get_mistral_model_codestral_mamba() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("codestral-mamba");
        let mistral_model = model.get_mistral_model();
        assert!(matches!(mistral_model, MistralModel::CodestralMamba));
    }

    #[test]
    fn test_get_mistral_model_unknown_defaults_to_small() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("unknown-model-xyz");
        let mistral_model = model.get_mistral_model();
        assert!(matches!(mistral_model, MistralModel::MistralSmallLatest));
    }

    // ============================================
    // Message conversion edge cases
    // ============================================

    #[test]
    fn test_convert_system_message() {
        let system_msg = Message::system("You are a helpful assistant.");
        let mistral_msg = convert_message_to_mistral(&system_msg).unwrap();
        assert!(matches!(mistral_msg.role, ChatMessageRole::System));
        assert_eq!(mistral_msg.content, "You are a helpful assistant.");
        assert!(mistral_msg.tool_calls.is_none());
    }

    #[test]
    fn test_convert_tool_message() {
        // Message::tool(content, tool_call_id)
        let tool_msg = Message::tool("The weather is sunny.", "tool_123");
        let mistral_msg = convert_message_to_mistral(&tool_msg).unwrap();
        assert!(matches!(mistral_msg.role, ChatMessageRole::Tool));
        assert_eq!(mistral_msg.content, "The weather is sunny.");
        assert!(mistral_msg.tool_calls.is_none());
    }

    #[test]
    fn test_convert_human_message_empty() {
        let human_msg = Message::human("");
        let mistral_msg = convert_message_to_mistral(&human_msg).unwrap();
        assert!(matches!(mistral_msg.role, ChatMessageRole::User));
        assert_eq!(mistral_msg.content, "");
    }

    #[test]
    fn test_convert_human_message_unicode() {
        let human_msg = Message::human("こんにちは! 🎉 Ça va? Привет!");
        let mistral_msg = convert_message_to_mistral(&human_msg).unwrap();
        assert_eq!(mistral_msg.content, "こんにちは! 🎉 Ça va? Привет!");
    }

    #[test]
    fn test_convert_human_message_with_newlines() {
        let human_msg = Message::human("Line 1\nLine 2\nLine 3");
        let mistral_msg = convert_message_to_mistral(&human_msg).unwrap();
        assert_eq!(mistral_msg.content, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_convert_ai_message_with_multiple_tool_calls() {
        let tool_calls = vec![
            ToolCall {
                id: "call_1".to_string(),
                name: "get_weather".to_string(),
                args: serde_json::json!({"city": "Paris"}),
                tool_type: "tool_call".to_string(),
                index: None,
            },
            ToolCall {
                id: "call_2".to_string(),
                name: "get_time".to_string(),
                args: serde_json::json!({"timezone": "UTC"}),
                tool_type: "tool_call".to_string(),
                index: None,
            },
        ];

        let ai_msg = Message::AI {
            content: "Let me check both.".into(),
            tool_calls,
            invalid_tool_calls: Vec::new(),
            usage_metadata: None,
            fields: Default::default(),
        };

        let mistral_msg = convert_message_to_mistral(&ai_msg).unwrap();
        let tool_calls = mistral_msg.tool_calls.unwrap();
        assert_eq!(tool_calls.len(), 2);
        assert_eq!(tool_calls[0].function.name, "get_weather");
        assert_eq!(tool_calls[1].function.name, "get_time");
    }

    // ============================================
    // Mistral response conversion edge cases
    // ============================================

    #[test]
    fn test_convert_mistral_message_without_usage() {
        let mistral_msg = MistralChatMessage {
            role: ChatMessageRole::Assistant,
            content: "Hello!".to_string(),
            tool_calls: None,
        };

        let dashflow_msg = convert_mistral_message_to_message(&mistral_msg, None).unwrap();
        match dashflow_msg {
            Message::AI { usage_metadata, .. } => {
                assert!(usage_metadata.is_none());
            }
            _ => panic!("Expected AI message"),
        }
    }

    #[test]
    fn test_convert_mistral_message_with_zero_usage() {
        let mistral_msg = MistralChatMessage {
            role: ChatMessageRole::Assistant,
            content: "".to_string(),
            tool_calls: None,
        };

        let usage = ResponseUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        };

        let dashflow_msg = convert_mistral_message_to_message(&mistral_msg, Some(&usage)).unwrap();
        match dashflow_msg {
            Message::AI { usage_metadata, .. } => {
                let meta = usage_metadata.unwrap();
                assert_eq!(meta.input_tokens, 0);
                assert_eq!(meta.output_tokens, 0);
                assert_eq!(meta.total_tokens, 0);
            }
            _ => panic!("Expected AI message"),
        }
    }

    #[test]
    fn test_convert_mistral_message_with_malformed_tool_args() {
        // Test that malformed JSON in tool call arguments is handled gracefully
        let mistral_msg = MistralChatMessage {
            role: ChatMessageRole::Assistant,
            content: "".to_string(),
            tool_calls: Some(vec![MistralToolCall {
                function: ToolCallFunction {
                    name: "bad_tool".to_string(),
                    arguments: "not valid json".to_string(),
                },
            }]),
        };

        let dashflow_msg = convert_mistral_message_to_message(&mistral_msg, None).unwrap();
        match dashflow_msg {
            Message::AI { tool_calls, .. } => {
                assert_eq!(tool_calls.len(), 1);
                assert_eq!(tool_calls[0].name, "bad_tool");
                // Should have error and raw fields
                assert!(tool_calls[0].args["error"].is_string());
                assert!(tool_calls[0].args["raw"].is_string());
            }
            _ => panic!("Expected AI message"),
        }
    }

    #[test]
    fn test_convert_mistral_message_multiple_tool_calls_with_indexes() {
        let mistral_msg = MistralChatMessage {
            role: ChatMessageRole::Assistant,
            content: "".to_string(),
            tool_calls: Some(vec![
                MistralToolCall {
                    function: ToolCallFunction {
                        name: "tool_a".to_string(),
                        arguments: r#"{"key": "value1"}"#.to_string(),
                    },
                },
                MistralToolCall {
                    function: ToolCallFunction {
                        name: "tool_b".to_string(),
                        arguments: r#"{"key": "value2"}"#.to_string(),
                    },
                },
                MistralToolCall {
                    function: ToolCallFunction {
                        name: "tool_c".to_string(),
                        arguments: r#"{"key": "value3"}"#.to_string(),
                    },
                },
            ]),
        };

        let dashflow_msg = convert_mistral_message_to_message(&mistral_msg, None).unwrap();
        match dashflow_msg {
            Message::AI { tool_calls, .. } => {
                assert_eq!(tool_calls.len(), 3);
                // IDs should be generated based on index
                assert_eq!(tool_calls[0].id, "call_0");
                assert_eq!(tool_calls[1].id, "call_1");
                assert_eq!(tool_calls[2].id, "call_2");
            }
            _ => panic!("Expected AI message"),
        }
    }

    // ============================================
    // Tool definition conversion tests
    // ============================================

    #[test]
    fn test_convert_tool_definition_basic() {
        let tool_def = ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get current weather".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "City name"
                    }
                }
            }),
        };

        let result = convert_tool_definition(&tool_def);
        assert!(result.is_ok());
    }

    #[test]
    fn test_convert_tool_definition_multiple_params() {
        let tool_def = ToolDefinition {
            name: "complex_tool".to_string(),
            description: "A complex tool".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "param1": {"type": "string", "description": "First param"},
                    "param2": {"type": "number", "description": "Second param"},
                    "param3": {"type": "boolean", "description": "Third param"}
                }
            }),
        };

        let result = convert_tool_definition(&tool_def);
        assert!(result.is_ok());
    }

    #[test]
    fn test_convert_tool_definition_no_description() {
        let tool_def = ToolDefinition {
            name: "simple_tool".to_string(),
            description: "".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "arg": {"type": "string"}
                }
            }),
        };

        // Should work with empty description
        let result = convert_tool_definition(&tool_def);
        assert!(result.is_ok());
    }

    #[test]
    fn test_convert_tool_definition_missing_properties() {
        let tool_def = ToolDefinition {
            name: "bad_tool".to_string(),
            description: "Missing properties".to_string(),
            parameters: serde_json::json!({
                "type": "object"
            }),
        };

        let result = convert_tool_definition(&tool_def);
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_tool_definition_invalid_parameters() {
        let tool_def = ToolDefinition {
            name: "bad_tool".to_string(),
            description: "Invalid params".to_string(),
            parameters: serde_json::json!("not an object"),
        };

        let result = convert_tool_definition(&tool_def);
        assert!(result.is_err());
    }

    // ============================================
    // JSON tool conversion tests
    // ============================================

    #[test]
    fn test_convert_json_tools_multiple() {
        let json_tools = vec![
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "tool1",
                    "description": "First tool",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "arg1": {"type": "string"}
                        }
                    }
                }
            }),
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "tool2",
                    "description": "Second tool",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "arg2": {"type": "string"}
                        }
                    }
                }
            }),
        ];

        let result = convert_json_tools_to_mistral(&json_tools);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[test]
    fn test_convert_json_tools_empty() {
        let json_tools: Vec<serde_json::Value> = vec![];
        let result = convert_json_tools_to_mistral(&json_tools);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_convert_json_tools_missing_description() {
        // Tool with no description should still work (defaults to empty string)
        let json_tools = vec![serde_json::json!({
            "type": "function",
            "function": {
                "name": "nodesc_tool",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "arg": {"type": "string"}
                    }
                }
            }
        })];

        let result = convert_json_tools_to_mistral(&json_tools);
        assert!(result.is_ok());
    }

    #[test]
    fn test_convert_json_tools_missing_parameters() {
        // Tool with no parameters at all
        let json_tools = vec![serde_json::json!({
            "type": "function",
            "function": {
                "name": "simple_tool",
                "description": "No params"
            }
        })];

        // Implementation allows tools without parameters - they get empty params list
        // This is valid for tools that take no arguments
        let result = convert_json_tools_to_mistral(&json_tools);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    // ============================================
    // ToolChoice conversion tests
    // ============================================

    #[test]
    fn test_convert_tool_choice_auto() {
        let choice = convert_tool_choice(&ToolChoice::Auto);
        assert!(matches!(choice, MistralToolChoice::Auto));
    }

    #[test]
    fn test_convert_tool_choice_none() {
        let choice = convert_tool_choice(&ToolChoice::None);
        assert!(matches!(choice, MistralToolChoice::None));
    }

    #[test]
    fn test_convert_tool_choice_required() {
        let choice = convert_tool_choice(&ToolChoice::Required);
        assert!(matches!(choice, MistralToolChoice::Any));
    }

    #[test]
    fn test_convert_tool_choice_specific_falls_back_to_auto() {
        // Mistral doesn't support specific tool forcing
        let choice = convert_tool_choice(&ToolChoice::Specific("my_tool".to_string()));
        assert!(matches!(choice, MistralToolChoice::Auto));
    }

    // ============================================
    // Edge case tests
    // ============================================

    #[test]
    fn test_special_chars_in_model_name() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("model-with-special_chars.v1");
        assert_eq!(model.model, "model-with-special_chars.v1");
    }

    #[test]
    fn test_unicode_in_api_key() {
        let model = ChatMistralAI::with_api_key("test-key-日本語-🔑");
        assert!(model.is_ok());
    }

    #[test]
    fn test_empty_string_model() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("");
        assert_eq!(model.model, "");
        // Unknown model should default to small
        let mistral_model = model.get_mistral_model();
        assert!(matches!(mistral_model, MistralModel::MistralSmallLatest));
    }

    #[test]
    fn test_whitespace_model_name() {
        let model = ChatMistralAI::with_api_key("key")
            .unwrap()
            .with_model("   ");
        // Should be stored as-is
        assert_eq!(model.model, "   ");
    }

    #[test]
    fn test_very_long_api_key() {
        let long_key = "x".repeat(10000);
        let model = ChatMistralAI::with_api_key(&long_key);
        assert!(model.is_ok());
    }
}

/// Standard conformance tests
///
/// These tests verify that ChatMistralAI behaves consistently with other
/// ChatModel implementations across the DashFlow ecosystem.
#[cfg(test)]
#[allow(deprecated, clippy::disallowed_methods)]
mod standard_tests {
    use super::*;
    use dashflow_standard_tests::chat_model_tests::*;
    use dashflow_test_utils::init_test_env;

    /// Helper function to create a test model with standard settings
    ///
    /// Uses mistral-small-latest for testing
    fn create_test_model() -> ChatMistralAI {
        init_test_env().ok();
        ChatMistralAI::new()
            .with_model("mistral-small-latest")
            .with_temperature(0.0) // Deterministic for testing
            .with_max_tokens(100) // Limit tokens for cost/speed
    }

    /// Standard Test 1: Basic invoke
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_invoke_standard() {
        let model = create_test_model();
        test_invoke(&model).await;
    }

    /// Standard Test 2: Streaming
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_stream_standard() {
        let model = create_test_model();
        test_stream(&model).await;
    }

    /// Standard Test 3: Batch processing
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_batch_standard() {
        let model = create_test_model();
        test_batch(&model).await;
    }

    /// Standard Test 4: Multi-turn conversation
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_conversation_standard() {
        let model = create_test_model();
        test_conversation(&model).await;
    }

    /// Standard Test 4b: Double messages conversation
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_double_messages_conversation_standard() {
        let model = create_test_model();
        test_double_messages_conversation(&model).await;
    }

    /// Standard Test 4c: Message with name field
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_message_with_name_standard() {
        let model = create_test_model();
        test_message_with_name(&model).await;
    }

    /// Standard Test 5: Stop sequences
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_stop_sequence_standard() {
        let model = create_test_model();
        test_stop_sequence(&model).await;
    }

    /// Standard Test 6: Usage metadata
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_usage_metadata_standard() {
        let model = create_test_model();
        test_usage_metadata(&model).await;
    }

    /// Standard Test 7: Empty messages
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_empty_messages_standard() {
        let model = create_test_model();
        test_empty_messages(&model).await;
    }

    /// Standard Test 8: Long conversation
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_long_conversation_standard() {
        let model = create_test_model();
        test_long_conversation(&model).await;
    }

    /// Standard Test 9: Special characters
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_special_characters_standard() {
        let model = create_test_model();
        test_special_characters(&model).await;
    }

    /// Standard Test 10: Unicode and emoji
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_unicode_standard() {
        let model = create_test_model();
        test_unicode(&model).await;
    }

    /// Standard Test 11: Tool calling
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_tool_calling_standard() {
        let model = create_test_model();
        test_tool_calling(&model).await;
    }

    /// Standard Test 12: Structured output
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_structured_output_standard() {
        let model = create_test_model();
        test_structured_output(&model).await;
    }

    /// Standard Test 13: JSON mode
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_json_mode_standard() {
        let model = create_test_model();
        test_json_mode(&model).await;
    }

    /// Standard Test 14: Usage metadata in streaming
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_usage_metadata_streaming_standard() {
        let model = create_test_model();
        test_usage_metadata_streaming(&model).await;
    }

    /// Standard Test 15: System message handling
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_system_message_standard() {
        let model = create_test_model();
        test_system_message(&model).await;
    }

    /// Standard Test 16: Empty content handling
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_empty_content_standard() {
        let model = create_test_model();
        test_empty_content(&model).await;
    }

    /// Standard Test 17: Large input handling
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_large_input_standard() {
        let model = create_test_model();
        test_large_input(&model).await;
    }

    /// Standard Test 18: Concurrent generation
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_concurrent_generation_standard() {
        let model = create_test_model();
        test_concurrent_generation(&model).await;
    }

    /// Standard Test 19: Error recovery
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_error_recovery_standard() {
        let model = create_test_model();
        test_error_recovery(&model).await;
    }

    /// Standard Test 20: Response consistency
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_response_consistency_standard() {
        let model = create_test_model();
        test_response_consistency(&model).await;
    }

    /// Standard Test 21: Tool calling with no arguments
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_tool_calling_with_no_arguments_standard() {
        let model = create_test_model();
        test_tool_calling_with_no_arguments(&model).await;
    }

    // ========================================================================
    // COMPREHENSIVE TESTS - Advanced Edge Cases
    // ========================================================================

    /// Comprehensive Test 1: Streaming with timeout
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_stream_with_timeout_comprehensive() {
        let model = create_test_model();
        test_stream_with_timeout(&model).await;
    }

    /// Comprehensive Test 2: Streaming interruption handling
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_stream_interruption_comprehensive() {
        let model = create_test_model();
        test_stream_interruption(&model).await;
    }

    /// Comprehensive Test 3: Empty stream handling
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_stream_empty_response_comprehensive() {
        let model = create_test_model();
        test_stream_empty_response(&model).await;
    }

    /// Comprehensive Test 4: Multiple system messages
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_multiple_system_messages_comprehensive() {
        let model = create_test_model();
        test_multiple_system_messages(&model).await;
    }

    /// Comprehensive Test 5: Empty system message
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_empty_system_message_comprehensive() {
        let model = create_test_model();
        test_empty_system_message(&model).await;
    }

    /// Comprehensive Test 6: Temperature edge cases
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_temperature_extremes_comprehensive() {
        let model = create_test_model();
        test_temperature_extremes(&model).await;
    }

    /// Comprehensive Test 7: Max tokens enforcement
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_max_tokens_limit_comprehensive() {
        let model = create_test_model();
        test_max_tokens_limit(&model).await;
    }

    /// Comprehensive Test 8: Invalid stop sequences
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_invalid_stop_sequences_comprehensive() {
        let model = create_test_model();
        test_invalid_stop_sequences(&model).await;
    }

    /// Comprehensive Test 9: Context window overflow
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_context_window_overflow_comprehensive() {
        let model = create_test_model();
        test_context_window_overflow(&model).await;
    }

    /// Comprehensive Test 10: Rapid consecutive calls
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_rapid_consecutive_calls_comprehensive() {
        let model = create_test_model();
        test_rapid_consecutive_calls(&model).await;
    }

    /// Comprehensive Test 11: Network error handling
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_network_error_handling_comprehensive() {
        let model = create_test_model();
        test_network_error_handling(&model).await;
    }

    /// Comprehensive Test 12: Malformed input recovery
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_malformed_input_recovery_comprehensive() {
        let model = create_test_model();
        test_malformed_input_recovery(&model).await;
    }

    /// Comprehensive Test 13: Very long single message
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_very_long_single_message_comprehensive() {
        let model = create_test_model();
        test_very_long_single_message(&model).await;
    }

    /// Comprehensive Test 14: Response format consistency
    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_response_format_consistency_comprehensive() {
        let model = create_test_model();
        test_response_format_consistency(&model).await;
    }
}
