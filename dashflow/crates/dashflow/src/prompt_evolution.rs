// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Prompt Self-Evolution
//!
//! This module enables AI agents to analyze and improve their own prompts
//! based on execution outcomes, forming a feedback loop for continuous improvement.
//!
//! # Overview
//!
//! AI agents often use prompts that may be suboptimal. This module provides
//! mechanisms to:
//!
//! 1. **Analyze prompt effectiveness** - Track which prompts led to retries,
//!    errors, or inefficient token usage
//! 2. **Generate improvement suggestions** - Based on analysis, suggest
//!    specific prompt modifications
//! 3. **Apply evolutions** - Allow the AI to improve its own prompts
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                   ExecutionTrace                             │
//! │  ┌───────────────────────────────────────────────────────┐  │
//! │  │ analyze_prompt_effectiveness()                         │  │
//! │  │   - Scans nodes for retry patterns                     │  │
//! │  │   - Identifies high error rates                        │  │
//! │  │   - Detects inefficient token usage                    │  │
//! │  └───────────────────────────────────────────────────────┘  │
//! │                            │                                 │
//! │                            ▼                                 │
//! │  ┌───────────────────────────────────────────────────────┐  │
//! │  │ evolve_prompts()                                       │  │
//! │  │   - Generates PromptEvolution suggestions              │  │
//! │  │   - Provides reasons and expected improvements         │  │
//! │  └───────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────┘
//!                            │
//!                            ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                   CompiledGraph                              │
//! │  ┌───────────────────────────────────────────────────────┐  │
//! │  │ apply_prompt_evolution()                               │  │
//! │  │   - Updates node prompts                               │  │
//! │  │   - Logs changes for traceability                      │  │
//! │  └───────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::prompt_evolution::{PromptEvolution, PromptAnalysis};
//!
//! // After execution, analyze prompt effectiveness
//! let trace = compiled.get_execution_trace(thread_id).await?;
//! let analyses = trace.analyze_prompt_effectiveness();
//!
//! // Check for problematic prompts
//! for analysis in analyses {
//!     if analysis.retry_rate > 0.3 {
//!         println!("Node '{}' has {}% retry rate", analysis.node, analysis.retry_rate * 100.0);
//!     }
//! }
//!
//! // Generate improvement suggestions
//! let evolutions = trace.evolve_prompts();
//!
//! // Apply improvements
//! for evolution in evolutions {
//!     println!("Improving prompt for '{}': {}", evolution.node, evolution.reason);
//!     compiled.apply_prompt_evolution(evolution)?;
//! }
//! ```

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ============================================================================
// Prompt Analysis Types
// ============================================================================

/// Reasons why a prompt might need improvement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PromptIssueType {
    /// High retry rate suggests unclear instructions
    HighRetryRate,
    /// High error rate indicates problematic prompt
    HighErrorRate,
    /// Excessive token usage suggests verbose prompt
    IneffientTokenUsage,
    /// Slow response times may indicate complex prompt
    SlowResponseTime,
    /// Inconsistent outputs suggest ambiguous prompt
    InconsistentOutputs,
    /// Frequent tool call failures may be prompt-related
    ToolCallFailures,
    /// Low confidence scores from model
    LowConfidenceOutputs,
    /// Repetitive or looping behavior
    RepetitiveBehavior,
}

impl std::fmt::Display for PromptIssueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HighRetryRate => write!(f, "high_retry_rate"),
            Self::HighErrorRate => write!(f, "high_error_rate"),
            Self::IneffientTokenUsage => write!(f, "inefficient_token_usage"),
            Self::SlowResponseTime => write!(f, "slow_response_time"),
            Self::InconsistentOutputs => write!(f, "inconsistent_outputs"),
            Self::ToolCallFailures => write!(f, "tool_call_failures"),
            Self::LowConfidenceOutputs => write!(f, "low_confidence_outputs"),
            Self::RepetitiveBehavior => write!(f, "repetitive_behavior"),
        }
    }
}

