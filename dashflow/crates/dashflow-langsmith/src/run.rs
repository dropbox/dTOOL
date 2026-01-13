//! Run types and structures for `LangSmith` tracing

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Type of run (chain, llm, tool, retriever, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunType {
    /// LLM run (completion or chat)
    Llm,
    /// Chain run (sequence of operations)
    Chain,
    /// Tool run (function/tool execution)
    Tool,
    /// Retriever run (document retrieval)
    Retriever,
    /// Embedding run (text embedding)
    Embedding,
    /// Prompt run (prompt template)
    Prompt,
    /// Parser run (output parsing)
    Parser,
}

/// A run represents a single execution trace in `LangSmith`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    /// Unique identifier for the run
    pub id: Uuid,

    /// Name of the run (e.g., "`ChatOpenAI`", "`RetrievalQA`")
    pub name: String,

    /// Type of run
    pub run_type: RunType,

    /// Start time of the run
    pub start_time: DateTime<Utc>,

    /// End time of the run (None if still running)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<DateTime<Utc>>,

    /// Parent run ID (if this is a child run)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<Uuid>,

    /// Input to the run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<serde_json::Value>,

    /// Output from the run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<serde_json::Value>,

    /// Error that occurred during the run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Serialized version of the object that was run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serialized: Option<serde_json::Value>,

    /// Tags for the run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,

    /// Session/project name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_name: Option<String>,

    /// Reference example ID (for evaluation runs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_example_id: Option<Uuid>,

    /// Extra information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<HashMap<String, serde_json::Value>>,

    /// Events that occurred during the run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<serde_json::Value>>,
}

/// Parameters for creating a new run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunCreate {
    /// Unique identifier for the run
    pub id: Uuid,

    /// Name of the run
    pub name: String,

    /// Type of run
    pub run_type: RunType,

    /// Start time of the run
    pub start_time: DateTime<Utc>,

    /// Parent run ID (if this is a child run)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<Uuid>,

    /// Input to the run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<serde_json::Value>,

    /// Serialized version of the object
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serialized: Option<serde_json::Value>,

    /// Tags for the run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,

    /// Session/project name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_name: Option<String>,

    /// Reference example ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_example_id: Option<Uuid>,

    /// Extra information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<HashMap<String, serde_json::Value>>,
}

/// Parameters for updating an existing run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunUpdate {
    /// End time of the run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<DateTime<Utc>>,

    /// Output from the run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<serde_json::Value>,

    /// Error that occurred
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Additional metadata to merge
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,

    /// Events that occurred
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<serde_json::Value>>,

    /// Extra information to merge
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<HashMap<String, serde_json::Value>>,
}

impl RunCreate {
    /// Create a new `RunCreate` with required fields
    pub fn new(id: Uuid, name: impl Into<String>, run_type: RunType) -> Self {
        Self {
            id,
            name: name.into(),
            run_type,
            start_time: Utc::now(),
            parent_run_id: None,
            inputs: None,
            serialized: None,
            tags: None,
            metadata: None,
            session_name: None,
            reference_example_id: None,
            extra: None,
        }
    }

    /// Set the parent run ID
    #[must_use]
    pub fn with_parent_run_id(mut self, parent_run_id: Uuid) -> Self {
        self.parent_run_id = Some(parent_run_id);
        self
    }

    /// Set the inputs
    #[must_use]
    pub fn with_inputs(mut self, inputs: serde_json::Value) -> Self {
        self.inputs = Some(inputs);
        self
    }

    /// Set the tags
    #[must_use]
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = Some(tags);
        self
    }

    /// Set the session name
    pub fn with_session_name(mut self, session_name: impl Into<String>) -> Self {
        self.session_name = Some(session_name.into());
        self
    }

    /// Set metadata
    #[must_use]
    pub fn with_metadata(mut self, metadata: HashMap<String, serde_json::Value>) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

impl RunUpdate {
    /// Create a new empty `RunUpdate`
    #[must_use]
    pub fn new() -> Self {
        Self {
            end_time: None,
            outputs: None,
            error: None,
            metadata: None,
            events: None,
            extra: None,
        }
    }

