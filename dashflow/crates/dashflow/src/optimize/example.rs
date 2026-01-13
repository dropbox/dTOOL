// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Training example representation for DashOptimize.
//!
//! In DashOptimize, training examples are represented as key-value maps where:
//! - Input fields are used for prompting
//! - Output fields are used for evaluation
//!
//! This module provides a convenient `Example` type that wraps `serde_json::Map`
//! and provides helper methods for working with training data.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A training example with input/output fields.
///
/// Examples are key-value maps where some fields are inputs (used in prompts)
/// and others are outputs (used for evaluation/supervision).
///
/// # Example
///
/// ```rust
/// use dashflow::optimize::Example;
///
/// // Create example from tuples
/// let example = Example::from([
///     ("question", "What is 2+2?"),
///     ("answer", "4"),
/// ]);
///
/// // Access fields
/// assert_eq!(example.get("question").and_then(|v| v.as_str()), Some("What is 2+2?"));
///
/// // Get only input fields (exclude output fields like "answer" if specified)
/// let inputs = example.inputs();
/// assert!(inputs.contains_key("question"));
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Example {
    /// Fields in this example
    data: Map<String, Value>,
    /// Explicitly tracked input field names (if specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    input_keys: Option<Vec<String>>,
}

impl Example {
    /// Create a new empty example.
    pub fn new() -> Self {
        Self {
            data: Map::new(),
            input_keys: None,
        }
    }

    /// Create an example from a JSON map.
    pub fn from_map(data: Map<String, Value>) -> Self {
        Self {
            data,
            input_keys: None,
        }
    }

    /// Insert a field into this example.
    pub fn with(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.data.insert(key.into(), value.into());
        self
    }

    /// Insert a field into this example (alias for compatibility).
    #[must_use]
    pub fn with_field(self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.with(key, value)
    }

    /// Specify which fields are inputs (the rest are considered outputs/labels).
    #[must_use]
    pub fn with_inputs(mut self, keys: &[&str]) -> Self {
        self.input_keys = Some(keys.iter().map(|&k| k.to_string()).collect());
        self
    }

    /// Get a field value.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }

    /// Get all fields.
    pub fn data(&self) -> &Map<String, Value> {
        &self.data
    }

    /// Get input fields only.
    ///
    /// If input_keys were explicitly specified via `with_inputs()`, returns only those fields.
    /// Otherwise, returns all fields except commonly known output fields.
    ///
    /// For KNN retrieval, we only want to embed the input fields, not the outputs.
    ///
    /// # Heuristic Behavior (M-868)
    ///
    /// When `with_inputs()` has NOT been called, this method uses a hardcoded list
    /// of "common output field names" to exclude:
    /// - `answer`, `output`, `label`, `category`, `classification`
    /// - `result`, `prediction`, `rationale`, `reasoning`
    ///
    /// **This heuristic may be surprising** if your examples use:
    /// - Different output field names (they'll be incorrectly included as inputs)
    /// - Input field names that match the list (they'll be incorrectly excluded)
    ///
    /// **Recommendation**: Always use `with_inputs()` to explicitly specify input fields:
    /// ```rust,ignore
    /// Example::new()
    ///     .with("question", "What is 2+2?")
    ///     .with("context", "Math problem")
    ///     .with("answer", 4)
    ///     .with_inputs(&["question", "context"])  // Explicit is better
    /// ```
    pub fn inputs(&self) -> Map<String, Value> {
        if let Some(input_keys) = &self.input_keys {
            // Use explicit input keys
            self.data
                .iter()
                .filter(|(key, _)| input_keys.contains(key))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        } else {
            // LIMITATION (M-868): Hardcoded heuristic for output field detection
            // This list may not match all use cases. See doc comment above for
            // recommendation to always use with_inputs() for explicit control.
            const OUTPUT_FIELDS: &[&str] = &[
                "answer",
                "output",
                "label",
                "category",
                "classification",
                "result",
                "prediction",
                "rationale",
                "reasoning",
            ];

            tracing::debug!(
                output_fields = ?OUTPUT_FIELDS,
                "Using heuristic to detect inputs (consider using with_inputs() for explicit control)"
            );

            self.data
                .iter()
                .filter(|(key, _)| !OUTPUT_FIELDS.contains(&key.as_str()))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        }
    }

