// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # ReAct Node
//!
//! A node that implements the Reasoning + Acting agent pattern. ReAct enables
//! LLMs to use tools, gather information, and solve multi-step tasks requiring
//! external interaction.
//!
//! ## Pattern
//! Each iteration:
//! 1. **Thought**: Model reasons about what to do next
//! 2. **Action**: Model selects a tool and provides arguments
//! 3. **Observation**: Tool executes and returns result
//!
//! Loop continues until model calls the "finish" tool or reaches max iterations.
//!
//! ## Reference
//! Based on "ReAct: Synergizing Reasoning and Acting in Language Models" (Yao et al. 2023)

use crate::core::error::Result as CoreResult;
use crate::core::language_models::ChatModel;
use crate::core::messages::Message;
use crate::core::tools::ToolInput;
use crate::node::Node;
use crate::optimize::{
    Field, FieldKind, Optimizable, OptimizationResult, OptimizationState, OptimizerConfig,
    Signature,
};
use crate::state::GraphState;
use crate::{Error, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tracing;

// Re-export core Tool trait for backwards compatibility
pub use crate::core::tools::Tool;

/// Type alias for tool function closures
pub type ToolFn = Arc<dyn Fn(&Value) -> Result<String> + Send + Sync>;

/// A simple tool implementation that wraps a closure.
///
/// This is useful for creating tools from functions without needing to
/// implement the full Tool trait.
#[derive(Clone)]
pub struct SimpleTool {
    name: String,
    description: String,
    parameters: HashMap<String, String>,
    func: ToolFn,
}

impl fmt::Debug for SimpleTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SimpleTool")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("parameters", &self.parameters)
            .finish()
    }
}

impl SimpleTool {
    /// Create a new SimpleTool
    ///
    /// # Arguments
    /// * `name` - The name of the tool
    /// * `description` - A description of what the tool does
    /// * `parameters` - Map of parameter names to descriptions
    /// * `func` - The function to execute when the tool is called
    pub fn new<F>(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: HashMap<String, String>,
        func: F,
    ) -> Self
    where
        F: Fn(&Value) -> Result<String> + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
            func: Arc::new(func),
        }
    }

    /// Get parameter descriptions as a map (for prompt building)
    ///
    /// This is a convenience method for ReAct-style agents that need
    /// parameter names and descriptions for prompt construction.
    pub fn parameters(&self) -> HashMap<String, String> {
        self.parameters.clone()
    }
}

#[async_trait]
impl Tool for SimpleTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn args_schema(&self) -> serde_json::Value {
        // Convert parameters HashMap to JSON Schema format
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for (param_name, param_desc) in &self.parameters {
            properties.insert(
                param_name.clone(),
                serde_json::json!({
                    "type": "string",
                    "description": param_desc
                }),
            );
            required.push(serde_json::Value::String(param_name.clone()));
        }

        serde_json::json!({
            "type": "object",
            "properties": properties,
            "required": required
        })
    }

    async fn _call(&self, input: ToolInput) -> CoreResult<String> {
        use crate::core::error::Error as CoreError;

        // Extract the Value from ToolInput
        let args = match input {
            ToolInput::String(s) => {
                // Try to parse as JSON, otherwise wrap in object
                serde_json::from_str(&s).unwrap_or_else(|_| serde_json::json!({"input": s}))
            }
            ToolInput::Structured(v) => v,
        };

        // Call the function and convert error types
        (self.func)(&args).map_err(|e| CoreError::tool_error(e.to_string()))
    }
}

/// ReAct node: Reasoning and Acting agent.
///
/// ReAct implements an iterative loop where the agent:
/// 1. Reasons about the current situation (thought)
/// 2. Decides which tool to call (tool selection)
/// 3. Calls the tool and observes the result (action/observation)
/// 4. Repeats until the task is complete
///
/// This pattern enables agents to use tools effectively while maintaining
/// a clear reasoning trace.
pub struct ReActNode<S: GraphState> {
    /// Original signature (input -> output)
    pub signature: Signature,

    /// Current optimization state (prompt, few-shot examples, etc.)
    pub optimization_state: OptimizationState,

    /// Available tools (user-provided tools only, finish tool added automatically)
    tools: HashMap<String, Arc<dyn Tool>>,

    /// Maximum number of reasoning iterations
    max_iters: usize,

    /// LLM client (e.g., ChatOpenAI)
    llm: Arc<dyn ChatModel>,

    /// Marker for state type
    _phantom: std::marker::PhantomData<S>,
}

impl<S: GraphState> ReActNode<S> {
    fn collect_tools(tools: Vec<Arc<dyn Tool>>) -> HashMap<String, Arc<dyn Tool>> {
        let mut tool_map: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        for tool in tools {
            let name = tool.name().to_string();
            if name == "finish" {
                tracing::warn!(
                    tool_name = %name,
                    "ReActNode reserves the 'finish' tool name; ignoring user tool"
                );
                continue;
            }
            if tool_map.contains_key(&name) {
                tracing::warn!(
                    tool_name = %name,
                    "Duplicate ReAct tool name; ignoring later tool"
                );
                continue;
            }
            tool_map.insert(name, tool);
        }
        tool_map
    }

