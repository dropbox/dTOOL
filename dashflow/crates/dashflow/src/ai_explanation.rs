// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # AI Explanation of Decisions
//!
//! This module provides capabilities for AI agents to explain their choices
//! and decisions in natural language, enabling transparency and interpretability.
//!
//! ## Overview
//!
//! AI agents can use this module to:
//! - Explain why they chose a particular execution path
//! - Describe the reasoning behind node transitions
//! - Generate human-readable summaries of execution decisions
//! - Provide context for tool calls and state changes
//!
//! ## Key Concepts
//!
//! - **Decision**: A choice made during execution (path taken, tool called, etc.)
//! - **DecisionReason**: The factors that influenced a decision
//! - **DecisionExplanation**: Natural language description of a decision
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::ai_explanation::{DecisionExplainer, DecisionType};
//!
//! let explainer = DecisionExplainer::new();
//! let explanation = explainer.explain_decision(&trace, "reasoning");
//!
//! println!("{}", explanation.natural_language);
//! // "At node 'reasoning', I chose to proceed to 'tool_call' because
//! //  the state field 'needs_search' was true, indicating the query
//! //  required external information. This matches the pattern of
//! //  research-oriented tasks where tool usage improves accuracy."
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::introspection::{ExecutionTrace, NodeExecution};

/// Type of decision made during execution
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DecisionType {
    /// Chose which node to execute next
    PathSelection {
        /// The node where the decision was made
        from_node: String,
        /// The node that was selected
        to_node: String,
        /// Other nodes that could have been selected
        alternatives: Vec<String>,
    },
    /// Decided to call a tool
    ToolCall {
        /// Node that made the call
        node: String,
        /// Tool that was called
        tool: String,
        /// Why the tool was selected
        reason: Option<String>,
    },
    /// Decided to retry a node
    Retry {
        /// Node that was retried
        node: String,
        /// Attempt number
        attempt: usize,
        /// What triggered the retry
        trigger: String,
    },
    /// Decided to terminate execution
    Termination {
        /// Final node
        node: String,
        /// Whether it was a successful termination
        success: bool,
    },
    /// Decided to enter/exit a loop
    LoopControl {
        /// Node involved in loop
        node: String,
        /// Whether entering or exiting
        action: LoopAction,
        /// Iteration count
        iteration: usize,
    },
    /// State modification decision
    StateChange {
        /// Node that modified state
        node: String,
        /// Fields that were modified
        fields_modified: Vec<String>,
    },
    /// Custom decision type
    Custom {
        /// Description of the decision
        description: String,
    },
}

impl std::fmt::Display for DecisionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecisionType::PathSelection {
                from_node, to_node, ..
            } => {
                write!(f, "path selection: {} → {}", from_node, to_node)
            }
            DecisionType::ToolCall { node, tool, .. } => {
                write!(f, "tool call: {} in {}", tool, node)
            }
            DecisionType::Retry { node, attempt, .. } => {
                write!(f, "retry: {} (attempt {})", node, attempt)
            }
            DecisionType::Termination { node, success } => {
                write!(
                    f,
                    "termination at {} ({})",
                    node,
                    if *success { "success" } else { "failure" }
                )
            }
            DecisionType::LoopControl {
                node,
                action,
                iteration,
            } => {
                write!(f, "loop {}: {} (iteration {})", action, node, iteration)
            }
            DecisionType::StateChange {
                node,
                fields_modified,
            } => {
                write!(
                    f,
                    "state change in {}: {}",
                    node,
                    fields_modified.join(", ")
                )
            }
            DecisionType::Custom { description } => {
                write!(f, "{}", description)
            }
        }
    }
}

/// Loop control action
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LoopAction {
    /// Entering a loop
    Enter,
    /// Continuing a loop iteration
    Continue,
    /// Exiting a loop
    Exit,
}

impl std::fmt::Display for LoopAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoopAction::Enter => write!(f, "enter"),
            LoopAction::Continue => write!(f, "continue"),
            LoopAction::Exit => write!(f, "exit"),
        }
    }
}

/// A reason that influenced a decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionReason {
    /// What factor influenced the decision
    pub factor: String,
    /// How much this factor contributed (0.0-1.0)
    pub weight: f64,
    /// Evidence supporting this reason
    pub evidence: String,
    /// The actual value that was considered
    pub value: Option<serde_json::Value>,
}

impl DecisionReason {
    /// Create a new decision reason
    #[must_use]
    pub fn new(factor: impl Into<String>, weight: f64, evidence: impl Into<String>) -> Self {
        Self {
            factor: factor.into(),
            weight: weight.clamp(0.0, 1.0),
            evidence: evidence.into(),
            value: None,
        }
    }

    /// Add the actual value that was considered
    #[must_use]
    pub fn with_value(mut self, value: serde_json::Value) -> Self {
        self.value = Some(value);
        self
    }

