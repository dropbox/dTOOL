//! `OpenAI` Assistant Agent implementation
//!
//! This module provides support for `OpenAI`'s Assistants API, allowing you to
//! create and run assistants with built-in tools (`code_interpreter`, `file_search`)
//! and custom `DashFlow` tools.
//!
//! # Example
//! ```no_run
//! use dashflow_openai::OpenAIAssistantRunnable;
//! use dashflow::core::runnable::Runnable;
//!
//! #[tokio::main]
//! async fn main() {
//!     let assistant = OpenAIAssistantRunnable::create_assistant(
//!         "Math Tutor",
//!         "You are a personal math tutor. Write and run code to answer math questions.",
//!         vec![serde_json::json!({"type": "code_interpreter"})],
//!         "gpt-4-turbo-preview",
//!         None,
//!     ).await.unwrap();
//!
//!     let mut input = std::collections::HashMap::new();
//!     input.insert("content".to_string(), serde_json::json!("What's 10 - 4 raised to the 2.7"));
//!
//!     let result = assistant.invoke(input, None).await.unwrap();
//!     println!("{:?}", result);
//! }
//! ```

use async_openai::{
    config::OpenAIConfig,
    types::{
        CreateAssistantRequestArgs, CreateMessageRequestArgs, CreateRunRequestArgs,
        CreateThreadRequestArgs, MessageContent, MessageRole, RunObject, RunStatus,
        SubmitToolOutputsRunRequest,
    },
    Client,
};
use dashflow::core::{
    agents::{AgentAction, AgentFinish},
    config::RunnableConfig,
    error::{Error, Result},
    runnable::Runnable,
    serialization::Serializable,
    tools::ToolInput,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc, time::Duration};

/// Agent finish result with `OpenAI` assistant metadata
///
/// This extends the standard `AgentFinish` with additional metadata from
/// the `OpenAI` Assistants API (`run_id` and `thread_id`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIAssistantFinish {
    /// Final output/answer
    pub output: String,

    /// Agent's reasoning for finishing
    pub log: String,

    /// `OpenAI` run ID
    pub run_id: String,

    /// `OpenAI` thread ID
    pub thread_id: String,

    /// Additional return values (e.g., attachments)
    pub return_values: HashMap<String, Value>,
}

impl OpenAIAssistantFinish {
    /// Create a new assistant finish result
    pub fn new(
        output: impl Into<String>,
        log: impl Into<String>,
        run_id: impl Into<String>,
        thread_id: impl Into<String>,
    ) -> Self {
        Self {
            output: output.into(),
            log: log.into(),
            run_id: run_id.into(),
            thread_id: thread_id.into(),
            return_values: HashMap::new(),
        }
    }

    /// Convert to standard `AgentFinish`
    #[must_use]
    pub fn to_agent_finish(&self) -> AgentFinish {
        AgentFinish::new(&self.output, &self.log)
    }
}

/// Agent action with `OpenAI` assistant metadata
///
/// This extends the standard `AgentAction` with additional metadata needed
/// to submit tool outputs back to the `OpenAI` run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIAssistantAction {
    /// Name of the tool to use
    pub tool: String,

    /// Input to pass to the tool
    pub tool_input: ToolInput,

    /// Agent's reasoning / thought process for this action
    pub log: String,

    /// `OpenAI` tool call ID (needed to submit tool output)
    pub tool_call_id: String,

    /// `OpenAI` run ID
    pub run_id: String,

    /// `OpenAI` thread ID
    pub thread_id: String,
}

impl OpenAIAssistantAction {
    /// Create a new assistant action
    pub fn new(
        tool: impl Into<String>,
        tool_input: ToolInput,
        log: impl Into<String>,
        tool_call_id: impl Into<String>,
        run_id: impl Into<String>,
        thread_id: impl Into<String>,
    ) -> Self {
        Self {
            tool: tool.into(),
            tool_input,
            log: log.into(),
            tool_call_id: tool_call_id.into(),
            run_id: run_id.into(),
            thread_id: thread_id.into(),
        }
    }

    /// Convert to standard `AgentAction`
    #[must_use]
    pub fn to_agent_action(&self) -> AgentAction {
        AgentAction {
            tool: self.tool.clone(),
            tool_input: self.tool_input.clone(),
            log: self.log.clone(),
        }
    }
}

