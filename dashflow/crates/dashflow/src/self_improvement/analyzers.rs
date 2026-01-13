// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for analyzers
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]

//! Analysis engines for the Self-Improving Introspection System.
//!
//! These analyzers examine ExecutionTraces and identify:
//! - Capability gaps (missing functionality)
//! - Deprecation candidates (unused/redundant components)
//! - Retrospective insights (what could have been done better)
//! - Patterns (recurring issues across multiple executions)
//!
//! See `archive/roadmaps/ROADMAP_SELF_IMPROVEMENT.md` for design documentation (archived).

use super::types::{
    CapabilityGap, Citation, Counterfactual, DeprecationRecommendation, DeprecationTarget,
    GapCategory, GapManifestation, Impact, MissingToolAnalysis, RetrospectiveAnalysis,
};
use crate::introspection::{ErrorTrace, ExecutionTrace};
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

// Static regex patterns compiled once for efficiency
static UUID_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
        .expect("UUID regex is valid")
});
static NUMBER_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\d+").expect("number regex is valid"));
static WHITESPACE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+").expect("whitespace regex is valid"));
static TOOL_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"tool\s*['"]([^'"]+)['"]"#).expect("tool regex is valid"));
static FUNCTION_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"function\s*['"]([^'"]+)['"]"#).expect("function regex is valid")
});
static NOT_FOUND_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"['"]([^'"]+)['"]\s*not\s*found"#).expect("not found regex is valid")
});
static SINGLE_QUOTE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"'([^']+)'").expect("single quote regex is valid"));
static DOUBLE_QUOTE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""([^"]+)""#).expect("double quote regex is valid"));
static BACKTICK_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"`([^`]+)`").expect("backtick regex is valid"));

// =============================================================================
// CapabilityGapAnalyzer - Identifies Missing or Needed Functionality
// =============================================================================

/// Configuration for capability gap analysis.
#[derive(Debug, Clone)]
pub struct CapabilityGapConfig {
    /// Minimum error count to consider a gap (default: 2)
    pub min_error_count: usize,
    /// Minimum retry rate to consider a gap (default: 0.1 = 10%)
    pub min_retry_rate: f64,
    /// Minimum occurrences of a pattern to consider it significant (default: 3)
    pub min_pattern_occurrences: usize,
    /// Confidence threshold for reporting gaps (default: 0.5)
    pub min_confidence: f64,
}

impl Default for CapabilityGapConfig {
    fn default() -> Self {
        Self {
            min_error_count: 2,
            min_retry_rate: 0.1,
            min_pattern_occurrences: 3,
            min_confidence: 0.5,
        }
    }
}

impl CapabilityGapConfig {
    /// Create a new capability gap configuration with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the minimum error count to consider a gap.
    #[must_use]
    pub fn with_min_error_count(mut self, count: usize) -> Self {
        self.min_error_count = count;
        self
    }

    /// Set the minimum retry rate to consider a gap.
    ///
    /// # Panics
    /// Panics if rate is not in the range [0.0, 1.0].
    #[must_use]
    pub fn with_min_retry_rate(mut self, rate: f64) -> Self {
        assert!(
            (0.0..=1.0).contains(&rate),
            "min_retry_rate must be in range [0.0, 1.0], got {rate}"
        );
        self.min_retry_rate = rate;
        self
    }

    /// Set the minimum pattern occurrences to consider significant.
    #[must_use]
    pub fn with_min_pattern_occurrences(mut self, count: usize) -> Self {
        self.min_pattern_occurrences = count;
        self
    }

    /// Set the minimum confidence threshold for reporting gaps.
    ///
    /// # Panics
    /// Panics if confidence is not in the range [0.0, 1.0].
    #[must_use]
    pub fn with_min_confidence(mut self, confidence: f64) -> Self {
        assert!(
            (0.0..=1.0).contains(&confidence),
            "min_confidence must be in range [0.0, 1.0], got {confidence}"
        );
        self.min_confidence = confidence;
        self
    }

    /// Validate the configuration.
    ///
    /// Returns an error if any values are out of range.
    pub fn validate(&self) -> Result<(), String> {
        if !(0.0..=1.0).contains(&self.min_retry_rate) {
            return Err(format!(
                "min_retry_rate must be in range [0.0, 1.0], got {}",
                self.min_retry_rate
            ));
        }
        if !(0.0..=1.0).contains(&self.min_confidence) {
            return Err(format!(
                "min_confidence must be in range [0.0, 1.0], got {}",
                self.min_confidence
            ));
        }
        Ok(())
    }
}

/// Analyzes ExecutionTraces to identify capability gaps.
///
/// This analyzer examines execution patterns to find:
/// - Recurring errors that suggest missing functionality
/// - High retry rates indicating inadequate tooling
/// - Performance bottlenecks
/// - Missing integrations
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::self_improvement::{CapabilityGapAnalyzer, CapabilityGapConfig};
/// use dashflow::introspection::ExecutionTrace;
///
/// let analyzer = CapabilityGapAnalyzer::new(CapabilityGapConfig::default());
/// let traces: Vec<ExecutionTrace> = vec![/* ... */];
/// let gaps = analyzer.analyze(&traces);
///
/// for gap in gaps {
///     println!("Gap: {} (confidence: {:.0}%)", gap.description, gap.confidence * 100.0);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct CapabilityGapAnalyzer {
    config: CapabilityGapConfig,
}

impl Default for CapabilityGapAnalyzer {
    fn default() -> Self {
        Self::new(CapabilityGapConfig::default())
    }
}

impl CapabilityGapAnalyzer {
    /// Create a new analyzer with the given configuration
    #[must_use]
    pub fn new(config: CapabilityGapConfig) -> Self {
        Self { config }
    }

    /// Analyze traces to find capability gaps
    #[must_use]
    pub fn analyze(&self, traces: &[ExecutionTrace]) -> Vec<CapabilityGap> {
        let mut gaps = Vec::new();

        // Analyze different gap patterns
        gaps.extend(self.analyze_error_patterns(traces));
        gaps.extend(self.analyze_retry_patterns(traces));
        gaps.extend(self.analyze_performance_gaps(traces));
        gaps.extend(self.analyze_missing_tools(traces));

        // Filter by confidence threshold and deduplicate
        gaps.retain(|g| g.confidence >= self.config.min_confidence);
        self.deduplicate_gaps(gaps)
    }

    /// Analyze error patterns to identify capability gaps
    fn analyze_error_patterns(&self, traces: &[ExecutionTrace]) -> Vec<CapabilityGap> {
        let mut gaps = Vec::new();
        let mut error_counts: HashMap<String, Vec<&ErrorTrace>> = HashMap::new();

        // Collect errors by type/message pattern
        for trace in traces {
            for error in &trace.errors {
                let key = self.normalize_error_message(&error.message);
                error_counts.entry(key).or_default().push(error);
            }
        }

        // Generate gaps for recurring errors
        for (error_pattern, errors) in error_counts {
            if errors.len() >= self.config.min_error_count {
                let affected_nodes: Vec<String> = errors
                    .iter()
                    .map(|e| e.node.clone())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();

                let gap = self.error_pattern_to_gap(&error_pattern, &errors, &affected_nodes);
                gaps.push(gap);
            }
        }

        gaps
    }

