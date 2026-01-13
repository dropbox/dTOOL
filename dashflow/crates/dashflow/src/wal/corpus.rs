// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Learning Corpus API (Observability Phase 4)
//!
//! Provides APIs for agents to query their execution history and learn from
//! past decisions. Enables self-aware agents that can improve over time.
//!
//! # Architecture
//!
//! The Learning Corpus builds on top of the EventStore to provide:
//! - Decision/outcome correlation
//! - Success rate calculations
//! - Similar execution discovery
//!
//! ```text
//! ┌──────────────────┐     ┌─────────────────┐     ┌──────────────┐
//! │  LearningCorpus  │────▶│   EventStore    │────▶│   WAL/Index  │
//! └──────────────────┘     └─────────────────┘     └──────────────┘
//!         │
//!         ▼
//! ┌──────────────────┐
//! │ Decision/Outcome │
//! │   Correlation    │
//! └──────────────────┘
//! ```

use std::collections::HashMap;

use super::store::{EventStore, EventStoreResult};
use super::writer::WALEventType;

/// Summary of a decision and its outcome.
#[derive(Debug, Clone)]
pub struct DecisionOutcome {
    /// Unique decision identifier
    pub decision_id: String,
    /// Type of decision (e.g., "routing", "tool_selection")
    pub decision_type: String,
    /// The option that was chosen
    pub chosen_option: String,
    /// Whether the outcome was successful
    pub success: bool,
    /// Outcome score if available
    pub score: Option<f64>,
    /// Execution ID containing this decision
    pub execution_id: String,
    /// Latency from decision to outcome (milliseconds)
    pub latency_ms: Option<u64>,
}

/// Summary of execution with decision statistics.
#[derive(Debug, Clone)]
pub struct ExecutionWithDecisions {
    /// Execution ID
    pub execution_id: String,
    /// Task type (from metadata)
    pub task_type: Option<String>,
    /// Total decisions made in this execution
    pub decision_count: usize,
    /// Successful decision outcomes
    pub successful_outcomes: usize,
    /// Failed decision outcomes
    pub failed_outcomes: usize,
    /// Average outcome score (if scores available)
    pub avg_score: Option<f64>,
    /// Total duration in milliseconds
    pub duration_ms: i64,
    /// Whether execution completed
    pub completed: bool,
}

/// Statistics for a specific decision type.
#[derive(Debug, Clone, Default)]
pub struct DecisionTypeStats {
    /// Total decisions of this type
    pub total_decisions: usize,
    /// Decisions with observed outcomes
    pub outcomes_observed: usize,
    /// Successful outcomes
    pub successful_outcomes: usize,
    /// Failed outcomes
    pub failed_outcomes: usize,
    /// Average score across all scored outcomes
    pub avg_score: Option<f64>,
    /// Most frequently chosen option
    pub most_common_option: Option<String>,
    /// Option choice frequency (option -> count)
    pub option_frequency: HashMap<String, usize>,
}

/// Learning Corpus for agent self-improvement.
///
/// Provides query APIs for agents to learn from their execution history:
/// - Find similar past executions
/// - Calculate success rates by decision type
/// - Correlate decisions with outcomes
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::wal::{LearningCorpus, EventStore, EventStoreConfig};
///
/// let store = EventStore::new(EventStoreConfig::default())?;
/// let corpus = LearningCorpus::new(store);
///
/// // Query success rate for routing decisions
/// let rate = corpus.success_rate("routing")?;
/// println!("Routing decisions succeed {}% of the time", rate * 100.0);
///
/// // Find similar executions
/// let similar = corpus.find_similar_executions("code_generation", 10)?;
/// for exec in similar {
///     println!("{}: {} decisions, {} successful",
///         exec.execution_id, exec.decision_count, exec.successful_outcomes);
/// }
/// ```
#[derive(Debug)]
pub struct LearningCorpus {
    store: EventStore,
}

impl LearningCorpus {
    /// Create a new Learning Corpus backed by the given event store.
    pub fn new(store: EventStore) -> Self {
        Self { store }
    }

    /// Create a Learning Corpus with default configuration from environment.
    pub fn from_env() -> EventStoreResult<Self> {
        Ok(Self::new(EventStore::from_env()?))
    }