/// Output type for `OpenAI` Assistant operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AssistantOutput {
    /// Agent finished with final result
    Finish(OpenAIAssistantFinish),

    /// Agent needs to execute actions
    Actions(Vec<OpenAIAssistantAction>),

    /// Raw messages from the thread (when not in agent mode)
    Messages(Vec<Value>),

    /// Raw tool calls (when not in agent mode)
    ToolCalls(Vec<Value>),
}

/// `OpenAI` Assistant Runnable
///
/// This runnable wraps the `OpenAI` Assistants API and provides integration
/// with `DashFlow`'s agent framework.
///
/// # Features
/// - Built-in tools: `code_interpreter`, `file_search`
/// - Custom tool integration
/// - Agent mode for use with `AgentExecutor`
/// - Async support
/// - Thread and run management
///
/// # Example with built-in tools
/// ```no_run
/// # use dashflow_openai::OpenAIAssistantRunnable;
/// # use dashflow::core::runnable::Runnable;
/// # use std::collections::HashMap;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let assistant = OpenAIAssistantRunnable::create_assistant(
///     "Code Helper",
///     "You write and execute Python code to help users.",
///     vec![serde_json::json!({"type": "code_interpreter"})],
///     "gpt-4-turbo-preview",
///     None,
/// ).await?;
///
/// let mut input = HashMap::new();
/// input.insert("content".to_string(), serde_json::json!("Calculate fibonacci(10)"));
/// let result = assistant.invoke(input, None).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct OpenAIAssistantRunnable {
    /// `OpenAI` client
    client: Arc<Client<OpenAIConfig>>,

    /// Assistant ID
    assistant_id: String,

    /// Frequency to check run progress (in milliseconds)
    check_every_ms: u64,

    /// Maximum time to wait for a run to complete (in seconds).
    /// Defaults to 300 seconds (5 minutes). Set to None for no timeout.
    max_wait_secs: Option<u64>,

    /// Whether to use agent mode (compatible with `AgentExecutor`)
    as_agent: bool,
}

impl OpenAIAssistantRunnable {
    /// Create a new `OpenAI` assistant and return a runnable
    ///
    /// # Arguments
    /// * `name` - Assistant name
    /// * `instructions` - System instructions for the assistant
    /// * `tools` - Tools available to the assistant (built-in or custom)
    /// * `model` - `OpenAI` model to use (e.g., "gpt-4-turbo-preview")
    /// * `client` - Optional `OpenAI` client (creates default if None)
    ///
    /// # Example
    /// ```no_run
    /// # use dashflow_openai::OpenAIAssistantRunnable;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let assistant = OpenAIAssistantRunnable::create_assistant(
    ///     "Math Tutor",
    ///     "You are a helpful math tutor.",
    ///     vec![serde_json::json!({"type": "code_interpreter"})],
    ///     "gpt-4-turbo-preview",
    ///     None,
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_assistant(
        name: impl Into<String>,
        instructions: impl Into<String>,
        tools: Vec<Value>,
        model: impl Into<String>,
        client: Option<Client<OpenAIConfig>>,
    ) -> Result<Self> {
        let client = client.unwrap_or_default();
        let name = name.into();
        let instructions = instructions.into();
        let model = model.into();

        // Convert tools to OpenAI format
        let assistant_tools = tools
            .into_iter()
            .map(|tool| {
                // Parse the JSON value into the appropriate tool type
                serde_json::from_value(tool).map_err(|e| {
                    Error::ToolExecution(format!("Failed to parse tool definition: {e}"))
                })
            })
            .collect::<Result<Vec<_>>>()?;

        // Create assistant via API
        let request = CreateAssistantRequestArgs::default()
            .name(&name)
            .instructions(&instructions)
            .model(&model)
            .tools(assistant_tools)
            .build()
            .map_err(|e| Error::Api(format!("Failed to build assistant request: {e}")))?;

        let assistant = client
            .assistants()
            .create(request)
            .await
            .map_err(|e| Error::Api(format!("Failed to create assistant: {e}")))?;

        Ok(Self {
            client: Arc::new(client),
            assistant_id: assistant.id,
            check_every_ms: 1000,
            max_wait_secs: Some(300), // 5 minute default timeout
            as_agent: false,
        })
    }