    /// Create a new ReAct node with a signature, tools, and LLM client
    ///
    /// # Arguments
    /// * `signature` - The input/output signature for the task
    /// * `tools` - List of tools the agent can use
    /// * `max_iters` - Maximum number of reasoning iterations (default: 5)
    /// * `llm` - LLM client for predictions
    pub fn new(
        signature: Signature,
        tools: Vec<Arc<dyn Tool>>,
        max_iters: usize,
        llm: Arc<dyn ChatModel>,
    ) -> Self {
        let tool_map = Self::collect_tools(tools);

        // Build instruction based on signature
        let instruction = Self::build_instruction(&signature, &tool_map);

        Self {
            signature,
            optimization_state: OptimizationState::new(instruction),
            tools: tool_map,
            max_iters,
            llm,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Build the ReAct instruction from signature and tools
    fn build_instruction(signature: &Signature, tools: &HashMap<String, Arc<dyn Tool>>) -> String {
        let inputs = signature
            .input_fields
            .iter()
            .map(|f| format!("`{}`", f.name))
            .collect::<Vec<_>>()
            .join(", ");

        let outputs = signature
            .output_fields
            .iter()
            .map(|f| format!("`{}`", f.name))
            .collect::<Vec<_>>()
            .join(", ");

        let mut instr = vec![
            format!(
                "You are an Agent. You will be given the fields {} as input.",
                inputs
            ),
            format!(
                "Your goal is to use one or more tools to collect information for producing {}.",
                outputs
            ),
            String::new(),
            "In each turn, you will provide:".to_string(),
            "- next_thought: Your reasoning about the current situation".to_string(),
            "- next_tool_name: The tool to call".to_string(),
            "- next_tool_args: Arguments for the tool (as JSON)".to_string(),
            String::new(),
            "Available tools:".to_string(),
        ];

        // Add tool descriptions (sorted by name for deterministic output)
        let mut tools_sorted: Vec<_> = tools.iter().collect();
        tools_sorted.sort_by_key(|(name, _)| *name);

        for (idx, (_, tool)) in tools_sorted.iter().enumerate() {
            // Extract parameter info from args_schema (JSON Schema format)
            let params = Self::extract_params_from_schema(&tool.args_schema());
            let params_str = if params.is_empty() {
                "none".to_string()
            } else {
                params.join(", ")
            };
            instr.push(format!(
                "{}. {} - {} (params: {})",
                idx + 1,
                tool.name(),
                tool.description(),
                params_str
            ));
        }

        // Add finish tool description
        let finish_idx = tools_sorted.len() + 1;
        instr.push(format!(
            "{}. finish - Mark the task as complete when you have all information needed for {}",
            finish_idx, outputs
        ));

        instr.join("\n")
    }

    /// Extract parameter names and descriptions from a JSON Schema
    fn extract_params_from_schema(schema: &serde_json::Value) -> Vec<String> {
        let mut params = Vec::new();

        if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
            // Collect and sort for deterministic output
            let mut props: Vec<_> = properties.iter().collect();
            props.sort_by_key(|(name, _)| *name);

            for (name, prop) in props {
                let desc = prop
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("(no description)");
                params.push(format!("{}: {}", name, desc));
            }
        }

        params
    }

    /// Get the ReAct signature with trajectory field added
    fn get_react_signature(&self) -> Signature {
        // Add trajectory as input field
        let trajectory_field = Field {
            name: "trajectory".to_string(),
            description: "History of thoughts, actions, and observations".to_string(),
            kind: FieldKind::Input,
            prefix: Some("Trajectory".to_string()),
        };

        // Add output fields for next action
        let thought_field = Field {
            name: "next_thought".to_string(),
            description: "Your reasoning about what to do next".to_string(),
            kind: FieldKind::Output,
            prefix: Some("Thought".to_string()),
        };

        let tool_name_field = Field {
            name: "next_tool_name".to_string(),
            description: "The name of the tool to call".to_string(),
            kind: FieldKind::Output,
            prefix: Some("Tool".to_string()),
        };

        let tool_args_field = Field {
            name: "next_tool_args".to_string(),
            description: "The arguments for the tool (as JSON)".to_string(),
            kind: FieldKind::Output,
            prefix: Some("Args".to_string()),
        };

        let mut input_fields = self.signature.input_fields.clone();
        input_fields.push(trajectory_field);

        let output_fields = vec![thought_field, tool_name_field, tool_args_field];

        Signature {
            name: format!("{}_react", self.signature.name),
            instructions: self.optimization_state.instruction.clone(),
            input_fields,
            output_fields,
        }
    }

    /// Format the trajectory as a human-readable string
    fn format_trajectory(&self, trajectory: &[(String, String, String, String)]) -> String {
        if trajectory.is_empty() {
            return "No actions taken yet.".to_string();
        }

        let mut formatted = vec![];
        for (idx, (thought, tool_name, tool_args, observation)) in trajectory.iter().enumerate() {
            formatted.push(format!("Step {}:", idx + 1));
            formatted.push(format!("  Thought: {}", thought));
            formatted.push(format!("  Tool: {} {}", tool_name, tool_args));
            formatted.push(format!("  Observation: {}", observation));
        }
        formatted.join("\n")
    }

    /// Extract input field values from state
    fn extract_inputs(&self, state: &S) -> Result<HashMap<String, String>> {
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

    /// Build prompt for tool selection
    fn build_tool_prompt(&self, inputs: &HashMap<String, String>, trajectory: &str) -> String {
        let react_sig = self.get_react_signature();
        let mut prompt = String::new();

        // 1. Add instruction
        prompt.push_str(&self.optimization_state.instruction);
        prompt.push_str("\n\n");

        // 2. Add few-shot examples with trajectory formatting
        for example in &self.optimization_state.few_shot_examples {
            // Input fields
            for field in &self.signature.input_fields {
                if let Some(value) = example.input.get(&field.name).and_then(|v| v.as_str()) {
                    prompt.push_str(&format!("{}: {}\n", field.get_prefix(), value));
                }
            }

            // Trajectory (if present in example)
            if let Some(traj) = example.input.get("trajectory") {
                if let Some(traj_str) = traj.as_str() {
                    prompt.push_str(&format!("Trajectory: {}\n", traj_str));
                }
            }

            // Output fields (thought, tool, args)
            for field in &react_sig.output_fields {
                if let Some(value) = example.output.get(&field.name).and_then(|v| v.as_str()) {
                    prompt.push_str(&format!("{}: {}\n", field.get_prefix(), value));
                }
            }

            prompt.push('\n');
        }

        // 3. Add current inputs
        for field in &self.signature.input_fields {
            if let Some(value) = inputs.get(&field.name) {
                prompt.push_str(&format!("{}: {}\n", field.get_prefix(), value));
            }
        }

        // 4. Add trajectory
        prompt.push_str(&format!("Trajectory: {}\n", trajectory));

        // 5. Prompt for next thought
        prompt.push_str("Thought: ");

        prompt
    }

    /// Parse LLM response for tool selection
    fn parse_tool_response(&self, response: &str) -> Result<(String, String, Value)> {
        let lines: Vec<&str> = response.trim().split('\n').collect();

        let mut thought = String::new();
        let mut tool_name = String::new();
        let mut tool_args = Value::Object(serde_json::Map::new());

        for line in &lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Check for "Thought:", "Tool:", "Args:" prefixes
            if let Some(rest) = trimmed.strip_prefix("Thought:") {
                thought = rest.trim().to_string();
            } else if let Some(rest) = trimmed.strip_prefix("Tool:") {
                tool_name = rest.trim().to_string();
            } else if let Some(rest) = trimmed.strip_prefix("Args:") {
                // Try to parse as JSON
                let args_str = rest.trim();
                match serde_json::from_str::<Value>(args_str) {
                    Ok(Value::Object(obj)) => tool_args = Value::Object(obj),
                    Ok(_) | Err(_) => tool_args = Value::Object(serde_json::Map::new()),
                }
            } else if tool_name.is_empty() {
                if !thought.is_empty() {
                    thought.push(' ');
                }
                thought.push_str(trimmed);
            }
        }

        // Validation
        if thought.is_empty() {
            return Err(Error::Validation(
                "Failed to parse thought from ReAct response".to_string(),
            ));
        }
        if tool_name.is_empty() {
            // Default to finish if no tool specified
            tool_name = "finish".to_string();
        }

        Ok((thought, tool_name, tool_args))
    }

    /// Build prompt for final answer extraction
    fn build_extract_prompt(&self, inputs: &HashMap<String, String>, trajectory: &str) -> String {
        let outputs = self
            .signature
            .output_fields
            .iter()
            .map(|f| format!("`{}`", f.name))
            .collect::<Vec<_>>()
            .join(", ");

        let mut prompt = String::new();

        // Instruction
        prompt.push_str(&format!("Extract {} from the trajectory.\n\n", outputs));

        // Input fields
        for field in &self.signature.input_fields {
            if let Some(value) = inputs.get(&field.name) {
                prompt.push_str(&format!("{}: {}\n", field.get_prefix(), value));
            }
        }

        // Trajectory
        prompt.push_str(&format!("\nTrajectory:\n{}\n\n", trajectory));

        // Prompt for first output field
        if let Some(first_output) = self.signature.output_fields.first() {
            prompt.push_str(&format!("{}: ", first_output.get_prefix()));
        }

        prompt
    }

    /// Parse extraction response
    fn parse_extract_response(&self, response: &str) -> Result<HashMap<String, String>> {
        let lines: Vec<&str> = response.trim().split('\n').collect();
        let mut outputs = HashMap::new();

        for line in &lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Check for output field prefixes
            for field in &self.signature.output_fields {
                let prefix = format!("{}:", field.get_prefix());
                if trimmed.starts_with(&prefix) {
                    let value = trimmed[prefix.len()..].trim().to_string();
                    outputs.insert(field.name.clone(), value);
                    break;
                }
            }
        }

        // If no structured output found, use first line as first output field
        if outputs.is_empty() {
            if let Some(first_output) = self.signature.output_fields.first() {
                if let Some(first_line) = lines.first() {
                    outputs.insert(first_output.name.clone(), first_line.trim().to_string());
                }
            }
        }

        Ok(outputs)
    }

