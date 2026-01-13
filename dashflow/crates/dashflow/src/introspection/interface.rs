// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # Interactive Introspection Interface
//!
//! This module provides a natural language interface for AI self-introspection,
//! allowing agents to answer questions about their own behavior, decisions,
//! and capabilities.
//!
//! ## Overview
//!
//! The introspection interface enables:
//! - Natural language queries about execution history
//! - "Why did I..." questions answered with causal analysis
//! - "What if..." questions answered with counterfactual analysis
//! - "How can I..." questions answered with optimization suggestions
//! - "Am I..." questions answered with self-assessment
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::introspection_interface::IntrospectionInterface;
//!
//! let interface = IntrospectionInterface::new();
//!
//! // Answer questions about execution
//! println!("{}", interface.ask(&trace, "Why did I call search 3 times?"));
//! println!("{}", interface.ask(&trace, "What if I had used parallel execution?"));
//! println!("{}", interface.ask(&trace, "How can I be faster?"));
//! println!("{}", interface.ask(&trace, "Am I performing well?"));
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ai_explanation::{DecisionExplainer, ExplainerConfig};
use crate::anomaly_detection::{AnomalyDetector, AnomalySeverity};
use crate::causal_analysis::{CausalAnalyzer, Effect};
use crate::counterfactual_analysis::{Alternative, CounterfactualAnalyzer};
use crate::introspection::ExecutionTrace;

/// Type of introspection query
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryType {
    /// "Why did..." questions - causal analysis
    WhyDid,
    /// "What if..." questions - counterfactual analysis
    WhatIf,
    /// "How can I..." questions - optimization suggestions
    HowCanI,
    /// "Am I..." questions - self-assessment
    AmI,
    /// "What is/are..." questions - state/structure queries
    WhatIs,
    /// "When did..." questions - timing queries
    WhenDid,
    /// "Which..." questions - selection queries
    Which,
    /// General query that doesn't fit other categories
    General,
}

impl std::fmt::Display for QueryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryType::WhyDid => write!(f, "causal inquiry"),
            QueryType::WhatIf => write!(f, "counterfactual inquiry"),
            QueryType::HowCanI => write!(f, "optimization inquiry"),
            QueryType::AmI => write!(f, "self-assessment"),
            QueryType::WhatIs => write!(f, "state query"),
            QueryType::WhenDid => write!(f, "timing query"),
            QueryType::Which => write!(f, "selection query"),
            QueryType::General => write!(f, "general query"),
        }
    }
}

/// A parsed introspection query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedQuery {
    /// Original question text
    pub original: String,
    /// Normalized/cleaned question
    pub normalized: String,
    /// Type of query
    pub query_type: QueryType,
    /// Key entities mentioned (node names, tool names, etc.)
    pub entities: Vec<String>,
    /// Keywords extracted from the query
    pub keywords: Vec<String>,
    /// Confidence in query parsing (0.0-1.0)
    pub confidence: f64,
}

impl ParsedQuery {
    /// Create a new parsed query
    #[must_use]
    pub fn new(original: impl Into<String>, query_type: QueryType) -> Self {
        let original = original.into();
        Self {
            normalized: original.to_lowercase(),
            original,
            query_type,
            entities: Vec::new(),
            keywords: Vec::new(),
            confidence: 1.0,
        }
    }

    /// Add entities
    #[must_use]
    pub fn with_entities(mut self, entities: Vec<String>) -> Self {
        self.entities = entities;
        self
    }

    /// Add keywords
    #[must_use]
    pub fn with_keywords(mut self, keywords: Vec<String>) -> Self {
        self.keywords = keywords;
        self
    }

    /// Set confidence
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }
}

/// Response to an introspection query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResponse {
    /// The parsed query
    pub query: ParsedQuery,
    /// Natural language answer
    pub answer: String,
    /// Detailed breakdown (if applicable)
    pub details: Vec<String>,
    /// Confidence in the answer (0.0-1.0)
    pub confidence: f64,
    /// Suggested follow-up questions
    pub follow_ups: Vec<String>,
    /// Additional structured data
    pub data: HashMap<String, serde_json::Value>,
}

impl QueryResponse {
    /// Create a new response
    #[must_use]
    pub fn new(query: ParsedQuery, answer: impl Into<String>) -> Self {
        Self {
            query,
            answer: answer.into(),
            details: Vec::new(),
            confidence: 1.0,
            follow_ups: Vec::new(),
            data: HashMap::new(),
        }
    }

    /// Add details
    #[must_use]
    pub fn with_details(mut self, details: Vec<String>) -> Self {
        self.details = details;
        self
    }

    /// Set confidence
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Add follow-up questions
    #[must_use]
    pub fn with_follow_ups(mut self, follow_ups: Vec<String>) -> Self {
        self.follow_ups = follow_ups;
        self
    }

    /// Add data
    #[must_use]
    pub fn with_data(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.data.insert(key.into(), value);
        self
    }

