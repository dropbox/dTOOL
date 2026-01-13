// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
// DashFlow Streaming - SQLite Backend

//! # SQLite Streaming Backend
//!
//! A SQLite-based backend for lightweight persistence without external dependencies.
//! Ideal for simple deployments, single-node applications, and development.
//!
//! ## Safety & Concurrency
//!
//! `rusqlite::Connection` is not `Send`/`Sync`. To avoid UB and runtime blocking,
//! this backend runs all SQLite I/O on a dedicated blocking worker thread and
//! communicates via async channels.

use super::traits::{BackendError, BackendResult, StreamBackend, StreamConsumer, StreamProducer};
use crate::codec::{decode_message_compatible, encode_message, DEFAULT_MAX_PAYLOAD_SIZE};
use crate::DashStreamMessage;
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::warn;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

/// Commands sent to the SQLite worker thread.
enum WorkerCommand {
    Insert {
        topic: String,
        data: Vec<u8>,
        timestamp_us: i64,
        resp: oneshot::Sender<BackendResult<()>>,
    },
    Fetch {
        topic: String,
        offset: i64,
        resp: oneshot::Sender<BackendResult<Option<Vec<u8>>>>,
    },
    GetCommitted {
        group_id: String,
        topic: String,
        resp: oneshot::Sender<BackendResult<Option<i64>>>,
    },
    SetCommitted {
        group_id: String,
        topic: String,
        committed_offset: i64,
        updated_at: i64,
        resp: oneshot::Sender<BackendResult<()>>,
    },
    Health {
        resp: oneshot::Sender<BackendResult<()>>,
    },
    Close,
}

