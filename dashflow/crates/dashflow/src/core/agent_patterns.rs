//! Advanced agent patterns and architectures
//!
//! This module provides sophisticated agent patterns that go beyond simple `ReAct` loops:
//!
//! - **Plan & Execute**: Decompose complex tasks into steps, then execute
//! - **Reflection**: Self-critique and iterative improvement
//! - **Multi-Agent Debate**: Collaborative reasoning through debate
//!
//! # Plan & Execute Pattern
//!
//! The Plan & Execute pattern is useful for complex, multi-step tasks where upfront planning
//! helps break down the problem:
//!
//! ```rust,ignore
//! use dashflow::core::agent_patterns::PlanAndExecuteAgent;
//!
//! let agent = PlanAndExecuteAgent::builder()
//!     .planner_llm(gpt4)
//!     .executor_llm(gpt4_mini)
//!     .tools(tools)
//!     .max_iterations(20)
//!     .build()?;
//!
//! let result = agent.run("Research AI trends and write a report").await?;
//! ```
//!
//! # Reflection Pattern
//!
//! The Reflection pattern uses an actor-critic architecture for iterative refinement:
//!
//! ```rust,ignore
//! use dashflow::core::agent_patterns::ReflectionAgent;
//!
//! let agent = ReflectionAgent::builder()
//!     .actor_llm(writer_llm)
//!     .critic_llm(critic_llm)
//!     .quality_threshold(0.8)
//!     .max_iterations(5)
//!     .build()?;
//!
//! let result = agent.run("Write a comprehensive analysis").await?;
//! ```
//!
//! # Multi-Agent Debate Pattern
//!
//! The Multi-Agent Debate pattern enables collaborative reasoning through structured debate:
//!
//! ```rust,ignore
//! use dashflow::core::agent_patterns::{Debater, MultiAgentDebate};
//!
//! let debate = MultiAgentDebate::builder()
//!     .add_debater("Alice", "Conservative perspective", conservative_llm)
//!     .add_debater("Bob", "Progressive perspective", progressive_llm)
//!     .add_debater("Carol", "Pragmatic perspective", pragmatic_llm)
//!     .moderator(moderator_llm)
//!     .max_rounds(3)
//!     .build()?;
//!
//! let result = debate.run("Should we adopt this technology?").await?;
//! ```
//!
//! # Architecture
//!
//! **Plan & Execute:**
//! - **Planner**: Uses a powerful LLM to create a step-by-step plan
//! - **Executor**: Uses a fast LLM to execute each step
//! - **Progress Tracker**: Monitors completion and adjusts plan if needed
//! - **Replanner**: Can create new plans based on execution results
//!
//! **Reflection:**
//! - **Actor**: Generates content/output
//! - **Critic**: Evaluates quality and provides feedback
//! - **Loop**: Iterates until quality threshold met or max iterations reached
//!
//! **Multi-Agent Debate:**
//! - **Debaters**: Multiple agents with different perspectives
//! - **Rounds**: Structured debate with multiple rounds
//! - **History**: Optional context from previous contributions
//! - **Moderator**: Optional synthesis of consensus from all perspectives

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::error::{Error, Result};
use crate::core::language_models::ChatModel;
use crate::core::messages::Message;
use crate::core::tools::Tool;

// ============================================================================
// Plan & Execute Types
// ============================================================================

/// A single step in an execution plan
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanStep {
    /// Step number (1-indexed)
    pub step_number: usize,

    /// Description of what to do in this step
    pub description: String,

    /// Whether this step has been completed
    pub completed: bool,

    /// Result of executing this step (if completed)
    pub result: Option<String>,

    /// Whether this step failed
    pub failed: bool,

    /// Error message if step failed
    pub error: Option<String>,
}

impl PlanStep {
    /// Create a new plan step
    pub fn new(step_number: usize, description: impl Into<String>) -> Self {
        Self {
            step_number,
            description: description.into(),
            completed: false,
            result: None,
            failed: false,
            error: None,
        }
    }

    /// Mark step as completed with a result
    pub fn complete(mut self, result: impl Into<String>) -> Self {
        self.completed = true;
        self.result = Some(result.into());
        self
    }

    /// Mark step as failed with an error
    pub fn fail(mut self, error: impl Into<String>) -> Self {
        self.failed = true;
        self.error = Some(error.into());
        self
    }
}

/// A plan for executing a complex task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    /// The original task/goal
    pub task: String,

    /// Steps to execute
    pub steps: Vec<PlanStep>,

    /// When the plan was created
    pub created_at: std::time::SystemTime,

    /// Number of times plan has been revised
    pub revision_count: usize,
}

impl ExecutionPlan {
    /// Create a new execution plan
    pub fn new(task: impl Into<String>, steps: Vec<PlanStep>) -> Self {
        Self {
            task: task.into(),
            steps,
            created_at: std::time::SystemTime::now(),
            revision_count: 0,
        }
    }

    /// Get the next incomplete step
    #[must_use]
    pub fn next_step(&self) -> Option<&PlanStep> {
        self.steps.iter().find(|s| !s.completed && !s.failed)
    }

    /// Get the next incomplete step (mutable)
    pub fn next_step_mut(&mut self) -> Option<&mut PlanStep> {
        self.steps.iter_mut().find(|s| !s.completed && !s.failed)
    }

    /// Check if all steps are completed
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.steps.iter().all(|s| s.completed)
    }

    /// Check if any steps have failed
    #[must_use]
    pub fn has_failures(&self) -> bool {
        self.steps.iter().any(|s| s.failed)
    }

    /// Get progress as a fraction (completed / total)
    ///
    /// Returns 1.0 if there are no steps (nothing to complete).
    #[must_use]
    pub fn progress(&self) -> f64 {
        if self.steps.is_empty() {
            return 1.0; // Empty plan is fully complete
        }
        let completed = self.steps.iter().filter(|s| s.completed).count();
        completed as f64 / self.steps.len() as f64
    }

    /// Create a revised plan
    #[must_use]
    pub fn revise(mut self, new_steps: Vec<PlanStep>) -> Self {
        self.steps = new_steps;
        self.revision_count += 1;
        self.created_at = std::time::SystemTime::now();
        self
    }
}

/// Configuration for the Plan & Execute agent
#[derive(Debug, Clone)]
pub struct PlanAndExecuteConfig {
    /// Maximum number of iterations (execution steps)
    pub max_iterations: usize,

    /// Whether to enable replanning on failures
    pub enable_replanning: bool,

    /// Maximum number of replans allowed
    pub max_replans: usize,

    /// Planner system message
    pub planner_system_message: String,

    /// Executor system message
    pub executor_system_message: String,

    /// Whether to include previous execution results in executor context
    pub include_execution_history: bool,

    /// Verbose output for debugging
    pub verbose: bool,
}

impl Default for PlanAndExecuteConfig {
    fn default() -> Self {
        Self {
            max_iterations: 20,
            enable_replanning: true,
            max_replans: 3,
            planner_system_message: "You are an expert task planner. Break down complex tasks into clear, actionable steps.".to_string(),
            executor_system_message: "You are a helpful assistant executing a step in a larger plan. Complete the assigned task.".to_string(),
            include_execution_history: true,
            verbose: false,
        }
    }
}

impl PlanAndExecuteConfig {
    /// Create a new config with default values
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum number of iterations
    #[must_use]
    pub const fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Enable or disable replanning on failures
    #[must_use]
    pub const fn with_enable_replanning(mut self, enable: bool) -> Self {
        self.enable_replanning = enable;
        self
    }

    /// Set maximum number of replans allowed
    #[must_use]
    pub const fn with_max_replans(mut self, max_replans: usize) -> Self {
        self.max_replans = max_replans;
        self
    }

    /// Set the planner system message
    #[must_use]
    pub fn with_planner_system_message(mut self, message: impl Into<String>) -> Self {
        self.planner_system_message = message.into();
        self
    }

    /// Set the executor system message
    #[must_use]
    pub fn with_executor_system_message(mut self, message: impl Into<String>) -> Self {
        self.executor_system_message = message.into();
        self
    }

    /// Set whether to include execution history in context
    #[must_use]
    pub const fn with_include_execution_history(mut self, include: bool) -> Self {
        self.include_execution_history = include;
        self
    }

    /// Enable or disable verbose output
    #[must_use]
    pub const fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }
}

/// Plan & Execute agent that decomposes tasks into steps and executes them
///
/// This agent follows a two-phase approach:
/// 1. **Planning**: Uses a planner LLM to break down a complex task into steps
/// 2. **Execution**: Uses an executor LLM to complete each step with available tools
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::agent_patterns::PlanAndExecuteAgent;
///
/// let agent = PlanAndExecuteAgent::builder()
///     .planner_llm(gpt4)
///     .executor_llm(gpt4_mini)
///     .tools(tools)
///     .max_iterations(20)
///     .build()?;
///
/// let result = agent.run("Research AI trends and write a 10-page report").await?;
/// ```
pub struct PlanAndExecuteAgent {
    /// LLM for planning (typically a powerful model like GPT-4)
    planner_llm: Arc<dyn ChatModel>,

    /// LLM for execution (typically a faster model like GPT-3.5)
    executor_llm: Arc<dyn ChatModel>,

    /// Available tools for the executor
    tools: Vec<Arc<dyn Tool>>,

    /// Configuration
    config: PlanAndExecuteConfig,
}

impl PlanAndExecuteAgent {
    /// Create a new Plan & Execute agent
    pub fn new(
        planner_llm: Arc<dyn ChatModel>,
        executor_llm: Arc<dyn ChatModel>,
        tools: Vec<Arc<dyn Tool>>,
    ) -> Self {
        Self {
            planner_llm,
            executor_llm,
            tools,
            config: PlanAndExecuteConfig::default(),
        }
    }

    /// Create a builder for configuring the agent
    #[must_use]
    pub fn builder() -> PlanAndExecuteAgentBuilder {
        PlanAndExecuteAgentBuilder::new()
    }

    /// Set the configuration
    #[must_use]
    pub fn with_config(mut self, config: PlanAndExecuteConfig) -> Self {
        self.config = config;
        self
    }

