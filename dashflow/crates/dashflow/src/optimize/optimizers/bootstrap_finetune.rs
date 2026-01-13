//! # BootstrapFinetune - Fine-Tuning Dataset Preparation
//!
//! BootstrapFinetune uses DashStream traces to generate fine-tuning datasets.
//! Instead of runtime interception, it consumes Kafka events after execution
//! to collect (input, output) pairs for model fine-tuning.
//!
//! NOTE: This module uses deprecated `TraceCollector` and `TraceEntry` types
//! from the trace module. These are intentionally allowed as they form the
//! core of the optimizer's trace collection API. Migration to ExecutionTrace
//! is tracked separately.

#![allow(deprecated)]
//!
//! ## Key Design:
//!
//! 1. **Execute graph** on training data (with DashStreamCallback)
//! 2. **Collect traces** from Kafka via TraceCollector
//! 3. **Filter by metric** (keep only successful examples)
//! 4. **Export to JSONL** in OpenAI fine-tuning format
//!
//! ## Example:
//!
//! ```rust,ignore
//! use dashflow::optimize::optimizers::BootstrapFinetune;
//! use dashflow::optimize::Example;
//!
//! // Create optimizer
//! let optimizer = BootstrapFinetune::new()
//!     .with_kafka_brokers("localhost:9092")
//!     .with_metric(accuracy_metric)
//!     .with_min_score(0.8);
//!
//! // Prepare training data
//! let trainset = vec![
//!     Example::new().with("query", "What is Rust?"),
//!     Example::new().with("query", "Explain async/await"),
//! ];
//!
//! // Run optimization (executes graph, collects traces, exports dataset)
//! optimizer.compile(graph, trainset).await?;
//!
//! // Result: finetune_dataset.jsonl created
//! ```
//!
//! ## References
//!
//! - **Source**: DSPy framework
//! - **Paper**: "DSPy: Compiling Declarative Language Model Calls into Self-Improving Pipelines"
//! - **Link**: <https://arxiv.org/abs/2310.03714>
//! - **Original**: <https://github.com/stanfordnlp/dspy/blob/main/dspy/teleprompt/>

use crate::optimize::example::Example;
use crate::optimize::telemetry::{
    record_iteration, record_optimization_complete, record_optimization_start,
};
use crate::optimize::trace::TraceCollector;
use crate::optimize::trace_types::{Prediction, PredictionOrFailed, TraceData};
use crate::StateGraph;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;

/// Error types for BootstrapFinetune
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BootstrapFinetuneError {
    /// Failed to collect execution traces from DashStream/Kafka.
    ///
    /// Check that the Kafka broker and topic are accessible.
    #[error("Trace collection failed: {0}")]
    TraceCollection(String),

    /// The graph execution failed for one or more examples.
    #[error("Graph execution failed: {0}")]
    GraphExecution(String),

    /// File I/O operation failed (reading/writing JSONL files).
    #[error("File I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization or deserialization failed.
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    /// No valid traces were collected.
    ///
    /// This occurs when all examples failed or were filtered out by the metric.
    /// Try using more examples or adjusting the quality threshold.
    #[error("No traces collected (all examples failed or filtered out)")]
    NoTraces,
}

/// Result type for bootstrap finetune operations.
pub type Result<T> = std::result::Result<T, BootstrapFinetuneError>;

/// Metric function for evaluating examples
///
/// Takes example, prediction, and trace, returns score 0.0-1.0
pub type MetricFn = Arc<
    dyn Fn(&Example, &Prediction, Option<&Vec<crate::optimize::trace_types::TraceEntry>>) -> f64
        + Send
        + Sync,
>;

/// BootstrapFinetune optimizer
///
/// Collects execution traces from DashStream and prepares fine-tuning datasets.
///
/// # Architecture:
///
/// ```text
/// Graph Execution → DashStream → Kafka
///                                  ↓
///                          TraceCollector
///                                  ↓
///                         Filter by Metric
///                                  ↓
///                      Export to JSONL Format
/// ```
///
/// # Benefits over DashOpt:
///
/// - ✅ Zero runtime overhead (async event logging)
/// - ✅ Decoupled (trace collection doesn't affect execution)
/// - ✅ Persistent (Kafka stores events for replay)
/// - ✅ Scalable (Kafka handles high volume)
pub struct BootstrapFinetune {
    /// Optional metric to filter training examples
    metric: Option<MetricFn>,

    /// Minimum score threshold (examples below this are filtered out)
    min_score: f64,

    /// Kafka broker address
    kafka_brokers: String,

    /// Kafka topic name
    topic: String,

    /// Output path for fine-tuning dataset
    output_path: PathBuf,
}