    /// Create a runnable from an existing assistant ID
    pub fn from_assistant_id(
        assistant_id: impl Into<String>,
        client: Option<Client<OpenAIConfig>>,
    ) -> Self {
        let client = client.unwrap_or_default();
        Self {
            client: Arc::new(client),
            assistant_id: assistant_id.into(),
            check_every_ms: 1000,
            max_wait_secs: Some(300), // 5 minute default timeout
            as_agent: false,
        }
    }

    /// Set whether to use agent mode
    #[must_use]
    pub fn with_as_agent(mut self, as_agent: bool) -> Self {
        self.as_agent = as_agent;
        self
    }

    /// Set the polling frequency (in milliseconds)
    #[must_use]
    pub fn with_check_every_ms(mut self, check_every_ms: u64) -> Self {
        self.check_every_ms = check_every_ms;
        self
    }

    /// Set the maximum time to wait for a run to complete (in seconds).
    ///
    /// Defaults to 300 seconds (5 minutes). Set to `None` for no timeout
    /// (not recommended for production use as runs can hang indefinitely).
    #[must_use]
    pub fn with_max_wait_secs(mut self, max_wait_secs: Option<u64>) -> Self {
        self.max_wait_secs = max_wait_secs;
        self
    }

    /// Parse intermediate steps from `AgentExecutor` into tool outputs
    ///
    /// This method converts `intermediate_steps` (list of action/output tuples)
    /// into the format needed to submit tool outputs to the `OpenAI` API.
    ///
    /// # Arguments
    /// * `intermediate_steps` - Array of [action, output] pairs from `AgentExecutor`
    ///
    /// # Returns
    /// `HashMap` with:
    /// - `tool_outputs`: Array of {output, `tool_call_id`} objects
    /// - `run_id`: The run ID from the last action
    /// - `thread_id`: The thread ID from the last action
    async fn parse_intermediate_steps(
        &self,
        intermediate_steps: &Value,
    ) -> Result<HashMap<String, Value>> {
        // intermediate_steps is an array of [action, output] tuples
        let steps = intermediate_steps.as_array().ok_or_else(|| {
            Error::InvalidInput("intermediate_steps must be an array".to_string())
        })?;

        if steps.is_empty() {
            return Err(Error::InvalidInput(
                "intermediate_steps is empty".to_string(),
            ));
        }

        // Get the last action to extract run_id and thread_id
        // SAFETY: empty check performed above on line 354
        #[allow(clippy::expect_used)] // Validated non-empty above
        let last_step = steps.last().expect("steps validated non-empty above");
        let last_action = last_step
            .as_array()
            .and_then(|arr| arr.first())
            .ok_or_else(|| Error::InvalidInput("Invalid intermediate_steps format".to_string()))?;

        let run_id = last_action
            .get("run_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing run_id in action".to_string()))?;
        let thread_id = last_action
            .get("thread_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing thread_id in action".to_string()))?;

        // Wait for the run to complete (or reach requires_action state)
        let run = self.wait_for_run(run_id, thread_id).await?;

        // Get the required tool call IDs from the run
        let required_tool_call_ids: std::collections::HashSet<String> =
            if let Some(required_action) = &run.required_action {
                required_action
                    .submit_tool_outputs
                    .tool_calls
                    .iter()
                    .map(|tc| tc.id.clone())
                    .collect()
            } else {
                std::collections::HashSet::new()
            };

        // Build tool_outputs array, filtering by required tool call IDs
        let mut tool_outputs = Vec::new();
        for step in steps {
            let step_array = step
                .as_array()
                .ok_or_else(|| Error::InvalidInput("Each step must be an array".to_string()))?;
            if step_array.len() != 2 {
                return Err(Error::InvalidInput(
                    "Each step must be [action, output]".to_string(),
                ));
            }

            let action = &step_array[0];
            let output = &step_array[1];

            let tool_call_id = action
                .get("tool_call_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::InvalidInput("Missing tool_call_id in action".to_string()))?;

            // Only include if this tool_call_id is in the required set
            if required_tool_call_ids.contains(tool_call_id) {
                tool_outputs.push(serde_json::json!({
                    "output": output.as_str().unwrap_or(""),
                    "tool_call_id": tool_call_id,
                }));
            }
        }

        // SAFETY: json! macro with object literal always returns Value::Object
        #[allow(clippy::unwrap_used)]
        Ok(serde_json::json!({
            "tool_outputs": tool_outputs,
            "run_id": run_id,
            "thread_id": thread_id,
        })
        .as_object()
        .unwrap()
        .clone()
        .into_iter()
        .collect())
    }

    /// Wait for a run to complete
    ///
    /// # Errors
    /// Returns an error if the run does not complete within `max_wait_secs` (default: 300s).
    async fn wait_for_run(&self, run_id: &str, thread_id: &str) -> Result<RunObject> {
        let poll_loop = async {
            loop {
                let run = self
                    .client
                    .threads()
                    .runs(thread_id)
                    .retrieve(run_id)
                    .await
                    .map_err(|e| Error::Api(format!("Failed to retrieve run: {e}")))?;

                match run.status {
                    RunStatus::Queued | RunStatus::InProgress => {
                        tokio::time::sleep(Duration::from_millis(self.check_every_ms)).await;
                    }
                    _ => return Ok(run),
                }
            }
        };

        // Apply timeout if configured
        if let Some(max_secs) = self.max_wait_secs {
            let timeout = Duration::from_secs(max_secs);
            tokio::time::timeout(timeout, poll_loop)
                .await
                .map_err(|e| {
                    Error::Timeout(format!(
                        "Run {run_id} did not complete within {max_secs} seconds: {e}"
                    ))
                })?
        } else {
            poll_loop.await
        }
    }

    /// Create a thread and run
    async fn create_thread_and_run(
        &self,
        input: &HashMap<String, Value>,
        content: String,
    ) -> Result<RunObject> {
        // Build thread with initial message
        let mut message_builder = CreateMessageRequestArgs::default();
        message_builder.role(MessageRole::User);
        message_builder.content(content);

        // Add attachments if provided (matching Python baseline line 352)
        if let Some(attachments) = input.get("attachments") {
            // Parse attachments array into MessageAttachment objects
            if let Some(attachments_array) = attachments.as_array() {
                use async_openai::types::{MessageAttachment, MessageAttachmentTool};

                let mut parsed_attachments = Vec::new();
                for attachment in attachments_array {
                    // Each attachment should have file_id and tools
                    if let Some(file_id) = attachment.get("file_id").and_then(|v| v.as_str()) {
                        let tools = if let Some(tools_array) =
                            attachment.get("tools").and_then(|v| v.as_array())
                        {
                            tools_array
                                .iter()
                                .filter_map(|tool| {
                                    tool.get("type").and_then(|t| t.as_str()).and_then(
                                        |t| match t {
                                            "code_interpreter" => {
                                                Some(MessageAttachmentTool::CodeInterpreter {})
                                            }
                                            "file_search" => {
                                                Some(MessageAttachmentTool::FileSearch {})
                                            }
                                            _ => None,
                                        },
                                    )
                                })
                                .collect()
                        } else {
                            vec![]
                        };

                        parsed_attachments.push(MessageAttachment {
                            file_id: file_id.to_string(),
                            tools,
                        });
                    }
                }

                if !parsed_attachments.is_empty() {
                    message_builder.attachments(parsed_attachments);
                }
            }
        }

        let message = message_builder
            .build()
            .map_err(|e| Error::Api(format!("Failed to build message: {e}")))?;

        let mut thread_builder = CreateThreadRequestArgs::default();
        thread_builder.messages(vec![message]);

        // Thread metadata: Not yet implemented (low priority)
        // Reason: async-openai CreateThreadRequestArgs requires API research
        // to determine correct metadata handling approach. Run metadata (see below)
        // is already implemented. Thread-level metadata is lower priority.

        let thread_request = thread_builder
            .build()
            .map_err(|e| Error::Api(format!("Failed to build thread: {e}")))?;

        // Build run request with all optional parameters (matching create_run)
        let mut run_builder = CreateRunRequestArgs::default();
        run_builder.assistant_id(&self.assistant_id);

        // Add optional run parameters (matching Python baseline)
        if let Some(instructions) = input.get("instructions").and_then(|v| v.as_str()) {
            run_builder.instructions(instructions);
        }
        if let Some(additional_instructions) = input
            .get("additional_instructions")
            .and_then(|v| v.as_str())
        {
            run_builder.additional_instructions(additional_instructions);
        }
        if let Some(model) = input.get("model").and_then(|v| v.as_str()) {
            run_builder.model(model);
        }
        if let Some(temp) = input.get("temperature").and_then(serde_json::Value::as_f64) {
            run_builder.temperature(temp as f32);
        }
        if let Some(top_p) = input.get("top_p").and_then(serde_json::Value::as_f64) {
            run_builder.top_p(top_p as f32);
        }
        if let Some(max_prompt_tokens) = input
            .get("max_prompt_tokens")
            .and_then(serde_json::Value::as_u64)
        {
            run_builder.max_prompt_tokens(max_prompt_tokens as u32);
        }
        if let Some(max_completion_tokens) = input
            .get("max_completion_tokens")
            .and_then(serde_json::Value::as_u64)
        {
            run_builder.max_completion_tokens(max_completion_tokens as u32);
        }
        if let Some(parallel_tool_calls) = input
            .get("parallel_tool_calls")
            .and_then(serde_json::Value::as_bool)
        {
            run_builder.parallel_tool_calls(parallel_tool_calls);
        }
        if let Some(run_metadata) = input.get("run_metadata") {
            if let Some(metadata_obj) = run_metadata.as_object() {
                let metadata: HashMap<String, Value> = metadata_obj.clone().into_iter().collect();
                run_builder.metadata(metadata);
            }
        }

        // Create thread first
        let thread = self
            .client
            .threads()
            .create(thread_request)
            .await
            .map_err(|e| Error::Api(format!("Failed to create thread: {e}")))?;

        // Then create run in the thread
        let run_request = run_builder
            .build()
            .map_err(|e| Error::Api(format!("Failed to build run: {e}")))?;

        let run = self
            .client
            .threads()
            .runs(&thread.id)
            .create(run_request)
            .await
            .map_err(|e| Error::Api(format!("Failed to create run: {e}")))?;

        Ok(run)
    }

    /// Create a run in an existing thread
    async fn create_run(
        &self,
        thread_id: &str,
        input: &HashMap<String, Value>,
    ) -> Result<RunObject> {
        let mut run_builder = CreateRunRequestArgs::default();
        run_builder.assistant_id(&self.assistant_id);

        // Add optional run parameters (matching Python baseline)
        if let Some(instructions) = input.get("instructions").and_then(|v| v.as_str()) {
            run_builder.instructions(instructions);
        }
        if let Some(additional_instructions) = input
            .get("additional_instructions")
            .and_then(|v| v.as_str())
        {
            run_builder.additional_instructions(additional_instructions);
        }
        if let Some(model) = input.get("model").and_then(|v| v.as_str()) {
            run_builder.model(model);
        }
        if let Some(temp) = input.get("temperature").and_then(serde_json::Value::as_f64) {
            run_builder.temperature(temp as f32);
        }
        if let Some(top_p) = input.get("top_p").and_then(serde_json::Value::as_f64) {
            run_builder.top_p(top_p as f32);
        }
        if let Some(max_prompt_tokens) = input
            .get("max_prompt_tokens")
            .and_then(serde_json::Value::as_u64)
        {
            run_builder.max_prompt_tokens(max_prompt_tokens as u32);
        }
        if let Some(max_completion_tokens) = input
            .get("max_completion_tokens")
            .and_then(serde_json::Value::as_u64)
        {
            run_builder.max_completion_tokens(max_completion_tokens as u32);
        }
        if let Some(parallel_tool_calls) = input
            .get("parallel_tool_calls")
            .and_then(serde_json::Value::as_bool)
        {
            run_builder.parallel_tool_calls(parallel_tool_calls);
        }
        if let Some(run_metadata) = input.get("run_metadata") {
            if let Some(metadata_obj) = run_metadata.as_object() {
                let metadata: HashMap<String, Value> = metadata_obj.clone().into_iter().collect();
                run_builder.metadata(metadata);
            }
        }
        // Note: tools override not implemented yet - requires converting DashFlow tools to OpenAI format

        let run_request = run_builder
            .build()
            .map_err(|e| Error::Api(format!("Failed to build run: {e}")))?;

        let run = self
            .client
            .threads()
            .runs(thread_id)
            .create(run_request)
            .await
            .map_err(|e| Error::Api(format!("Failed to create run: {e}")))?;

        Ok(run)
    }

    /// Process a completed run and return the appropriate output
    async fn get_response(&self, run: RunObject) -> Result<AssistantOutput> {
        match run.status {
            RunStatus::Completed => {
                // Fetch messages from the thread
                let messages = self
                    .client
                    .threads()
                    .messages(&run.thread_id)
                    .list(&[("order", "asc"), ("limit", "100")])
                    .await
                    .map_err(|e| Error::Api(format!("Failed to list messages: {e}")))?;

                // Filter to messages from this run
                let new_messages: Vec<_> = messages
                    .data
                    .into_iter()
                    .filter(|msg| msg.run_id.as_deref() == Some(&run.id))
                    .collect();

                if !self.as_agent {
                    // Return raw messages
                    let message_values: Vec<Value> = new_messages
                        .iter()
                        .map(|msg| serde_json::to_value(msg).unwrap_or(Value::Null))
                        .collect();
                    return Ok(AssistantOutput::Messages(message_values));
                }

                // Extract text content from messages
                let mut answer_parts = Vec::new();
                for msg in &new_messages {
                    for content in &msg.content {
                        if let MessageContent::Text(text) = content {
                            answer_parts.push(text.text.value.clone());
                        }
                    }
                }

                let answer = answer_parts.join("\n");
                let mut return_values = HashMap::new();
                return_values.insert("output".to_string(), Value::String(answer.clone()));
                return_values.insert(
                    "thread_id".to_string(),
                    Value::String(run.thread_id.clone()),
                );
                return_values.insert("run_id".to_string(), Value::String(run.id.clone()));

                Ok(AssistantOutput::Finish(OpenAIAssistantFinish {
                    output: answer,
                    log: String::new(),
                    run_id: run.id,
                    thread_id: run.thread_id,
                    return_values,
                }))
            }
            RunStatus::RequiresAction => {
                // Handle required tool calls
                if let Some(required_action) = run.required_action {
                    let tool_calls = required_action.submit_tool_outputs.tool_calls;

                    if !self.as_agent {
                        // Return raw tool calls
                        let tool_call_values: Vec<Value> = tool_calls
                            .iter()
                            .map(|tc| serde_json::to_value(tc).unwrap_or(Value::Null))
                            .collect();
                        return Ok(AssistantOutput::ToolCalls(tool_call_values));
                    }

                    // Convert to OpenAIAssistantActions
                    let mut actions = Vec::new();
                    for tool_call in tool_calls {
                        let function = tool_call.function;

                        // Parse function arguments
                        let args: Value = serde_json::from_str(&function.arguments)
                            .unwrap_or_else(|_| Value::String(function.arguments.clone()));

                        // Handle __arg1 wrapper (from Python)
                        let tool_input = if let Some(obj) = args.as_object() {
                            if obj.len() == 1 && obj.contains_key("__arg1") {
                                ToolInput::from(obj["__arg1"].clone())
                            } else {
                                ToolInput::from(args)
                            }
                        } else {
                            ToolInput::from(args)
                        };

                        actions.push(OpenAIAssistantAction::new(
                            function.name,
                            tool_input,
                            String::new(),
                            tool_call.id,
                            run.id.clone(),
                            run.thread_id.clone(),
                        ));
                    }

                    Ok(AssistantOutput::Actions(actions))
                } else {
                    Err(Error::Api(
                        "Run requires action but no required_action field present".to_string(),
                    ))
                }
            }
            _ => Err(Error::Api(format!(
                "Unexpected run status: {:?}",
                run.status
            ))),
        }
    }
}

