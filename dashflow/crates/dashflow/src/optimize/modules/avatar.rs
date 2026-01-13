// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for avatar module
// - needless_pass_by_value: Action tracking requires owned strings for history
// - clone_on_ref_ptr: Arc cloned for parallel action execution
#![allow(clippy::needless_pass_by_value, clippy::clone_on_ref_ptr)]

//! Avatar module - ReAct-style agent with explicit action tracking.
//!
//! Avatar is similar to ReAct but tracks actions more explicitly and allows
//! for optimization of instructions via the AvatarOptimizer. The agent:
//! 1. Reasons about the task and available tools
//! 2. Selects and calls tools iteratively
//! 3. Tracks each action (tool name, input, output)
//! 4. Produces final output when done
//!
//! This pattern enables automatic prompt engineering where the optimizer can
//! learn better instructions from positive/negative examples.

use crate::core::language_models::ChatModel;
use crate::core::tools::ToolInput;
use crate::optimize::llm_node::LLMNode;
use crate::optimize::modules::{SimpleTool, Tool};
use crate::optimize::{Field, FieldKind, Signature};
use crate::{GraphState, MergeableState, Node, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tracing;

/// An Avatar-specific tool wrapper with metadata.
///
/// Similar to the Tool trait but with additional metadata for Avatar's
/// explicit action tracking.
pub struct AvatarTool {
    /// The name of the tool
    pub name: String,
    /// Description of what the tool does
    pub description: String,
    /// Optional description of valid input format
    pub input_type: Option<String>,
    /// The actual tool implementation
    pub tool: Box<dyn Tool>,
}

impl fmt::Debug for AvatarTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AvatarTool")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("input_type", &self.input_type)
            .field("tool", &"<dyn Tool>")
            .finish()
    }
}

impl AvatarTool {
    /// Create a new AvatarTool
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_type: Option<String>,
        tool: Box<dyn Tool>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_type,
            tool,
        }
    }

    /// Get the tool's display string (name with optional input type hint)
    pub fn display_string(&self) -> String {
        if let Some(input_type) = &self.input_type {
            format!(
                "{}(valid_input: {}): {}",
                self.name, input_type, self.description
            )
        } else {
            format!("{}: {}", self.name, self.description)
        }
    }
}

/// An action to be taken by the Avatar agent.
///
/// Represents a single tool invocation with the tool name and input query.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Action {
    /// Name of the tool to use
    pub tool_name: String,
    /// Query/input to pass to the tool
    pub tool_input_query: String,
}

impl Action {
    /// Create a new Action
    pub fn new(tool_name: impl Into<String>, tool_input_query: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            tool_input_query: tool_input_query.into(),
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.tool_name, self.tool_input_query)
    }
}

/// The result of executing an action.
///
/// Contains the action taken plus the tool's output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActionOutput {
    /// Name of the tool that was called
    pub tool_name: String,
    /// Query that was passed to the tool
    pub tool_input_query: String,
    /// Output returned by the tool
    pub tool_output: String,
}

impl ActionOutput {
    /// Create a new ActionOutput
    pub fn new(
        tool_name: impl Into<String>,
        tool_input_query: impl Into<String>,
        tool_output: impl Into<String>,
    ) -> Self {
        Self {
            tool_name: tool_name.into(),
            tool_input_query: tool_input_query.into(),
            tool_output: tool_output.into(),
        }
    }

    /// Create from an Action and tool output
    pub fn from_action(action: &Action, tool_output: impl Into<String>) -> Self {
        Self {
            tool_name: action.tool_name.clone(),
            tool_input_query: action.tool_input_query.clone(),
            tool_output: tool_output.into(),
        }
    }
}

impl fmt::Display for ActionOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}({}) -> {}",
            self.tool_name, self.tool_input_query, self.tool_output
        )
    }
}

