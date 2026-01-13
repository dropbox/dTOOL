//! Core agent types for decision-making and execution tracking.
//!
//! This module defines the fundamental types used by all agent implementations:
//!
//! - [`AgentDecision`]: The result of agent planning - either take an action or finish
//! - [`AgentAction`]: A decision to call a specific tool with given input
//! - [`AgentFinish`]: A decision to return a final answer to the user
//! - [`AgentStep`]: A record of an action taken and its observation
//!
//! These types form the contract between agents, executors, and middleware,
//! enabling consistent behavior across different agent implementations.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::core::agents::{AgentDecision, AgentAction, AgentFinish};
//!
//! // Agent decides to use a tool
//! let action = AgentDecision::Action(AgentAction {
//!     tool: "search".to_string(),
//!     tool_input: "weather forecast".into(),
//!     log: "I need to search for weather information.".to_string(),
//! });
//!
//! // Agent decides to finish with an answer
//! let finish = AgentDecision::Finish(AgentFinish {
//!     output: "The weather will be sunny tomorrow.".to_string(),
//!     log: "I have enough information to answer.".to_string(),
//! });
//! ```

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::core::tools::ToolInput;

/// Decision made by an agent - either take an action or finish
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum AgentDecision {
    /// Agent decided to use a tool
    Action(AgentAction),
    /// Agent decided to return final answer
    Finish(AgentFinish),
}

impl AgentDecision {
    /// Check if this is an action decision
    #[must_use]
    pub const fn is_action(&self) -> bool {
        matches!(self, AgentDecision::Action(_))
    }

    /// Check if this is a finish decision
    #[must_use]
    pub const fn is_finish(&self) -> bool {
        matches!(self, AgentDecision::Finish(_))
    }

    /// Get the action, if this is an action decision
    #[must_use]
    pub const fn as_action(&self) -> Option<&AgentAction> {
        match self {
            AgentDecision::Action(action) => Some(action),
            _ => None,
        }
    }

    /// Get the finish result, if this is a finish decision
    #[must_use]
    pub const fn as_finish(&self) -> Option<&AgentFinish> {
        match self {
            AgentDecision::Finish(finish) => Some(finish),
            _ => None,
        }
    }
}

/// Action to take - represents a decision to use a tool
///
/// When an agent decides it needs to use a tool, it returns an `AgentAction`
/// containing the tool name, input, and reasoning.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::agents::AgentAction;
/// use dashflow::core::tools::ToolInput;
///
/// let action = AgentAction {
///     tool: "calculator".to_string(),
///     tool_input: ToolInput::from("25 * 4"),
///     log: "I need to calculate 25 times 4".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAction {
    /// Name of the tool to use
    pub tool: String,

    /// Input to pass to the tool
    pub tool_input: ToolInput,

    /// Agent's reasoning / thought process for this action
    ///
    /// This log is useful for debugging and understanding the agent's
    /// decision-making process.
    pub log: String,
}

impl AgentAction {
    /// Create a new agent action
    pub fn new(tool: impl Into<String>, tool_input: ToolInput, log: impl Into<String>) -> Self {
        Self {
            tool: tool.into(),
            tool_input,
            log: log.into(),
        }
    }
}

impl fmt::Display for AgentAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AgentAction(tool='{}', input={:?}, log='{}')",
            self.tool, self.tool_input, self.log
        )
    }
}

/// Final answer from the agent
///
/// When an agent determines it has enough information to answer the original
/// question, it returns an `AgentFinish` with the output and reasoning.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow::core::agents::AgentFinish;
///
/// let finish = AgentFinish {
///     output: "The answer is 100".to_string(),
///     log: "I have calculated the result successfully".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentFinish {
    /// Final output/answer
    pub output: String,

    /// Agent's reasoning for finishing
    pub log: String,
}

impl AgentFinish {
    /// Create a new agent finish result
    pub fn new(output: impl Into<String>, log: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            log: log.into(),
        }
    }
}

impl fmt::Display for AgentFinish {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AgentFinish(output='{}', log='{}')",
            self.output, self.log
        )
    }
}

