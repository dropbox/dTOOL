//! StateGraph-based search workflow for Librarian.
//!
//! This module exists to make Librarian a true DashFlow paragon by using:
//! - DashFlow's StateGraph runtime for orchestration
//! - `dashflow::generate()` for LLM calls (automatic telemetry)

use crate::{fan_out::SearchStrategy, AnswerSynthesizer, FanOutSearcher, SearchResult};
use anyhow::Result;
use dashflow::core::language_models::ChatModel;
use dashflow::{CompiledGraph, Error as DashflowError, Result as DashflowResult, StateGraph, END};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Serializable timing info for a single fan-out strategy.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StrategyTiming {
    /// Strategy identifier.
    pub strategy: SearchStrategy,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Result count returned by that strategy.
    pub result_count: usize,
}

/// State flowing through the Librarian query workflow graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryWorkflowState {
    /// User query.
    pub query: String,
    /// Per-strategy result limit (input to fan-out).
    pub limit: usize,
    /// Strategies to execute.
    pub strategies: Vec<SearchStrategy>,
    /// Final merged results.
    pub results: Vec<SearchResult>,
    /// Per-strategy timings (best-effort).
    pub timings: Vec<StrategyTiming>,
    /// Whether to synthesize an answer from results.
    pub synthesize: bool,
    /// Synthesized answer (if requested).
    pub answer: Option<String>,
}

impl QueryWorkflowState {
    /// Create an initial state for running the query workflow.
    pub fn new(query: String, limit: usize, strategies: Vec<SearchStrategy>, synthesize: bool) -> Self {
        Self {
            query,
            limit,
            strategies,
            results: Vec::new(),
            timings: Vec::new(),
            synthesize,
            answer: None,
        }
    }
}

impl dashflow::state::MergeableState for QueryWorkflowState {
    fn merge(&mut self, other: &Self) {
        // Conservative merge semantics: keep the original query/limit, merge results/timings.
        self.synthesize = self.synthesize || other.synthesize;
        if self.answer.is_none() {
            self.answer = other.answer.clone();
        }

        if self.strategies.is_empty() && !other.strategies.is_empty() {
            self.strategies = other.strategies.clone();
        }

        self.timings.extend(other.timings.iter().cloned());

        let mut combined = Vec::with_capacity(self.results.len() + other.results.len());
        combined.extend(self.results.iter().cloned());
        combined.extend(other.results.iter().cloned());

        combined.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut seen: HashMap<(String, i64), usize> = HashMap::new();
        let mut merged: Vec<SearchResult> = Vec::new();

        for result in combined {
            let key = (result.book_id.clone(), result.chunk_index);
            if let Some(&idx) = seen.get(&key) {
                if result.score > merged[idx].score {
                    merged[idx] = result;
                }
            } else {
                seen.insert(key, merged.len());
                merged.push(result);
            }
        }

        if merged.len() > self.limit {
            merged.truncate(self.limit);
        }

        self.results = merged;
    }
}

/// Build a StateGraph that runs: fan_out → analyze → synthesize.
pub fn build_query_workflow_graph(
    fan_out: &Arc<FanOutSearcher>,
    model: Option<&Arc<dyn ChatModel>>,
) -> Result<CompiledGraph<QueryWorkflowState>> {
    let mut graph: StateGraph<QueryWorkflowState> = StateGraph::new();

    let fan_out_node = {
        let fan_out = Arc::clone(fan_out);
        move |mut state: QueryWorkflowState| -> std::pin::Pin<
            Box<dyn std::future::Future<Output = DashflowResult<QueryWorkflowState>> + Send>,
        > {
            let fan_out = Arc::clone(&fan_out);
            Box::pin(async move {
                let result = fan_out
                    .search(&state.query, state.strategies.clone(), state.limit)
                    .await
                    .map_err(|e| DashflowError::NodeExecution {
                        node: "fan_out".to_string(),
                        source: Box::new(std::io::Error::other(e.to_string())),
                    })?;

                state.timings = result
                    .strategy_results
                    .iter()
                    .map(|sr| StrategyTiming {
                        strategy: sr.strategy.clone(),
                        duration_ms: sr.execution_time.as_millis() as u64,
                        result_count: sr.results.len(),
                    })
                    .collect();

                state.results = result.results;
                Ok(state)
            })
        }
    };

    let analyze_node = |mut state: QueryWorkflowState| -> std::pin::Pin<
        Box<dyn std::future::Future<Output = DashflowResult<QueryWorkflowState>> + Send>,
    > {
        Box::pin(async move {
            // FanOutSearcher already deduplicates and sorts; this final pass enforces
            // the user's overall limit (instead of per-strategy limit).
            if state.results.len() > state.limit {
                state.results.truncate(state.limit);
            }
            Ok(state)
        })
    };

    let synthesize_node = {
        let model = model.map(Arc::clone);
        move |mut state: QueryWorkflowState| -> std::pin::Pin<
            Box<dyn std::future::Future<Output = DashflowResult<QueryWorkflowState>> + Send>,
        > {
            let model = model.as_ref().map(Arc::clone);
            Box::pin(async move {
                if !state.synthesize || state.results.is_empty() {
                    return Ok(state);
                }

                let Some(model) = model else {
                    return Ok(state);
                };

                let synthesizer = AnswerSynthesizer::new(model);
                let answer = synthesizer
                    .synthesize(&state.query, &state.results)
                    .await
                    .map_err(|e| DashflowError::NodeExecution {
                        node: "synthesize".to_string(),
                        source: Box::new(std::io::Error::other(e.to_string())),
                    })?;
                state.answer = Some(answer);

                Ok(state)
            })
        }
    };

    graph.add_node_from_fn("fan_out", fan_out_node);
    graph.add_node_from_fn("analyze", analyze_node);
    graph.add_node_from_fn("synthesize", synthesize_node);

    graph.set_entry_point("fan_out");
    graph.add_edge("fan_out", "analyze");
    graph.add_edge("analyze", "synthesize");
    graph.add_edge("synthesize", END);

    Ok(graph.compile()?)
}