    /// Get output/label fields only (inverse of inputs).
    ///
    /// If input_keys were explicitly specified, returns all fields except inputs.
    /// Otherwise, returns only commonly known output fields.
    ///
    /// # Heuristic Behavior (M-868)
    ///
    /// When `with_inputs()` has NOT been called, this method uses a hardcoded list
    /// of "common output field names" to include:
    /// - `answer`, `output`, `label`, `category`, `classification`
    /// - `result`, `prediction`, `rationale`, `reasoning`
    ///
    /// **This heuristic may be surprising** if your examples use different output
    /// field names - they'll be incorrectly excluded from labels.
    ///
    /// **Recommendation**: Always use `with_inputs()` to explicitly define the
    /// input/output boundary. See `inputs()` doc for example.
    pub fn labels(&self) -> Map<String, Value> {
        if let Some(input_keys) = &self.input_keys {
            // Return fields that are NOT inputs
            self.data
                .iter()
                .filter(|(key, _)| !input_keys.contains(key))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        } else {
            // LIMITATION (M-868): Hardcoded heuristic for output field detection
            // Same list as inputs() for consistency. See doc comment above.
            const OUTPUT_FIELDS: &[&str] = &[
                "answer",
                "output",
                "label",
                "category",
                "classification",
                "result",
                "prediction",
                "rationale",
                "reasoning",
            ];

            tracing::debug!(
                output_fields = ?OUTPUT_FIELDS,
                "Using heuristic to detect labels (consider using with_inputs() for explicit control)"
            );

            self.data
                .iter()
                .filter(|(key, _)| OUTPUT_FIELDS.contains(&key.as_str()))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        }
    }

    /// Get the number of fields.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if this example is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl Default for Example {
    fn default() -> Self {
        Self::new()
    }
}

/// Create an Example from an array of string tuples.
impl<const N: usize> From<[(&str, &str); N]> for Example {
    fn from(arr: [(&str, &str); N]) -> Self {
        let mut data = Map::new();
        for (key, value) in arr {
            data.insert(key.to_string(), Value::String(value.to_string()));
        }
        Self {
            data,
            input_keys: None,
        }
    }
}

/// Create an Example from a Vec of string tuples.
impl From<Vec<(String, String)>> for Example {
    fn from(vec: Vec<(String, String)>) -> Self {
        let mut data = Map::new();
        for (key, value) in vec {
            data.insert(key, Value::String(value));
        }
        Self {
            data,
            input_keys: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example_creation() {
        let example = Example::from([("question", "What is 2+2?"), ("answer", "4")]);
        assert_eq!(example.len(), 2);
        assert_eq!(
            example.get("question").and_then(|v| v.as_str()),
            Some("What is 2+2?")
        );
        assert_eq!(example.get("answer").and_then(|v| v.as_str()), Some("4"));
    }

    #[test]
    fn test_example_inputs() {
        let example = Example::from([
            ("question", "What is AI?"),
            ("context", "Technology"),
            ("answer", "Artificial Intelligence"),
        ]);

        let inputs = example.inputs();
        assert_eq!(inputs.len(), 2); // question and context, not answer
        assert!(inputs.contains_key("question"));
        assert!(inputs.contains_key("context"));
        assert!(!inputs.contains_key("answer")); // answer is an output field
    }

    #[test]
    fn test_example_with() {
        let example = Example::new()
            .with("input", "test")
            .with("output", "result");
        assert_eq!(example.len(), 2);
        assert_eq!(example.get("input").and_then(|v| v.as_str()), Some("test"));
    }

    #[test]
    fn test_example_empty() {
        let example = Example::new();
        assert!(example.is_empty());
        assert_eq!(example.len(), 0);
    }

    #[test]
    fn test_with_field_alias() {
        let example = Example::new()
            .with_field("question", "What is 2+2?")
            .with_field("answer", "4");
        assert_eq!(example.len(), 2);
        assert_eq!(
            example.get("question").and_then(|v| v.as_str()),
            Some("What is 2+2?")
        );
    }

    #[test]
    fn test_explicit_input_keys() {
        let example = Example::new()
            .with_field("question", "What is AI?")
            .with_field("answer", "Artificial Intelligence")
            .with_inputs(&["question"]);

        let inputs = example.inputs();
        assert_eq!(inputs.len(), 1);
        assert!(inputs.contains_key("question"));
        assert!(!inputs.contains_key("answer"));

        let labels = example.labels();
        assert_eq!(labels.len(), 1);
        assert!(labels.contains_key("answer"));
        assert!(!labels.contains_key("question"));
    }

    #[test]
    fn test_labels_heuristic() {
        let example = Example::from([
            ("question", "What is AI?"),
            ("context", "Technology"),
            ("answer", "Artificial Intelligence"),
        ]);

        let labels = example.labels();
        assert_eq!(labels.len(), 1); // Only "answer" is recognized as output
        assert!(labels.contains_key("answer"));
    }
}
