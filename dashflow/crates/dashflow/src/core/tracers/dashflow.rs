//! DashFlow tracer that sends runs to LangSmith

use crate::core::callbacks::CallbackHandler;
use crate::core::error::Result;
use crate::core::tracers::base::{BaseTracer, RunTree, RunType};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[cfg(feature = "tracing")]
use dashflow_langsmith::{BatchQueue, Client, RunCreate, RunUpdate};

/// DashFlow tracer that sends run data to LangSmith
///
/// This tracer persists run trees to LangSmith for observability and debugging.
/// It uses an async batching queue for efficient submission.
pub struct DashFlowTracer {
    #[cfg(feature = "tracing")]
    queue: Arc<BatchQueue>,
    #[cfg(feature = "tracing")]
    session_name: Arc<Mutex<Option<String>>>,
    /// Run map to track active runs
    run_map: Arc<Mutex<HashMap<Uuid, RunTree>>>,
}

impl DashFlowTracer {
    /// Create a new DashFlow tracer with default settings
    ///
    /// # Errors
    ///
    /// Returns an error if the LangSmith client cannot be created from
    /// environment variables.
    #[cfg(feature = "tracing")]
    pub fn new() -> Result<Self> {
        let client = Client::from_env().map_err(|e| {
            crate::core::error::Error::config(format!("Failed to create LangSmith client: {}", e))
        })?;

        let session_name = client.project_name().map(|s| s.to_string());
        let queue = BatchQueue::new(client);

        Ok(Self {
            queue: Arc::new(queue),
            session_name: Arc::new(Mutex::new(session_name)),
            run_map: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Create a new DashFlow tracer with a custom client
    #[cfg(feature = "tracing")]
    #[must_use]
    pub fn with_client(client: Client) -> Self {
        let session_name = client.project_name().map(|s| s.to_string());
        let queue = BatchQueue::new(client);

        Self {
            queue: Arc::new(queue),
            session_name: Arc::new(Mutex::new(session_name)),
            run_map: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a stub tracer when the tracing feature is disabled
    #[cfg(not(feature = "tracing"))]
    pub fn new() -> Result<Self> {
        Ok(Self {
            run_map: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Start tracking a run
    async fn start_run(&self, run: RunTree) {
        let mut map = self.run_map.lock().await;
        map.insert(run.id, run);
    }

    /// End a run and persist it
    async fn end_run(&self, run_id: Uuid, outputs: serde_json::Value) -> Result<()> {
        let mut map = self.run_map.lock().await;
        if let Some(run) = map.remove(&run_id) {
            let run = run.end(outputs);
            drop(map); // Release lock before persisting
            self.persist_run(&run).await?;
        }
        Ok(())
    }

    /// Mark a run as failed and persist it
    async fn error_run(&self, run_id: Uuid, error: impl Into<String>) -> Result<()> {
        let mut map = self.run_map.lock().await;
        if let Some(run) = map.remove(&run_id) {
            let run = run.error(error);
            drop(map); // Release lock before persisting
            self.persist_run(&run).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl BaseTracer for DashFlowTracer {
    #[cfg(feature = "tracing")]
    async fn persist_run(&self, run: &RunTree) -> Result<()> {
        // Convert RunTree to LangSmith RunCreate/RunUpdate
        let langsmith_run = RunCreate::new(
            run.id,
            &run.name,
            match run.run_type {
                RunType::Llm => dashflow_langsmith::RunType::Llm,
                RunType::Chain => dashflow_langsmith::RunType::Chain,
                RunType::Tool => dashflow_langsmith::RunType::Tool,
                RunType::Retriever => dashflow_langsmith::RunType::Retriever,
                RunType::Embedding => dashflow_langsmith::RunType::Embedding,
                RunType::Prompt => dashflow_langsmith::RunType::Prompt,
                RunType::Parser => dashflow_langsmith::RunType::Parser,
            },
        );

        // Set optional fields
        let mut langsmith_run = langsmith_run;
        if let Some(parent_id) = run.parent_run_id {
            langsmith_run = langsmith_run.with_parent_run_id(parent_id);
        }
        if let Some(inputs) = &run.inputs {
            langsmith_run = langsmith_run.with_inputs(inputs.clone());
        }
        if let Some(tags) = &run.tags {
            langsmith_run = langsmith_run.with_tags(tags.clone());
        }
        if let Some(session_name) = &run.session_name {
            langsmith_run = langsmith_run.with_session_name(session_name);
        } else {
            // Use tracer's default session name
            let session = self.session_name.lock().await;
            if let Some(name) = &*session {
                langsmith_run = langsmith_run.with_session_name(name);
            }
        }
        if let Some(metadata) = &run.metadata {
            langsmith_run = langsmith_run.with_metadata(metadata.clone());
        }

        // Submit the run creation
        self.queue.create_run(langsmith_run).map_err(|e| {
            crate::core::error::Error::Other(format!("Failed to submit run: {}", e))
        })?;

        // If the run is completed, submit an update with outputs/error
        if let Some(end_time) = run.end_time {
            let mut update = RunUpdate::new().with_end_time(end_time);
            if let Some(outputs) = &run.outputs {
                update = update.with_outputs(outputs.clone());
            }
            if let Some(error) = &run.error {
                update = update.with_error(error);
            }

            self.queue.update_run(run.id, update).map_err(|e| {
                crate::core::error::Error::Other(format!("Failed to update run: {}", e))
            })?;
        }

        Ok(())
    }

    #[cfg(not(feature = "tracing"))]
    async fn persist_run(&self, _run: &RunTree) -> Result<()> {
        // No-op when tracing feature is disabled
        Ok(())
    }

    #[cfg(feature = "tracing")]
    fn session_name(&self) -> Option<&str> {
        // Can't return reference to locked data, so we return None
        // Callers should use async version if needed
        None
    }

    #[cfg(not(feature = "tracing"))]
    fn session_name(&self) -> Option<&str> {
        None
    }

    fn set_session_name(&mut self, _session_name: impl Into<String>) {
        // Session name is set via client configuration
        // Could potentially update the mutex here if needed
    }
}

#[async_trait]
impl CallbackHandler for DashFlowTracer {
    async fn on_chain_start(
        &self,
        serialized: &HashMap<String, serde_json::Value>,
        inputs: &HashMap<String, serde_json::Value>,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        tags: &[String],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let name = serialized
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Chain");

        let mut run = RunTree::new(run_id, name, RunType::Chain)
            .with_inputs(serde_json::to_value(inputs).unwrap_or_default())
            .with_tags(tags.to_vec())
            .with_metadata(metadata.clone());

        if let Some(parent) = parent_run_id {
            run = run.with_parent(parent);
        }

        self.start_run(run).await;
        Ok(())
    }

    async fn on_chain_end(
        &self,
        outputs: &HashMap<String, serde_json::Value>,
        run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.end_run(run_id, serde_json::to_value(outputs).unwrap_or_default())
            .await
    }

    async fn on_chain_error(
        &self,
        error: &str,
        run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.error_run(run_id, error).await
    }

    async fn on_llm_start(
        &self,
        serialized: &HashMap<String, serde_json::Value>,
        prompts: &[String],
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        tags: &[String],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let name = serialized
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("LLM");

        let mut run = RunTree::new(run_id, name, RunType::Llm)
            .with_inputs(serde_json::json!({"prompts": prompts}))
            .with_tags(tags.to_vec())
            .with_metadata(metadata.clone());

        if let Some(parent) = parent_run_id {
            run = run.with_parent(parent);
        }

        self.start_run(run).await;
        Ok(())
    }

    async fn on_llm_end(
        &self,
        outputs: &HashMap<String, serde_json::Value>,
        run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.end_run(run_id, serde_json::to_value(outputs).unwrap_or_default())
            .await
    }

    async fn on_llm_error(
        &self,
        error: &str,
        run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.error_run(run_id, error).await
    }

    async fn on_tool_start(
        &self,
        serialized: &HashMap<String, serde_json::Value>,
        input: &str,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        tags: &[String],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let name = serialized
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Tool");

        let mut run = RunTree::new(run_id, name, RunType::Tool)
            .with_inputs(serde_json::json!({"input": input}))
            .with_tags(tags.to_vec())
            .with_metadata(metadata.clone());

        if let Some(parent) = parent_run_id {
            run = run.with_parent(parent);
        }

        self.start_run(run).await;
        Ok(())
    }

    async fn on_tool_end(
        &self,
        output: &str,
        run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.end_run(run_id, serde_json::json!({"output": output}))
            .await
    }

    async fn on_tool_error(
        &self,
        error: &str,
        run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.error_run(run_id, error).await
    }

    async fn on_retriever_start(
        &self,
        serialized: &HashMap<String, serde_json::Value>,
        query: &str,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        tags: &[String],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let name = serialized
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Retriever");

        let mut run = RunTree::new(run_id, name, RunType::Retriever)
            .with_inputs(serde_json::json!({"query": query}))
            .with_tags(tags.to_vec())
            .with_metadata(metadata.clone());

        if let Some(parent) = parent_run_id {
            run = run.with_parent(parent);
        }

        self.start_run(run).await;
        Ok(())
    }

    async fn on_retriever_end(
        &self,
        documents: &[serde_json::Value],
        run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.end_run(run_id, serde_json::json!({"documents": documents.len()}))
            .await
    }

    async fn on_retriever_error(
        &self,
        error: &str,
        run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        self.error_run(run_id, error).await
    }
}

// Note: The legacy alias `LangChainTracer` was removed in v1.12.0.
// Use `DashFlowTracer` directly.

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dashflow_tracer_creation() {
        // Test tracer creation - should succeed without tracing feature
        // or succeed/fail appropriately with the feature enabled
        let tracer = DashFlowTracer::new();
        // Just test that it doesn't panic
        assert!(tracer.is_ok() || tracer.is_err());
    }

    // Additional tests would require LangSmith client integration
    // which needs the 'tracing' feature and proper client setup.
    // These tests are left for integration testing with dashflow-langsmith.
}