/// Record of a single step in agent execution
///
/// Each step consists of an action taken by the agent and the observation
/// (tool output) received from executing that action.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow::core::agents::{AgentAction, AgentStep};
/// use dashflow::core::tools::ToolInput;
///
/// let action = AgentAction::new(
///     "calculator",
///     ToolInput::from("2 + 2"),
///     "Need to add numbers",
/// );
///
/// let step = AgentStep {
///     action,
///     observation: "4".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    /// Action taken by the agent
    pub action: AgentAction,

    /// Observation (tool output) from executing the action
    pub observation: String,
}

impl AgentStep {
    /// Create a new agent step
    pub fn new(action: AgentAction, observation: impl Into<String>) -> Self {
        Self {
            action,
            observation: observation.into(),
        }
    }
}

impl fmt::Display for AgentStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AgentStep(action={}, observation='{}')",
            self.action, self.observation
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    // ============================================================================
    // AgentAction Tests
    // ============================================================================

    #[test]
    fn test_agent_action_new() {
        let action = AgentAction::new(
            "calculator",
            ToolInput::from("2 + 2"),
            "Need to calculate",
        );
        assert_eq!(action.tool, "calculator");
        assert_eq!(action.log, "Need to calculate");
    }

    #[test]
    fn test_agent_action_new_with_string_types() {
        let action = AgentAction::new(
            String::from("search"),
            ToolInput::from("rust async"),
            String::from("Searching for information"),
        );
        assert_eq!(action.tool, "search");
        assert_eq!(action.log, "Searching for information");
    }

    #[test]
    fn test_agent_action_display() {
        let action = AgentAction::new("calculator", ToolInput::from("5 * 5"), "Multiplying");
        let display = format!("{}", action);
        assert!(display.contains("AgentAction"));
        assert!(display.contains("calculator"));
        assert!(display.contains("Multiplying"));
    }

    #[test]
    fn test_agent_action_clone() {
        let action = AgentAction::new("tool1", ToolInput::from("input"), "log");
        let cloned = action.clone();
        assert_eq!(action.tool, cloned.tool);
        assert_eq!(action.log, cloned.log);
    }

    #[test]
    fn test_agent_action_debug() {
        let action = AgentAction::new("debug_tool", ToolInput::from("test"), "testing");
        let debug = format!("{:?}", action);
        assert!(debug.contains("AgentAction"));
        assert!(debug.contains("debug_tool"));
    }

    #[test]
    fn test_agent_action_serialization() {
        let action = AgentAction::new("serialize_tool", ToolInput::from("data"), "serialize test");
        let json = serde_json::to_string(&action).expect("Failed to serialize AgentAction");
        assert!(json.contains("serialize_tool"));
        assert!(json.contains("serialize test"));
    }

    #[test]
    fn test_agent_action_deserialization() {
        let json = r#"{"tool":"test_tool","tool_input":"input_value","log":"test log"}"#;
        let action: AgentAction =
            serde_json::from_str(json).expect("Failed to deserialize AgentAction");
        assert_eq!(action.tool, "test_tool");
        assert_eq!(action.log, "test log");
    }

    #[test]
    fn test_agent_action_roundtrip_serialization() {
        let original = AgentAction::new("roundtrip", ToolInput::from("test input"), "roundtrip log");
        let json = serde_json::to_string(&original).expect("Failed to serialize");
        let deserialized: AgentAction = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(original.tool, deserialized.tool);
        assert_eq!(original.log, deserialized.log);
    }

    #[test]
    fn test_agent_action_empty_strings() {
        let action = AgentAction::new("", ToolInput::from(""), "");
        assert_eq!(action.tool, "");
        assert_eq!(action.log, "");
    }

    #[test]
    fn test_agent_action_unicode() {
        let action = AgentAction::new(
            "工具",
            ToolInput::from("输入数据"),
            "使用中文日志",
        );
        assert_eq!(action.tool, "工具");
        assert_eq!(action.log, "使用中文日志");
    }

    #[test]
    fn test_agent_action_special_characters() {
        let action = AgentAction::new(
            "tool-with-dashes_and_underscores",
            ToolInput::from("input with\nnewlines\tand\ttabs"),
            "log with 'quotes' and \"double quotes\"",
        );
        assert_eq!(action.tool, "tool-with-dashes_and_underscores");
        assert!(action.log.contains("quotes"));
    }

    // ============================================================================
    // AgentFinish Tests
    // ============================================================================

    #[test]
    fn test_agent_finish_new() {
        let finish = AgentFinish::new("The answer is 42", "Calculation complete");
        assert_eq!(finish.output, "The answer is 42");
        assert_eq!(finish.log, "Calculation complete");
    }

    #[test]
    fn test_agent_finish_new_with_string_types() {
        let finish = AgentFinish::new(
            String::from("Result"),
            String::from("Finished successfully"),
        );
        assert_eq!(finish.output, "Result");
        assert_eq!(finish.log, "Finished successfully");
    }

    #[test]
    fn test_agent_finish_display() {
        let finish = AgentFinish::new("output text", "log message");
        let display = format!("{}", finish);
        assert!(display.contains("AgentFinish"));
        assert!(display.contains("output text"));
        assert!(display.contains("log message"));
    }

    #[test]
    fn test_agent_finish_clone() {
        let finish = AgentFinish::new("output", "log");
        let cloned = finish.clone();
        assert_eq!(finish.output, cloned.output);
        assert_eq!(finish.log, cloned.log);
    }

    #[test]
    fn test_agent_finish_debug() {
        let finish = AgentFinish::new("debug output", "debug log");
        let debug = format!("{:?}", finish);
        assert!(debug.contains("AgentFinish"));
        assert!(debug.contains("debug output"));
    }

    #[test]
    fn test_agent_finish_serialization() {
        let finish = AgentFinish::new("serialize output", "serialize log");
        let json = serde_json::to_string(&finish).expect("Failed to serialize AgentFinish");
        assert!(json.contains("serialize output"));
        assert!(json.contains("serialize log"));
    }

    #[test]
    fn test_agent_finish_deserialization() {
        let json = r#"{"output":"test output","log":"test log"}"#;
        let finish: AgentFinish =
            serde_json::from_str(json).expect("Failed to deserialize AgentFinish");
        assert_eq!(finish.output, "test output");
        assert_eq!(finish.log, "test log");
    }

    #[test]
    fn test_agent_finish_roundtrip_serialization() {
        let original = AgentFinish::new("roundtrip output", "roundtrip log");
        let json = serde_json::to_string(&original).expect("Failed to serialize");
        let deserialized: AgentFinish = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(original.output, deserialized.output);
        assert_eq!(original.log, deserialized.log);
    }

    #[test]
    fn test_agent_finish_empty_strings() {
        let finish = AgentFinish::new("", "");
        assert_eq!(finish.output, "");
        assert_eq!(finish.log, "");
    }

    #[test]
    fn test_agent_finish_multiline() {
        let finish = AgentFinish::new(
            "Line 1\nLine 2\nLine 3",
            "Multiple\nlines\nof\nlog",
        );
        assert!(finish.output.contains('\n'));
        assert!(finish.log.contains('\n'));
    }

    // ============================================================================
    // AgentDecision Tests
    // ============================================================================

    #[test]
    fn test_agent_decision_action() {
        let action = AgentAction::new("tool", ToolInput::from("input"), "log");
        let decision = AgentDecision::Action(action);
        assert!(decision.is_action());
        assert!(!decision.is_finish());
    }

    #[test]
    fn test_agent_decision_finish() {
        let finish = AgentFinish::new("output", "log");
        let decision = AgentDecision::Finish(finish);
        assert!(decision.is_finish());
        assert!(!decision.is_action());
    }

    #[test]
    fn test_agent_decision_as_action() {
        let action = AgentAction::new("my_tool", ToolInput::from("data"), "reasoning");
        let decision = AgentDecision::Action(action);
        let extracted = decision.as_action();
        assert!(extracted.is_some());
        assert_eq!(extracted.unwrap().tool, "my_tool");
    }

    #[test]
    fn test_agent_decision_as_action_on_finish() {
        let finish = AgentFinish::new("output", "log");
        let decision = AgentDecision::Finish(finish);
        assert!(decision.as_action().is_none());
    }

    #[test]
    fn test_agent_decision_as_finish() {
        let finish = AgentFinish::new("final_output", "done");
        let decision = AgentDecision::Finish(finish);
        let extracted = decision.as_finish();
        assert!(extracted.is_some());
        assert_eq!(extracted.unwrap().output, "final_output");
    }

    #[test]
    fn test_agent_decision_as_finish_on_action() {
        let action = AgentAction::new("tool", ToolInput::from("input"), "log");
        let decision = AgentDecision::Action(action);
        assert!(decision.as_finish().is_none());
    }

    #[test]
    fn test_agent_decision_clone() {
        let action = AgentAction::new("tool", ToolInput::from("input"), "log");
        let decision = AgentDecision::Action(action);
        let cloned = decision.clone();
        assert!(cloned.is_action());
        assert_eq!(
            cloned.as_action().unwrap().tool,
            decision.as_action().unwrap().tool
        );
    }

    #[test]
    fn test_agent_decision_debug() {
        let finish = AgentFinish::new("out", "log");
        let decision = AgentDecision::Finish(finish);
        let debug = format!("{:?}", decision);
        assert!(debug.contains("Finish"));
    }

    #[test]
    fn test_agent_decision_action_serialization() {
        let action = AgentAction::new("serialize_tool", ToolInput::from("data"), "log");
        let decision = AgentDecision::Action(action);
        let json = serde_json::to_string(&decision).expect("Failed to serialize");
        assert!(json.contains("action"));
        assert!(json.contains("serialize_tool"));
    }

    #[test]
    fn test_agent_decision_finish_serialization() {
        let finish = AgentFinish::new("serialize_output", "serialize_log");
        let decision = AgentDecision::Finish(finish);
        let json = serde_json::to_string(&decision).expect("Failed to serialize");
        assert!(json.contains("finish"));
        assert!(json.contains("serialize_output"));
    }

    #[test]
    fn test_agent_decision_action_deserialization() {
        let json = r#"{"type":"action","tool":"test_tool","tool_input":"test_input","log":"test_log"}"#;
        let decision: AgentDecision =
            serde_json::from_str(json).expect("Failed to deserialize");
        assert!(decision.is_action());
        assert_eq!(decision.as_action().unwrap().tool, "test_tool");
    }

    #[test]
    fn test_agent_decision_finish_deserialization() {
        let json = r#"{"type":"finish","output":"test_output","log":"test_log"}"#;
        let decision: AgentDecision =
            serde_json::from_str(json).expect("Failed to deserialize");
        assert!(decision.is_finish());
        assert_eq!(decision.as_finish().unwrap().output, "test_output");
    }

    // ============================================================================
    // AgentStep Tests
    // ============================================================================

    #[test]
    fn test_agent_step_new() {
        let action = AgentAction::new("calculator", ToolInput::from("2+2"), "calculating");
        let step = AgentStep::new(action, "4");
        assert_eq!(step.action.tool, "calculator");
        assert_eq!(step.observation, "4");
    }

    #[test]
    fn test_agent_step_new_with_string() {
        let action = AgentAction::new("search", ToolInput::from("query"), "searching");
        let step = AgentStep::new(action, String::from("search results here"));
        assert_eq!(step.observation, "search results here");
    }

    #[test]
    fn test_agent_step_display() {
        let action = AgentAction::new("tool", ToolInput::from("input"), "log");
        let step = AgentStep::new(action, "observation text");
        let display = format!("{}", step);
        assert!(display.contains("AgentStep"));
        assert!(display.contains("observation text"));
    }

    #[test]
    fn test_agent_step_clone() {
        let action = AgentAction::new("tool", ToolInput::from("input"), "log");
        let step = AgentStep::new(action, "obs");
        let cloned = step.clone();
        assert_eq!(step.action.tool, cloned.action.tool);
        assert_eq!(step.observation, cloned.observation);
    }

    #[test]
    fn test_agent_step_debug() {
        let action = AgentAction::new("debug_tool", ToolInput::from("input"), "log");
        let step = AgentStep::new(action, "debug_obs");
        let debug = format!("{:?}", step);
        assert!(debug.contains("AgentStep"));
        assert!(debug.contains("debug_tool"));
    }

    #[test]
    fn test_agent_step_serialization() {
        let action = AgentAction::new("ser_tool", ToolInput::from("data"), "log");
        let step = AgentStep::new(action, "ser_obs");
        let json = serde_json::to_string(&step).expect("Failed to serialize AgentStep");
        assert!(json.contains("ser_tool"));
        assert!(json.contains("ser_obs"));
    }

    #[test]
    fn test_agent_step_deserialization() {
        let json = r#"{"action":{"tool":"deser_tool","tool_input":"deser_input","log":"deser_log"},"observation":"deser_obs"}"#;
        let step: AgentStep = serde_json::from_str(json).expect("Failed to deserialize AgentStep");
        assert_eq!(step.action.tool, "deser_tool");
        assert_eq!(step.observation, "deser_obs");
    }

    #[test]
    fn test_agent_step_roundtrip_serialization() {
        let action = AgentAction::new("roundtrip_tool", ToolInput::from("rt_input"), "rt_log");
        let original = AgentStep::new(action, "roundtrip_obs");
        let json = serde_json::to_string(&original).expect("Failed to serialize");
        let deserialized: AgentStep = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(original.action.tool, deserialized.action.tool);
        assert_eq!(original.observation, deserialized.observation);
    }

    #[test]
    fn test_agent_step_empty_observation() {
        let action = AgentAction::new("tool", ToolInput::from("input"), "log");
        let step = AgentStep::new(action, "");
        assert_eq!(step.observation, "");
    }

    #[test]
    fn test_agent_step_long_observation() {
        let action = AgentAction::new("tool", ToolInput::from("input"), "log");
        let long_obs = "x".repeat(10000);
        let step = AgentStep::new(action, long_obs.clone());
        assert_eq!(step.observation.len(), 10000);
    }

    // ============================================================================
    // Integration / Edge Case Tests
    // ============================================================================

    #[test]
    fn test_multiple_steps_workflow() {
        let step1 = AgentStep::new(
            AgentAction::new("search", ToolInput::from("rust"), "searching"),
            "Found Rust documentation",
        );
        let step2 = AgentStep::new(
            AgentAction::new("extract", ToolInput::from("content"), "extracting"),
            "Extracted key points",
        );
        let step3 = AgentStep::new(
            AgentAction::new("summarize", ToolInput::from("points"), "summarizing"),
            "Created summary",
        );

        let steps = [step1, step2, step3];
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].action.tool, "search");
        assert_eq!(steps[1].action.tool, "extract");
        assert_eq!(steps[2].action.tool, "summarize");
    }

    #[test]
    fn test_decision_workflow() {
        // Simulate a workflow: action -> action -> finish
        let decisions: Vec<AgentDecision> = vec![
            AgentDecision::Action(AgentAction::new("tool1", ToolInput::from("i1"), "l1")),
            AgentDecision::Action(AgentAction::new("tool2", ToolInput::from("i2"), "l2")),
            AgentDecision::Finish(AgentFinish::new("final answer", "done")),
        ];

        assert!(decisions[0].is_action());
        assert!(decisions[1].is_action());
        assert!(decisions[2].is_finish());
    }

    #[test]
    fn test_nested_json_serialization() {
        // Test complex nested structure serialization
        let action = AgentAction::new(
            "complex_tool",
            ToolInput::from(r#"{"key": "value", "nested": {"inner": 42}}"#),
            "Complex input test",
        );
        let step = AgentStep::new(action, r#"{"result": "success"}"#);
        let decision = AgentDecision::Action(AgentAction::new(
            "wrapper",
            ToolInput::from("wrapper_input"),
            "wrapping",
        ));

        // All should serialize without error
        let _ = serde_json::to_string(&step).expect("Step serialization failed");
        let _ = serde_json::to_string(&decision).expect("Decision serialization failed");
    }
}
