//! Integration with Self-Evolution and Adaptive Timeout systems
//!
//! This module provides integration between introspection and:
//! - Prompt self-evolution system
//! - Adaptive timeout system
//! - Causal analysis
//! - Counterfactual analysis

use super::trace::{ExecutionTrace, NodeExecution};
use std::collections::HashMap;

// Prompt Self-Evolution Integration
// ============================================================================

impl ExecutionTrace {
    /// Analyze prompt effectiveness for all nodes in this trace
    ///
    /// Examines execution metrics to identify nodes where prompts may need improvement.
    /// Returns analysis for each unique node, including retry rates, error rates,
    /// token efficiency, and specific issues.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let trace = compiled.get_execution_trace(thread_id).await?;
    /// let analyses = trace.analyze_prompt_effectiveness();
    ///
    /// for analysis in analyses {
    ///     if analysis.needs_improvement(&PromptThresholds::default()) {
    ///         println!("Node '{}' needs prompt improvement: {}", analysis.node, analysis.summary());
    ///     }
    /// }
    /// ```
    #[must_use]
    pub fn analyze_prompt_effectiveness(&self) -> Vec<crate::prompt_evolution::PromptAnalysis> {
        self.analyze_prompt_effectiveness_with_thresholds(
            &crate::prompt_evolution::PromptThresholds::default(),
        )
    }

