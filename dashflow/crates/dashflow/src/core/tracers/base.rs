//! Base tracer trait and run tree structure

use crate::core::callbacks::CallbackHandler;
use crate::core::error::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Type of run being traced
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunType {
    /// LLM completion or chat
    Llm,
    /// Chain execution
    Chain,
    /// Tool execution
    Tool,
    /// Document retrieval
    Retriever,
    /// Embedding generation
    Embedding,
    /// Prompt rendering
    Prompt,
    /// Output parsing
    Parser,
}

/// A single run in the execution trace tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunTree {
    /// Unique run identifier
    pub id: Uuid,

    /// Name of the component being run
    pub name: String,

    /// Type of run
    pub run_type: RunType,

    /// When the run started
    pub start_time: DateTime<Utc>,

    /// When the run ended (if completed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<DateTime<Utc>>,

    /// Parent run ID (if this is a child run)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<Uuid>,

    /// Inputs to the run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<serde_json::Value>,

    /// Outputs from the run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<serde_json::Value>,

    /// Error if run failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Serialized representation of the component
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serialized: Option<serde_json::Value>,

    /// Tags for categorization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,

    /// Session/project name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_name: Option<String>,

    /// Reference to example ID (for evaluation/testing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_example_id: Option<Uuid>,

    /// Extra information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<HashMap<String, serde_json::Value>>,
}

impl RunTree {
    /// Create a new run tree
    pub fn new(id: Uuid, name: impl Into<String>, run_type: RunType) -> Self {
        Self {
            id,
            name: name.into(),
            run_type,
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
        }
    }

    /// Set the parent run ID
    #[must_use]
    pub fn with_parent(mut self, parent_run_id: Uuid) -> Self {
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

    /// Set metadata
    #[must_use]
    pub fn with_metadata(mut self, metadata: HashMap<String, serde_json::Value>) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Set the session name
    #[must_use]
    pub fn with_session_name(mut self, session_name: impl Into<String>) -> Self {
        self.session_name = Some(session_name.into());
        self
    }

    /// Mark the run as ended with outputs
    #[must_use]
    pub fn end(mut self, outputs: serde_json::Value) -> Self {
        self.end_time = Some(Utc::now());
        self.outputs = Some(outputs);
        self
    }

    /// Mark the run as failed with an error
    pub fn error(mut self, error: impl Into<String>) -> Self {
        self.end_time = Some(Utc::now());
        self.error = Some(error.into());
        self
    }
}

/// Base trait for tracers that persist run trees
///
/// Tracers extend the `CallbackHandler` interface to provide structured
/// tracing with run trees that can be persisted to external systems.
#[async_trait]
pub trait BaseTracer: CallbackHandler {
    /// Persist a completed run tree
    ///
    /// This is called when a run completes (successfully or with error).
    /// Implementations should persist the run tree to their backing store.
    async fn persist_run(&self, run: &RunTree) -> Result<()>;

    /// Get the current session/project name
    fn session_name(&self) -> Option<&str> {
        None
    }

    /// Set the session/project name
    fn set_session_name(&mut self, _session_name: impl Into<String>) {
        // Default implementation does nothing
    }
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;

    #[test]
    fn test_run_tree_creation() {
        let run_id = Uuid::new_v4();
        let run = RunTree::new(run_id, "TestRun", RunType::Chain);

        assert_eq!(run.id, run_id);
        assert_eq!(run.name, "TestRun");
        assert_eq!(run.run_type, RunType::Chain);
        assert!(run.end_time.is_none());
    }

    #[test]
    fn test_run_tree_builder() {
        let run_id = Uuid::new_v4();
        let parent_id = Uuid::new_v4();

        let run = RunTree::new(run_id, "TestRun", RunType::Llm)
            .with_parent(parent_id)
            .with_inputs(serde_json::json!({"prompt": "test"}))
            .with_tags(vec!["test".to_string()])
            .with_session_name("test-session");

        assert_eq!(run.parent_run_id, Some(parent_id));
        assert!(run.inputs.is_some());
        assert_eq!(run.tags, Some(vec!["test".to_string()]));
        assert_eq!(run.session_name, Some("test-session".to_string()));
    }

    #[test]
    fn test_run_tree_end() {
        let run = RunTree::new(Uuid::new_v4(), "TestRun", RunType::Tool)
            .end(serde_json::json!({"result": "success"}));

        assert!(run.end_time.is_some());
        assert!(run.outputs.is_some());
        assert!(run.error.is_none());
    }

    #[test]
    fn test_run_tree_error() {
        let run =
            RunTree::new(Uuid::new_v4(), "TestRun", RunType::Chain).error("Something went wrong");

        assert!(run.end_time.is_some());
        assert_eq!(run.error, Some("Something went wrong".to_string()));
        assert!(run.outputs.is_none());
    }

    #[test]
    fn test_run_type_serialization() {
        let run_type = RunType::Llm;
        let json = serde_json::to_string(&run_type).unwrap();
        assert_eq!(json, "\"llm\"");

        let deserialized: RunType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, RunType::Llm);
    }
}