    /// Analyze retry patterns to identify capability gaps
    fn analyze_retry_patterns(&self, traces: &[ExecutionTrace]) -> Vec<CapabilityGap> {
        let mut gaps = Vec::new();
        let mut node_retries: HashMap<String, (usize, usize)> = HashMap::new(); // (retries, total)

        // Count retries per node
        for trace in traces {
            for error in &trace.errors {
                if error.retry_attempted {
                    let entry = node_retries.entry(error.node.clone()).or_insert((0, 0));
                    entry.0 += 1;
                }
            }
            for node in &trace.nodes_executed {
                let entry = node_retries.entry(node.node.clone()).or_insert((0, 0));
                entry.1 += 1;
            }
        }

        // Identify nodes with high retry rates
        for (node, (retries, total)) in node_retries {
            if total > 0 {
                let retry_rate = retries as f64 / total as f64;
                if retry_rate >= self.config.min_retry_rate {
                    let gap = CapabilityGap::new(
                        format!("High retry rate in node '{node}'"),
                        GapCategory::InadequateFunctionality {
                            node: node.clone(),
                            limitation: format!(
                                "Node has {:.1}% retry rate ({} retries in {} executions)",
                                retry_rate * 100.0,
                                retries,
                                total
                            ),
                        },
                        GapManifestation::Retries {
                            rate: retry_rate,
                            affected_nodes: vec![node.clone()],
                        },
                    )
                    .with_solution(format!(
                        "Investigate why '{node}' requires retries and improve error handling or add fallback logic"
                    ))
                    .with_impact(Impact::medium(format!(
                        "Reduce {:.1}% retry rate to improve reliability",
                        retry_rate * 100.0
                    )))
                    .with_confidence(self.calculate_retry_confidence(retry_rate, total));

                    gaps.push(gap);
                }
            }
        }

        gaps
    }

    /// Analyze performance patterns to identify performance gaps
    fn analyze_performance_gaps(&self, traces: &[ExecutionTrace]) -> Vec<CapabilityGap> {
        let mut gaps = Vec::new();
        let mut node_durations: HashMap<String, Vec<u64>> = HashMap::new();

        // Collect durations per node
        for trace in traces {
            for node in &trace.nodes_executed {
                node_durations
                    .entry(node.node.clone())
                    .or_default()
                    .push(node.duration_ms);
            }
        }

        // Identify slow nodes (p95 > 1000ms)
        for (node, durations) in node_durations {
            if durations.len() >= self.config.min_pattern_occurrences {
                let p95 = percentile_95(&durations);
                let avg = durations.iter().sum::<u64>() / durations.len() as u64;

                if p95 > 1000 {
                    let gap = CapabilityGap::new(
                        format!("Performance bottleneck in node '{node}'"),
                        GapCategory::PerformanceGap {
                            bottleneck: format!(
                                "p95 latency: {}ms, avg: {}ms over {} executions",
                                p95,
                                avg,
                                durations.len()
                            ),
                        },
                        GapManifestation::SuboptimalPaths {
                            description: format!(
                                "Node '{node}' consistently takes >1s (p95: {}ms)",
                                p95
                            ),
                        },
                    )
                    .with_solution(format!(
                        "Optimize '{node}' node - consider caching, parallel execution, or algorithm improvements"
                    ))
                    .with_impact(Impact {
                        error_reduction: 0.0,
                        latency_reduction_ms: (p95 - avg) as f64,
                        accuracy_improvement: 0.0,
                        description: format!("Reduce latency from {}ms p95 to ~{}ms", p95, avg),
                    })
                    .with_confidence(0.7);

                    gaps.push(gap);
                }
            }
        }

        gaps
    }

    /// Analyze tool usage patterns to identify missing tools
    fn analyze_missing_tools(&self, traces: &[ExecutionTrace]) -> Vec<CapabilityGap> {
        let mut gaps = Vec::new();
        let mut tool_errors: HashMap<String, usize> = HashMap::new();

        // Look for patterns suggesting missing tools
        for trace in traces {
            for error in &trace.errors {
                // Check for tool-related errors
                if error.message.contains("tool")
                    || error.message.contains("function")
                    || error.message.contains("not found")
                    || error.message.contains("undefined")
                {
                    let key = self.extract_missing_tool_name(&error.message);
                    *tool_errors.entry(key).or_insert(0) += 1;
                }
            }
        }

        for (tool_pattern, count) in tool_errors {
            if count >= self.config.min_error_count && !tool_pattern.is_empty() {
                let gap = CapabilityGap::new(
                    format!("Missing tool: {tool_pattern}"),
                    GapCategory::MissingTool {
                        tool_description: format!(
                            "Tool '{tool_pattern}' referenced but not available ({} errors)",
                            count
                        ),
                    },
                    GapManifestation::Errors {
                        count,
                        sample_messages: vec![format!(
                            "Tool '{}' not found or undefined",
                            tool_pattern
                        )],
                    },
                )
                .with_solution(format!(
                    "Implement tool '{}' or provide alternative functionality",
                    tool_pattern
                ))
                .with_impact(Impact::high(format!(
                    "Enable functionality currently failing due to missing '{}'",
                    tool_pattern
                )))
                .with_confidence(0.8);

                gaps.push(gap);
            }
        }

        gaps
    }

    /// Normalize error messages to find patterns
    fn normalize_error_message(&self, message: &str) -> String {
        // Remove UUIDs, timestamps, and specific values to find patterns
        let mut normalized = message.to_lowercase();

        // Remove UUIDs (8-4-4-4-12 pattern)
        normalized = UUID_PATTERN.replace_all(&normalized, "<uuid>").to_string();

        // Remove numbers
        normalized = NUMBER_PATTERN.replace_all(&normalized, "<n>").to_string();

        // Remove quotes and excessive whitespace
        normalized = normalized.replace(['"', '\''], "");
        normalized = WHITESPACE_PATTERN
            .replace_all(&normalized, " ")
            .trim()
            .to_string();

        normalized
    }

    /// Extract a potential missing tool name from an error message
    fn extract_missing_tool_name(&self, message: &str) -> String {
        // Look for patterns like "tool 'xxx'" or "function 'xxx'" or "'xxx' not found"
        let patterns: &[&Regex] = &[&*TOOL_PATTERN, &*FUNCTION_PATTERN, &*NOT_FOUND_PATTERN];

        for pattern in patterns {
            if let Some(captures) = pattern.captures(message) {
                if let Some(name) = captures.get(1) {
                    return name.as_str().to_string();
                }
            }
        }

        String::new()
    }

    /// Convert an error pattern to a capability gap
    fn error_pattern_to_gap(
        &self,
        pattern: &str,
        errors: &[&ErrorTrace],
        affected_nodes: &[String],
    ) -> CapabilityGap {
        let sample_messages: Vec<String> =
            errors.iter().take(3).map(|e| e.message.clone()).collect();

        let citations: Vec<Citation> = errors
            .iter()
            .filter_map(|e| e.timestamp.as_ref())
            .take(3)
            .map(|ts| Citation::trace(format!("error-{}", ts)))
            .collect();

        let category = self.categorize_error_gap(pattern, affected_nodes);
        let confidence = self.calculate_error_confidence(errors.len(), affected_nodes.len());

        CapabilityGap::new(
            format!("Recurring error pattern: {}", truncate(pattern, 60)),
            category,
            GapManifestation::Errors {
                count: errors.len(),
                sample_messages,
            },
        )
        .with_evidence(citations)
        .with_solution(format!(
            "Address root cause of '{}' errors in nodes: {}",
            truncate(pattern, 40),
            affected_nodes.join(", ")
        ))
        .with_impact(Impact::medium(format!(
            "Eliminate {} recurring errors",
            errors.len()
        )))
        .with_confidence(confidence)
    }

    /// Categorize an error gap based on the pattern
    fn categorize_error_gap(&self, pattern: &str, affected_nodes: &[String]) -> GapCategory {
        if pattern.contains("timeout") || pattern.contains("timed out") {
            GapCategory::PerformanceGap {
                bottleneck: "Timeout errors".to_string(),
            }
        } else if pattern.contains("not found") || pattern.contains("undefined") {
            GapCategory::MissingTool {
                tool_description: "Missing dependency or tool".to_string(),
            }
        } else if pattern.contains("validation") || pattern.contains("invalid") {
            GapCategory::InadequateFunctionality {
                node: affected_nodes.first().cloned().unwrap_or_default(),
                limitation: "Validation failures".to_string(),
            }
        } else {
            GapCategory::InadequateFunctionality {
                node: affected_nodes.first().cloned().unwrap_or_default(),
                limitation: format!("Error pattern: {}", truncate(pattern, 50)),
            }
        }
    }

    /// Calculate confidence based on error count and node spread
    fn calculate_error_confidence(&self, error_count: usize, node_count: usize) -> f64 {
        let count_factor = (error_count as f64 / 10.0).min(1.0);
        let spread_factor = (node_count as f64 / 3.0).min(1.0);
        (0.5 + count_factor * 0.3 + spread_factor * 0.2).clamp(0.0, 1.0)
    }

