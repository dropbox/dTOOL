//! Legacy agent executor implementation.
//!
//! This module provides the deprecated [`AgentExecutor`] for running agents with tools.
//! For new code, use [`create_react_agent()`](crate::prebuilt::create_react_agent) instead.

use serde::{Deserialize, Serialize};

use crate::core::error::Result;
use crate::core::tools::validate_tool_input_for;

use super::{
    Agent, AgentAction, AgentCheckpointState, AgentContext, AgentDecision, AgentMiddleware,
    AgentStep, Checkpoint, Memory,
};

/// Configuration for the legacy agent executor.
///
/// **DEPRECATED:** Use [`create_react_agent()`](crate::prebuilt::create_react_agent) instead.
/// The new API is Python-compatible and integrates with DashFlow features
/// (checkpointing, streaming, human-in-the-loop).
///
/// # Fields
///
/// * `max_iterations` - Maximum number of agent iterations before stopping (default: 15)
/// * `max_execution_time` - Optional timeout in seconds for total execution
/// * `early_stopping_method` - How to handle iteration limit: "force" or "generate"
/// * `handle_parsing_errors` - If true, tool errors become observations instead of errors
/// * `checkpoint_id` - Optional checkpoint identifier for resumable execution
#[deprecated(
    since = "1.9.0",
    note = "Use create_react_agent() from dashflow instead. \
            The new API is Python-compatible and integrates with DashFlow features \
            (checkpointing, streaming, human-in-the-loop). \
            Example: `use dashflow::prebuilt::create_react_agent; \
            let agent = create_react_agent(model, tools)?;`"
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExecutorConfig {
    /// Maximum number of agent iterations before stopping (default: 15).
    pub max_iterations: usize,
    /// Optional timeout in seconds for total execution time.
    pub max_execution_time: Option<f64>,
    /// How to handle iteration limit: "force" (use last observation) or "generate" (ask agent for final answer).
    pub early_stopping_method: String,
    /// If true, tool errors become observations instead of propagating as errors.
    pub handle_parsing_errors: bool,
    /// Optional checkpoint identifier for resumable execution.
    pub checkpoint_id: Option<String>,
}

#[allow(deprecated)]
impl Default for AgentExecutorConfig {
    fn default() -> Self {
        Self {
            max_iterations: 15,
            max_execution_time: None,
            early_stopping_method: "force".to_string(),
            handle_parsing_errors: true,
            checkpoint_id: None,
        }
    }
}

/// Result of agent execution.
///
/// Contains the final output, all intermediate steps taken, and the iteration count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExecutorResult {
    /// The final output produced by the agent.
    pub output: String,
    /// All intermediate steps (tool calls and observations) during execution.
    pub intermediate_steps: Vec<AgentStep>,
    /// Number of iterations performed.
    pub iterations: usize,
}

/// Legacy agent executor that runs an agent loop with tools.
///
/// **DEPRECATED:** Use [`create_react_agent()`](crate::prebuilt::create_react_agent) instead.
/// The new API is Python-compatible and integrates with DashFlow features
/// (checkpointing, streaming, human-in-the-loop).
///
/// The executor runs an agent in a loop:
/// 1. Agent decides next action (tool call or finish)
/// 2. Executor runs the tool and collects observation
/// 3. Observation is passed back to the agent
/// 4. Repeat until agent finishes or max iterations reached
///
/// # Features
///
/// * **Tool execution**: Validates inputs and runs tools, returning observations
/// * **Memory**: Optional conversation memory for multi-turn interactions
/// * **Checkpointing**: Save/resume execution state for long-running agents
/// * **Middleware**: Hooks for logging, transformation, and error recovery
///
/// # Example
///
/// ```rust,ignore
/// #[allow(deprecated)]
/// use dashflow::core::agents::{AgentExecutor, AgentExecutorConfig};
///
/// let executor = AgentExecutor::new(Box::new(my_agent))
///     .with_tools(vec![Box::new(calculator), Box::new(search)])
///     .with_config(AgentExecutorConfig {
///         max_iterations: 10,
///         ..Default::default()
///     });
///
/// let result = executor.execute("What is 2+2?").await?;
/// println!("Output: {}", result.output);
/// ```
#[deprecated(
    since = "1.9.0",
    note = "Use create_react_agent() from dashflow instead. \
            The new API is Python-compatible and integrates with DashFlow features \
            (checkpointing, streaming, human-in-the-loop). \
            Example: `use dashflow::prebuilt::create_react_agent; \
            let agent = create_react_agent(model, tools)?;`"
)]
pub struct AgentExecutor {
    pub(super) agent: Box<dyn Agent>,
    pub(super) tools: Vec<Box<dyn crate::core::tools::Tool>>,
    #[allow(deprecated)]
    pub(super) config: AgentExecutorConfig,
    pub(super) middlewares: Vec<Box<dyn AgentMiddleware>>,
    pub(super) memory: Option<std::sync::Arc<tokio::sync::Mutex<Box<dyn Memory>>>>,
    pub(super) checkpoint: Option<std::sync::Arc<tokio::sync::Mutex<Box<dyn Checkpoint>>>>,
}