    /// Set max iterations
    #[must_use]
    pub const fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.config.max_iterations = max_iterations;
        self
    }

    /// Enable/disable replanning
    #[must_use]
    pub const fn with_replanning(mut self, enable: bool) -> Self {
        self.config.enable_replanning = enable;
        self
    }

    /// Set verbose output
    #[must_use]
    pub const fn with_verbose(mut self, verbose: bool) -> Self {
        self.config.verbose = verbose;
        self
    }

    /// Create an execution plan for the task
    ///
    /// Uses the planner LLM to break down the task into actionable steps.
    async fn create_plan(&self, task: &str) -> Result<ExecutionPlan> {
        let prompt = self.build_planning_prompt(task);

        if self.config.verbose {
            tracing::debug!("[Plan & Execute] Creating plan for task: {task}");
        }

        let messages = vec![
            Message::system(self.config.planner_system_message.as_str()),
            Message::human(prompt.as_str()),
        ];

        let response = self
            .planner_llm
            .generate(&messages, None, None, None, None)
            .await?;

        let plan_text = response
            .generations
            .first()
            .ok_or_else(|| Error::Agent("No generation from planner LLM".to_string()))?
            .message
            .content()
            .as_text();
        let steps = self.parse_plan(&plan_text)?;

        if self.config.verbose {
            tracing::debug!("[Plan & Execute] Created plan with {} steps", steps.len());
            for step in &steps {
                tracing::debug!("  Step {}: {}", step.step_number, step.description);
            }
        }

        Ok(ExecutionPlan::new(task, steps))
    }

    /// Build the planning prompt
    fn build_planning_prompt(&self, task: &str) -> String {
        format!(
            r"Task: {}

Please create a detailed step-by-step plan to complete this task.

Format your response as a numbered list:
1. [First step description]
2. [Second step description]
3. [Third step description]
...

Make each step:
- Clear and actionable
- Specific about what needs to be done
- Reasonable in scope (can be completed in one execution)

Available tools: {}

Plan:",
            task,
            self.tools
                .iter()
                .map(|t| t.name())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    /// Parse the plan from the LLM response
    fn parse_plan(&self, plan_text: &str) -> Result<Vec<PlanStep>> {
        let mut steps = Vec::new();

        for line in plan_text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Look for lines starting with "N." or "N)"
            if let Some((num_str, desc)) = Self::parse_numbered_line(line) {
                if let Ok(num) = num_str.parse::<usize>() {
                    steps.push(PlanStep::new(num, desc));
                }
            }
        }

        if steps.is_empty() {
            return Err(Error::Agent(
                "Failed to parse any steps from plan".to_string(),
            ));
        }

        Ok(steps)
    }

    /// Parse a numbered line (e.g., "1. Do something" or "2) Do something else")
    fn parse_numbered_line(line: &str) -> Option<(&str, String)> {
        // Try "N. Description" format
        if let Some(pos) = line.find('.') {
            let num_str = &line[..pos];
            if num_str.chars().all(char::is_numeric) {
                let desc = line[pos + 1..].trim().to_string();
                return Some((num_str, desc));
            }
        }

        // Try "N) Description" format
        if let Some(pos) = line.find(')') {
            let num_str = &line[..pos];
            if num_str.chars().all(char::is_numeric) {
                let desc = line[pos + 1..].trim().to_string();
                return Some((num_str, desc));
            }
        }

        None
    }

    /// Execute a single step of the plan
    async fn execute_step(&self, step: &PlanStep, execution_history: &str) -> Result<String> {
        if self.config.verbose {
            tracing::debug!(
                "[Plan & Execute] Executing step {}: {}",
                step.step_number,
                step.description
            );
        }

        let prompt = self.build_execution_prompt(step, execution_history);

        let messages = vec![
            Message::system(self.config.executor_system_message.as_str()),
            Message::human(prompt.as_str()),
        ];

        let response = self
            .executor_llm
            .generate(&messages, None, None, None, None)
            .await?;

        let result = response
            .generations
            .first()
            .ok_or_else(|| Error::Agent("No generation from executor LLM".to_string()))?
            .message
            .content()
            .as_text();

        if self.config.verbose {
            tracing::debug!(
                "[Plan & Execute] Step {} result: {}",
                step.step_number,
                result
            );
        }

        Ok(result)
    }

    /// Build the execution prompt for a step
    fn build_execution_prompt(&self, step: &PlanStep, execution_history: &str) -> String {
        let mut prompt = format!(
            "You are executing Step {} of a larger plan.\n\n",
            step.step_number
        );

        prompt.push_str(&format!("Task: {}\n\n", step.description));

        if self.config.include_execution_history && !execution_history.is_empty() {
            prompt.push_str("Previous steps completed:\n");
            prompt.push_str(execution_history);
            prompt.push_str("\n\n");
        }

        prompt.push_str("Please complete this step and provide your result.");

        prompt
    }

    /// Build execution history string from completed steps
    fn build_execution_history(&self, plan: &ExecutionPlan) -> String {
        let mut history = String::new();

        for step in &plan.steps {
            if step.completed {
                history.push_str(&format!(
                    "Step {}: {} → {}\n",
                    step.step_number,
                    step.description,
                    step.result.as_deref().unwrap_or("(no result)")
                ));
            }
        }

        history
    }

    /// Run the agent on a task
    pub async fn run(&self, task: &str) -> Result<String> {
        // Create initial plan
        let mut plan = self.create_plan(task).await?;
        let mut iteration = 0;

        // Execute steps
        while !plan.is_complete() && iteration < self.config.max_iterations {
            iteration += 1;

            let step = match plan.next_step() {
                Some(s) => s.clone(),
                None => break,
            };

            let execution_history = self.build_execution_history(&plan);

            // Execute the step
            match self.execute_step(&step, &execution_history).await {
                Ok(result) => {
                    // Mark step as completed
                    if let Some(step_mut) = plan.next_step_mut() {
                        step_mut.completed = true;
                        step_mut.result = Some(result);
                    }
                }
                Err(e) => {
                    // Mark step as failed
                    if let Some(step_mut) = plan.next_step_mut() {
                        step_mut.failed = true;
                        step_mut.error = Some(e.to_string());
                    }

                    // Attempt replanning if enabled
                    if self.config.enable_replanning
                        && plan.revision_count < self.config.max_replans
                    {
                        if self.config.verbose {
                            tracing::debug!(
                                "[Plan & Execute] Step failed, attempting replan (revision {})",
                                plan.revision_count + 1
                            );
                        }

                        // Create new plan accounting for failures
                        let replan_task = format!(
                            "{}\n\nNote: Previous attempt failed at step {}: {}",
                            task, step.step_number, e
                        );
                        plan = self.create_plan(&replan_task).await?;
                    } else {
                        return Err(Error::Agent(format!(
                            "Step {} failed: {}",
                            step.step_number, e
                        )));
                    }
                }
            }
        }

        if iteration >= self.config.max_iterations {
            return Err(Error::Agent(
                "Max iterations reached before completing plan".to_string(),
            ));
        }

        // Compile final result from all completed steps
        let final_result = self.compile_final_result(&plan);

        if self.config.verbose {
            tracing::debug!("[Plan & Execute] Completed all steps");
            tracing::debug!("Final result: {final_result}");
        }

        Ok(final_result)
    }

    /// Compile the final result from all completed steps
    fn compile_final_result(&self, plan: &ExecutionPlan) -> String {
        let mut result = format!("Task: {}\n\nResults:\n\n", plan.task);

        for step in &plan.steps {
            if step.completed {
                result.push_str(&format!(
                    "Step {}: {}\nResult: {}\n\n",
                    step.step_number,
                    step.description,
                    step.result.as_deref().unwrap_or("(no result)")
                ));
            }
        }

        result
    }
}

// ============================================================================
// Builder Pattern
// ============================================================================

/// Builder for `PlanAndExecuteAgent`
pub struct PlanAndExecuteAgentBuilder {
    planner_llm: Option<Arc<dyn ChatModel>>,
    executor_llm: Option<Arc<dyn ChatModel>>,
    tools: Vec<Arc<dyn Tool>>,
    config: PlanAndExecuteConfig,
}

impl PlanAndExecuteAgentBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self {
            planner_llm: None,
            executor_llm: None,
            tools: Vec::new(),
            config: PlanAndExecuteConfig::default(),
        }
    }

    /// Set the planner LLM
    #[must_use]
    pub fn planner_llm(mut self, llm: Arc<dyn ChatModel>) -> Self {
        self.planner_llm = Some(llm);
        self
    }

    /// Set the executor LLM
    #[must_use]
    pub fn executor_llm(mut self, llm: Arc<dyn ChatModel>) -> Self {
        self.executor_llm = Some(llm);
        self
    }

    /// Set the tools
    #[must_use]
    pub fn tools(mut self, tools: Vec<Arc<dyn Tool>>) -> Self {
        self.tools = tools;
        self
    }

    /// Add a single tool
    #[must_use]
    pub fn tool(mut self, tool: Arc<dyn Tool>) -> Self {
        self.tools.push(tool);
        self
    }

    /// Set max iterations
    #[must_use]
    pub const fn max_iterations(mut self, max_iterations: usize) -> Self {
        self.config.max_iterations = max_iterations;
        self
    }

    /// Enable/disable replanning
    #[must_use]
    pub const fn enable_replanning(mut self, enable: bool) -> Self {
        self.config.enable_replanning = enable;
        self
    }

    /// Set max replans
    #[must_use]
    pub const fn max_replans(mut self, max_replans: usize) -> Self {
        self.config.max_replans = max_replans;
        self
    }

    /// Set planner system message
    #[must_use]
    pub fn planner_system_message(mut self, message: impl Into<String>) -> Self {
        self.config.planner_system_message = message.into();
        self
    }

    /// Set executor system message
    #[must_use]
    pub fn executor_system_message(mut self, message: impl Into<String>) -> Self {
        self.config.executor_system_message = message.into();
        self
    }

    /// Include execution history in executor context
    #[must_use]
    pub const fn include_execution_history(mut self, include: bool) -> Self {
        self.config.include_execution_history = include;
        self
    }

    /// Set verbose output
    #[must_use]
    pub const fn verbose(mut self, verbose: bool) -> Self {
        self.config.verbose = verbose;
        self
    }

    /// Build the agent
    pub fn build(self) -> Result<PlanAndExecuteAgent> {
        let planner_llm = self
            .planner_llm
            .ok_or_else(|| Error::Agent("Planner LLM is required".to_string()))?;

        let executor_llm = self
            .executor_llm
            .ok_or_else(|| Error::Agent("Executor LLM is required".to_string()))?;

        Ok(PlanAndExecuteAgent {
            planner_llm,
            executor_llm,
            tools: self.tools,
            config: self.config,
        })
    }
}

impl Default for PlanAndExecuteAgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(
    clippy::clone_on_ref_ptr,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::unwrap_used
)]
mod tests {
    use crate::test_prelude::*;

    #[test]
    fn test_plan_step_creation() {
        let step = PlanStep::new(1, "Test step");
        assert_eq!(step.step_number, 1);
        assert_eq!(step.description, "Test step");
        assert!(!step.completed);
        assert!(!step.failed);
    }

    #[test]
    fn test_plan_step_completion() {
        let step = PlanStep::new(1, "Test step").complete("Success");
        assert!(step.completed);
        assert_eq!(step.result, Some("Success".to_string()));
    }