impl PromptIssueType {
    /// Get a human-readable description of this issue type
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::HighRetryRate => "Prompt produces outputs that frequently need retries",
            Self::HighErrorRate => "Prompt leads to high error rates during execution",
            Self::IneffientTokenUsage => "Prompt uses more tokens than necessary",
            Self::SlowResponseTime => "Prompt complexity causes slow response times",
            Self::InconsistentOutputs => "Prompt produces inconsistent or variable outputs",
            Self::ToolCallFailures => "Prompt leads to frequent tool call failures",
            Self::LowConfidenceOutputs => "Model shows low confidence in outputs",
            Self::RepetitiveBehavior => "Prompt causes repetitive or looping behavior",
        }
    }

    /// Get suggested improvements for this issue type
    #[must_use]
    pub fn improvement_suggestions(&self) -> Vec<&'static str> {
        match self {
            Self::HighRetryRate => vec![
                "Add explicit output format instructions",
                "Include examples in the prompt",
                "Break complex tasks into simpler steps",
                "Add validation criteria",
            ],
            Self::HighErrorRate => vec![
                "Add error handling instructions",
                "Clarify edge cases",
                "Include fallback behaviors",
                "Simplify the task requirements",
            ],
            Self::IneffientTokenUsage => vec![
                "Reduce system prompt verbosity",
                "Summarize conversation history",
                "Use more concise instructions",
                "Remove redundant context",
            ],
            Self::SlowResponseTime => vec![
                "Simplify the prompt structure",
                "Reduce context length",
                "Split into multiple smaller prompts",
                "Use streaming for better UX",
            ],
            Self::InconsistentOutputs => vec![
                "Add stricter output format requirements",
                "Include more examples",
                "Use temperature=0 for determinism",
                "Add validation rules",
            ],
            Self::ToolCallFailures => vec![
                "Clarify tool usage instructions",
                "Add tool-specific examples",
                "Include error handling for tools",
                "Simplify tool selection criteria",
            ],
            Self::LowConfidenceOutputs => vec![
                "Provide more context",
                "Add relevant examples",
                "Clarify the task objective",
                "Include domain-specific knowledge",
            ],
            Self::RepetitiveBehavior => vec![
                "Add loop detection instructions",
                "Include termination criteria",
                "Track and reference previous actions",
                "Limit recursion depth explicitly",
            ],
        }
    }
}

/// Analysis of a specific prompt's effectiveness
///
/// Contains metrics and insights about how a prompt performed during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptAnalysis {
    /// Node name where the prompt is used
    pub node: String,
    /// Retry rate (0.0 to 1.0) - percentage of executions that needed retries
    pub retry_rate: f64,
    /// Error rate (0.0 to 1.0) - percentage of executions that failed
    pub error_rate: f64,
    /// Average token usage per execution
    pub avg_tokens: f64,
    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,
    /// Total number of executions analyzed
    pub execution_count: usize,
    /// Number of successful executions
    pub success_count: usize,
    /// Number of failed executions
    pub failure_count: usize,
    /// Number of retries observed
    pub retry_count: usize,
    /// Issues identified in this prompt
    pub issues: Vec<PromptIssue>,
    /// Overall effectiveness score (0.0 to 1.0, higher is better)
    pub effectiveness_score: f64,
    /// Confidence in this analysis (0.0 to 1.0)
    pub confidence: f64,
}

impl PromptAnalysis {
    /// Create a new prompt analysis for a node
    #[must_use]
    pub fn new(node: impl Into<String>) -> Self {
        Self {
            node: node.into(),
            retry_rate: 0.0,
            error_rate: 0.0,
            avg_tokens: 0.0,
            avg_response_time_ms: 0.0,
            execution_count: 0,
            success_count: 0,
            failure_count: 0,
            retry_count: 0,
            issues: Vec::new(),
            effectiveness_score: 1.0,
            confidence: 0.0,
        }
    }

    /// Check if this prompt has any issues requiring attention
    #[must_use]
    pub fn has_issues(&self) -> bool {
        !self.issues.is_empty()
    }