    /// Update state with output field values and trajectory
    fn update_state(
        &self,
        mut state: S,
        outputs: HashMap<String, String>,
        trajectory: String,
    ) -> Result<S> {
        let mut json = serde_json::to_value(&state)
            .map_err(|e| Error::Validation(format!("Failed to serialize state: {}", e)))?;

        let obj = json.as_object_mut().ok_or_else(|| {
            Error::Validation("State must serialize to a JSON object".to_string())
        })?;

        // Update output fields
        for (key, value) in outputs {
            obj.insert(key, Value::String(value));
        }

        // Add trajectory if state has trajectory field
        if obj.contains_key("trajectory") {
            obj.insert("trajectory".to_string(), Value::String(trajectory));
        }

        state = serde_json::from_value(json)
            .map_err(|e| Error::Validation(format!("Failed to deserialize state: {}", e)))?;

        Ok(state)
    }

    /// Execute a single tool call iteration
    async fn execute_iteration(
        &self,
        inputs: &HashMap<String, String>,
        trajectory: &[(String, String, String, String)],
    ) -> Result<(String, String, String, String)> {
        // Build prompt
        let trajectory_str = self.format_trajectory(trajectory);
        let prompt = self.build_tool_prompt(inputs, &trajectory_str);

        // Call LLM
        let messages = vec![Message::human(prompt)];
        let result = self
            .llm
            .generate(&messages, None, None, None, None)
            .await
            .map_err(|e| Error::NodeExecution {
                node: "ReActNode".to_string(),
                source: Box::new(e),
            })?;

        let response_text = result
            .generations
            .first()
            .ok_or_else(|| Error::NodeExecution {
                node: "ReActNode".to_string(),
                source: Box::new(Error::Generic("LLM returned empty response".to_string())),
            })?
            .message
            .content()
            .as_text();

        // Parse thought, tool name, tool args
        let (thought, tool_name, tool_args) = self.parse_tool_response(&response_text)?;

        // Execute tool
        let observation = if tool_name == "finish" {
            "Task marked as complete.".to_string()
        } else if let Some(tool) = self.tools.get(&tool_name) {
            tool._call(ToolInput::Structured(tool_args.clone()))
                .await
                .unwrap_or_else(|e| format!("Error calling {}: {}", tool_name, e))
        } else {
            format!("Error: Unknown tool '{}'", tool_name)
        };

        Ok((thought, tool_name, tool_args.to_string(), observation))
    }
}