#[allow(deprecated)]
impl AgentExecutor {
    /// Creates a new agent executor with the given agent.
    ///
    /// Use the builder methods to configure tools, memory, checkpointing, and middlewares.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let executor = AgentExecutor::new(Box::new(my_agent))
    ///     .with_tools(tools)
    ///     .with_memory(memory);
    /// ```
    #[must_use]
    pub fn new(agent: Box<dyn Agent>) -> Self {
        Self {
            agent,
            tools: Vec::new(),
            config: AgentExecutorConfig::default(),
            middlewares: Vec::new(),
            memory: None,
            checkpoint: None,
        }
    }

    /// Sets the tools available to the agent during execution.
    #[must_use]
    pub fn with_tools(mut self, tools: Vec<Box<dyn crate::core::tools::Tool>>) -> Self {
        self.tools = tools;
        self
    }

    /// Sets the execution configuration (max iterations, timeouts, etc.).
    #[must_use]
    pub fn with_config(mut self, config: AgentExecutorConfig) -> Self {
        self.config = config;
        self
    }

    /// Sets all middlewares that will process agent steps.
    #[must_use]
    pub fn with_middlewares(mut self, middlewares: Vec<Box<dyn AgentMiddleware>>) -> Self {
        self.middlewares = middlewares;
        self
    }

    /// Adds a single middleware to the execution pipeline.
    #[must_use]
    pub fn with_middleware(mut self, middleware: Box<dyn AgentMiddleware>) -> Self {
        self.middlewares.push(middleware);
        self
    }

    /// Attaches conversation memory for context across executions.
    #[must_use]
    pub fn with_memory(mut self, memory: Box<dyn Memory>) -> Self {
        self.memory = Some(std::sync::Arc::new(tokio::sync::Mutex::new(memory)));
        self
    }

    /// Enables checkpoint-based resumption for long-running executions.
    #[must_use]
    pub fn with_checkpoint(mut self, checkpoint: Box<dyn Checkpoint>) -> Self {
        self.checkpoint = Some(std::sync::Arc::new(tokio::sync::Mutex::new(checkpoint)));
        self
    }

    /// Executes the agent with the given input string.
    ///
    /// Returns the final result after the agent completes its reasoning and action loop.
    pub async fn execute(&self, input: &str) -> Result<AgentExecutorResult> {
        let mut context = AgentContext::new(input);
        let start_time = std::time::Instant::now();

        let memory_context = if let Some(memory) = &self.memory {
            let mem = memory.lock().await;
            let ctx = mem.load_context().await?;
            drop(mem);
            ctx
        } else {
            String::new()
        };

        if !memory_context.is_empty() {
            context
                .metadata
                .insert("memory_context".to_string(), memory_context);
        }

        for iteration in 0..self.config.max_iterations {
            context.iteration = iteration;

            if let Some(max_time) = self.config.max_execution_time {
                if start_time.elapsed().as_secs_f64() > max_time {
                    return Err(crate::core::Error::timeout(format!(
                        "Agent execution exceeded max time of {max_time} seconds"
                    )));
                }
            }

            for middleware in &self.middlewares {
                middleware.before_plan(&mut context).await?;
            }

            let decision = match self
                .agent
                .plan(&context.input, &context.intermediate_steps)
                .await
            {
                Ok(d) => d,
                Err(e) => {
                    let mut recovered = None;
                    for middleware in &self.middlewares {
                        if let Some(obs) = middleware.on_error(&e).await? {
                            recovered = Some(obs);
                            break;
                        }
                    }
                    if let Some(obs) = recovered {
                        if let Some(last_action) =
                            context.intermediate_steps.last().map(|s| s.action.clone())
                        {
                            context.intermediate_steps.push(AgentStep {
                                action: last_action,
                                observation: obs,
                            });
                            continue;
                        }
                    }
                    return Err(e);
                }
            };

            let mut decision = decision;
            for middleware in &self.middlewares {
                decision = middleware.after_plan(&context, decision).await?;
            }

            match decision {
                AgentDecision::Action(action) => {
                    let mut action = action;
                    for middleware in &self.middlewares {
                        action = middleware.before_tool(&action).await?;
                    }

                    let observation = match self.execute_tool(&action).await {
                        Ok(obs) => obs,
                        Err(e) => {
                            if e.to_string().contains("not found") {
                                return Err(e);
                            }

                            let mut recovered = None;
                            for middleware in &self.middlewares {
                                if let Some(obs) = middleware.on_error(&e).await? {
                                    recovered = Some(obs);
                                    break;
                                }
                            }
                            if let Some(obs) = recovered {
                                obs
                            } else if self.config.handle_parsing_errors {
                                format!("Error executing tool '{}': {}", action.tool, e)
                            } else {
                                return Err(e);
                            }
                        }
                    };

                    let mut observation = observation;
                    for middleware in &self.middlewares {
                        observation = middleware.after_tool(&action, &observation).await?;
                    }

                    context.intermediate_steps.push(AgentStep {
                        action,
                        observation,
                    });

                    if let (Some(checkpoint), Some(checkpoint_id)) =
                        (&self.checkpoint, &self.config.checkpoint_id)
                    {
                        let mut ckpt = checkpoint.lock().await;
                        let state = AgentCheckpointState::from_context(&context);
                        ckpt.save_state(checkpoint_id, &state).await?;
                    }
                }
                AgentDecision::Finish(finish) => {
                    if let Some(memory) = &self.memory {
                        let mut mem = memory.lock().await;
                        mem.save_context(input, &finish.output).await?;
                    }

                    return Ok(AgentExecutorResult {
                        output: finish.output,
                        intermediate_steps: context.intermediate_steps,
                        iterations: iteration + 1,
                    });
                }
            }
        }

        match self.config.early_stopping_method.as_str() {
            "force" => {
                let output = if let Some(last_step) = context.intermediate_steps.last() {
                    last_step.observation.clone()
                } else {
                    "Agent stopped due to iteration limit".to_string()
                };

                if let Some(memory) = &self.memory {
                    let mut mem = memory.lock().await;
                    mem.save_context(input, &output).await?;
                }

                Ok(AgentExecutorResult {
                    output,
                    intermediate_steps: context.intermediate_steps,
                    iterations: self.config.max_iterations,
                })
            }
            "generate" => {
                let decision = self
                    .agent
                    .plan(&context.input, &context.intermediate_steps)
                    .await?;
                let output = match decision {
                    AgentDecision::Finish(finish) => finish.output,
                    AgentDecision::Action(_) => "Agent could not generate final answer".to_string(),
                };

                if let Some(memory) = &self.memory {
                    let mut mem = memory.lock().await;
                    mem.save_context(input, &output).await?;
                }

                Ok(AgentExecutorResult {
                    output,
                    intermediate_steps: context.intermediate_steps,
                    iterations: self.config.max_iterations,
                })
            }
            _ => Err(crate::core::Error::other(format!(
                "Agent stopped after {} iterations",
                self.config.max_iterations
            ))),
        }
    }