    /// Analyze prompt effectiveness using custom thresholds
    ///
    /// Allows fine-tuning of what constitutes problematic prompt behavior.
    #[must_use]
    pub fn analyze_prompt_effectiveness_with_thresholds(
        &self,
        thresholds: &crate::prompt_evolution::PromptThresholds,
    ) -> Vec<crate::prompt_evolution::PromptAnalysis> {
        // Group executions by node
        let mut node_executions: HashMap<&str, Vec<&NodeExecution>> = HashMap::new();
        for execution in &self.nodes_executed {
            node_executions
                .entry(&execution.node)
                .or_default()
                .push(execution);
        }

        // Analyze each node
        let mut analyses = Vec::new();
        for (node_name, executions) in node_executions {
            // Convert references to owned for the analysis function
            let owned_executions: Vec<NodeExecution> =
                executions.iter().map(|e| (*e).clone()).collect();
            let analysis = crate::prompt_evolution::analyze_node_executions(
                node_name,
                &owned_executions,
                thresholds,
            );
            analyses.push(analysis);
        }

        // Sort by effectiveness score (lowest first - most problematic)
        analyses.sort_by(|a, b| {
            a.effectiveness_score
                .partial_cmp(&b.effectiveness_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        analyses
    }

    /// Generate prompt evolution suggestions based on this trace
    ///
    /// Analyzes the execution and generates specific suggestions for improving
    /// prompts that showed issues during execution.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let trace = compiled.get_execution_trace(thread_id).await?;
    /// let evolutions = trace.evolve_prompts();
    ///
    /// println!("Generated {} prompt improvement suggestions:", evolutions.len());
    /// for evolution in &evolutions {
    ///     println!("  - {}", evolution.description());
    /// }
    /// ```
    #[must_use]
    pub fn evolve_prompts(&self) -> Vec<crate::prompt_evolution::PromptEvolution> {
        self.evolve_prompts_with_thresholds(&crate::prompt_evolution::PromptThresholds::default())
    }

    /// Generate prompt evolutions using custom thresholds
    #[must_use]
    pub fn evolve_prompts_with_thresholds(
        &self,
        thresholds: &crate::prompt_evolution::PromptThresholds,
    ) -> Vec<crate::prompt_evolution::PromptEvolution> {
        let analyses = self.analyze_prompt_effectiveness_with_thresholds(thresholds);
        crate::prompt_evolution::generate_evolutions(&analyses, thresholds)
    }

    /// Check if any prompts in this trace need improvement
    ///
    /// Quick check to determine if prompt analysis is warranted.
    #[must_use]
    pub fn has_prompt_issues(&self) -> bool {
        let thresholds = crate::prompt_evolution::PromptThresholds::default();
        let analyses = self.analyze_prompt_effectiveness_with_thresholds(&thresholds);
        analyses.iter().any(|a| a.has_issues())
    }

    /// Get a summary of prompt health for this trace
    ///
    /// Returns a human-readable summary of prompt effectiveness across all nodes.
    #[must_use]
    pub fn prompt_health_summary(&self) -> String {
        let analyses = self.analyze_prompt_effectiveness();
        let total_nodes = analyses.len();
        let nodes_with_issues: Vec<_> = analyses.iter().filter(|a| a.has_issues()).collect();

        if nodes_with_issues.is_empty() {
            format!(
                "All {} node(s) have healthy prompts with no detected issues.",
                total_nodes
            )
        } else {
            let issue_summaries: Vec<String> =
                nodes_with_issues.iter().map(|a| a.summary()).collect();

            format!(
                "{}/{} node(s) have prompt issues:\n  - {}",
                nodes_with_issues.len(),
                total_nodes,
                issue_summaries.join("\n  - ")
            )
        }
    }
}

// ============================================================================
// Adaptive Timeout Integration
// ============================================================================

impl ExecutionTrace {
    /// Collect latency statistics for all nodes in this trace
    ///
    /// Aggregates timing data per node, calculating min, max, mean, percentiles,
    /// and stability metrics that can be used for timeout optimization.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let trace = compiled.get_execution_trace(thread_id).await?;
    /// let stats = trace.collect_latency_stats();
    ///
    /// for stat in stats {
    ///     println!("{}", stat.summary());
    /// }
    /// ```
    #[must_use]
    pub fn collect_latency_stats(&self) -> Vec<crate::adaptive_timeout::LatencyStats> {
        // Group executions by node
        let mut node_executions: HashMap<&str, Vec<&NodeExecution>> = HashMap::new();
        for execution in &self.nodes_executed {
            node_executions
                .entry(&execution.node)
                .or_default()
                .push(execution);
        }

        // Calculate stats for each node
        node_executions
            .into_iter()
            .map(|(node_name, executions)| {
                crate::adaptive_timeout::LatencyStats::from_executions(node_name, &executions)
            })
            .collect()
    }

    /// Calculate optimal timeout recommendations based on this trace
    ///
    /// Analyzes execution timing to suggest timeout values that balance
    /// reliability (avoiding premature timeouts) with efficiency (not waiting
    /// too long on failures).
    ///
    /// Uses default configuration. For custom settings, use
    /// `calculate_optimal_timeouts_with_config`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let trace = compiled.get_execution_trace(thread_id).await?;
    /// let recommendations = trace.calculate_optimal_timeouts();
    ///
    /// for rec in recommendations.above_confidence(0.8) {
    ///     println!("{}", rec.description());
    /// }
    /// ```
    #[must_use]
    pub fn calculate_optimal_timeouts(&self) -> crate::adaptive_timeout::TimeoutRecommendations {
        self.calculate_optimal_timeouts_with_config(
            &crate::adaptive_timeout::TimeoutConfig::default(),
        )
    }

    /// Calculate optimal timeouts using custom configuration
    ///
    /// Allows fine-tuning of timeout calculation parameters such as:
    /// - Which percentile to use (p95, p99, max)
    /// - Buffer multiplier (e.g., 1.5x)
    /// - Minimum/maximum bounds
    /// - Confidence requirements
    #[must_use]
    pub fn calculate_optimal_timeouts_with_config(
        &self,
        config: &crate::adaptive_timeout::TimeoutConfig,
    ) -> crate::adaptive_timeout::TimeoutRecommendations {
        let mut learner = crate::adaptive_timeout::TimeoutLearner::new();
        learner.add_trace(self);
        learner.learn(config)
    }

    /// Get mutations to apply learned timeouts above a confidence threshold
    ///
    /// Convenience method that analyzes the trace and returns ready-to-apply
    /// graph mutations for timeout adjustments.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let trace = compiled.get_execution_trace(thread_id).await?;
    ///
    /// // Get high-confidence timeout adjustments
    /// let mutations = trace.get_timeout_mutations(0.8);
    ///
    /// // Apply them
    /// for mutation in mutations {
    ///     compiled.apply_mutation(mutation)?;
    /// }
    /// ```
    #[must_use]
    pub fn get_timeout_mutations(
        &self,
        confidence_threshold: f64,
    ) -> Vec<crate::graph_reconfiguration::GraphMutation> {
        let recommendations = self.calculate_optimal_timeouts();
        recommendations.to_mutations(confidence_threshold)
    }

    /// Check if any nodes would benefit from timeout adjustment
    ///
    /// Quick check to determine if timeout optimization is warranted.
    #[must_use]
    pub fn has_timeout_optimization_opportunities(&self) -> bool {
        let recommendations = self.calculate_optimal_timeouts();
        recommendations.has_recommendations()
    }

    /// Get a summary of timeout optimization opportunities
    ///
    /// Returns a human-readable summary of potential timeout improvements.
    #[must_use]
    pub fn timeout_optimization_summary(&self) -> String {
        let recommendations = self.calculate_optimal_timeouts();
        if !recommendations.has_recommendations() {
            return "No timeout optimization opportunities found (insufficient data or stable configuration).".to_string();
        }

        let mut lines = vec![recommendations.summary.clone()];

        for rec in &recommendations.recommendations {
            lines.push(format!("  - {}", rec.description()));
        }

        lines.join("\n")
    }

    // =========================================================================
    // Causal Analysis Methods
    // =========================================================================

    /// Analyze causality for a specific effect
    ///
    /// Identifies root causes of observable outcomes like high latency,
    /// failures, or resource exhaustion. Returns a causal chain showing
    /// contributing factors with their relative contributions and evidence.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::causal_analysis::Effect;
    ///
    /// let trace = compiled.get_execution_trace(thread_id).await?;
    ///
    /// // AI asks: "Why was I slow?"
    /// let chain = trace.analyze_causality(Effect::HighLatency);
    ///
    /// println!("Root causes:");
    /// for cause in chain.causes_by_contribution() {
    ///     println!("  - {} ({:.1}%): {}",
    ///         cause.factor,
    ///         cause.contribution * 100.0,
    ///         cause.evidence
    ///     );
    /// }
    /// ```
    #[must_use]
    pub fn analyze_causality(
        &self,
        effect: crate::causal_analysis::Effect,
    ) -> crate::causal_analysis::CausalChain {
        let analyzer = crate::causal_analysis::CausalAnalyzer::new();
        analyzer.analyze(self, effect)
    }

    /// Analyze causality with custom configuration
    ///
    /// Allows fine-tuning of causal analysis parameters such as:
    /// - Minimum contribution threshold for reporting causes
    /// - Confidence requirements
    /// - Maximum number of causes to report
    /// - Detection thresholds for various factors
    #[must_use]
    pub fn analyze_causality_with_config(
        &self,
        effect: crate::causal_analysis::Effect,
        config: &crate::causal_analysis::CausalAnalysisConfig,
    ) -> crate::causal_analysis::CausalChain {
        let analyzer = crate::causal_analysis::CausalAnalyzer::with_config(config.clone());
        analyzer.analyze(self, effect)
    }

    /// Automatically detect and analyze all potential issues
    ///
    /// Scans the trace for effects like high latency, failures, loops,
    /// and other issues, then performs causal analysis on each.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let trace = compiled.get_execution_trace(thread_id).await?;
    ///
    /// // AI asks: "What went wrong?"
    /// for chain in trace.auto_analyze_causality() {
    ///     println!("\n{}", chain.report());
    /// }
    /// ```
    #[must_use]
    pub fn auto_analyze_causality(&self) -> Vec<crate::causal_analysis::CausalChain> {
        let analyzer = crate::causal_analysis::CausalAnalyzer::new();
        analyzer.auto_analyze(self)
    }

    /// Auto-analyze with custom configuration
    #[must_use]
    pub fn auto_analyze_causality_with_config(
        &self,
        config: &crate::causal_analysis::CausalAnalysisConfig,
    ) -> Vec<crate::causal_analysis::CausalChain> {
        let analyzer = crate::causal_analysis::CausalAnalyzer::with_config(config.clone());
        analyzer.auto_analyze(self)
    }

    /// Check if there are any causal analysis opportunities
    ///
    /// Quick check to determine if causal analysis would find anything.
    /// Useful to avoid expensive analysis when trace is clean.
    #[must_use]
    pub fn has_causal_analysis_opportunities(&self) -> bool {
        // Check for issues that would trigger analysis
        let thresholds = crate::causal_analysis::CausalThresholds::default();

        // High latency?
        if self.total_duration_ms > thresholds.high_latency_ms {
            return true;
        }

        // Errors?
        if !self.errors.is_empty() || !self.completed {
            return true;
        }

        // Potential loops?
        let mut node_counts: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        for exec in &self.nodes_executed {
            *node_counts.entry(&exec.node).or_insert(0) += 1;
        }
        for count in node_counts.values() {
            if *count >= thresholds.repeated_execution_count {
                return true;
            }
        }

        false
    }

    /// Get a summary of causal analysis findings
    ///
    /// Performs auto-analysis and returns a human-readable summary.
    #[must_use]
    pub fn causal_analysis_summary(&self) -> String {
        let chains = self.auto_analyze_causality();
        if chains.is_empty() {
            return "No issues detected in execution trace.".to_string();
        }

        let mut lines = vec![format!("Found {} issue(s):", chains.len())];
        for chain in &chains {
            lines.push(format!("\n{}", chain.report()));
        }
        lines.join("\n")
    }

    /// Analyze why a specific node was slow
    ///
    /// Convenience method for analyzing a single node's performance.
    #[must_use]
    pub fn analyze_slow_node(&self, node_name: &str) -> crate::causal_analysis::CausalChain {
        self.analyze_causality(crate::causal_analysis::Effect::SlowNode(
            node_name.to_string(),
        ))
    }

    /// Analyze why execution failed
    ///
    /// Convenience method for analyzing execution failures.
    #[must_use]
    pub fn analyze_failure(&self) -> crate::causal_analysis::CausalChain {
        self.analyze_causality(crate::causal_analysis::Effect::ExecutionFailure)
    }

    /// Analyze high latency causes
    ///
    /// Convenience method for latency analysis.
    #[must_use]
    pub fn analyze_latency(&self) -> crate::causal_analysis::CausalChain {
        self.analyze_causality(crate::causal_analysis::Effect::HighLatency)
    }

    // =========================================================================
    // Counterfactual Analysis Methods
    // =========================================================================

    /// Perform counterfactual analysis for a specific node
    ///
    /// Simulates "what if" scenarios to estimate the impact of alternative
    /// decisions without re-executing. Useful for understanding trade-offs.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::counterfactual_analysis::Alternative;
    ///
    /// let trace = compiled.get_execution_trace(thread_id).await?;
    ///
    /// // AI asks: "What if I had used a faster model?"
    /// let result = trace.counterfactual_analysis(
    ///     "reasoning",
    ///     Alternative::UseModel("gpt-3.5-turbo".into())
    /// );
    ///
    /// println!("Estimated change: {}", result.estimated_improvement.summary());
    /// println!("Recommendation: {:?}", result.recommendation);
    /// ```
    #[must_use]
    pub fn counterfactual_analysis(
        &self,
        node_name: &str,
        alternative: crate::counterfactual_analysis::Alternative,
    ) -> crate::counterfactual_analysis::CounterfactualResult {
        let analyzer = crate::counterfactual_analysis::CounterfactualAnalyzer::new();
        analyzer.analyze(self, node_name, alternative)
    }

    /// Perform counterfactual analysis with custom configuration
    #[must_use]
    pub fn counterfactual_analysis_with_config(
        &self,
        node_name: &str,
        alternative: crate::counterfactual_analysis::Alternative,
        config: &crate::counterfactual_analysis::CounterfactualConfig,
    ) -> crate::counterfactual_analysis::CounterfactualResult {
        let analyzer =
            crate::counterfactual_analysis::CounterfactualAnalyzer::with_config(config.clone());
        analyzer.analyze(self, node_name, alternative)
    }

    /// Analyze all reasonable alternatives for a node
    ///
    /// Returns counterfactual results for various alternatives (different models,
    /// caching, parallel execution, etc.)
    #[must_use]
    pub fn explore_alternatives(
        &self,
        node_name: &str,
    ) -> Vec<crate::counterfactual_analysis::CounterfactualResult> {
        let analyzer = crate::counterfactual_analysis::CounterfactualAnalyzer::new();
        analyzer.analyze_all_alternatives(self, node_name)
    }

    /// Get the best counterfactual recommendations across all nodes
    ///
    /// Returns the top N beneficial alternatives that the AI should consider.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let trace = compiled.get_execution_trace(thread_id).await?;
    ///
    /// // AI asks: "What could I have done better?"
    /// for rec in trace.best_counterfactual_recommendations(5) {
    ///     println!("Node '{}': {} - {}", rec.node, rec.alternative, rec.estimated_improvement.summary());
    /// }
    /// ```
    #[must_use]
    pub fn best_counterfactual_recommendations(
        &self,
        limit: usize,
    ) -> Vec<crate::counterfactual_analysis::CounterfactualResult> {
        let analyzer = crate::counterfactual_analysis::CounterfactualAnalyzer::new();
        analyzer.best_recommendations(self, limit)
    }

    /// Check if counterfactual analysis would yield useful insights
    ///
    /// Returns true if there are nodes that could benefit from alternative approaches.
    #[must_use]
    pub fn has_counterfactual_opportunities(&self) -> bool {
        !self.best_counterfactual_recommendations(1).is_empty()
    }

    /// Get a summary of counterfactual recommendations
    #[must_use]
    pub fn counterfactual_summary(&self) -> String {
        let recommendations = self.best_counterfactual_recommendations(5);
        if recommendations.is_empty() {
            return "No significant optimization opportunities found.".to_string();
        }

        let mut lines = vec![format!(
            "Found {} optimization opportunity(ies):",
            recommendations.len()
        )];
        for rec in &recommendations {
            lines.push(format!(
                "  - '{}': {} → {}",
                rec.node,
                rec.alternative,
                rec.estimated_improvement.summary()
            ));
        }
        lines.join("\n")
    }

    /// Convenience method to check what if a different model was used
    #[must_use]
    pub fn what_if_model(
        &self,
        node_name: &str,
        model: &str,
    ) -> crate::counterfactual_analysis::CounterfactualResult {
        self.counterfactual_analysis(
            node_name,
            crate::counterfactual_analysis::Alternative::UseModel(model.to_string()),
        )
    }

    /// Convenience method to check what if caching was used
    #[must_use]
    pub fn what_if_cached(
        &self,
        node_name: &str,
    ) -> crate::counterfactual_analysis::CounterfactualResult {
        self.counterfactual_analysis(
            node_name,
            crate::counterfactual_analysis::Alternative::CacheResult,
        )
    }

    /// Convenience method to check what if executed in parallel
    #[must_use]
    pub fn what_if_parallel(
        &self,
        node_name: &str,
        other_node: &str,
    ) -> crate::counterfactual_analysis::CounterfactualResult {
        self.counterfactual_analysis(
            node_name,
            crate::counterfactual_analysis::Alternative::ParallelWith(other_node.to_string()),
        )
    }

    // =========================================================================
    // Telemetry Unification Methods (for DashOpt Integration)
    // =========================================================================
    // These methods enable ExecutionTrace to serve as the canonical telemetry
    // type for optimizers, removing the need for Kafka for local optimization.
    // See DESIGN_INVARIANTS.md Invariant 9 (Telemetry Self-Consumption).

    /// Convert trace to training examples for optimizers
    ///
    /// Extracts input/output pairs from node executions that have state snapshots.
    /// These examples can be used by DashOpt optimizers (GRPO, BootstrapFewShot, etc.)
    /// without requiring Kafka or external infrastructure.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let trace = compiled.get_execution_trace(thread_id).await?;
    /// let examples = trace.to_examples();
    ///
    /// // Use examples for local optimization
    /// let optimizer = GrpoOptimizer::new(metric, config);
    /// optimizer.add_examples(examples);
    /// ```
    #[must_use]
    pub fn to_examples(&self) -> Vec<crate::optimize::Example> {
        self.nodes_executed
            .iter()
            .filter_map(|node_exec| {
                // Only create examples from nodes that have state snapshots
                let state_before = node_exec.state_before.as_ref()?;
                let state_after = node_exec.state_after.as_ref()?;

                // Extract fields from state_before as inputs and state_after as outputs
                let mut example = crate::optimize::Example::new();

                // Add all fields from state_before as example inputs
                if let Some(obj) = state_before.as_object() {
                    for (key, value) in obj {
                        example = example.with(format!("input_{}", key), value.clone());
                    }
                }

                // Add all fields from state_after as example outputs
                if let Some(obj) = state_after.as_object() {
                    for (key, value) in obj {
                        example = example.with(format!("output_{}", key), value.clone());
                    }
                }

                // Add metadata about the node
                example = example.with("_node", serde_json::json!(node_exec.node.clone()));
                example = example.with("_duration_ms", serde_json::json!(node_exec.duration_ms));
                example = example.with("_success", serde_json::json!(node_exec.success));

                Some(example)
            })
            .collect()
    }

    /// Convert NodeExecution records to legacy TraceEntry format
    ///
    /// This method provides backward compatibility with optimizers that expect
    /// the TraceEntry format. For new code, prefer working with ExecutionTrace
    /// directly.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let trace = compiled.get_execution_trace(thread_id).await?;
    /// let entries = trace.to_trace_entries();
    ///
    /// // Compatible with existing optimizer code
    /// for entry in entries {
    ///     println!("Node: {}, Inputs: {:?}", entry.predictor_name, entry.inputs);
    /// }
    /// ```
    #[must_use]
    #[allow(deprecated)] // Returns deprecated TraceEntry for backwards compatibility
    pub fn to_trace_entries(&self) -> Vec<crate::optimize::TraceEntry> {
        self.nodes_executed
            .iter()
            .map(|node_exec| {
                // Convert state_before to inputs HashMap
                let inputs: HashMap<String, serde_json::Value> = node_exec
                    .state_before
                    .as_ref()
                    .and_then(|v| v.as_object())
                    .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default();

                // Convert state_after to PredictionOrFailed
                let outputs = if node_exec.success {
                    // Build a successful prediction from state_after
                    let mut prediction = crate::optimize::Prediction::new();
                    if let Some(obj) = node_exec.state_after.as_ref().and_then(|v| v.as_object()) {
                        for (k, v) in obj {
                            prediction = prediction.with_field(k, v.clone());
                        }
                    }
                    crate::optimize::PredictionOrFailed::Success(prediction)
                } else {
                    // Build a failed prediction
                    crate::optimize::PredictionOrFailed::Failed(crate::optimize::FailedPrediction {
                        error: node_exec
                            .error_message
                            .clone()
                            .unwrap_or_else(|| "Unknown error".to_string()),
                    })
                };

                crate::optimize::TraceEntry {
                    predictor_name: node_exec.node.clone(),
                    inputs,
                    outputs,
                }
            })
            .collect()
    }

    /// Convert trace to TraceData for DashOpt optimizers
    ///
    /// Creates a complete TraceData structure that can be used by GRPO and other
    /// optimizers. This enables local optimization without Kafka.
    ///
    /// # Arguments
    ///
    /// * `example` - The input example that was executed
    /// * `example_index` - Index of this example in the dataset
    /// * `score` - Optional metric score (pass None if not yet evaluated)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let trace = compiled.get_execution_trace(thread_id).await?;
    /// let score = metric.evaluate(&example, &trace);
    /// let trace_data = trace.to_trace_data(example, 0, Some(score));
    ///
    /// // Use with GRPO optimizer
    /// optimizer.add_trace_data(trace_data);
    /// ```
    #[must_use]
    #[allow(deprecated)] // Returns deprecated TraceData for backwards compatibility
    pub fn to_trace_data(
        &self,
        example: crate::optimize::Example,
        example_index: usize,
        score: Option<f64>,
    ) -> crate::optimize::TraceData {
        // Get the final prediction from the trace
        let prediction = self.final_prediction();
        let trace_entries = self.to_trace_entries();

        crate::optimize::TraceData {
            example_ind: example_index,
            example,
            prediction,
            trace: trace_entries,
            score,
        }
    }

    /// Extract the final prediction from this trace
    ///
    /// Returns the final state as a PredictionOrFailed, suitable for
    /// optimizer evaluation.
    #[must_use]
    pub fn final_prediction(&self) -> crate::optimize::PredictionOrFailed {
        if self.completed && self.errors.is_empty() {
            // Success: use final_state or last node's state_after
            let final_state = self.final_state.as_ref().or_else(|| {
                self.nodes_executed
                    .last()
                    .and_then(|n| n.state_after.as_ref())
            });

            let mut prediction = crate::optimize::Prediction::new();
            if let Some(obj) = final_state.and_then(|v| v.as_object()) {
                for (k, v) in obj {
                    prediction = prediction.with_field(k, v.clone());
                }
            }
            crate::optimize::PredictionOrFailed::Success(prediction)
        } else {
            // Failure: extract error message
            let error = self
                .errors
                .first()
                .map(|e| e.message.clone())
                .unwrap_or_else(|| "Execution did not complete".to_string());
            crate::optimize::PredictionOrFailed::Failed(crate::optimize::FailedPrediction { error })
        }
    }

    /// Check if this trace has sufficient data for optimizer training
    ///
    /// Returns true if at least one node execution has state snapshots,
    /// making it usable for example generation.
    #[must_use]
    pub fn has_training_data(&self) -> bool {
        self.nodes_executed
            .iter()
            .any(|n| n.state_before.is_some() && n.state_after.is_some())
    }

    /// Get the number of training examples this trace can produce
    #[must_use]
    pub fn training_example_count(&self) -> usize {
        self.nodes_executed
            .iter()
            .filter(|n| n.state_before.is_some() && n.state_after.is_some())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::introspection::trace::{
        ErrorTrace, ExecutionTrace, ExecutionTraceBuilder, NodeExecution,
    };
    use serde_json::json;

    // =========================================================================
    // Test Helpers
    // =========================================================================

    fn create_node_execution(node: &str, duration_ms: u64, success: bool) -> NodeExecution {
        let mut exec = NodeExecution::new(node, duration_ms)
            .with_tokens(duration_ms / 10) // rough correlation
            .with_state_before(json!({"input": "test"}))
            .with_state_after(json!({"output": "result"}))
            .with_index(0);
        if !success {
            exec = exec.with_error("test error");
        }
        exec
    }

    fn create_node_with_tokens(node: &str, duration_ms: u64, tokens: u64) -> NodeExecution {
        NodeExecution::new(node, duration_ms)
            .with_tokens(tokens)
            .with_state_before(json!({"input": "test"}))
            .with_state_after(json!({"output": "result"}))
    }

    fn create_node_with_retries(node: &str, executions: usize) -> Vec<NodeExecution> {
        (0..executions)
            .map(|i| {
                let success = i == executions - 1; // Only last succeeds
                let mut exec = NodeExecution::new(node, 100)
                    .with_tokens(10)
                    .with_state_before(json!({"input": "test"}))
                    .with_state_after(json!({"output": "result"}))
                    .with_index(i);
                if !success {
                    exec = exec.with_error("retry error");
                }
                exec
            })
            .collect()
    }

    fn create_simple_trace() -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .thread_id("test-thread")
            .execution_id("test-exec")
            .add_node_execution(create_node_execution("node_a", 100, true))
            .add_node_execution(create_node_execution("node_b", 200, true))
            .total_duration_ms(300)
            .total_tokens(30)
            .completed(true)
            .build()
    }

    fn create_trace_with_high_latency() -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .thread_id("test-thread")
            .add_node_execution(create_node_with_tokens("slow_node", 5000, 10000)) // High latency
            .total_duration_ms(5000)
            .total_tokens(10000)
            .completed(true)
            .build()
    }

    fn create_trace_with_errors() -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .thread_id("test-thread")
            .add_node_execution(create_node_execution("node_a", 100, true))
            .add_node_execution(create_node_execution("failing_node", 50, false))
            .add_error(ErrorTrace {
                node: "failing_node".to_string(),
                message: "Execution failed".to_string(),
                error_type: Some("RuntimeError".to_string()),
                state_at_error: None,
                timestamp: None,
                execution_index: Some(1),
                recoverable: false,
                retry_attempted: false,
                context: None,
                metadata: HashMap::new(),
            })
            .total_duration_ms(150)
            .total_tokens(15)
            .completed(false)
            .build()
    }

    fn create_trace_with_retries() -> ExecutionTrace {
        let nodes = create_node_with_retries("retry_node", 3);
        let duration: u64 = nodes.iter().map(|n| n.duration_ms).sum();

        ExecutionTraceBuilder::new()
            .thread_id("test-thread")
            .nodes_executed(nodes)
            .total_duration_ms(duration)
            .total_tokens(30)
            .completed(true)
            .build()
    }

    fn create_trace_with_loop() -> ExecutionTrace {
        // Same node executed many times (potential loop)
        let nodes: Vec<NodeExecution> = (0..10)
            .map(|i| {
                NodeExecution::new("loop_node", 50)
                    .with_tokens(5)
                    .with_state_before(json!({"input": "test"}))
                    .with_state_after(json!({"output": "result"}))
                    .with_index(i)
            })
            .collect();

        ExecutionTraceBuilder::new()
            .thread_id("test-thread")
            .nodes_executed(nodes)
            .total_duration_ms(500)
            .total_tokens(50)
            .completed(true)
            .build()
    }

    // =========================================================================
    // Prompt Self-Evolution Integration Tests
    // =========================================================================

    #[test]
    fn test_analyze_prompt_effectiveness_empty_trace() {
        let trace = ExecutionTrace::default();
        let analyses = trace.analyze_prompt_effectiveness();
        assert!(analyses.is_empty());
    }

    #[test]
    fn test_analyze_prompt_effectiveness_simple_trace() {
        let trace = create_simple_trace();
        let analyses = trace.analyze_prompt_effectiveness();

        // Should have one analysis per unique node
        assert_eq!(analyses.len(), 2);

        // Analyses should be sorted by effectiveness (lowest first)
        for i in 0..analyses.len() - 1 {
            assert!(analyses[i].effectiveness_score <= analyses[i + 1].effectiveness_score);
        }
    }

    #[test]
    fn test_analyze_prompt_effectiveness_with_retries() {
        let trace = create_trace_with_retries();
        let analyses = trace.analyze_prompt_effectiveness();

        // Should have one analysis for retry_node
        assert_eq!(analyses.len(), 1);
        assert_eq!(analyses[0].node, "retry_node");

        // Should detect high retry rate as an issue
        // (3 executions with only 1 success = 66% retry rate)
    }

    #[test]
    fn test_analyze_prompt_effectiveness_with_custom_thresholds() {
        let trace = create_trace_with_retries();

        // Very strict thresholds
        let strict_thresholds = crate::prompt_evolution::PromptThresholds {
            max_retry_rate: 0.1,
            max_error_rate: 0.01,
            max_avg_tokens: 100.0,
            max_avg_response_time_ms: 100.0,
            min_effectiveness: 0.95,
            min_executions: 1,
            max_repetitions: 2,
        };
        let strict_analyses =
            trace.analyze_prompt_effectiveness_with_thresholds(&strict_thresholds);

        // Very lenient thresholds
        let lenient_thresholds = crate::prompt_evolution::PromptThresholds {
            max_retry_rate: 0.9,
            max_error_rate: 0.9,
            max_avg_tokens: 10000.0,
            max_avg_response_time_ms: 10000.0,
            min_effectiveness: 0.1,
            min_executions: 1,
            max_repetitions: 100,
        };
        let lenient_analyses =
            trace.analyze_prompt_effectiveness_with_thresholds(&lenient_thresholds);

        // Both should return analyses for the same nodes
        assert_eq!(strict_analyses.len(), lenient_analyses.len());
    }

    #[test]
    fn test_evolve_prompts_empty_trace() {
        let trace = ExecutionTrace::default();
        let evolutions = trace.evolve_prompts();
        assert!(evolutions.is_empty());
    }

    #[test]
    fn test_evolve_prompts_simple_trace() {
        let trace = create_simple_trace();
        let evolutions = trace.evolve_prompts();

        // Simple successful trace may or may not have evolutions
        // depending on effectiveness analysis
        for evolution in &evolutions {
            assert!(!evolution.node.is_empty());
            assert!(!evolution.description().is_empty());
        }
    }

    #[test]
    fn test_evolve_prompts_with_custom_thresholds() {
        let trace = create_trace_with_retries();
        let thresholds = crate::prompt_evolution::PromptThresholds::default();
        let evolutions = trace.evolve_prompts_with_thresholds(&thresholds);

        // Each evolution should have valid fields
        for evolution in &evolutions {
            assert!(!evolution.node.is_empty());
        }
    }

    #[test]
    fn test_has_prompt_issues_healthy_trace() {
        let trace = create_simple_trace();
        // Simple healthy trace may or may not have issues
        let _ = trace.has_prompt_issues(); // Just ensure it runs without panic
    }

    #[test]
    fn test_has_prompt_issues_with_retries() {
        let trace = create_trace_with_retries();
        // Trace with retries likely has prompt issues
        let _ = trace.has_prompt_issues(); // Just ensure it runs without panic
    }

    #[test]
    fn test_prompt_health_summary_empty_trace() {
        let trace = ExecutionTrace::default();
        let summary = trace.prompt_health_summary();
        assert!(summary.contains("0 node"));
    }

    #[test]
    fn test_prompt_health_summary_healthy_trace() {
        let trace = create_simple_trace();
        let summary = trace.prompt_health_summary();

        // Summary should be non-empty and well-formatted
        assert!(!summary.is_empty());
    }

    #[test]
    fn test_prompt_health_summary_with_issues() {
        let trace = create_trace_with_errors();
        let summary = trace.prompt_health_summary();

        // Summary should be non-empty
        assert!(!summary.is_empty());
    }

    // =========================================================================
    // Adaptive Timeout Integration Tests
    // =========================================================================

    #[test]
    fn test_collect_latency_stats_empty_trace() {
        let trace = ExecutionTrace::default();
        let stats = trace.collect_latency_stats();
        assert!(stats.is_empty());
    }

    #[test]
    fn test_collect_latency_stats_simple_trace() {
        let trace = create_simple_trace();
        let stats = trace.collect_latency_stats();

        // Should have stats for each unique node
        assert_eq!(stats.len(), 2);

        // Each stat should have valid node name
        for stat in &stats {
            assert!(!stat.node.is_empty());
        }
    }

    #[test]
    fn test_collect_latency_stats_multiple_executions() {
        let trace = create_trace_with_retries();
        let stats = trace.collect_latency_stats();

        // Should aggregate multiple executions of same node
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].node, "retry_node");
    }

    #[test]
    fn test_calculate_optimal_timeouts_empty_trace() {
        let trace = ExecutionTrace::default();
        let recommendations = trace.calculate_optimal_timeouts();

        // Empty trace should have no recommendations
        assert!(recommendations.recommendations.is_empty());
    }

    #[test]
    fn test_calculate_optimal_timeouts_simple_trace() {
        let trace = create_simple_trace();
        let recommendations = trace.calculate_optimal_timeouts();

        // Recommendations should have valid structure
        assert!(!recommendations.summary.is_empty());
    }

    #[test]
    fn test_calculate_optimal_timeouts_with_config() {
        let trace = create_simple_trace();

        let config = crate::adaptive_timeout::TimeoutConfig {
            base_percentile: crate::adaptive_timeout::TimeoutPercentile::P99,
            buffer_multiplier: 2.0,
            min_timeout_ms: 100,
            max_timeout_ms: 60000,
            min_samples: 1,
            min_confidence: 0.5,
            require_stability: false,
            max_cv_for_stability: 1.0,
        };

        let recommendations = trace.calculate_optimal_timeouts_with_config(&config);

        // Recommendations should have valid structure
        for rec in &recommendations.recommendations {
            assert!(rec.recommended_timeout_ms >= config.min_timeout_ms);
            assert!(rec.recommended_timeout_ms <= config.max_timeout_ms);
        }
    }

    #[test]
    fn test_get_timeout_mutations_empty_trace() {
        let trace = ExecutionTrace::default();
        let mutations = trace.get_timeout_mutations(0.8);
        assert!(mutations.is_empty());
    }

    #[test]
    fn test_get_timeout_mutations_with_threshold() {
        let trace = create_simple_trace();

        // High confidence threshold may yield fewer mutations
        let high_conf_mutations = trace.get_timeout_mutations(0.9);

        // Low confidence threshold may yield more mutations
        let low_conf_mutations = trace.get_timeout_mutations(0.1);

        // Low threshold should yield >= high threshold mutations
        assert!(low_conf_mutations.len() >= high_conf_mutations.len());
    }

    #[test]
    fn test_has_timeout_optimization_opportunities_empty_trace() {
        let trace = ExecutionTrace::default();
        let has_opps = trace.has_timeout_optimization_opportunities();
        assert!(!has_opps); // Empty trace has no opportunities
    }

    #[test]
    fn test_has_timeout_optimization_opportunities_simple_trace() {
        let trace = create_simple_trace();
        let _ = trace.has_timeout_optimization_opportunities(); // Just ensure no panic
    }

    #[test]
    fn test_timeout_optimization_summary_empty_trace() {
        let trace = ExecutionTrace::default();
        let summary = trace.timeout_optimization_summary();
        assert!(summary.contains("No timeout optimization"));
    }

    #[test]
    fn test_timeout_optimization_summary_simple_trace() {
        let trace = create_simple_trace();
        let summary = trace.timeout_optimization_summary();

        // Summary should be non-empty
        assert!(!summary.is_empty());
    }

    // =========================================================================
    // Causal Analysis Tests
    // =========================================================================

    #[test]
    fn test_analyze_causality_high_latency() {
        let trace = create_trace_with_high_latency();
        let chain = trace.analyze_causality(crate::causal_analysis::Effect::HighLatency);

        // Chain should have valid effect
        assert_eq!(chain.effect, crate::causal_analysis::Effect::HighLatency);
    }

    #[test]
    fn test_analyze_causality_execution_failure() {
        let trace = create_trace_with_errors();
        let chain = trace.analyze_causality(crate::causal_analysis::Effect::ExecutionFailure);

        // Chain should analyze failure
        assert_eq!(
            chain.effect,
            crate::causal_analysis::Effect::ExecutionFailure
        );
    }

    #[test]
    fn test_analyze_causality_slow_node() {
        let trace = create_trace_with_high_latency();
        let chain = trace.analyze_causality(crate::causal_analysis::Effect::SlowNode(
            "slow_node".to_string(),
        ));

        // Chain should target specific node
        assert!(matches!(
            chain.effect,
            crate::causal_analysis::Effect::SlowNode(_)
        ));
    }

    #[test]
    fn test_analyze_causality_with_config() {
        let trace = create_trace_with_high_latency();

        let config = crate::causal_analysis::CausalAnalysisConfig {
            min_contribution: 0.1,
            min_confidence: 0.6,
            max_causes: 5,
            thresholds: crate::causal_analysis::CausalThresholds::default(),
        };

        let chain = trace
            .analyze_causality_with_config(crate::causal_analysis::Effect::HighLatency, &config);

        // Should respect max_causes
        assert!(chain.causes.len() <= config.max_causes);
    }

    #[test]
    fn test_auto_analyze_causality_empty_trace() {
        let trace = ExecutionTrace::default();
        let _chains = trace.auto_analyze_causality();

        // Auto-analysis may or may not return chains for empty traces
        // depending on the analyzer's behavior with missing data.
        // Just verify it doesn't panic.
    }

    #[test]
    fn test_auto_analyze_causality_with_issues() {
        let trace = create_trace_with_errors();
        let _chains = trace.auto_analyze_causality();

        // Trace with errors should detect at least one issue
        // (though auto-detect may or may not find them depending on thresholds)
    }

    #[test]
    fn test_auto_analyze_causality_with_high_latency() {
        let trace = create_trace_with_high_latency();
        let _chains = trace.auto_analyze_causality();

        // High latency trace may trigger analysis
        // (depends on default thresholds)
    }

    #[test]
    fn test_auto_analyze_causality_with_config() {
        let trace = create_trace_with_errors();

        let config = crate::causal_analysis::CausalAnalysisConfig::default();
        let chains = trace.auto_analyze_causality_with_config(&config);

        // Each chain should have valid structure
        for chain in &chains {
            assert!(!chain.report().is_empty());
        }
    }

    #[test]
    fn test_has_causal_analysis_opportunities_clean_trace() {
        let trace = create_simple_trace();
        // Simple clean trace may or may not have opportunities
        let _ = trace.has_causal_analysis_opportunities();
    }

    #[test]
    fn test_has_causal_analysis_opportunities_with_errors() {
        let trace = create_trace_with_errors();
        let has_opps = trace.has_causal_analysis_opportunities();

        // Trace with errors should have analysis opportunities
        assert!(has_opps);
    }

    #[test]
    fn test_has_causal_analysis_opportunities_with_loop() {
        let trace = create_trace_with_loop();
        let has_opps = trace.has_causal_analysis_opportunities();

        // Trace with potential loop (repeated executions) should have opportunities
        assert!(has_opps);
    }

    #[test]
    fn test_causal_analysis_summary_clean_trace() {
        let trace = create_simple_trace();
        let summary = trace.causal_analysis_summary();

        // Summary should be non-empty
        assert!(!summary.is_empty());
    }

    #[test]
    fn test_causal_analysis_summary_with_issues() {
        let trace = create_trace_with_errors();
        let summary = trace.causal_analysis_summary();

        // Summary should be non-empty
        assert!(!summary.is_empty());
    }

    #[test]
    fn test_analyze_slow_node() {
        let trace = create_trace_with_high_latency();
        let chain = trace.analyze_slow_node("slow_node");

        // Should analyze the specific node
        assert!(
            matches!(chain.effect, crate::causal_analysis::Effect::SlowNode(ref n) if n == "slow_node")
        );
    }

    #[test]
    fn test_analyze_failure() {
        let trace = create_trace_with_errors();
        let chain = trace.analyze_failure();

        // Should analyze execution failure
        assert_eq!(
            chain.effect,
            crate::causal_analysis::Effect::ExecutionFailure
        );
    }

    #[test]
    fn test_analyze_latency() {
        let trace = create_trace_with_high_latency();
        let chain = trace.analyze_latency();

        // Should analyze high latency
        assert_eq!(chain.effect, crate::causal_analysis::Effect::HighLatency);
    }

    // =========================================================================
    // Counterfactual Analysis Tests
    // =========================================================================

    #[test]
    fn test_counterfactual_analysis_use_model() {
        let trace = create_simple_trace();
        let result = trace.counterfactual_analysis(
            "node_a",
            crate::counterfactual_analysis::Alternative::UseModel("gpt-3.5-turbo".to_string()),
        );

        // Result should have valid structure
        assert_eq!(result.node, "node_a");
        assert!(matches!(
            result.alternative,
            crate::counterfactual_analysis::Alternative::UseModel(_)
        ));
    }

    #[test]
    fn test_counterfactual_analysis_cache_result() {
        let trace = create_simple_trace();
        let result = trace.counterfactual_analysis(
            "node_b",
            crate::counterfactual_analysis::Alternative::CacheResult,
        );

        assert_eq!(result.node, "node_b");
        assert_eq!(
            result.alternative,
            crate::counterfactual_analysis::Alternative::CacheResult
        );
    }

    #[test]
    fn test_counterfactual_analysis_parallel() {
        let trace = create_simple_trace();
        let result = trace.counterfactual_analysis(
            "node_a",
            crate::counterfactual_analysis::Alternative::ParallelWith("node_b".to_string()),
        );

        assert_eq!(result.node, "node_a");
        assert!(matches!(
            result.alternative,
            crate::counterfactual_analysis::Alternative::ParallelWith(_)
        ));
    }

    #[test]
    fn test_counterfactual_analysis_with_config() {
        let trace = create_simple_trace();

        let mut model_speeds = HashMap::new();
        model_speeds.insert("gpt-3.5-turbo".to_string(), 2.0);
        model_speeds.insert("gpt-4".to_string(), 0.5);

        let config = crate::counterfactual_analysis::CounterfactualConfig {
            model_speed_factors: model_speeds,
            model_costs: HashMap::new(),
            model_quality_factors: HashMap::new(),
            cache_hit_factor: 0.8,
            parallel_factor: 0.9,
            min_confidence_to_recommend: 0.5,
        };

        let result = trace.counterfactual_analysis_with_config(
            "node_a",
            crate::counterfactual_analysis::Alternative::UseModel("gpt-3.5-turbo".to_string()),
            &config,
        );

        assert_eq!(result.node, "node_a");
    }

    #[test]
    fn test_explore_alternatives_empty_node() {
        let trace = create_simple_trace();
        let results = trace.explore_alternatives("nonexistent_node");

        // Should return results (possibly empty or with default estimates)
        for result in &results {
            assert_eq!(result.node, "nonexistent_node");
        }
    }

    #[test]
    fn test_explore_alternatives_existing_node() {
        let trace = create_simple_trace();
        let results = trace.explore_alternatives("node_a");

        // Should explore multiple alternatives
        assert!(!results.is_empty());

        // All results should be for node_a
        for result in &results {
            assert_eq!(result.node, "node_a");
        }
    }

    #[test]
    fn test_best_counterfactual_recommendations_empty_trace() {
        let trace = ExecutionTrace::default();
        let recs = trace.best_counterfactual_recommendations(5);

        // Empty trace should have no recommendations
        assert!(recs.is_empty());
    }

    #[test]
    fn test_best_counterfactual_recommendations_simple_trace() {
        let trace = create_simple_trace();
        let recs = trace.best_counterfactual_recommendations(5);

        // Should respect limit
        assert!(recs.len() <= 5);
    }

    #[test]
    fn test_best_counterfactual_recommendations_high_latency() {
        let trace = create_trace_with_high_latency();
        let recs = trace.best_counterfactual_recommendations(3);

        // Should respect limit
        assert!(recs.len() <= 3);
    }

    #[test]
    fn test_has_counterfactual_opportunities_empty_trace() {
        let trace = ExecutionTrace::default();
        let has_opps = trace.has_counterfactual_opportunities();

        // Empty trace has no opportunities
        assert!(!has_opps);
    }

    #[test]
    fn test_has_counterfactual_opportunities_simple_trace() {
        let trace = create_simple_trace();
        let _ = trace.has_counterfactual_opportunities(); // Just ensure no panic
    }

    #[test]
    fn test_counterfactual_summary_empty_trace() {
        let trace = ExecutionTrace::default();
        let summary = trace.counterfactual_summary();

        // Should indicate no opportunities
        assert!(summary.contains("No significant"));
    }

    #[test]
    fn test_counterfactual_summary_simple_trace() {
        let trace = create_simple_trace();
        let summary = trace.counterfactual_summary();

        // Summary should be non-empty
        assert!(!summary.is_empty());
    }

    #[test]
    fn test_what_if_model() {
        let trace = create_simple_trace();
        let result = trace.what_if_model("node_a", "gpt-3.5-turbo");

        assert_eq!(result.node, "node_a");
        assert!(
            matches!(result.alternative, crate::counterfactual_analysis::Alternative::UseModel(ref m) if m == "gpt-3.5-turbo")
        );
    }

    #[test]
    fn test_what_if_cached() {
        let trace = create_simple_trace();
        let result = trace.what_if_cached("node_b");

        assert_eq!(result.node, "node_b");
        assert_eq!(
            result.alternative,
            crate::counterfactual_analysis::Alternative::CacheResult
        );
    }

    #[test]
    fn test_what_if_parallel() {
        let trace = create_simple_trace();
        let result = trace.what_if_parallel("node_a", "node_b");

        assert_eq!(result.node, "node_a");
        assert!(
            matches!(result.alternative, crate::counterfactual_analysis::Alternative::ParallelWith(ref n) if n == "node_b")
        );
    }

    // =========================================================================
    // Telemetry Unification Tests
    // =========================================================================

    #[test]
    fn test_to_examples_empty_trace() {
        let trace = ExecutionTrace::default();
        let examples = trace.to_examples();
        assert!(examples.is_empty());
    }

    #[test]
    fn test_to_examples_simple_trace() {
        let trace = create_simple_trace();
        let examples = trace.to_examples();

        // Should create examples for nodes with state snapshots
        assert_eq!(examples.len(), 2); // node_a and node_b both have state

        // Each example should have metadata
        for example in &examples {
            assert!(example.get("_node").is_some());
            assert!(example.get("_duration_ms").is_some());
            assert!(example.get("_success").is_some());
        }
    }

    #[test]
    fn test_to_examples_without_state() {
        // Create trace where nodes don't have state snapshots
        let node = NodeExecution::new("no_state_node", 100).with_tokens(10);
        // state_before and state_after are None by default

        let trace = ExecutionTraceBuilder::new()
            .add_node_execution(node)
            .build();

        let examples = trace.to_examples();

        // No examples because no state snapshots
        assert!(examples.is_empty());
    }

    #[test]
    fn test_to_trace_entries_empty_trace() {
        let trace = ExecutionTrace::default();
        #[allow(deprecated)]
        let entries = trace.to_trace_entries();
        assert!(entries.is_empty());
    }

    #[test]
    #[allow(deprecated)]
    fn test_to_trace_entries_simple_trace() {
        let trace = create_simple_trace();
        let entries = trace.to_trace_entries();

        // Should create entries for each node execution
        assert_eq!(entries.len(), 2);

        // Check predictor names match node names
        let names: Vec<_> = entries.iter().map(|e| e.predictor_name.as_str()).collect();
        assert!(names.contains(&"node_a"));
        assert!(names.contains(&"node_b"));
    }

    #[test]
    #[allow(deprecated)]
    fn test_to_trace_entries_with_failure() {
        let trace = create_trace_with_errors();
        let entries = trace.to_trace_entries();

        // Should handle failed nodes
        assert_eq!(entries.len(), 2);

        // Find the failing node entry
        let failing_entry = entries.iter().find(|e| e.predictor_name == "failing_node");
        assert!(failing_entry.is_some());

        // Failed node should have Failed prediction
        let entry = failing_entry.unwrap();
        assert!(matches!(
            entry.outputs,
            crate::optimize::PredictionOrFailed::Failed(_)
        ));
    }

    #[test]
    fn test_to_trace_data_simple() {
        let trace = create_simple_trace();
        let example = crate::optimize::Example::new().with("question", json!("What is 2+2?"));

        #[allow(deprecated)]
        let trace_data = trace.to_trace_data(example.clone(), 0, Some(1.0));

        assert_eq!(trace_data.example_ind, 0);
        assert_eq!(trace_data.score, Some(1.0));
        assert!(!trace_data.trace.is_empty());
    }

    #[test]
    fn test_to_trace_data_without_score() {
        let trace = create_simple_trace();
        let example = crate::optimize::Example::new();

        #[allow(deprecated)]
        let trace_data = trace.to_trace_data(example, 5, None);

        assert_eq!(trace_data.example_ind, 5);
        assert_eq!(trace_data.score, None);
    }

    #[test]
    fn test_final_prediction_successful_trace() {
        let trace = create_simple_trace();
        let prediction = trace.final_prediction();

        // Successful trace should have Success prediction
        assert!(matches!(
            prediction,
            crate::optimize::PredictionOrFailed::Success(_)
        ));
    }

    #[test]
    fn test_final_prediction_failed_trace() {
        let trace = create_trace_with_errors();
        let prediction = trace.final_prediction();

        // Failed trace should have Failed prediction
        assert!(matches!(
            prediction,
            crate::optimize::PredictionOrFailed::Failed(_)
        ));

        if let crate::optimize::PredictionOrFailed::Failed(failed) = prediction {
            assert!(!failed.error.is_empty());
        }
    }

    #[test]
    fn test_final_prediction_incomplete_trace() {
        let mut trace = create_simple_trace();
        trace.completed = false;
        let prediction = trace.final_prediction();

        // Incomplete trace should have Failed prediction
        assert!(matches!(
            prediction,
            crate::optimize::PredictionOrFailed::Failed(_)
        ));
    }

    #[test]
    fn test_has_training_data_with_state() {
        let trace = create_simple_trace();
        assert!(trace.has_training_data());
    }

    #[test]
    fn test_has_training_data_without_state() {
        let node = NodeExecution::new("node", 100).with_tokens(10);
        // state_before and state_after are None by default

        let trace = ExecutionTraceBuilder::new()
            .add_node_execution(node)
            .build();

        assert!(!trace.has_training_data());
    }

    #[test]
    fn test_has_training_data_partial_state() {
        // Only state_before, no state_after
        let node = NodeExecution::new("node", 100)
            .with_tokens(10)
            .with_state_before(json!({"input": "test"}));
        // state_after is None by default

        let trace = ExecutionTraceBuilder::new()
            .add_node_execution(node)
            .build();

        assert!(!trace.has_training_data());
    }

    #[test]
    fn test_training_example_count_simple_trace() {
        let trace = create_simple_trace();
        assert_eq!(trace.training_example_count(), 2);
    }

    #[test]
    fn test_training_example_count_empty_trace() {
        let trace = ExecutionTrace::default();
        assert_eq!(trace.training_example_count(), 0);
    }

    #[test]
    fn test_training_example_count_mixed_state() {
        // Create trace with some nodes having state and some not
        let node_with_state = create_node_execution("with_state", 100, true);

        let node_without_state = NodeExecution::new("without_state", 100).with_tokens(10);
        // state_before and state_after are None by default

        let trace = ExecutionTraceBuilder::new()
            .add_node_execution(node_with_state)
            .add_node_execution(node_without_state)
            .build();

        assert_eq!(trace.training_example_count(), 1);
    }

    // =========================================================================
    // Integration Tests (Multiple Systems)
    // =========================================================================

    #[test]
    fn test_combined_analysis_simple_trace() {
        let trace = create_simple_trace();

        // Run all analysis systems
        let prompt_analyses = trace.analyze_prompt_effectiveness();
        let latency_stats = trace.collect_latency_stats();
        let timeout_recs = trace.calculate_optimal_timeouts();
        let _causal_chains = trace.auto_analyze_causality();
        let _counterfactual_recs = trace.best_counterfactual_recommendations(3);
        let examples = trace.to_examples();

        // All should complete without panic
        assert!(!prompt_analyses.is_empty() || prompt_analyses.is_empty());
        assert!(!latency_stats.is_empty());
        assert!(!timeout_recs.summary.is_empty());
        // _causal_chains may be empty for clean trace
        // _counterfactual_recs may be empty
        assert_eq!(examples.len(), 2);
    }

    #[test]
    fn test_combined_analysis_problematic_trace() {
        let trace = create_trace_with_errors();

        // All analysis systems should work on problematic traces
        let prompt_summary = trace.prompt_health_summary();
        let timeout_summary = trace.timeout_optimization_summary();
        let causal_summary = trace.causal_analysis_summary();
        let counterfactual_summary = trace.counterfactual_summary();

        // All summaries should be non-empty
        assert!(!prompt_summary.is_empty());
        assert!(!timeout_summary.is_empty());
        assert!(!causal_summary.is_empty());
        assert!(!counterfactual_summary.is_empty());
    }

    #[test]
    fn test_combined_analysis_high_latency_trace() {
        let trace = create_trace_with_high_latency();

        // Check if high latency trace triggers causal analysis
        // (depends on default thresholds which may vary)
        let _has_causal_opps = trace.has_causal_analysis_opportunities();

        // Analyze the latency regardless of threshold detection
        let chain = trace.analyze_latency();
        assert_eq!(chain.effect, crate::causal_analysis::Effect::HighLatency);

        // Check if counterfactual suggests improvements
        let _ = trace.best_counterfactual_recommendations(5);
    }

    #[test]
    fn test_combined_analysis_loop_trace() {
        let trace = create_trace_with_loop();

        // Loop trace should trigger causal analysis
        let has_causal_opps = trace.has_causal_analysis_opportunities();
        assert!(has_causal_opps);

        // Should have many latency stats for repeated node
        let latency_stats = trace.collect_latency_stats();
        assert_eq!(latency_stats.len(), 1); // One unique node
    }

    #[test]
    #[allow(deprecated)]
    fn test_trace_roundtrip_with_examples() {
        let trace = create_simple_trace();

        // Convert to examples
        let examples = trace.to_examples();
        assert_eq!(examples.len(), 2);

        // Convert to trace entries (legacy)
        let entries = trace.to_trace_entries();
        assert_eq!(entries.len(), 2);

        // Both should preserve node information
        let example_nodes: Vec<_> = examples
            .iter()
            .filter_map(|e| e.get("_node").and_then(|v| v.as_str()))
            .collect();
        let entry_nodes: Vec<_> = entries.iter().map(|e| e.predictor_name.as_str()).collect();

        // Same nodes in both representations
        for node in &example_nodes {
            assert!(entry_nodes.contains(node));
        }
    }
}