/// Helper to get ordinal suffix for numbers (1st, 2nd, 3rd, 4th, ...)
fn get_number_suffix(n: usize) -> String {
    match n % 10 {
        1 if n % 100 != 11 => format!("{}st", n),
        2 if n % 100 != 12 => format!("{}nd", n),
        3 if n % 100 != 13 => format!("{}rd", n),
        _ => format!("{}th", n),
    }
}

/// Avatar agent node.
///
/// Avatar is similar to ReAct but with explicit action tracking and support for
/// instruction optimization via AvatarOptimizer.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow::optimize::modules::{AvatarNode, AvatarTool, SimpleTool};
/// use dashflow::optimize::{Signature, Field, FieldKind};
/// use std::collections::HashMap;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create a calculator tool
/// let calc_tool = SimpleTool::new(
///     "calculator",
///     "Performs arithmetic calculations",
///     HashMap::from([("expression".to_string(), "Math expression".to_string())]),
///     |args| {
///         let expr = args["expression"].as_str().unwrap_or("0");
///         Ok(format!("Result: {}", expr))
///     },
/// );
///
/// let avatar_tool = AvatarTool::new(
///     "calculator",
///     "Performs arithmetic calculations",
///     Some("mathematical expression".to_string()),
///     Box::new(calc_tool),
/// );
///
/// // Create Avatar signature
/// let signature = Signature::new("Answer math questions")
///     .with_input(Field::new("question", "Math question to answer", FieldKind::Input))
///     .with_output(Field::new("answer", "The answer", FieldKind::Output));
///
/// // Create Avatar node
/// // let llm = ...; // ChatOpenAI or other LLM
/// // let avatar = AvatarNode::new(signature, vec![avatar_tool], 5, llm)?;
/// # Ok(())
/// # }
/// ```
pub struct AvatarNode<S: GraphState> {
    /// Original task signature (e.g., question -> answer)
    signature: Signature,

    /// Available tools (user tools + Finish tool)
    tools: HashMap<String, AvatarTool>,

    /// Actor LLM node for tool selection
    actor: LLMNode<S>,

    /// Clone of actor for reset after forward
    actor_clone: LLMNode<S>,

    /// Maximum number of tool iterations
    max_iters: usize,

    /// Whether to print verbose output
    verbose: bool,
}

impl<S: GraphState> AvatarNode<S> {
    /// Create a new Avatar agent.
    ///
    /// # Arguments
    /// * `signature` - The task signature (e.g., "question -> answer")
    /// * `tools` - List of tools the agent can use
    /// * `max_iters` - Maximum number of tool iterations (default: 3)
    /// * `llm` - Language model client
    pub fn new(
        signature: Signature,
        tools: Vec<AvatarTool>,
        max_iters: usize,
        llm: Arc<dyn ChatModel>,
    ) -> Result<Self> {
        // Create tools map
        let mut tools_map: HashMap<String, AvatarTool> =
            tools.into_iter().map(|t| (t.name.clone(), t)).collect();

        // Add the Finish tool
        let finish_tool = AvatarTool::new(
            "Finish",
            "returns the final output and finishes the task",
            None,
            Box::new(SimpleTool::new(
                "Finish",
                "Finish the task",
                HashMap::new(),
                |_| Ok("Task complete".to_string()),
            )),
        );
        tools_map.insert("Finish".to_string(), finish_tool);

        // Build the actor signature
        let actor_signature = Self::build_initial_actor_signature(&signature, &tools_map)?;

        // Create the actor LLM node
        let actor = LLMNode::new(actor_signature.clone(), llm.clone());
        let actor_clone = LLMNode::new(actor_signature, llm);

        Ok(Self {
            signature,
            tools: tools_map,
            actor,
            actor_clone,
            max_iters,
            verbose: false,
        })
    }