    /// Get the most severe issue
    #[must_use]
    pub fn most_severe_issue(&self) -> Option<&PromptIssue> {
        self.issues.iter().max_by(|a, b| {
            a.severity
                .partial_cmp(&b.severity)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Check if this prompt needs improvement based on thresholds
    #[must_use]
    pub fn needs_improvement(&self, thresholds: &PromptThresholds) -> bool {
        self.retry_rate > thresholds.max_retry_rate
            || self.error_rate > thresholds.max_error_rate
            || self.avg_tokens > thresholds.max_avg_tokens
            || self.effectiveness_score < thresholds.min_effectiveness
    }

    /// Get a human-readable summary
    #[must_use]
    pub fn summary(&self) -> String {
        if self.issues.is_empty() {
            format!(
                "Node '{}': {} executions, {:.1}% effectiveness - No issues detected",
                self.node,
                self.execution_count,
                self.effectiveness_score * 100.0
            )
        } else {
            format!(
                "Node '{}': {} executions, {:.1}% effectiveness - {} issue(s): {}",
                self.node,
                self.execution_count,
                self.effectiveness_score * 100.0,
                self.issues.len(),
                self.issues
                    .iter()
                    .map(|i| i.issue_type.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
}

/// A specific issue identified in a prompt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptIssue {
    /// Type of issue
    pub issue_type: PromptIssueType,
    /// Severity (0.0 to 1.0, higher is more severe)
    pub severity: f64,
    /// Description of the issue
    pub description: String,
    /// Evidence supporting this issue
    pub evidence: String,
    /// Suggested fix
    pub suggestion: String,
}

impl PromptIssue {
    /// Create a new prompt issue
    #[must_use]
    pub fn new(issue_type: PromptIssueType, severity: f64) -> Self {
        Self {
            issue_type,
            severity: severity.clamp(0.0, 1.0),
            description: issue_type.description().to_string(),
            evidence: String::new(),
            suggestion: String::new(),
        }
    }

    /// Set the evidence for this issue
    #[must_use]
    pub fn with_evidence(mut self, evidence: impl Into<String>) -> Self {
        self.evidence = evidence.into();
        self
    }

    /// Set the suggestion for this issue
    #[must_use]
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = suggestion.into();
        self
    }

    /// Set a custom description
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }
}

// ============================================================================
// Prompt Evolution Types
// ============================================================================

/// A proposed evolution/improvement to a prompt
///
/// Represents a specific change that could be made to improve prompt effectiveness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptEvolution {
    /// Node name where the prompt should be updated
    pub node: String,
    /// Original prompt (if known)
    pub original_prompt: Option<String>,
    /// Suggested improved prompt (if generated)
    pub improved_prompt: Option<String>,
    /// Type of improvement suggested
    pub improvement_type: PromptImprovementType,
    /// Reason for this evolution
    pub reason: String,
    /// Expected improvement from applying this evolution
    pub expected_improvement: String,
    /// Confidence in this evolution (0.0 to 1.0)
    pub confidence: f64,
    /// Issues this evolution addresses
    pub addresses_issues: Vec<PromptIssueType>,
    /// Priority of this evolution (higher = more important)
    pub priority: PromptEvolutionPriority,
}

impl PromptEvolution {
    /// Create a new prompt evolution
    #[must_use]
    pub fn new(node: impl Into<String>, improvement_type: PromptImprovementType) -> Self {
        Self {
            node: node.into(),
            original_prompt: None,
            improved_prompt: None,
            improvement_type,
            reason: String::new(),
            expected_improvement: String::new(),
            confidence: 0.5,
            addresses_issues: Vec::new(),
            priority: PromptEvolutionPriority::Medium,
        }
    }

    /// Set the original prompt
    #[must_use]
    pub fn with_original_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.original_prompt = Some(prompt.into());
        self
    }

    /// Set the improved prompt
    #[must_use]
    pub fn with_improved_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.improved_prompt = Some(prompt.into());
        self
    }

    /// Set the reason for this evolution
    #[must_use]
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = reason.into();
        self
    }

    /// Set the expected improvement
    #[must_use]
    pub fn with_expected_improvement(mut self, improvement: impl Into<String>) -> Self {
        self.expected_improvement = improvement.into();
        self
    }

    /// Set the confidence level
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Add an issue this evolution addresses
    #[must_use]
    pub fn addresses(mut self, issue: PromptIssueType) -> Self {
        self.addresses_issues.push(issue);
        self
    }

    /// Set the priority
    #[must_use]
    pub fn with_priority(mut self, priority: PromptEvolutionPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Get a human-readable description
    #[must_use]
    pub fn description(&self) -> String {
        format!(
            "[{:?}] {} prompt for '{}': {} (confidence: {:.0}%)",
            self.priority,
            self.improvement_type,
            self.node,
            self.reason,
            self.confidence * 100.0
        )
    }
}

/// Type of improvement to apply to a prompt
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PromptImprovementType {
    /// Add clarifying instructions
    AddClarity,
    /// Add examples to the prompt
    AddExamples,
    /// Simplify the prompt
    Simplify,
    /// Add output format specification
    AddFormatSpec,
    /// Add error handling instructions
    AddErrorHandling,
    /// Reduce verbosity
    ReduceVerbosity,
    /// Add validation criteria
    AddValidation,
    /// Add context/background
    AddContext,
    /// Add termination criteria
    AddTerminationCriteria,
    /// Restructure the prompt
    Restructure,
}

impl std::fmt::Display for PromptImprovementType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AddClarity => write!(f, "Add clarity"),
            Self::AddExamples => write!(f, "Add examples"),
            Self::Simplify => write!(f, "Simplify"),
            Self::AddFormatSpec => write!(f, "Add format specification"),
            Self::AddErrorHandling => write!(f, "Add error handling"),
            Self::ReduceVerbosity => write!(f, "Reduce verbosity"),
            Self::AddValidation => write!(f, "Add validation"),
            Self::AddContext => write!(f, "Add context"),
            Self::AddTerminationCriteria => write!(f, "Add termination criteria"),
            Self::Restructure => write!(f, "Restructure"),
        }
    }
}

/// Priority level for prompt evolutions
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PromptEvolutionPriority {
    /// Low priority - nice to have
    Low,
    /// Medium priority - should address
    Medium,
    /// High priority - important to fix
    High,
    /// Critical priority - must fix immediately
    Critical,
}

// ============================================================================
// Thresholds and Configuration
// ============================================================================

/// Thresholds for determining when prompts need improvement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptThresholds {
    /// Maximum acceptable retry rate (0.0 to 1.0)
    pub max_retry_rate: f64,
    /// Maximum acceptable error rate (0.0 to 1.0)
    pub max_error_rate: f64,
    /// Maximum average tokens per execution
    pub max_avg_tokens: f64,
    /// Maximum average response time in milliseconds
    pub max_avg_response_time_ms: f64,
    /// Minimum effectiveness score (0.0 to 1.0)
    pub min_effectiveness: f64,
    /// Minimum executions needed for reliable analysis
    pub min_executions: usize,
    /// Maximum repetitions before flagging repetitive behavior
    pub max_repetitions: usize,
}