    /// Calculate confidence based on retry rate and sample size
    fn calculate_retry_confidence(&self, retry_rate: f64, sample_size: usize) -> f64 {
        let rate_factor = retry_rate.min(1.0);
        let sample_factor = (sample_size as f64 / 20.0).min(1.0);
        (0.4 + rate_factor * 0.4 + sample_factor * 0.2).clamp(0.0, 1.0)
    }

    /// Deduplicate gaps with similar descriptions
    fn deduplicate_gaps(&self, mut gaps: Vec<CapabilityGap>) -> Vec<CapabilityGap> {
        gaps.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut seen_patterns: Vec<String> = Vec::new();
        gaps.retain(|gap| {
            let normalized = self.normalize_error_message(&gap.description);
            if seen_patterns
                .iter()
                .any(|p| self.patterns_similar(p, &normalized))
            {
                false
            } else {
                seen_patterns.push(normalized);
                true
            }
        });

        gaps
    }

    /// Check if two patterns are similar enough to be considered duplicates
    fn patterns_similar(&self, a: &str, b: &str) -> bool {
        // Simple similarity check - could be enhanced with edit distance
        let a_words: std::collections::HashSet<_> = a.split_whitespace().collect();
        let b_words: std::collections::HashSet<_> = b.split_whitespace().collect();

        if a_words.is_empty() || b_words.is_empty() {
            return false;
        }

        let intersection = a_words.intersection(&b_words).count();
        let union = a_words.union(&b_words).count();

        intersection as f64 / union as f64 > 0.7
    }
}

// =============================================================================
// DeprecationAnalyzer - Identifies Unused/Redundant Components
// =============================================================================

/// Configuration for deprecation analysis.
#[derive(Debug, Clone)]
pub struct DeprecationConfig {
    /// Minimum executions to consider for deprecation analysis
    pub min_total_executions: usize,
    /// Nodes with usage below this percentage are candidates (default: 0.01 = 1%)
    pub usage_threshold: f64,
    /// Minimum confidence to report deprecation (default: 0.6)
    pub min_confidence: f64,
}

impl Default for DeprecationConfig {
    fn default() -> Self {
        Self {
            min_total_executions: 10,
            usage_threshold: 0.01,
            min_confidence: 0.6,
        }
    }
}

/// Analyzes ExecutionTraces to identify deprecation candidates.
///
/// This analyzer examines execution patterns to find:
/// - Nodes that are never or rarely executed
/// - Redundant edges (never taken)
/// - Unused tools
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::self_improvement::{DeprecationAnalyzer, DeprecationConfig};
/// use dashflow::introspection::ExecutionTrace;
///
/// let analyzer = DeprecationAnalyzer::new(DeprecationConfig::default());
/// let traces: Vec<ExecutionTrace> = vec![/* ... */];
/// let deprecations = analyzer.analyze(&traces, &known_nodes);
/// ```
#[derive(Debug, Clone)]
pub struct DeprecationAnalyzer {
    config: DeprecationConfig,
}

impl Default for DeprecationAnalyzer {
    fn default() -> Self {
        Self::new(DeprecationConfig::default())
    }
}

impl DeprecationAnalyzer {
    /// Create a new analyzer with the given configuration
    #[must_use]
    pub fn new(config: DeprecationConfig) -> Self {
        Self { config }
    }

    /// Analyze traces to find deprecation candidates
    ///
    /// `known_nodes` is the list of all nodes defined in the graph, including
    /// those that may not have been executed in the analyzed traces.
    #[must_use]
    pub fn analyze(
        &self,
        traces: &[ExecutionTrace],
        known_nodes: &[String],
    ) -> Vec<DeprecationRecommendation> {
        if traces.len() < self.config.min_total_executions {
            return Vec::new();
        }

        let mut deprecations = Vec::new();

        // Analyze node usage
        deprecations.extend(self.analyze_unused_nodes(traces, known_nodes));

        // Analyze tool usage
        deprecations.extend(self.analyze_unused_tools(traces));

        // Filter by confidence
        deprecations.retain(|d| d.confidence >= self.config.min_confidence);
        deprecations
    }

    /// Find nodes that are never or rarely executed
    fn analyze_unused_nodes(
        &self,
        traces: &[ExecutionTrace],
        known_nodes: &[String],
    ) -> Vec<DeprecationRecommendation> {
        let mut deprecations = Vec::new();
        let mut node_usage: HashMap<String, usize> = HashMap::new();

        // Count node executions
        for trace in traces {
            for node in &trace.nodes_executed {
                *node_usage.entry(node.node.clone()).or_insert(0) += 1;
            }
        }

        let total_executions = traces.len();

        // Check known nodes against actual usage
        for node in known_nodes {
            let usage = node_usage.get(node).copied().unwrap_or(0);
            let usage_rate = usage as f64 / total_executions as f64;

            if usage_rate < self.config.usage_threshold {
                let dep = DeprecationRecommendation::new(
                    DeprecationTarget::Node {
                        name: node.clone(),
                        usage_count: usage,
                    },
                    if usage == 0 {
                        format!(
                            "Node '{}' was never executed in {} traces",
                            node, total_executions
                        )
                    } else {
                        format!(
                            "Node '{}' was executed only {} times ({:.2}%) in {} traces",
                            node,
                            usage,
                            usage_rate * 100.0,
                            total_executions
                        )
                    },
                )
                .with_benefits(vec![
                    "Remove unused code".to_string(),
                    "Simplify graph structure".to_string(),
                    "Reduce maintenance burden".to_string(),
                ])
                .with_risks(if usage == 0 {
                    vec!["Node may be used in untested scenarios".to_string()]
                } else {
                    vec![format!(
                        "Node is used in {:.2}% of executions - verify these are not critical",
                        usage_rate * 100.0
                    )]
                })
                .with_confidence(
                    self.calculate_node_deprecation_confidence(usage, total_executions),
                )
                .with_evidence(vec![Citation::aggregation(
                    format!("SELECT node, COUNT(*) FROM traces WHERE node = '{}'", node),
                    format!(
                        "{} executions of '{}' in {} traces",
                        usage, node, total_executions
                    ),
                )]);

                deprecations.push(dep);
            }
        }

        deprecations
    }

    /// Find tools that are never used
    fn analyze_unused_tools(&self, traces: &[ExecutionTrace]) -> Vec<DeprecationRecommendation> {
        let mut deprecations = Vec::new();
        let mut tool_usage: HashMap<String, usize> = HashMap::new();
        let mut all_tools: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Collect all tools and their usage
        for trace in traces {
            for node in &trace.nodes_executed {
                for tool in &node.tools_called {
                    *tool_usage.entry(tool.clone()).or_insert(0) += 1;
                    all_tools.insert(tool.clone());
                }
            }
        }

        // Currently we can only analyze tools that have been called at least once
        // To find completely unused tools, we would need a registry of available tools
        // This could be enhanced by accepting a list of known tools as parameter

        // For now, identify tools with very low usage compared to others
        if !tool_usage.is_empty() {
            let max_usage = *tool_usage.values().max().unwrap_or(&1);
            let avg_usage = tool_usage.values().sum::<usize>() as f64 / tool_usage.len() as f64;

            for (tool, usage) in &tool_usage {
                let relative_usage = *usage as f64 / max_usage as f64;
                if relative_usage < 0.05 && (*usage as f64) < avg_usage * 0.1 {
                    let dep = DeprecationRecommendation::new(
                        DeprecationTarget::Tool {
                            name: tool.clone(),
                            last_used: None,
                        },
                        format!(
                            "Tool '{}' is rarely used ({} calls, {:.1}% of most used tool)",
                            tool,
                            usage,
                            relative_usage * 100.0
                        ),
                    )
                    .with_benefits(vec![
                        "Reduce tool maintenance".to_string(),
                        "Simplify tool registry".to_string(),
                    ])
                    .with_risks(vec![format!(
                        "Tool is still called {} times - verify not critical",
                        usage
                    )])
                    .with_confidence(0.5); // Lower confidence for tool deprecation

                    deprecations.push(dep);
                }
            }
        }

        deprecations
    }