    /// Set the end time
    #[must_use]
    pub fn with_end_time(mut self, end_time: DateTime<Utc>) -> Self {
        self.end_time = Some(end_time);
        self
    }

    /// Set the outputs
    #[must_use]
    pub fn with_outputs(mut self, outputs: serde_json::Value) -> Self {
        self.outputs = Some(outputs);
        self
    }

    /// Set an error
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }
}

impl Default for RunUpdate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // ===== RunType Tests =====

    #[test]
    fn test_run_type_serialization() {
        let run_type = RunType::Llm;
        let json = serde_json::to_string(&run_type).unwrap();
        assert_eq!(json, "\"llm\"");

        let deserialized: RunType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, RunType::Llm);
    }

    #[test]
    fn test_run_type_all_variants_serialization() {
        let variants = [
            (RunType::Llm, "\"llm\""),
            (RunType::Chain, "\"chain\""),
            (RunType::Tool, "\"tool\""),
            (RunType::Retriever, "\"retriever\""),
            (RunType::Embedding, "\"embedding\""),
            (RunType::Prompt, "\"prompt\""),
            (RunType::Parser, "\"parser\""),
        ];

        for (run_type, expected_json) in variants {
            let json = serde_json::to_string(&run_type).unwrap();
            assert_eq!(json, expected_json, "Failed for {:?}", run_type);

            let deserialized: RunType = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, run_type);
        }
    }

    #[test]
    fn test_run_type_clone() {
        let rt = RunType::Chain;
        let cloned = rt.clone();
        assert_eq!(rt, cloned);
    }

    #[test]
    fn test_run_type_copy() {
        let rt = RunType::Tool;
        let copied = rt;
        assert_eq!(rt, copied);
    }

    #[test]
    fn test_run_type_debug() {
        let rt = RunType::Retriever;
        let debug = format!("{:?}", rt);
        assert!(debug.contains("Retriever"));
    }

    #[test]
    fn test_run_type_eq() {
        assert_eq!(RunType::Llm, RunType::Llm);
        assert_ne!(RunType::Llm, RunType::Chain);
    }

    // ===== RunCreate Tests =====

    #[test]
    fn test_run_create_builder() {
        let run_id = Uuid::new_v4();
        let parent_id = Uuid::new_v4();

        let run = RunCreate::new(run_id, "TestRun", RunType::Chain)
            .with_parent_run_id(parent_id)
            .with_tags(vec!["test".to_string()])
            .with_session_name("test-session");

        assert_eq!(run.id, run_id);
        assert_eq!(run.name, "TestRun");
        assert_eq!(run.run_type, RunType::Chain);
        assert_eq!(run.parent_run_id, Some(parent_id));
        assert_eq!(run.session_name, Some("test-session".to_string()));
    }

    #[test]
    fn test_run_create_new_minimal() {
        let run_id = Uuid::new_v4();
        let run = RunCreate::new(run_id, "minimal", RunType::Llm);

        assert_eq!(run.id, run_id);
        assert_eq!(run.name, "minimal");
        assert_eq!(run.run_type, RunType::Llm);
        assert!(run.parent_run_id.is_none());
        assert!(run.inputs.is_none());
        assert!(run.serialized.is_none());
        assert!(run.tags.is_none());
        assert!(run.metadata.is_none());
        assert!(run.session_name.is_none());
        assert!(run.reference_example_id.is_none());
        assert!(run.extra.is_none());
    }

    #[test]
    fn test_run_create_with_parent_run_id() {
        let run_id = Uuid::new_v4();
        let parent_id = Uuid::new_v4();
        let run = RunCreate::new(run_id, "child", RunType::Tool)
            .with_parent_run_id(parent_id);

        assert_eq!(run.parent_run_id, Some(parent_id));
    }

    #[test]
    fn test_run_create_with_inputs() {
        let run = RunCreate::new(Uuid::new_v4(), "test", RunType::Llm)
            .with_inputs(serde_json::json!({"prompt": "hello"}));

        assert!(run.inputs.is_some());
        let inputs = run.inputs.unwrap();
        assert_eq!(inputs["prompt"], "hello");
    }

    #[test]
    fn test_run_create_with_tags() {
        let tags = vec!["tag1".to_string(), "tag2".to_string(), "tag3".to_string()];
        let run = RunCreate::new(Uuid::new_v4(), "test", RunType::Chain)
            .with_tags(tags.clone());

        assert_eq!(run.tags, Some(tags));
    }

    #[test]
    fn test_run_create_with_empty_tags() {
        let run = RunCreate::new(Uuid::new_v4(), "test", RunType::Chain)
            .with_tags(vec![]);

        assert_eq!(run.tags, Some(vec![]));
    }

    #[test]
    fn test_run_create_with_session_name() {
        let run = RunCreate::new(Uuid::new_v4(), "test", RunType::Retriever)
            .with_session_name("my-session");

        assert_eq!(run.session_name, Some("my-session".to_string()));
    }

    #[test]
    fn test_run_create_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("key1".to_string(), serde_json::json!("value1"));
        metadata.insert("key2".to_string(), serde_json::json!(42));

        let run = RunCreate::new(Uuid::new_v4(), "test", RunType::Embedding)
            .with_metadata(metadata.clone());

        assert_eq!(run.metadata, Some(metadata));
    }

    #[test]
    fn test_run_create_chaining_all_builders() {
        let run_id = Uuid::new_v4();
        let parent_id = Uuid::new_v4();
        let mut metadata = HashMap::new();
        metadata.insert("foo".to_string(), serde_json::json!("bar"));

        let run = RunCreate::new(run_id, "full", RunType::Parser)
            .with_parent_run_id(parent_id)
            .with_inputs(serde_json::json!({"input": "data"}))
            .with_tags(vec!["production".to_string()])
            .with_session_name("session-1")
            .with_metadata(metadata);

        assert_eq!(run.id, run_id);
        assert_eq!(run.name, "full");
        assert_eq!(run.parent_run_id, Some(parent_id));
        assert!(run.inputs.is_some());
        assert!(run.tags.is_some());
        assert!(run.session_name.is_some());
        assert!(run.metadata.is_some());
    }

    #[test]
    fn test_run_create_serialization() {
        let run_id = Uuid::new_v4();
        let run = RunCreate::new(run_id, "serialize-test", RunType::Llm)
            .with_inputs(serde_json::json!({"x": 1}))
            .with_tags(vec!["test".to_string()]);

        let json = serde_json::to_string(&run).unwrap();
        assert!(json.contains("serialize-test"));
        assert!(json.contains("llm"));
        assert!(json.contains(&run_id.to_string()));

        let deserialized: RunCreate = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, run_id);
        assert_eq!(deserialized.name, "serialize-test");
        assert_eq!(deserialized.run_type, RunType::Llm);
    }

    #[test]
    fn test_run_create_skip_serializing_none_fields() {
        let run = RunCreate::new(Uuid::new_v4(), "test", RunType::Chain);
        let json = serde_json::to_string(&run).unwrap();

        // None fields should not appear in JSON
        assert!(!json.contains("parent_run_id"));
        assert!(!json.contains("inputs"));
        assert!(!json.contains("serialized"));
        assert!(!json.contains("tags"));
        assert!(!json.contains("metadata"));
        assert!(!json.contains("session_name"));
        assert!(!json.contains("reference_example_id"));
        assert!(!json.contains("extra"));
    }

    #[test]
    fn test_run_create_clone() {
        let run = RunCreate::new(Uuid::new_v4(), "clone-test", RunType::Tool)
            .with_tags(vec!["tag".to_string()]);

        let cloned = run.clone();
        assert_eq!(run.id, cloned.id);
        assert_eq!(run.name, cloned.name);
        assert_eq!(run.tags, cloned.tags);
    }

    #[test]
    fn test_run_create_debug() {
        let run = RunCreate::new(Uuid::new_v4(), "debug-run", RunType::Prompt);
        let debug = format!("{:?}", run);
        assert!(debug.contains("RunCreate"));
        assert!(debug.contains("debug-run"));
    }

    // ===== RunUpdate Tests =====

    #[test]
    fn test_run_update_builder() {
        let update = RunUpdate::new()
            .with_end_time(Utc::now())
            .with_outputs(serde_json::json!({"result": "success"}))
            .with_error("test error");

        assert!(update.end_time.is_some());
        assert!(update.outputs.is_some());
        assert_eq!(update.error, Some("test error".to_string()));
    }

    #[test]
    fn test_run_update_new() {
        let update = RunUpdate::new();
        assert!(update.end_time.is_none());
        assert!(update.outputs.is_none());
        assert!(update.error.is_none());
        assert!(update.metadata.is_none());
        assert!(update.events.is_none());
        assert!(update.extra.is_none());
    }

    #[test]
    fn test_run_update_default() {
        let update = RunUpdate::default();
        assert!(update.end_time.is_none());
        assert!(update.outputs.is_none());
        assert!(update.error.is_none());
    }

    #[test]
    fn test_run_update_with_end_time() {
        let now = Utc::now();
        let update = RunUpdate::new().with_end_time(now);

        assert_eq!(update.end_time, Some(now));
    }

    #[test]
    fn test_run_update_with_outputs() {
        let outputs = serde_json::json!({"answer": "42", "confidence": 0.95});
        let update = RunUpdate::new().with_outputs(outputs.clone());

        assert_eq!(update.outputs, Some(outputs));
    }

    #[test]
    fn test_run_update_with_error() {
        let update = RunUpdate::new().with_error("Connection timeout");
        assert_eq!(update.error, Some("Connection timeout".to_string()));
    }

    #[test]
    fn test_run_update_with_error_string() {
        let error_string = String::from("Dynamic error");
        let update = RunUpdate::new().with_error(error_string.clone());
        assert_eq!(update.error, Some(error_string));
    }

    #[test]
    fn test_run_update_chaining() {
        let now = Utc::now();
        let update = RunUpdate::new()
            .with_end_time(now)
            .with_outputs(serde_json::json!({"status": "done"}))
            .with_error("partial failure");

        assert!(update.end_time.is_some());
        assert!(update.outputs.is_some());
        assert!(update.error.is_some());
    }

    #[test]
    fn test_run_update_serialization() {
        let update = RunUpdate::new()
            .with_outputs(serde_json::json!({"output": "test"}))
            .with_error("error msg");

        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains("output"));
        assert!(json.contains("test"));
        assert!(json.contains("error msg"));

        let deserialized: RunUpdate = serde_json::from_str(&json).unwrap();
        assert!(deserialized.outputs.is_some());
        assert_eq!(deserialized.error, Some("error msg".to_string()));
    }

    #[test]
    fn test_run_update_skip_serializing_none_fields() {
        let update = RunUpdate::new();
        let json = serde_json::to_string(&update).unwrap();

        // Should be nearly empty JSON object
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_run_update_clone() {
        let update = RunUpdate::new()
            .with_error("clone test")
            .with_outputs(serde_json::json!({"k": "v"}));

        let cloned = update.clone();
        assert_eq!(update.error, cloned.error);
        assert_eq!(update.outputs, cloned.outputs);
    }

    #[test]
    fn test_run_update_debug() {
        let update = RunUpdate::new().with_error("debug error");
        let debug = format!("{:?}", update);
        assert!(debug.contains("RunUpdate"));
        assert!(debug.contains("debug error"));
    }

    // ===== Run Tests =====

    #[test]
    fn test_run_struct_serialization() {
        let run = Run {
            id: Uuid::new_v4(),
            name: "test-run".to_string(),
            run_type: RunType::Llm,
            start_time: Utc::now(),
            end_time: None,
            parent_run_id: None,
            inputs: Some(serde_json::json!({"prompt": "hello"})),
            outputs: Some(serde_json::json!({"response": "hi"})),
            error: None,
            serialized: None,
            tags: Some(vec!["tag1".to_string()]),
            metadata: None,
            session_name: Some("session".to_string()),
            reference_example_id: None,
            extra: None,
            events: None,
        };

        let json = serde_json::to_string(&run).unwrap();
        assert!(json.contains("test-run"));
        assert!(json.contains("llm"));
        assert!(json.contains("prompt"));
        assert!(json.contains("response"));

        let deserialized: Run = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "test-run");
        assert_eq!(deserialized.run_type, RunType::Llm);
    }

    #[test]
    fn test_run_struct_all_fields() {
        let run_id = Uuid::new_v4();
        let parent_id = Uuid::new_v4();
        let ref_id = Uuid::new_v4();
        let now = Utc::now();
        let mut metadata = HashMap::new();
        metadata.insert("k".to_string(), serde_json::json!("v"));
        let mut extra = HashMap::new();
        extra.insert("e".to_string(), serde_json::json!(1));

        let run = Run {
            id: run_id,
            name: "full-run".to_string(),
            run_type: RunType::Chain,
            start_time: now,
            end_time: Some(now),
            parent_run_id: Some(parent_id),
            inputs: Some(serde_json::json!({})),
            outputs: Some(serde_json::json!({})),
            error: Some("err".to_string()),
            serialized: Some(serde_json::json!({})),
            tags: Some(vec!["t".to_string()]),
            metadata: Some(metadata),
            session_name: Some("sess".to_string()),
            reference_example_id: Some(ref_id),
            extra: Some(extra),
            events: Some(vec![serde_json::json!({"event": "start"})]),
        };

        let json = serde_json::to_string(&run).unwrap();
        let deserialized: Run = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, run_id);
        assert_eq!(deserialized.parent_run_id, Some(parent_id));
        assert_eq!(deserialized.reference_example_id, Some(ref_id));
        assert!(deserialized.events.is_some());
    }

    #[test]
    fn test_run_struct_clone() {
        let run = Run {
            id: Uuid::new_v4(),
            name: "clone".to_string(),
            run_type: RunType::Tool,
            start_time: Utc::now(),
            end_time: None,
            parent_run_id: None,
            inputs: None,
            outputs: None,
            error: None,
            serialized: None,
            tags: None,
            metadata: None,
            session_name: None,
            reference_example_id: None,
            extra: None,
            events: None,
        };

        let cloned = run.clone();
        assert_eq!(run.id, cloned.id);
        assert_eq!(run.name, cloned.name);
    }

    #[test]
    fn test_run_struct_debug() {
        let run = Run {
            id: Uuid::new_v4(),
            name: "debug-run".to_string(),
            run_type: RunType::Parser,
            start_time: Utc::now(),
            end_time: None,
            parent_run_id: None,
            inputs: None,
            outputs: None,
            error: None,
            serialized: None,
            tags: None,
            metadata: None,
            session_name: None,
            reference_example_id: None,
            extra: None,
            events: None,
        };

        let debug = format!("{:?}", run);
        assert!(debug.contains("Run"));
        assert!(debug.contains("debug-run"));
    }

    // ===== Edge Cases =====

    #[test]
    fn test_run_create_name_from_string() {
        let name = String::from("dynamic name");
        let run = RunCreate::new(Uuid::new_v4(), name.clone(), RunType::Llm);
        assert_eq!(run.name, name);
    }

    #[test]
    fn test_run_create_name_from_str() {
        let run = RunCreate::new(Uuid::new_v4(), "static name", RunType::Chain);
        assert_eq!(run.name, "static name");
    }

    #[test]
    fn test_run_update_outputs_null() {
        let update = RunUpdate::new().with_outputs(serde_json::Value::Null);
        assert_eq!(update.outputs, Some(serde_json::Value::Null));
    }

    #[test]
    fn test_run_update_outputs_array() {
        let arr = serde_json::json!([1, 2, 3]);
        let update = RunUpdate::new().with_outputs(arr.clone());
        assert_eq!(update.outputs, Some(arr));
    }

    #[test]
    fn test_run_create_inputs_complex() {
        let complex = serde_json::json!({
            "messages": [
                {"role": "user", "content": "Hello"},
                {"role": "assistant", "content": "Hi!"}
            ],
            "temperature": 0.7,
            "max_tokens": 100
        });

        let run = RunCreate::new(Uuid::new_v4(), "complex", RunType::Llm)
            .with_inputs(complex);

        assert!(run.inputs.is_some());
        let inputs = run.inputs.unwrap();
        assert!(inputs["messages"].is_array());
        assert_eq!(inputs["temperature"], 0.7);
    }
}