impl Default for PromptThresholds {
    fn default() -> Self {
        Self {
            max_retry_rate: 0.3,               // 30% retry rate is concerning
            max_error_rate: 0.1,               // 10% error rate is concerning
            max_avg_tokens: 5000.0,            // 5k tokens per execution
            max_avg_response_time_ms: 30000.0, // 30 seconds
            min_effectiveness: 0.7,            // 70% effectiveness
            min_executions: 3,                 // Need at least 3 executions
            max_repetitions: 3,                // 3+ repetitions is concerning
        }
    }
}

impl PromptThresholds {
    /// Create thresholds with stricter requirements
    #[must_use]
    pub fn strict() -> Self {
        Self {
            max_retry_rate: 0.1,
            max_error_rate: 0.05,
            max_avg_tokens: 2000.0,
            max_avg_response_time_ms: 10000.0,
            min_effectiveness: 0.9,
            min_executions: 5,
            max_repetitions: 2,
        }
    }

    /// Create thresholds with more lenient requirements
    #[must_use]
    pub fn lenient() -> Self {
        Self {
            max_retry_rate: 0.5,
            max_error_rate: 0.2,
            max_avg_tokens: 10000.0,
            max_avg_response_time_ms: 60000.0,
            min_effectiveness: 0.5,
            min_executions: 2,
            max_repetitions: 5,
        }
    }
}

// ============================================================================
// Evolution Result
// ============================================================================

/// Result of applying prompt evolutions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptEvolutionResult {
    /// Evolutions that were successfully applied
    pub applied: Vec<PromptEvolution>,
    /// Evolutions that could not be applied
    pub skipped: Vec<(PromptEvolution, String)>,
    /// Summary of changes
    pub summary: String,
}

impl PromptEvolutionResult {
    /// Create a new empty result
    #[must_use]
    pub fn new() -> Self {
        Self {
            applied: Vec::new(),
            skipped: Vec::new(),
            summary: String::new(),
        }
    }

    /// Add an applied evolution
    pub fn add_applied(&mut self, evolution: PromptEvolution) {
        self.applied.push(evolution);
    }

    /// Add a skipped evolution with reason
    pub fn add_skipped(&mut self, evolution: PromptEvolution, reason: impl Into<String>) {
        self.skipped.push((evolution, reason.into()));
    }

    /// Generate summary
    pub fn generate_summary(&mut self) {
        self.summary = format!(
            "Applied {} evolution(s), skipped {}",
            self.applied.len(),
            self.skipped.len()
        );
    }

    /// Check if any evolutions were applied
    #[must_use]
    pub fn has_changes(&self) -> bool {
        !self.applied.is_empty()
    }
}