/// Start a blocking worker thread that owns the SQLite connection.
async fn start_worker(path: Option<PathBuf>) -> BackendResult<mpsc::Sender<WorkerCommand>> {
    let (tx, mut rx) = mpsc::channel::<WorkerCommand>(128);
    let (ready_tx, ready_rx) = oneshot::channel::<BackendResult<()>>();

    tokio::task::spawn_blocking(move || {
        let open_result: BackendResult<rusqlite::Connection> = match path {
            Some(ref p) => match rusqlite::Connection::open(p) {
                Ok(conn) => {
                    if let Err(e) =
                        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
                    {
                        Err(BackendError::Database(e.to_string()))
                    } else {
                        Ok(conn)
                    }
                }
                Err(e) => Err(BackendError::Database(e.to_string())),
            },
            None => {
                // S-18: Log warning when using in-memory mode (data loss on restart)
                warn!(
                    "SQLite backend using in-memory storage - DATA WILL BE LOST on restart. \
                     Set path to enable persistence."
                );
                rusqlite::Connection::open_in_memory()
                    .map_err(|e| BackendError::Database(e.to_string()))
            }
        };

        let conn = match open_result {
            Ok(c) => c,
            Err(e) => {
                // M-630: Log initialization error if receiver dropped
                let err_msg = e.to_string();
                if ready_tx.send(Err(e)).is_err() {
                    warn!(error = %err_msg, "SQLite backend initialization error lost (receiver dropped)");
                }
                return;
            }
        };

        // Initialize schema.
        if let Err(e) = conn
            .execute_batch(
                r#"
            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                topic TEXT NOT NULL,
                offset INTEGER NOT NULL,
                timestamp_us INTEGER NOT NULL,
                data BLOB NOT NULL,
                UNIQUE(topic, offset)
            );

            CREATE INDEX IF NOT EXISTS idx_messages_topic_offset
            ON messages(topic, offset);

            CREATE TABLE IF NOT EXISTS consumer_offsets (
                group_id TEXT NOT NULL,
                topic TEXT NOT NULL,
                committed_offset INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY(group_id, topic)
            );
            "#,
            )
            .map_err(|e| BackendError::Database(e.to_string()))
        {
            // M-630: Log schema initialization error if receiver dropped
            let err_msg = e.to_string();
            if ready_tx.send(Err(e)).is_err() {
                warn!(error = %err_msg, "SQLite schema initialization error lost (receiver dropped)");
            }
            return;
        }

        // Success notification - if receiver dropped, backend was still initialized
        let _ = ready_tx.send(Ok(()));

        // Process commands serially.
        while let Some(cmd) = rx.blocking_recv() {
            match cmd {
                WorkerCommand::Insert {
                    topic,
                    data,
                    timestamp_us,
                    resp,
                } => {
                    let result = (|| {
                        let offset: Option<i64> = conn
                            .query_row(
                                "SELECT MAX(offset) FROM messages WHERE topic = ?",
                                [&topic],
                                |row| row.get(0),
                            )
                            .map_err(|e| BackendError::Database(e.to_string()))?;
                        let offset = offset.map_or(0, |o| o + 1);

                        conn.execute(
                            "INSERT INTO messages (topic, offset, timestamp_us, data) VALUES (?, ?, ?, ?)",
                            rusqlite::params![topic, offset, timestamp_us, data],
                        )
                        .map_err(|e| BackendError::Database(e.to_string()))?;
                        Ok(())
                    })();
                    // Log database errors that cannot be delivered to caller
                    let err_msg = result.as_ref().err().map(|e: &BackendError| e.to_string());
                    if resp.send(result).is_err() {
                        if let Some(e) = err_msg {
                            warn!(topic = %topic, error = %e, "Insert database error lost (receiver dropped)");
                        }
                    }
                }
                WorkerCommand::Fetch {
                    topic,
                    offset,
                    resp,
                } => {
                    let result = match conn.query_row(
                        "SELECT data FROM messages WHERE topic = ? AND offset = ?",
                        rusqlite::params![topic, offset],
                        |row| row.get::<_, Vec<u8>>(0),
                    ) {
                        Ok(data) => Ok(Some(data)),
                        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                        Err(e) => Err(BackendError::Database(e.to_string())),
                    };
                    // Log database errors that cannot be delivered to caller
                    let err_msg = result.as_ref().err().map(|e: &BackendError| e.to_string());
                    if resp.send(result).is_err() {
                        if let Some(e) = err_msg {
                            warn!(topic = %topic, offset = %offset, error = %e, "Fetch database error lost (receiver dropped)");
                        }
                    }
                }
                WorkerCommand::GetCommitted {
                    group_id,
                    topic,
                    resp,
                } => {
                    let result = match conn.query_row(
                        "SELECT committed_offset FROM consumer_offsets WHERE group_id = ? AND topic = ?",
                        rusqlite::params![group_id, topic],
                        |row| row.get::<_, i64>(0),
                    ) {
                        Ok(offset) => Ok(Some(offset)),
                        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                        Err(e) => Err(BackendError::Database(e.to_string())),
                    };
                    // Log database errors that cannot be delivered to caller
                    let err_msg = result.as_ref().err().map(|e: &BackendError| e.to_string());
                    if resp.send(result).is_err() {
                        if let Some(e) = err_msg {
                            warn!(group_id = %group_id, topic = %topic, error = %e, "GetCommitted database error lost (receiver dropped)");
                        }
                    }
                }
                WorkerCommand::SetCommitted {
                    group_id,
                    topic,
                    committed_offset,
                    updated_at,
                    resp,
                } => {
                    let result = conn
                        .execute(
                            r#"
                            INSERT OR REPLACE INTO consumer_offsets (group_id, topic, committed_offset, updated_at)
                            VALUES (?, ?, ?, ?)
                            "#,
                            rusqlite::params![group_id, topic, committed_offset, updated_at],
                        )
                        .map_err(|e| BackendError::Database(e.to_string()))
                        .map(|_| ());
                    // Log database errors that cannot be delivered to caller
                    let err_msg = result.as_ref().err().map(|e: &BackendError| e.to_string());
                    if resp.send(result).is_err() {
                        if let Some(e) = err_msg {
                            warn!(group_id = %group_id, topic = %topic, committed_offset = %committed_offset, error = %e, "SetCommitted database error lost (receiver dropped)");
                        }
                    }
                }
                WorkerCommand::Health { resp } => {
                    let result = conn
                        .execute_batch("SELECT 1")
                        .map_err(|e| BackendError::Database(e.to_string()));
                    // Log database errors that cannot be delivered to caller
                    let err_msg = result.as_ref().err().map(|e: &BackendError| e.to_string());
                    if resp.send(result).is_err() {
                        if let Some(e) = err_msg {
                            warn!(error = %e, "Health check database error lost (receiver dropped)");
                        }
                    }
                }
                WorkerCommand::Close => break,
            }
        }
    });

    // Await worker readiness.
    match ready_rx.await {
        Ok(Ok(())) => Ok(tx),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(BackendError::Closed),
    }
}

/// SQLite-based streaming backend.
pub struct SqliteBackend {
    worker_tx: mpsc::Sender<WorkerCommand>,
    closed: AtomicBool,
}