    /// Resumes execution from a previously saved checkpoint.
    ///
    /// # Errors
    ///
    /// Returns an error if no checkpoint backend is configured or if the
    /// checkpoint cannot be loaded.
    pub async fn resume_from_checkpoint(&self, checkpoint_id: &str) -> Result<AgentExecutorResult> {
        let checkpoint = self.checkpoint.as_ref().ok_or_else(|| {
            crate::core::Error::config("No checkpoint backend configured".to_string())
        })?;

        let ckpt = checkpoint.lock().await;
        let state = ckpt.load_state(checkpoint_id).await?;
        drop(ckpt);

        let mut context = state.to_context();
        let start_time = std::time::Instant::now();

        for iteration in context.iteration..self.config.max_iterations {
            context.iteration = iteration;

            if let Some(max_time) = self.config.max_execution_time {
                if start_time.elapsed().as_secs_f64() > max_time {
                    return Err(crate::core::Error::timeout(format!(
                        "Agent execution exceeded max time of {max_time} seconds"
                    )));
                }
            }

            for middleware in &self.middlewares {
                middleware.before_plan(&mut context).await?;
            }

            let decision = match self
                .agent
                .plan(&context.input, &context.intermediate_steps)
                .await
            {
                Ok(d) => d,
                Err(e) => {
                    let mut recovered = None;
                    for middleware in &self.middlewares {
                        if let Some(obs) = middleware.on_error(&e).await? {
                            recovered = Some(obs);
                            break;
                        }
                    }
                    if let Some(obs) = recovered {
                        if let Some(last_action) =
                            context.intermediate_steps.last().map(|s| s.action.clone())
                        {
                            context.intermediate_steps.push(AgentStep {
                                action: last_action,
                                observation: obs,
                            });
                            continue;
                        }
                    }
                    return Err(e);
                }
            };

            let mut decision = decision;
            for middleware in &self.middlewares {
                decision = middleware.after_plan(&context, decision).await?;
            }

            match decision {
                AgentDecision::Action(action) => {
                    let mut action = action;
                    for middleware in &self.middlewares {
                        action = middleware.before_tool(&action).await?;
                    }

                    let observation = match self.execute_tool(&action).await {
                        Ok(obs) => obs,
                        Err(e) => {
                            if e.to_string().contains("not found") {
                                return Err(e);
                            }

                            let mut recovered = None;
                            for middleware in &self.middlewares {
                                if let Some(obs) = middleware.on_error(&e).await? {
                                    recovered = Some(obs);
                                    break;
                                }
                            }
                            if let Some(obs) = recovered {
                                obs
                            } else if self.config.handle_parsing_errors {
                                format!("Error executing tool '{}': {}", action.tool, e)
                            } else {
                                return Err(e);
                            }
                        }
                    };

                    let mut observation = observation;
                    for middleware in &self.middlewares {
                        observation = middleware.after_tool(&action, &observation).await?;
                    }

                    context.intermediate_steps.push(AgentStep {
                        action,
                        observation,
                    });

                    let mut ckpt = checkpoint.lock().await;
                    let state = AgentCheckpointState::from_context(&context);
                    ckpt.save_state(checkpoint_id, &state).await?;
                }
                AgentDecision::Finish(finish) => {
                    if let Some(memory) = &self.memory {
                        let mut mem = memory.lock().await;
                        mem.save_context(&context.input, &finish.output).await?;
                    }

                    return Ok(AgentExecutorResult {
                        output: finish.output,
                        intermediate_steps: context.intermediate_steps,
                        iterations: iteration + 1,
                    });
                }
            }
        }

        match self.config.early_stopping_method.as_str() {
            "force" => {
                let output = if let Some(last_step) = context.intermediate_steps.last() {
                    last_step.observation.clone()
                } else {
                    "Agent stopped due to iteration limit".to_string()
                };

                if let Some(memory) = &self.memory {
                    let mut mem = memory.lock().await;
                    mem.save_context(&context.input, &output).await?;
                }

                Ok(AgentExecutorResult {
                    output,
                    intermediate_steps: context.intermediate_steps,
                    iterations: self.config.max_iterations,
                })
            }
            "generate" => {
                let decision = self
                    .agent
                    .plan(&context.input, &context.intermediate_steps)
                    .await?;
                let output = match decision {
                    AgentDecision::Finish(finish) => finish.output,
                    AgentDecision::Action(_) => "Agent could not generate final answer".to_string(),
                };

                if let Some(memory) = &self.memory {
                    let mut mem = memory.lock().await;
                    mem.save_context(&context.input, &output).await?;
                }

                Ok(AgentExecutorResult {
                    output,
                    intermediate_steps: context.intermediate_steps,
                    iterations: self.config.max_iterations,
                })
            }
            _ => Err(crate::core::Error::other(format!(
                "Agent stopped after {} iterations",
                self.config.max_iterations
            ))),
        }
    }