    /// Build the initial actor signature.
    ///
    /// Creates a signature with:
    /// - goal (input): Task description from signature instructions
    /// - tools (input): List of available tool names
    /// - Original signature inputs (e.g., question)
    /// - action_1 (output): First action to take
    fn build_initial_actor_signature(
        signature: &Signature,
        _tools: &HashMap<String, AvatarTool>,
    ) -> Result<Signature> {
        // Build instruction
        let instruction = "You will be given `Tools` which will be a list of tools to use to accomplish the `Goal`. \
            Given the user query, your task is to decide which tool to use and what input values to provide.\n\n\
            You will output action needed to accomplish the `Goal`. `Action` should have a tool to use and the input query to pass to the tool.\n\n\
            Note: You can opt to use no tools and provide the final answer directly. \
            You can also use one tool multiple times with different input queries if applicable.".to_string();

        // Start building signature
        let mut actor_sig = Signature::new("Actor");

        // Add goal field
        actor_sig = actor_sig.with_input(
            Field::new("goal", "Task to be accomplished.", FieldKind::Input).with_prefix("Goal:"),
        );

        // Add tools field (as list of tool names/descriptions)
        actor_sig = actor_sig.with_input(
            Field::new("tools", "list of tools to use", FieldKind::Input).with_prefix("Tools:"),
        );

        // Add original signature input fields
        for field in &signature.input_fields {
            let prefix = field.prefix.clone().unwrap_or_else(|| field.get_prefix());
            actor_sig = actor_sig.with_input(
                Field::new(&field.name, &field.description, FieldKind::Input).with_prefix(prefix),
            );
        }

        // Add action_1 output
        actor_sig = actor_sig.with_output(
            Field::new("action_1", "1st action to take. JSON object with fields: tool_name (string), tool_input_query (string)", FieldKind::Output)
                .with_prefix("Action 1:"),
        );

        // Set instructions
        actor_sig = actor_sig.with_instructions(&instruction);

        Ok(actor_sig)
    }

    /// Call a tool by name with the given input query.
    async fn call_tool(&self, tool_name: &str, tool_input_query: &str) -> Result<String> {
        use crate::error::Error;

        let tool = self
            .tools
            .get(tool_name)
            .ok_or_else(|| Error::Validation(format!("Unknown tool: {}", tool_name)))?;

        // Build args as JSON object
        let args = serde_json::json!({ "query": tool_input_query });

        // Call the tool and convert the CoreError to crate-level Error
        tool.tool
            ._call(ToolInput::Structured(args))
            .await
            .map_err(|e| Error::Generic(e.to_string()))
    }

    /// Update the actor signature after an action (static version for execute()).
    ///
    /// Adds:
    /// - action_N (input): The action that was taken
    /// - result_N (input): The result of that action
    /// - action_N+1 (output): The next action OR final outputs (if omit_action=true)
    fn update_signature(
        sig: &mut Signature,
        idx: usize,
        omit_action: bool,
        original_signature: &Signature,
    ) -> Result<()> {
        // Convert action_N from output to input
        let action_field_name = format!("action_{}", idx);
        sig.output_fields.retain(|f| f.name != action_field_name);
        sig.input_fields.push(
            Field::new(
                &action_field_name,
                format!("{} action taken", get_number_suffix(idx)),
                FieldKind::Input,
            )
            .with_prefix(format!("Action {}:", idx)),
        );

        // Add result_N input
        sig.input_fields.push(
            Field::new(
                format!("result_{}", idx),
                format!("{} result", get_number_suffix(idx)),
                FieldKind::Input,
            )
            .with_prefix(format!("Result {}:", idx)),
        );

        // Add next output field
        if omit_action {
            // Add original output fields instead of next action
            for field in &original_signature.output_fields {
                let prefix = field.prefix.clone().unwrap_or_else(|| field.get_prefix());
                sig.output_fields.push(
                    Field::new(&field.name, &field.description, FieldKind::Output)
                        .with_prefix(prefix),
                );
            }
        } else {
            // Add action_N+1 output
            let next_idx = idx + 1;
            sig.output_fields.push(
                Field::new(
                    format!("action_{}", next_idx),
                    format!("{} action to take. JSON object with fields: tool_name (string), tool_input_query (string)", get_number_suffix(next_idx)),
                    FieldKind::Output,
                )
                .with_prefix(format!("Action {}:", next_idx)),
            );
        }

        Ok(())
    }

