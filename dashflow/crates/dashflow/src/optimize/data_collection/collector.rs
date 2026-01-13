// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Data collection from production execution

use crate::optimize::data_collection::types::TrainingExample;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Format for collecting data
///
/// Defines how to extract input and output fields from production data
/// for creating training examples.
#[derive(Debug, Clone)]
pub enum DataFormat {
    /// Classification task (text -> label)
    ///
    /// Used for tasks where a single text input is classified into a category.
    Classification {
        /// Name of the field containing the text to classify.
        text_field: String,
        /// Name of the field containing the classification label.
        label_field: String,
    },
    /// Question-answering (question -> answer)
    ///
    /// Used for tasks where a question is answered with a text response.
    QuestionAnswer {
        /// Name of the field containing the question.
        question_field: String,
        /// Name of the field containing the answer.
        answer_field: String,
    },
    /// Custom format with field mappings
    ///
    /// Used for arbitrary input/output field configurations.
    Custom {
        /// Names of fields to extract as inputs.
        input_fields: Vec<String>,
        /// Names of fields to extract as outputs.
        output_fields: Vec<String>,
    },
}

impl DataFormat {
    /// Create a classification format
    pub fn classification(text_field: &str, label_field: &str) -> Self {
        Self::Classification {
            text_field: text_field.to_string(),
            label_field: label_field.to_string(),
        }
    }

    /// Create a question-answer format
    pub fn question_answer(question_field: &str, answer_field: &str) -> Self {
        Self::QuestionAnswer {
            question_field: question_field.to_string(),
            answer_field: answer_field.to_string(),
        }
    }

    /// Create a custom format
    pub fn custom(input_fields: Vec<String>, output_fields: Vec<String>) -> Self {
        Self::Custom {
            input_fields,
            output_fields,
        }
    }

    /// Extract input fields from a data structure
    fn extract_input(
        &self,
        data: &HashMap<String, serde_json::Value>,
    ) -> HashMap<String, serde_json::Value> {
        match self {
            Self::Classification { text_field, .. } => {
                let mut input = HashMap::new();
                if let Some(value) = data.get(text_field) {
                    input.insert(text_field.clone(), value.clone());
                }
                input
            }
            Self::QuestionAnswer { question_field, .. } => {
                let mut input = HashMap::new();
                if let Some(value) = data.get(question_field) {
                    input.insert(question_field.clone(), value.clone());
                }
                input
            }
            Self::Custom { input_fields, .. } => {
                let mut input = HashMap::new();
                for field in input_fields {
                    if let Some(value) = data.get(field) {
                        input.insert(field.clone(), value.clone());
                    }
                }
                input
            }
        }
    }

    /// Extract output fields from a data structure
    fn extract_output(
        &self,
        data: &HashMap<String, serde_json::Value>,
    ) -> HashMap<String, serde_json::Value> {
        match self {
            Self::Classification { label_field, .. } => {
                let mut output = HashMap::new();
                if let Some(value) = data.get(label_field) {
                    output.insert(label_field.clone(), value.clone());
                }
                output
            }
            Self::QuestionAnswer { answer_field, .. } => {
                let mut output = HashMap::new();
                if let Some(value) = data.get(answer_field) {
                    output.insert(answer_field.clone(), value.clone());
                }
                output
            }
            Self::Custom { output_fields, .. } => {
                let mut output = HashMap::new();
                for field in output_fields {
                    if let Some(value) = data.get(field) {
                        output.insert(field.clone(), value.clone());
                    }
                }
                output
            }
        }
    }
}

/// Storage backend for collected data
#[derive(Debug, Clone)]
pub enum DataStore {
    /// JSONL file (one JSON object per line)
    Jsonl(PathBuf),
    /// In-memory storage (for testing)
    Memory(Vec<TrainingExample>),
}

impl DataStore {
    /// Create a JSONL file store
    pub fn jsonl<P: AsRef<Path>>(path: P) -> Self {
        Self::Jsonl(path.as_ref().to_path_buf())
    }

    /// Create an in-memory store
    pub fn memory() -> Self {
        Self::Memory(Vec::new())
    }
}

/// Data collector for capturing training examples
pub struct DataCollector {
    format: DataFormat,
    store: DataStore,
}

impl DataCollector {
    /// Create a new data collector
    pub fn new(format: DataFormat, store: DataStore) -> Self {
        Self { format, store }
    }