    /// Generate natural language for this reason
    #[must_use]
    pub fn to_natural_language(&self) -> String {
        if let Some(ref value) = self.value {
            format!("{} was {} ({})", self.factor, value, self.evidence)
        } else {
            format!("{}: {}", self.factor, self.evidence)
        }
    }
}

/// A decision with its explanation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    /// The type of decision
    pub decision_type: DecisionType,
    /// When the decision was made (execution index)
    pub execution_index: usize,
    /// Reasons for this decision
    pub reasons: Vec<DecisionReason>,
    /// Confidence in the decision (0.0-1.0)
    pub confidence: f64,
    /// Alternative choices that were considered
    pub alternatives_considered: Vec<String>,
    /// Additional context
    pub context: HashMap<String, serde_json::Value>,
}

impl Decision {
    /// Create a new decision
    #[must_use]
    pub fn new(decision_type: DecisionType, execution_index: usize) -> Self {
        Self {
            decision_type,
            execution_index,
            reasons: Vec::new(),
            confidence: 1.0,
            alternatives_considered: Vec::new(),
            context: HashMap::new(),
        }
    }

    /// Add a reason
    #[must_use]
    pub fn with_reason(mut self, reason: DecisionReason) -> Self {
        self.reasons.push(reason);
        self
    }

    /// Add multiple reasons
    #[must_use]
    pub fn with_reasons(mut self, reasons: impl IntoIterator<Item = DecisionReason>) -> Self {
        self.reasons.extend(reasons);
        self
    }

    /// Set confidence
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Add alternatives that were considered
    #[must_use]
    pub fn with_alternatives(mut self, alternatives: Vec<String>) -> Self {
        self.alternatives_considered = alternatives;
        self
    }

    /// Add context
    #[must_use]
    pub fn with_context(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.context.insert(key.into(), value);
        self
    }