    /// Calculate confidence for node deprecation
    fn calculate_node_deprecation_confidence(&self, usage: usize, total: usize) -> f64 {
        if usage == 0 {
            // Never used - high confidence if enough samples
            (0.7 + (total as f64 / 100.0).min(0.3)).clamp(0.0, 1.0)
        } else {
            // Low usage - confidence depends on how low
            let usage_rate = usage as f64 / total as f64;
            (0.5 + (1.0 - usage_rate) * 0.3 + (total as f64 / 100.0).min(0.2)).clamp(0.0, 1.0)
        }
    }
}

// =============================================================================
// RetrospectiveAnalyzer - Counterfactual Analysis
// =============================================================================

/// Configuration for retrospective analysis.
#[derive(Debug, Clone)]
pub struct RetrospectiveConfig {
    /// Minimum improvement to suggest an alternative (default: 0.1 = 10%)
    pub min_improvement_threshold: f64,
    /// Include platform-level insights (default: true)
    pub include_platform_insights: bool,
}

impl Default for RetrospectiveConfig {
    fn default() -> Self {
        Self {
            min_improvement_threshold: 0.1,
            include_platform_insights: true,
        }
    }
}

/// Analyzes ExecutionTraces to generate retrospective insights.
///
/// This analyzer examines what happened and suggests what could have been done
/// differently, including counterfactual analysis and missing tool identification.
#[derive(Debug, Clone)]
pub struct RetrospectiveAnalyzer {
    config: RetrospectiveConfig,
}

impl Default for RetrospectiveAnalyzer {
    fn default() -> Self {
        Self::new(RetrospectiveConfig::default())
    }
}

impl RetrospectiveAnalyzer {
    /// Create a new analyzer with the given configuration
    #[must_use]
    pub fn new(config: RetrospectiveConfig) -> Self {
        Self { config }
    }

    /// Generate retrospective analysis from traces
    #[must_use]
    pub fn analyze(&self, traces: &[ExecutionTrace]) -> RetrospectiveAnalysis {
        let platform_insights = if self.config.include_platform_insights {
            self.generate_platform_insights(traces)
        } else {
            Vec::new()
        };

        RetrospectiveAnalysis {
            actual_execution: self.build_execution_summary(traces),
            counterfactuals: self.generate_counterfactuals(traces),
            missing_tools: self.identify_missing_tools(traces),
            application_insights: self.generate_application_insights(traces),
            task_insights: self.generate_task_insights(traces),
            platform_insights,
        }
    }

    /// Build execution summary from traces
    fn build_execution_summary(
        &self,
        traces: &[ExecutionTrace],
    ) -> crate::self_improvement::types::ReportExecutionSummary {
        use crate::self_improvement::types::ReportExecutionSummary;

        let total = traces.len();
        let successful = traces.iter().filter(|t| t.is_successful()).count();
        let failed = total - successful;

        let total_duration: u64 = traces.iter().map(|t| t.total_duration_ms).sum();
        let avg_duration = if total > 0 {
            total_duration as f64 / total as f64
        } else {
            0.0
        };

        let total_tokens: u64 = traces.iter().map(|t| t.total_tokens).sum();
        let avg_tokens = if total > 0 {
            total_tokens as f64 / total as f64
        } else {
            0.0
        };

        let retries: usize = traces
            .iter()
            .flat_map(|t| &t.errors)
            .filter(|e| e.retry_attempted)
            .count();
        let retry_rate = if total > 0 {
            retries as f64 / total as f64
        } else {
            0.0
        };

        ReportExecutionSummary {
            total_executions: total,
            successful_executions: successful,
            failed_executions: failed,
            success_rate: if total > 0 {
                successful as f64 / total as f64
            } else {
                0.0
            },
            avg_duration_ms: avg_duration,
            total_tokens,
            avg_tokens,
            retry_rate,
            vs_previous: None,
        }
    }

    /// Generate counterfactual suggestions
    fn generate_counterfactuals(&self, traces: &[ExecutionTrace]) -> Vec<Counterfactual> {
        let mut counterfactuals = Vec::new();

        // Analyze failed executions for counterfactuals
        let failed_traces: Vec<_> = traces.iter().filter(|t| !t.is_successful()).collect();

        if !failed_traces.is_empty() {
            // Look for patterns in failures
            let mut failure_nodes: HashMap<String, usize> = HashMap::new();
            for trace in &failed_traces {
                for error in &trace.errors {
                    *failure_nodes.entry(error.node.clone()).or_insert(0) += 1;
                }
            }

            // Suggest counterfactuals for frequent failure points
            for (node, count) in failure_nodes {
                if count >= 2 {
                    counterfactuals.push(Counterfactual {
                        alternative: format!(
                            "Add retry logic or fallback mechanism for '{}'",
                            node
                        ),
                        expected_outcome: format!(
                            "Reduce failures from {} occurrences in '{}'",
                            count, node
                        ),
                        why_not_taken: "No fallback mechanism was implemented".to_string(),
                        confidence: 0.7,
                    });
                }
            }
        }

        // Analyze slow executions
        let slow_traces: Vec<_> = traces
            .iter()
            .filter(|t| t.total_duration_ms > 5000)
            .collect();

        if !slow_traces.is_empty() {
            let avg_slow_duration = slow_traces.iter().map(|t| t.total_duration_ms).sum::<u64>()
                / slow_traces.len() as u64;

            counterfactuals.push(Counterfactual {
                alternative: "Implement caching or parallel execution for slow operations"
                    .to_string(),
                expected_outcome: format!(
                    "Reduce average execution time from {}ms for {} slow executions",
                    avg_slow_duration,
                    slow_traces.len()
                ),
                why_not_taken: "Performance optimization not prioritized".to_string(),
                confidence: 0.6,
            });
        }

        counterfactuals
    }

    /// Identify tools that would have been helpful
    fn identify_missing_tools(&self, traces: &[ExecutionTrace]) -> Vec<MissingToolAnalysis> {
        let mut missing_tools = Vec::new();

        // Analyze error patterns for missing functionality
        let mut error_patterns: HashMap<String, usize> = HashMap::new();
        for trace in traces {
            for error in &trace.errors {
                // Look for patterns suggesting missing tools
                if error.message.contains("not found")
                    || error.message.contains("unavailable")
                    || error.message.contains("not implemented")
                {
                    *error_patterns.entry(error.message.clone()).or_insert(0) += 1;
                }
            }
        }

        for (pattern, count) in error_patterns {
            if count >= 2 {
                missing_tools.push(MissingToolAnalysis {
                    tool_name: extract_tool_name_from_error(&pattern),
                    description: format!("Tool to handle: {}", truncate(&pattern, 60)),
                    benefit: format!("Would prevent {} errors", count),
                    impact: Impact::medium(format!("Eliminate {} recurring errors", count)),
                });
            }
        }

        missing_tools
    }

    /// Generate application-specific insights
    fn generate_application_insights(&self, traces: &[ExecutionTrace]) -> Vec<String> {
        let mut insights = Vec::new();

        // Analyze execution patterns
        let total = traces.len();
        let with_errors = traces.iter().filter(|t| t.has_errors()).count();
        let success_rate = if total > 0 {
            (total - with_errors) as f64 / total as f64
        } else {
            0.0
        };

        if success_rate < 0.9 {
            insights.push(format!(
                "Success rate of {:.1}% is below target. Focus on error reduction.",
                success_rate * 100.0
            ));
        }

        // Analyze node distribution
        let mut node_counts: HashMap<String, usize> = HashMap::new();
        for trace in traces {
            for node in &trace.nodes_executed {
                *node_counts.entry(node.node.clone()).or_insert(0) += 1;
            }
        }

        if let Some((hottest_node, count)) = node_counts.iter().max_by_key(|(_, c)| *c) {
            let usage_pct = (*count as f64 / total as f64) * 100.0;
            if usage_pct > 150.0 {
                // More than 1.5x per execution on average
                insights.push(format!(
                    "Node '{}' is heavily used ({:.0}% of executions). Consider optimizing.",
                    hottest_node, usage_pct
                ));
            }
        }

        insights
    }