    async fn execute_tool(&self, action: &AgentAction) -> Result<String> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name() == action.tool)
            .ok_or_else(|| {
                crate::core::Error::tool_error(format!("Tool '{}' not found", action.tool))
            })?;

        if let Err(e) =
            validate_tool_input_for(tool.name(), &action.tool_input, &tool.args_schema())
        {
            if self.config.handle_parsing_errors {
                return Ok(format!(
                    "Error: Tool input validation failed for '{}': {}",
                    action.tool, e
                ));
            } else {
                return Err(e);
            }
        }

        match tool._call(action.tool_input.clone()).await {
            Ok(output) => Ok(output),
            Err(e) => {
                if self.config.handle_parsing_errors {
                    Ok(format!("Error executing tool '{}': {}", action.tool, e))
                } else {
                    Err(e)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(deprecated)]

    use super::*;

    use crate::core::agents::{AgentFinish, AgentStep};
    use crate::core::tools::{Tool, ToolInput};

    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct RecordingMemory {
        load_context_value: String,
        saved: Arc<Mutex<Vec<(String, String)>>>,
    }

    impl RecordingMemory {
        fn new(load_context_value: impl Into<String>) -> Self {
            Self {
                load_context_value: load_context_value.into(),
                saved: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    #[async_trait::async_trait]
    impl Memory for RecordingMemory {
        async fn load_context(&self) -> Result<String> {
            Ok(self.load_context_value.clone())
        }

        async fn save_context(&mut self, input: &str, output: &str) -> Result<()> {
            self.saved
                .lock()
                .expect("memory lock poisoned")
                .push((input.to_string(), output.to_string()));
            Ok(())
        }

        async fn clear(&mut self) -> Result<()> {
            self.saved.lock().expect("memory lock poisoned").clear();
            Ok(())
        }

        fn get_history(&self) -> Vec<(String, String)> {
            self.saved
                .lock()
                .expect("memory lock poisoned")
                .clone()
        }
    }

    #[derive(Clone)]
    struct AgentFn {
        f: Arc<dyn Fn(&str, &[AgentStep]) -> Result<AgentDecision> + Send + Sync>,
    }

    impl AgentFn {
        fn new<F>(f: F) -> Self
        where
            F: Fn(&str, &[AgentStep]) -> Result<AgentDecision> + Send + Sync + 'static,
        {
            Self { f: Arc::new(f) }
        }
    }

    #[async_trait::async_trait]
    impl Agent for AgentFn {
        async fn plan(&self, input: &str, intermediate_steps: &[AgentStep]) -> Result<AgentDecision> {
            (self.f)(input, intermediate_steps)
        }
    }

    #[derive(Clone)]
    struct CountingTool {
        name: String,
        schema: serde_json::Value,
        call_count: Arc<AtomicUsize>,
        fail_with: Option<String>,
    }

    impl CountingTool {
        fn new(name: impl Into<String>) -> Self {
            Self {
                name: name.into(),
                schema: json!({
                    "type": "string"
                }),
                call_count: Arc::new(AtomicUsize::new(0)),
                fail_with: None,
            }
        }

        fn with_schema(mut self, schema: serde_json::Value) -> Self {
            self.schema = schema;
            self
        }

        fn failing(mut self, message: impl Into<String>) -> Self {
            self.fail_with = Some(message.into());
            self
        }
    }

    #[async_trait::async_trait]
    impl Tool for CountingTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "test tool"
        }

        fn args_schema(&self) -> serde_json::Value {
            self.schema.clone()
        }

        async fn _call(&self, input: ToolInput) -> Result<String> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            if let Some(message) = &self.fail_with {
                return Err(crate::core::Error::tool_error(message.clone()));
            }
            Ok(match input {
                ToolInput::String(s) => s,
                ToolInput::Structured(v) => v.to_string(),
            })
        }
    }

    #[derive(Clone, Default)]
    struct RecordingMiddleware {
        calls: Arc<Mutex<Vec<String>>>,
        saw_memory_context: Arc<Mutex<bool>>,
        recover_with: Option<String>,
        override_decision: Option<AgentDecision>,
        override_action_tool_input: Option<ToolInput>,
        override_observation: Option<String>,
    }

    impl RecordingMiddleware {
        fn new() -> Self {
            Self::default()
        }

        fn recovering_with(mut self, obs: impl Into<String>) -> Self {
            self.recover_with = Some(obs.into());
            self
        }

        fn overriding_decision(mut self, decision: AgentDecision) -> Self {
            self.override_decision = Some(decision);
            self
        }

        fn overriding_action_tool_input(mut self, tool_input: ToolInput) -> Self {
            self.override_action_tool_input = Some(tool_input);
            self
        }

        fn overriding_observation(mut self, observation: impl Into<String>) -> Self {
            self.override_observation = Some(observation.into());
            self
        }
    }

    #[async_trait::async_trait]
    impl AgentMiddleware for RecordingMiddleware {
        async fn before_plan(&self, context: &mut AgentContext) -> Result<()> {
            self.calls
                .lock()
                .expect("middleware lock poisoned")
                .push(format!("before_plan:{}", context.iteration));
            if context.metadata.contains_key("memory_context") {
                *self
                    .saw_memory_context
                    .lock()
                    .expect("middleware lock poisoned") = true;
            }
            Ok(())
        }

        async fn after_plan(
            &self,
            _context: &AgentContext,
            decision: AgentDecision,
        ) -> Result<AgentDecision> {
            self.calls
                .lock()
                .expect("middleware lock poisoned")
                .push("after_plan".to_string());
            Ok(self.override_decision.clone().unwrap_or(decision))
        }

        async fn before_tool(&self, action: &AgentAction) -> Result<AgentAction> {
            self.calls
                .lock()
                .expect("middleware lock poisoned")
                .push("before_tool".to_string());
            let mut action = action.clone();
            if let Some(tool_input) = self.override_action_tool_input.clone() {
                action.tool_input = tool_input;
            }
            Ok(action)
        }

        async fn after_tool(&self, _action: &AgentAction, observation: &str) -> Result<String> {
            self.calls
                .lock()
                .expect("middleware lock poisoned")
                .push("after_tool".to_string());
            Ok(self
                .override_observation
                .clone()
                .unwrap_or_else(|| observation.to_string()))
        }

        async fn on_error(&self, _error: &crate::core::Error) -> Result<Option<String>> {
            self.calls
                .lock()
                .expect("middleware lock poisoned")
                .push("on_error".to_string());
            Ok(self.recover_with.clone())
        }
    }

    fn finish(output: impl Into<String>) -> AgentDecision {
        AgentDecision::Finish(AgentFinish {
            output: output.into(),
            log: "done".to_string(),
        })
    }

    #[test]
    fn config_default_values() {
        let config = AgentExecutorConfig::default();
        assert_eq!(config.max_iterations, 15);
        assert!(config.max_execution_time.is_none());
        assert_eq!(config.early_stopping_method, "force");
        assert!(config.handle_parsing_errors);
        assert!(config.checkpoint_id.is_none());
    }

    #[tokio::test]
    async fn execute_finish_saves_memory() {
        let agent = AgentFn::new(|_input, _steps| Ok(finish("final")));
        let memory = RecordingMemory::new("");
        let saved = memory.saved.clone();

        let executor = AgentExecutor::new(Box::new(agent)).with_memory(Box::new(memory));
        let result = executor.execute("hello").await.unwrap();

        assert_eq!(result.output, "final");
        assert_eq!(result.iterations, 1);
        assert!(result.intermediate_steps.is_empty());

        let saved = saved.lock().expect("memory lock poisoned").clone();
        assert_eq!(saved, vec![("hello".to_string(), "final".to_string())]);
    }

    #[tokio::test]
    async fn execute_action_adds_step_and_passes_observation_to_agent() {
        let tool = CountingTool::new("echo");

        let agent = AgentFn::new(|_input, steps| {
            if steps.is_empty() {
                Ok(AgentDecision::Action(AgentAction::new(
                    "echo",
                    ToolInput::from("obs"),
                    "call tool",
                )))
            } else {
                Ok(finish(steps[0].observation.clone()))
            }
        });

        let executor =
            AgentExecutor::new(Box::new(agent)).with_tools(vec![Box::new(tool.clone())]);
        let result = executor.execute("ignored").await.unwrap();

        assert_eq!(tool.call_count.load(Ordering::SeqCst), 1);
        assert_eq!(result.output, "obs");
        assert_eq!(result.iterations, 2);
        assert_eq!(result.intermediate_steps.len(), 1);
        assert_eq!(result.intermediate_steps[0].observation, "obs");
    }

    #[tokio::test]
    async fn middleware_hooks_run_and_can_transform_action_and_observation() {
        let tool = CountingTool::new("echo");
        let middleware = RecordingMiddleware::new()
            .overriding_action_tool_input(ToolInput::from("changed"))
            .overriding_observation("observed");

        let calls = middleware.calls.clone();

        let agent = AgentFn::new(|_input, steps| {
            if steps.is_empty() {
                Ok(AgentDecision::Action(AgentAction::new(
                    "echo",
                    ToolInput::from("original"),
                    "call tool",
                )))
            } else {
                Ok(finish(steps[0].observation.clone()))
            }
        });

        let executor = AgentExecutor::new(Box::new(agent))
            .with_tools(vec![Box::new(tool)])
            .with_middleware(Box::new(middleware));
        let result = executor.execute("ignored").await.unwrap();

        assert_eq!(result.output, "observed");
        let calls = calls.lock().expect("middleware lock poisoned").clone();
        assert_eq!(
            calls,
            vec![
                "before_plan:0".to_string(),
                "after_plan".to_string(),
                "before_tool".to_string(),
                "after_tool".to_string(),
                "before_plan:1".to_string(),
                "after_plan".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn middleware_can_override_decision_in_after_plan() {
        let tool = CountingTool::new("echo");
        let middleware = RecordingMiddleware::new().overriding_decision(finish("short-circuit"));

        let agent = AgentFn::new(|_input, _steps| {
            Ok(AgentDecision::Action(AgentAction::new(
                "echo",
                ToolInput::from("obs"),
                "call tool",
            )))
        });

        let executor = AgentExecutor::new(Box::new(agent))
            .with_tools(vec![Box::new(tool.clone())])
            .with_middleware(Box::new(middleware));

        let result = executor.execute("ignored").await.unwrap();
        assert_eq!(result.output, "short-circuit");
        assert_eq!(tool.call_count.load(Ordering::SeqCst), 0);
        assert!(result.intermediate_steps.is_empty());
    }

    #[tokio::test]
    async fn memory_context_is_injected_into_metadata_for_middlewares() {
        let agent = AgentFn::new(|_input, _steps| Ok(finish("final")));

        let middleware = RecordingMiddleware::new();
        let saw_memory_context = middleware.saw_memory_context.clone();

        let memory = RecordingMemory::new("previous conversation");
        let executor = AgentExecutor::new(Box::new(agent))
            .with_memory(Box::new(memory))
            .with_middleware(Box::new(middleware));

        executor.execute("hello").await.unwrap();

        assert!(*saw_memory_context.lock().expect("middleware lock poisoned"));
    }

    #[tokio::test]
    async fn tool_input_validation_failure_becomes_observation_when_enabled() {
        let schema = json!({
            "type": "object",
            "properties": {
                "x": { "type": "string" }
            },
            "required": ["x"]
        });
        let tool = CountingTool::new("schema_tool").with_schema(schema);

        let agent = AgentFn::new(|_input, steps| {
            if steps.is_empty() {
                Ok(AgentDecision::Action(AgentAction::new(
                    "schema_tool",
                    ToolInput::Structured(json!({})),
                    "invalid input",
                )))
            } else {
                Ok(finish(steps[0].observation.clone()))
            }
        });

        let executor =
            AgentExecutor::new(Box::new(agent)).with_tools(vec![Box::new(tool.clone())]);
        let result = executor.execute("ignored").await.unwrap();

        assert_eq!(tool.call_count.load(Ordering::SeqCst), 0);
        assert!(result.output.contains("Tool input validation failed"));
    }

    #[tokio::test]
    async fn tool_input_validation_failure_errors_when_disabled() {
        let schema = json!({
            "type": "object",
            "properties": {
                "x": { "type": "string" }
            },
            "required": ["x"]
        });
        let tool = CountingTool::new("schema_tool").with_schema(schema);

        let agent = AgentFn::new(|_input, _steps| {
            Ok(AgentDecision::Action(AgentAction::new(
                "schema_tool",
                ToolInput::Structured(json!({})),
                "invalid input",
            )))
        });

        let executor = AgentExecutor::new(Box::new(agent))
            .with_tools(vec![Box::new(tool)])
            .with_config(AgentExecutorConfig {
                handle_parsing_errors: false,
                ..AgentExecutorConfig::default()
            });

        let err = executor.execute("ignored").await.unwrap_err();
        assert!(err.to_string().contains("input validation failed"));
    }

    #[tokio::test]
    async fn tool_call_error_becomes_observation_when_enabled() {
        let tool = CountingTool::new("failing").failing("boom");

        let agent = AgentFn::new(|_input, steps| {
            if steps.is_empty() {
                Ok(AgentDecision::Action(AgentAction::new(
                    "failing",
                    ToolInput::from("x"),
                    "call tool",
                )))
            } else {
                Ok(finish(steps[0].observation.clone()))
            }
        });

        let executor =
            AgentExecutor::new(Box::new(agent)).with_tools(vec![Box::new(tool.clone())]);
        let result = executor.execute("ignored").await.unwrap();

        assert_eq!(tool.call_count.load(Ordering::SeqCst), 1);
        assert!(result.output.contains("Error executing tool 'failing'"));
    }

    #[tokio::test]
    async fn tool_call_error_can_be_recovered_by_middleware_when_disabled() {
        let tool = CountingTool::new("failing").failing("boom");

        let middleware = RecordingMiddleware::new().recovering_with("recovered");

        let agent = AgentFn::new(|_input, steps| {
            if steps.is_empty() {
                Ok(AgentDecision::Action(AgentAction::new(
                    "failing",
                    ToolInput::from("x"),
                    "call tool",
                )))
            } else {
                Ok(finish(steps[0].observation.clone()))
            }
        });

        let executor = AgentExecutor::new(Box::new(agent))
            .with_tools(vec![Box::new(tool.clone())])
            .with_middleware(Box::new(middleware))
            .with_config(AgentExecutorConfig {
                handle_parsing_errors: false,
                ..AgentExecutorConfig::default()
            });

        let result = executor.execute("ignored").await.unwrap();
        assert_eq!(tool.call_count.load(Ordering::SeqCst), 1);
        assert_eq!(result.output, "recovered");
    }

    #[tokio::test]
    async fn tool_not_found_is_error_and_not_recoverable() {
        let middleware = RecordingMiddleware::new().recovering_with("recovered");

        let agent = AgentFn::new(|_input, _steps| {
            Ok(AgentDecision::Action(AgentAction::new(
                "missing",
                ToolInput::from("x"),
                "call tool",
            )))
        });

        let executor = AgentExecutor::new(Box::new(agent)).with_middleware(Box::new(middleware));
        let err = executor.execute("ignored").await.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn plan_error_recovery_requires_last_action() {
        let middleware = RecordingMiddleware::new().recovering_with("recovered");
        let calls = middleware.calls.clone();

        let agent = AgentFn::new(|_input, _steps| Err(crate::core::Error::other("boom".to_string())));

        let executor = AgentExecutor::new(Box::new(agent)).with_middleware(Box::new(middleware));
        let err = executor.execute("ignored").await.unwrap_err();

        let calls = calls.lock().expect("middleware lock poisoned").clone();
        assert_eq!(
            calls,
            vec!["before_plan:0".to_string(), "on_error".to_string()]
        );
        assert!(err.to_string().contains("boom"));
    }

    #[tokio::test]
    async fn plan_error_can_be_recovered_after_first_action() {
        let middleware = RecordingMiddleware::new().recovering_with("recovered_plan");

        let tool = CountingTool::new("echo");
        let plan_calls = Arc::new(AtomicUsize::new(0));

        let agent = {
            let plan_calls = plan_calls.clone();
            AgentFn::new(move |_input, steps| {
                let call = plan_calls.fetch_add(1, Ordering::SeqCst);
                match call {
                    0 => Ok(AgentDecision::Action(AgentAction::new(
                        "echo",
                        ToolInput::from("first"),
                        "call tool",
                    ))),
                    1 => Err(crate::core::Error::other("plan failed".to_string())),
                    _ => Ok(finish(format!(
                        "steps:{} last:{}",
                        steps.len(),
                        steps.last().map(|s| s.observation.clone()).unwrap_or_default()
                    ))),
                }
            })
        };

        let executor = AgentExecutor::new(Box::new(agent))
            .with_tools(vec![Box::new(tool)])
            .with_middleware(Box::new(middleware))
            .with_config(AgentExecutorConfig {
                max_iterations: 5,
                ..AgentExecutorConfig::default()
            });

        let result = executor.execute("ignored").await.unwrap();

        assert_eq!(result.intermediate_steps.len(), 2);
        assert_eq!(result.intermediate_steps[0].observation, "first");
        assert_eq!(result.intermediate_steps[1].observation, "recovered_plan");
        assert_eq!(result.output, "steps:2 last:recovered_plan");
    }

    #[tokio::test]
    async fn max_execution_time_is_enforced() {
        // Use negative timeout to guarantee elapsed() > max_time is always true.
        // Using 0.0 is flaky because elapsed() can be exactly 0 on fast systems.
        let agent = AgentFn::new(|_input, _steps| Ok(finish("final")));

        let executor = AgentExecutor::new(Box::new(agent)).with_config(AgentExecutorConfig {
            max_execution_time: Some(-1.0),
            ..AgentExecutorConfig::default()
        });

        let err = executor.execute("ignored").await.unwrap_err();
        assert!(err.to_string().contains("exceeded max time"));
    }

    #[tokio::test]
    async fn early_stopping_force_uses_last_observation() {
        let tool = CountingTool::new("echo");
        let agent = AgentFn::new(|_input, _steps| {
            Ok(AgentDecision::Action(AgentAction::new(
                "echo",
                ToolInput::from("obs"),
                "call tool",
            )))
        });

        let executor = AgentExecutor::new(Box::new(agent))
            .with_tools(vec![Box::new(tool)])
            .with_config(AgentExecutorConfig {
                max_iterations: 1,
                early_stopping_method: "force".to_string(),
                ..AgentExecutorConfig::default()
            });

        let result = executor.execute("ignored").await.unwrap();
        assert_eq!(result.output, "obs");
        assert_eq!(result.iterations, 1);
        assert_eq!(result.intermediate_steps.len(), 1);
    }

    #[tokio::test]
    async fn early_stopping_force_without_steps_returns_default_message() {
        let agent = AgentFn::new(|_input, _steps| Ok(finish("should not run")));
        let executor = AgentExecutor::new(Box::new(agent)).with_config(AgentExecutorConfig {
            max_iterations: 0,
            early_stopping_method: "force".to_string(),
            ..AgentExecutorConfig::default()
        });

        let result = executor.execute("ignored").await.unwrap();
        assert_eq!(result.output, "Agent stopped due to iteration limit");
        assert_eq!(result.iterations, 0);
        assert!(result.intermediate_steps.is_empty());
    }

    #[tokio::test]
    async fn early_stopping_generate_calls_plan_for_final_answer() {
        let tool = CountingTool::new("echo");
        let plan_calls = Arc::new(AtomicUsize::new(0));

        let agent = {
            let plan_calls = plan_calls.clone();
            AgentFn::new(move |_input, steps| {
                let call = plan_calls.fetch_add(1, Ordering::SeqCst);
                if call == 0 {
                    Ok(AgentDecision::Action(AgentAction::new(
                        "echo",
                        ToolInput::from("obs"),
                        "call tool",
                    )))
                } else {
                    Ok(finish(format!("final:{}", steps.len())))
                }
            })
        };

        let memory = RecordingMemory::new("");
        let saved = memory.saved.clone();

        let executor = AgentExecutor::new(Box::new(agent))
            .with_tools(vec![Box::new(tool)])
            .with_memory(Box::new(memory))
            .with_config(AgentExecutorConfig {
                max_iterations: 1,
                early_stopping_method: "generate".to_string(),
                ..AgentExecutorConfig::default()
            });

        let result = executor.execute("hello").await.unwrap();
        assert_eq!(result.output, "final:1");
        assert_eq!(result.iterations, 1);

        let saved = saved.lock().expect("memory lock poisoned").clone();
        assert_eq!(saved, vec![("hello".to_string(), "final:1".to_string())]);
    }

    #[tokio::test]
    async fn early_stopping_unknown_method_is_error() {
        let agent = AgentFn::new(|_input, _steps| Ok(finish("final")));
        let executor = AgentExecutor::new(Box::new(agent)).with_config(AgentExecutorConfig {
            max_iterations: 0,
            early_stopping_method: "unknown".to_string(),
            ..AgentExecutorConfig::default()
        });

        let err = executor.execute("ignored").await.unwrap_err();
        assert!(err.to_string().contains("stopped after 0 iterations"));
    }

    #[tokio::test]
    async fn resume_from_checkpoint_requires_backend() {
        let agent = AgentFn::new(|_input, _steps| Ok(finish("final")));
        let executor = AgentExecutor::new(Box::new(agent));

        let err = executor.resume_from_checkpoint("ckpt1").await.unwrap_err();
        assert!(err.to_string().contains("No checkpoint backend configured"));
    }

    #[tokio::test]
    async fn resume_from_checkpoint_restores_state_and_finishes() {
        let tool = CountingTool::new("echo");
        let agent = AgentFn::new(|_input, steps| {
            if steps.is_empty() {
                Ok(AgentDecision::Action(AgentAction::new(
                    "echo",
                    ToolInput::from("obs"),
                    "call tool",
                )))
            } else {
                Ok(finish("resumed"))
            }
        });

        let executor = AgentExecutor::new(Box::new(agent))
            .with_tools(vec![Box::new(tool)])
            .with_checkpoint(Box::new(crate::core::agents::MemoryCheckpoint::new()))
            .with_config(AgentExecutorConfig {
                max_iterations: 1,
                checkpoint_id: Some("ckpt1".to_string()),
                ..AgentExecutorConfig::default()
            });

        let first = executor.execute("ignored").await.unwrap();
        assert_eq!(first.output, "obs");
        assert_eq!(first.intermediate_steps.len(), 1);

        let resumed = executor.resume_from_checkpoint("ckpt1").await.unwrap();
        assert_eq!(resumed.output, "resumed");
        assert_eq!(resumed.iterations, 1);
        assert_eq!(resumed.intermediate_steps.len(), 1);
    }
}