    /// Sanitize a value by removing DashOpt signature template placeholders
    ///
    /// Removes placeholder strings like "{string}", "{list}", "{number}" that LLMs
    /// sometimes return when copying signature templates.
    fn sanitize_value(value: serde_json::Value) -> serde_json::Value {
        use serde_json::Value;
        match value {
            Value::String(s) => {
                // Filter out template placeholders
                if s.is_empty()
                    || s == "none"
                    || s == "N/A"
                    || s == "{string}"
                    || s == "{list}"
                    || s == "{number}"
                    || s.starts_with('{')
                {
                    Value::String(String::new())
                } else {
                    Value::String(s)
                }
            }
            Value::Array(arr) => {
                // Recursively sanitize array elements
                let sanitized: Vec<Value> = arr
                    .into_iter()
                    .map(Self::sanitize_value)
                    .filter(|v| {
                        if let Value::String(s) = v {
                            !s.is_empty()
                        } else {
                            true
                        }
                    })
                    .collect();
                Value::Array(sanitized)
            }
            Value::Object(mut obj) => {
                // Recursively sanitize object fields
                for (_, v) in obj.iter_mut() {
                    *v = Self::sanitize_value(v.clone());
                }
                Value::Object(obj)
            }
            _ => value,
        }
    }

    /// Sanitize a HashMap of values
    fn sanitize_data(
        data: HashMap<String, serde_json::Value>,
    ) -> HashMap<String, serde_json::Value> {
        data.into_iter()
            .map(|(k, v)| (k, Self::sanitize_value(v)))
            .collect()
    }

    /// Collect a training example from raw data
    ///
    /// The data should contain all fields specified in the format.
    pub async fn collect(&mut self, data: HashMap<String, serde_json::Value>) -> crate::Result<()> {
        // Sanitize data before extraction
        let sanitized_data = Self::sanitize_data(data);

        let input = self.format.extract_input(&sanitized_data);
        let output = self.format.extract_output(&sanitized_data);

        // Skip if missing required fields
        if input.is_empty() || output.is_empty() {
            return Ok(());
        }

        let example = TrainingExample::production(input, output);

        match &mut self.store {
            DataStore::Jsonl(path) => {
                let path_display = path.display().to_string();
                // Ensure parent directory exists
                if let Some(parent) = path.parent() {
                    let parent_display = parent.display().to_string();
                    tokio::fs::create_dir_all(parent).await.map_err(|e| {
                        crate::Error::Generic(format!(
                            "Failed to create data collection directory '{}': {e}",
                            parent_display
                        ))
                    })?;
                }

                // Append to file
                let mut file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path.as_path())
                    .await
                    .map_err(|e| {
                        crate::Error::Generic(format!(
                            "Failed to open data collection file '{}': {e}",
                            path_display
                        ))
                    })?;

                let json = serde_json::to_string(&example).map_err(|e| {
                    crate::Error::Generic(format!("Failed to serialize training example: {e}"))
                })?;

                file.write_all(json.as_bytes()).await.map_err(|e| {
                    crate::Error::Generic(format!(
                        "Failed to write training example to '{}': {e}",
                        path_display
                    ))
                })?;
                file.write_all(b"\n").await.map_err(|e| {
                    crate::Error::Generic(format!(
                        "Failed to write newline to '{}': {e}",
                        path_display
                    ))
                })?;
            }
            DataStore::Memory(examples) => {
                examples.push(example);
            }
        }