    /// Generate task-specific insights
    fn generate_task_insights(&self, traces: &[ExecutionTrace]) -> Vec<String> {
        let mut insights = Vec::new();

        // Look at execution durations
        let durations: Vec<u64> = traces.iter().map(|t| t.total_duration_ms).collect();
        if !durations.is_empty() {
            let avg = durations.iter().sum::<u64>() / durations.len() as u64;
            let max = *durations.iter().max().unwrap_or(&0);
            let min = *durations.iter().min().unwrap_or(&0);

            if max > avg * 3 {
                insights.push(format!(
                    "High variance in execution time ({}ms - {}ms). Identify outlier causes.",
                    min, max
                ));
            }
        }

        // Look at token usage
        let tokens: Vec<u64> = traces.iter().map(|t| t.total_tokens).collect();
        if !tokens.is_empty() {
            let avg_tokens = tokens.iter().sum::<u64>() / tokens.len() as u64;
            if avg_tokens > 10000 {
                insights.push(format!(
                    "High average token usage ({} tokens/execution). Consider prompt optimization.",
                    avg_tokens
                ));
            }
        }

        insights
    }

    /// Generate platform-level insights
    fn generate_platform_insights(&self, traces: &[ExecutionTrace]) -> Vec<String> {
        let mut insights = Vec::new();

        // Check for common platform issues
        let timeout_errors = traces
            .iter()
            .flat_map(|t| &t.errors)
            .filter(|e| e.message.contains("timeout") || e.error_type.as_deref() == Some("Timeout"))
            .count();

        if timeout_errors > 0 {
            insights.push(format!(
                "{} timeout errors detected. Consider increasing timeout limits or optimizing slow nodes.",
                timeout_errors
            ));
        }

        // Check for retry patterns
        let retries = traces
            .iter()
            .flat_map(|t| &t.errors)
            .filter(|e| e.retry_attempted)
            .count();

        if retries > traces.len() / 2 {
            insights.push(format!(
                "High retry count ({} retries in {} executions). Implement better error handling.",
                retries,
                traces.len()
            ));
        }

        insights
    }
}

// =============================================================================
// PatternDetector - Identifies Recurring Issues
// =============================================================================
//
// **DEPRECATION NOTICE**: Consider using `pattern_engine::UnifiedPatternEngine`
// which provides a unified API across all pattern detection systems.
//
// Migration:
//   use dashflow::pattern_engine::{UnifiedPatternEngineBuilder, PatternSource};
//   let engine = UnifiedPatternEngineBuilder::new()
//       .enable_self_improvement_patterns()  // This includes PatternDetector
//       .build();
//   let patterns = engine.detect(&traces);
//

/// Configuration for pattern detection.
#[derive(Debug, Clone)]
pub struct PatternConfig {
    /// Minimum occurrences to consider a pattern (default: 3)
    pub min_pattern_occurrences: usize,
    /// Minimum confidence for pattern (default: 0.6)
    pub min_confidence: f64,
}

impl Default for PatternConfig {
    fn default() -> Self {
        Self {
            min_pattern_occurrences: 3,
            min_confidence: 0.6,
        }
    }
}

/// A detected pattern in execution traces.
#[derive(Debug, Clone)]
pub struct DetectedPattern {
    /// Pattern description
    pub description: String,
    /// Pattern type
    pub pattern_type: PatternType,
    /// Number of occurrences
    pub occurrences: usize,
    /// Affected nodes
    pub affected_nodes: Vec<String>,
    /// Confidence in the pattern (0.0 - 1.0)
    pub confidence: f64,
    /// Suggested action
    pub suggestion: String,
    /// Supporting evidence
    pub evidence: Vec<Citation>,
}

/// Type of detected pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternType {
    /// Recurring error pattern
    RecurringError,
    /// Performance degradation pattern
    PerformanceDegradation,
    /// Execution flow pattern
    ExecutionFlow,
    /// Resource usage pattern
    ResourceUsage,
}

/// Detects recurring patterns across execution traces.
///
/// This analyzer identifies:
/// - Recurring error patterns
/// - Performance degradation over time
/// - Common execution paths
/// - Resource usage anomalies
#[derive(Debug, Clone)]
pub struct PatternDetector {
    config: PatternConfig,
}

impl Default for PatternDetector {
    fn default() -> Self {
        Self::new(PatternConfig::default())
    }
}

impl PatternDetector {
    /// Create a new pattern detector
    #[must_use]
    pub fn new(config: PatternConfig) -> Self {
        Self { config }
    }

    /// Detect patterns in execution traces
    #[must_use]
    pub fn detect(&self, traces: &[ExecutionTrace]) -> Vec<DetectedPattern> {
        let mut patterns = Vec::new();

        patterns.extend(self.detect_error_patterns(traces));
        patterns.extend(self.detect_performance_patterns(traces));
        patterns.extend(self.detect_execution_flow_patterns(traces));
        patterns.extend(self.detect_resource_patterns(traces));

        // Filter by confidence
        patterns.retain(|p| p.confidence >= self.config.min_confidence);
        patterns
    }

    /// Detect recurring error patterns
    fn detect_error_patterns(&self, traces: &[ExecutionTrace]) -> Vec<DetectedPattern> {
        let mut patterns = Vec::new();
        let mut error_groups: HashMap<String, Vec<&ErrorTrace>> = HashMap::new();

        // Group similar errors
        for trace in traces {
            for error in &trace.errors {
                let key = normalize_for_pattern(&error.message);
                error_groups.entry(key).or_default().push(error);
            }
        }

        // Create patterns for recurring errors
        for (pattern_key, errors) in error_groups {
            if errors.len() >= self.config.min_pattern_occurrences {
                let affected_nodes: Vec<String> = errors
                    .iter()
                    .map(|e| e.node.clone())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();

                patterns.push(DetectedPattern {
                    description: format!(
                        "Recurring error: {} ({} occurrences)",
                        truncate(&pattern_key, 50),
                        errors.len()
                    ),
                    pattern_type: PatternType::RecurringError,
                    occurrences: errors.len(),
                    affected_nodes: affected_nodes.clone(),
                    confidence: calculate_pattern_confidence(errors.len(), traces.len()),
                    suggestion: format!(
                        "Investigate root cause in nodes: {}",
                        affected_nodes.join(", ")
                    ),
                    evidence: vec![Citation::aggregation(
                        "Error pattern analysis",
                        format!("{} similar errors detected", errors.len()),
                    )],
                });
            }
        }

        patterns
    }

    /// Detect performance degradation patterns
    fn detect_performance_patterns(&self, traces: &[ExecutionTrace]) -> Vec<DetectedPattern> {
        let mut patterns = Vec::new();

        if traces.len() < self.config.min_pattern_occurrences * 2 {
            return patterns;
        }

        // Split traces into first and second half
        let mid = traces.len() / 2;
        let first_half: Vec<_> = traces[..mid].iter().collect();
        let second_half: Vec<_> = traces[mid..].iter().collect();

        let first_avg = if first_half.is_empty() {
            0.0
        } else {
            first_half.iter().map(|t| t.total_duration_ms).sum::<u64>() as f64
                / first_half.len() as f64
        };

        let second_avg = if second_half.is_empty() {
            0.0
        } else {
            second_half.iter().map(|t| t.total_duration_ms).sum::<u64>() as f64
                / second_half.len() as f64
        };

        // Check for significant degradation (>20% increase)
        if second_avg > first_avg * 1.2 && first_avg > 0.0 {
            let increase_pct = ((second_avg - first_avg) / first_avg) * 100.0;
            patterns.push(DetectedPattern {
                description: format!(
                    "Performance degradation detected: {:.1}% increase in execution time",
                    increase_pct
                ),
                pattern_type: PatternType::PerformanceDegradation,
                occurrences: traces.len(),
                affected_nodes: vec![],
                confidence: calculate_degradation_confidence(increase_pct, traces.len()),
                suggestion:
                    "Investigate performance regression - compare recent vs older executions"
                        .to_string(),
                evidence: vec![Citation::aggregation(
                    "Performance trend analysis",
                    format!(
                        "Avg time increased from {:.0}ms to {:.0}ms",
                        first_avg, second_avg
                    ),
                )],
            });
        }

        patterns
    }