#[async_trait::async_trait]
impl Runnable for OpenAIAssistantRunnable {
    type Input = HashMap<String, Value>;
    type Output = AssistantOutput;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        // Determine what operation to perform based on input
        let run = if self.as_agent && input.contains_key("intermediate_steps") {
            // Being run within AgentExecutor - parse intermediate steps and submit tool outputs
            let intermediate_steps = input
                .get("intermediate_steps")
                .ok_or_else(|| Error::InvalidInput("Missing intermediate_steps".to_string()))?;
            let parsed = self.parse_intermediate_steps(intermediate_steps).await?;

            let run_id = parsed
                .get("run_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    Error::InvalidInput("Missing run_id from parsed steps".to_string())
                })?;
            let thread_id = parsed
                .get("thread_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    Error::InvalidInput("Missing thread_id from parsed steps".to_string())
                })?;

            let tool_outputs = parsed.get("tool_outputs").ok_or_else(|| {
                Error::InvalidInput("Missing tool_outputs from parsed steps".to_string())
            })?;

            let submit_request: SubmitToolOutputsRunRequest =
                serde_json::from_value(serde_json::json!({
                    "tool_outputs": tool_outputs,
                }))
                .map_err(|e| Error::Api(format!("Failed to parse tool outputs: {e}")))?;

            self.client
                .threads()
                .runs(thread_id)
                .submit_tool_outputs(run_id, submit_request)
                .await
                .map_err(|e| Error::Api(format!("Failed to submit tool outputs: {e}")))?
        } else if let Some(tool_outputs) = input.get("tool_outputs") {
            // Submitting tool outputs to existing run
            let run_id = input
                .get("run_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    Error::InvalidInput("Missing run_id for tool output submission".to_string())
                })?;
            let thread_id = input
                .get("thread_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    Error::InvalidInput("Missing thread_id for tool output submission".to_string())
                })?;

            // Parse tool outputs via JSON serialization (stable workaround)
            // Type-safe async-openai ToolOutput types: Deferred (low priority)
            // Reason: Current JSON ser/deser works reliably, type-safe version
            // requires API research into async-openai v0.25 ToolOutput improvements
            let submit_request: SubmitToolOutputsRunRequest =
                serde_json::from_value(serde_json::json!({
                    "tool_outputs": tool_outputs,
                }))
                .map_err(|e| Error::Api(format!("Failed to parse tool outputs: {e}")))?;

            self.client
                .threads()
                .runs(thread_id)
                .submit_tool_outputs(run_id, submit_request)
                .await
                .map_err(|e| Error::Api(format!("Failed to submit tool outputs: {e}")))?
        } else if let Some(thread_id) = input.get("thread_id").and_then(|v| v.as_str()) {
            // Existing thread - add message and create run
            let content = input
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::InvalidInput("Missing content field".to_string()))?;

            // Create message in thread
            let message = CreateMessageRequestArgs::default()
                .role(MessageRole::User)
                .content(content)
                .build()
                .map_err(|e| Error::Api(format!("Failed to build message: {e}")))?;

            self.client
                .threads()
                .messages(thread_id)
                .create(message)
                .await
                .map_err(|e| Error::Api(format!("Failed to create message: {e}")))?;

            // Create run
            self.create_run(thread_id, &input).await?
        } else {
            // New thread - create thread and run
            let content = input
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::InvalidInput("Missing content field".to_string()))?;

            self.create_thread_and_run(&input, content.to_string())
                .await?
        };

        // Wait for run to complete
        let completed_run = self.wait_for_run(&run.id, &run.thread_id).await?;

        // Get response
        self.get_response(completed_run).await
    }
}