impl Default for PromptEvolutionResult {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Analysis Functions
// ============================================================================

/// Analyze execution trace to identify prompt effectiveness issues
///
/// This function examines node executions and identifies patterns that
/// suggest prompts may need improvement.
pub fn analyze_node_executions(
    node_name: &str,
    executions: &[crate::introspection::NodeExecution],
    thresholds: &PromptThresholds,
) -> PromptAnalysis {
    let mut analysis = PromptAnalysis::new(node_name);

    if executions.is_empty() {
        return analysis;
    }

    analysis.execution_count = executions.len();

    // Calculate metrics
    let mut total_tokens = 0u64;
    let mut total_duration = 0u64;
    let mut failures = 0usize;
    let mut retries = 0usize;

    // Track consecutive identical executions for repetition detection
    let mut consecutive_similar = 0usize;
    let mut prev_state: Option<&serde_json::Value> = None;

    for exec in executions {
        total_tokens += exec.tokens_used;
        total_duration += exec.duration_ms;

        if !exec.success {
            failures += 1;
        }

        // Detect retries (same node executed multiple times in quick succession)
        // This is a heuristic - consecutive executions of the same node suggest retries
        if exec.index > 0 {
            let prev_idx = exec.index - 1;
            if executions
                .iter()
                .any(|e| e.index == prev_idx && e.node == exec.node)
            {
                retries += 1;
            }
        }

        // Check for repetitive behavior (similar state_after values)
        if let Some(state_after) = &exec.state_after {
            if let Some(prev) = prev_state {
                if state_after == prev {
                    consecutive_similar += 1;
                } else {
                    consecutive_similar = 0;
                }
            }
            prev_state = Some(state_after);
        }
    }

    analysis.success_count = analysis.execution_count - failures;
    analysis.failure_count = failures;
    analysis.retry_count = retries;

    let exec_count_f64 = analysis.execution_count as f64;
    analysis.avg_tokens = total_tokens as f64 / exec_count_f64;
    analysis.avg_response_time_ms = total_duration as f64 / exec_count_f64;
    analysis.retry_rate = retries as f64 / exec_count_f64;
    analysis.error_rate = failures as f64 / exec_count_f64;

    // Calculate effectiveness score (inverse of issues)
    let mut effectiveness = 1.0;
    effectiveness -= analysis.error_rate * 0.5; // Errors are severe
    effectiveness -= analysis.retry_rate * 0.3; // Retries indicate issues
    if analysis.avg_tokens > thresholds.max_avg_tokens {
        effectiveness -= 0.1; // Token inefficiency
    }
    if analysis.avg_response_time_ms > thresholds.max_avg_response_time_ms {
        effectiveness -= 0.1; // Slow response
    }
    analysis.effectiveness_score = effectiveness.max(0.0);

    // Confidence based on sample size
    analysis.confidence = (analysis.execution_count as f64 / 10.0).min(1.0);

    // Identify specific issues
    if analysis.execution_count >= thresholds.min_executions {
        // High retry rate
        if analysis.retry_rate > thresholds.max_retry_rate {
            let issue = PromptIssue::new(
                PromptIssueType::HighRetryRate,
                (analysis.retry_rate / thresholds.max_retry_rate).min(1.0),
            )
            .with_evidence(format!(
                "Retry rate: {:.1}% (threshold: {:.1}%)",
                analysis.retry_rate * 100.0,
                thresholds.max_retry_rate * 100.0
            ))
            .with_suggestion("Add explicit output format instructions and examples");
            analysis.issues.push(issue);
        }

        // High error rate
        if analysis.error_rate > thresholds.max_error_rate {
            let issue = PromptIssue::new(
                PromptIssueType::HighErrorRate,
                (analysis.error_rate / thresholds.max_error_rate).min(1.0),
            )
            .with_evidence(format!(
                "Error rate: {:.1}% (threshold: {:.1}%)",
                analysis.error_rate * 100.0,
                thresholds.max_error_rate * 100.0
            ))
            .with_suggestion("Add error handling instructions and clarify edge cases");
            analysis.issues.push(issue);
        }

        // Inefficient token usage
        if analysis.avg_tokens > thresholds.max_avg_tokens {
            let issue = PromptIssue::new(
                PromptIssueType::IneffientTokenUsage,
                ((analysis.avg_tokens - thresholds.max_avg_tokens) / thresholds.max_avg_tokens)
                    .min(1.0),
            )
            .with_evidence(format!(
                "Average tokens: {:.0} (threshold: {:.0})",
                analysis.avg_tokens, thresholds.max_avg_tokens
            ))
            .with_suggestion("Reduce system prompt verbosity and summarize context");
            analysis.issues.push(issue);
        }

        // Slow response time
        if analysis.avg_response_time_ms > thresholds.max_avg_response_time_ms {
            let issue = PromptIssue::new(
                PromptIssueType::SlowResponseTime,
                ((analysis.avg_response_time_ms - thresholds.max_avg_response_time_ms)
                    / thresholds.max_avg_response_time_ms)
                    .min(1.0),
            )
            .with_evidence(format!(
                "Average response time: {:.0}ms (threshold: {:.0}ms)",
                analysis.avg_response_time_ms, thresholds.max_avg_response_time_ms
            ))
            .with_suggestion("Simplify prompt structure or split into smaller prompts");
            analysis.issues.push(issue);
        }

        // Repetitive behavior
        if consecutive_similar >= thresholds.max_repetitions {
            let issue = PromptIssue::new(PromptIssueType::RepetitiveBehavior, 0.8)
                .with_evidence(format!(
                    "Detected {} consecutive similar outputs",
                    consecutive_similar
                ))
                .with_suggestion("Add termination criteria and loop detection instructions");
            analysis.issues.push(issue);
        }
    }

    analysis
}

/// Generate prompt evolutions based on analysis
///
/// Creates specific improvement suggestions based on identified issues.
pub fn generate_evolutions(
    analyses: &[PromptAnalysis],
    thresholds: &PromptThresholds,
) -> Vec<PromptEvolution> {
    let mut evolutions = Vec::new();

    for analysis in analyses {
        if !analysis.needs_improvement(thresholds) {
            continue;
        }

        for issue in &analysis.issues {
            let evolution = create_evolution_for_issue(analysis, issue);
            evolutions.push(evolution);
        }
    }

    // Sort by priority (highest first)
    evolutions.sort_by(|a, b| b.priority.cmp(&a.priority));

    evolutions
}

/// Create an evolution for a specific issue
fn create_evolution_for_issue(analysis: &PromptAnalysis, issue: &PromptIssue) -> PromptEvolution {
    let (improvement_type, priority) = match issue.issue_type {
        PromptIssueType::HighRetryRate => (
            PromptImprovementType::AddClarity,
            if issue.severity > 0.7 {
                PromptEvolutionPriority::High
            } else {
                PromptEvolutionPriority::Medium
            },
        ),
        PromptIssueType::HighErrorRate => (
            PromptImprovementType::AddErrorHandling,
            PromptEvolutionPriority::Critical,
        ),
        PromptIssueType::IneffientTokenUsage => (
            PromptImprovementType::ReduceVerbosity,
            PromptEvolutionPriority::Medium,
        ),
        PromptIssueType::SlowResponseTime => (
            PromptImprovementType::Simplify,
            PromptEvolutionPriority::Medium,
        ),
        PromptIssueType::InconsistentOutputs => (
            PromptImprovementType::AddFormatSpec,
            PromptEvolutionPriority::High,
        ),
        PromptIssueType::ToolCallFailures => (
            PromptImprovementType::AddExamples,
            PromptEvolutionPriority::High,
        ),
        PromptIssueType::LowConfidenceOutputs => (
            PromptImprovementType::AddContext,
            PromptEvolutionPriority::Medium,
        ),
        PromptIssueType::RepetitiveBehavior => (
            PromptImprovementType::AddTerminationCriteria,
            PromptEvolutionPriority::Critical,
        ),
    };

    PromptEvolution::new(&analysis.node, improvement_type)
        .with_reason(issue.description.clone())
        .with_expected_improvement(issue.suggestion.clone())
        .with_confidence(analysis.confidence * (1.0 - issue.severity * 0.2))
        .addresses(issue.issue_type)
        .with_priority(priority)
}

// ============================================================================
// Prompt Evolution Storage
// ============================================================================

/// Stores prompt evolutions for tracking and analysis
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptEvolutionHistory {
    /// All evolutions that have been generated
    pub evolutions: Vec<PromptEvolutionRecord>,
    /// Map of node name to number of evolutions
    pub evolution_counts: HashMap<String, usize>,
}

/// Record of a prompt evolution event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptEvolutionRecord {
    /// The evolution that was generated
    pub evolution: PromptEvolution,
    /// Whether it was applied
    pub applied: bool,
    /// Timestamp when generated
    pub generated_at: String,
    /// Timestamp when applied (if applicable)
    pub applied_at: Option<String>,
    /// Outcome after applying (if tracked)
    pub outcome: Option<EvolutionOutcome>,
}

