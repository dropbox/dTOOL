//! # Core Trace Types for Optimizer Training
//!
//! This module provides the core data structures for trace-based optimization.
//! These types are always available (no feature gates) to enable local optimization
//! without requiring Kafka or external infrastructure.
//!
//! ## Design Principle
//!
//! Per DESIGN_INVARIANTS.md: "Local analysis NEVER requires external infrastructure."
//! These types can be populated from:
//! - Local ExecutionTrace (introspection.rs)
//! - Remote DashStream events (when dashstream feature is enabled)
//!
//! ## Types
//!
//! - `TraceEntry`: A single node execution with inputs and outputs
//! - `Prediction`: Successful output from a node
//! - `FailedPrediction`: Failed output with error message
//! - `PredictionOrFailed`: Either success or failure
//! - `TraceData`: Complete trace data for optimizer training
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::optimize::{Example, TraceEntry, Prediction, PredictionOrFailed, TraceData};
//!
//! // Create a prediction
//! let pred = Prediction::new()
//!     .with_field("answer", serde_json::json!("42"));
//!
//! // Create a trace entry
//! let entry = TraceEntry {
//!     predictor_name: "llm_node".to_string(),
//!     inputs: HashMap::new(),
//!     outputs: PredictionOrFailed::Success(pred),
//! };
//!
//! // Create trace data for optimization
//! let trace_data = TraceData {
//!     example_ind: 0,
//!     example: Example::new().with("question", "What is 6 * 7?"),
//!     prediction: PredictionOrFailed::Success(Prediction::new()),
//!     trace: vec![entry],
//!     score: Some(1.0),
//! };
//! ```

use crate::optimize::example::Example;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single entry in the execution trace (compatible with DashOpt format)
///
/// Each TraceEntry represents one node execution with its inputs and outputs.
/// This type is always available for local optimization.
///
/// **DEPRECATED:** Use `ExecutionTrace` and `NodeExecution` from the introspection module instead.
/// Convert ExecutionTrace to TraceEntry format using `ExecutionTrace::to_trace_entries()`.
#[deprecated(
    since = "1.11.3",
    note = "Use ExecutionTrace::to_trace_entries() instead. See introspection module."
)]
#[allow(deprecated)] // Allow internal impl usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEntry {
    /// Name of the node/predictor that was executed
    pub predictor_name: String,

    /// Input fields to the node
    pub inputs: HashMap<String, serde_json::Value>,

    /// Output from the node (either successful or failed)
    pub outputs: PredictionOrFailed,
}

#[allow(deprecated)] // Allow impl for deprecated type
impl TraceEntry {
    /// Create a new trace entry with the given predictor name
    pub fn new(predictor_name: impl Into<String>) -> Self {
        Self {
            predictor_name: predictor_name.into(),
            inputs: HashMap::new(),
            outputs: PredictionOrFailed::Success(Prediction::new()),
        }
    }

    /// Set the inputs for this entry
    #[must_use]
    pub fn with_inputs(mut self, inputs: HashMap<String, serde_json::Value>) -> Self {
        self.inputs = inputs;
        self
    }

    /// Add an input field
    #[must_use]
    pub fn with_input(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.inputs.insert(key.into(), value);
        self
    }

    /// Set the outputs for this entry
    #[must_use]
    pub fn with_outputs(mut self, outputs: PredictionOrFailed) -> Self {
        self.outputs = outputs;
        self
    }

    /// Create a successful entry with the given prediction
    #[must_use]
    pub fn with_success(mut self, prediction: Prediction) -> Self {
        self.outputs = PredictionOrFailed::Success(prediction);
        self
    }

    /// Create a failed entry with the given error message
    #[must_use]
    pub fn with_failure(mut self, error: impl Into<String>) -> Self {
        self.outputs = PredictionOrFailed::Failed(FailedPrediction {
            error: error.into(),
        });
        self
    }
}