#[async_trait]
impl<S: GraphState> Node<S> for ReActNode<S> {
    async fn execute(&self, state: S) -> Result<S> {
        // 1. Extract input fields from state
        let inputs = self.extract_inputs(&state)?;

        // 2. Initialize trajectory
        let mut trajectory: Vec<(String, String, String, String)> = Vec::new();

        // 3. Main ReAct loop
        for _iter in 0..self.max_iters {
            let (thought, tool_name, tool_args, observation) =
                self.execute_iteration(&inputs, &trajectory).await?;

            trajectory.push((thought, tool_name.clone(), tool_args, observation));

            // Check if done
            if tool_name == "finish" {
                break;
            }
        }

        // 4. Extract final answer from trajectory
        let trajectory_str = self.format_trajectory(&trajectory);
        let extract_prompt = self.build_extract_prompt(&inputs, &trajectory_str);

        let messages = vec![Message::human(extract_prompt)];
        let result = self
            .llm
            .generate(&messages, None, None, None, None)
            .await
            .map_err(|e| Error::NodeExecution {
                node: "ReActNode".to_string(),
                source: Box::new(e),
            })?;

        let response_text = result
            .generations
            .first()
            .ok_or_else(|| Error::NodeExecution {
                node: "ReActNode".to_string(),
                source: Box::new(Error::Generic("LLM returned empty response".to_string())),
            })?
            .message
            .content()
            .as_text();
        let outputs = self.parse_extract_response(&response_text)?;

        // 5. Update state with outputs and trajectory
        self.update_state(state, outputs, trajectory_str)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn is_optimizable(&self) -> bool {
        true
    }

    fn may_use_llm(&self) -> bool {
        true
    }
}

#[async_trait]
impl<S: GraphState> Optimizable<S> for ReActNode<S> {
    async fn optimize(
        &mut self,
        examples: &[S],
        metric: &crate::optimize::MetricFn<S>,
        config: &OptimizerConfig,
    ) -> Result<OptimizationResult> {
        use crate::optimize::optimizers::BootstrapFewShot;
        use std::time::Instant;

        let start = Instant::now();

        if examples.is_empty() {
            return Err(Error::Validation(
                "Cannot optimize with empty training set".to_string(),
            ));
        }

        // 1. Evaluate initial score
        let initial_score = self.evaluate_score(examples, metric).await.map_err(|e| {
            Error::Validation(format!("ReAct initial score evaluation failed: {}", e))
        })?;

        tracing::debug!(
            score_pct = %format!("{:.2}%", initial_score * 100.0),
            correct = (initial_score * examples.len() as f64) as usize,
            total = examples.len(),
            "ReAct initial score"
        );

        // 2. Create BootstrapFewShot optimizer
        let optimizer = BootstrapFewShot::new().with_config(config.clone());

        // 3. Bootstrap demonstrations
        let demos = optimizer
            .bootstrap(self, examples, metric)
            .await
            .map_err(|e| Error::Validation(format!("ReAct demo bootstrap failed: {}", e)))?;

        // 4. Update optimization state
        self.optimization_state.few_shot_examples = demos;
        self.optimization_state
            .metadata
            .insert("optimizer".to_string(), "BootstrapFewShot".to_string());
        self.optimization_state
            .metadata
            .insert("timestamp".to_string(), chrono::Utc::now().to_rfc3339());
        self.optimization_state.metadata.insert(
            "num_demos".to_string(),
            self.optimization_state.few_shot_examples.len().to_string(),
        );

        // 5. Evaluate final score
        let final_score = self.evaluate_score(examples, metric).await.map_err(|e| {
            Error::Validation(format!("ReAct final score evaluation failed: {}", e))
        })?;

        tracing::debug!(
            score_pct = %format!("{:.2}%", final_score * 100.0),
            correct = (final_score * examples.len() as f64) as usize,
            total = examples.len(),
            "ReAct final score"
        );

        let duration = start.elapsed();
        let improvement = final_score - initial_score;
        let converged = improvement >= config.min_improvement;

        tracing::info!(
            status = if converged { "converged" } else { "complete" },
            duration_secs = %format!("{:.1}", duration.as_secs_f64()),
            improvement_pct = %format!("{:+.1}%", improvement * 100.0),
            "ReAct optimization complete"
        );

        Ok(OptimizationResult {
            initial_score,
            final_score,
            iterations: 1,
            converged,
            duration_secs: duration.as_secs_f64(),
        })
    }

    fn get_optimization_state(&self) -> OptimizationState {
        self.optimization_state.clone()
    }