impl SqliteBackend {
    /// Create a new SQLite backend with file-based database.
    pub async fn new(path: PathBuf) -> BackendResult<Self> {
        let worker_tx = start_worker(Some(path)).await?;
        Ok(Self {
            worker_tx,
            closed: AtomicBool::new(false),
        })
    }

    /// Create a new SQLite backend with in-memory database (for testing).
    pub async fn new_in_memory() -> BackendResult<Self> {
        let worker_tx = start_worker(None).await?;
        Ok(Self {
            worker_tx,
            closed: AtomicBool::new(false),
        })
    }
}

#[async_trait]
impl StreamBackend for SqliteBackend {
    type Producer = SqliteProducer;
    type Consumer = SqliteConsumer;

    async fn create_producer(&self, topic: &str) -> BackendResult<Self::Producer> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(BackendError::Closed);
        }
        Ok(SqliteProducer {
            topic: topic.to_string(),
            worker_tx: self.worker_tx.clone(),
        })
    }

    async fn create_consumer(&self, topic: &str, group_id: &str) -> BackendResult<Self::Consumer> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(BackendError::Closed);
        }

        let (resp_tx, resp_rx) = oneshot::channel();
        self.worker_tx
            .send(WorkerCommand::GetCommitted {
                group_id: group_id.to_string(),
                topic: topic.to_string(),
                resp: resp_tx,
            })
            .await
            .map_err(|e| BackendError::ChannelError(format!("create_consumer send failed: {e}")))?;

        let start_offset = resp_rx
            .await
            .map_err(|e| BackendError::ChannelError(format!("create_consumer recv failed: {e}")))??
            .unwrap_or(0);

        Ok(SqliteConsumer {
            topic: topic.to_string(),
            group_id: group_id.to_string(),
            worker_tx: self.worker_tx.clone(),
            current_offset: start_offset,
        })
    }

    async fn health_check(&self) -> BackendResult<()> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(BackendError::Closed);
        }
        let (resp_tx, resp_rx) = oneshot::channel();
        self.worker_tx
            .send(WorkerCommand::Health { resp: resp_tx })
            .await
            .map_err(|e| BackendError::ChannelError(format!("health_check send failed: {e}")))?;
        resp_rx.await.map_err(|e| BackendError::ChannelError(format!("health_check recv failed: {e}")))?
    }

    async fn close(&self) -> BackendResult<()> {
        self.closed.store(true, Ordering::Relaxed);
        // Best-effort shutdown signal.
        let _ = self.worker_tx.send(WorkerCommand::Close).await;
        Ok(())
    }
}

/// SQLite producer.
pub struct SqliteProducer {
    topic: String,
    worker_tx: mpsc::Sender<WorkerCommand>,
}

#[async_trait]
impl StreamProducer for SqliteProducer {
    async fn send(&self, message: DashStreamMessage) -> BackendResult<()> {
        let data =
            encode_message(&message).map_err(|e| BackendError::Serialization(e.to_string()))?;
        let timestamp_us = chrono::Utc::now().timestamp_micros();

        let (resp_tx, resp_rx) = oneshot::channel();
        self.worker_tx
            .send(WorkerCommand::Insert {
                topic: self.topic.clone(),
                data,
                timestamp_us,
                resp: resp_tx,
            })
            .await
            .map_err(|e| BackendError::ChannelError(format!("producer send failed: {e}")))?;

        resp_rx.await.map_err(|e| BackendError::ChannelError(format!("producer recv failed: {e}")))?
    }

    async fn flush(&self) -> BackendResult<()> {
        // SQLite commits are synchronous, no flushing needed.
        Ok(())
    }

    fn topic(&self) -> &str {
        &self.topic
    }
}

/// SQLite consumer.
pub struct SqliteConsumer {
    topic: String,
    group_id: String,
    worker_tx: mpsc::Sender<WorkerCommand>,
    current_offset: i64,
}

#[async_trait]
impl StreamConsumer for SqliteConsumer {
    async fn next(&mut self) -> Option<BackendResult<DashStreamMessage>> {
        loop {
            let (resp_tx, resp_rx) = oneshot::channel();
            if let Err(e) = self
                .worker_tx
                .send(WorkerCommand::Fetch {
                    topic: self.topic.clone(),
                    offset: self.current_offset,
                    resp: resp_tx,
                })
                .await
            {
                return Some(Err(BackendError::ChannelError(format!(
                    "consumer next send failed: {e}"
                ))));
            }

            match resp_rx.await {
                Ok(Ok(Some(data))) => {
                    let message =
                        match decode_message_compatible(&data, DEFAULT_MAX_PAYLOAD_SIZE) {
                            Ok(m) => m,
                            Err(e) => {
                                return Some(Err(BackendError::Deserialization(e.to_string())))
                            }
                        };
                    self.current_offset += 1;
                    return Some(Ok(message));
                }
                Ok(Ok(None)) => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
                Ok(Err(e)) => return Some(Err(e)),
                Err(e) => {
                    return Some(Err(BackendError::ChannelError(format!(
                        "consumer next recv failed: {e}"
                    ))))
                }
            }
        }
    }

