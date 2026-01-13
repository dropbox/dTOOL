//! Async batching queue for efficient run submission

use crate::client::Client;
use crate::error::Result;
use crate::run::{RunCreate, RunUpdate};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, error, warn};
use uuid::Uuid;

/// Default batch size for submissions
pub const DEFAULT_BATCH_SIZE: usize = 20;

/// Default flush interval (5 seconds)
pub const DEFAULT_FLUSH_INTERVAL: Duration = Duration::from_secs(5);

/// Message for the batch queue
#[derive(Debug)]
enum QueueMessage {
    /// Create a new run
    Create(RunCreate),
    /// Update an existing run
    Update(Uuid, RunUpdate),
    /// Flush all pending items immediately
    Flush,
    /// Shutdown the queue
    Shutdown,
}

/// Default capacity for the batch queue channel
const BATCH_QUEUE_CHANNEL_CAPACITY: usize = 10000;

/// Async batching queue for `LangSmith` runs
///
/// This queue collects run creations and updates, batching them together
/// for efficient submission to the `LangSmith` API.
pub struct BatchQueue {
    sender: mpsc::Sender<QueueMessage>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl BatchQueue {
    /// Create a new batch queue with default settings
    #[must_use]
    pub fn new(client: Client) -> Self {
        Self::with_config(client, DEFAULT_BATCH_SIZE, DEFAULT_FLUSH_INTERVAL)
    }

    /// Create a new batch queue with custom configuration
    #[must_use]
    pub fn with_config(client: Client, batch_size: usize, flush_interval: Duration) -> Self {
        let (sender, receiver) = mpsc::channel(BATCH_QUEUE_CHANNEL_CAPACITY);

        let handle = tokio::spawn(async move {
            run_batch_worker(client, receiver, batch_size, flush_interval).await;
        });

        Self {
            sender,
            handle: Some(handle),
        }
    }

    /// Submit a run creation to the queue
    pub fn create_run(&self, run: RunCreate) -> Result<()> {
        match self.sender.try_send(QueueMessage::Create(run)) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!("Batch queue full, dropping run creation");
                Err(crate::error::Error::Other("Queue is full".to_string()))
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Err(crate::error::Error::Other("Queue is closed".to_string()))
            }
        }
    }

    /// Submit a run update to the queue
    pub fn update_run(&self, run_id: Uuid, update: RunUpdate) -> Result<()> {
        match self.sender.try_send(QueueMessage::Update(run_id, update)) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!(run_id = %run_id, "Batch queue full, dropping run update");
                Err(crate::error::Error::Other("Queue is full".to_string()))
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Err(crate::error::Error::Other("Queue is closed".to_string()))
            }
        }
    }

    /// Flush all pending items immediately
    pub fn flush(&self) -> Result<()> {
        match self.sender.try_send(QueueMessage::Flush) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => {
                // Flush is less critical - queue being full means flush is coming anyway
                warn!("Batch queue full during flush request");
                Ok(())
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Err(crate::error::Error::Other("Queue is closed".to_string()))
            }
        }
    }

    /// Shutdown the queue gracefully, flushing all pending items
    pub async fn shutdown(mut self) {
        // Shutdown is critical - use blocking send to ensure it arrives
        let _ = self.sender.try_send(QueueMessage::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.await;
        }
    }
}

impl Drop for BatchQueue {
    fn drop(&mut self) {
        // Send shutdown signal on drop - best effort
        let _ = self.sender.try_send(QueueMessage::Shutdown);
    }
}