    #[test]
    fn test_plan_step_failure() {
        let step = PlanStep::new(1, "Test step").fail("Error occurred");
        assert!(step.failed);
        assert_eq!(step.error, Some("Error occurred".to_string()));
    }

    #[test]
    fn test_execution_plan_creation() {
        let steps = vec![
            PlanStep::new(1, "Step 1"),
            PlanStep::new(2, "Step 2"),
            PlanStep::new(3, "Step 3"),
        ];

        let plan = ExecutionPlan::new("Test task", steps);
        assert_eq!(plan.task, "Test task");
        assert_eq!(plan.steps.len(), 3);
        assert_eq!(plan.revision_count, 0);
    }

    #[test]
    fn test_execution_plan_next_step() {
        let steps = vec![
            PlanStep::new(1, "Step 1").complete("Done"),
            PlanStep::new(2, "Step 2"),
            PlanStep::new(3, "Step 3"),
        ];

        let plan = ExecutionPlan::new("Test task", steps);
        let next = plan.next_step().unwrap();
        assert_eq!(next.step_number, 2);
    }

    #[test]
    fn test_execution_plan_progress() {
        let steps = vec![
            PlanStep::new(1, "Step 1").complete("Done"),
            PlanStep::new(2, "Step 2").complete("Done"),
            PlanStep::new(3, "Step 3"),
        ];

        let plan = ExecutionPlan::new("Test task", steps);
        assert!((plan.progress() - 0.6666).abs() < 0.01);
    }

    #[test]
    fn test_execution_plan_is_complete() {
        let steps = vec![
            PlanStep::new(1, "Step 1").complete("Done"),
            PlanStep::new(2, "Step 2").complete("Done"),
        ];

        let plan = ExecutionPlan::new("Test task", steps);
        assert!(plan.is_complete());
    }

    #[test]
    fn test_execution_plan_has_failures() {
        let steps = vec![
            PlanStep::new(1, "Step 1").complete("Done"),
            PlanStep::new(2, "Step 2").fail("Error"),
        ];

        let plan = ExecutionPlan::new("Test task", steps);
        assert!(plan.has_failures());
    }

    #[test]
    fn test_parse_numbered_line_dot_format() {
        let (num, desc) = PlanAndExecuteAgent::parse_numbered_line("1. First step").unwrap();
        assert_eq!(num, "1");
        assert_eq!(desc, "First step");
    }

    #[test]
    fn test_parse_numbered_line_paren_format() {
        let (num, desc) = PlanAndExecuteAgent::parse_numbered_line("2) Second step").unwrap();
        assert_eq!(num, "2");
        assert_eq!(desc, "Second step");
    }

    #[test]
    fn test_parse_numbered_line_invalid() {
        assert!(PlanAndExecuteAgent::parse_numbered_line("Not a numbered line").is_none());
        assert!(PlanAndExecuteAgent::parse_numbered_line("Step 1: Something").is_none());
    }