/// Either a successful prediction or a failed one (compatible with DashOpt format)
///
/// This enum represents the result of an LLM prediction during trace collection.
/// It can either contain a successful prediction with output fields, or a failed
/// prediction with error information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PredictionOrFailed {
    /// A successful prediction containing the LLM's output fields.
    ///
    /// The prediction contains a map of field names to their values.
    Success(Prediction),
    /// A failed prediction with error information.
    ///
    /// Contains details about why the prediction failed (e.g., API error,
    /// parsing failure, timeout).
    Failed(FailedPrediction),
}

impl PredictionOrFailed {
    /// Check if this is a successful prediction
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success(_))
    }

    /// Check if this is a failed prediction
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }

    /// Get the successful prediction if this is a success
    pub fn as_success(&self) -> Option<&Prediction> {
        match self {
            Self::Success(p) => Some(p),
            Self::Failed(_) => None,
        }
    }

    /// Get the failed prediction if this is a failure
    pub fn as_failed(&self) -> Option<&FailedPrediction> {
        match self {
            Self::Success(_) => None,
            Self::Failed(f) => Some(f),
        }
    }
}

/// A successful prediction with output fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    /// Output fields produced by the prediction.
    #[serde(flatten)]
    pub fields: HashMap<String, serde_json::Value>,
}

impl Prediction {
    /// Create a new empty prediction
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
        }
    }

    /// Add a field to this prediction
    #[must_use]
    pub fn with_field(mut self, key: &str, value: serde_json::Value) -> Self {
        self.fields.insert(key.to_string(), value);
        self
    }

    /// Get a field value by key
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.fields.get(key)
    }

    /// Check if this prediction has a specific field
    pub fn has_field(&self, key: &str) -> bool {
        self.fields.contains_key(key)
    }

    /// Get the number of fields in this prediction
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Iterate over all fields
    pub fn iter(&self) -> impl Iterator<Item = (&String, &serde_json::Value)> {
        self.fields.iter()
    }
}

impl Default for Prediction {
    fn default() -> Self {
        Self::new()
    }
}

/// A failed prediction with error information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedPrediction {
    /// The error message describing why the prediction failed
    pub error: String,
}

impl FailedPrediction {
    /// Create a new failed prediction with the given error message
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
        }
    }
}

/// Data collected from tracing a program's execution (compatible with DashOpt format)
///
/// This struct contains all the data needed for optimizer training:
/// - The input example
/// - The final prediction (success or failure)
/// - The trace of node executions
/// - An optional score from metric evaluation
#[allow(deprecated)] // Uses deprecated TraceEntry for backwards compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceData {
    /// Index of the example in the dataset
    pub example_ind: usize,

    /// The input example
    pub example: Example,

    /// The prediction output
    pub prediction: PredictionOrFailed,

    /// Trace of node executions
    pub trace: Vec<TraceEntry>,

    /// Optional score from metric evaluation
    pub score: Option<f64>,
}

#[allow(deprecated)] // Uses deprecated TraceEntry for backwards compatibility
impl TraceData {
    /// Create new trace data
    pub fn new(example: Example, example_ind: usize) -> Self {
        Self {
            example_ind,
            example,
            prediction: PredictionOrFailed::Success(Prediction::new()),
            trace: Vec::new(),
            score: None,
        }
    }

    /// Set the prediction
    #[must_use]
    pub fn with_prediction(mut self, prediction: PredictionOrFailed) -> Self {
        self.prediction = prediction;
        self
    }

    /// Add a trace entry
    #[must_use]
    pub fn with_trace_entry(mut self, entry: TraceEntry) -> Self {
        self.trace.push(entry);
        self
    }

    /// Set all trace entries
    #[must_use]
    pub fn with_trace(mut self, trace: Vec<TraceEntry>) -> Self {
        self.trace = trace;
        self
    }

    /// Set the score
    #[must_use]
    pub fn with_score(mut self, score: f64) -> Self {
        self.score = Some(score);
        self
    }

    /// Check if this trace data has a score
    pub fn has_score(&self) -> bool {
        self.score.is_some()
    }

    /// Check if the execution was successful
    pub fn is_success(&self) -> bool {
        self.prediction.is_success()
    }
}

#[cfg(test)]
#[allow(deprecated)] // Tests use deprecated TraceEntry for backward compatibility testing
mod tests {
    use super::*;