    /// Update the actor signature after an action (mutable version for optimization).
    ///
    /// Planned for AvatarOptimizer integration to enable signature mutation during
    /// optimization passes. Currently the immutable version is used.
    #[allow(dead_code)] // Architectural: Reserved for AvatarOptimizer mutable signature updates
    fn update_actor_signature(&mut self, idx: usize, omit_action: bool) -> Result<()> {
        Self::update_signature(&mut self.actor.signature, idx, omit_action, &self.signature)
    }

    /// Enable or disable verbose output
    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
    }

    /// Get the original task signature
    pub fn signature(&self) -> &Signature {
        &self.signature
    }

    /// Get the number of available tools (including Finish)
    pub fn num_tools(&self) -> usize {
        self.tools.len()
    }

    /// Get reference to the actor LLM node.
    ///
    /// Used by AvatarOptimizer to access and modify the actor's signature.
    pub fn actor(&self) -> &LLMNode<S> {
        &self.actor
    }

    /// Get mutable reference to the actor LLM node.
    ///
    /// Used by AvatarOptimizer to modify the actor's signature during optimization.
    pub fn actor_mut(&mut self) -> &mut LLMNode<S> {
        &mut self.actor
    }

    /// Get the list of available tools as display strings.
    ///
    /// Used by AvatarOptimizer for feedback generation.
    pub fn tools_display(&self) -> Vec<String> {
        self.tools.values().map(|t| t.display_string()).collect()
    }

    /// Update the actor's instruction.
    ///
    /// Used by AvatarOptimizer to apply optimized instructions.
    pub fn update_instruction(&mut self, new_instruction: &str) -> Result<()> {
        // Update both actor and actor_clone
        self.actor.signature.instructions = new_instruction.to_string();
        self.actor.optimization_state.instruction = new_instruction.to_string();

        self.actor_clone.signature.instructions = new_instruction.to_string();
        self.actor_clone.optimization_state.instruction = new_instruction.to_string();

        Ok(())
    }

    /// Extract input fields from state (similar to LLMNode)
    fn extract_inputs(&self, state: &S) -> Result<HashMap<String, String>> {
        use crate::error::Error;

        let json = serde_json::to_value(state)
            .map_err(|e| Error::Validation(format!("Failed to serialize state: {}", e)))?;

        let obj = json.as_object().ok_or_else(|| {
            Error::Validation("State must serialize to a JSON object".to_string())
        })?;

        let mut inputs = HashMap::new();
        for field in &self.signature.input_fields {
            let value = obj
                .get(&field.name)
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    Error::Validation(format!(
                        "Input field '{}' not found or not a string in state",
                        field.name
                    ))
                })?
                .to_string();
            inputs.insert(field.name.clone(), value);
        }

        Ok(inputs)
    }

    /// Build prompt from signature and args (static version)
    fn build_prompt(signature: &Signature, args: &HashMap<String, String>) -> String {
        let mut prompt = String::new();

        // Add instruction
        if !signature.instructions.is_empty() {
            prompt.push_str(&signature.instructions);
            prompt.push_str("\n\n");
        }

        // Add input fields
        for field in &signature.input_fields {
            if let Some(value) = args.get(&field.name) {
                prompt.push_str(&format!("{}: {}\n", field.get_prefix(), value));
            }
        }

        // Add output field prompts
        prompt.push('\n');
        for field in &signature.output_fields {
            prompt.push_str(&format!("{}:\n", field.get_prefix()));
        }

        prompt
    }

    /// Extract a field from response text (static version)
    fn extract_field_static(
        signature: &Signature,
        response: &str,
        field_name: &str,
    ) -> Result<String> {
        use crate::error::Error;

        // Find the field in signature
        let field = signature
            .output_fields
            .iter()
            .find(|f| f.name == field_name)
            .ok_or_else(|| Error::Validation(format!("Field {} not found", field_name)))?;

        let prefix = field.get_prefix();

        // Try to find field by prefix (prefix already includes colon if needed)
        if let Some(idx) = response.find(&prefix) {
            let start = idx + prefix.len();
            let rest = &response[start..];

            // Find end (next field prefix or end of string)
            let mut end = rest.len();
            for other_field in &signature.output_fields {
                if other_field.name != field_name {
                    let other_prefix = other_field.get_prefix();
                    if let Some(next_idx) = rest.find(&other_prefix) {
                        end = end.min(next_idx);
                    }
                }
            }

            let value = rest[..end].trim();
            return Ok(value.to_string());
        }

        Err(Error::Validation(format!(
            "Could not find field '{}' (prefix: '{}') in response: {}",
            field_name, prefix, response
        )))
    }

    /// Parse output fields from final response (static version)
    fn parse_outputs_static(
        original_signature: &Signature,
        current_signature: &Signature,
        response: &str,
    ) -> Result<HashMap<String, String>> {
        let mut outputs = HashMap::new();

        for field in &original_signature.output_fields {
            let value = Self::extract_field_static(current_signature, response, &field.name)?;
            outputs.insert(field.name.clone(), value);
        }

        Ok(outputs)
    }

    /// Parse Action from LLM output string (handles JSON strings, markdown code blocks).
    fn parse_action_str(action_str: &str) -> Result<Action> {
        use crate::error::Error;

        // Strip markdown code blocks
        let cleaned = action_str
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        serde_json::from_str::<Action>(cleaned).map_err(|e| {
            Error::Validation(format!(
                "Failed to parse action from string. LLM returned: '{}' (cleaned: '{}')\n\nError: {}",
                action_str, cleaned, e
            ))
        })
    }
}

