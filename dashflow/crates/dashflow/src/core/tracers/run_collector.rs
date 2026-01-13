//! A tracer that collects all nested runs in a list.
//!
//! This tracer is useful for inspection and evaluation purposes.

use crate::core::callbacks::CallbackHandler;
use crate::core::error::Result;
use crate::core::tracers::base::{BaseTracer, RunTree};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Tracer that collects all nested runs in a list.
///
/// This tracer is useful for inspection and evaluation purposes.
#[derive(Debug, Clone)]
pub struct RunCollectorCallbackHandler {
    /// The ID of the example being traced (for evaluation)
    example_id: Option<Uuid>,
    /// Collected runs
    traced_runs: Arc<Mutex<Vec<RunTree>>>,
}

impl RunCollectorCallbackHandler {
    /// Create a new `RunCollectorCallbackHandler`
    ///
    /// # Arguments
    /// * `example_id` - Optional example ID to associate with all collected runs
    #[must_use]
    pub fn new(example_id: Option<Uuid>) -> Self {
        Self {
            example_id,
            traced_runs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get all collected runs
    #[must_use]
    pub fn get_traced_runs(&self) -> Vec<RunTree> {
        self.traced_runs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Clear all collected runs
    pub fn clear(&self) {
        self.traced_runs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }

    /// Get the number of collected runs
    #[must_use]
    pub fn len(&self) -> usize {
        self.traced_runs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }

    /// Check if any runs have been collected
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.traced_runs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty()
    }
}

#[async_trait]
impl CallbackHandler for RunCollectorCallbackHandler {
    // Use default implementations for all callback methods
}

#[async_trait]
impl BaseTracer for RunCollectorCallbackHandler {
    /// Persist a run by adding it to the `traced_runs` list
    async fn persist_run(&self, run: &RunTree) -> Result<()> {
        let mut run = run.clone();
        // Set the reference_example_id if provided
        run.reference_example_id = self.example_id;

        self.traced_runs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(run);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::core::tracers::base::{BaseTracer, RunType};
    use crate::test_prelude::*;

    #[tokio::test]
    async fn test_run_collector_creation() {
        let collector = RunCollectorCallbackHandler::new(None);
        assert!(collector.is_empty());
        assert_eq!(collector.len(), 0);
    }

    #[tokio::test]
    async fn test_run_collector_with_example_id() {
        let example_id = Uuid::new_v4();
        let collector = RunCollectorCallbackHandler::new(Some(example_id));

        let run = RunTree::new(Uuid::new_v4(), "TestRun", RunType::Chain);
        collector.persist_run(&run).await.unwrap();

        let runs = collector.get_traced_runs();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].reference_example_id, Some(example_id));
        assert_eq!(runs[0].name, "TestRun");
    }

    #[tokio::test]
    async fn test_run_collector_multiple_runs() {
        let collector = RunCollectorCallbackHandler::new(None);

        for i in 0..5 {
            let run = RunTree::new(Uuid::new_v4(), format!("TestRun{}", i), RunType::Llm);
            collector.persist_run(&run).await.unwrap();
        }

        assert_eq!(collector.len(), 5);
        let runs = collector.get_traced_runs();
        assert_eq!(runs.len(), 5);

        for (i, run) in runs.iter().enumerate() {
            assert_eq!(run.name, format!("TestRun{}", i));
            assert_eq!(run.run_type, RunType::Llm);
        }
    }

    #[tokio::test]
    async fn test_run_collector_clear() {
        let collector = RunCollectorCallbackHandler::new(None);

        for i in 0..3 {
            let run = RunTree::new(Uuid::new_v4(), format!("TestRun{}", i), RunType::Tool);
            collector.persist_run(&run).await.unwrap();
        }

        assert_eq!(collector.len(), 3);

        collector.clear();
        assert!(collector.is_empty());
        assert_eq!(collector.len(), 0);
    }

    #[tokio::test]
    async fn test_run_collector_nested_runs() {
        let collector = RunCollectorCallbackHandler::new(None);

        // Create parent run
        let parent_id = Uuid::new_v4();
        let parent_run = RunTree::new(parent_id, "ParentRun", RunType::Chain);
        collector.persist_run(&parent_run).await.unwrap();

        // Create child runs
        for i in 0..3 {
            let child_run = RunTree::new(Uuid::new_v4(), format!("ChildRun{}", i), RunType::Llm)
                .with_parent(parent_id);
            collector.persist_run(&child_run).await.unwrap();
        }

        assert_eq!(collector.len(), 4); // 1 parent + 3 children
        let runs = collector.get_traced_runs();

        // First should be parent
        assert_eq!(runs[0].name, "ParentRun");
        assert!(runs[0].parent_run_id.is_none());

        // Rest should be children with parent_run_id set
        for (idx, run) in runs.iter().enumerate().skip(1) {
            assert_eq!(run.name, format!("ChildRun{}", idx - 1));
            assert_eq!(run.parent_run_id, Some(parent_id));
        }
    }
}