    fn set_optimization_state(&mut self, state: OptimizationState) {
        self.optimization_state = state;
    }
}

impl<S: GraphState> ReActNode<S> {
    /// Evaluate the node's performance on a set of examples
    async fn evaluate_score(
        &self,
        examples: &[S],
        metric: &crate::optimize::MetricFn<S>,
    ) -> Result<f64> {
        if examples.is_empty() {
            return Ok(0.0);
        }

        let mut total_score = 0.0;
        let mut count = 0;

        for example in examples {
            if let Ok(prediction) = self.execute(example.clone()).await {
                if let Ok(score) = metric(example, &prediction) {
                    total_score += score;
                    count += 1;
                }
            }
        }

        if count == 0 {
            Ok(0.0)
        } else {
            Ok(total_score / count as f64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::MergeableState;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestState {
        question: String,
        #[serde(default)]
        answer: String,
        #[serde(default)]
        trajectory: String,
    }

    impl crate::state::MergeableState for TestState {
        fn merge(&mut self, other: &Self) {
            if !other.question.is_empty() {
                self.question = other.question.clone();
            }
            if !other.answer.is_empty() {
                self.answer = other.answer.clone();
            }
            if !other.trajectory.is_empty() {
                self.trajectory = other.trajectory.clone();
            }
        }
    }

    #[test]
    fn test_simple_tool_creation() {
        let tool = SimpleTool::new(
            "test_tool",
            "A test tool",
            HashMap::from([("arg1".to_string(), "First argument".to_string())]),
            |args| Ok(format!("Result: {}", args)),
        );

        assert_eq!(tool.name(), "test_tool");
        assert_eq!(tool.description(), "A test tool");
        assert_eq!(tool.parameters().len(), 1);
    }

    #[test]
    fn test_simple_tool_call() {
        use tokio::runtime::Runtime;

        let tool = SimpleTool::new(
            "echo",
            "Echo the input",
            HashMap::from([("message".to_string(), "Message to echo".to_string())]),
            |args| {
                let msg = args["message"].as_str().unwrap_or("(empty)");
                Ok(format!("Echo: {}", msg))
            },
        );

        let rt = Runtime::new().unwrap();
        let result = rt
            .block_on(tool._call(ToolInput::Structured(serde_json::json!({"message": "Hello"}))))
            .unwrap();
        assert_eq!(result, "Echo: Hello");
    }

    #[test]
    fn test_react_instruction_building() {
        let signature = Signature::new("QA")
            .with_input(Field::input("question", "The user's question"))
            .with_output(Field::output("answer", "The answer"));

        let weather_tool = Arc::new(SimpleTool::new(
            "get_weather",
            "Get weather for a city",
            HashMap::from([("city".to_string(), "City name".to_string())]),
            |args| {
                let city = args["city"].as_str().unwrap_or("Unknown");
                Ok(format!("Sunny in {}", city))
            },
        )) as Arc<dyn Tool>;

        let tools = vec![weather_tool.clone()];
        let tool_map: HashMap<String, Arc<dyn Tool>> = tools
            .into_iter()
            .map(|t| (t.name().to_string(), t))
            .collect();

        let instruction = ReActNode::<TestState>::build_instruction(&signature, &tool_map);

        assert!(instruction.contains("You are an Agent"));
        assert!(instruction.contains("`question`"));
        assert!(instruction.contains("`answer`"));
        assert!(instruction.contains("get_weather"));
        assert!(instruction.contains("finish"));
    }

    #[test]
    fn test_trajectory_formatting() {
        let signature = Signature::new("QA")
            .with_input(Field::input("question", "The user's question"))
            .with_output(Field::output("answer", "The answer"));

        // Create a mock LLM (we won't use it in this test)
        let llm = Arc::new(crate::core::language_models::FakeChatModel::new(vec![
            "test".to_string(),
        ])) as Arc<dyn ChatModel>;

        let react = ReActNode::<TestState>::new(signature, vec![], 5, llm);

        let trajectory = vec![
            (
                "I need to search".to_string(),
                "search".to_string(),
                r#"{"query": "test"}"#.to_string(),
                "Found results".to_string(),
            ),
            (
                "Now I'll finish".to_string(),
                "finish".to_string(),
                "{}".to_string(),
                "Done".to_string(),
            ),
        ];

        let formatted = react.format_trajectory(&trajectory);
        assert!(formatted.contains("Step 1:"));
        assert!(formatted.contains("Step 2:"));
        assert!(formatted.contains("I need to search"));
        assert!(formatted.contains("Found results"));
    }

    // ============================================================================
    // SimpleTool Tests
    // ============================================================================

    #[test]
    fn test_simple_tool_debug() {
        let tool = SimpleTool::new(
            "debug_tool",
            "A debug tool",
            HashMap::from([("x".to_string(), "X value".to_string())]),
            |_| Ok("ok".to_string()),
        );

        let debug_str = format!("{:?}", tool);
        assert!(debug_str.contains("SimpleTool"));
        assert!(debug_str.contains("debug_tool"));
    }

    #[test]
    fn test_simple_tool_empty_parameters() {
        let tool = SimpleTool::new(
            "no_params",
            "Tool without parameters",
            HashMap::new(),
            |_| Ok("done".to_string()),
        );

        assert_eq!(tool.name(), "no_params");
        assert!(tool.parameters().is_empty());
    }

    #[test]
    fn test_simple_tool_multiple_parameters() {
        let params = HashMap::from([
            ("param1".to_string(), "First parameter".to_string()),
            ("param2".to_string(), "Second parameter".to_string()),
            ("param3".to_string(), "Third parameter".to_string()),
        ]);

        let tool = SimpleTool::new(
            "multi_param",
            "Tool with many params",
            params.clone(),
            |_| Ok("result".to_string()),
        );

        assert_eq!(tool.parameters().len(), 3);
        assert!(tool.parameters().contains_key("param1"));
        assert!(tool.parameters().contains_key("param2"));
        assert!(tool.parameters().contains_key("param3"));
    }

    #[tokio::test]
    async fn test_simple_tool_returns_error() {
        let tool = SimpleTool::new(
            "error_tool",
            "Tool that returns error",
            HashMap::new(),
            |_| Err(crate::Error::Validation("Tool error".to_string())),
        );

        let result = tool._call(ToolInput::Structured(serde_json::json!({}))).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_simple_tool_with_complex_args() {
        let tool = SimpleTool::new(
            "complex",
            "Tool with complex args",
            HashMap::from([("data".to_string(), "JSON data".to_string())]),
            |args| {
                let data = &args["data"];
                if data.is_object() {
                    Ok("object".to_string())
                } else if data.is_array() {
                    Ok("array".to_string())
                } else {
                    Ok("other".to_string())
                }
            },
        );

        let result1 = tool
            ._call(ToolInput::Structured(serde_json::json!({"data": {"key": "value"}})))
            .await
            .unwrap();
        assert_eq!(result1, "object");

        let result2 = tool
            ._call(ToolInput::Structured(serde_json::json!({"data": [1, 2, 3]})))
            .await
            .unwrap();
        assert_eq!(result2, "array");
    }

    // ============================================================================
    // Trajectory Formatting Tests
    // ============================================================================

    #[test]
    fn test_format_empty_trajectory() {
        let signature = Signature::new("Test")
            .with_input(Field::input("input", "Input"))
            .with_output(Field::output("output", "Output"));

        let llm = Arc::new(crate::core::language_models::FakeChatModel::new(vec![
            "test".to_string(),
        ])) as Arc<dyn ChatModel>;

        let react = ReActNode::<TestState>::new(signature, vec![], 5, llm);
        let formatted = react.format_trajectory(&[]);

        assert_eq!(formatted, "No actions taken yet.");
    }

    #[test]
    fn test_format_single_step_trajectory() {
        let signature = Signature::new("Test")
            .with_input(Field::input("input", "Input"))
            .with_output(Field::output("output", "Output"));

        let llm = Arc::new(crate::core::language_models::FakeChatModel::new(vec![
            "test".to_string(),
        ])) as Arc<dyn ChatModel>;

        let react = ReActNode::<TestState>::new(signature, vec![], 5, llm);

        let trajectory = vec![(
            "thinking".to_string(),
            "action".to_string(),
            "{}".to_string(),
            "result".to_string(),
        )];

        let formatted = react.format_trajectory(&trajectory);
        assert!(formatted.contains("Step 1:"));
        assert!(formatted.contains("Thought: thinking"));
        assert!(formatted.contains("Tool: action"));
        assert!(formatted.contains("Observation: result"));
        assert!(!formatted.contains("Step 2:"));
    }

    // ============================================================================
    // Tool Response Parsing Tests
    // ============================================================================

    #[test]
    fn test_parse_tool_response_complete() {
        let signature = Signature::new("Test")
            .with_input(Field::input("q", "Question"))
            .with_output(Field::output("a", "Answer"));

        let llm = Arc::new(crate::core::language_models::FakeChatModel::new(vec![
            "test".to_string(),
        ])) as Arc<dyn ChatModel>;

        let react = ReActNode::<TestState>::new(signature, vec![], 5, llm);

        let response = "Thought: I need to search\nTool: search\nArgs: {\"query\": \"test\"}";
        let (thought, tool_name, tool_args) = react.parse_tool_response(response).unwrap();

        assert_eq!(thought, "I need to search");
        assert_eq!(tool_name, "search");
        assert_eq!(tool_args["query"], "test");
    }

    #[test]
    fn test_parse_tool_response_defaults_to_finish() {
        let signature = Signature::new("Test")
            .with_input(Field::input("q", "Question"))
            .with_output(Field::output("a", "Answer"));

        let llm = Arc::new(crate::core::language_models::FakeChatModel::new(vec![
            "test".to_string(),
        ])) as Arc<dyn ChatModel>;

        let react = ReActNode::<TestState>::new(signature, vec![], 5, llm);

        let response = "Thought: I have all the information I need";
        let (thought, tool_name, _) = react.parse_tool_response(response).unwrap();

        assert_eq!(thought, "I have all the information I need");
        assert_eq!(tool_name, "finish"); // Default when no tool specified
    }

    #[test]
    fn test_parse_tool_response_invalid_json_args() {
        let signature = Signature::new("Test")
            .with_input(Field::input("q", "Question"))
            .with_output(Field::output("a", "Answer"));

        let llm = Arc::new(crate::core::language_models::FakeChatModel::new(vec![
            "test".to_string(),
        ])) as Arc<dyn ChatModel>;

        let react = ReActNode::<TestState>::new(signature, vec![], 5, llm);

        let response = "Thought: testing\nTool: my_tool\nArgs: not valid json";
        let result = react.parse_tool_response(response);

        // Should succeed with empty object for invalid JSON
        assert!(result.is_ok());
        let (_, _, args) = result.unwrap();
        assert!(args.is_object());
    }

    // ============================================================================
    // Extract Response Parsing Tests
    // ============================================================================

    #[test]
    fn test_parse_extract_response() {
        let signature = Signature::new("QA")
            .with_input(Field::input("question", "The question"))
            .with_output(Field::output("answer", "The answer"));

        let llm = Arc::new(crate::core::language_models::FakeChatModel::new(vec![
            "test".to_string(),
        ])) as Arc<dyn ChatModel>;

        let react = ReActNode::<TestState>::new(signature, vec![], 5, llm);

        let response = "Answer: Paris is the capital of France";
        let outputs = react.parse_extract_response(response).unwrap();

        assert_eq!(
            outputs.get("answer").unwrap(),
            "Paris is the capital of France"
        );
    }

    #[test]
    fn test_parse_extract_response_no_prefix() {
        let signature = Signature::new("QA")
            .with_input(Field::input("question", "The question"))
            .with_output(Field::output("answer", "The answer"));

        let llm = Arc::new(crate::core::language_models::FakeChatModel::new(vec![
            "test".to_string(),
        ])) as Arc<dyn ChatModel>;

        let react = ReActNode::<TestState>::new(signature, vec![], 5, llm);

        let response = "Just the answer without prefix";
        let outputs = react.parse_extract_response(response).unwrap();

        // Should use first line as first output field
        assert_eq!(
            outputs.get("answer").unwrap(),
            "Just the answer without prefix"
        );
    }

    // ============================================================================
    // React Signature Tests
    // ============================================================================

    #[test]
    fn test_react_signature_has_trajectory() {
        let signature = Signature::new("QA")
            .with_input(Field::input("question", "The question"))
            .with_output(Field::output("answer", "The answer"));

        let llm = Arc::new(crate::core::language_models::FakeChatModel::new(vec![
            "test".to_string(),
        ])) as Arc<dyn ChatModel>;

        let react = ReActNode::<TestState>::new(signature, vec![], 5, llm);
        let react_sig = react.get_react_signature();

        // Should have original input fields plus trajectory
        assert_eq!(react_sig.input_fields.len(), 2);
        assert!(react_sig.input_fields.iter().any(|f| f.name == "question"));
        assert!(react_sig
            .input_fields
            .iter()
            .any(|f| f.name == "trajectory"));

        // Should have thought, tool_name, tool_args as outputs
        assert_eq!(react_sig.output_fields.len(), 3);
        assert!(react_sig
            .output_fields
            .iter()
            .any(|f| f.name == "next_thought"));
        assert!(react_sig
            .output_fields
            .iter()
            .any(|f| f.name == "next_tool_name"));
        assert!(react_sig
            .output_fields
            .iter()
            .any(|f| f.name == "next_tool_args"));
    }

    // ============================================================================
    // Instruction Building Tests
    // ============================================================================

    #[test]
    fn test_build_instruction_no_tools() {
        let signature = Signature::new("Simple")
            .with_input(Field::input("input", "Input"))
            .with_output(Field::output("output", "Output"));

        let tool_map: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let instruction = ReActNode::<TestState>::build_instruction(&signature, &tool_map);

        assert!(instruction.contains("`input`"));
        assert!(instruction.contains("`output`"));
        assert!(instruction.contains("finish")); // finish tool always present
    }

    #[test]
    fn test_build_instruction_multiple_tools() {
        let signature = Signature::new("Multi")
            .with_input(Field::input("query", "Search query"))
            .with_output(Field::output("result", "Search result"));

        let tool1 = Arc::new(SimpleTool::new(
            "search",
            "Search the web",
            HashMap::from([("q".to_string(), "query".to_string())]),
            |_| Ok("results".to_string()),
        )) as Arc<dyn Tool>;

        let tool2 = Arc::new(SimpleTool::new(
            "calculate",
            "Do math",
            HashMap::from([("expr".to_string(), "expression".to_string())]),
            |_| Ok("42".to_string()),
        )) as Arc<dyn Tool>;

        let tool_map: HashMap<String, Arc<dyn Tool>> = vec![tool1, tool2]
            .into_iter()
            .map(|t| (t.name().to_string(), t))
            .collect();

        let instruction = ReActNode::<TestState>::build_instruction(&signature, &tool_map);

        assert!(instruction.contains("search"));
        assert!(instruction.contains("calculate"));
        assert!(instruction.contains("Search the web"));
        assert!(instruction.contains("Do math"));
    }

    #[test]
    fn test_build_instruction_deterministic_order_and_params() {
        let signature = Signature::new("Multi")
            .with_input(Field::input("query", "Search query"))
            .with_output(Field::output("result", "Search result"));

        let tool1 = Arc::new(SimpleTool::new(
            "search",
            "Search the web",
            HashMap::from([
                ("b".to_string(), "second".to_string()),
                ("a".to_string(), "first".to_string()),
            ]),
            |_| Ok("results".to_string()),
        )) as Arc<dyn Tool>;

        let tool2 = Arc::new(SimpleTool::new(
            "calculate",
            "Do math",
            HashMap::from([("expr".to_string(), "expression".to_string())]),
            |_| Ok("42".to_string()),
        )) as Arc<dyn Tool>;

        let tool_map: HashMap<String, Arc<dyn Tool>> = vec![tool1, tool2]
            .into_iter()
            .map(|t| (t.name().to_string(), t))
            .collect();

        let instruction = ReActNode::<TestState>::build_instruction(&signature, &tool_map);

        let calculate_pos = instruction.find("1. calculate").unwrap();
        let search_pos = instruction.find("2. search").unwrap();
        assert!(calculate_pos < search_pos);
        assert!(instruction.contains("params: a: first, b: second"));
    }

    #[test]
    fn test_finish_tool_name_is_reserved() {
        let user_finish = Arc::new(SimpleTool::new(
            "finish",
            "User-provided finish should be ignored",
            HashMap::new(),
            |_| Ok("should not be called".to_string()),
        )) as Arc<dyn Tool>;

        let tool_map = ReActNode::<TestState>::collect_tools(vec![user_finish]);
        assert!(!tool_map.contains_key("finish"));
    }

    // ============================================================================
    // State Extraction Tests
    // ============================================================================

    #[test]
    fn test_extract_inputs_success() {
        let signature = Signature::new("QA")
            .with_input(Field::input("question", "The question"))
            .with_output(Field::output("answer", "The answer"));

        let llm = Arc::new(crate::core::language_models::FakeChatModel::new(vec![
            "test".to_string(),
        ])) as Arc<dyn ChatModel>;

        let react = ReActNode::<TestState>::new(signature, vec![], 5, llm);

        let state = TestState {
            question: "What is 2+2?".to_string(),
            answer: String::new(),
            trajectory: String::new(),
        };

        let inputs = react.extract_inputs(&state).unwrap();
        assert_eq!(inputs.get("question").unwrap(), "What is 2+2?");
    }

    // ============================================================================
    // State Update Tests
    // ============================================================================

    #[test]
    fn test_update_state_with_trajectory() {
        let signature = Signature::new("QA")
            .with_input(Field::input("question", "The question"))
            .with_output(Field::output("answer", "The answer"));

        let llm = Arc::new(crate::core::language_models::FakeChatModel::new(vec![
            "test".to_string(),
        ])) as Arc<dyn ChatModel>;

        let react = ReActNode::<TestState>::new(signature, vec![], 5, llm);

        let initial_state = TestState {
            question: "Test question".to_string(),
            answer: String::new(),
            trajectory: String::new(),
        };

        let mut outputs = HashMap::new();
        outputs.insert("answer".to_string(), "Test answer".to_string());

        let updated = react
            .update_state(initial_state, outputs, "Test trajectory".to_string())
            .unwrap();

        assert_eq!(updated.answer, "Test answer");
        assert_eq!(updated.trajectory, "Test trajectory");
    }

    // ============================================================================
    // TestState Tests
    // ============================================================================

    #[test]
    fn test_test_state_serialization() {
        let state = TestState {
            question: "What is AI?".to_string(),
            answer: "Artificial Intelligence".to_string(),
            trajectory: "Step 1: research".to_string(),
        };

        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["question"], "What is AI?");
        assert_eq!(json["answer"], "Artificial Intelligence");
        assert_eq!(json["trajectory"], "Step 1: research");
    }

    #[test]
    fn test_test_state_deserialization() {
        let json = serde_json::json!({
            "question": "How does it work?",
            "answer": "Through algorithms",
            "trajectory": ""
        });

        let state: TestState = serde_json::from_value(json).unwrap();
        assert_eq!(state.question, "How does it work?");
        assert_eq!(state.answer, "Through algorithms");
    }

    #[test]
    fn test_test_state_merge() {
        let mut state1 = TestState {
            question: "Original question".to_string(),
            answer: "".to_string(),
            trajectory: "".to_string(),
        };

        let state2 = TestState {
            question: "".to_string(),
            answer: "New answer".to_string(),
            trajectory: "New trajectory".to_string(),
        };

        state1.merge(&state2);

        assert_eq!(state1.question, "Original question"); // Not overwritten
        assert_eq!(state1.answer, "New answer");
        assert_eq!(state1.trajectory, "New trajectory");
    }

    // ============================================================================
    // Prompt Building Tests
    // ============================================================================

    #[test]
    fn test_build_tool_prompt_structure() {
        let signature = Signature::new("QA")
            .with_input(Field::input("question", "The question"))
            .with_output(Field::output("answer", "The answer"));

        let llm = Arc::new(crate::core::language_models::FakeChatModel::new(vec![
            "test".to_string(),
        ])) as Arc<dyn ChatModel>;

        let react = ReActNode::<TestState>::new(signature, vec![], 5, llm);

        let mut inputs = HashMap::new();
        inputs.insert("question".to_string(), "What is 2+2?".to_string());

        let prompt = react.build_tool_prompt(&inputs, "No actions taken yet.");

        assert!(prompt.contains("Question: What is 2+2?"));
        assert!(prompt.contains("Trajectory: No actions taken yet."));
        assert!(prompt.ends_with("Thought: "));
    }

    #[test]
    fn test_build_extract_prompt_structure() {
        let signature = Signature::new("QA")
            .with_input(Field::input("question", "The question"))
            .with_output(Field::output("answer", "The answer"));

        let llm = Arc::new(crate::core::language_models::FakeChatModel::new(vec![
            "test".to_string(),
        ])) as Arc<dyn ChatModel>;

        let react = ReActNode::<TestState>::new(signature, vec![], 5, llm);

        let mut inputs = HashMap::new();
        inputs.insert("question".to_string(), "What is AI?".to_string());

        let prompt = react.build_extract_prompt(&inputs, "Step 1: Searched\nStep 2: Found info");

        assert!(prompt.contains("Extract"));
        assert!(prompt.contains("`answer`"));
        assert!(prompt.contains("Question: What is AI?"));
        assert!(prompt.contains("Step 1: Searched"));
    }

    // ============================================================================
    // Tool Clone Tests
    // ============================================================================

    #[test]
    fn test_simple_tool_clone() {
        let tool = SimpleTool::new("clone_test", "Test cloning", HashMap::new(), |_| {
            Ok("ok".to_string())
        });

        let cloned = tool.clone();
        assert_eq!(tool.name(), cloned.name());
        assert_eq!(tool.description(), cloned.description());
    }
}