#[async_trait]
impl<S> Node<S> for AvatarNode<S>
where
    S: GraphState + MergeableState,
{
    async fn execute(&self, state: S) -> Result<S> {
        use crate::core::messages::Message;
        use crate::error::Error;

        if self.verbose {
            tracing::info!("Starting Avatar task");
        }

        // Extract input fields from state
        let inputs = self.extract_inputs(&state)?;

        // Build initial args as HashMap
        let mut args = HashMap::new();

        // Add goal
        args.insert("goal".to_string(), self.signature.instructions.clone());

        // Build tools list
        let tools_list: Vec<String> = self.tools.values().map(|t| t.display_string()).collect();
        args.insert("tools".to_string(), tools_list.join("\n"));

        // Add user inputs
        for (key, value) in inputs {
            args.insert(key, value);
        }

        // Track actions
        let mut action_results: Vec<ActionOutput> = Vec::new();
        let mut idx = 1;
        let mut tool_name = String::new();

        // Clone actor signature for modifications
        let mut current_signature = self.actor.signature.clone();

        // Main loop
        while tool_name != "Finish" && idx <= self.max_iters {
            // Build prompt from current signature and args
            let prompt = Self::build_prompt(&current_signature, &args);

            // Call LLM
            let messages = vec![Message::human(prompt)];
            let result = self
                .actor
                .llm
                .generate(&messages, None, None, None, None)
                .await
                .map_err(|e| Error::NodeExecution {
                    node: "AvatarNode".to_string(),
                    source: Box::new(e),
                })?;

            let response_text = result
                .generations
                .first()
                .ok_or_else(|| Error::NodeExecution {
                    node: "AvatarNode".to_string(),
                    source: Box::new(Error::Generic("LLM returned empty response".to_string())),
                })?
                .message
                .content()
                .as_text();

            // Parse action from response
            let action_field = format!("action_{}", idx);
            let action_str =
                Self::extract_field_static(&current_signature, &response_text, &action_field)?;

            // Parse Action (LLM might return JSON object or string)
            let action: Action = Self::parse_action_str(&action_str)?;

            tool_name = action.tool_name.clone();

            if self.verbose {
                tracing::debug!(idx, action = %action, "Action");
            }

            // Execute tool (unless Finish)
            if tool_name != "Finish" {
                let tool_output = self
                    .call_tool(&action.tool_name, &action.tool_input_query)
                    .await?;

                if self.verbose {
                    tracing::debug!(idx, result = %tool_output, "Tool result");
                }

                action_results.push(ActionOutput::from_action(&action, &tool_output));

                // Update signature and args for next iteration
                Self::update_signature(&mut current_signature, idx, false, &self.signature)?;
                args.insert(
                    format!("action_{}", idx),
                    serde_json::to_string(&action)
                        .map_err(|e| Error::Validation(format!("Serialize action: {}", e)))?,
                );
                args.insert(format!("result_{}", idx), tool_output);
            } else {
                // Finish tool - add output fields and break
                Self::update_signature(&mut current_signature, idx, true, &self.signature)?;
                args.insert(
                    format!("action_{}", idx),
                    serde_json::to_string(&action)
                        .map_err(|e| Error::Validation(format!("Serialize action: {}", e)))?,
                );
                args.insert(
                    format!("result_{}", idx),
                    "Gathered all information needed to finish the task.".to_string(),
                );
                break;
            }

            idx += 1;
        }

        // Final call to actor to extract outputs
        let prompt = Self::build_prompt(&current_signature, &args);
        let messages = vec![Message::human(prompt)];
        let result = self
            .actor
            .llm
            .generate(&messages, None, None, None, None)
            .await
            .map_err(|e| Error::NodeExecution {
                node: "AvatarNode".to_string(),
                source: Box::new(e),
            })?;

        let response_text = result
            .generations
            .first()
            .ok_or_else(|| Error::NodeExecution {
                node: "AvatarNode".to_string(),
                source: Box::new(Error::Generic("LLM returned empty response".to_string())),
            })?
            .message
            .content()
            .as_text();

        // Parse outputs
        let outputs =
            Self::parse_outputs_static(&self.signature, &current_signature, &response_text)?;

        // Merge output fields back into state using JSON round-trip
        let mut json = serde_json::to_value(&state)
            .map_err(|e| Error::Validation(format!("Failed to serialize state: {}", e)))?;

        let obj = json.as_object_mut().ok_or_else(|| {
            Error::Validation("State must serialize to a JSON object".to_string())
        })?;

        // Update output fields
        for (key, value) in outputs {
            obj.insert(key, serde_json::Value::String(value));
        }

        // Add actions to state
        let actions_value = serde_json::to_value(&action_results)
            .map_err(|e| Error::Validation(format!("Serialize actions: {}", e)))?;
        obj.insert("actions".to_string(), actions_value);

        let state = serde_json::from_value(json)
            .map_err(|e| Error::Validation(format!("Failed to deserialize state: {}", e)))?;

        if self.verbose {
            tracing::info!("Avatar task complete");
        }

        Ok(state)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl<S: GraphState> fmt::Display for AvatarNode<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Avatar(max_iters={}, tools={}, signature={})",
            self.max_iters,
            self.num_tools(),
            self.signature.instructions
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_creation() {
        let action = Action::new("search", "What is Rust?");
        assert_eq!(action.tool_name, "search");
        assert_eq!(action.tool_input_query, "What is Rust?");
    }

    #[test]
    fn test_action_display() {
        let action = Action::new("calculator", "2 + 2");
        assert_eq!(format!("{}", action), "calculator(2 + 2)");
    }

    #[test]
    fn test_action_output_creation() {
        let output = ActionOutput::new(
            "search",
            "Rust language",
            "Rust is a systems programming language",
        );
        assert_eq!(output.tool_name, "search");
        assert_eq!(output.tool_input_query, "Rust language");
        assert_eq!(output.tool_output, "Rust is a systems programming language");
    }

    #[test]
    fn test_action_output_from_action() {
        let action = Action::new("calculator", "2 + 2");
        let output = ActionOutput::from_action(&action, "4");
        assert_eq!(output.tool_name, "calculator");
        assert_eq!(output.tool_input_query, "2 + 2");
        assert_eq!(output.tool_output, "4");
    }

    #[test]
    fn test_action_output_display() {
        let output = ActionOutput::new("search", "test query", "test result");
        assert_eq!(format!("{}", output), "search(test query) -> test result");
    }

    #[test]
    fn test_number_suffix() {
        assert_eq!(get_number_suffix(1), "1st");
        assert_eq!(get_number_suffix(2), "2nd");
        assert_eq!(get_number_suffix(3), "3rd");
        assert_eq!(get_number_suffix(4), "4th");
        assert_eq!(get_number_suffix(11), "11th");
        assert_eq!(get_number_suffix(12), "12th");
        assert_eq!(get_number_suffix(13), "13th");
        assert_eq!(get_number_suffix(21), "21st");
        assert_eq!(get_number_suffix(22), "22nd");
        assert_eq!(get_number_suffix(23), "23rd");
    }

    #[test]
    fn test_avatar_tool_creation() {
        let tool = SimpleTool::new("test", "A test tool", HashMap::new(), |_| {
            Ok("result".to_string())
        });
        let avatar_tool = AvatarTool::new(
            "test",
            "A test tool",
            Some("string".to_string()),
            Box::new(tool),
        );

        assert_eq!(avatar_tool.name, "test");
        assert_eq!(avatar_tool.description, "A test tool");
        assert_eq!(avatar_tool.input_type, Some("string".to_string()));
    }

    #[test]
    fn test_avatar_tool_display_string() {
        let tool = SimpleTool::new("calc", "Calculator", HashMap::new(), |_| {
            Ok("42".to_string())
        });

        // With input_type
        let avatar_tool = AvatarTool::new(
            "calculator",
            "Performs calculations",
            Some("mathematical expression".to_string()),
            Box::new(tool.clone()),
        );
        assert_eq!(
            avatar_tool.display_string(),
            "calculator(valid_input: mathematical expression): Performs calculations"
        );

        // Without input_type
        let avatar_tool =
            AvatarTool::new("calculator", "Performs calculations", None, Box::new(tool));
        assert_eq!(
            avatar_tool.display_string(),
            "calculator: Performs calculations"
        );
    }

    #[test]
    fn test_action_serialization() {
        let action = Action::new("search", "test query");
        let json = serde_json::to_string(&action).unwrap();
        let deserialized: Action = serde_json::from_str(&json).unwrap();
        assert_eq!(action, deserialized);
    }

    #[test]
    fn test_action_output_serialization() {
        let output = ActionOutput::new("search", "test query", "test result");
        let json = serde_json::to_string(&output).unwrap();
        let deserialized: ActionOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(output, deserialized);
    }

    #[test]
    fn test_extract_field_static() {
        // Build a signature with action_1 output field
        let sig = Signature::new("test").with_output(
            Field::new("action_1", "First action", FieldKind::Output).with_prefix("Action 1:"),
        );

        // Test response with newline after prefix
        let response =
            "Action 1:\n{\"tool_name\": \"calculator\", \"tool_input_query\": \"2 + 2\"}";

        let result = AvatarNode::<()>::extract_field_static(&sig, response, "action_1");
        assert!(
            result.is_ok(),
            "Failed to extract field: {:?}",
            result.err()
        );

        let value = result.unwrap();
        assert_eq!(
            value,
            "{\"tool_name\": \"calculator\", \"tool_input_query\": \"2 + 2\"}"
        );
    }

    // Integration tests with mock LLM
    #[cfg(test)]
    mod integration {
        use super::*;
        use crate::core::language_models::ChatModel;
        use crate::optimize::Signature;
        use std::collections::HashMap;

        /// Mock LLM that returns predefined responses
        struct MockLLM {
            responses: Vec<String>,
            call_count: std::sync::Arc<std::sync::Mutex<usize>>,
        }

        impl MockLLM {
            fn new(responses: Vec<String>) -> Self {
                Self {
                    responses,
                    call_count: std::sync::Arc::new(std::sync::Mutex::new(0)),
                }
            }
        }

	        #[async_trait]
	        impl ChatModel for MockLLM {
	            async fn _generate(
	                &self,
	                _messages: &[crate::core::messages::BaseMessage],
	                _stop: Option<&[String]>,
	                _tools: Option<&[crate::core::language_models::ToolDefinition]>,
	                _tool_choice: Option<&crate::core::language_models::ToolChoice>,
	                _run_manager: Option<&crate::core::callbacks::CallbackManager>,
            ) -> crate::core::error::Result<crate::core::language_models::ChatResult> {
                let mut count = self.call_count.lock().unwrap();
                let response = self
                    .responses
                    .get(*count)
                    .unwrap_or(&self.responses[self.responses.len() - 1])
                    .clone();
                *count += 1;

                Ok(crate::core::language_models::ChatResult {
                    generations: vec![crate::core::language_models::ChatGeneration {
                        message: crate::core::messages::Message::AI {
                            content: response.into(),
                            tool_calls: Vec::new(),
                            invalid_tool_calls: Vec::new(),
                            usage_metadata: None,
                            fields: Default::default(),
                        },
                        generation_info: None,
                    }],
                    llm_output: None,
                })
            }

            fn llm_type(&self) -> &str {
                "mock"
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        #[tokio::test]
        async fn test_avatar_with_mock_llm() {
            use crate::state::MergeableState;
            use serde::{Deserialize, Serialize};

            // Define a simple state
            #[derive(Clone, Serialize, Deserialize)]
            struct TestState {
                question: String,
                answer: Option<String>,
                actions: Option<Vec<ActionOutput>>,
            }

            impl MergeableState for TestState {
                fn merge(&mut self, other: &Self) {
                    if other.answer.is_some() {
                        self.answer = other.answer.clone();
                    }
                    if other.actions.is_some() {
                        self.actions = other.actions.clone();
                    }
                }
            }

            // Create a calculator tool
            let calc_tool = SimpleTool::new(
                "calculator",
                "Performs calculations",
                HashMap::new(),
                |args| {
                    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("0");
                    Ok(format!("Result of {} is 42", query))
                },
            );

            let avatar_tool = AvatarTool::new(
                "calculator",
                "Performs arithmetic calculations",
                Some("mathematical expression".to_string()),
                Box::new(calc_tool),
            );

            // Create signature
            let signature = Signature::new("Answer math questions")
                .with_input(Field::new("question", "Math question", FieldKind::Input))
                .with_output(Field::new("answer", "The answer", FieldKind::Output));

            // Mock LLM responses:
            // 1. Action: use calculator
            // 2. Final answer after seeing result
            let mock_llm = Arc::new(MockLLM::new(vec![
                r#"Action 1:
{"tool_name": "calculator", "tool_input_query": "2 + 2"}"#
                    .to_string(),
                r#"Action 2:
{"tool_name": "Finish", "tool_input_query": "done"}"#
                    .to_string(),
                r#"Answer:
The answer is 42 based on the calculation."#
                    .to_string(),
            ]));

            // Create Avatar node
            let avatar = AvatarNode::new(signature, vec![avatar_tool], 3, mock_llm).unwrap();

            // Create initial state
            let state = TestState {
                question: "What is 2 + 2?".to_string(),
                answer: None,
                actions: None,
            };

            // Execute
            let result = avatar.execute(state).await.unwrap();

            // Verify answer was set
            assert!(result.answer.is_some());
            let answer = result.answer.unwrap();
            assert!(answer.contains("42"));

            // Verify actions were recorded
            assert!(result.actions.is_some());
            let actions = result.actions.unwrap();
            assert_eq!(actions.len(), 1); // One tool call (calculator)
            assert_eq!(actions[0].tool_name, "calculator");
            assert_eq!(actions[0].tool_input_query, "2 + 2");
        }
    }
}