impl Default for BootstrapFinetune {
    fn default() -> Self {
        Self {
            metric: None,
            min_score: 0.0,
            kafka_brokers: "localhost:9092".to_string(),
            topic: "dashstream-events".to_string(),
            output_path: PathBuf::from("finetune_dataset.jsonl"),
        }
    }
}

impl BootstrapFinetune {
    /// Create a new BootstrapFinetune optimizer with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set metric function for filtering examples
    ///
    /// Only examples with score >= min_score will be included in the dataset.
    #[must_use]
    pub fn with_metric(mut self, metric: MetricFn) -> Self {
        self.metric = Some(metric);
        self
    }

    /// Set minimum score threshold (default: 0.0)
    #[must_use]
    pub fn with_min_score(mut self, min_score: f64) -> Self {
        self.min_score = min_score;
        self
    }

    /// Set Kafka broker address (default: "localhost:9092")
    #[must_use]
    pub fn with_kafka_brokers(mut self, brokers: impl Into<String>) -> Self {
        self.kafka_brokers = brokers.into();
        self
    }

    /// Set Kafka topic name (default: "dashstream-events")
    #[must_use]
    pub fn with_topic(mut self, topic: impl Into<String>) -> Self {
        self.topic = topic.into();
        self
    }

    /// Set output path for fine-tuning dataset (default: "finetune_dataset.jsonl")
    #[must_use]
    pub fn with_output_path(mut self, path: PathBuf) -> Self {
        self.output_path = path;
        self
    }

    /// Run optimization: execute graph on training data, collect traces, export dataset
    ///
    /// # Arguments
    ///
    /// * `graph` - The StateGraph to execute (will be compiled internally)
    /// * `trainset` - Training examples
    ///
    /// # Returns
    ///
    /// The compiled graph (unchanged) and the path to the exported dataset
    ///
    /// # Process:
    ///
    /// 1. Compile the graph to get executable CompiledGraph
    /// 2. Create TraceCollector for Kafka consumption
    /// 3. For each training example:
    ///    - Execute compiled graph with unique thread_id
    ///    - Collect traces from Kafka
    ///    - Evaluate with metric (if provided)
    ///    - Keep traces that meet min_score threshold
    /// 4. Export successful traces to JSONL format
    pub async fn compile<S>(
        &self,
        graph: StateGraph<S>,
        trainset: Vec<Example>,
    ) -> Result<(crate::executor::CompiledGraph<S>, PathBuf)>
    where
        S: crate::state::MergeableState + Clone + Send + Sync + 'static,
        S: TryFrom<Example> + Into<Example>,
        <S as TryFrom<Example>>::Error: std::fmt::Display,
    {
        use std::time::Instant;
        let start = Instant::now();

        // Record telemetry start
        record_optimization_start("bootstrap_finetune");

        tracing::info!("BootstrapFinetune: Starting optimization...");
        tracing::debug!("  Training examples: {}", trainset.len());
        tracing::debug!("  Kafka brokers: {}", self.kafka_brokers);
        tracing::debug!("  Topic: {}", self.topic);
        tracing::debug!(
            "  Metric: {}",
            if self.metric.is_some() {
                "enabled"
            } else {
                "disabled"
            }
        );
        tracing::debug!("  Min score: {}", self.min_score);

        // Compile graph first
        tracing::debug!("  Compiling graph...");
        let compiled = graph
            .compile()
            .map_err(|e| BootstrapFinetuneError::GraphExecution(e.to_string()))?;

        // Create trace collector
        let mut collector = TraceCollector::new(&self.kafka_brokers, &self.topic)
            .await
            .map_err(|e| BootstrapFinetuneError::TraceCollection(e.to_string()))?;

        // Collect traces from training examples
        let mut trace_data = Vec::new();

        for (i, example) in trainset.iter().enumerate() {
            let thread_id = format!("bootstrap-finetune-{}", i);

            // Record iteration telemetry
            record_iteration("bootstrap_finetune");

            // Convert example to initial state
            let initial_state = S::try_from(example.clone())
                .map_err(|e| BootstrapFinetuneError::GraphExecution(e.to_string()))?;

            // Execute compiled graph (events automatically logged to Kafka)
            tracing::debug!("  Executing example {}/{}...", i + 1, trainset.len());
            let execution_result = compiled
                .invoke(initial_state)
                .await
                .map_err(|e| BootstrapFinetuneError::GraphExecution(e.to_string()))?;

            // Collect traces from Kafka
            let trace = collector
                .collect_for_thread(&thread_id)
                .await
                .map_err(|e| BootstrapFinetuneError::TraceCollection(e.to_string()))?;

            if trace.is_empty() {
                tracing::warn!("    No trace collected for example {}", i);
                continue;
            }

            // Convert final state to prediction
            let final_example: Example = execution_result.final_state.into();
            let mut prediction = Prediction::new();
            for (key, value) in final_example.data().iter() {
                prediction.fields.insert(key.clone(), value.clone());
            }

            // Evaluate with metric if provided
            let score = if let Some(metric) = &self.metric {
                let score = metric(example, &prediction, Some(&trace));
                tracing::debug!("    Score: {:.4}", score);
                score
            } else {
                1.0 // No filtering
            };

            // Keep traces that meet threshold
            if score >= self.min_score {
                trace_data.push(TraceData {
                    example_ind: i,
                    example: example.clone(),
                    prediction: PredictionOrFailed::Success(prediction),
                    trace,
                    score: Some(score),
                });
                tracing::debug!("    Included in dataset");
            } else {
                tracing::debug!("    Filtered out (score < {})", self.min_score);
            }
        }

        if trace_data.is_empty() {
            return Err(BootstrapFinetuneError::NoTraces);
        }

        tracing::info!(
            "Collected {} successful traces ({}% success rate)",
            trace_data.len(),
            (trace_data.len() * 100) / trainset.len()
        );

        // Export to JSONL
        tracing::info!("Exporting fine-tuning dataset to {:?}...", self.output_path);
        export_to_jsonl(&trace_data, &self.output_path)?;

        tracing::info!("BootstrapFinetune complete!");
        tracing::debug!("  Dataset: {:?}", self.output_path);
        tracing::debug!("  Examples: {}", trace_data.len());

        // Record telemetry completion
        // For fine-tuning, we don't have a traditional score improvement metric
        let success_rate = trace_data.len() as f64 / trainset.len() as f64;
        record_optimization_complete(
            "bootstrap_finetune",
            trainset.len() as u64,
            trace_data.len() as u64,
            0.0, // No initial score for fine-tuning prep
            success_rate,
            start.elapsed().as_secs_f64(),
        );

        Ok((compiled, self.output_path.clone()))
    }
}