    #[test]
    fn test_prediction_creation() {
        let pred = Prediction::new()
            .with_field("answer", serde_json::json!("42"))
            .with_field("confidence", serde_json::json!(0.95));

        assert_eq!(pred.get("answer"), Some(&serde_json::json!("42")));
        assert_eq!(pred.get("confidence"), Some(&serde_json::json!(0.95)));
        assert_eq!(pred.field_count(), 2);
        assert!(pred.has_field("answer"));
        assert!(!pred.has_field("missing"));
    }

    #[test]
    fn test_prediction_default() {
        let pred = Prediction::default();
        assert_eq!(pred.fields.len(), 0);
    }

    #[test]
    fn test_trace_entry_builder() {
        let entry = TraceEntry::new("llm_node")
            .with_input("query", serde_json::json!("What is Rust?"))
            .with_success(
                Prediction::new().with_field("answer", serde_json::json!("A programming language")),
            );

        assert_eq!(entry.predictor_name, "llm_node");
        assert_eq!(
            entry.inputs.get("query"),
            Some(&serde_json::json!("What is Rust?"))
        );
        assert!(entry.outputs.is_success());
    }

    #[test]
    fn test_trace_entry_failure() {
        let entry = TraceEntry::new("slow_node").with_failure("Timeout after 30s");

        assert!(entry.outputs.is_failed());
        if let PredictionOrFailed::Failed(f) = &entry.outputs {
            assert_eq!(f.error, "Timeout after 30s");
        }
    }

    #[test]
    fn test_trace_entry_serialization() {
        let mut inputs = HashMap::new();
        inputs.insert("query".to_string(), serde_json::json!("What is Rust?"));

        let pred = Prediction::new().with_field(
            "answer",
            serde_json::json!("A systems programming language"),
        );

        let entry = TraceEntry {
            predictor_name: "llm_node".to_string(),
            inputs,
            outputs: PredictionOrFailed::Success(pred),
        };

        // Serialize to JSON
        let json = serde_json::to_string(&entry).expect("Failed to serialize");
        assert!(json.contains("llm_node"));
        assert!(json.contains("query"));
        assert!(json.contains("answer"));

        // Deserialize back
        let deserialized: TraceEntry = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(deserialized.predictor_name, "llm_node");
    }

    #[test]
    fn test_failed_prediction() {
        let failed = FailedPrediction::new("Timeout");
        assert_eq!(failed.error, "Timeout");
    }

    #[test]
    fn test_prediction_or_failed_methods() {
        let success = PredictionOrFailed::Success(Prediction::new());
        let failure = PredictionOrFailed::Failed(FailedPrediction::new("Error"));

        assert!(success.is_success());
        assert!(!success.is_failed());
        assert!(success.as_success().is_some());
        assert!(success.as_failed().is_none());

        assert!(!failure.is_success());
        assert!(failure.is_failed());
        assert!(failure.as_success().is_none());
        assert!(failure.as_failed().is_some());
    }

    #[test]
    fn test_trace_data_builder() {
        let example = Example::new().with("question", "What is 6 * 7?");
        let entry = TraceEntry::new("calculator")
            .with_success(Prediction::new().with_field("answer", serde_json::json!(42)));

        let trace_data = TraceData::new(example, 0)
            .with_trace_entry(entry)
            .with_score(1.0)
            .with_prediction(PredictionOrFailed::Success(
                Prediction::new().with_field("answer", serde_json::json!(42)),
            ));

        assert_eq!(trace_data.example_ind, 0);
        assert_eq!(trace_data.trace.len(), 1);
        assert!(trace_data.has_score());
        assert!(trace_data.is_success());
    }

    #[test]
    fn test_trace_data_serialization() {
        let example = Example::new().with("input", "test");
        let trace_data = TraceData::new(example, 5).with_score(0.8);

        let json = serde_json::to_string(&trace_data).expect("Failed to serialize");
        let deserialized: TraceData = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(deserialized.example_ind, 5);
        assert_eq!(deserialized.score, Some(0.8));
    }
}