    /// Get the primary reason (highest weight)
    #[must_use]
    pub fn primary_reason(&self) -> Option<&DecisionReason> {
        self.reasons.iter().max_by(|a, b| {
            a.weight
                .partial_cmp(&b.weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

/// Natural language explanation of a decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionExplanation {
    /// The decision being explained
    pub decision: Decision,
    /// Natural language summary
    pub natural_language: String,
    /// Detailed breakdown in natural language
    pub detailed_breakdown: Vec<String>,
    /// Questions this explanation answers
    pub answers_questions: Vec<String>,
    /// Suggested follow-up questions
    pub follow_up_questions: Vec<String>,
}

impl DecisionExplanation {
    /// Create a new explanation
    #[must_use]
    pub fn new(decision: Decision, natural_language: impl Into<String>) -> Self {
        Self {
            decision,
            natural_language: natural_language.into(),
            detailed_breakdown: Vec::new(),
            answers_questions: Vec::new(),
            follow_up_questions: Vec::new(),
        }
    }

    /// Add detailed breakdown points
    #[must_use]
    pub fn with_breakdown(mut self, points: Vec<String>) -> Self {
        self.detailed_breakdown = points;
        self
    }

    /// Add questions this answers
    #[must_use]
    pub fn with_answered_questions(mut self, questions: Vec<String>) -> Self {
        self.answers_questions = questions;
        self
    }

    /// Add follow-up questions
    #[must_use]
    pub fn with_follow_ups(mut self, questions: Vec<String>) -> Self {
        self.follow_up_questions = questions;
        self
    }

    /// Generate a full report
    #[must_use]
    pub fn report(&self) -> String {
        let mut lines = vec![
            format!("Decision: {}", self.decision.decision_type),
            String::new(),
            "Explanation:".to_string(),
            self.natural_language.clone(),
        ];

        if !self.detailed_breakdown.is_empty() {
            lines.push(String::new());
            lines.push("Detailed Breakdown:".to_string());
            for (i, point) in self.detailed_breakdown.iter().enumerate() {
                lines.push(format!("  {}. {}", i + 1, point));
            }
        }

        if !self.decision.alternatives_considered.is_empty() {
            lines.push(String::new());
            lines.push("Alternatives Considered:".to_string());
            for alt in &self.decision.alternatives_considered {
                lines.push(format!("  - {}", alt));
            }
        }

        if !self.follow_up_questions.is_empty() {
            lines.push(String::new());
            lines.push("You might also want to know:".to_string());
            for q in &self.follow_up_questions {
                lines.push(format!("  - {}", q));
            }
        }

        lines.join("\n")
    }
}

/// Configuration for the decision explainer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainerConfig {
    /// Include state values in explanations
    pub include_state_values: bool,
    /// Maximum length of natural language explanations
    pub max_explanation_length: usize,
    /// Include timing information
    pub include_timing: bool,
    /// Include token usage information
    pub include_tokens: bool,
    /// Generate follow-up questions
    pub generate_follow_ups: bool,
    /// Verbosity level (0-3)
    pub verbosity: u8,
}

impl Default for ExplainerConfig {
    fn default() -> Self {
        Self {
            include_state_values: true,
            max_explanation_length: 500,
            include_timing: true,
            include_tokens: true,
            generate_follow_ups: true,
            verbosity: 1,
        }
    }
}

impl ExplainerConfig {
    /// Create a brief configuration
    #[must_use]
    pub fn brief() -> Self {
        Self {
            include_state_values: false,
            max_explanation_length: 150,
            include_timing: false,
            include_tokens: false,
            generate_follow_ups: false,
            verbosity: 0,
        }
    }

    /// Create a detailed configuration
    #[must_use]
    pub fn detailed() -> Self {
        Self {
            include_state_values: true,
            max_explanation_length: 2000,
            include_timing: true,
            include_tokens: true,
            generate_follow_ups: true,
            verbosity: 3,
        }
    }
}

/// Explains AI decisions in natural language
pub struct DecisionExplainer {
    config: ExplainerConfig,
}

impl Default for DecisionExplainer {
    fn default() -> Self {
        Self::new()
    }
}

impl DecisionExplainer {
    /// Create a new explainer with default configuration
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: ExplainerConfig::default(),
        }
    }

    /// Create an explainer with custom configuration
    #[must_use]
    pub fn with_config(config: ExplainerConfig) -> Self {
        Self { config }
    }

    /// Extract all decisions from an execution trace
    #[must_use]
    pub fn extract_decisions(&self, trace: &ExecutionTrace) -> Vec<Decision> {
        let mut decisions = Vec::new();

        // Extract path decisions from node transitions
        for i in 0..trace.nodes_executed.len() {
            let exec = &trace.nodes_executed[i];

            // Path selection decision
            if i < trace.nodes_executed.len() - 1 {
                let next_exec = &trace.nodes_executed[i + 1];
                let decision = self.analyze_path_decision(exec, next_exec, i);
                decisions.push(decision);
            }

            // Tool call decisions
            for tool in &exec.tools_called {
                let decision = self.analyze_tool_decision(exec, tool, i);
                decisions.push(decision);
            }

            // Check for retry (same node appearing consecutively)
            if i > 0 && trace.nodes_executed[i - 1].node == exec.node {
                let attempt = trace.nodes_executed[..=i]
                    .iter()
                    .filter(|e| e.node == exec.node)
                    .count();
                let decision = self.analyze_retry_decision(exec, attempt, i);
                decisions.push(decision);
            }
        }

        // Termination decision
        if let Some(last) = trace.nodes_executed.last() {
            let decision = self.analyze_termination_decision(
                last,
                trace.completed,
                trace.nodes_executed.len() - 1,
            );
            decisions.push(decision);
        }

        // State change decisions
        for (i, exec) in trace.nodes_executed.iter().enumerate() {
            // Skip nodes without before/after state
            let (Some(before), Some(after)) = (&exec.state_before, &exec.state_after) else {
                continue;
            };

            let changes = self.detect_state_changes(before, after);
            if !changes.is_empty() {
                let decision = Decision::new(
                    DecisionType::StateChange {
                        node: exec.node.clone(),
                        fields_modified: changes,
                    },
                    i,
                );
                decisions.push(decision);
            }
        }

        decisions
    }

    /// Explain a specific decision about a node
    #[must_use]
    pub fn explain_decision(&self, trace: &ExecutionTrace, node: &str) -> DecisionExplanation {
        // Find the node execution
        let exec = trace.nodes_executed.iter().find(|e| e.node == node);

        match exec {
            Some(exec) => self.explain_node_execution(exec, trace),
            None => DecisionExplanation::new(
                Decision::new(
                    DecisionType::Custom {
                        description: format!("Node '{}' not found", node),
                    },
                    0,
                ),
                format!("Node '{}' was not executed in this trace.", node),
            ),
        }
    }

    /// Explain all decisions in a trace
    #[must_use]
    pub fn explain_all(&self, trace: &ExecutionTrace) -> Vec<DecisionExplanation> {
        self.extract_decisions(trace)
            .into_iter()
            .map(|d| self.explain_single_decision(&d, trace))
            .collect()
    }

    /// Generate a summary explanation of the entire execution
    #[must_use]
    pub fn explain_execution(&self, trace: &ExecutionTrace) -> String {
        let mut parts = Vec::new();

        // Opening summary
        let status = if trace.completed {
            "successfully"
        } else {
            "with errors"
        };
        parts.push(format!(
            "The execution completed {} after visiting {} node(s).",
            status,
            trace.nodes_executed.len()
        ));

        // Path taken
        let path: Vec<_> = trace
            .nodes_executed
            .iter()
            .map(|e| e.node.as_str())
            .collect();
        parts.push(format!("Path taken: {}", path.join(" → ")));

        // Key decisions
        let decisions = self.extract_decisions(trace);
        let path_decisions: Vec<_> = decisions
            .iter()
            .filter(|d| matches!(d.decision_type, DecisionType::PathSelection { .. }))
            .collect();

        if !path_decisions.is_empty() {
            parts.push(format!(
                "\n{} path decision(s) were made:",
                path_decisions.len()
            ));
            for (i, decision) in path_decisions.iter().take(3).enumerate() {
                if let Some(reason) = decision.primary_reason() {
                    parts.push(format!("  {}. {}", i + 1, reason.to_natural_language()));
                }
            }
        }

        // Tool usage
        let tool_decisions: Vec<_> = decisions
            .iter()
            .filter(|d| matches!(d.decision_type, DecisionType::ToolCall { .. }))
            .collect();

        if !tool_decisions.is_empty() {
            parts.push(format!("\n{} tool(s) were called:", tool_decisions.len()));
            for decision in tool_decisions.iter().take(5) {
                if let DecisionType::ToolCall { tool, node, .. } = &decision.decision_type {
                    parts.push(format!("  - {} (in {})", tool, node));
                }
            }
        }

        // Performance summary
        if self.config.include_timing {
            parts.push(format!(
                "\nTotal execution time: {}ms",
                trace.total_duration_ms
            ));
        }
        if self.config.include_tokens && trace.total_tokens > 0 {
            parts.push(format!("Total tokens used: {}", trace.total_tokens));
        }

        // Errors
        if !trace.errors.is_empty() {
            parts.push(format!("\n{} error(s) occurred:", trace.errors.len()));
            for error in trace.errors.iter().take(3) {
                parts.push(format!("  - {}: {}", error.node, error.message));
            }
        }

        parts.join("\n")
    }

    /// Explain why a specific path was taken
    #[must_use]
    pub fn explain_path(
        &self,
        trace: &ExecutionTrace,
        from_node: &str,
        to_node: &str,
    ) -> DecisionExplanation {
        // Find the transition
        for i in 0..trace.nodes_executed.len().saturating_sub(1) {
            let from = &trace.nodes_executed[i];
            let to = &trace.nodes_executed[i + 1];

            if from.node == from_node && to.node == to_node {
                return self.explain_transition(from, to, trace, i);
            }
        }

        DecisionExplanation::new(
            Decision::new(
                DecisionType::Custom {
                    description: format!("Transition {} → {} not found", from_node, to_node),
                },
                0,
            ),
            format!(
                "The transition from '{}' to '{}' was not found in this trace.",
                from_node, to_node
            ),
        )
    }

    /// Explain why a tool was called
    #[must_use]
    pub fn explain_tool_call(
        &self,
        trace: &ExecutionTrace,
        tool_name: &str,
    ) -> DecisionExplanation {
        for (i, exec) in trace.nodes_executed.iter().enumerate() {
            if exec.tools_called.contains(&tool_name.to_string()) {
                let decision = self.analyze_tool_decision(exec, tool_name, i);
                return self.explain_single_decision(&decision, trace);
            }
        }

        DecisionExplanation::new(
            Decision::new(
                DecisionType::Custom {
                    description: format!("Tool '{}' not found", tool_name),
                },
                0,
            ),
            format!("Tool '{}' was not called in this execution.", tool_name),
        )
    }

    fn analyze_path_decision(
        &self,
        from: &NodeExecution,
        to: &NodeExecution,
        index: usize,
    ) -> Decision {
        let mut reasons = Vec::new();

        // Analyze state changes that might have influenced the decision
        if let Some(ref state) = from.state_after {
            if let Some(obj) = state.as_object() {
                // Look for common routing fields
                for (key, value) in obj {
                    if key.contains("next") || key.contains("route") || key.contains("path") {
                        reasons.push(
                            DecisionReason::new(
                                format!("state.{}", key),
                                0.9,
                                format!("Routing field '{}' directed to next node", key),
                            )
                            .with_value(value.clone()),
                        );
                    }
                }
            }
        }

        // If no specific routing field found, add generic reason
        if reasons.is_empty() {
            reasons.push(DecisionReason::new(
                "graph structure",
                0.7,
                format!("Edge from '{}' to '{}' was followed", from.node, to.node),
            ));
        }

        // Success/failure influence
        if !from.success {
            reasons.push(DecisionReason::new(
                "error handling",
                0.8,
                "Previous node failed, following error path",
            ));
        }

        Decision::new(
            DecisionType::PathSelection {
                from_node: from.node.clone(),
                to_node: to.node.clone(),
                alternatives: Vec::new(), // Would need graph info to populate
            },
            index,
        )
        .with_reasons(reasons)
    }

    fn analyze_tool_decision(&self, exec: &NodeExecution, tool: &str, index: usize) -> Decision {
        let reasons = vec![DecisionReason::new(
            "task requirement",
            0.8,
            format!("Tool '{}' was needed to complete the task", tool),
        )];

        Decision::new(
            DecisionType::ToolCall {
                node: exec.node.clone(),
                tool: tool.to_string(),
                reason: Some("Task required external capabilities".to_string()),
            },
            index,
        )
        .with_reasons(reasons)
    }

    fn analyze_retry_decision(
        &self,
        exec: &NodeExecution,
        attempt: usize,
        index: usize,
    ) -> Decision {
        let trigger = exec
            .error_message
            .clone()
            .unwrap_or_else(|| "previous attempt did not succeed".to_string());

        let reasons = vec![DecisionReason::new(
            "retry policy",
            0.9,
            format!("Attempt {} after: {}", attempt, trigger),
        )];

        Decision::new(
            DecisionType::Retry {
                node: exec.node.clone(),
                attempt,
                trigger,
            },
            index,
        )
        .with_reasons(reasons)
    }

    fn analyze_termination_decision(
        &self,
        exec: &NodeExecution,
        success: bool,
        index: usize,
    ) -> Decision {
        let reasons = if success {
            vec![DecisionReason::new(
                "completion",
                1.0,
                "All required nodes executed successfully",
            )]
        } else {
            vec![DecisionReason::new(
                "failure",
                1.0,
                exec.error_message
                    .clone()
                    .unwrap_or_else(|| "Execution could not continue".to_string()),
            )]
        };

        Decision::new(
            DecisionType::Termination {
                node: exec.node.clone(),
                success,
            },
            index,
        )
        .with_reasons(reasons)
    }

    fn detect_state_changes(
        &self,
        before: &serde_json::Value,
        after: &serde_json::Value,
    ) -> Vec<String> {
        let mut changes = Vec::new();

        if let (Some(before_obj), Some(after_obj)) = (before.as_object(), after.as_object()) {
            for (key, after_val) in after_obj {
                match before_obj.get(key) {
                    Some(before_val) if before_val != after_val => {
                        changes.push(key.clone());
                    }
                    None => {
                        changes.push(format!("{}(new)", key));
                    }
                    _ => {}
                }
            }
        }

        changes
    }

    fn explain_node_execution(
        &self,
        exec: &NodeExecution,
        trace: &ExecutionTrace,
    ) -> DecisionExplanation {
        let mut parts = Vec::new();

        // Basic execution info
        parts.push(format!(
            "Node '{}' was executed as step {} in the workflow.",
            exec.node,
            exec.index + 1
        ));

        // Performance info
        if self.config.include_timing {
            parts.push(format!("It took {}ms to complete.", exec.duration_ms));
        }

        // Token usage
        if self.config.include_tokens && exec.tokens_used > 0 {
            parts.push(format!("Used {} tokens.", exec.tokens_used));
        }

        // Tool calls
        if !exec.tools_called.is_empty() {
            parts.push(format!(
                "Called {} tool(s): {}",
                exec.tools_called.len(),
                exec.tools_called.join(", ")
            ));
        }

        // Success/failure
        if !exec.success {
            if let Some(ref msg) = exec.error_message {
                parts.push(format!("Execution failed: {}", msg));
            }
        }

        // What came before
        if exec.index > 0 {
            if let Some(prev) = trace.nodes_executed.get(exec.index - 1) {
                parts.push(format!("This followed '{}' in the execution.", prev.node));
            }
        }

        // What comes after
        if let Some(next) = trace.nodes_executed.get(exec.index + 1) {
            parts.push(format!("Afterwards, '{}' was executed.", next.node));
        }

        let natural_language = parts.join(" ");

        let decision = Decision::new(
            DecisionType::Custom {
                description: format!("Execution of '{}'", exec.node),
            },
            exec.index,
        );

        let mut explanation = DecisionExplanation::new(decision, natural_language);

        // Add follow-up questions
        if self.config.generate_follow_ups {
            let mut follow_ups = Vec::new();
            if !exec.tools_called.is_empty() {
                follow_ups.push(format!("Why was '{}' tool called?", exec.tools_called[0]));
            }
            if exec.index < trace.nodes_executed.len() - 1 {
                let next = &trace.nodes_executed[exec.index + 1];
                follow_ups.push(format!("Why did execution go to '{}' next?", next.node));
            }
            explanation = explanation.with_follow_ups(follow_ups);
        }

        explanation
    }

    fn explain_transition(
        &self,
        from: &NodeExecution,
        to: &NodeExecution,
        _trace: &ExecutionTrace,
        index: usize,
    ) -> DecisionExplanation {
        let decision = self.analyze_path_decision(from, to, index);

        let mut natural_parts = Vec::new();
        natural_parts.push(format!(
            "After completing '{}', the execution proceeded to '{}'.",
            from.node, to.node
        ));

        // Explain reasons
        for reason in &decision.reasons {
            natural_parts.push(reason.to_natural_language());
        }

        let natural_language = natural_parts.join(" ");

        let mut breakdown = Vec::new();
        breakdown.push(format!("Source node: {}", from.node));
        breakdown.push(format!("Target node: {}", to.node));
        if self.config.include_timing {
            breakdown.push(format!("Source duration: {}ms", from.duration_ms));
        }
        if from.success {
            breakdown.push("Source completed successfully".to_string());
        } else {
            breakdown.push("Source failed (may have influenced routing)".to_string());
        }

        DecisionExplanation::new(decision, natural_language)
            .with_breakdown(breakdown)
            .with_answered_questions(vec![format!(
                "Why did execution go from '{}' to '{}'?",
                from.node, to.node
            )])
    }

    fn explain_single_decision(
        &self,
        decision: &Decision,
        _trace: &ExecutionTrace,
    ) -> DecisionExplanation {
        let natural_language = match &decision.decision_type {
            DecisionType::PathSelection {
                from_node, to_node, ..
            } => {
                let reason_text = decision
                    .primary_reason()
                    .map(|r| r.to_natural_language())
                    .unwrap_or_else(|| "following the graph structure".to_string());
                format!(
                    "At '{}', I chose to proceed to '{}' because {}.",
                    from_node, to_node, reason_text
                )
            }
            DecisionType::ToolCall { node, tool, reason } => {
                let reason_text = reason.as_deref().unwrap_or("it was required for the task");
                format!(
                    "In '{}', I called '{}' because {}.",
                    node, tool, reason_text
                )
            }
            DecisionType::Retry {
                node,
                attempt,
                trigger,
            } => {
                format!(
                    "I retried '{}' (attempt {}) because {}.",
                    node, attempt, trigger
                )
            }
            DecisionType::Termination { node, success } => {
                if *success {
                    format!(
                        "I completed execution at '{}' after achieving the goal.",
                        node
                    )
                } else {
                    format!("Execution stopped at '{}' due to an error.", node)
                }
            }
            DecisionType::LoopControl {
                node,
                action,
                iteration,
            } => {
                format!(
                    "I {}ed the loop at '{}' (iteration {}).",
                    action, node, iteration
                )
            }
            DecisionType::StateChange {
                node,
                fields_modified,
            } => {
                format!("In '{}', I modified: {}.", node, fields_modified.join(", "))
            }
            DecisionType::Custom { description } => description.clone(),
        };

        let breakdown: Vec<String> = decision
            .reasons
            .iter()
            .map(|r| {
                format!(
                    "{} ({:.0}% weight): {}",
                    r.factor,
                    r.weight * 100.0,
                    r.evidence
                )
            })
            .collect();

        DecisionExplanation::new(decision.clone(), natural_language)
            .with_breakdown(breakdown)
            .with_follow_ups(if self.config.generate_follow_ups {
                vec!["What alternatives were considered?".to_string()]
            } else {
                Vec::new()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_prelude::{
        create_error_test_trace, create_standard_test_trace, ExecutionTraceBuilder, NodeExecution,
    };

    #[test]
    fn test_decision_type_display() {
        let path = DecisionType::PathSelection {
            from_node: "a".to_string(),
            to_node: "b".to_string(),
            alternatives: vec![],
        };
        assert!(path.to_string().contains("a → b"));

        let tool = DecisionType::ToolCall {
            node: "test".to_string(),
            tool: "search".to_string(),
            reason: None,
        };
        assert!(tool.to_string().contains("search"));
    }

    #[test]
    fn test_loop_action_display() {
        assert_eq!(LoopAction::Enter.to_string(), "enter");
        assert_eq!(LoopAction::Continue.to_string(), "continue");
        assert_eq!(LoopAction::Exit.to_string(), "exit");
    }

    #[test]
    fn test_decision_reason_creation() {
        let reason = DecisionReason::new("test_factor", 0.75, "test evidence")
            .with_value(serde_json::json!(true));

        assert_eq!(reason.factor, "test_factor");
        assert_eq!(reason.weight, 0.75);
        assert!(reason.value.is_some());
    }

    #[test]
    fn test_decision_reason_weight_clamping() {
        let high = DecisionReason::new("test", 1.5, "");
        assert_eq!(high.weight, 1.0);

        let low = DecisionReason::new("test", -0.5, "");
        assert_eq!(low.weight, 0.0);
    }

    #[test]
    fn test_decision_reason_natural_language() {
        let reason =
            DecisionReason::new("field", 0.5, "was checked").with_value(serde_json::json!(true));

        let nl = reason.to_natural_language();
        assert!(nl.contains("field"));
        assert!(nl.contains("true"));
    }

    #[test]
    fn test_decision_creation() {
        let decision = Decision::new(
            DecisionType::Termination {
                node: "end".to_string(),
                success: true,
            },
            5,
        )
        .with_confidence(0.9)
        .with_alternatives(vec!["retry".to_string()]);

        assert_eq!(decision.execution_index, 5);
        assert_eq!(decision.confidence, 0.9);
        assert_eq!(decision.alternatives_considered.len(), 1);
    }

    #[test]
    fn test_decision_primary_reason() {
        let decision = Decision::new(
            DecisionType::Custom {
                description: "test".to_string(),
            },
            0,
        )
        .with_reason(DecisionReason::new("low", 0.3, ""))
        .with_reason(DecisionReason::new("high", 0.8, ""));

        let primary = decision.primary_reason().unwrap();
        assert_eq!(primary.factor, "high");
    }

    #[test]
    fn test_explanation_creation() {
        let decision = Decision::new(
            DecisionType::Custom {
                description: "test".to_string(),
            },
            0,
        );
        let explanation = DecisionExplanation::new(decision, "Test explanation")
            .with_breakdown(vec!["Point 1".to_string()])
            .with_follow_ups(vec!["Question 1".to_string()]);

        assert_eq!(explanation.natural_language, "Test explanation");
        assert_eq!(explanation.detailed_breakdown.len(), 1);
        assert_eq!(explanation.follow_up_questions.len(), 1);
    }

    #[test]
    fn test_explanation_report() {
        let decision = Decision::new(
            DecisionType::PathSelection {
                from_node: "a".to_string(),
                to_node: "b".to_string(),
                alternatives: vec!["c".to_string()],
            },
            0,
        )
        .with_alternatives(vec!["c".to_string()]);

        let explanation = DecisionExplanation::new(decision, "Chose b over c")
            .with_breakdown(vec!["Reason 1".to_string()])
            .with_follow_ups(vec!["Why not c?".to_string()]);

        let report = explanation.report();
        assert!(report.contains("path selection"));
        assert!(report.contains("Chose b over c"));
        assert!(report.contains("Alternatives"));
        assert!(report.contains("Why not c?"));
    }

    #[test]
    fn test_config_presets() {
        let brief = ExplainerConfig::brief();
        assert!(!brief.include_state_values);
        assert_eq!(brief.verbosity, 0);

        let detailed = ExplainerConfig::detailed();
        assert!(detailed.include_state_values);
        assert_eq!(detailed.verbosity, 3);
    }

    #[test]
    fn test_explainer_creation() {
        let default_explainer = DecisionExplainer::new();
        assert!(default_explainer.config.include_timing);

        let custom = DecisionExplainer::with_config(ExplainerConfig::brief());
        assert!(!custom.config.include_timing);
    }

    #[test]
    fn test_extract_decisions() {
        let trace = create_standard_test_trace();
        let explainer = DecisionExplainer::new();

        let decisions = explainer.extract_decisions(&trace);

        // Should have path decisions, tool decisions, and termination
        assert!(!decisions.is_empty());

        // Should have path decisions
        let path_count = decisions
            .iter()
            .filter(|d| matches!(d.decision_type, DecisionType::PathSelection { .. }))
            .count();
        assert!(path_count >= 2); // input→reasoning, reasoning→tool_call, tool_call→output

        // Should have tool call decisions
        let tool_count = decisions
            .iter()
            .filter(|d| matches!(d.decision_type, DecisionType::ToolCall { .. }))
            .count();
        assert_eq!(tool_count, 2); // search and calculate

        // Should have termination
        let term_count = decisions
            .iter()
            .filter(|d| matches!(d.decision_type, DecisionType::Termination { .. }))
            .count();
        assert_eq!(term_count, 1);
    }

    #[test]
    fn test_explain_decision() {
        let trace = create_standard_test_trace();
        let explainer = DecisionExplainer::new();

        let explanation = explainer.explain_decision(&trace, "reasoning");

        assert!(explanation.natural_language.contains("reasoning"));
        assert!(explanation.natural_language.contains("step"));
    }

    #[test]
    fn test_explain_decision_not_found() {
        let trace = create_standard_test_trace();
        let explainer = DecisionExplainer::new();

        let explanation = explainer.explain_decision(&trace, "nonexistent");

        assert!(explanation.natural_language.contains("not executed"));
    }

    #[test]
    fn test_explain_all() {
        let trace = create_standard_test_trace();
        let explainer = DecisionExplainer::new();

        let explanations = explainer.explain_all(&trace);

        assert!(!explanations.is_empty());
        for exp in &explanations {
            assert!(!exp.natural_language.is_empty());
        }
    }

    #[test]
    fn test_explain_execution() {
        let trace = create_standard_test_trace();
        let explainer = DecisionExplainer::new();

        let summary = explainer.explain_execution(&trace);

        assert!(summary.contains("successfully"));
        assert!(summary.contains("4 node(s)")); // input, reasoning, tool_call, output
        assert!(summary.contains("input → reasoning"));
        assert!(summary.contains("tool(s) were called"));
        assert!(summary.contains("1800ms"));
    }

    #[test]
    fn test_explain_execution_with_errors() {
        let trace = create_error_test_trace();
        let explainer = DecisionExplainer::new();

        let summary = explainer.explain_execution(&trace);

        assert!(summary.contains("with errors"));
        assert!(summary.contains("error(s) occurred"));
        assert!(summary.contains("Connection timeout"));
    }

    #[test]
    fn test_explain_path() {
        let trace = create_standard_test_trace();
        let explainer = DecisionExplainer::new();

        let explanation = explainer.explain_path(&trace, "reasoning", "tool_call");

        assert!(explanation.natural_language.contains("reasoning"));
        assert!(explanation.natural_language.contains("tool_call"));
    }

    #[test]
    fn test_explain_path_not_found() {
        let trace = create_standard_test_trace();
        let explainer = DecisionExplainer::new();

        let explanation = explainer.explain_path(&trace, "reasoning", "nonexistent");

        assert!(explanation.natural_language.contains("not found"));
    }

    #[test]
    fn test_explain_tool_call() {
        let trace = create_standard_test_trace();
        let explainer = DecisionExplainer::new();

        let explanation = explainer.explain_tool_call(&trace, "search");

        assert!(explanation.natural_language.contains("search"));
        assert!(explanation.natural_language.contains("tool_call"));
    }

    #[test]
    fn test_explain_tool_call_not_found() {
        let trace = create_standard_test_trace();
        let explainer = DecisionExplainer::new();

        let explanation = explainer.explain_tool_call(&trace, "nonexistent");

        assert!(explanation.natural_language.contains("not called"));
    }

    #[test]
    fn test_detect_state_changes() {
        let explainer = DecisionExplainer::new();

        let before = serde_json::json!({
            "count": 1,
            "status": "running"
        });
        let after = serde_json::json!({
            "count": 2,
            "status": "running",
            "result": "done"
        });

        let changes = explainer.detect_state_changes(&before, &after);

        assert!(changes.contains(&"count".to_string()));
        assert!(changes.iter().any(|c| c.contains("result")));
        assert!(!changes.contains(&"status".to_string())); // unchanged
    }

    #[test]
    fn test_brief_config_explanations() {
        let trace = create_standard_test_trace();
        let explainer = DecisionExplainer::with_config(ExplainerConfig::brief());

        let summary = explainer.explain_execution(&trace);

        // Brief config should not include timing
        assert!(!summary.contains("1800ms"));
    }

    #[test]
    fn test_retry_detection() {
        // Create a trace with retries
        let trace = ExecutionTraceBuilder::new()
            .add_node_execution(NodeExecution::new("retry_node", 100).with_error("Failed"))
            .add_node_execution(NodeExecution::new("retry_node", 150))
            .add_node_execution(NodeExecution::new("retry_node", 200))
            .total_duration_ms(450)
            .completed(true)
            .build();

        let explainer = DecisionExplainer::new();
        let decisions = explainer.extract_decisions(&trace);

        let retry_count = decisions
            .iter()
            .filter(|d| matches!(d.decision_type, DecisionType::Retry { .. }))
            .count();

        assert!(retry_count >= 1);
    }

    #[test]
    fn test_state_change_detection() {
        let trace = ExecutionTraceBuilder::new()
            .add_node_execution(
                NodeExecution::new("modifier", 100)
                    .with_state_before(serde_json::json!({"value": 1}))
                    .with_state_after(serde_json::json!({"value": 2})),
            )
            .total_duration_ms(100)
            .completed(true)
            .build();

        let explainer = DecisionExplainer::new();
        let decisions = explainer.extract_decisions(&trace);

        let state_changes = decisions
            .iter()
            .filter(|d| matches!(d.decision_type, DecisionType::StateChange { .. }))
            .count();

        assert_eq!(state_changes, 1);
    }

    #[test]
    fn test_decision_serialization() {
        let decision = Decision::new(
            DecisionType::ToolCall {
                node: "test".to_string(),
                tool: "search".to_string(),
                reason: Some("needed data".to_string()),
            },
            0,
        );

        let json = serde_json::to_string(&decision).unwrap();
        let parsed: Decision = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.execution_index, decision.execution_index);
    }

    #[test]
    fn test_explanation_serialization() {
        let decision = Decision::new(
            DecisionType::Custom {
                description: "test".to_string(),
            },
            0,
        );
        let explanation = DecisionExplanation::new(decision, "Test");

        let json = serde_json::to_string(&explanation).unwrap();
        let parsed: DecisionExplanation = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.natural_language, "Test");
    }
}