/// Export trace data to OpenAI fine-tuning JSONL format
///
/// Format: One JSON object per line with "messages" array
///
/// ```json
/// {"messages": [
///   {"role": "system", "content": "You are a helpful assistant."},
///   {"role": "user", "content": "What is Rust?"},
///   {"role": "assistant", "content": "A systems programming language..."}
/// ]}
/// ```
fn export_to_jsonl(trace_data: &[TraceData], output_path: &PathBuf) -> Result<()> {
    use std::io::Write;

    let mut file = std::fs::File::create(output_path)?;

    for trace in trace_data {
        // Convert trace to fine-tuning format
        let finetune_entry = trace_to_finetune_entry(trace)?;

        // Write as single-line JSON
        let json_line = serde_json::to_string(&finetune_entry)?;
        writeln!(file, "{}", json_line)?;
    }

    Ok(())
}

/// Convert TraceData to OpenAI fine-tuning format
///
/// Extracts (input, output) pairs from trace entries and formats as messages.
fn trace_to_finetune_entry(trace: &TraceData) -> Result<FinetuneEntry> {
    let mut messages = Vec::new();

    // Add system message if available
    messages.push(Message {
        role: "system".to_string(),
        content: "You are a helpful AI assistant.".to_string(),
    });

    // Extract first trace entry as the primary (input, output) pair
    // In a multi-node graph, we typically want to fine-tune the final LLM call
    if let Some(first_entry) = trace.trace.first() {
        // Build user message from inputs
        let user_content = if first_entry.inputs.is_empty() {
            // Fallback to example inputs
            format!("{:?}", trace.example.data())
        } else {
            // Format inputs as natural text
            format_inputs_as_text(&first_entry.inputs)
        };

        messages.push(Message {
            role: "user".to_string(),
            content: user_content,
        });

        // Build assistant message from outputs
        if let PredictionOrFailed::Success(prediction) = &first_entry.outputs {
            let assistant_content = format_prediction_as_text(prediction);
            messages.push(Message {
                role: "assistant".to_string(),
                content: assistant_content,
            });
        }
    }

    Ok(FinetuneEntry { messages })
}