    /// Calculate the success rate for a specific decision type.
    ///
    /// Returns a value between 0.0 and 1.0 representing the proportion of
    /// decisions that led to successful outcomes.
    ///
    /// # Arguments
    ///
    /// * `decision_type` - The type of decision to analyze (e.g., "routing", "tool_selection")
    ///
    /// # Returns
    ///
    /// Success rate as a float (0.0 to 1.0), or 0.0 if no outcomes observed.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let rate = corpus.success_rate("tool_selection")?;
    /// if rate < 0.5 {
    ///     println!("Tool selection is failing too often!");
    /// }
    /// ```
    pub fn success_rate(&self, decision_type: &str) -> EventStoreResult<f64> {
        let stats = self.decision_type_stats(decision_type)?;

        if stats.outcomes_observed == 0 {
            return Ok(0.0);
        }

        Ok(stats.successful_outcomes as f64 / stats.outcomes_observed as f64)
    }

    /// Get detailed statistics for a decision type.
    ///
    /// Returns comprehensive statistics including success rate, average score,
    /// and option frequency distribution.
    pub fn decision_type_stats(&self, decision_type: &str) -> EventStoreResult<DecisionTypeStats> {
        let mut stats = DecisionTypeStats::default();
        let mut scores: Vec<f64> = Vec::new();

        // Get recent executions and analyze their decision events
        let executions = self.store.recent_executions(1000)?;

        for exec_summary in executions {
            // Get events for this execution
            let events = match self.store.execution_events(&exec_summary.execution_id) {
                Ok(events) => events,
                Err(_) => continue,
            };

            // Track decisions and outcomes by decision_id
            let mut decisions: HashMap<String, (String, String)> = HashMap::new(); // id -> (type, option)
            let mut outcomes: HashMap<String, (bool, Option<f64>)> = HashMap::new(); // id -> (success, score)

            for event in events {
                match event.event_type {
                    WALEventType::DecisionMade => {
                        if let Some(payload) = event.payload.as_object() {
                            let dec_type = payload
                                .get("decision_type")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let dec_id = payload
                                .get("decision_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let option = payload
                                .get("chosen_option")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");

                            if dec_type == decision_type {
                                decisions
                                    .insert(dec_id.to_string(), (dec_type.to_string(), option.to_string()));
                            }
                        }
                    }
                    WALEventType::OutcomeObserved => {
                        if let Some(payload) = event.payload.as_object() {
                            let dec_id = payload
                                .get("decision_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let success = payload
                                .get("success")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            let score = payload.get("score").and_then(|v| v.as_f64());

                            outcomes.insert(dec_id.to_string(), (success, score));
                        }
                    }
                    _ => {}
                }
            }

            // Correlate decisions with outcomes
            for (dec_id, (_, option)) in &decisions {
                stats.total_decisions += 1;
                *stats.option_frequency.entry(option.clone()).or_insert(0) += 1;

                if let Some((success, score)) = outcomes.get(dec_id) {
                    stats.outcomes_observed += 1;
                    if *success {
                        stats.successful_outcomes += 1;
                    } else {
                        stats.failed_outcomes += 1;
                    }
                    if let Some(s) = score {
                        scores.push(*s);
                    }
                }
            }
        }

        // Calculate average score
        if !scores.is_empty() {
            stats.avg_score = Some(scores.iter().sum::<f64>() / scores.len() as f64);
        }

        // Find most common option
        stats.most_common_option = stats
            .option_frequency
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(option, _)| option.clone());

        Ok(stats)
    }

    /// Find executions similar to a given task type.
    ///
    /// Returns executions that match the specified task type, ordered by
    /// most recent first. Task type is determined from execution metadata.
    ///
    /// # Arguments
    ///
    /// * `task_type` - The task type to search for (e.g., "code_generation")
    /// * `limit` - Maximum number of results
    ///
    /// # Returns
    ///
    /// Vector of execution summaries with decision statistics.
    pub fn find_similar_executions(
        &self,
        task_type: &str,
        limit: usize,
    ) -> EventStoreResult<Vec<ExecutionWithDecisions>> {
        let mut results = Vec::new();
        let executions = self.store.recent_executions(limit * 10)?; // Over-fetch to filter

        for exec_summary in executions {
            if results.len() >= limit {
                break;
            }

            // Get events to check task type and count decisions
            let events = match self.store.execution_events(&exec_summary.execution_id) {
                Ok(events) => events,
                Err(_) => continue,
            };

            // Check task type from execution trace metadata
            let mut exec_task_type: Option<String> = None;
            let mut decision_count = 0;
            let mut successful_outcomes = 0;
            let mut failed_outcomes = 0;
            let mut scores: Vec<f64> = Vec::new();
            let mut decision_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
            let mut outcome_decisions: std::collections::HashSet<String> = std::collections::HashSet::new();

            for event in &events {
                match event.event_type {
                    WALEventType::ExecutionTrace => {
                        if let Some(payload) = event.payload.as_object() {
                            if let Some(metadata) = payload.get("metadata").and_then(|m| m.as_object()) {
                                exec_task_type = metadata
                                    .get("task_type")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                            }
                        }
                    }
                    WALEventType::DecisionMade => {
                        if let Some(payload) = event.payload.as_object() {
                            if let Some(dec_id) = payload.get("decision_id").and_then(|v| v.as_str()) {
                                decision_ids.insert(dec_id.to_string());
                                decision_count += 1;
                            }
                        }
                    }
                    WALEventType::OutcomeObserved => {
                        if let Some(payload) = event.payload.as_object() {
                            let dec_id = payload
                                .get("decision_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let success = payload
                                .get("success")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            let score = payload.get("score").and_then(|v| v.as_f64());

                            if decision_ids.contains(dec_id) && !outcome_decisions.contains(dec_id) {
                                outcome_decisions.insert(dec_id.to_string());
                                if success {
                                    successful_outcomes += 1;
                                } else {
                                    failed_outcomes += 1;
                                }
                                if let Some(s) = score {
                                    scores.push(s);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            // Filter by task type
            let matches = exec_task_type
                .as_ref()
                .map(|t| t == task_type)
                .unwrap_or(false);

            if matches {
                let avg_score = if !scores.is_empty() {
                    Some(scores.iter().sum::<f64>() / scores.len() as f64)
                } else {
                    None
                };

                results.push(ExecutionWithDecisions {
                    execution_id: exec_summary.execution_id,
                    task_type: exec_task_type,
                    decision_count,
                    successful_outcomes,
                    failed_outcomes,
                    avg_score,
                    duration_ms: exec_summary.duration_ms,
                    completed: exec_summary.completed,
                });
            }
        }

        Ok(results)
    }

    /// Get all decision-outcome pairs for analysis.
    ///
    /// Returns correlated decisions and outcomes from recent executions.
    /// Useful for detailed analysis of agent behavior patterns.
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of executions to scan
    pub fn decision_outcomes(&self, limit: usize) -> EventStoreResult<Vec<DecisionOutcome>> {
        let mut results = Vec::new();
        let executions = self.store.recent_executions(limit)?;

        for exec_summary in executions {
            let events = match self.store.execution_events(&exec_summary.execution_id) {
                Ok(events) => events,
                Err(_) => continue,
            };

            // Collect decisions and outcomes
            let mut decisions: HashMap<String, (String, String, u64)> = HashMap::new(); // id -> (type, option, timestamp)
            let mut outcomes: HashMap<String, (bool, Option<f64>, u64)> = HashMap::new(); // id -> (success, score, timestamp)

            for event in events {
                match event.event_type {
                    WALEventType::DecisionMade => {
                        if let Some(payload) = event.payload.as_object() {
                            let dec_id = payload
                                .get("decision_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let dec_type = payload
                                .get("decision_type")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let option = payload
                                .get("chosen_option")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();

                            decisions.insert(dec_id, (dec_type, option, event.timestamp_ms));
                        }
                    }
                    WALEventType::OutcomeObserved => {
                        if let Some(payload) = event.payload.as_object() {
                            let dec_id = payload
                                .get("decision_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let success = payload
                                .get("success")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            let score = payload.get("score").and_then(|v| v.as_f64());

                            outcomes.insert(dec_id, (success, score, event.timestamp_ms));
                        }
                    }
                    _ => {}
                }
            }

            // Correlate decisions with outcomes
            for (dec_id, (dec_type, option, dec_ts)) in decisions {
                if let Some((success, score, outcome_ts)) = outcomes.get(&dec_id) {
                    let latency_ms = if *outcome_ts > dec_ts {
                        Some(*outcome_ts - dec_ts)
                    } else {
                        None
                    };

                    results.push(DecisionOutcome {
                        decision_id: dec_id,
                        decision_type: dec_type,
                        chosen_option: option,
                        success: *success,
                        score: *score,
                        execution_id: exec_summary.execution_id.clone(),
                        latency_ms,
                    });
                }
            }
        }

        Ok(results)
    }

    /// Get a reference to the underlying event store.
    pub fn store(&self) -> &EventStore {
        &self.store
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::store::EventStoreConfig;
    use super::super::writer::{WALEvent, WALWriterConfig};
    use crate::introspection::ExecutionTrace;
    use std::collections::HashMap;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tempfile::TempDir;

    fn test_config(temp: &TempDir) -> EventStoreConfig {
        EventStoreConfig {
            wal: WALWriterConfig {
                wal_dir: temp.path().join("wal"),
                max_segment_bytes: 1024 * 1024,
                fsync_on_write: false,
                segment_extension: ".wal".to_string(),
            },
            index_path: temp.path().join("index.db"),
            auto_compaction: false, // Disable for tests to avoid background threads
        }
    }

    fn timestamp_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn write_decision_event(
        store: &EventStore,
        execution_id: &str,
        decision_id: &str,
        decision_type: &str,
        chosen_option: &str,
    ) -> EventStoreResult<()> {
        let event = WALEvent {
            timestamp_ms: timestamp_ms(),
            event_type: WALEventType::DecisionMade,
            execution_id: Some(execution_id.to_string()),
            parent_execution_id: None,
            root_execution_id: None,
            depth: Some(0),
            payload: serde_json::json!({
                "decision_id": decision_id,
                "decision_type": decision_type,
                "decision_maker": "test_agent",
                "chosen_option": chosen_option,
                "alternatives_considered": [],
                "confidence": 0.9,
                "reasoning": "test decision",
                "context": {}
            }),
        };
        store.writer().write_event(&event)?;
        Ok(())
    }

    fn write_outcome_event(
        store: &EventStore,
        execution_id: &str,
        decision_id: &str,
        success: bool,
        score: Option<f64>,
    ) -> EventStoreResult<()> {
        let event = WALEvent {
            timestamp_ms: timestamp_ms(),
            event_type: WALEventType::OutcomeObserved,
            execution_id: Some(execution_id.to_string()),
            parent_execution_id: None,
            root_execution_id: None,
            depth: Some(0),
            payload: serde_json::json!({
                "decision_id": decision_id,
                "success": success,
                "score": score,
                "outcome_description": if success { "Success" } else { "Failure" },
                "latency_ms": 100,
                "metrics": {}
            }),
        };
        store.writer().write_event(&event)?;
        Ok(())
    }

    fn write_trace_with_task_type(
        store: &EventStore,
        execution_id: &str,
        task_type: &str,
    ) -> EventStoreResult<()> {
        let mut metadata = HashMap::new();
        metadata.insert("task_type".to_string(), serde_json::json!(task_type));

        let trace = ExecutionTrace {
            thread_id: Some("test-thread".to_string()),
            execution_id: Some(execution_id.to_string()),
            parent_execution_id: None,
            root_execution_id: None,
            depth: Some(0),
            nodes_executed: vec![],
            total_duration_ms: 100,
            total_tokens: 50,
            errors: vec![],
            completed: true,
            started_at: Some("2025-01-01T00:00:00Z".to_string()),
            ended_at: Some("2025-01-01T00:00:01Z".to_string()),
            final_state: None,
            metadata,
            execution_metrics: None,
            performance_metrics: None,
        };

        store.write_trace(&trace)?;
        Ok(())
    }

    #[test]
    fn test_corpus_creation() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);
        let store = EventStore::new(config).unwrap();
        let _corpus = LearningCorpus::new(store);
    }

    #[test]
    fn test_success_rate_no_decisions() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);
        let store = EventStore::new(config).unwrap();
        let corpus = LearningCorpus::new(store);

        let rate = corpus.success_rate("nonexistent_type").unwrap();
        assert_eq!(rate, 0.0);
    }

    #[test]
    fn test_success_rate_all_successful() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);
        let store = EventStore::new(config).unwrap();

        // Write trace first
        write_trace_with_task_type(&store, "exec-1", "test_task").unwrap();

        // Write decisions with successful outcomes
        for i in 0..5 {
            write_decision_event(&store, "exec-1", &format!("dec-{i}"), "routing", "option_a").unwrap();
            write_outcome_event(&store, "exec-1", &format!("dec-{i}"), true, Some(0.9)).unwrap();
        }
        store.flush().unwrap();

        let corpus = LearningCorpus::new(store);
        let rate = corpus.success_rate("routing").unwrap();
        assert!((rate - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_success_rate_mixed() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);
        let store = EventStore::new(config).unwrap();

        // Write trace
        write_trace_with_task_type(&store, "exec-1", "test_task").unwrap();

        // 3 successful, 2 failed = 60% success rate
        write_decision_event(&store, "exec-1", "dec-1", "tool_selection", "search").unwrap();
        write_outcome_event(&store, "exec-1", "dec-1", true, Some(0.9)).unwrap();

        write_decision_event(&store, "exec-1", "dec-2", "tool_selection", "search").unwrap();
        write_outcome_event(&store, "exec-1", "dec-2", true, Some(0.8)).unwrap();

        write_decision_event(&store, "exec-1", "dec-3", "tool_selection", "code").unwrap();
        write_outcome_event(&store, "exec-1", "dec-3", true, Some(0.7)).unwrap();

        write_decision_event(&store, "exec-1", "dec-4", "tool_selection", "code").unwrap();
        write_outcome_event(&store, "exec-1", "dec-4", false, Some(0.2)).unwrap();

        write_decision_event(&store, "exec-1", "dec-5", "tool_selection", "search").unwrap();
        write_outcome_event(&store, "exec-1", "dec-5", false, Some(0.3)).unwrap();

        store.flush().unwrap();

        let corpus = LearningCorpus::new(store);
        let rate = corpus.success_rate("tool_selection").unwrap();
        assert!((rate - 0.6).abs() < 0.001);
    }

    #[test]
    fn test_decision_type_stats() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);
        let store = EventStore::new(config).unwrap();

        write_trace_with_task_type(&store, "exec-1", "test_task").unwrap();

        // Add decisions with different options
        write_decision_event(&store, "exec-1", "dec-1", "routing", "path_a").unwrap();
        write_outcome_event(&store, "exec-1", "dec-1", true, Some(0.9)).unwrap();

        write_decision_event(&store, "exec-1", "dec-2", "routing", "path_a").unwrap();
        write_outcome_event(&store, "exec-1", "dec-2", true, Some(0.8)).unwrap();

        write_decision_event(&store, "exec-1", "dec-3", "routing", "path_b").unwrap();
        write_outcome_event(&store, "exec-1", "dec-3", false, Some(0.3)).unwrap();

        store.flush().unwrap();

        let corpus = LearningCorpus::new(store);
        let stats = corpus.decision_type_stats("routing").unwrap();

        assert_eq!(stats.total_decisions, 3);
        assert_eq!(stats.outcomes_observed, 3);
        assert_eq!(stats.successful_outcomes, 2);
        assert_eq!(stats.failed_outcomes, 1);
        assert_eq!(stats.most_common_option, Some("path_a".to_string()));
        assert_eq!(stats.option_frequency.get("path_a"), Some(&2));
        assert_eq!(stats.option_frequency.get("path_b"), Some(&1));
    }

    #[test]
    fn test_find_similar_executions() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);
        let store = EventStore::new(config).unwrap();

        // Write executions with different task types
        write_trace_with_task_type(&store, "exec-code-1", "code_generation").unwrap();
        write_decision_event(&store, "exec-code-1", "dec-1", "strategy", "llm").unwrap();
        write_outcome_event(&store, "exec-code-1", "dec-1", true, Some(0.9)).unwrap();

        write_trace_with_task_type(&store, "exec-code-2", "code_generation").unwrap();
        write_decision_event(&store, "exec-code-2", "dec-2", "strategy", "template").unwrap();
        write_outcome_event(&store, "exec-code-2", "dec-2", false, Some(0.4)).unwrap();

        write_trace_with_task_type(&store, "exec-search-1", "search_task").unwrap();
        write_decision_event(&store, "exec-search-1", "dec-3", "strategy", "vector").unwrap();
        write_outcome_event(&store, "exec-search-1", "dec-3", true, Some(0.8)).unwrap();

        store.flush().unwrap();

        let corpus = LearningCorpus::new(store);
        let similar = corpus.find_similar_executions("code_generation", 10).unwrap();

        assert_eq!(similar.len(), 2);
        assert!(similar.iter().all(|e| e.task_type.as_deref() == Some("code_generation")));
    }

    #[test]
    fn test_decision_outcomes() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);
        let store = EventStore::new(config).unwrap();

        write_trace_with_task_type(&store, "exec-1", "test_task").unwrap();

        write_decision_event(&store, "exec-1", "dec-1", "routing", "path_a").unwrap();
        write_outcome_event(&store, "exec-1", "dec-1", true, Some(0.9)).unwrap();

        write_decision_event(&store, "exec-1", "dec-2", "tool_selection", "search").unwrap();
        write_outcome_event(&store, "exec-1", "dec-2", false, Some(0.3)).unwrap();

        store.flush().unwrap();

        let corpus = LearningCorpus::new(store);
        let outcomes = corpus.decision_outcomes(10).unwrap();

        assert_eq!(outcomes.len(), 2);

        let routing_outcome = outcomes.iter().find(|o| o.decision_id == "dec-1").unwrap();
        assert_eq!(routing_outcome.decision_type, "routing");
        assert!(routing_outcome.success);
        assert_eq!(routing_outcome.score, Some(0.9));

        let tool_outcome = outcomes.iter().find(|o| o.decision_id == "dec-2").unwrap();
        assert_eq!(tool_outcome.decision_type, "tool_selection");
        assert!(!tool_outcome.success);
        assert_eq!(tool_outcome.score, Some(0.3));
    }
}