    // Helper to create a minimal agent for testing static methods
    fn create_test_agent() -> PlanAndExecuteAgent {
        use crate::core::callbacks::CallbackManager;
        use crate::core::language_models::{
            ChatGeneration, ChatResult, ToolChoice, ToolDefinition,
        };
        use crate::core::messages::{AIMessage, BaseMessage};
        use async_trait::async_trait;

        struct MockChatModel;

	        #[async_trait]
	        impl ChatModel for MockChatModel {
	            async fn _generate(
	                &self,
	                _messages: &[BaseMessage],
	                _stop: Option<&[String]>,
	                _tools: Option<&[ToolDefinition]>,
	                _tool_choice: Option<&ToolChoice>,
	                _run_manager: Option<&CallbackManager>,
            ) -> Result<ChatResult> {
                Ok(ChatResult {
                    generations: vec![ChatGeneration {
                        message: AIMessage::new("Mock response").into(),
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

        PlanAndExecuteAgent::new(Arc::new(MockChatModel), Arc::new(MockChatModel), Vec::new())
    }

    #[test]
    fn test_execution_plan_next_step_mut() {
        let steps = vec![
            PlanStep::new(1, "Step 1").complete("Done"),
            PlanStep::new(2, "Step 2"),
            PlanStep::new(3, "Step 3"),
        ];

        let mut plan = ExecutionPlan::new("Test task", steps);
        let next = plan.next_step_mut().unwrap();
        assert_eq!(next.step_number, 2);

        // Modify it
        next.completed = true;
        next.result = Some("Modified".to_string());

        // Verify mutation worked
        assert_eq!(plan.steps[1].result, Some("Modified".to_string()));
    }

    #[test]
    fn test_execution_plan_next_step_when_all_complete() {
        let steps = vec![
            PlanStep::new(1, "Step 1").complete("Done"),
            PlanStep::new(2, "Step 2").complete("Done"),
        ];

        let plan = ExecutionPlan::new("Test task", steps);
        assert!(plan.next_step().is_none());
    }

    #[test]
    fn test_execution_plan_next_step_skips_failed() {
        let steps = vec![
            PlanStep::new(1, "Step 1").complete("Done"),
            PlanStep::new(2, "Step 2").fail("Error"),
            PlanStep::new(3, "Step 3"),
        ];

        let plan = ExecutionPlan::new("Test task", steps);
        let next = plan.next_step().unwrap();
        assert_eq!(next.step_number, 3); // Should skip failed step 2
    }

    #[test]
    fn test_execution_plan_revise() {
        let steps = vec![PlanStep::new(1, "Step 1"), PlanStep::new(2, "Step 2")];

        let plan = ExecutionPlan::new("Test task", steps);
        assert_eq!(plan.revision_count, 0);

        let new_steps = vec![
            PlanStep::new(1, "Revised Step 1"),
            PlanStep::new(2, "Revised Step 2"),
            PlanStep::new(3, "New Step 3"),
        ];

        let revised_plan = plan.revise(new_steps);
        assert_eq!(revised_plan.revision_count, 1);
        assert_eq!(revised_plan.steps.len(), 3);
        assert_eq!(revised_plan.steps[0].description, "Revised Step 1");
    }

    #[test]
    fn test_execution_plan_progress_empty() {
        let steps = vec![PlanStep::new(1, "Step 1"), PlanStep::new(2, "Step 2")];

        let plan = ExecutionPlan::new("Test task", steps);
        assert_eq!(plan.progress(), 0.0);
    }

    #[test]
    fn test_execution_plan_progress_complete() {
        let steps = vec![
            PlanStep::new(1, "Step 1").complete("Done"),
            PlanStep::new(2, "Step 2").complete("Done"),
        ];

        let plan = ExecutionPlan::new("Test task", steps);
        assert_eq!(plan.progress(), 1.0);
    }

    #[test]
    fn test_execution_plan_progress_no_steps() {
        // M-992: Verify progress() returns 1.0 for empty steps (not NaN)
        let plan = ExecutionPlan::new("Empty plan", Vec::new());
        assert_eq!(plan.progress(), 1.0);
        assert!(plan.is_complete()); // Empty plan is complete
    }

    #[test]
    fn test_plan_and_execute_config_default() {
        let config = PlanAndExecuteConfig::default();
        assert_eq!(config.max_iterations, 20);
        assert!(config.enable_replanning);
        assert_eq!(config.max_replans, 3);
        assert!(config.include_execution_history);
        assert!(!config.verbose);
    }

    #[test]
    fn test_plan_and_execute_agent_builder() {
        let agent = create_test_agent();
        let configured = agent
            .with_max_iterations(10)
            .with_replanning(false)
            .with_verbose(true);

        assert_eq!(configured.config.max_iterations, 10);
        assert!(!configured.config.enable_replanning);
        assert!(configured.config.verbose);
    }

    #[test]
    fn test_plan_and_execute_agent_builder_pattern() {
        use crate::core::callbacks::CallbackManager;
        use crate::core::language_models::{
            ChatGeneration, ChatResult, ToolChoice, ToolDefinition,
        };
        use crate::core::messages::{AIMessage, BaseMessage};
        use async_trait::async_trait;

        struct MockChatModel;

	        #[async_trait]
	        impl ChatModel for MockChatModel {
	            async fn _generate(
	                &self,
	                _messages: &[BaseMessage],
	                _stop: Option<&[String]>,
	                _tools: Option<&[ToolDefinition]>,
	                _tool_choice: Option<&ToolChoice>,
	                _run_manager: Option<&CallbackManager>,
            ) -> Result<ChatResult> {
                Ok(ChatResult {
                    generations: vec![ChatGeneration {
                        message: AIMessage::new("Mock response").into(),
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

        let planner = Arc::new(MockChatModel);
        let executor = Arc::new(MockChatModel);

        let agent = PlanAndExecuteAgentBuilder::new()
            .planner_llm(planner)
            .executor_llm(executor)
            .max_iterations(15)
            .enable_replanning(false)
            .max_replans(5)
            .planner_system_message("Custom planner")
            .executor_system_message("Custom executor")
            .include_execution_history(false)
            .verbose(true)
            .build()
            .unwrap();

        assert_eq!(agent.config.max_iterations, 15);
        assert!(!agent.config.enable_replanning);
        assert_eq!(agent.config.max_replans, 5);
        assert_eq!(agent.config.planner_system_message, "Custom planner");
        assert_eq!(agent.config.executor_system_message, "Custom executor");
        assert!(!agent.config.include_execution_history);
        assert!(agent.config.verbose);
    }

    #[test]
    fn test_plan_and_execute_agent_builder_validation() {
        // Missing planner LLM
        let result = PlanAndExecuteAgentBuilder::new().build();
        assert!(result.is_err());

        use crate::core::callbacks::CallbackManager;
        use crate::core::language_models::{
            ChatGeneration, ChatResult, ToolChoice, ToolDefinition,
        };
        use crate::core::messages::{AIMessage, BaseMessage};
        use async_trait::async_trait;

        struct MockChatModel;

	        #[async_trait]
	        impl ChatModel for MockChatModel {
	            async fn _generate(
	                &self,
	                _messages: &[BaseMessage],
	                _stop: Option<&[String]>,
	                _tools: Option<&[ToolDefinition]>,
	                _tool_choice: Option<&ToolChoice>,
	                _run_manager: Option<&CallbackManager>,
            ) -> Result<ChatResult> {
                Ok(ChatResult {
                    generations: vec![ChatGeneration {
                        message: AIMessage::new("Mock response").into(),
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

        // Missing executor LLM
        let result = PlanAndExecuteAgentBuilder::new()
            .planner_llm(Arc::new(MockChatModel))
            .build();
        assert!(result.is_err());

        // Both provided - should succeed
        let result = PlanAndExecuteAgentBuilder::new()
            .planner_llm(Arc::new(MockChatModel))
            .executor_llm(Arc::new(MockChatModel))
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_plan_and_execute_agent_builder_tool_methods() {
        use crate::core::callbacks::CallbackManager;
        use crate::core::language_models::{
            ChatGeneration, ChatResult, ToolChoice, ToolDefinition,
        };
        use crate::core::messages::{AIMessage, BaseMessage};
        use crate::core::tools::Tool;
        use async_trait::async_trait;

        struct MockChatModel;

	        #[async_trait]
	        impl ChatModel for MockChatModel {
	            async fn _generate(
	                &self,
	                _messages: &[BaseMessage],
	                _stop: Option<&[String]>,
	                _tools: Option<&[ToolDefinition]>,
	                _tool_choice: Option<&ToolChoice>,
	                _run_manager: Option<&CallbackManager>,
            ) -> Result<ChatResult> {
                Ok(ChatResult {
                    generations: vec![ChatGeneration {
                        message: AIMessage::new("Mock response").into(),
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

        struct MockTool;

        #[async_trait::async_trait]
        impl Tool for MockTool {
            fn name(&self) -> &str {
                "mock_tool"
            }

            fn description(&self) -> &str {
                "A mock tool"
            }

            async fn _call(&self, _input: crate::core::tools::ToolInput) -> Result<String> {
                Ok("mock result".to_string())
            }
        }

        let planner = Arc::new(MockChatModel);
        let executor = Arc::new(MockChatModel);
        let tool1 = Arc::new(MockTool) as Arc<dyn Tool>;
        let tool2 = Arc::new(MockTool) as Arc<dyn Tool>;

        // Test tools() method
        let agent = PlanAndExecuteAgentBuilder::new()
            .planner_llm(planner.clone())
            .executor_llm(executor.clone())
            .tools(vec![tool1.clone(), tool2.clone()])
            .build()
            .unwrap();

        assert_eq!(agent.tools.len(), 2);

        // Test tool() method (incremental)
        let agent = PlanAndExecuteAgentBuilder::new()
            .planner_llm(planner)
            .executor_llm(executor)
            .tool(tool1)
            .tool(tool2)
            .build()
            .unwrap();

        assert_eq!(agent.tools.len(), 2);
    }

    #[test]
    fn test_parse_plan_empty() {
        let agent = create_test_agent();
        let result = agent.parse_plan("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_plan_valid() {
        let agent = create_test_agent();
        let plan_text = "1. First step\n2. Second step\n3. Third step";
        let steps = agent.parse_plan(plan_text).unwrap();
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].description, "First step");
        assert_eq!(steps[1].description, "Second step");
        assert_eq!(steps[2].description, "Third step");
    }

    #[test]
    fn test_parse_plan_mixed_format() {
        let agent = create_test_agent();
        let plan_text = "1. First step\n2) Second step\n3. Third step";
        let steps = agent.parse_plan(plan_text).unwrap();
        assert_eq!(steps.len(), 3);
    }

    #[test]
    fn test_parse_plan_with_extra_lines() {
        let agent = create_test_agent();
        let plan_text = "Here is the plan:\n\n1. First step\n2. Second step\n\nThat's all.";
        let steps = agent.parse_plan(plan_text).unwrap();
        assert_eq!(steps.len(), 2);
    }

    #[test]
    fn test_build_planning_prompt() {
        let agent = create_test_agent();
        let prompt = agent.build_planning_prompt("Test task");
        assert!(prompt.contains("Test task"));
        assert!(prompt.contains("step-by-step plan"));
        assert!(prompt.contains("Available tools:"));
    }

    #[test]
    fn test_build_execution_prompt() {
        let agent = create_test_agent();
        let step = PlanStep::new(2, "Do something");
        let prompt = agent.build_execution_prompt(&step, "");
        assert!(prompt.contains("Step 2"));
        assert!(prompt.contains("Do something"));
    }

    #[test]
    fn test_build_execution_prompt_with_history() {
        let agent = create_test_agent();
        let step = PlanStep::new(2, "Do something");
        let history = "Step 1: Completed\n";
        let prompt = agent.build_execution_prompt(&step, history);
        assert!(prompt.contains("Previous steps completed"));
        assert!(prompt.contains("Step 1: Completed"));
    }

    #[test]
    fn test_build_execution_history() {
        let agent = create_test_agent();
        let steps = vec![
            PlanStep::new(1, "Step 1").complete("Result 1"),
            PlanStep::new(2, "Step 2"),
            PlanStep::new(3, "Step 3").complete("Result 3"),
        ];
        let plan = ExecutionPlan::new("Test task", steps);
        let history = agent.build_execution_history(&plan);

        assert!(history.contains("Step 1: Step 1 → Result 1"));
        assert!(!history.contains("Step 2")); // Not completed
        assert!(history.contains("Step 3: Step 3 → Result 3"));
    }

    #[test]
    fn test_compile_final_result() {
        let agent = create_test_agent();
        let steps = vec![
            PlanStep::new(1, "Step 1").complete("Result 1"),
            PlanStep::new(2, "Step 2").complete("Result 2"),
        ];
        let plan = ExecutionPlan::new("Test task", steps);
        let result = agent.compile_final_result(&plan);

        assert!(result.contains("Test task"));
        assert!(result.contains("Step 1: Step 1"));
        assert!(result.contains("Result: Result 1"));
        assert!(result.contains("Step 2: Step 2"));
        assert!(result.contains("Result: Result 2"));
    }
}

// ============================================================================
// Reflection Agent Types
// ============================================================================

/// Result of a single iteration in the reflection loop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationResult {
    /// The iteration number (1-indexed)
    pub iteration: usize,

    /// The content generated by the actor
    pub content: String,

    /// The critique provided by the critic
    pub critique: String,

    /// Quality score (0.0-1.0) extracted from critique
    pub quality_score: f64,

    /// Whether this iteration met the quality threshold
    pub meets_threshold: bool,
}

impl IterationResult {
    /// Create a new iteration result
    pub fn new(
        iteration: usize,
        content: impl Into<String>,
        critique: impl Into<String>,
        quality_score: f64,
        meets_threshold: bool,
    ) -> Self {
        Self {
            iteration,
            content: content.into(),
            critique: critique.into(),
            quality_score,
            meets_threshold,
        }
    }
}

/// State for the reflection process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionState {
    /// The original task/goal
    pub task: String,

    /// History of iterations
    pub iterations: Vec<IterationResult>,

    /// Whether the process has converged
    pub converged: bool,

    /// Final refined content
    pub final_content: Option<String>,
}

impl ReflectionState {
    /// Create a new reflection state
    pub fn new(task: impl Into<String>) -> Self {
        Self {
            task: task.into(),
            iterations: Vec::new(),
            converged: false,
            final_content: None,
        }
    }

    /// Get the latest iteration result
    #[must_use]
    pub fn latest_iteration(&self) -> Option<&IterationResult> {
        self.iterations.last()
    }

    /// Get the number of iterations completed
    #[must_use]
    pub fn iteration_count(&self) -> usize {
        self.iterations.len()
    }

    /// Check if any iteration met the quality threshold
    #[must_use]
    pub fn has_quality_result(&self) -> bool {
        self.iterations.iter().any(|i| i.meets_threshold)
    }

    /// Get the best iteration by quality score
    #[must_use]
    pub fn best_iteration(&self) -> Option<&IterationResult> {
        self.iterations.iter().max_by(|a, b| {
            a.quality_score
                .partial_cmp(&b.quality_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

/// Configuration for reflection agent
#[derive(Debug, Clone)]
pub struct ReflectionConfig {
    /// Maximum number of reflection iterations
    pub max_iterations: usize,

    /// Quality threshold (0.0-1.0) for convergence
    pub quality_threshold: f64,

    /// Whether to use verbose logging
    pub verbose: bool,

    /// System message for the actor (content generator)
    pub actor_system_message: String,

    /// System message for the critic (content evaluator)
    pub critic_system_message: String,

    /// Whether to include previous critiques in actor context
    pub include_critique_history: bool,
}

impl Default for ReflectionConfig {
    fn default() -> Self {
        Self {
            max_iterations: 5,
            quality_threshold: 0.8,
            verbose: false,
            actor_system_message: "You are an expert content creator. Generate high-quality \
                                   content that meets the requirements. Consider any feedback \
                                   provided to improve your output."
                .to_string(),
            critic_system_message: "You are an expert content evaluator. Critically assess the \
                                    quality of the content provided. Provide specific, actionable \
                                    feedback. Rate the quality on a scale of 0.0 to 1.0 using \
                                    the format: QUALITY_SCORE: X.X"
                .to_string(),
            include_critique_history: true,
        }
    }
}

impl ReflectionConfig {
    /// Create a new config with default values
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum number of reflection iterations
    #[must_use]
    pub const fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Set quality threshold (0.0-1.0) for convergence
    #[must_use]
    pub const fn with_quality_threshold(mut self, threshold: f64) -> Self {
        self.quality_threshold = threshold;
        self
    }

    /// Enable or disable verbose logging
    #[must_use]
    pub const fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Set the actor (content generator) system message
    #[must_use]
    pub fn with_actor_system_message(mut self, message: impl Into<String>) -> Self {
        self.actor_system_message = message.into();
        self
    }

    /// Set the critic (content evaluator) system message
    #[must_use]
    pub fn with_critic_system_message(mut self, message: impl Into<String>) -> Self {
        self.critic_system_message = message.into();
        self
    }

    /// Set whether to include previous critiques in actor context
    #[must_use]
    pub const fn with_include_critique_history(mut self, include: bool) -> Self {
        self.include_critique_history = include;
        self
    }
}

/// Reflection agent that uses actor-critic pattern for iterative refinement
///
/// The agent follows this process:
/// 1. Actor generates initial content
/// 2. Critic evaluates and provides feedback with quality score
/// 3. If quality meets threshold → Done
/// 4. If not → Actor revises based on feedback → Repeat from step 2
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::agent_patterns::ReflectionAgent;
///
/// let agent = ReflectionAgent::builder()
///     .actor_llm(writer_llm)
///     .critic_llm(critic_llm)
///     .quality_threshold(0.8)
///     .max_iterations(5)
///     .build()?;
///
/// let result = agent.run("Write a technical analysis of Rust async programming").await?;
/// println!("Final content after {} iterations", result.iteration_count());
/// ```
pub struct ReflectionAgent {
    /// LLM for generating content (actor)
    actor_llm: Arc<dyn ChatModel>,

    /// LLM for critiquing content (critic)
    critic_llm: Arc<dyn ChatModel>,

    /// Configuration
    config: ReflectionConfig,
}

impl ReflectionAgent {
    /// Create a new reflection agent
    pub fn new(actor_llm: Arc<dyn ChatModel>, critic_llm: Arc<dyn ChatModel>) -> Self {
        Self {
            actor_llm,
            critic_llm,
            config: ReflectionConfig::default(),
        }
    }

    /// Create a builder for configuring the agent
    #[must_use]
    pub fn builder() -> ReflectionAgentBuilder {
        ReflectionAgentBuilder::new()
    }

    /// Set configuration
    #[must_use]
    pub fn with_config(mut self, config: ReflectionConfig) -> Self {
        self.config = config;
        self
    }

    /// Set maximum iterations
    #[must_use]
    pub const fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.config.max_iterations = max_iterations;
        self
    }

    /// Set quality threshold
    #[must_use]
    pub fn with_quality_threshold(mut self, threshold: f64) -> Self {
        self.config.quality_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Set verbose mode
    #[must_use]
    pub const fn with_verbose(mut self, verbose: bool) -> Self {
        self.config.verbose = verbose;
        self
    }

    /// Run the reflection loop
    pub async fn run(&self, task: impl Into<String>) -> Result<ReflectionState> {
        let task = task.into();
        let mut state = ReflectionState::new(task.clone());

        if self.config.verbose {
            tracing::debug!("[Reflection] Starting reflection loop for: {task}");
            tracing::debug!(
                "[Reflection] Max iterations: {}, Quality threshold: {}",
                self.config.max_iterations,
                self.config.quality_threshold
            );
        }

        for iteration in 1..=self.config.max_iterations {
            if self.config.verbose {
                tracing::debug!("[Reflection] Iteration {iteration}");
            }

            // Step 1: Actor generates/revises content
            let content = self.generate_content(&state, iteration).await?;

            if self.config.verbose {
                tracing::debug!("[Reflection] Generated content ({} chars)", content.len());
            }

            // Step 2: Critic evaluates content
            let (critique, quality_score) = self.evaluate_content(&content, &task).await?;

            if self.config.verbose {
                tracing::debug!("[Reflection] Quality score: {quality_score:.2}");
                tracing::debug!("[Reflection] Critique: {critique}");
            }

            // Step 3: Check if quality threshold met
            let meets_threshold = quality_score >= self.config.quality_threshold;

            let iter_result = IterationResult::new(
                iteration,
                content.clone(),
                critique,
                quality_score,
                meets_threshold,
            );

            state.iterations.push(iter_result);

            if meets_threshold {
                if self.config.verbose {
                    tracing::debug!("[Reflection] Quality threshold met! Converged.");
                }
                state.converged = true;
                state.final_content = Some(content);
                break;
            }

            if iteration == self.config.max_iterations {
                if self.config.verbose {
                    tracing::debug!("[Reflection] Max iterations reached. Using best result.");
                }
                // Use best iteration's content as final
                if let Some(best) = state.best_iteration() {
                    state.final_content = Some(best.content.clone());
                }
            }
        }

        Ok(state)
    }

    /// Generate or revise content based on current state
    async fn generate_content(&self, state: &ReflectionState, iteration: usize) -> Result<String> {
        let prompt = if iteration == 1 {
            // First iteration - generate initial content
            format!(
                "Task: {}\n\nGenerate content to complete this task.",
                state.task
            )
        } else {
            // Subsequent iterations - revise based on feedback
            let latest = state
                .latest_iteration()
                .ok_or_else(|| Error::invalid_input("No iterations in state"))?;
            let mut p = format!(
                "Task: {}\n\nPrevious attempt:\n{}\n\nCritique:\n{}\n\n",
                state.task, latest.content, latest.critique
            );

            if self.config.include_critique_history && state.iterations.len() > 1 {
                p.push_str("Previous critiques:\n");
                for (i, iter) in state.iterations[..state.iterations.len() - 1]
                    .iter()
                    .enumerate()
                {
                    p.push_str(&format!(
                        "Iteration {}: Score {:.2} - {}\n",
                        i + 1,
                        iter.quality_score,
                        iter.critique
                    ));
                }
                p.push('\n');
            }

            p.push_str("Revise your content based on the feedback above.");
            p
        };

        let messages = vec![
            Message::system(self.config.actor_system_message.clone()),
            Message::human(prompt),
        ];

        let result = self
            .actor_llm
            .generate(&messages, None, None, None, None)
            .await?;
        let content = result
            .generations
            .first()
            .ok_or_else(|| Error::invalid_input("No generation from actor"))?
            .message
            .content()
            .as_text();

        Ok(content)
    }

    /// Evaluate content quality and extract score
    async fn evaluate_content(&self, content: &str, task: &str) -> Result<(String, f64)> {
        let prompt = format!(
            "Task: {task}\n\nContent to evaluate:\n{content}\n\n\
             Please evaluate this content and provide:\n\
             1. Specific feedback on strengths and weaknesses\n\
             2. Actionable suggestions for improvement\n\
             3. A quality score from 0.0 to 1.0\n\n\
             Format your score as: QUALITY_SCORE: X.X"
        );

        let messages = vec![
            Message::system(self.config.critic_system_message.clone()),
            Message::human(prompt),
        ];

        let result = self
            .critic_llm
            .generate(&messages, None, None, None, None)
            .await?;
        let critique = result
            .generations
            .first()
            .ok_or_else(|| Error::invalid_input("No generation from critic"))?
            .message
            .content()
            .as_text();

        // Extract quality score from critique
        let quality_score = Self::extract_quality_score(&critique)?;

        Ok((critique, quality_score))
    }

    /// Extract quality score from critique text
    ///
    /// Looks for pattern: "`QUALITY_SCORE`: X.X"
    fn extract_quality_score(text: &str) -> Result<f64> {
        // Look for pattern "QUALITY_SCORE: X.X"
        for line in text.lines() {
            if line.contains("QUALITY_SCORE:") {
                // Extract the number after the colon
                if let Some(score_str) = line.split(':').nth(1) {
                    if let Ok(score) = score_str.trim().parse::<f64>() {
                        return Ok(score.clamp(0.0, 1.0));
                    }
                }
            }
        }

        Err(Error::Agent(
            "Could not extract QUALITY_SCORE from critique".to_string(),
        ))
    }
}

/// Builder for configuring `ReflectionAgent`
pub struct ReflectionAgentBuilder {
    actor_llm: Option<Arc<dyn ChatModel>>,
    critic_llm: Option<Arc<dyn ChatModel>>,
    max_iterations: usize,
    quality_threshold: f64,
    actor_system_message: Option<String>,
    critic_system_message: Option<String>,
    include_critique_history: bool,
    verbose: bool,
}

impl ReflectionAgentBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        let defaults = ReflectionConfig::default();
        Self {
            actor_llm: None,
            critic_llm: None,
            max_iterations: defaults.max_iterations,
            quality_threshold: defaults.quality_threshold,
            actor_system_message: None,
            critic_system_message: None,
            include_critique_history: defaults.include_critique_history,
            verbose: defaults.verbose,
        }
    }

    /// Set the actor LLM (content generator)
    #[must_use]
    pub fn actor_llm(mut self, llm: Arc<dyn ChatModel>) -> Self {
        self.actor_llm = Some(llm);
        self
    }

    /// Set the critic LLM (content evaluator)
    #[must_use]
    pub fn critic_llm(mut self, llm: Arc<dyn ChatModel>) -> Self {
        self.critic_llm = Some(llm);
        self
    }

    /// Set maximum number of iterations
    #[must_use]
    pub const fn max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Set quality threshold for convergence (0.0-1.0)
    #[must_use]
    pub fn quality_threshold(mut self, threshold: f64) -> Self {
        self.quality_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Set custom actor system message
    #[must_use]
    pub fn actor_system_message(mut self, message: impl Into<String>) -> Self {
        self.actor_system_message = Some(message.into());
        self
    }

    /// Set custom critic system message
    #[must_use]
    pub fn critic_system_message(mut self, message: impl Into<String>) -> Self {
        self.critic_system_message = Some(message.into());
        self
    }

    /// Set whether to include critique history in actor context
    #[must_use]
    pub const fn include_critique_history(mut self, include: bool) -> Self {
        self.include_critique_history = include;
        self
    }

    /// Set verbose mode
    #[must_use]
    pub const fn verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Build the `ReflectionAgent`
    pub fn build(self) -> Result<ReflectionAgent> {
        let actor_llm = self
            .actor_llm
            .ok_or_else(|| Error::Agent("Actor LLM is required".to_string()))?;

        let critic_llm = self
            .critic_llm
            .ok_or_else(|| Error::Agent("Critic LLM is required".to_string()))?;

        let defaults = ReflectionConfig::default();
        let config = ReflectionConfig {
            max_iterations: self.max_iterations,
            quality_threshold: self.quality_threshold,
            verbose: self.verbose,
            actor_system_message: self
                .actor_system_message
                .unwrap_or(defaults.actor_system_message),
            critic_system_message: self
                .critic_system_message
                .unwrap_or(defaults.critic_system_message),
            include_critique_history: self.include_critique_history,
        };

        Ok(ReflectionAgent {
            actor_llm,
            critic_llm,
            config,
        })
    }
}

impl Default for ReflectionAgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Reflection Agent Tests
// ============================================================================

#[cfg(test)]
#[allow(
    clippy::clone_on_ref_ptr,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::unwrap_used
)]
mod reflection_tests {
    use crate::core::callbacks::CallbackManager;
    use crate::core::language_models::{
        ChatGeneration, ChatModel, ChatResult, ToolChoice, ToolDefinition,
    };
    use crate::core::messages::AIMessage;
    use crate::test_prelude::*;

    struct MockActorLLM {
        iteration: std::sync::atomic::AtomicUsize,
    }

    impl MockActorLLM {
        fn new() -> Self {
            Self {
                iteration: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

	    #[async_trait::async_trait]
	    impl ChatModel for MockActorLLM {
	        async fn _generate(
	            &self,
	            _messages: &[crate::core::messages::BaseMessage],
	            _stop: Option<&[String]>,
	            _tools: Option<&[ToolDefinition]>,
	            _tool_choice: Option<&ToolChoice>,
	            _run_manager: Option<&CallbackManager>,
        ) -> Result<ChatResult> {
            let iter = self
                .iteration
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            let content = match iter {
                0 => "Initial draft: Basic content.",
                1 => "Revised draft: Added more details.",
                2 => "Final draft: Comprehensive content with examples.",
                _ => "Further revision.",
            };

            Ok(ChatResult {
                generations: vec![ChatGeneration {
                    message: AIMessage::new(content).into(),
                    generation_info: None,
                }],
                llm_output: None,
            })
        }

        fn llm_type(&self) -> &str {
            "mock_actor"
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    struct MockCriticLLM {
        iteration: std::sync::atomic::AtomicUsize,
    }

    impl MockCriticLLM {
        fn new() -> Self {
            Self {
                iteration: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

	    #[async_trait::async_trait]
	    impl ChatModel for MockCriticLLM {
	        async fn _generate(
	            &self,
	            _messages: &[crate::core::messages::BaseMessage],
	            _stop: Option<&[String]>,
	            _tools: Option<&[ToolDefinition]>,
	            _tool_choice: Option<&ToolChoice>,
	            _run_manager: Option<&CallbackManager>,
        ) -> Result<ChatResult> {
            let iter = self
                .iteration
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            let critique = match iter {
                0 => "Too brief. Needs more detail. QUALITY_SCORE: 0.4",
                1 => "Better but lacks examples. QUALITY_SCORE: 0.6",
                2 => "Excellent! Comprehensive with examples. QUALITY_SCORE: 0.9",
                _ => "Good quality. QUALITY_SCORE: 0.8",
            };

            Ok(ChatResult {
                generations: vec![ChatGeneration {
                    message: AIMessage::new(critique).into(),
                    generation_info: None,
                }],
                llm_output: None,
            })
        }

        fn llm_type(&self) -> &str {
            "mock_critic"
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[tokio::test]
    async fn test_reflection_convergence() {
        let actor = Arc::new(MockActorLLM::new());
        let critic = Arc::new(MockCriticLLM::new());

        let agent = ReflectionAgent::new(actor, critic)
            .with_quality_threshold(0.8)
            .with_max_iterations(5);

        let result = agent.run("Write an analysis").await.unwrap();

        assert!(result.converged);
        assert_eq!(result.iteration_count(), 3); // Should converge on iteration 3
        assert!(result.final_content.is_some());
        assert!(result.has_quality_result());
    }

    #[tokio::test]
    async fn test_reflection_max_iterations() {
        let actor = Arc::new(MockActorLLM::new());
        let critic = Arc::new(MockCriticLLM::new());

        let agent = ReflectionAgent::new(actor, critic)
            .with_quality_threshold(1.0) // Unreachable threshold
            .with_max_iterations(2);

        let result = agent.run("Write an analysis").await.unwrap();

        assert!(!result.converged);
        assert_eq!(result.iteration_count(), 2);
        assert!(result.final_content.is_some()); // Should still have best result
    }

    #[tokio::test]
    async fn test_iteration_result_creation() {
        let iter = IterationResult::new(1, "content", "critique", 0.75, false);

        assert_eq!(iter.iteration, 1);
        assert_eq!(iter.content, "content");
        assert_eq!(iter.critique, "critique");
        assert_eq!(iter.quality_score, 0.75);
        assert!(!iter.meets_threshold);
    }

    #[tokio::test]
    async fn test_reflection_state() {
        let mut state = ReflectionState::new("Test task");

        assert_eq!(state.task, "Test task");
        assert_eq!(state.iteration_count(), 0);
        assert!(state.latest_iteration().is_none());
        assert!(!state.has_quality_result());

        state
            .iterations
            .push(IterationResult::new(1, "content1", "critique1", 0.5, false));
        state
            .iterations
            .push(IterationResult::new(2, "content2", "critique2", 0.9, true));

        assert_eq!(state.iteration_count(), 2);
        assert!(state.latest_iteration().is_some());
        assert_eq!(state.latest_iteration().unwrap().iteration, 2);
        assert!(state.has_quality_result());
        assert_eq!(state.best_iteration().unwrap().quality_score, 0.9);
    }

    #[tokio::test]
    async fn test_quality_score_extraction() {
        let text1 = "This is good. QUALITY_SCORE: 0.85";
        let score1 = ReflectionAgent::extract_quality_score(text1).unwrap();
        assert_eq!(score1, 0.85);

        let text2 = "Feedback here.\nQUALITY_SCORE: 0.6\nMore text.";
        let score2 = ReflectionAgent::extract_quality_score(text2).unwrap();
        assert_eq!(score2, 0.6);

        let text3 = "No score here";
        assert!(ReflectionAgent::extract_quality_score(text3).is_err());
    }

    #[tokio::test]
    async fn test_reflection_builder() {
        let actor = Arc::new(MockActorLLM::new());
        let critic = Arc::new(MockCriticLLM::new());

        let agent = ReflectionAgent::builder()
            .actor_llm(actor)
            .critic_llm(critic)
            .max_iterations(10)
            .quality_threshold(0.75)
            .verbose(true)
            .include_critique_history(false)
            .build()
            .unwrap();

        assert_eq!(agent.config.max_iterations, 10);
        assert_eq!(agent.config.quality_threshold, 0.75);
        assert!(agent.config.verbose);
        assert!(!agent.config.include_critique_history);
    }

    #[tokio::test]
    async fn test_reflection_builder_validation() {
        // Missing actor LLM
        let result = ReflectionAgent::builder()
            .critic_llm(Arc::new(MockCriticLLM::new()))
            .build();
        assert!(result.is_err());

        // Missing critic LLM
        let result = ReflectionAgent::builder()
            .actor_llm(Arc::new(MockActorLLM::new()))
            .build();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_quality_threshold_clamping() {
        let actor = Arc::new(MockActorLLM::new());
        let critic = Arc::new(MockCriticLLM::new());

        let agent = ReflectionAgent::builder()
            .actor_llm(actor)
            .critic_llm(critic)
            .quality_threshold(1.5) // Out of range
            .build()
            .unwrap();

        assert_eq!(agent.config.quality_threshold, 1.0); // Clamped to 1.0
    }
}

// ============================================================================
// Multi-Agent Debate Types
// ============================================================================

/// A debater with a specific perspective/role
#[derive(Clone)]
pub struct Debater {
    /// Name/identifier for this debater
    pub name: String,

    /// Perspective or role description for this debater
    pub perspective: String,

    /// The LLM that will represent this debater
    pub llm: Arc<dyn ChatModel>,
}

impl std::fmt::Debug for Debater {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Debater")
            .field("name", &self.name)
            .field("perspective", &self.perspective)
            .field("llm", &"<ChatModel>")
            .finish()
    }
}

impl Debater {
    /// Create a new debater
    pub fn new(
        name: impl Into<String>,
        perspective: impl Into<String>,
        llm: Arc<dyn ChatModel>,
    ) -> Self {
        Self {
            name: name.into(),
            perspective: perspective.into(),
            llm,
        }
    }
}

/// A single contribution in the debate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebateContribution {
    /// The debater who made this contribution
    pub debater_name: String,

    /// The round number (1-indexed)
    pub round: usize,

    /// The contribution text
    pub content: String,
}

impl DebateContribution {
    /// Create a new debate contribution
    pub fn new(debater_name: impl Into<String>, round: usize, content: impl Into<String>) -> Self {
        Self {
            debater_name: debater_name.into(),
            round,
            content: content.into(),
        }
    }
}

/// A complete round of debate (all debaters contribute once)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebateRound {
    /// Round number (1-indexed)
    pub round_number: usize,

    /// Contributions in this round
    pub contributions: Vec<DebateContribution>,
}

impl DebateRound {
    /// Create a new debate round
    #[must_use]
    pub const fn new(round_number: usize) -> Self {
        Self {
            round_number,
            contributions: Vec::new(),
        }
    }

    /// Add a contribution to this round
    pub fn add_contribution(&mut self, contribution: DebateContribution) {
        self.contributions.push(contribution);
    }
}

/// State for the multi-agent debate process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebateState {
    /// The question or topic being debated
    pub topic: String,

    /// History of debate rounds
    pub rounds: Vec<DebateRound>,

    /// Final consensus (if reached)
    pub consensus: Option<String>,

    /// Whether the debate has concluded
    pub concluded: bool,
}

impl DebateState {
    /// Create a new debate state
    pub fn new(topic: impl Into<String>) -> Self {
        Self {
            topic: topic.into(),
            rounds: Vec::new(),
            consensus: None,
            concluded: false,
        }
    }

    /// Get the number of completed rounds
    #[must_use]
    pub fn round_count(&self) -> usize {
        self.rounds.len()
    }

    /// Get all contributions in chronological order
    #[must_use]
    pub fn all_contributions(&self) -> Vec<&DebateContribution> {
        self.rounds
            .iter()
            .flat_map(|round| &round.contributions)
            .collect()
    }

    /// Get the latest round
    #[must_use]
    pub fn latest_round(&self) -> Option<&DebateRound> {
        self.rounds.last()
    }

    /// Get contributions from a specific debater
    #[must_use]
    pub fn contributions_by_debater(&self, debater_name: &str) -> Vec<&DebateContribution> {
        self.all_contributions()
            .into_iter()
            .filter(|c| c.debater_name == debater_name)
            .collect()
    }
}

/// Configuration for multi-agent debate
#[derive(Debug, Clone)]
pub struct DebateConfig {
    /// Maximum number of debate rounds
    pub max_rounds: usize,

    /// System message for debaters
    pub debater_system_message: Option<String>,

    /// System message for moderator
    pub moderator_system_message: Option<String>,

    /// Whether to include debate history in prompts
    pub include_debate_history: bool,

    /// Whether to print verbose output
    pub verbose: bool,
}

impl Default for DebateConfig {
    fn default() -> Self {
        Self {
            max_rounds: 3,
            debater_system_message: Some(
                "You are participating in a collaborative debate. \
                Present your perspective clearly and engage constructively with other viewpoints. \
                Your role: {perspective}".to_string()
            ),
            moderator_system_message: Some(
                "You are moderating a multi-agent debate. \
                After hearing all perspectives, synthesize the key points and provide a balanced consensus. \
                Consider all viewpoints fairly.".to_string()
            ),
            include_debate_history: true,
            verbose: false,
        }
    }
}

impl DebateConfig {
    /// Create a new config with default values
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum number of debate rounds
    #[must_use]
    pub const fn with_max_rounds(mut self, max_rounds: usize) -> Self {
        self.max_rounds = max_rounds;
        self
    }

    /// Set the debater system message
    #[must_use]
    pub fn with_debater_system_message(mut self, message: impl Into<String>) -> Self {
        self.debater_system_message = Some(message.into());
        self
    }

    /// Set the moderator system message
    #[must_use]
    pub fn with_moderator_system_message(mut self, message: impl Into<String>) -> Self {
        self.moderator_system_message = Some(message.into());
        self
    }

    /// Set whether to include debate history in prompts
    #[must_use]
    pub const fn with_include_debate_history(mut self, include: bool) -> Self {
        self.include_debate_history = include;
        self
    }

    /// Enable or disable verbose output
    #[must_use]
    pub const fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }
}

/// Multi-agent debate agent for collaborative reasoning
pub struct MultiAgentDebate {
    /// List of debaters participating
    debaters: Vec<Debater>,

    /// Optional moderator LLM to synthesize consensus
    moderator: Option<Arc<dyn ChatModel>>,

    /// Configuration
    config: DebateConfig,
}

impl MultiAgentDebate {
    /// Create a new multi-agent debate
    #[must_use]
    pub fn new(debaters: Vec<Debater>) -> Self {
        Self {
            debaters,
            moderator: None,
            config: DebateConfig::default(),
        }
    }

    /// Set the moderator LLM
    #[must_use]
    pub fn with_moderator(mut self, moderator: Arc<dyn ChatModel>) -> Self {
        self.moderator = Some(moderator);
        self
    }

    /// Set the maximum number of rounds
    #[must_use]
    pub const fn with_max_rounds(mut self, max_rounds: usize) -> Self {
        self.config.max_rounds = max_rounds;
        self
    }

    /// Set the debater system message
    #[must_use]
    pub fn with_debater_system_message(mut self, message: impl Into<String>) -> Self {
        self.config.debater_system_message = Some(message.into());
        self
    }

    /// Set the moderator system message
    #[must_use]
    pub fn with_moderator_system_message(mut self, message: impl Into<String>) -> Self {
        self.config.moderator_system_message = Some(message.into());
        self
    }

    /// Set whether to include debate history
    #[must_use]
    pub const fn with_include_debate_history(mut self, include: bool) -> Self {
        self.config.include_debate_history = include;
        self
    }

    /// Set verbose mode
    #[must_use]
    pub const fn with_verbose(mut self, verbose: bool) -> Self {
        self.config.verbose = verbose;
        self
    }

    /// Create a builder for configuring the debate
    #[must_use]
    pub fn builder() -> MultiAgentDebateBuilder {
        MultiAgentDebateBuilder::new()
    }

    /// Run the multi-agent debate
    pub async fn run(&self, topic: impl Into<String>) -> Result<DebateState> {
        let topic = topic.into();
        let mut state = DebateState::new(&topic);

        if self.debaters.is_empty() {
            return Err(Error::InvalidInput(
                "At least one debater is required".to_string(),
            ));
        }

        if self.config.verbose {
            tracing::debug!("[Multi-Agent Debate] Starting debate on: {topic}");
            tracing::debug!(
                "[Multi-Agent Debate] Debaters: {}",
                self.debaters
                    .iter()
                    .map(|d| d.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            tracing::debug!(
                "[Multi-Agent Debate] Max rounds: {}",
                self.config.max_rounds
            );
        }

        // Conduct debate rounds
        for round_num in 1..=self.config.max_rounds {
            if self.config.verbose {
                tracing::debug!("[Multi-Agent Debate] === Round {round_num} ===");
            }

            let mut round = DebateRound::new(round_num);

            // Each debater contributes in sequence
            for debater in &self.debaters {
                let contribution = self
                    .get_debater_contribution(debater, &state, round_num)
                    .await?;

                if self.config.verbose {
                    tracing::debug!("[{}] {}", debater.name, contribution.content);
                }

                round.add_contribution(contribution);
            }

            state.rounds.push(round);
        }

        // Synthesize consensus with moderator (if available)
        if let Some(ref moderator) = self.moderator {
            if self.config.verbose {
                tracing::debug!("[Multi-Agent Debate] === Synthesizing Consensus ===");
            }

            let consensus = self.synthesize_consensus(moderator, &state).await?;

            if self.config.verbose {
                tracing::debug!("[Moderator] {consensus}");
            }

            state.consensus = Some(consensus);
        } else {
            // No moderator - return all contributions as summary
            if self.config.verbose {
                tracing::debug!("[Multi-Agent Debate] No moderator - debate complete");
            }
        }

        state.concluded = true;

        if self.config.verbose {
            tracing::debug!("[Multi-Agent Debate] Debate concluded");
        }

        Ok(state)
    }

    /// Get a contribution from a debater
    async fn get_debater_contribution(
        &self,
        debater: &Debater,
        state: &DebateState,
        round_num: usize,
    ) -> Result<DebateContribution> {
        let mut prompt = format!(
            "Topic: {}\n\nYour perspective: {}\n\n",
            state.topic, debater.perspective
        );

        // Add debate history if enabled
        if self.config.include_debate_history && !state.rounds.is_empty() {
            prompt.push_str("Previous contributions:\n");
            for contribution in state.all_contributions() {
                prompt.push_str(&format!(
                    "\n[{}] (Round {}): {}\n",
                    contribution.debater_name, contribution.round, contribution.content
                ));
            }
            prompt.push('\n');
        }

        prompt.push_str("Please provide your contribution to the debate:");

        // Create messages with system message if available
        let mut messages = Vec::new();

        if let Some(ref system_msg) = self.config.debater_system_message {
            let system_msg = system_msg.replace("{perspective}", &debater.perspective);
            messages.push(Message::system(system_msg.as_str()));
        }

        messages.push(Message::human(prompt.as_str()));

        // Generate response
        let result = debater
            .llm
            ._generate(&messages, None, None, None, None)
            .await?;

        let content = result
            .generations
            .first()
            .ok_or_else(|| Error::invalid_input("No generation from debater"))?
            .message
            .content()
            .as_text();

        Ok(DebateContribution::new(
            debater.name.clone(),
            round_num,
            content,
        ))
    }

    /// Synthesize consensus from debate using moderator
    async fn synthesize_consensus(
        &self,
        moderator: &Arc<dyn ChatModel>,
        state: &DebateState,
    ) -> Result<String> {
        let mut prompt = format!("Topic: {}\n\nDebate Summary:\n", state.topic);

        // Add all contributions organized by round
        for round in &state.rounds {
            prompt.push_str(&format!("\n--- Round {} ---\n", round.round_number));
            for contribution in &round.contributions {
                prompt.push_str(&format!(
                    "\n[{}]: {}\n",
                    contribution.debater_name, contribution.content
                ));
            }
        }

        prompt.push_str(
            "\n\nPlease synthesize these perspectives into a balanced consensus that \
            captures the key insights from all debaters:",
        );

        // Create messages with system message if available
        let mut messages = Vec::new();

        if let Some(ref system_msg) = self.config.moderator_system_message {
            messages.push(Message::system(system_msg.as_str()));
        }

        messages.push(Message::human(prompt.as_str()));

        // Generate response
        let result = moderator
            ._generate(&messages, None, None, None, None)
            .await?;

        let consensus = result
            .generations
            .first()
            .ok_or_else(|| Error::invalid_input("No consensus from moderator"))?
            .message
            .content()
            .as_text();

        Ok(consensus)
    }
}

/// Builder for `MultiAgentDebate`
#[derive(Default)]
pub struct MultiAgentDebateBuilder {
    debaters: Vec<Debater>,
    moderator: Option<Arc<dyn ChatModel>>,
    max_rounds: Option<usize>,
    debater_system_message: Option<String>,
    moderator_system_message: Option<String>,
    include_debate_history: Option<bool>,
    verbose: Option<bool>,
}

impl MultiAgentDebateBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a debater with name and perspective
    #[must_use]
    pub fn add_debater(
        mut self,
        name: impl Into<String>,
        perspective: impl Into<String>,
        llm: Arc<dyn ChatModel>,
    ) -> Self {
        self.debaters.push(Debater::new(name, perspective, llm));
        self
    }

    /// Add a pre-configured debater
    #[must_use]
    pub fn add_debater_instance(mut self, debater: Debater) -> Self {
        self.debaters.push(debater);
        self
    }

    /// Set all debaters at once
    #[must_use]
    pub fn debaters(mut self, debaters: Vec<Debater>) -> Self {
        self.debaters = debaters;
        self
    }

    /// Set the moderator
    #[must_use]
    pub fn moderator(mut self, moderator: Arc<dyn ChatModel>) -> Self {
        self.moderator = Some(moderator);
        self
    }

    /// Set max rounds
    #[must_use]
    pub const fn max_rounds(mut self, max_rounds: usize) -> Self {
        self.max_rounds = Some(max_rounds);
        self
    }

    /// Set debater system message
    #[must_use]
    pub fn debater_system_message(mut self, message: impl Into<String>) -> Self {
        self.debater_system_message = Some(message.into());
        self
    }

    /// Set moderator system message
    #[must_use]
    pub fn moderator_system_message(mut self, message: impl Into<String>) -> Self {
        self.moderator_system_message = Some(message.into());
        self
    }

    /// Set whether to include debate history
    #[must_use]
    pub const fn include_debate_history(mut self, include: bool) -> Self {
        self.include_debate_history = Some(include);
        self
    }

    /// Set verbose mode
    #[must_use]
    pub const fn verbose(mut self, verbose: bool) -> Self {
        self.verbose = Some(verbose);
        self
    }

    /// Build the `MultiAgentDebate`
    pub fn build(self) -> Result<MultiAgentDebate> {
        if self.debaters.is_empty() {
            return Err(Error::InvalidInput(
                "At least one debater is required".to_string(),
            ));
        }

        let mut config = DebateConfig::default();

        if let Some(max_rounds) = self.max_rounds {
            config.max_rounds = max_rounds;
        }

        if let Some(msg) = self.debater_system_message {
            config.debater_system_message = Some(msg);
        }

        if let Some(msg) = self.moderator_system_message {
            config.moderator_system_message = Some(msg);
        }

        if let Some(include) = self.include_debate_history {
            config.include_debate_history = include;
        }

        if let Some(verbose) = self.verbose {
            config.verbose = verbose;
        }

        Ok(MultiAgentDebate {
            debaters: self.debaters,
            moderator: self.moderator,
            config,
        })
    }
}

// ============================================================================
// Tests - Multi-Agent Debate
// ============================================================================

#[cfg(test)]
#[allow(
    clippy::clone_on_ref_ptr,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::panic,
    clippy::unwrap_used
)]
mod multi_agent_debate_tests {
    use crate::core::callbacks::CallbackManager;
    use crate::core::language_models::{ChatGeneration, ChatResult, ToolChoice, ToolDefinition};
    use crate::core::messages::{AIMessage, BaseMessage};
    use crate::test_prelude::*;
    use async_trait::async_trait;

    /// Mock debater LLM that generates perspective-based responses
    struct MockDebaterLLM {
        name: String,
        responses: Vec<String>,
        call_count: std::sync::Arc<std::sync::Mutex<usize>>,
    }

    impl MockDebaterLLM {
        fn new(name: impl Into<String>, responses: Vec<String>) -> Self {
            Self {
                name: name.into(),
                responses,
                call_count: std::sync::Arc::new(std::sync::Mutex::new(0)),
            }
        }
    }

	    #[async_trait]
	    impl ChatModel for MockDebaterLLM {
	        async fn _generate(
	            &self,
	            _messages: &[BaseMessage],
	            _stop: Option<&[String]>,
	            _tools: Option<&[ToolDefinition]>,
	            _tool_choice: Option<&ToolChoice>,
	            _run_manager: Option<&CallbackManager>,
        ) -> Result<ChatResult> {
            let mut count = self.call_count.lock().unwrap();
            let response = self
                .responses
                .get(*count)
                .cloned()
                .unwrap_or_else(|| format!("{} contribution {}", self.name, *count + 1));
            *count += 1;

            Ok(ChatResult {
                generations: vec![ChatGeneration {
                    message: AIMessage::new(response).into(),
                    generation_info: None,
                }],
                llm_output: None,
            })
        }

        fn llm_type(&self) -> &str {
            "mock_debater"
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    /// Mock moderator LLM that synthesizes consensus
    struct MockModeratorLLM {
        consensus: String,
    }

    impl MockModeratorLLM {
        fn new(consensus: impl Into<String>) -> Self {
            Self {
                consensus: consensus.into(),
            }
        }
    }

	    #[async_trait]
	    impl ChatModel for MockModeratorLLM {
	        async fn _generate(
	            &self,
	            _messages: &[BaseMessage],
	            _stop: Option<&[String]>,
	            _tools: Option<&[ToolDefinition]>,
	            _tool_choice: Option<&ToolChoice>,
	            _run_manager: Option<&CallbackManager>,
        ) -> Result<ChatResult> {
            Ok(ChatResult {
                generations: vec![ChatGeneration {
                    message: AIMessage::new(self.consensus.clone()).into(),
                    generation_info: None,
                }],
                llm_output: None,
            })
        }

        fn llm_type(&self) -> &str {
            "mock_moderator"
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[tokio::test]
    async fn test_debate_contribution_creation() {
        let contribution = DebateContribution::new("Alice", 1, "I think we should proceed");

        assert_eq!(contribution.debater_name, "Alice");
        assert_eq!(contribution.round, 1);
        assert_eq!(contribution.content, "I think we should proceed");
    }

    #[tokio::test]
    async fn test_debate_round_creation() {
        let mut round = DebateRound::new(1);
        assert_eq!(round.round_number, 1);
        assert!(round.contributions.is_empty());

        round.add_contribution(DebateContribution::new("Alice", 1, "First point"));
        round.add_contribution(DebateContribution::new("Bob", 1, "Second point"));

        assert_eq!(round.contributions.len(), 2);
    }

    #[tokio::test]
    async fn test_debate_state_creation() {
        let state = DebateState::new("Should we adopt AI?");

        assert_eq!(state.topic, "Should we adopt AI?");
        assert_eq!(state.round_count(), 0);
        assert!(state.consensus.is_none());
        assert!(!state.concluded);
    }

    #[tokio::test]
    async fn test_debate_state_queries() {
        let mut state = DebateState::new("Test topic");

        let mut round1 = DebateRound::new(1);
        round1.add_contribution(DebateContribution::new("Alice", 1, "Alice's point"));
        round1.add_contribution(DebateContribution::new("Bob", 1, "Bob's point"));

        let mut round2 = DebateRound::new(2);
        round2.add_contribution(DebateContribution::new("Alice", 2, "Alice's followup"));

        state.rounds.push(round1);
        state.rounds.push(round2);

        assert_eq!(state.round_count(), 2);
        assert_eq!(state.all_contributions().len(), 3);
        assert_eq!(state.latest_round().unwrap().round_number, 2);

        let alice_contributions = state.contributions_by_debater("Alice");
        assert_eq!(alice_contributions.len(), 2);
    }

    #[tokio::test]
    async fn test_multi_agent_debate_basic() {
        let alice_llm = Arc::new(MockDebaterLLM::new(
            "Alice",
            vec![
                "We should prioritize safety".to_string(),
                "Safety metrics are crucial".to_string(),
                "I agree with the balanced approach".to_string(),
            ],
        ));

        let bob_llm = Arc::new(MockDebaterLLM::new(
            "Bob",
            vec![
                "We should move quickly".to_string(),
                "Speed enables innovation".to_string(),
                "Balance is important".to_string(),
            ],
        ));

        let debate = MultiAgentDebate::new(vec![
            Debater::new("Alice", "Safety-focused", alice_llm),
            Debater::new("Bob", "Innovation-focused", bob_llm),
        ])
        .with_max_rounds(3);

        let result = debate
            .run("Should we deploy this AI system?")
            .await
            .unwrap();

        assert_eq!(result.round_count(), 3);
        assert_eq!(result.all_contributions().len(), 6); // 2 debaters × 3 rounds
        assert!(result.concluded);
    }

    #[tokio::test]
    async fn test_multi_agent_debate_with_moderator() {
        let alice_llm = Arc::new(MockDebaterLLM::new(
            "Alice",
            vec!["Safety first".to_string()],
        ));
        let bob_llm = Arc::new(MockDebaterLLM::new(
            "Bob",
            vec!["Innovation first".to_string()],
        ));
        let moderator_llm = Arc::new(MockModeratorLLM::new(
            "Both safety and innovation are important. We should adopt a phased approach.",
        ));

        let debate = MultiAgentDebate::new(vec![
            Debater::new("Alice", "Safety-focused", alice_llm),
            Debater::new("Bob", "Innovation-focused", bob_llm),
        ])
        .with_moderator(moderator_llm)
        .with_max_rounds(1);

        let result = debate.run("AI adoption strategy").await.unwrap();

        assert!(result.consensus.is_some());
        assert!(result
            .consensus
            .unwrap()
            .contains("Both safety and innovation"));
        assert!(result.concluded);
    }

    #[tokio::test]
    async fn test_multi_agent_debate_builder() {
        let alice_llm = Arc::new(MockDebaterLLM::new("Alice", vec![]));
        let bob_llm = Arc::new(MockDebaterLLM::new("Bob", vec![]));
        let moderator_llm = Arc::new(MockModeratorLLM::new("Consensus"));

        let debate = MultiAgentDebate::builder()
            .add_debater("Alice", "Conservative", alice_llm)
            .add_debater("Bob", "Progressive", bob_llm)
            .moderator(moderator_llm)
            .max_rounds(2)
            .verbose(false)
            .build()
            .unwrap();

        assert_eq!(debate.debaters.len(), 2);
        assert!(debate.moderator.is_some());
        assert_eq!(debate.config.max_rounds, 2);
    }

    #[tokio::test]
    async fn test_multi_agent_debate_builder_validation() {
        // Empty debaters should fail
        let result = MultiAgentDebate::builder().build();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_debater_creation() {
        let llm = Arc::new(MockDebaterLLM::new("Test", vec![]));
        let debater = Debater::new("Alice", "Risk-averse perspective", llm);

        assert_eq!(debater.name, "Alice");
        assert_eq!(debater.perspective, "Risk-averse perspective");
    }
}

// ============================================================================
// Config Builder Tests
// ============================================================================

#[cfg(test)]
mod config_builder_tests {
    use super::*;

    // -------------------------------------------------------------------------
    // PlanAndExecuteConfig tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_plan_and_execute_config_default() {
        let config = PlanAndExecuteConfig::new();
        assert_eq!(config.max_iterations, 20);
        assert!(config.enable_replanning);
        assert_eq!(config.max_replans, 3);
        assert!(config.include_execution_history);
        assert!(!config.verbose);
    }

    #[test]
    fn test_plan_and_execute_config_full_builder() {
        let config = PlanAndExecuteConfig::new()
            .with_max_iterations(10)
            .with_enable_replanning(false)
            .with_max_replans(5)
            .with_planner_system_message("Custom planner message")
            .with_executor_system_message("Custom executor message")
            .with_include_execution_history(false)
            .with_verbose(true);

        assert_eq!(config.max_iterations, 10);
        assert!(!config.enable_replanning);
        assert_eq!(config.max_replans, 5);
        assert_eq!(config.planner_system_message, "Custom planner message");
        assert_eq!(config.executor_system_message, "Custom executor message");
        assert!(!config.include_execution_history);
        assert!(config.verbose);
    }

    #[test]
    fn test_plan_and_execute_config_partial_builder() {
        let config = PlanAndExecuteConfig::new()
            .with_max_iterations(30)
            .with_verbose(true);

        assert_eq!(config.max_iterations, 30);
        assert!(config.enable_replanning); // default
        assert_eq!(config.max_replans, 3); // default
        assert!(config.verbose);
    }

    // -------------------------------------------------------------------------
    // ReflectionConfig tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_reflection_config_default() {
        let config = ReflectionConfig::new();
        assert_eq!(config.max_iterations, 5);
        assert!((config.quality_threshold - 0.8).abs() < f64::EPSILON);
        assert!(!config.verbose);
        assert!(config.include_critique_history);
    }

    #[test]
    fn test_reflection_config_full_builder() {
        let config = ReflectionConfig::new()
            .with_max_iterations(10)
            .with_quality_threshold(0.9)
            .with_verbose(true)
            .with_actor_system_message("Custom actor message")
            .with_critic_system_message("Custom critic message")
            .with_include_critique_history(false);

        assert_eq!(config.max_iterations, 10);
        assert!((config.quality_threshold - 0.9).abs() < f64::EPSILON);
        assert!(config.verbose);
        assert_eq!(config.actor_system_message, "Custom actor message");
        assert_eq!(config.critic_system_message, "Custom critic message");
        assert!(!config.include_critique_history);
    }

    #[test]
    fn test_reflection_config_partial_builder() {
        let config = ReflectionConfig::new()
            .with_quality_threshold(0.95)
            .with_max_iterations(3);

        assert_eq!(config.max_iterations, 3);
        assert!((config.quality_threshold - 0.95).abs() < f64::EPSILON);
        assert!(!config.verbose); // default
        assert!(config.include_critique_history); // default
    }

    // -------------------------------------------------------------------------
    // DebateConfig tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_debate_config_default() {
        let config = DebateConfig::new();
        assert_eq!(config.max_rounds, 3);
        assert!(config.debater_system_message.is_some());
        assert!(config.moderator_system_message.is_some());
        assert!(config.include_debate_history);
        assert!(!config.verbose);
    }

    #[test]
    fn test_debate_config_full_builder() {
        let config = DebateConfig::new()
            .with_max_rounds(5)
            .with_debater_system_message("Custom debater prompt")
            .with_moderator_system_message("Custom moderator prompt")
            .with_include_debate_history(false)
            .with_verbose(true);

        assert_eq!(config.max_rounds, 5);
        assert_eq!(
            config.debater_system_message.as_deref(),
            Some("Custom debater prompt")
        );
        assert_eq!(
            config.moderator_system_message.as_deref(),
            Some("Custom moderator prompt")
        );
        assert!(!config.include_debate_history);
        assert!(config.verbose);
    }

    #[test]
    fn test_debate_config_partial_builder() {
        let config = DebateConfig::new()
            .with_max_rounds(7)
            .with_verbose(true);

        assert_eq!(config.max_rounds, 7);
        assert!(config.debater_system_message.is_some()); // default
        assert!(config.include_debate_history); // default
        assert!(config.verbose);
    }
}