    /// Detect execution flow patterns
    fn detect_execution_flow_patterns(&self, traces: &[ExecutionTrace]) -> Vec<DetectedPattern> {
        let mut patterns = Vec::new();
        let mut flow_counts: HashMap<String, usize> = HashMap::new();

        // Create flow signatures (sequence of nodes)
        for trace in traces {
            let flow: Vec<_> = trace
                .nodes_executed
                .iter()
                .map(|n| n.node.as_str())
                .collect();
            let signature = flow.join(" -> ");
            *flow_counts.entry(signature).or_insert(0) += 1;
        }

        // Find dominant flows
        let total = traces.len();
        for (flow, count) in &flow_counts {
            let ratio = *count as f64 / total as f64;

            // Report very common flows (>50%)
            if *count >= self.config.min_pattern_occurrences && ratio > 0.5 {
                let nodes: Vec<String> = flow.split(" -> ").map(String::from).collect();
                patterns.push(DetectedPattern {
                    description: format!(
                        "Dominant execution flow ({:.1}% of traces): {}",
                        ratio * 100.0,
                        truncate(flow, 60)
                    ),
                    pattern_type: PatternType::ExecutionFlow,
                    occurrences: *count,
                    affected_nodes: nodes,
                    confidence: ratio,
                    suggestion: "This is the happy path - ensure it's optimized".to_string(),
                    evidence: vec![Citation::aggregation(
                        "Execution flow analysis",
                        format!("{} of {} traces follow this flow", count, total),
                    )],
                });
            }

            // Report rare alternative flows (<5%) that might indicate issues
            if *count >= 2 && ratio < 0.05 && *count >= self.config.min_pattern_occurrences {
                patterns.push(DetectedPattern {
                    description: format!(
                        "Rare execution flow ({} occurrences): {}",
                        count,
                        truncate(flow, 50)
                    ),
                    pattern_type: PatternType::ExecutionFlow,
                    occurrences: *count,
                    affected_nodes: flow.split(" -> ").map(String::from).collect(),
                    confidence: 0.6,
                    suggestion:
                        "Investigate why this rare path is taken - may indicate edge case issues"
                            .to_string(),
                    evidence: vec![],
                });
            }
        }

        patterns
    }

    /// Detect resource usage patterns
    fn detect_resource_patterns(&self, traces: &[ExecutionTrace]) -> Vec<DetectedPattern> {
        let mut patterns = Vec::new();

        if traces.len() < self.config.min_pattern_occurrences {
            return patterns;
        }

        // Analyze token usage patterns
        let tokens: Vec<u64> = traces.iter().map(|t| t.total_tokens).collect();
        if !tokens.is_empty() {
            let avg = tokens.iter().sum::<u64>() as f64 / tokens.len() as f64;
            let max = *tokens.iter().max().unwrap_or(&0);

            // Detect high token usage outliers
            let high_usage_count = tokens.iter().filter(|&&t| t as f64 > avg * 2.0).count();
            if high_usage_count >= self.config.min_pattern_occurrences {
                patterns.push(DetectedPattern {
                    description: format!(
                        "High token usage outliers: {} executions with >2x average tokens",
                        high_usage_count
                    ),
                    pattern_type: PatternType::ResourceUsage,
                    occurrences: high_usage_count,
                    affected_nodes: vec![],
                    confidence: 0.7,
                    suggestion: "Investigate executions with unusually high token consumption"
                        .to_string(),
                    evidence: vec![Citation::aggregation(
                        "Token usage analysis",
                        format!("Avg: {:.0} tokens, Max: {} tokens", avg, max),
                    )],
                });
            }
        }

        patterns
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Calculate the 95th percentile of a slice of values
fn percentile_95(values: &[u64]) -> u64 {
    if values.is_empty() {
        return 0;
    }

    let mut sorted = values.to_vec();
    sorted.sort_unstable();

    let index = ((sorted.len() as f64) * 0.95).ceil() as usize;
    sorted[index.min(sorted.len() - 1)]
}

/// Truncate a string to a maximum length (UTF-8 safe)
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        // Find safe character boundary using char_indices to avoid panic on multi-byte UTF-8
        let target_len = max_len.saturating_sub(3);
        let truncate_at = s
            .char_indices()
            .take_while(|(idx, _)| *idx < target_len)
            .last()
            .map(|(idx, c)| idx + c.len_utf8())
            .unwrap_or(0);
        format!("{}...", &s[..truncate_at])
    }
}

/// Normalize a message for pattern matching
fn normalize_for_pattern(message: &str) -> String {
    let mut normalized = message.to_lowercase();

    // Remove UUIDs
    normalized = UUID_PATTERN.replace_all(&normalized, "<uuid>").to_string();

    // Remove numbers
    normalized = NUMBER_PATTERN.replace_all(&normalized, "<n>").to_string();

    // Normalize whitespace
    WHITESPACE_PATTERN
        .replace_all(&normalized, " ")
        .trim()
        .to_string()
}

/// Calculate confidence for a pattern based on occurrences
fn calculate_pattern_confidence(occurrences: usize, total: usize) -> f64 {
    let occurrence_factor = (occurrences as f64 / 10.0).min(1.0);
    let ratio_factor = if total > 0 {
        (occurrences as f64 / total as f64).min(0.5)
    } else {
        0.0
    };
    (0.5 + occurrence_factor * 0.3 + ratio_factor * 0.2).clamp(0.0, 1.0)
}

/// Calculate confidence for performance degradation
fn calculate_degradation_confidence(increase_pct: f64, sample_size: usize) -> f64 {
    let increase_factor = (increase_pct / 100.0).min(1.0);
    let sample_factor = (sample_size as f64 / 50.0).min(1.0);
    (0.4 + increase_factor * 0.4 + sample_factor * 0.2).clamp(0.0, 1.0)
}