    async fn next_timeout(
        &mut self,
        timeout: Duration,
    ) -> Option<BackendResult<DashStreamMessage>> {
        tokio::time::timeout(timeout, self.next())
            .await
            .unwrap_or(None)
    }

    async fn commit(&mut self) -> BackendResult<()> {
        let updated_at = chrono::Utc::now().timestamp_micros();
        let (resp_tx, resp_rx) = oneshot::channel();
        self.worker_tx
            .send(WorkerCommand::SetCommitted {
                group_id: self.group_id.clone(),
                topic: self.topic.clone(),
                committed_offset: self.current_offset,
                updated_at,
                resp: resp_tx,
            })
            .await
            .map_err(|e| BackendError::ChannelError(format!("consumer commit send failed: {e}")))?;
        resp_rx.await.map_err(|e| BackendError::ChannelError(format!("consumer commit recv failed: {e}")))?
    }

    fn topic(&self) -> &str {
        &self.topic
    }

    fn group_id(&self) -> &str {
        &self.group_id
    }

    fn current_offset(&self) -> i64 {
        self.current_offset
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Event, EventType, Header, MessageType};

    fn create_test_event(thread_id: &str, sequence: u64) -> Event {
        Event {
            header: Some(Header {
                message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
                timestamp_us: chrono::Utc::now().timestamp_micros(),
                tenant_id: "test-tenant".to_string(),
                thread_id: thread_id.to_string(),
                sequence,
                r#type: MessageType::Event as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            event_type: EventType::GraphStart as i32,
            node_id: format!("node-{}", sequence),
            attributes: Default::default(),
            duration_us: 0,
            llm_request_id: "".to_string(),
        }
    }

    #[tokio::test]
    async fn test_backend_new_in_memory() {
        let backend = SqliteBackend::new_in_memory().await.unwrap();
        assert!(backend.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_create_producer() {
        let backend = SqliteBackend::new_in_memory().await.unwrap();
        let producer = backend.create_producer("test-topic").await.unwrap();
        assert_eq!(producer.topic(), "test-topic");
    }

    #[tokio::test]
    async fn test_create_consumer() {
        let backend = SqliteBackend::new_in_memory().await.unwrap();
        let consumer = backend
            .create_consumer("test-topic", "test-group")
            .await
            .unwrap();
        assert_eq!(consumer.topic(), "test-topic");
        assert_eq!(consumer.group_id(), "test-group");
        assert_eq!(consumer.current_offset(), 0);
    }

    #[tokio::test]
    async fn test_send_and_receive() {
        let backend = SqliteBackend::new_in_memory().await.unwrap();

        let producer = backend.create_producer("events").await.unwrap();
        let mut consumer = backend
            .create_consumer("events", "test-group")
            .await
            .unwrap();

        producer
            .send_event(create_test_event("thread-1", 1))
            .await
            .unwrap();

        let received = consumer
            .next_timeout(Duration::from_millis(100))
            .await
            .unwrap()
            .unwrap();

        match received.message {
            Some(crate::dash_stream_message::Message::Event(e)) => {
                assert_eq!(e.node_id, "node-1");
            }
            _ => panic!("Expected Event message"),
        }

        assert_eq!(consumer.current_offset(), 1);
    }

    #[tokio::test]
    async fn test_offset_persistence() {
        let backend = SqliteBackend::new_in_memory().await.unwrap();

        let producer = backend.create_producer("events").await.unwrap();
        for i in 1..=3 {
            producer
                .send_event(create_test_event("thread-1", i))
                .await
                .unwrap();
        }

        {
            let mut consumer = backend
                .create_consumer("events", "test-group")
                .await
                .unwrap();
            let _ = consumer.next_timeout(Duration::from_millis(100)).await;
            let _ = consumer.next_timeout(Duration::from_millis(100)).await;
            consumer.commit().await.unwrap();
        }

        let consumer = backend
            .create_consumer("events", "test-group")
            .await
            .unwrap();
        assert_eq!(consumer.current_offset(), 2);
    }
}