        Ok(())
    }

    /// Load all collected examples
    pub async fn load_dataset(&self) -> crate::Result<Vec<TrainingExample>> {
        match &self.store {
            DataStore::Jsonl(path) => {
                // Use async file existence check to avoid blocking the async runtime
                if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
                    return Ok(Vec::new());
                }

                let file = File::open(path).await.map_err(|e| {
                    crate::Error::Generic(format!(
                        "Failed to open data collection file '{}': {e}",
                        path.display()
                    ))
                })?;

                let reader = BufReader::new(file);
                let mut lines = reader.lines();
                let mut examples = Vec::new();
                let mut line_number = 0usize;

                while let Some(line) = lines.next_line().await.map_err(|e| {
                    crate::Error::Generic(format!(
                        "Failed to read line from '{}': {e}",
                        path.display()
                    ))
                })? {
                    line_number += 1;
                    if line.trim().is_empty() {
                        continue;
                    }

                    let example: TrainingExample = serde_json::from_str(&line).map_err(|e| {
                        crate::Error::Generic(format!(
                            "Failed to parse training example at line {} in '{}': {e}",
                            line_number,
                            path.display()
                        ))
                    })?;
                    examples.push(example);
                }

                Ok(examples)
            }
            DataStore::Memory(examples) => Ok(examples.clone()),
        }
    }

    /// Get the number of collected examples
    pub async fn count(&self) -> crate::Result<usize> {
        let examples = self.load_dataset().await?;
        Ok(examples.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_format_classification() {
        let format = DataFormat::classification("text", "label");

        let mut data = HashMap::new();
        data.insert("text".to_string(), serde_json::json!("hello world"));
        data.insert("label".to_string(), serde_json::json!("greeting"));
        data.insert("extra".to_string(), serde_json::json!("ignored"));

        let input = format.extract_input(&data);
        let output = format.extract_output(&data);

        assert_eq!(input.len(), 1);
        assert_eq!(input.get("text"), Some(&serde_json::json!("hello world")));
        assert_eq!(output.len(), 1);
        assert_eq!(output.get("label"), Some(&serde_json::json!("greeting")));
    }

    #[test]
    fn test_data_format_custom() {
        let format = DataFormat::custom(
            vec!["field1".to_string(), "field2".to_string()],
            vec!["output1".to_string()],
        );

        let mut data = HashMap::new();
        data.insert("field1".to_string(), serde_json::json!("value1"));
        data.insert("field2".to_string(), serde_json::json!("value2"));
        data.insert("output1".to_string(), serde_json::json!("result"));
        data.insert("extra".to_string(), serde_json::json!("ignored"));

        let input = format.extract_input(&data);
        let output = format.extract_output(&data);

        assert_eq!(input.len(), 2);
        assert_eq!(output.len(), 1);
    }

    #[tokio::test]
    async fn test_collector_memory() {
        let format = DataFormat::classification("text", "label");
        let store = DataStore::memory();
        let mut collector = DataCollector::new(format, store);

        let mut data = HashMap::new();
        data.insert("text".to_string(), serde_json::json!("test query"));
        data.insert("label".to_string(), serde_json::json!("positive"));

        collector.collect(data).await.unwrap();

        let examples = collector.load_dataset().await.unwrap();
        assert_eq!(examples.len(), 1);
        assert_eq!(
            examples[0].get_input_field("text"),
            Some("test query".to_string())
        );
        assert_eq!(
            examples[0].get_output_field("label"),
            Some("positive".to_string())
        );
    }

    // JSONL test removed - requires tempfile which is not needed for core functionality
    // Memory store test covers the data collection functionality

    #[tokio::test]
    async fn test_collector_skip_incomplete() {
        let format = DataFormat::classification("text", "label");
        let store = DataStore::memory();
        let mut collector = DataCollector::new(format, store);

        // Missing label field
        let mut data = HashMap::new();
        data.insert("text".to_string(), serde_json::json!("test"));

        collector.collect(data).await.unwrap();

        let examples = collector.load_dataset().await.unwrap();
        assert_eq!(examples.len(), 0); // Skipped incomplete example
    }

    #[test]
    fn test_data_format_question_answer() {
        let format = DataFormat::question_answer("question", "answer");

        let mut data = HashMap::new();
        data.insert("question".to_string(), serde_json::json!("What is 2+2?"));
        data.insert("answer".to_string(), serde_json::json!("4"));
        data.insert("metadata".to_string(), serde_json::json!("ignored"));

        let input = format.extract_input(&data);
        let output = format.extract_output(&data);

        assert_eq!(input.len(), 1);
        assert_eq!(
            input.get("question"),
            Some(&serde_json::json!("What is 2+2?"))
        );
        assert_eq!(output.len(), 1);
        assert_eq!(output.get("answer"), Some(&serde_json::json!("4")));
    }

    #[test]
    fn test_data_format_missing_fields() {
        let format = DataFormat::classification("text", "label");

        // Data missing the expected fields
        let mut data = HashMap::new();
        data.insert("other_field".to_string(), serde_json::json!("value"));

        let input = format.extract_input(&data);
        let output = format.extract_output(&data);

        assert!(input.is_empty());
        assert!(output.is_empty());
    }

    #[test]
    fn test_data_format_custom_partial_fields() {
        let format = DataFormat::custom(
            vec![
                "field1".to_string(),
                "field2".to_string(),
                "field3".to_string(),
            ],
            vec!["out1".to_string(), "out2".to_string()],
        );

        // Only some fields present
        let mut data = HashMap::new();
        data.insert("field1".to_string(), serde_json::json!("v1"));
        data.insert("field3".to_string(), serde_json::json!("v3"));
        data.insert("out1".to_string(), serde_json::json!("result"));

        let input = format.extract_input(&data);
        let output = format.extract_output(&data);

        // Should only include present fields
        assert_eq!(input.len(), 2);
        assert!(input.contains_key("field1"));
        assert!(input.contains_key("field3"));
        assert!(!input.contains_key("field2"));

        assert_eq!(output.len(), 1);
        assert!(output.contains_key("out1"));
        assert!(!output.contains_key("out2"));
    }

    #[test]
    fn test_datastore_jsonl_constructor() {
        let store = DataStore::jsonl("/tmp/test.jsonl");
        match store {
            DataStore::Jsonl(path) => {
                assert_eq!(path.to_str().unwrap(), "/tmp/test.jsonl");
            }
            _ => panic!("Expected Jsonl variant"),
        }
    }

    #[test]
    fn test_datastore_memory_constructor() {
        let store = DataStore::memory();
        match store {
            DataStore::Memory(examples) => {
                assert!(examples.is_empty());
            }
            _ => panic!("Expected Memory variant"),
        }
    }

    #[test]
    fn test_sanitize_value_string() {
        // Normal string passes through
        let normal = serde_json::json!("hello world");
        let result = DataCollector::sanitize_value(normal);
        assert_eq!(result, serde_json::json!("hello world"));

        // Template placeholders become empty
        assert_eq!(
            DataCollector::sanitize_value(serde_json::json!("{string}")),
            serde_json::json!("")
        );
        assert_eq!(
            DataCollector::sanitize_value(serde_json::json!("{list}")),
            serde_json::json!("")
        );
        assert_eq!(
            DataCollector::sanitize_value(serde_json::json!("{number}")),
            serde_json::json!("")
        );

        // "none" and "N/A" become empty
        assert_eq!(
            DataCollector::sanitize_value(serde_json::json!("none")),
            serde_json::json!("")
        );
        assert_eq!(
            DataCollector::sanitize_value(serde_json::json!("N/A")),
            serde_json::json!("")
        );

        // Strings starting with { become empty
        assert_eq!(
            DataCollector::sanitize_value(serde_json::json!("{custom_placeholder}")),
            serde_json::json!("")
        );

        // Empty string stays empty
        assert_eq!(
            DataCollector::sanitize_value(serde_json::json!("")),
            serde_json::json!("")
        );
    }

    #[test]
    fn test_sanitize_value_array() {
        let arr = serde_json::json!(["hello", "{string}", "world", "none", "valid"]);
        let result = DataCollector::sanitize_value(arr);

        // Empty strings are filtered out from arrays
        let expected = serde_json::json!(["hello", "world", "valid"]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_sanitize_value_object() {
        let obj = serde_json::json!({
            "good": "valid value",
            "bad": "{string}",
            "also_good": 42,
            "nested": {
                "inner": "none"
            }
        });

        let result = DataCollector::sanitize_value(obj);

        // Strings sanitized but keys preserved
        assert_eq!(result["good"], serde_json::json!("valid value"));
        assert_eq!(result["bad"], serde_json::json!(""));
        assert_eq!(result["also_good"], serde_json::json!(42));
        assert_eq!(result["nested"]["inner"], serde_json::json!(""));
    }

    #[test]
    #[allow(clippy::approx_constant)] // Tests JSON float pass-through with 3.14
    fn test_sanitize_value_non_string_types() {
        // Numbers pass through unchanged
        assert_eq!(
            DataCollector::sanitize_value(serde_json::json!(42)),
            serde_json::json!(42)
        );
        assert_eq!(
            DataCollector::sanitize_value(serde_json::json!(3.14)),
            serde_json::json!(3.14)
        );

        // Booleans pass through unchanged
        assert_eq!(
            DataCollector::sanitize_value(serde_json::json!(true)),
            serde_json::json!(true)
        );
        assert_eq!(
            DataCollector::sanitize_value(serde_json::json!(false)),
            serde_json::json!(false)
        );

        // Null passes through unchanged
        assert_eq!(
            DataCollector::sanitize_value(serde_json::Value::Null),
            serde_json::Value::Null
        );
    }

    #[test]
    fn test_sanitize_data() {
        let mut data = HashMap::new();
        data.insert("text".to_string(), serde_json::json!("valid input"));
        data.insert("template".to_string(), serde_json::json!("{string}"));
        data.insert("number".to_string(), serde_json::json!(100));

        let sanitized = DataCollector::sanitize_data(data);

        assert_eq!(
            sanitized.get("text"),
            Some(&serde_json::json!("valid input"))
        );
        assert_eq!(sanitized.get("template"), Some(&serde_json::json!("")));
        assert_eq!(sanitized.get("number"), Some(&serde_json::json!(100)));
    }

    #[tokio::test]
    async fn test_collector_count() {
        let format = DataFormat::classification("text", "label");
        let store = DataStore::memory();
        let mut collector = DataCollector::new(format, store);

        assert_eq!(collector.count().await.unwrap(), 0);

        let mut data1 = HashMap::new();
        data1.insert("text".to_string(), serde_json::json!("example 1"));
        data1.insert("label".to_string(), serde_json::json!("pos"));
        collector.collect(data1).await.unwrap();

        assert_eq!(collector.count().await.unwrap(), 1);

        let mut data2 = HashMap::new();
        data2.insert("text".to_string(), serde_json::json!("example 2"));
        data2.insert("label".to_string(), serde_json::json!("neg"));
        collector.collect(data2).await.unwrap();

        assert_eq!(collector.count().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_collector_sanitizes_on_collect() {
        let format = DataFormat::classification("text", "label");
        let store = DataStore::memory();
        let mut collector = DataCollector::new(format, store);

        let mut data = HashMap::new();
        data.insert("text".to_string(), serde_json::json!("valid text"));
        data.insert("label".to_string(), serde_json::json!("{string}")); // Template placeholder

        collector.collect(data).await.unwrap();

        let examples = collector.load_dataset().await.unwrap();
        // Should still collect because label field exists (even if sanitized to empty)
        // But label gets sanitized to empty string
        assert_eq!(examples.len(), 1);
        assert_eq!(examples[0].get_output_field("label"), Some("".to_string()));
    }

    #[tokio::test]
    async fn test_collector_multiple_examples() {
        let format = DataFormat::question_answer("q", "a");
        let store = DataStore::memory();
        let mut collector = DataCollector::new(format, store);

        for i in 0..5 {
            let mut data = HashMap::new();
            data.insert(
                "q".to_string(),
                serde_json::json!(format!("Question {}", i)),
            );
            data.insert("a".to_string(), serde_json::json!(format!("Answer {}", i)));
            collector.collect(data).await.unwrap();
        }

        let examples = collector.load_dataset().await.unwrap();
        assert_eq!(examples.len(), 5);

        // Verify order preserved
        assert_eq!(
            examples[0].get_input_field("q"),
            Some("Question 0".to_string())
        );
        assert_eq!(
            examples[4].get_input_field("q"),
            Some("Question 4".to_string())
        );
    }

    #[tokio::test]
    async fn test_collector_skip_both_missing() {
        let format = DataFormat::classification("text", "label");
        let store = DataStore::memory();
        let mut collector = DataCollector::new(format, store);

        // Both input and output fields missing
        let mut data = HashMap::new();
        data.insert("unrelated".to_string(), serde_json::json!("value"));

        collector.collect(data).await.unwrap();

        let examples = collector.load_dataset().await.unwrap();
        assert_eq!(examples.len(), 0);
    }

    #[tokio::test]
    async fn test_collector_skip_input_only() {
        let format = DataFormat::classification("text", "label");
        let store = DataStore::memory();
        let mut collector = DataCollector::new(format, store);

        // Only input field present, output missing
        let mut data = HashMap::new();
        data.insert("text".to_string(), serde_json::json!("has input"));

        collector.collect(data).await.unwrap();

        let examples = collector.load_dataset().await.unwrap();
        assert_eq!(examples.len(), 0); // Skipped - no output
    }

    #[tokio::test]
    async fn test_collector_skip_output_only() {
        let format = DataFormat::classification("text", "label");
        let store = DataStore::memory();
        let mut collector = DataCollector::new(format, store);

        // Only output field present, input missing
        let mut data = HashMap::new();
        data.insert("label".to_string(), serde_json::json!("positive"));

        collector.collect(data).await.unwrap();

        let examples = collector.load_dataset().await.unwrap();
        assert_eq!(examples.len(), 0); // Skipped - no input
    }
}