/// Outcome of applying a prompt evolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionOutcome {
    /// Whether the evolution improved effectiveness
    pub improved: bool,
    /// Change in effectiveness score (-1.0 to 1.0)
    pub effectiveness_delta: f64,
    /// Change in error rate (-1.0 to 1.0)
    pub error_rate_delta: f64,
    /// Change in retry rate (-1.0 to 1.0)
    pub retry_rate_delta: f64,
    /// Notes about the outcome
    pub notes: Option<String>,
}

impl PromptEvolutionHistory {
    /// Create a new empty history
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an evolution record
    pub fn add_record(&mut self, record: PromptEvolutionRecord) {
        let node = record.evolution.node.clone();
        *self.evolution_counts.entry(node).or_insert(0) += 1;
        self.evolutions.push(record);
    }

    /// Get evolutions for a specific node
    #[must_use]
    pub fn evolutions_for_node(&self, node: &str) -> Vec<&PromptEvolutionRecord> {
        self.evolutions
            .iter()
            .filter(|r| r.evolution.node == node)
            .collect()
    }

    /// Get successful evolutions (those that improved effectiveness)
    #[must_use]
    pub fn successful_evolutions(&self) -> Vec<&PromptEvolutionRecord> {
        self.evolutions
            .iter()
            .filter(|r| r.outcome.as_ref().is_some_and(|o| o.improved))
            .collect()
    }

    /// Get the success rate of evolutions
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        let with_outcomes: Vec<_> = self
            .evolutions
            .iter()
            .filter(|r| r.outcome.is_some())
            .collect();

        if with_outcomes.is_empty() {
            return 0.0;
        }

        let successful = with_outcomes
            .iter()
            .filter(|r| r.outcome.as_ref().is_some_and(|o| o.improved))
            .count();

        successful as f64 / with_outcomes.len() as f64
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::introspection::NodeExecution;

    #[test]
    fn test_prompt_issue_type_display() {
        assert_eq!(
            PromptIssueType::HighRetryRate.to_string(),
            "high_retry_rate"
        );
        assert_eq!(
            PromptIssueType::HighErrorRate.to_string(),
            "high_error_rate"
        );
        assert_eq!(
            PromptIssueType::IneffientTokenUsage.to_string(),
            "inefficient_token_usage"
        );
    }

    #[test]
    fn test_prompt_issue_type_description() {
        let issue_type = PromptIssueType::HighRetryRate;
        let desc = issue_type.description();
        assert!(!desc.is_empty());
        assert!(desc.contains("retries")); // "Prompt produces outputs that frequently need retries"
    }

    #[test]
    fn test_prompt_issue_type_suggestions() {
        let suggestions = PromptIssueType::HighRetryRate.improvement_suggestions();
        assert!(!suggestions.is_empty());
    }

    #[test]
    fn test_prompt_analysis_new() {
        let analysis = PromptAnalysis::new("test_node");
        assert_eq!(analysis.node, "test_node");
        assert_eq!(analysis.execution_count, 0);
        assert_eq!(analysis.effectiveness_score, 1.0);
    }

