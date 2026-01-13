//! Tracers that call listener functions on run lifecycle events
//!
//! This module provides `RootListenersTracer` for invoking custom listener
//! functions when runs start, end, or error. Used by `with_listeners()` method.

use crate::core::callbacks::CallbackHandler;
use crate::core::config::RunnableConfig;
use crate::core::error::Result;
use crate::core::tracers::base::{BaseTracer, RunTree};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Sync listener function type
///
/// Listeners can accept just the Run, or both Run and `RunnableConfig`
pub type Listener = Arc<dyn Fn(&RunTree, &RunnableConfig) + Send + Sync>;

/// Async listener function type
pub type AsyncListener = Arc<
    dyn Fn(
            &RunTree,
            &RunnableConfig,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        + Send
        + Sync,
>;

/// Tracer that calls listener functions on run start, end, and error
///
/// This tracer is used internally by `with_listeners()` to execute custom
/// callback functions at specific points in a Runnable's lifecycle.
///
/// # Behavior
///
/// - Only operates on the root run (ignores child runs)
/// - Calls `on_start` when the root run begins
/// - Calls `on_end` when the root run completes successfully
/// - Calls `on_error` when the root run fails
///
/// # Python Baseline Compatibility
///
/// Matches `RootListenersTracer` in `dashflow_core/tracers/root_listeners.py:23-76`.
pub struct RootListenersTracer {
    config: RunnableConfig,
    on_start: Option<AsyncListener>,
    on_end: Option<AsyncListener>,
    on_error: Option<AsyncListener>,
    root_id: Arc<RwLock<Option<Uuid>>>,
}

impl RootListenersTracer {
    /// Create a new `RootListenersTracer`
    #[must_use]
    pub fn new(
        config: RunnableConfig,
        on_start: Option<AsyncListener>,
        on_end: Option<AsyncListener>,
        on_error: Option<AsyncListener>,
    ) -> Self {
        Self {
            config,
            on_start,
            on_end,
            on_error,
            root_id: Arc::new(RwLock::new(None)),
        }
    }
}

#[async_trait]
impl CallbackHandler for RootListenersTracer {
    async fn on_chain_start(
        &self,
        _serialized: &HashMap<String, serde_json::Value>,
        _inputs: &HashMap<String, serde_json::Value>,
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        tags: &[String],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        // Only handle root run
        if parent_run_id.is_some() {
            return Ok(());
        }

        let mut root = self.root_id.write().await;
        if root.is_some() {
            return Ok(());
        }

        *root = Some(run_id);
        drop(root);

        if let Some(on_start) = &self.on_start {
            let run = RunTree::new(run_id, "", crate::core::tracers::base::RunType::Chain)
                .with_tags(tags.to_vec())
                .with_metadata(metadata.clone());

            on_start(&run, &self.config).await;
        }

        Ok(())
    }

    async fn on_chain_end(
        &self,
        outputs: &HashMap<String, serde_json::Value>,
        run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        let root = self.root_id.read().await;
        if *root != Some(run_id) {
            return Ok(());
        }
        drop(root);

        if let Some(on_end) = &self.on_end {
            let mut run = RunTree::new(run_id, "", crate::core::tracers::base::RunType::Chain);
            run.outputs = Some(serde_json::to_value(outputs)?);
            run.end_time = Some(chrono::Utc::now());

            on_end(&run, &self.config).await;
        }

        Ok(())
    }

    async fn on_chain_error(
        &self,
        error: &str,
        run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        let root = self.root_id.read().await;
        if *root != Some(run_id) {
            return Ok(());
        }
        drop(root);

        if let Some(on_error) = &self.on_error {
            let mut run = RunTree::new(run_id, "", crate::core::tracers::base::RunType::Chain);
            run.error = Some(error.to_string());
            run.end_time = Some(chrono::Utc::now());

            on_error(&run, &self.config).await;
        }

        Ok(())
    }

    async fn on_llm_start(
        &self,
        _serialized: &HashMap<String, serde_json::Value>,
        _prompts: &[String],
        run_id: Uuid,
        parent_run_id: Option<Uuid>,
        tags: &[String],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        // Same logic as on_chain_start for root LLM runs
        if parent_run_id.is_some() {
            return Ok(());
        }

        let mut root = self.root_id.write().await;
        if root.is_some() {
            return Ok(());
        }

        *root = Some(run_id);
        drop(root);

        if let Some(on_start) = &self.on_start {
            let run = RunTree::new(run_id, "", crate::core::tracers::base::RunType::Llm)
                .with_tags(tags.to_vec())
                .with_metadata(metadata.clone());

            on_start(&run, &self.config).await;
        }

        Ok(())
    }

    async fn on_llm_end(
        &self,
        _response: &HashMap<String, serde_json::Value>,
        run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        let root = self.root_id.read().await;
        if *root != Some(run_id) {
            return Ok(());
        }
        drop(root);

        if let Some(on_end) = &self.on_end {
            let mut run = RunTree::new(run_id, "", crate::core::tracers::base::RunType::Llm);
            run.end_time = Some(chrono::Utc::now());

            on_end(&run, &self.config).await;
        }

        Ok(())
    }

    async fn on_llm_error(
        &self,
        error: &str,
        run_id: Uuid,
        _parent_run_id: Option<Uuid>,
    ) -> Result<()> {
        let root = self.root_id.read().await;
        if *root != Some(run_id) {
            return Ok(());
        }
        drop(root);

        if let Some(on_error) = &self.on_error {
            let mut run = RunTree::new(run_id, "", crate::core::tracers::base::RunType::Llm);
            run.error = Some(error.to_string());
            run.end_time = Some(chrono::Utc::now());

            on_error(&run, &self.config).await;
        }

        Ok(())
    }
}

#[async_trait]
impl BaseTracer for RootListenersTracer {
    async fn persist_run(&self, _run: &RunTree) -> Result<()> {
        // RootListenersTracer doesn't persist runs - it just calls listeners
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::AsyncListener;
    use crate::test_prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::RwLock;

    #[tokio::test]
    async fn test_root_listeners_tracer_on_start() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let on_start: AsyncListener = Arc::new(move |_run, _config| {
            let counter = counter_clone.clone();
            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);
            })
        });

        let tracer = RootListenersTracer::new(RunnableConfig::new(), Some(on_start), None, None);

        let run_id = Uuid::new_v4();
        tracer
            .on_chain_start(
                &HashMap::new(),
                &HashMap::new(),
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_root_listeners_tracer_ignores_child_runs() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let on_start: AsyncListener = Arc::new(move |_run, _config| {
            let counter = counter_clone.clone();
            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);
            })
        });

        let tracer = RootListenersTracer::new(RunnableConfig::new(), Some(on_start), None, None);

        let parent_id = Uuid::new_v4();
        let child_id = Uuid::new_v4();

        // Child run should be ignored
        tracer
            .on_chain_start(
                &HashMap::new(),
                &HashMap::new(),
                child_id,
                Some(parent_id),
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();

        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_root_listeners_tracer_on_end() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let on_end: AsyncListener = Arc::new(move |_run, _config| {
            let counter = counter_clone.clone();
            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);
            })
        });

        let tracer = RootListenersTracer::new(RunnableConfig::new(), None, Some(on_end), None);

        let run_id = Uuid::new_v4();

        // Start the root run
        tracer
            .on_chain_start(
                &HashMap::new(),
                &HashMap::new(),
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();

        // End the root run
        tracer
            .on_chain_end(&HashMap::new(), run_id, None)
            .await
            .unwrap();

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_root_listeners_tracer_on_error() {
        let error_msg = Arc::new(RwLock::new(String::new()));
        let error_msg_clone = error_msg.clone();

        let on_error: AsyncListener = Arc::new(move |run, _config| {
            let error_msg = error_msg_clone.clone();
            let err = run.error.clone();
            Box::pin(async move {
                if let Some(e) = err {
                    *error_msg.write().await = e;
                }
            })
        });

        let tracer = RootListenersTracer::new(RunnableConfig::new(), None, None, Some(on_error));

        let run_id = Uuid::new_v4();

        // Start the root run
        tracer
            .on_chain_start(
                &HashMap::new(),
                &HashMap::new(),
                run_id,
                None,
                &[],
                &HashMap::new(),
            )
            .await
            .unwrap();

        // Error on the root run
        tracer
            .on_chain_error("test error", run_id, None)
            .await
            .unwrap();

        let msg = error_msg.read().await;
        assert_eq!(*msg, "test error");
    }
}