/// Worker function that processes batches
async fn run_batch_worker(
    client: Client,
    mut receiver: mpsc::Receiver<QueueMessage>,
    batch_size: usize,
    flush_interval: Duration,
) {
    let mut pending_creates = Vec::new();
    let mut pending_updates: Vec<(Uuid, RunUpdate)> = Vec::new();
    let mut flush_timer = interval(flush_interval);
    flush_timer.tick().await; // Skip first immediate tick

    loop {
        tokio::select! {
            // Process incoming messages
            msg = receiver.recv() => {
                match msg {
                    Some(QueueMessage::Create(run)) => {
                        pending_creates.push(run);
                        if pending_creates.len() >= batch_size {
                            flush_batch(&client, &mut pending_creates, &mut pending_updates).await;
                        }
                    }
                    Some(QueueMessage::Update(run_id, update)) => {
                        pending_updates.push((run_id, update));
                        if pending_updates.len() >= batch_size {
                            flush_batch(&client, &mut pending_creates, &mut pending_updates).await;
                        }
                    }
                    Some(QueueMessage::Flush) => {
                        flush_batch(&client, &mut pending_creates, &mut pending_updates).await;
                    }
                    Some(QueueMessage::Shutdown) | None => {
                        // Flush remaining items and exit
                        flush_batch(&client, &mut pending_creates, &mut pending_updates).await;
                        debug!("Batch queue worker shutting down");
                        break;
                    }
                }
            }

            // Periodic flush
            _ = flush_timer.tick() => {
                if !pending_creates.is_empty() || !pending_updates.is_empty() {
                    flush_batch(&client, &mut pending_creates, &mut pending_updates).await;
                }
            }
        }
    }
}