    /// Format as a complete report
    #[must_use]
    pub fn report(&self) -> String {
        let mut lines = vec![
            format!("Question: {}", self.query.original),
            format!("Type: {}", self.query.query_type),
            String::new(),
            "Answer:".to_string(),
            self.answer.clone(),
        ];

        if !self.details.is_empty() {
            lines.push(String::new());
            lines.push("Details:".to_string());
            for detail in &self.details {
                lines.push(format!("  - {}", detail));
            }
        }

        if !self.follow_ups.is_empty() {
            lines.push(String::new());
            lines.push("You might also ask:".to_string());
            for q in &self.follow_ups {
                lines.push(format!("  - {}", q));
            }
        }

        lines.push(String::new());
        lines.push(format!("Confidence: {:.0}%", self.confidence * 100.0));

        lines.join("\n")
    }
}

/// Configuration for the introspection interface
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceConfig {
    /// Maximum response length
    pub max_response_length: usize,
    /// Include detailed breakdowns
    pub include_details: bool,
    /// Generate follow-up questions
    pub generate_follow_ups: bool,
    /// Include timing information
    pub include_timing: bool,
    /// Include token usage
    pub include_tokens: bool,
    /// Verbosity level (0-3)
    pub verbosity: u8,
}

impl Default for InterfaceConfig {
    fn default() -> Self {
        Self {
            max_response_length: 1000,
            include_details: true,
            generate_follow_ups: true,
            include_timing: true,
            include_tokens: true,
            verbosity: 1,
        }
    }
}

impl InterfaceConfig {
    /// Brief responses
    #[must_use]
    pub fn brief() -> Self {
        Self {
            max_response_length: 200,
            include_details: false,
            generate_follow_ups: false,
            include_timing: false,
            include_tokens: false,
            verbosity: 0,
        }
    }

    /// Detailed responses
    #[must_use]
    pub fn detailed() -> Self {
        Self {
            max_response_length: 5000,
            include_details: true,
            generate_follow_ups: true,
            include_timing: true,
            include_tokens: true,
            verbosity: 3,
        }
    }
}

/// Interactive introspection interface for AI self-inquiry
pub struct IntrospectionInterface {
    config: InterfaceConfig,
    explainer: DecisionExplainer,
    causal_analyzer: CausalAnalyzer,
    counterfactual_analyzer: CounterfactualAnalyzer,
    anomaly_detector: AnomalyDetector,
}

impl Default for IntrospectionInterface {
    fn default() -> Self {
        Self::new()
    }
}