/// Extract a tool name from an error message
fn extract_tool_name_from_error(message: &str) -> String {
    // Try common patterns
    let patterns: &[&Regex] = &[
        &*SINGLE_QUOTE_PATTERN,
        &*DOUBLE_QUOTE_PATTERN,
        &*BACKTICK_PATTERN,
    ];

    for pattern in patterns {
        if let Some(captures) = pattern.captures(message) {
            if let Some(name) = captures.get(1) {
                let tool_name = name.as_str();
                // Filter out common non-tool matches
                if !tool_name.contains(' ') && tool_name.len() < 50 {
                    return tool_name.to_string();
                }
            }
        }
    }

    "unknown_tool".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_trace(
        nodes: Vec<(&str, u64, bool)>,
        errors: Vec<(&str, &str)>,
    ) -> ExecutionTrace {
        use crate::introspection::NodeExecution;

        let mut trace = ExecutionTrace::new();

        for (i, (node, duration, success)) in nodes.iter().enumerate() {
            let mut exec = NodeExecution::new(*node, *duration);
            exec.success = *success;
            exec.index = i;
            trace.nodes_executed.push(exec);
        }

        trace.total_duration_ms = nodes.iter().map(|(_, d, _)| d).sum();
        trace.completed = errors.is_empty();

        for (node, message) in errors {
            trace
                .errors
                .push(ErrorTrace::new(node.to_string(), message.to_string()));
        }

        trace
    }

    #[test]
    fn test_capability_gap_analyzer_creation() {
        let analyzer = CapabilityGapAnalyzer::default();
        assert_eq!(analyzer.config.min_error_count, 2);
    }

    #[test]
    fn test_analyze_error_patterns() {
        let analyzer = CapabilityGapAnalyzer::default();

        let traces = vec![
            create_test_trace(
                vec![("node1", 100, false)],
                vec![("node1", "Connection timeout")],
            ),
            create_test_trace(
                vec![("node1", 100, false)],
                vec![("node1", "Connection timeout")],
            ),
            create_test_trace(
                vec![("node1", 100, false)],
                vec![("node1", "Connection timeout to server")],
            ),
        ];

        let gaps = analyzer.analyze(&traces);
        assert!(!gaps.is_empty(), "Should detect recurring error pattern");
    }

    #[test]
    fn test_analyze_retry_patterns() {
        let analyzer = CapabilityGapAnalyzer::default();

        let mut traces = Vec::new();
        for _ in 0..10 {
            let mut trace = create_test_trace(vec![("node1", 100, true)], vec![]);
            trace.errors.push({
                let mut e = ErrorTrace::new("node1", "Temporary failure");
                e.retry_attempted = true;
                e
            });
            traces.push(trace);
        }

        let gaps = analyzer.analyze(&traces);
        let retry_gap = gaps.iter().find(|g| g.description.contains("retry"));
        assert!(retry_gap.is_some(), "Should detect high retry rate");
    }

    #[test]
    fn test_analyze_performance_gaps() {
        let analyzer = CapabilityGapAnalyzer::default();

        // Create traces with a consistently slow node
        let traces: Vec<ExecutionTrace> = (0..5)
            .map(|_| create_test_trace(vec![("slow_node", 2000, true)], vec![]))
            .collect();

        let gaps = analyzer.analyze(&traces);
        let perf_gap = gaps.iter().find(|g| g.description.contains("Performance"));
        assert!(perf_gap.is_some(), "Should detect performance bottleneck");
    }

    #[test]
    fn test_deprecation_analyzer_unused_nodes() {
        let analyzer = DeprecationAnalyzer::default();

        let traces: Vec<ExecutionTrace> = (0..20)
            .map(|_| create_test_trace(vec![("used_node", 100, true)], vec![]))
            .collect();

        let known_nodes = vec!["used_node".to_string(), "unused_node".to_string()];

        let deprecations = analyzer.analyze(&traces, &known_nodes);
        let unused_dep = deprecations.iter().find(
            |d| matches!(&d.target, DeprecationTarget::Node { name, .. } if name == "unused_node"),
        );

        assert!(unused_dep.is_some(), "Should detect unused node");
    }

    #[test]
    fn test_retrospective_analyzer() {
        let analyzer = RetrospectiveAnalyzer::default();

        let traces = vec![
            create_test_trace(vec![("node1", 100, true)], vec![]),
            create_test_trace(
                vec![("node1", 100, false)],
                vec![("node1", "Error occurred")],
            ),
        ];

        let analysis = analyzer.analyze(&traces);
        assert_eq!(analysis.actual_execution.total_executions, 2);
        assert_eq!(analysis.actual_execution.successful_executions, 1);
    }

    #[test]
    fn test_pattern_detector_recurring_errors() {
        let detector = PatternDetector::default();

        let traces: Vec<ExecutionTrace> = (0..5)
            .map(|_| {
                create_test_trace(
                    vec![("node1", 100, false)],
                    vec![("node1", "Same error message")],
                )
            })
            .collect();

        let patterns = detector.detect(&traces);
        let error_pattern = patterns
            .iter()
            .find(|p| p.pattern_type == PatternType::RecurringError);

        assert!(
            error_pattern.is_some(),
            "Should detect recurring error pattern"
        );
        assert!(error_pattern.unwrap().occurrences >= 5);
    }

    #[test]
    fn test_pattern_detector_execution_flow() {
        let detector = PatternDetector::default();

        // Create traces that all follow the same path
        let traces: Vec<ExecutionTrace> = (0..10)
            .map(|_| {
                create_test_trace(
                    vec![
                        ("start", 50, true),
                        ("process", 100, true),
                        ("end", 50, true),
                    ],
                    vec![],
                )
            })
            .collect();

        let patterns = detector.detect(&traces);
        let flow_pattern = patterns
            .iter()
            .find(|p| p.pattern_type == PatternType::ExecutionFlow);

        assert!(
            flow_pattern.is_some(),
            "Should detect dominant execution flow"
        );
    }

    #[test]
    fn test_percentile_95() {
        let values = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 100];
        let p95 = percentile_95(&values);
        assert_eq!(p95, 100);

        let values2 = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        let p95_2 = percentile_95(&values2);
        assert_eq!(p95_2, 100);
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("this is a long string", 10), "this is...");
    }

    #[test]
    fn test_normalize_for_pattern() {
        let msg = "Error at 2025-01-01 with ID 12345678-1234-1234-1234-123456789abc";
        let normalized = normalize_for_pattern(msg);
        assert!(normalized.contains("<uuid>"));
        assert!(normalized.contains("<n>"));
    }

    // =========================================================================
    // Config Types Tests
    // =========================================================================

    #[test]
    fn test_capability_gap_config_defaults() {
        let config = CapabilityGapConfig::default();
        assert_eq!(config.min_error_count, 2);
        assert!(config.min_retry_rate > 0.0);
        assert!(config.min_pattern_occurrences > 0);
        assert!(config.min_confidence > 0.0);
    }

    #[test]
    fn test_capability_gap_config_custom() {
        let config = CapabilityGapConfig {
            min_error_count: 5,
            min_retry_rate: 0.2,
            min_pattern_occurrences: 5,
            min_confidence: 0.7,
        };
        assert_eq!(config.min_error_count, 5);
        assert_eq!(config.min_retry_rate, 0.2);
        assert_eq!(config.min_pattern_occurrences, 5);
        assert_eq!(config.min_confidence, 0.7);
    }

    #[test]
    fn test_deprecation_config_defaults() {
        let config = DeprecationConfig::default();
        assert!(config.min_total_executions > 0);
        assert!(config.usage_threshold > 0.0);
        assert!(config.min_confidence > 0.0);
    }

    #[test]
    fn test_retrospective_config_defaults() {
        let config = RetrospectiveConfig::default();
        assert!(config.min_improvement_threshold > 0.0);
        assert!(config.include_platform_insights);
    }

    #[test]
    fn test_pattern_config_defaults() {
        let config = PatternConfig::default();
        assert!(config.min_pattern_occurrences > 0);
    }

    // =========================================================================
    // PatternType Tests
    // =========================================================================

    #[test]
    fn test_pattern_type_variants() {
        // Verify all variants exist
        let _recurring = PatternType::RecurringError;
        let _flow = PatternType::ExecutionFlow;
        let _perf = PatternType::PerformanceDegradation;
        let _resource = PatternType::ResourceUsage;

        // Test equality
        assert_eq!(PatternType::RecurringError, PatternType::RecurringError);
        assert_ne!(PatternType::RecurringError, PatternType::ExecutionFlow);
    }

    // =========================================================================
    // DetectedPattern Tests
    // =========================================================================

    #[test]
    fn test_detected_pattern_creation() {
        let pattern = DetectedPattern {
            description: "Same error occurred multiple times".to_string(),
            pattern_type: PatternType::RecurringError,
            occurrences: 5,
            affected_nodes: vec!["node1".to_string(), "node2".to_string()],
            confidence: 0.85,
            suggestion: "Fix the root cause".to_string(),
            evidence: vec![],
        };

        assert_eq!(pattern.pattern_type, PatternType::RecurringError);
        assert_eq!(pattern.occurrences, 5);
        assert_eq!(pattern.confidence, 0.85);
        assert_eq!(pattern.affected_nodes.len(), 2);
    }

    #[test]
    fn test_detected_pattern_high_confidence() {
        let pattern = DetectedPattern {
            description: "Node execution time spiked".to_string(),
            pattern_type: PatternType::PerformanceDegradation,
            occurrences: 10,
            affected_nodes: vec![],
            confidence: 0.95,
            suggestion: "Optimize node".to_string(),
            evidence: vec![],
        };

        assert!(pattern.confidence > 0.9);
    }

    // =========================================================================
    // Analyzer Configuration Tests
    // =========================================================================

    #[test]
    fn test_capability_gap_analyzer_with_custom_config() {
        let config = CapabilityGapConfig {
            min_error_count: 10,
            min_retry_rate: 0.3,
            min_pattern_occurrences: 10,
            min_confidence: 0.8,
        };
        let analyzer = CapabilityGapAnalyzer { config };
        assert_eq!(analyzer.config.min_error_count, 10);
    }

    #[test]
    fn test_deprecation_analyzer_with_custom_config() {
        let config = DeprecationConfig {
            min_total_executions: 50,
            usage_threshold: 0.02,
            min_confidence: 0.7,
        };
        let analyzer = DeprecationAnalyzer { config };
        assert_eq!(analyzer.config.min_total_executions, 50);
    }

    #[test]
    fn test_retrospective_analyzer_with_custom_config() {
        let config = RetrospectiveConfig {
            min_improvement_threshold: 0.2,
            include_platform_insights: false,
        };
        let analyzer = RetrospectiveAnalyzer { config };
        assert_eq!(analyzer.config.min_improvement_threshold, 0.2);
    }

    #[test]
    fn test_pattern_detector_with_custom_config() {
        let config = PatternConfig {
            min_pattern_occurrences: 5,
            min_confidence: 0.7,
        };
        let detector = PatternDetector { config };
        assert_eq!(detector.config.min_pattern_occurrences, 5);
        assert_eq!(detector.config.min_confidence, 0.7);
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    #[test]
    fn test_empty_traces_no_gaps() {
        let analyzer = CapabilityGapAnalyzer::default();
        let traces: Vec<ExecutionTrace> = vec![];
        let gaps = analyzer.analyze(&traces);
        assert!(gaps.is_empty());
    }

    #[test]
    fn test_single_trace_insufficient_data() {
        let analyzer = CapabilityGapAnalyzer::default();
        let traces = vec![create_test_trace(vec![("node1", 100, true)], vec![])];
        // Single successful trace should not produce gaps
        let gaps = analyzer.analyze(&traces);
        // May or may not have gaps depending on implementation
        // Just verify it doesn't panic
        let _ = gaps;
    }

    #[test]
    fn test_deprecation_empty_known_nodes() {
        let analyzer = DeprecationAnalyzer::default();
        let traces: Vec<ExecutionTrace> = (0..10)
            .map(|_| create_test_trace(vec![("node1", 100, true)], vec![]))
            .collect();
        let known_nodes: Vec<String> = vec![];
        let deprecations = analyzer.analyze(&traces, &known_nodes);
        assert!(deprecations.is_empty());
    }

    #[test]
    fn test_pattern_detector_empty_traces() {
        let detector = PatternDetector::default();
        let traces: Vec<ExecutionTrace> = vec![];
        let patterns = detector.detect(&traces);
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_retrospective_single_successful_trace() {
        let analyzer = RetrospectiveAnalyzer::default();
        let traces = vec![create_test_trace(vec![("node1", 100, true)], vec![])];
        let analysis = analyzer.analyze(&traces);
        assert_eq!(analysis.actual_execution.total_executions, 1);
        assert_eq!(analysis.actual_execution.successful_executions, 1);
        assert_eq!(analysis.actual_execution.success_rate, 1.0);
    }

    // =========================================================================
    // Percentile Helper Tests
    // =========================================================================

    #[test]
    fn test_percentile_95_single_value() {
        let values = vec![42];
        let p95 = percentile_95(&values);
        assert_eq!(p95, 42);
    }

    #[test]
    fn test_percentile_95_two_values() {
        let values = vec![10, 90];
        let p95 = percentile_95(&values);
        assert_eq!(p95, 90);
    }

    #[test]
    fn test_percentile_95_sorted_input() {
        // Already sorted
        let values = vec![1, 2, 3, 4, 5];
        let p95 = percentile_95(&values);
        assert_eq!(p95, 5);
    }

    #[test]
    fn test_truncate_exact_length() {
        assert_eq!(truncate("exactly", 7), "exactly");
    }

    #[test]
    fn test_truncate_shorter_than_limit() {
        assert_eq!(truncate("hi", 100), "hi");
    }

    #[test]
    fn test_truncate_utf8_safe() {
        // Test with multi-byte UTF-8 characters (each Japanese char is 3 bytes)
        // "こんにちは" = 5 chars, 15 bytes
        let japanese = "こんにちは";
        assert_eq!(japanese.len(), 15); // Verify byte count

        // Truncating at 10 bytes would panic without UTF-8 safe handling
        // because byte 7 falls in the middle of "に" (bytes 6-8)
        let truncated = truncate(japanese, 10);
        // Should safely truncate at character boundary and add "..."
        assert!(truncated.ends_with("..."));
        assert!(truncated.is_char_boundary(truncated.len())); // Valid UTF-8

        // Verify no panic with various byte boundaries
        for max_len in 1..20 {
            let result = truncate(japanese, max_len);
            assert!(result.is_char_boundary(result.len())); // Must be valid UTF-8
        }
    }

    #[test]
    fn test_normalize_for_pattern_no_special() {
        let msg = "Simple error message";
        let normalized = normalize_for_pattern(msg);
        // normalize_for_pattern lowercases the string
        assert_eq!(normalized, "simple error message");
    }

    #[test]
    fn test_normalize_for_pattern_numbers() {
        let msg = "Error code 12345";
        let normalized = normalize_for_pattern(msg);
        assert!(normalized.contains("<n>"));
    }

    // =========================================================================
    // Builder Pattern Tests for CapabilityGapConfig
    // =========================================================================

    #[test]
    fn test_capability_gap_config_new() {
        let config = CapabilityGapConfig::new();
        assert_eq!(config.min_error_count, 2);
        assert_eq!(config.min_retry_rate, 0.1);
        assert_eq!(config.min_pattern_occurrences, 3);
        assert_eq!(config.min_confidence, 0.5);
    }

    #[test]
    fn test_capability_gap_config_builder_min_error_count() {
        let config = CapabilityGapConfig::new().with_min_error_count(5);
        assert_eq!(config.min_error_count, 5);
    }

    #[test]
    fn test_capability_gap_config_builder_min_retry_rate() {
        let config = CapabilityGapConfig::new().with_min_retry_rate(0.25);
        assert_eq!(config.min_retry_rate, 0.25);
    }

    #[test]
    fn test_capability_gap_config_builder_min_pattern_occurrences() {
        let config = CapabilityGapConfig::new().with_min_pattern_occurrences(10);
        assert_eq!(config.min_pattern_occurrences, 10);
    }

    #[test]
    fn test_capability_gap_config_builder_min_confidence() {
        let config = CapabilityGapConfig::new().with_min_confidence(0.75);
        assert_eq!(config.min_confidence, 0.75);
    }

    #[test]
    fn test_capability_gap_config_builder_chaining() {
        let config = CapabilityGapConfig::new()
            .with_min_error_count(10)
            .with_min_retry_rate(0.3)
            .with_min_pattern_occurrences(5)
            .with_min_confidence(0.9);

        assert_eq!(config.min_error_count, 10);
        assert_eq!(config.min_retry_rate, 0.3);
        assert_eq!(config.min_pattern_occurrences, 5);
        assert_eq!(config.min_confidence, 0.9);
    }

    #[test]
    fn test_capability_gap_config_validate_success() {
        let config = CapabilityGapConfig::new();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_capability_gap_config_validate_boundary_values() {
        // Test boundary values (0.0 and 1.0)
        let config_low = CapabilityGapConfig::new()
            .with_min_retry_rate(0.0)
            .with_min_confidence(0.0);
        assert!(config_low.validate().is_ok());

        let config_high = CapabilityGapConfig::new()
            .with_min_retry_rate(1.0)
            .with_min_confidence(1.0);
        assert!(config_high.validate().is_ok());
    }

    #[test]
    #[should_panic(expected = "min_retry_rate must be in range [0.0, 1.0]")]
    fn test_capability_gap_config_builder_retry_rate_too_high() {
        let _ = CapabilityGapConfig::new().with_min_retry_rate(1.5);
    }

    #[test]
    #[should_panic(expected = "min_confidence must be in range [0.0, 1.0]")]
    fn test_capability_gap_config_builder_confidence_too_high() {
        let _ = CapabilityGapConfig::new().with_min_confidence(1.5);
    }

    #[test]
    #[should_panic(expected = "min_retry_rate must be in range [0.0, 1.0]")]
    fn test_capability_gap_config_builder_retry_rate_negative() {
        let _ = CapabilityGapConfig::new().with_min_retry_rate(-0.1);
    }

    #[test]
    #[should_panic(expected = "min_confidence must be in range [0.0, 1.0]")]
    fn test_capability_gap_config_builder_confidence_negative() {
        let _ = CapabilityGapConfig::new().with_min_confidence(-0.1);
    }
}