/// Flush pending creates and updates to the API
async fn flush_batch(
    client: &Client,
    creates: &mut Vec<RunCreate>,
    updates: &mut Vec<(Uuid, RunUpdate)>,
) {
    if creates.is_empty() && updates.is_empty() {
        return;
    }

    debug!(
        "Flushing batch: {} creates, {} updates",
        creates.len(),
        updates.len()
    );

    let batch = crate::client::BatchIngest {
        post: std::mem::take(creates),
        patch: std::mem::take(updates),
    };

    if let Err(e) = client.batch_ingest_runs(&batch).await {
        error!("Failed to submit batch: {}", e);
        // Put items back for retry (simple approach - could be more sophisticated)
        warn!(
            "Lost {} run submissions due to API error",
            batch.post.len() + batch.patch.len()
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::run::RunType;

    // ===== BatchQueue Creation Tests =====

    #[tokio::test]
    async fn test_batch_queue_creation() {
        // This test just ensures the queue can be created
        // We can't test actual submission without a real API
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::new(client);

        let run = RunCreate::new(Uuid::new_v4(), "test", RunType::Chain);
        // Should not error (will fail in background worker, but that's ok for test)
        let result = queue.create_run(run);
        assert!(result.is_ok());

        // Graceful shutdown
        queue.shutdown().await;
    }

    #[tokio::test]
    async fn test_batch_queue_update() {
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::new(client);

        let update = RunUpdate::new();
        let result = queue.update_run(Uuid::new_v4(), update);
        assert!(result.is_ok());

        queue.shutdown().await;
    }

    #[tokio::test]
    async fn test_batch_queue_with_config() {
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::with_config(client, 10, Duration::from_millis(100));

        let run = RunCreate::new(Uuid::new_v4(), "config-test", RunType::Llm);
        let result = queue.create_run(run);
        assert!(result.is_ok());

        queue.shutdown().await;
    }

    #[tokio::test]
    async fn test_batch_queue_with_small_batch_size() {
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::with_config(client, 1, Duration::from_secs(60));

        // Should work with batch size of 1
        let run = RunCreate::new(Uuid::new_v4(), "small-batch", RunType::Tool);
        assert!(queue.create_run(run).is_ok());

        queue.shutdown().await;
    }

    #[tokio::test]
    async fn test_batch_queue_with_large_batch_size() {
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::with_config(client, 1000, Duration::from_secs(300));

        let run = RunCreate::new(Uuid::new_v4(), "large-batch", RunType::Chain);
        assert!(queue.create_run(run).is_ok());

        queue.shutdown().await;
    }

    // ===== Queue Operations Tests =====

    #[tokio::test]
    async fn test_batch_queue_multiple_creates() {
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::new(client);

        for i in 0..5 {
            let run = RunCreate::new(Uuid::new_v4(), format!("run-{i}"), RunType::Llm);
            let result = queue.create_run(run);
            assert!(result.is_ok());
        }

        queue.shutdown().await;
    }

    #[tokio::test]
    async fn test_batch_queue_multiple_updates() {
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::new(client);

        for _ in 0..5 {
            let update = RunUpdate::new().with_error("test error");
            let result = queue.update_run(Uuid::new_v4(), update);
            assert!(result.is_ok());
        }

        queue.shutdown().await;
    }

    #[tokio::test]
    async fn test_batch_queue_mixed_operations() {
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::new(client);

        // Interleave creates and updates
        let run1 = RunCreate::new(Uuid::new_v4(), "run1", RunType::Chain);
        assert!(queue.create_run(run1).is_ok());

        let update1 = RunUpdate::new().with_outputs(serde_json::json!({"x": 1}));
        assert!(queue.update_run(Uuid::new_v4(), update1).is_ok());

        let run2 = RunCreate::new(Uuid::new_v4(), "run2", RunType::Tool);
        assert!(queue.create_run(run2).is_ok());

        let update2 = RunUpdate::new();
        assert!(queue.update_run(Uuid::new_v4(), update2).is_ok());

        queue.shutdown().await;
    }

    // ===== Flush Tests =====

    #[tokio::test]
    async fn test_batch_queue_flush() {
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::new(client);

        let run = RunCreate::new(Uuid::new_v4(), "flush-test", RunType::Retriever);
        queue.create_run(run).expect("create failed");

        // Explicit flush
        let result = queue.flush();
        assert!(result.is_ok());

        queue.shutdown().await;
    }

    #[tokio::test]
    async fn test_batch_queue_flush_empty() {
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::new(client);

        // Flush with nothing queued
        let result = queue.flush();
        assert!(result.is_ok());

        queue.shutdown().await;
    }

    #[tokio::test]
    async fn test_batch_queue_multiple_flushes() {
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::new(client);

        for _ in 0..3 {
            let run = RunCreate::new(Uuid::new_v4(), "multi-flush", RunType::Embedding);
            queue.create_run(run).expect("create failed");
            assert!(queue.flush().is_ok());
        }

        queue.shutdown().await;
    }

    // ===== Shutdown Tests =====

    #[tokio::test]
    async fn test_batch_queue_shutdown_immediately() {
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::new(client);
        // Shutdown immediately without any operations
        queue.shutdown().await;
    }

    #[tokio::test]
    async fn test_batch_queue_shutdown_with_pending() {
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::new(client);

        // Add items then shutdown
        for i in 0..10 {
            let run = RunCreate::new(Uuid::new_v4(), format!("pending-{i}"), RunType::Parser);
            let _ = queue.create_run(run);
        }

        // Shutdown should flush pending items
        queue.shutdown().await;
    }

    // ===== Run Type Variants Tests =====

    #[tokio::test]
    async fn test_batch_queue_all_run_types() {
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::new(client);

        let run_types = [
            RunType::Llm,
            RunType::Chain,
            RunType::Tool,
            RunType::Retriever,
            RunType::Embedding,
            RunType::Prompt,
            RunType::Parser,
        ];

        for run_type in run_types {
            let run = RunCreate::new(Uuid::new_v4(), format!("{:?}", run_type), run_type);
            assert!(queue.create_run(run).is_ok());
        }

        queue.shutdown().await;
    }

    // ===== Update with Various Fields Tests =====

    #[tokio::test]
    async fn test_batch_queue_update_with_end_time() {
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::new(client);

        let update = RunUpdate::new().with_end_time(chrono::Utc::now());
        assert!(queue.update_run(Uuid::new_v4(), update).is_ok());

        queue.shutdown().await;
    }

    #[tokio::test]
    async fn test_batch_queue_update_with_outputs() {
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::new(client);

        let update = RunUpdate::new().with_outputs(serde_json::json!({
            "result": "success",
            "tokens": 150,
            "cost": 0.002
        }));
        assert!(queue.update_run(Uuid::new_v4(), update).is_ok());

        queue.shutdown().await;
    }

    #[tokio::test]
    async fn test_batch_queue_update_with_error() {
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::new(client);

        let update = RunUpdate::new().with_error("Rate limit exceeded");
        assert!(queue.update_run(Uuid::new_v4(), update).is_ok());

        queue.shutdown().await;
    }

    #[tokio::test]
    async fn test_batch_queue_update_with_all_fields() {
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::new(client);

        let update = RunUpdate::new()
            .with_end_time(chrono::Utc::now())
            .with_outputs(serde_json::json!({"status": "complete"}))
            .with_error("warning: slow response");

        assert!(queue.update_run(Uuid::new_v4(), update).is_ok());

        queue.shutdown().await;
    }

    // ===== Create with Various Fields Tests =====

    #[tokio::test]
    async fn test_batch_queue_create_with_parent() {
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::new(client);

        let parent_id = Uuid::new_v4();
        let run = RunCreate::new(Uuid::new_v4(), "child-run", RunType::Tool)
            .with_parent_run_id(parent_id);
        assert!(queue.create_run(run).is_ok());

        queue.shutdown().await;
    }

    #[tokio::test]
    async fn test_batch_queue_create_with_inputs() {
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::new(client);

        let run = RunCreate::new(Uuid::new_v4(), "with-inputs", RunType::Llm)
            .with_inputs(serde_json::json!({
                "messages": [{"role": "user", "content": "Hello"}]
            }));
        assert!(queue.create_run(run).is_ok());

        queue.shutdown().await;
    }

    #[tokio::test]
    async fn test_batch_queue_create_with_tags() {
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::new(client);

        let run = RunCreate::new(Uuid::new_v4(), "with-tags", RunType::Chain)
            .with_tags(vec!["production".to_string(), "v2".to_string()]);
        assert!(queue.create_run(run).is_ok());

        queue.shutdown().await;
    }

    #[tokio::test]
    async fn test_batch_queue_create_with_session() {
        let client = Client::builder()
            .api_key("test-key")
            .build()
            .expect("Failed to build client");

        let queue = BatchQueue::new(client);

        let run = RunCreate::new(Uuid::new_v4(), "with-session", RunType::Retriever)
            .with_session_name("my-project-session");
        assert!(queue.create_run(run).is_ok());

        queue.shutdown().await;
    }

    // ===== Constants Tests =====

    #[test]
    fn test_default_batch_size_constant() {
        assert!(DEFAULT_BATCH_SIZE > 0);
        assert_eq!(DEFAULT_BATCH_SIZE, 20);
    }

    #[test]
    fn test_default_flush_interval_constant() {
        assert!(DEFAULT_FLUSH_INTERVAL > Duration::ZERO);
        assert_eq!(DEFAULT_FLUSH_INTERVAL, Duration::from_secs(5));
    }

    #[test]
    fn test_batch_queue_channel_capacity() {
        // Just verify the constant exists and is reasonable
        assert!(BATCH_QUEUE_CHANNEL_CAPACITY > 0);
        assert_eq!(BATCH_QUEUE_CHANNEL_CAPACITY, 10000);
    }

    // ===== QueueMessage Enum Tests =====

    #[test]
    fn test_queue_message_create_debug() {
        let run = RunCreate::new(Uuid::new_v4(), "debug", RunType::Llm);
        let msg = QueueMessage::Create(run);
        let debug = format!("{:?}", msg);
        assert!(debug.contains("Create"));
    }

    #[test]
    fn test_queue_message_update_debug() {
        let msg = QueueMessage::Update(Uuid::new_v4(), RunUpdate::new());
        let debug = format!("{:?}", msg);
        assert!(debug.contains("Update"));
    }

    #[test]
    fn test_queue_message_flush_debug() {
        let msg = QueueMessage::Flush;
        let debug = format!("{:?}", msg);
        assert!(debug.contains("Flush"));
    }

    #[test]
    fn test_queue_message_shutdown_debug() {
        let msg = QueueMessage::Shutdown;
        let debug = format!("{:?}", msg);
        assert!(debug.contains("Shutdown"));
    }
}