    #[test]
    fn test_prompt_analysis_has_issues() {
        let mut analysis = PromptAnalysis::new("test_node");
        assert!(!analysis.has_issues());

        analysis
            .issues
            .push(PromptIssue::new(PromptIssueType::HighRetryRate, 0.5));
        assert!(analysis.has_issues());
    }

    #[test]
    fn test_prompt_analysis_needs_improvement() {
        let thresholds = PromptThresholds::default();

        let mut analysis = PromptAnalysis::new("test_node");
        analysis.retry_rate = 0.1;
        analysis.error_rate = 0.05;
        assert!(!analysis.needs_improvement(&thresholds));

        analysis.retry_rate = 0.5; // Above threshold
        assert!(analysis.needs_improvement(&thresholds));
    }

    #[test]
    fn test_prompt_issue_builder() {
        let issue = PromptIssue::new(PromptIssueType::HighErrorRate, 0.8)
            .with_evidence("Error rate is 50%")
            .with_suggestion("Add error handling");

        assert_eq!(issue.severity, 0.8);
        assert!(!issue.evidence.is_empty());
        assert!(!issue.suggestion.is_empty());
    }

    #[test]
    fn test_prompt_evolution_builder() {
        let evolution = PromptEvolution::new("reasoning", PromptImprovementType::AddClarity)
            .with_reason("High retry rate detected")
            .with_expected_improvement("Reduce retries by 50%")
            .with_confidence(0.85)
            .with_priority(PromptEvolutionPriority::High);

        assert_eq!(evolution.node, "reasoning");
        assert_eq!(evolution.confidence, 0.85);
        assert_eq!(evolution.priority, PromptEvolutionPriority::High);
    }

    #[test]
    fn test_prompt_evolution_confidence_clamping() {
        let evolution =
            PromptEvolution::new("test", PromptImprovementType::AddClarity).with_confidence(1.5);
        assert_eq!(evolution.confidence, 1.0);

        let evolution2 =
            PromptEvolution::new("test", PromptImprovementType::AddClarity).with_confidence(-0.5);
        assert_eq!(evolution2.confidence, 0.0);
    }

    #[test]
    fn test_prompt_improvement_type_display() {
        assert_eq!(PromptImprovementType::AddClarity.to_string(), "Add clarity");
        assert_eq!(
            PromptImprovementType::AddExamples.to_string(),
            "Add examples"
        );
    }

    #[test]
    fn test_prompt_thresholds_default() {
        let thresholds = PromptThresholds::default();
        assert_eq!(thresholds.max_retry_rate, 0.3);
        assert_eq!(thresholds.max_error_rate, 0.1);
        assert_eq!(thresholds.min_executions, 3);
    }

    #[test]
    fn test_prompt_thresholds_strict() {
        let thresholds = PromptThresholds::strict();
        assert!(thresholds.max_retry_rate < PromptThresholds::default().max_retry_rate);
        assert!(thresholds.max_error_rate < PromptThresholds::default().max_error_rate);
    }

    #[test]
    fn test_prompt_thresholds_lenient() {
        let thresholds = PromptThresholds::lenient();
        assert!(thresholds.max_retry_rate > PromptThresholds::default().max_retry_rate);
        assert!(thresholds.max_error_rate > PromptThresholds::default().max_error_rate);
    }

    #[test]
    fn test_prompt_evolution_result() {
        let mut result = PromptEvolutionResult::new();
        assert!(!result.has_changes());

        let evolution = PromptEvolution::new("test", PromptImprovementType::AddClarity);
        result.add_applied(evolution);
        assert!(result.has_changes());

        result.generate_summary();
        assert!(result.summary.contains("Applied 1"));
    }

    #[test]
    fn test_analyze_node_executions_empty() {
        let thresholds = PromptThresholds::default();
        let executions: Vec<NodeExecution> = Vec::new();

        let analysis = analyze_node_executions("test_node", &executions, &thresholds);
        assert_eq!(analysis.execution_count, 0);
        assert!(!analysis.has_issues());
    }

    #[test]
    fn test_analyze_node_executions_healthy() {
        let thresholds = PromptThresholds::default();
        // Non-consecutive indices simulate normal graph execution where this node
        // is called with other nodes in between (not retries)
        let executions = vec![
            NodeExecution::new("test_node", 100).with_index(0),
            NodeExecution::new("test_node", 120).with_index(5), // Not consecutive
            NodeExecution::new("test_node", 110).with_index(10), // Not consecutive
        ];

        let analysis = analyze_node_executions("test_node", &executions, &thresholds);
        assert_eq!(analysis.execution_count, 3);
        assert!(!analysis.has_issues());
    }

    #[test]
    fn test_analyze_node_executions_with_errors() {
        let thresholds = PromptThresholds::default();
        let executions = vec![
            NodeExecution::new("test_node", 100).with_index(0),
            NodeExecution::new("test_node", 120)
                .with_index(1)
                .with_error("Test error"),
            NodeExecution::new("test_node", 110)
                .with_index(2)
                .with_error("Another error"),
        ];

        let analysis = analyze_node_executions("test_node", &executions, &thresholds);
        assert_eq!(analysis.failure_count, 2);
        // Error rate is 66%, which exceeds the 10% threshold
        assert!(analysis.has_issues());
    }