/// Format inputs as natural text for user message
///
/// M-906: Keys are sorted alphabetically for reproducible output across runs.
/// This ensures fine-tuning datasets are deterministic and comparable.
fn format_inputs_as_text(inputs: &HashMap<String, serde_json::Value>) -> String {
    // Collect and sort keys for reproducible output
    let mut keys: Vec<_> = inputs.keys().collect();
    keys.sort();

    // Simple formatting: "field1: value1\nfield2: value2"
    keys.iter()
        .filter_map(|key| {
            inputs.get(*key).map(|value| {
                let value_str = match value {
                    serde_json::Value::String(s) => s.clone(),
                    _ => value.to_string(),
                };
                format!("{}: {}", key, value_str)
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format prediction as natural text for assistant message
///
/// M-906: Keys are sorted alphabetically for reproducible output (consistent with format_inputs_as_text).
fn format_prediction_as_text(prediction: &Prediction) -> String {
    // Collect and sort keys for reproducible output
    let mut keys: Vec<_> = prediction.fields.keys().collect();
    keys.sort();

    // Simple formatting: "field1: value1\nfield2: value2"
    keys.iter()
        .filter_map(|key| {
            prediction.fields.get(*key).map(|value| {
                let value_str = match value {
                    serde_json::Value::String(s) => s.clone(),
                    _ => value.to_string(),
                };
                format!("{}: {}", key, value_str)
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// OpenAI fine-tuning entry format
#[derive(Debug, Serialize, Deserialize)]
struct FinetuneEntry {
    messages: Vec<Message>,
}

/// OpenAI message format
#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bootstrap_finetune_default() {
        let optimizer = BootstrapFinetune::new();
        assert!(optimizer.metric.is_none());
        assert_eq!(optimizer.min_score, 0.0);
        assert_eq!(optimizer.kafka_brokers, "localhost:9092");
        assert_eq!(optimizer.topic, "dashstream-events");
    }

    #[test]
    fn test_bootstrap_finetune_with_config() {
        let metric: MetricFn = Arc::new(|_ex, _pred, _trace| 0.95);

        let optimizer = BootstrapFinetune::new()
            .with_metric(metric)
            .with_min_score(0.8)
            .with_kafka_brokers("kafka:9092")
            .with_topic("custom-topic")
            .with_output_path(PathBuf::from("custom.jsonl"));

        assert!(optimizer.metric.is_some());
        assert_eq!(optimizer.min_score, 0.8);
        assert_eq!(optimizer.kafka_brokers, "kafka:9092");
        assert_eq!(optimizer.topic, "custom-topic");
        assert_eq!(optimizer.output_path, PathBuf::from("custom.jsonl"));
    }

    #[test]
    fn test_format_inputs_as_text() {
        let mut inputs = HashMap::new();
        inputs.insert("query".to_string(), serde_json::json!("What is Rust?"));
        inputs.insert("context".to_string(), serde_json::json!("programming"));

        let text = format_inputs_as_text(&inputs);
        // M-906: Keys are now sorted alphabetically for reproducible output
        assert_eq!(text, "context: programming\nquery: What is Rust?");
    }

    #[test]
    fn test_format_prediction_as_text() {
        let mut prediction = Prediction::new();
        prediction.fields.insert(
            "answer".to_string(),
            serde_json::json!("A systems programming language"),
        );

        let text = format_prediction_as_text(&prediction);
        assert_eq!(text, "answer: A systems programming language");
    }

    #[test]
    fn test_finetune_entry_serialization() {
        let entry = FinetuneEntry {
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: "You are a helpful assistant.".to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: "What is Rust?".to_string(),
                },
                Message {
                    role: "assistant".to_string(),
                    content: "A systems programming language".to_string(),
                },
            ],
        };

        let json = serde_json::to_string(&entry).expect("Failed to serialize");
        assert!(json.contains("system"));
        assert!(json.contains("user"));
        assert!(json.contains("assistant"));
        assert!(json.contains("What is Rust?"));
    }

    #[test]
    fn test_trace_to_finetune_entry() {
        use crate::optimize::trace_types::TraceEntry;

        let mut inputs = HashMap::new();
        inputs.insert("query".to_string(), serde_json::json!("What is Rust?"));

        let mut prediction = Prediction::new();
        prediction.fields.insert(
            "answer".to_string(),
            serde_json::json!("A programming language"),
        );

        let trace_entry = TraceEntry {
            predictor_name: "llm_node".to_string(),
            inputs,
            outputs: PredictionOrFailed::Success(prediction),
        };

        let trace_data = TraceData {
            example_ind: 0,
            example: Example::new().with("query", "What is Rust?"),
            prediction: PredictionOrFailed::Success(Prediction::new()),
            trace: vec![trace_entry],
            score: Some(0.95),
        };

        let finetune = trace_to_finetune_entry(&trace_data).expect("Failed to convert");
        assert_eq!(finetune.messages.len(), 3); // system + user + assistant
        assert_eq!(finetune.messages[0].role, "system");
        assert_eq!(finetune.messages[1].role, "user");
        assert_eq!(finetune.messages[2].role, "assistant");
    }
}