impl IntrospectionInterface {
    /// Create a new interface with default configuration
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: InterfaceConfig::default(),
            explainer: DecisionExplainer::new(),
            causal_analyzer: CausalAnalyzer::new(),
            counterfactual_analyzer: CounterfactualAnalyzer::new(),
            anomaly_detector: AnomalyDetector::new(),
        }
    }

    /// Create with custom configuration
    #[must_use]
    pub fn with_config(config: InterfaceConfig) -> Self {
        let explainer_config = ExplainerConfig {
            include_state_values: config.include_details,
            max_explanation_length: config.max_response_length,
            include_timing: config.include_timing,
            include_tokens: config.include_tokens,
            generate_follow_ups: config.generate_follow_ups,
            verbosity: config.verbosity,
        };

        Self {
            config,
            explainer: DecisionExplainer::with_config(explainer_config),
            causal_analyzer: CausalAnalyzer::new(),
            counterfactual_analyzer: CounterfactualAnalyzer::new(),
            anomaly_detector: AnomalyDetector::new(),
        }
    }

    /// Answer a natural language question about the execution
    #[must_use]
    pub fn ask(&self, trace: &ExecutionTrace, question: &str) -> QueryResponse {
        let parsed = self.parse_query(question, trace);

        match parsed.query_type {
            QueryType::WhyDid => self.answer_why_did(&parsed, trace),
            QueryType::WhatIf => self.answer_what_if(&parsed, trace),
            QueryType::HowCanI => self.answer_how_can_i(&parsed, trace),
            QueryType::AmI => self.answer_am_i(&parsed, trace),
            QueryType::WhatIs => self.answer_what_is(&parsed, trace),
            QueryType::WhenDid => self.answer_when_did(&parsed, trace),
            QueryType::Which => self.answer_which(&parsed, trace),
            QueryType::General => self.answer_general(&parsed, trace),
        }
    }

    /// Parse a natural language query
    #[must_use]
    pub fn parse_query(&self, question: &str, trace: &ExecutionTrace) -> ParsedQuery {
        let lower = question.to_lowercase();
        let normalized = lower.trim();

        // Determine query type
        let query_type = if normalized.starts_with("why did")
            || normalized.starts_with("why was")
            || normalized.starts_with("why")
        {
            QueryType::WhyDid
        } else if normalized.starts_with("what if")
            || normalized.starts_with("what would")
            || normalized.contains("instead")
        {
            QueryType::WhatIf
        } else if normalized.starts_with("how can")
            || normalized.starts_with("how do")
            || normalized.starts_with("how should")
            || normalized.contains("improve")
            || normalized.contains("optimize")
            || normalized.contains("faster")
            || normalized.contains("better")
        {
            QueryType::HowCanI
        } else if normalized.starts_with("am i")
            || normalized.starts_with("was i")
            || normalized.starts_with("did i")
            || normalized.contains("performing")
            || normalized.contains("good")
            || normalized.contains("well")
        {
            QueryType::AmI
        } else if normalized.starts_with("what is")
            || normalized.starts_with("what are")
            || normalized.starts_with("what was")
        {
            QueryType::WhatIs
        } else if normalized.starts_with("when did")
            || normalized.starts_with("when was")
            || normalized.contains("timing")
            || normalized.contains("duration")
        {
            QueryType::WhenDid
        } else if normalized.starts_with("which") {
            QueryType::Which
        } else {
            QueryType::General
        };

        // Extract entities (node names, tool names from trace)
        let mut entities = Vec::new();
        for exec in &trace.nodes_executed {
            if normalized.contains(&exec.node.to_lowercase()) {
                entities.push(exec.node.clone());
            }
            for tool in &exec.tools_called {
                if normalized.contains(&tool.to_lowercase()) {
                    entities.push(tool.clone());
                }
            }
        }

        // Extract keywords
        let keywords = self.extract_keywords(normalized);

        // Calculate confidence based on how well we understood the query
        let confidence = if entities.is_empty() && keywords.len() < 2 {
            0.5
        } else if query_type == QueryType::General {
            0.6
        } else {
            0.9
        };

        ParsedQuery::new(question, query_type)
            .with_entities(entities)
            .with_keywords(keywords)
            .with_confidence(confidence)
    }

    fn extract_keywords(&self, text: &str) -> Vec<String> {
        let stopwords = [
            "the", "a", "an", "is", "are", "was", "were", "did", "do", "does", "i", "you", "it",
            "this", "that", "my", "to", "in", "for", "of", "and", "or", "but", "if", "then", "so",
            "why", "what", "how", "when", "which", "can", "could", "would", "should", "be", "been",
        ];

        text.split_whitespace()
            .filter(|w| !stopwords.contains(w) && w.len() > 2)
            .map(|s| s.to_string())
            .collect()
    }

    fn answer_why_did(&self, query: &ParsedQuery, trace: &ExecutionTrace) -> QueryResponse {
        let mut answer_parts = Vec::new();
        let mut details = Vec::new();

        // If specific entities mentioned, analyze those
        if !query.entities.is_empty() {
            for entity in &query.entities {
                // Check if it's a node
                if trace.nodes_executed.iter().any(|e| &e.node == entity) {
                    let explanation = self.explainer.explain_decision(trace, entity);
                    answer_parts.push(explanation.natural_language);
                    details.extend(explanation.detailed_breakdown);
                }
                // Check if it's a tool
                else if trace
                    .nodes_executed
                    .iter()
                    .any(|e| e.tools_called.contains(entity))
                {
                    let explanation = self.explainer.explain_tool_call(trace, entity);
                    answer_parts.push(explanation.natural_language);
                }
            }
        }

        // General causal analysis if no specific entity or nothing found
        if answer_parts.is_empty() {
            // Detect what effect they might be asking about
            let effect = self.detect_effect_from_query(query, trace);
            let chain = self.causal_analyzer.analyze(trace, effect);

            answer_parts.push(chain.summary.clone());

            for cause in chain.causes_by_contribution().into_iter().take(3) {
                details.push(cause.description());
            }
        }

        let answer = if answer_parts.is_empty() {
            "I couldn't determine the specific cause from the execution trace.".to_string()
        } else {
            answer_parts.join("\n\n")
        };

        let follow_ups = if self.config.generate_follow_ups {
            vec![
                "What if I had done something differently?".to_string(),
                "How can I improve this?".to_string(),
            ]
        } else {
            Vec::new()
        };

        QueryResponse::new(query.clone(), answer)
            .with_details(details)
            .with_confidence(query.confidence)
            .with_follow_ups(follow_ups)
    }

    fn answer_what_if(&self, query: &ParsedQuery, trace: &ExecutionTrace) -> QueryResponse {
        let mut answer_parts = Vec::new();
        let mut details = Vec::new();

        // Try to detect what alternative they're asking about
        let alternative = self.detect_alternative_from_query(query);

        // Run counterfactual analysis on first node (or detected entity)
        let node = query
            .entities
            .first()
            .map(|s| s.as_str())
            .or_else(|| trace.nodes_executed.first().map(|e| e.node.as_str()))
            .unwrap_or("unknown");

        let result = self
            .counterfactual_analyzer
            .analyze(trace, node, alternative);

        answer_parts.push(format!(
            "If {} had been different in '{}', here's what might have happened:",
            result.alternative, node
        ));

        // Describe improvements
        let improvement = &result.estimated_improvement;
        if improvement.latency_ms != 0 {
            let change = if improvement.latency_ms > 0 {
                format!("{}ms faster", improvement.latency_ms)
            } else {
                format!("{}ms slower", -improvement.latency_ms)
            };
            details.push(format!("Latency: {}", change));
        }
        if improvement.tokens != 0 {
            let change = if improvement.tokens > 0 {
                format!("{} fewer tokens", improvement.tokens)
            } else {
                format!("{} more tokens", -improvement.tokens)
            };
            details.push(format!("Token usage: {}", change));
        }

        answer_parts.push(result.reasoning.clone());

        let answer = answer_parts.join("\n\n");

        let follow_ups = if self.config.generate_follow_ups {
            vec![
                "Should I implement this change?".to_string(),
                "What are the trade-offs?".to_string(),
            ]
        } else {
            Vec::new()
        };

        QueryResponse::new(query.clone(), answer)
            .with_details(details)
            .with_confidence(result.confidence)
            .with_follow_ups(follow_ups)
    }

    fn answer_how_can_i(&self, query: &ParsedQuery, trace: &ExecutionTrace) -> QueryResponse {
        let mut suggestions = Vec::new();
        let mut details = Vec::new();

        // Check for anomalies that suggest improvements
        let anomalies = self.anomaly_detector.detect(trace);
        for anomaly in anomalies
            .iter()
            .filter(|a| a.severity != AnomalySeverity::Info)
        {
            suggestions.push(format!(
                "Address {}: {}",
                anomaly.metric, anomaly.explanation
            ));
        }

        // Run causal analysis to find bottlenecks
        let latency_chain = self.causal_analyzer.analyze(trace, Effect::HighLatency);
        for cause in latency_chain.significant_causes(0.2) {
            if let Some(ref remediation) = cause.remediation {
                suggestions.push(remediation.clone());
                details.push(cause.description());
            }
        }

        // Add general suggestions based on trace analysis
        if trace.total_tokens > 5000 {
            suggestions.push("Consider reducing token usage by shortening prompts".to_string());
        }

        // Check for repeated nodes (potential caching opportunity)
        let mut node_counts: HashMap<&str, usize> = HashMap::new();
        for exec in &trace.nodes_executed {
            *node_counts.entry(&exec.node).or_insert(0) += 1;
        }
        for (node, count) in &node_counts {
            if *count > 2 {
                suggestions.push(format!(
                    "Consider caching results for '{}' (executed {} times)",
                    node, count
                ));
            }
        }

        let answer = if suggestions.is_empty() {
            "The execution looks optimized. I don't have specific improvement suggestions."
                .to_string()
        } else {
            let intro = "Here are suggestions to improve:\n";
            let list: Vec<_> = suggestions
                .iter()
                .take(5)
                .enumerate()
                .map(|(i, s)| format!("{}. {}", i + 1, s))
                .collect();
            format!("{}{}", intro, list.join("\n"))
        };

        let follow_ups = if self.config.generate_follow_ups {
            vec![
                "What caused the current performance?".to_string(),
                "Which improvement would have the biggest impact?".to_string(),
            ]
        } else {
            Vec::new()
        };

        QueryResponse::new(query.clone(), answer)
            .with_details(details)
            .with_confidence(0.8)
            .with_follow_ups(follow_ups)
    }

    fn answer_am_i(&self, query: &ParsedQuery, trace: &ExecutionTrace) -> QueryResponse {
        let mut assessments = Vec::new();
        let mut details = Vec::new();
        let mut overall_score = 1.0f64;

        // Completion status
        if trace.completed {
            assessments.push("Execution completed successfully.".to_string());
        } else {
            assessments.push("Execution did not complete successfully.".to_string());
            overall_score -= 0.3;
        }

        // Error rate
        if trace.errors.is_empty() {
            assessments.push("No errors occurred.".to_string());
        } else {
            assessments.push(format!("{} error(s) occurred.", trace.errors.len()));
            overall_score -= 0.1 * trace.errors.len().min(5) as f64;
            for error in trace.errors.iter().take(3) {
                details.push(format!("Error in '{}': {}", error.node, error.message));
            }
        }

        // Performance assessment
        let anomalies = self.anomaly_detector.detect(trace);
        let critical_anomalies = anomalies
            .iter()
            .filter(|a| a.severity == AnomalySeverity::Critical)
            .count();
        let warning_anomalies = anomalies
            .iter()
            .filter(|a| a.severity == AnomalySeverity::Warning)
            .count();

        if critical_anomalies > 0 {
            assessments.push(format!(
                "{} critical performance issue(s) detected.",
                critical_anomalies
            ));
            overall_score -= 0.2 * critical_anomalies as f64;
        }
        if warning_anomalies > 0 {
            assessments.push(format!("{} performance warning(s).", warning_anomalies));
            overall_score -= 0.1 * warning_anomalies as f64;
        }
        if critical_anomalies == 0 && warning_anomalies == 0 {
            assessments.push("Performance looks normal.".to_string());
        }

        // Resource usage
        if self.config.include_tokens && trace.total_tokens > 0 {
            details.push(format!("Total tokens used: {}", trace.total_tokens));
        }
        if self.config.include_timing {
            details.push(format!(
                "Total execution time: {}ms",
                trace.total_duration_ms
            ));
        }

        overall_score = overall_score.clamp(0.0, 1.0);

        let performance_rating = if overall_score >= 0.9 {
            "Excellent"
        } else if overall_score >= 0.7 {
            "Good"
        } else if overall_score >= 0.5 {
            "Fair"
        } else {
            "Needs improvement"
        };

        let answer = format!(
            "Overall assessment: {} ({:.0}%)\n\n{}",
            performance_rating,
            overall_score * 100.0,
            assessments.join("\n")
        );

        let follow_ups = if self.config.generate_follow_ups {
            vec![
                "How can I improve my performance?".to_string(),
                "Why did specific issues occur?".to_string(),
            ]
        } else {
            Vec::new()
        };

        QueryResponse::new(query.clone(), answer)
            .with_details(details)
            .with_confidence(0.85)
            .with_follow_ups(follow_ups)
            .with_data("score".to_string(), serde_json::json!(overall_score))
    }

    fn answer_what_is(&self, query: &ParsedQuery, trace: &ExecutionTrace) -> QueryResponse {
        let mut answer_parts = Vec::new();
        let mut details = Vec::new();

        // Provide information about mentioned entities
        for entity in &query.entities {
            // Is it a node?
            let node_execs: Vec<_> = trace
                .nodes_executed
                .iter()
                .filter(|e| &e.node == entity)
                .collect();

            if !node_execs.is_empty() {
                let total_time: u64 = node_execs.iter().map(|e| e.duration_ms).sum();
                let total_tokens: u64 = node_execs.iter().map(|e| e.tokens_used).sum();

                answer_parts.push(format!(
                    "'{}' is a node that was executed {} time(s), taking {}ms total.",
                    entity,
                    node_execs.len(),
                    total_time
                ));

                if total_tokens > 0 {
                    details.push(format!("Token usage: {}", total_tokens));
                }
                for exec in node_execs.iter().take(3) {
                    if !exec.tools_called.is_empty() {
                        details.push(format!("Tools called: {}", exec.tools_called.join(", ")));
                    }
                }
            }

            // Is it a tool?
            for exec in &trace.nodes_executed {
                if exec.tools_called.contains(entity) {
                    answer_parts.push(format!(
                        "'{}' is a tool that was called in node '{}'.",
                        entity, exec.node
                    ));
                }
            }
        }

        // If no specific entities, provide general execution info
        if answer_parts.is_empty() {
            let path: Vec<_> = trace
                .nodes_executed
                .iter()
                .map(|e| e.node.as_str())
                .collect();
            answer_parts.push(format!(
                "This execution visited {} nodes: {}",
                trace.nodes_executed.len(),
                path.join(" → ")
            ));

            if trace.completed {
                details.push("Status: Completed successfully".to_string());
            } else {
                details.push("Status: Did not complete".to_string());
            }

            if self.config.include_timing {
                details.push(format!("Total duration: {}ms", trace.total_duration_ms));
            }
            if self.config.include_tokens && trace.total_tokens > 0 {
                details.push(format!("Total tokens: {}", trace.total_tokens));
            }
        }

        let answer = answer_parts.join("\n\n");

        QueryResponse::new(query.clone(), answer)
            .with_details(details)
            .with_confidence(0.9)
    }

    fn answer_when_did(&self, query: &ParsedQuery, trace: &ExecutionTrace) -> QueryResponse {
        let mut answer_parts = Vec::new();
        let mut details = Vec::new();

        // Provide timing info for mentioned entities
        for entity in &query.entities {
            let node_execs: Vec<_> = trace
                .nodes_executed
                .iter()
                .enumerate()
                .filter(|(_, e)| &e.node == entity)
                .collect();

            for (idx, exec) in &node_execs {
                answer_parts.push(format!(
                    "'{}' was executed at step {} (index {}).",
                    entity,
                    idx + 1,
                    idx
                ));
                if let Some(ref started) = exec.started_at {
                    details.push(format!("Started at: {}", started));
                }
                details.push(format!("Duration: {}ms", exec.duration_ms));
            }
        }

        // General timing info
        if answer_parts.is_empty() {
            answer_parts.push(format!(
                "The execution took {}ms total across {} nodes.",
                trace.total_duration_ms,
                trace.nodes_executed.len()
            ));

            // Show node timing breakdown
            for exec in trace.nodes_executed.iter().take(5) {
                details.push(format!("'{}': {}ms", exec.node, exec.duration_ms));
            }
        }

        let answer = answer_parts.join("\n");

        QueryResponse::new(query.clone(), answer)
            .with_details(details)
            .with_confidence(0.9)
    }

    fn answer_which(&self, query: &ParsedQuery, trace: &ExecutionTrace) -> QueryResponse {
        let normalized = &query.normalized;
        let mut answer_parts = Vec::new();
        let mut details = Vec::new();

        // "Which node was slowest/fastest"
        if normalized.contains("slow") || normalized.contains("fast") {
            let mut nodes_by_time: Vec<_> = trace
                .nodes_executed
                .iter()
                .map(|e| (&e.node, e.duration_ms))
                .collect();
            nodes_by_time.sort_by_key(|(_, t)| *t);

            if normalized.contains("slow") {
                nodes_by_time.reverse();
                if let Some((node, time)) = nodes_by_time.first() {
                    answer_parts.push(format!("The slowest node was '{}' at {}ms.", node, time));
                }
            } else if let Some((node, time)) = nodes_by_time.first() {
                answer_parts.push(format!("The fastest node was '{}' at {}ms.", node, time));
            }

            // Show ranking
            for (i, (node, time)) in nodes_by_time.iter().take(5).enumerate() {
                details.push(format!("{}. '{}': {}ms", i + 1, node, time));
            }
        }
        // "Which node used most tokens"
        else if normalized.contains("token") {
            let mut nodes_by_tokens: Vec<_> = trace
                .nodes_executed
                .iter()
                .map(|e| (&e.node, e.tokens_used))
                .collect();
            nodes_by_tokens.sort_by_key(|(_, t)| std::cmp::Reverse(*t));

            if let Some((node, tokens)) = nodes_by_tokens.first() {
                answer_parts.push(format!(
                    "The node using most tokens was '{}' with {} tokens.",
                    node, tokens
                ));
            }

            for (i, (node, tokens)) in nodes_by_tokens.iter().take(5).enumerate() {
                details.push(format!("{}. '{}': {} tokens", i + 1, node, tokens));
            }
        }
        // "Which tools were called"
        else if normalized.contains("tool") {
            let mut all_tools: Vec<_> = trace
                .nodes_executed
                .iter()
                .flat_map(|e| e.tools_called.iter().cloned())
                .collect();
            all_tools.sort();
            all_tools.dedup();

            if all_tools.is_empty() {
                answer_parts.push("No tools were called during this execution.".to_string());
            } else {
                answer_parts.push(format!(
                    "{} tool(s) were called: {}",
                    all_tools.len(),
                    all_tools.join(", ")
                ));
            }
        }
        // "Which node failed"
        else if normalized.contains("fail") || normalized.contains("error") {
            let failed: Vec<_> = trace.nodes_executed.iter().filter(|e| !e.success).collect();

            if failed.is_empty() {
                answer_parts.push("No nodes failed during this execution.".to_string());
            } else {
                for exec in &failed {
                    answer_parts.push(format!("'{}' failed.", exec.node));
                    if let Some(ref msg) = exec.error_message {
                        details.push(format!("Error: {}", msg));
                    }
                }
            }
        }
        // General fallback
        else {
            let nodes: Vec<_> = trace
                .nodes_executed
                .iter()
                .map(|e| e.node.as_str())
                .collect();
            answer_parts.push(format!(
                "The following nodes were executed: {}",
                nodes.join(", ")
            ));
        }

        let answer = answer_parts.join("\n");

        QueryResponse::new(query.clone(), answer)
            .with_details(details)
            .with_confidence(0.85)
    }

    fn answer_general(&self, query: &ParsedQuery, trace: &ExecutionTrace) -> QueryResponse {
        // For general queries, provide a comprehensive summary
        let summary = self.explainer.explain_execution(trace);

        let follow_ups = if self.config.generate_follow_ups {
            vec![
                "Why did specific decisions happen?".to_string(),
                "How can I improve performance?".to_string(),
                "Am I performing well?".to_string(),
            ]
        } else {
            Vec::new()
        };

        QueryResponse::new(query.clone(), summary)
            .with_confidence(0.7)
            .with_follow_ups(follow_ups)
    }

    fn detect_effect_from_query(&self, query: &ParsedQuery, _trace: &ExecutionTrace) -> Effect {
        let normalized = &query.normalized;

        if normalized.contains("slow") || normalized.contains("long") || normalized.contains("time")
        {
            Effect::HighLatency
        } else if normalized.contains("fail") || normalized.contains("error") {
            Effect::ExecutionFailure
        } else if normalized.contains("token") {
            Effect::HighTokenUsage
        } else if normalized.contains("retry") || normalized.contains("again") {
            Effect::HighRetryRate
        } else if normalized.contains("loop") {
            Effect::InfiniteLoop
        } else if !query.entities.is_empty() {
            // If specific node mentioned, analyze that node
            Effect::SlowNode(query.entities[0].clone())
        } else {
            Effect::HighLatency // Default
        }
    }

    fn detect_alternative_from_query(&self, query: &ParsedQuery) -> Alternative {
        let normalized = &query.normalized;

        if normalized.contains("parallel") {
            Alternative::ParallelWith("other_node".to_string())
        } else if normalized.contains("cache") || normalized.contains("cached") {
            Alternative::CacheResult
        } else if normalized.contains("skip") {
            Alternative::SkipNode
        } else if normalized.contains("different model") || normalized.contains("faster model") {
            Alternative::UseModel("gpt-3.5-turbo".to_string())
        } else if normalized.contains("timeout") || normalized.contains("shorter") {
            Alternative::UseTimeout(5000)
        } else {
            Alternative::SkipNode // Default
        }
    }

    /// Get suggested questions based on the trace
    #[must_use]
    pub fn suggest_questions(&self, trace: &ExecutionTrace) -> Vec<String> {
        let mut questions = Vec::new();

        // Basic questions
        questions.push("What happened during this execution?".to_string());

        // Based on node count
        if trace.nodes_executed.len() > 3 {
            questions.push(format!(
                "Why were {} nodes executed?",
                trace.nodes_executed.len()
            ));
        }

        // Based on errors
        if !trace.errors.is_empty() {
            questions.push("Why did errors occur?".to_string());
        }

        // Based on tools
        let total_tools: usize = trace
            .nodes_executed
            .iter()
            .map(|e| e.tools_called.len())
            .sum();
        if total_tools > 0 {
            questions.push(format!("Why were {} tools called?", total_tools));
        }

        // Performance questions
        if trace.total_duration_ms > 5000 {
            questions.push("Why was the execution slow?".to_string());
            questions.push("How can I make this faster?".to_string());
        }

        // Token questions
        if trace.total_tokens > 3000 {
            questions.push("Why were so many tokens used?".to_string());
        }

        // Self-assessment
        questions.push("Am I performing well?".to_string());
        questions.push("What if I had used caching?".to_string());

        questions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_prelude::{create_error_test_trace, create_standard_test_trace};

    #[test]
    fn test_query_type_display() {
        assert_eq!(QueryType::WhyDid.to_string(), "causal inquiry");
        assert_eq!(QueryType::WhatIf.to_string(), "counterfactual inquiry");
        assert_eq!(QueryType::HowCanI.to_string(), "optimization inquiry");
        assert_eq!(QueryType::AmI.to_string(), "self-assessment");
    }

    #[test]
    fn test_parsed_query_creation() {
        let query = ParsedQuery::new("Why did I fail?", QueryType::WhyDid)
            .with_entities(vec!["node1".to_string()])
            .with_confidence(0.8);

        assert_eq!(query.original, "Why did I fail?");
        assert_eq!(query.query_type, QueryType::WhyDid);
        assert_eq!(query.entities.len(), 1);
        assert_eq!(query.confidence, 0.8);
    }

    #[test]
    fn test_query_response_creation() {
        let query = ParsedQuery::new("Test", QueryType::General);
        let response = QueryResponse::new(query, "Test answer")
            .with_details(vec!["Detail 1".to_string()])
            .with_confidence(0.9);

        assert_eq!(response.answer, "Test answer");
        assert_eq!(response.details.len(), 1);
        assert_eq!(response.confidence, 0.9);
    }

    #[test]
    fn test_query_response_report() {
        let query = ParsedQuery::new("Why did I fail?", QueryType::WhyDid);
        let response = QueryResponse::new(query, "Because of X")
            .with_details(vec!["Detail".to_string()])
            .with_follow_ups(vec!["How to fix?".to_string()]);

        let report = response.report();
        assert!(report.contains("Why did I fail?"));
        assert!(report.contains("Because of X"));
        assert!(report.contains("Detail"));
        assert!(report.contains("How to fix?"));
    }

    #[test]
    fn test_config_presets() {
        let brief = InterfaceConfig::brief();
        assert!(!brief.include_details);
        assert_eq!(brief.verbosity, 0);

        let detailed = InterfaceConfig::detailed();
        assert!(detailed.include_details);
        assert_eq!(detailed.verbosity, 3);
    }

    #[test]
    fn test_interface_creation() {
        let interface = IntrospectionInterface::new();
        assert!(interface.config.include_details);

        let custom = IntrospectionInterface::with_config(InterfaceConfig::brief());
        assert!(!custom.config.include_details);
    }

    #[test]
    fn test_parse_why_query() {
        let trace = create_standard_test_trace();
        let interface = IntrospectionInterface::new();

        let parsed = interface.parse_query("Why did I call search?", &trace);
        assert_eq!(parsed.query_type, QueryType::WhyDid);
        assert!(parsed.entities.contains(&"search".to_string()));
    }

    #[test]
    fn test_parse_what_if_query() {
        let trace = create_standard_test_trace();
        let interface = IntrospectionInterface::new();

        let parsed = interface.parse_query("What if I had used parallel execution?", &trace);
        assert_eq!(parsed.query_type, QueryType::WhatIf);
    }

    #[test]
    fn test_parse_how_can_i_query() {
        let trace = create_standard_test_trace();
        let interface = IntrospectionInterface::new();

        let parsed = interface.parse_query("How can I be faster?", &trace);
        assert_eq!(parsed.query_type, QueryType::HowCanI);
    }

    #[test]
    fn test_parse_am_i_query() {
        let trace = create_standard_test_trace();
        let interface = IntrospectionInterface::new();

        let parsed = interface.parse_query("Am I performing well?", &trace);
        assert_eq!(parsed.query_type, QueryType::AmI);
    }

    #[test]
    fn test_parse_what_is_query() {
        let trace = create_standard_test_trace();
        let interface = IntrospectionInterface::new();

        let parsed = interface.parse_query("What is reasoning?", &trace);
        assert_eq!(parsed.query_type, QueryType::WhatIs);
        assert!(parsed.entities.contains(&"reasoning".to_string()));
    }

    #[test]
    fn test_parse_when_did_query() {
        let trace = create_standard_test_trace();
        let interface = IntrospectionInterface::new();

        let parsed = interface.parse_query("When did tool_call execute?", &trace);
        assert_eq!(parsed.query_type, QueryType::WhenDid);
    }

    #[test]
    fn test_parse_which_query() {
        let trace = create_standard_test_trace();
        let interface = IntrospectionInterface::new();

        let parsed = interface.parse_query("Which node was slowest?", &trace);
        assert_eq!(parsed.query_type, QueryType::Which);
    }

    #[test]
    fn test_ask_why_did() {
        let trace = create_standard_test_trace();
        let interface = IntrospectionInterface::new();

        let response = interface.ask(&trace, "Why did I call search?");
        assert_eq!(response.query.query_type, QueryType::WhyDid);
        assert!(!response.answer.is_empty());
    }

    #[test]
    fn test_ask_what_if() {
        let trace = create_standard_test_trace();
        let interface = IntrospectionInterface::new();

        let response = interface.ask(&trace, "What if I had used caching?");
        assert_eq!(response.query.query_type, QueryType::WhatIf);
        assert!(!response.answer.is_empty());
    }

    #[test]
    fn test_ask_how_can_i() {
        let trace = create_standard_test_trace();
        let interface = IntrospectionInterface::new();

        let response = interface.ask(&trace, "How can I be faster?");
        assert_eq!(response.query.query_type, QueryType::HowCanI);
        assert!(!response.answer.is_empty());
    }

    #[test]
    fn test_ask_am_i() {
        let trace = create_standard_test_trace();
        let interface = IntrospectionInterface::new();

        let response = interface.ask(&trace, "Am I performing well?");
        assert_eq!(response.query.query_type, QueryType::AmI);
        assert!(response.answer.contains("assessment"));
        assert!(response.data.contains_key("score"));
    }

    #[test]
    fn test_ask_am_i_with_errors() {
        let trace = create_error_test_trace();
        let interface = IntrospectionInterface::new();

        let response = interface.ask(&trace, "Am I performing well?");
        assert!(response.answer.contains("error") || response.answer.contains("not complete"));
    }

    #[test]
    fn test_ask_what_is() {
        let trace = create_standard_test_trace();
        let interface = IntrospectionInterface::new();

        let response = interface.ask(&trace, "What is reasoning?");
        assert!(response.answer.contains("reasoning"));
    }

    #[test]
    fn test_ask_when_did() {
        let trace = create_standard_test_trace();
        let interface = IntrospectionInterface::new();

        let response = interface.ask(&trace, "When did the nodes execute?");
        assert!(response.answer.contains("ms"));
    }

    #[test]
    fn test_ask_which_slowest() {
        let trace = create_standard_test_trace();
        let interface = IntrospectionInterface::new();

        let response = interface.ask(&trace, "Which node was slowest?");
        assert!(response.answer.contains("slowest"));
        assert!(response.answer.contains("tool_call")); // Slowest at 1000ms
    }

    #[test]
    fn test_ask_which_tools() {
        let trace = create_standard_test_trace();
        let interface = IntrospectionInterface::new();

        let response = interface.ask(&trace, "Which tools were called?");
        assert!(response.answer.contains("search"));
    }

    #[test]
    fn test_ask_which_failed() {
        let trace = create_standard_test_trace();
        let interface = IntrospectionInterface::new();

        let response = interface.ask(&trace, "Which nodes failed?");
        assert!(response.answer.contains("No nodes failed"));
    }

    #[test]
    fn test_ask_which_failed_with_errors() {
        let trace = create_error_test_trace();
        let interface = IntrospectionInterface::new();

        let response = interface.ask(&trace, "Which nodes failed?");
        assert!(response.answer.contains("processing"));
    }

    #[test]
    fn test_ask_general() {
        let trace = create_standard_test_trace();
        let interface = IntrospectionInterface::new();

        let response = interface.ask(&trace, "Tell me about this execution");
        assert_eq!(response.query.query_type, QueryType::General);
        assert!(!response.answer.is_empty());
    }

    #[test]
    fn test_suggest_questions() {
        let trace = create_standard_test_trace();
        let interface = IntrospectionInterface::new();

        let questions = interface.suggest_questions(&trace);
        assert!(!questions.is_empty());
        assert!(questions.iter().any(|q| q.contains("executed")));
    }

    #[test]
    fn test_suggest_questions_with_errors() {
        let trace = create_error_test_trace();
        let interface = IntrospectionInterface::new();

        let questions = interface.suggest_questions(&trace);
        assert!(questions.iter().any(|q| q.contains("error")));
    }

    #[test]
    fn test_extract_keywords() {
        let interface = IntrospectionInterface::new();
        let keywords = interface.extract_keywords("why did the slow node take so long");

        assert!(keywords.contains(&"slow".to_string()));
        assert!(keywords.contains(&"node".to_string()));
        assert!(keywords.contains(&"long".to_string()));
        assert!(!keywords.contains(&"the".to_string())); // stopword
    }

    #[test]
    fn test_detect_effect_from_query() {
        let interface = IntrospectionInterface::new();
        let trace = create_standard_test_trace();

        let query = ParsedQuery::new("why was it slow", QueryType::WhyDid);
        let effect = interface.detect_effect_from_query(&query, &trace);
        assert!(matches!(effect, Effect::HighLatency));

        let query = ParsedQuery::new("why did it fail", QueryType::WhyDid);
        let effect = interface.detect_effect_from_query(&query, &trace);
        assert!(matches!(effect, Effect::ExecutionFailure));
    }

    #[test]
    fn test_detect_alternative_from_query() {
        let interface = IntrospectionInterface::new();

        let query = ParsedQuery::new("what if parallel", QueryType::WhatIf)
            .with_keywords(vec!["parallel".to_string()]);
        let alt = interface.detect_alternative_from_query(&query);
        assert!(matches!(alt, Alternative::ParallelWith(_)));

        let query = ParsedQuery::new("what if cached", QueryType::WhatIf)
            .with_keywords(vec!["cached".to_string()]);
        let alt = interface.detect_alternative_from_query(&query);
        assert!(matches!(alt, Alternative::CacheResult));
    }

    #[test]
    fn test_brief_config_responses() {
        let trace = create_standard_test_trace();
        let interface = IntrospectionInterface::with_config(InterfaceConfig::brief());

        let response = interface.ask(&trace, "Am I performing well?");
        assert!(response.follow_ups.is_empty()); // Brief config disables follow-ups
    }

    #[test]
    fn test_query_serialization() {
        let query = ParsedQuery::new("Test", QueryType::WhyDid);
        let json = serde_json::to_string(&query).unwrap();
        let parsed: ParsedQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.original, "Test");
    }

    #[test]
    fn test_response_serialization() {
        let query = ParsedQuery::new("Test", QueryType::General);
        let response =
            QueryResponse::new(query, "Answer").with_data("key".to_string(), serde_json::json!(42));

        let json = serde_json::to_string(&response).unwrap();
        let parsed: QueryResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.answer, "Answer");
        assert_eq!(parsed.data.get("key").unwrap(), &serde_json::json!(42));
    }
}