    #[test]
    fn test_analyze_node_executions_high_tokens() {
        let thresholds = PromptThresholds::default();
        let executions = vec![
            NodeExecution::new("test_node", 100)
                .with_tokens(10000)
                .with_index(0),
            NodeExecution::new("test_node", 120)
                .with_tokens(12000)
                .with_index(1),
            NodeExecution::new("test_node", 110)
                .with_tokens(11000)
                .with_index(2),
        ];

        let analysis = analyze_node_executions("test_node", &executions, &thresholds);
        // Average tokens is 11000, exceeding 5000 threshold
        assert!(analysis.avg_tokens > thresholds.max_avg_tokens);
        assert!(analysis.has_issues());
        assert!(analysis
            .issues
            .iter()
            .any(|i| i.issue_type == PromptIssueType::IneffientTokenUsage));
    }

    #[test]
    fn test_generate_evolutions() {
        let thresholds = PromptThresholds::default();

        let mut analysis = PromptAnalysis::new("test_node");
        analysis.execution_count = 5;
        analysis.retry_rate = 0.5; // High retry rate
        analysis.issues.push(
            PromptIssue::new(PromptIssueType::HighRetryRate, 0.7)
                .with_evidence("50% retry rate")
                .with_suggestion("Add clarity"),
        );
        analysis.effectiveness_score = 0.5;

        let analyses = vec![analysis];
        let evolutions = generate_evolutions(&analyses, &thresholds);

        assert!(!evolutions.is_empty());
        assert_eq!(evolutions[0].node, "test_node");
        assert!(evolutions[0]
            .addresses_issues
            .contains(&PromptIssueType::HighRetryRate));
    }

    #[test]
    fn test_prompt_evolution_history() {
        let mut history = PromptEvolutionHistory::new();

        let evolution = PromptEvolution::new("test_node", PromptImprovementType::AddClarity);
        let record = PromptEvolutionRecord {
            evolution,
            applied: true,
            generated_at: "2025-01-01T00:00:00Z".to_string(),
            applied_at: Some("2025-01-01T00:00:01Z".to_string()),
            outcome: Some(EvolutionOutcome {
                improved: true,
                effectiveness_delta: 0.2,
                error_rate_delta: -0.1,
                retry_rate_delta: -0.15,
                notes: None,
            }),
        };

        history.add_record(record);

        assert_eq!(history.evolutions.len(), 1);
        assert_eq!(*history.evolution_counts.get("test_node").unwrap(), 1);
        assert_eq!(history.successful_evolutions().len(), 1);
        assert_eq!(history.success_rate(), 1.0);
    }

    #[test]
    fn test_prompt_evolution_description() {
        let evolution = PromptEvolution::new("reasoning", PromptImprovementType::AddClarity)
            .with_reason("High retry rate")
            .with_confidence(0.8)
            .with_priority(PromptEvolutionPriority::High);

        let desc = evolution.description();
        assert!(desc.contains("High"));
        assert!(desc.contains("reasoning"));
        assert!(desc.contains("80%"));
    }

    #[test]
    fn test_prompt_analysis_summary() {
        let mut analysis = PromptAnalysis::new("test_node");
        analysis.execution_count = 10;
        analysis.effectiveness_score = 0.85;

        let summary = analysis.summary();
        assert!(summary.contains("test_node"));
        assert!(summary.contains("10 executions"));
        assert!(summary.contains("85.0%"));
        assert!(summary.contains("No issues"));

        // Add an issue
        analysis
            .issues
            .push(PromptIssue::new(PromptIssueType::HighRetryRate, 0.5));

        let summary_with_issue = analysis.summary();
        assert!(summary_with_issue.contains("1 issue"));
        assert!(summary_with_issue.contains("high_retry_rate"));
    }

    #[test]
    fn test_most_severe_issue() {
        let mut analysis = PromptAnalysis::new("test_node");

        analysis
            .issues
            .push(PromptIssue::new(PromptIssueType::HighRetryRate, 0.3));
        analysis
            .issues
            .push(PromptIssue::new(PromptIssueType::HighErrorRate, 0.9));
        analysis
            .issues
            .push(PromptIssue::new(PromptIssueType::SlowResponseTime, 0.5));

        let most_severe = analysis.most_severe_issue().unwrap();
        assert_eq!(most_severe.issue_type, PromptIssueType::HighErrorRate);
        assert_eq!(most_severe.severity, 0.9);
    }

    #[test]
    fn test_priority_ordering() {
        assert!(PromptEvolutionPriority::Critical > PromptEvolutionPriority::High);
        assert!(PromptEvolutionPriority::High > PromptEvolutionPriority::Medium);
        assert!(PromptEvolutionPriority::Medium > PromptEvolutionPriority::Low);
    }
}