impl Serializable for OpenAIAssistantRunnable {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "agents".to_string(),
            "openai_assistant".to_string(),
            "OpenAIAssistantRunnable".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        false // Contains client which cannot be serialized
    }

    fn to_json(&self) -> dashflow::core::serialization::SerializedObject {
        dashflow::core::serialization::SerializedObject::not_implemented("OpenAIAssistantRunnable")
    }

    fn lc_secrets(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assistant_action_creation() {
        let action = OpenAIAssistantAction::new(
            "calculator",
            ToolInput::from(serde_json::json!({"operation": "add", "a": 1, "b": 2})),
            "Need to add numbers",
            "call_123",
            "run_456",
            "thread_789",
        );

        assert_eq!(action.tool, "calculator");
        assert_eq!(action.tool_call_id, "call_123");
        assert_eq!(action.run_id, "run_456");
        assert_eq!(action.thread_id, "thread_789");
    }

    #[test]
    fn test_assistant_finish_creation() {
        let finish = OpenAIAssistantFinish::new(
            "The answer is 42",
            "Calculation complete",
            "run_123",
            "thread_456",
        );

        assert_eq!(finish.output, "The answer is 42");
        assert_eq!(finish.run_id, "run_123");
        assert_eq!(finish.thread_id, "thread_456");
    }

    #[test]
    fn test_action_to_agent_action() {
        let action = OpenAIAssistantAction::new(
            "search",
            ToolInput::from("test query"),
            "Searching for info",
            "call_1",
            "run_1",
            "thread_1",
        );

        let agent_action = action.to_agent_action();
        assert_eq!(agent_action.tool, "search");
        assert_eq!(agent_action.log, "Searching for info");
    }
}
