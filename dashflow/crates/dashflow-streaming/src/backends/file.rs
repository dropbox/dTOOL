// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
// DashFlow Streaming - File Backend (JSONL)

//! # File-Based Streaming Backend
//!
//! A file-based backend using JSONL (JSON Lines) format for local development
//! and debugging. Each topic is stored as a separate `.jsonl` file.
//!
//! ## Features
//!
//! - Human-readable JSONL format for easy debugging
//! - Persistent storage across restarts
//! - File rotation support (optional)
//! - Consumer group offset tracking in `.offsets` files
//! - Compatible with standard JSONL tools
//!
//! ## Example
//!
//! ```rust,no_run
//! use dashflow_streaming::backends::{StreamBackend, FileBackend};
//! use std::path::PathBuf;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let backend = FileBackend::new(PathBuf::from("/tmp/dashflow-streams"));
//!
//!     // Create producer and consumer
//!     let producer = backend.create_producer("events").await?;
//!     let mut consumer = backend.create_consumer("events", "debug-group").await?;
//!
//!     // Files created:
//!     // - /tmp/dashflow-streams/events.jsonl
//!     // - /tmp/dashflow-streams/.offsets/debug-group.json
//!
//!     Ok(())
//! }
//! ```

use super::traits::{BackendError, BackendResult, StreamBackend, StreamConsumer, StreamProducer};
use crate::codec::{decode_message_compatible, encode_message, DEFAULT_MAX_PAYLOAD_SIZE};
use crate::DashStreamMessage;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::RwLock;

/// JSONL message format
#[derive(Serialize, Deserialize)]
struct JsonlMessage {
    offset: i64,
    timestamp_us: i64,
    #[serde(with = "base64_bytes")]
    data: Vec<u8>,
}

/// Base64 encoding for binary data in JSONL
mod base64_bytes {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        STANDARD.decode(s).map_err(serde::de::Error::custom)
    }
}

/// Consumer group offsets stored in JSON
#[derive(Serialize, Deserialize, Default)]
struct OffsetStore {
    offsets: HashMap<String, i64>, // topic -> offset
}

/// File-based streaming backend
///
/// Stores messages in JSONL files, one file per topic.
/// Offset tracking is stored in a separate `.offsets` directory.
pub struct FileBackend {
    base_path: PathBuf,
    closed: AtomicBool,
    topic_locks: Arc<RwLock<HashMap<String, Arc<RwLock<()>>>>>,
    /// Shared per-topic next offsets to avoid collisions across producers.
    topic_offsets: Arc<RwLock<HashMap<String, Arc<AtomicI64>>>>,
}

impl FileBackend {
    /// Create a new file backend with the given base directory
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            base_path,
            closed: AtomicBool::new(false),
            topic_locks: Arc::new(RwLock::new(HashMap::new())),
            topic_offsets: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the file path for a topic
    fn topic_path(&self, topic: &str) -> PathBuf {
        self.base_path.join(format!("{}.jsonl", topic))
    }

    /// Get the offset file path for a consumer group
    fn offset_path(&self, group_id: &str) -> PathBuf {
        self.base_path
            .join(".offsets")
            .join(format!("{}.json", group_id))
    }

    /// Get or create a lock for a topic
    async fn get_topic_lock(&self, topic: &str) -> Arc<RwLock<()>> {
        let mut locks = self.topic_locks.write().await;
        Arc::clone(
            locks
                .entry(topic.to_string())
                .or_insert_with(|| Arc::new(RwLock::new(()))),
        )
    }

    /// Ensure base directory exists
    async fn ensure_dirs(&self) -> BackendResult<()> {
        tokio::fs::create_dir_all(&self.base_path).await?;
        tokio::fs::create_dir_all(self.base_path.join(".offsets")).await?;
        Ok(())
    }

    /// Load offsets for a consumer group
    ///
    /// Uses direct file read with error handling instead of exists() check
    /// to avoid TOCTOU race conditions.
    async fn load_offsets(&self, group_id: &str) -> BackendResult<OffsetStore> {
        let path = self.offset_path(group_id);
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => serde_json::from_str(&content)
                .map_err(|e| BackendError::Deserialization(e.to_string())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(OffsetStore::default()),
            Err(e) => Err(BackendError::Io(e)),
        }
    }

    // NOTE: Consumer group offset persistence is implemented in FileConsumer::commit()
    // which uses atomic file operations (write to temp + rename) for crash safety.
    // See test_offset_persistence for verification.

    /// Count lines in a topic file (current message count)
    ///
    /// Uses direct file open with error handling instead of exists() check
    /// to avoid TOCTOU race conditions.
    async fn count_messages(&self, topic: &str) -> BackendResult<i64> {
        let path = self.topic_path(topic);
        let file = match File::open(&path).await {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(BackendError::Io(e)),
        };

        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut count = 0i64;
        while lines.next_line().await?.is_some() {
            count += 1;
        }
        Ok(count)
    }
}

#[async_trait]
impl StreamBackend for FileBackend {
    type Producer = FileProducer;
    type Consumer = FileConsumer;

    async fn create_producer(&self, topic: &str) -> BackendResult<Self::Producer> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(BackendError::Closed);
        }

        self.ensure_dirs().await?;
        let topic_lock = self.get_topic_lock(topic).await;
        // Initialize or fetch a shared next_offset for this topic to avoid
        // collisions when multiple producers are created.
        // Note: avoid holding a read guard across an `.await` that takes a write lock,
        // which can deadlock with tokio::sync::RwLock.
        let existing = {
            let offsets = self.topic_offsets.read().await;
            offsets.get(topic).cloned()
        };

        let next_offset = if let Some(existing) = existing {
            existing
        } else {
            // Guard the topic while counting to prevent races with concurrent writes.
            let _guard = topic_lock.write().await;
            let count = self.count_messages(topic).await?;
            let mut offsets = self.topic_offsets.write().await;
            Arc::clone(
                offsets
                    .entry(topic.to_string())
                    .or_insert_with(|| Arc::new(AtomicI64::new(count))),
            )
        };

        Ok(FileProducer {
            topic: topic.to_string(),
            path: self.topic_path(topic),
            topic_lock,
            next_offset,
        })
    }

    async fn create_consumer(&self, topic: &str, group_id: &str) -> BackendResult<Self::Consumer> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(BackendError::Closed);
        }

        self.ensure_dirs().await?;

        let offsets = self.load_offsets(group_id).await?;
        let start_offset = offsets.offsets.get(topic).copied().unwrap_or(0);

        Ok(FileConsumer {
            topic: topic.to_string(),
            group_id: group_id.to_string(),
            path: self.topic_path(topic),
            offset_path: self.offset_path(group_id),
            current_offset: start_offset,
            cached_offsets: offsets,
            reader: None,
        })
    }

    async fn health_check(&self) -> BackendResult<()> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(BackendError::Closed);
        }
        // Check if base directory is accessible
        self.ensure_dirs().await?;
        Ok(())
    }

    async fn close(&self) -> BackendResult<()> {
        self.closed.store(true, Ordering::Relaxed);
        Ok(())
    }
}

/// File-based producer
pub struct FileProducer {
    topic: String,
    path: PathBuf,
    topic_lock: Arc<RwLock<()>>,
    next_offset: Arc<AtomicI64>,
}

#[async_trait]
impl StreamProducer for FileProducer {
    async fn send(&self, message: DashStreamMessage) -> BackendResult<()> {
        let data =
            encode_message(&message).map_err(|e| BackendError::Serialization(e.to_string()))?;

        let offset = self.next_offset.fetch_add(1, Ordering::SeqCst);
        let timestamp_us = chrono::Utc::now().timestamp_micros();

        let jsonl_msg = JsonlMessage {
            offset,
            timestamp_us,
            data,
        };

        let line = serde_json::to_string(&jsonl_msg)
            .map_err(|e| BackendError::Serialization(e.to_string()))?;

        // Acquire lock for appending
        let _guard = self.topic_lock.write().await;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;

        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.flush().await?;

        Ok(())
    }

    async fn flush(&self) -> BackendResult<()> {
        // Files are flushed after each write
        Ok(())
    }

    fn topic(&self) -> &str {
        &self.topic
    }
}

/// File-based consumer
pub struct FileConsumer {
    topic: String,
    group_id: String,
    path: PathBuf,
    offset_path: PathBuf,
    current_offset: i64,
    cached_offsets: OffsetStore,
    reader: Option<BufReader<File>>,
}

impl FileConsumer {
    /// Lazily open the topic file and seek to current_offset.
    ///
    /// Uses direct file open with error handling instead of exists() check
    /// to avoid TOCTOU race conditions.
    async fn ensure_reader(&mut self) -> BackendResult<()> {
        if self.reader.is_some() {
            return Ok(());
        }

        let file = match File::open(&self.path).await {
            Ok(f) => f,
            // File doesn't exist yet - leave reader as None, caller will retry
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(BackendError::Io(e)),
        };
        let mut reader = BufReader::new(file);

        // Skip already-consumed lines once (avoid O(n^2) rescans).
        let mut skipped = 0i64;
        while skipped < self.current_offset {
            let mut line = String::new();
            let bytes = reader.read_line(&mut line).await?;
            if bytes == 0 {
                break;
            }
            skipped += 1;
        }

        // If file is shorter than committed offset, reset to end.
        if skipped < self.current_offset {
            self.current_offset = skipped;
        }

        self.reader = Some(reader);
        Ok(())
    }
}

#[async_trait]
impl StreamConsumer for FileConsumer {
    async fn next(&mut self) -> Option<BackendResult<DashStreamMessage>> {
        loop {
            if let Err(e) = self.ensure_reader().await {
                return Some(Err(e));
            }

            let Some(reader) = self.reader.as_mut() else {
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            };
            let mut line = String::new();
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    // EOF: wait for new data
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
                Ok(_) => {
                    let jsonl_msg: JsonlMessage = match serde_json::from_str(line.trim_end()) {
                        Ok(m) => m,
                        Err(e) => return Some(Err(BackendError::Deserialization(e.to_string()))),
                    };

                    let message = match decode_message_compatible(
                        &jsonl_msg.data,
                        DEFAULT_MAX_PAYLOAD_SIZE,
                    ) {
                        Ok(m) => m,
                        Err(e) => return Some(Err(BackendError::Deserialization(e.to_string()))),
                    };

                    self.current_offset += 1;
                    return Some(Ok(message));
                }
                Err(e) => return Some(Err(BackendError::Io(e))),
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
        self.cached_offsets
            .offsets
            .insert(self.topic.clone(), self.current_offset);

        // Write offsets atomically to avoid corrupting the offset store on crash/kill mid-write.
        let content = serde_json::to_vec_pretty(&self.cached_offsets)
            .map_err(|e| BackendError::Serialization(e.to_string()))?;

        if let Some(parent) = self.offset_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let tmp_path = self
            .offset_path
            .with_extension(format!("json.tmp.{}", uuid::Uuid::new_v4()));

        let mut tmp = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp_path)
            .await?;
        tmp.write_all(&content).await?;
        tmp.flush().await?;
        tmp.sync_all().await?;
        drop(tmp);

        // Best effort cross-platform replace:
        // - Unix rename replaces atomically
        // - Windows rename may fail if destination exists; remove then retry.
        match tokio::fs::rename(&tmp_path, &self.offset_path).await {
            Ok(()) => {}
            Err(_) => {
                // SAFETY: Remove is best-effort for Windows compatibility - if it fails,
                // the subsequent rename will also fail and we'll propagate that error.
                let _ = tokio::fs::remove_file(&self.offset_path).await;
                tokio::fs::rename(&tmp_path, &self.offset_path).await?;
            }
        }

        Ok(())
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

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{Event, EventType, Header, MessageType};
    use tempfile::TempDir;

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
    async fn test_backend_new() {
        let temp_dir = TempDir::new().unwrap();
        let backend = FileBackend::new(temp_dir.path().to_path_buf());
        assert!(backend.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_create_producer() {
        let temp_dir = TempDir::new().unwrap();
        let backend = FileBackend::new(temp_dir.path().to_path_buf());

        let producer = backend.create_producer("test-topic").await.unwrap();
        assert_eq!(producer.topic(), "test-topic");
    }

    #[tokio::test]
    async fn test_create_consumer() {
        let temp_dir = TempDir::new().unwrap();
        let backend = FileBackend::new(temp_dir.path().to_path_buf());

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
        let temp_dir = TempDir::new().unwrap();
        let backend = FileBackend::new(temp_dir.path().to_path_buf());

        let producer = backend.create_producer("events").await.unwrap();
        let mut consumer = backend
            .create_consumer("events", "test-group")
            .await
            .unwrap();

        // Send event
        let event = create_test_event("thread-1", 1);
        producer.send_event(event.clone()).await.unwrap();

        // Receive event
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
    async fn test_multiple_messages() {
        let temp_dir = TempDir::new().unwrap();
        let backend = FileBackend::new(temp_dir.path().to_path_buf());

        let producer = backend.create_producer("events").await.unwrap();
        let mut consumer = backend
            .create_consumer("events", "test-group")
            .await
            .unwrap();

        // Send multiple events
        for i in 1..=5 {
            let event = create_test_event("thread-1", i);
            producer.send_event(event).await.unwrap();
        }

        // Receive all events
        for i in 1..=5 {
            let received = consumer
                .next_timeout(Duration::from_millis(100))
                .await
                .unwrap()
                .unwrap();

            match received.message {
                Some(crate::dash_stream_message::Message::Event(e)) => {
                    assert_eq!(e.node_id, format!("node-{}", i));
                }
                _ => panic!("Expected Event message"),
            }
        }

        assert_eq!(consumer.current_offset(), 5);
    }

    #[tokio::test]
    async fn test_offset_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let backend = FileBackend::new(temp_dir.path().to_path_buf());

        let producer = backend.create_producer("events").await.unwrap();

        // Send events
        for i in 1..=3 {
            producer
                .send_event(create_test_event("thread-1", i))
                .await
                .unwrap();
        }

        // First consumer reads and commits
        {
            let mut consumer = backend
                .create_consumer("events", "test-group")
                .await
                .unwrap();
            let _ = consumer.next_timeout(Duration::from_millis(100)).await;
            let _ = consumer.next_timeout(Duration::from_millis(100)).await;
            consumer.commit().await.unwrap();
        }

        // New consumer should start from committed offset
        let consumer = backend
            .create_consumer("events", "test-group")
            .await
            .unwrap();
        assert_eq!(consumer.current_offset(), 2);
    }

    #[tokio::test]
    async fn test_jsonl_file_format() {
        let temp_dir = TempDir::new().unwrap();
        let backend = FileBackend::new(temp_dir.path().to_path_buf());

        let producer = backend.create_producer("events").await.unwrap();
        producer
            .send_event(create_test_event("thread-1", 1))
            .await
            .unwrap();

        // Check file exists and is valid JSONL
        let path = temp_dir.path().join("events.jsonl");
        assert!(path.exists());

        let content = tokio::fs::read_to_string(&path).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1);

        // Parse as JSON
        let jsonl_msg: JsonlMessage = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(jsonl_msg.offset, 0);
        assert!(!jsonl_msg.data.is_empty());
    }

    #[tokio::test]
    async fn test_multiple_topics() {
        let temp_dir = TempDir::new().unwrap();
        let backend = FileBackend::new(temp_dir.path().to_path_buf());

        let producer1 = backend.create_producer("topic-1").await.unwrap();
        let producer2 = backend.create_producer("topic-2").await.unwrap();

        producer1
            .send_event(create_test_event("t1", 1))
            .await
            .unwrap();
        producer2
            .send_event(create_test_event("t2", 1))
            .await
            .unwrap();

        // Both files should exist
        assert!(temp_dir.path().join("topic-1.jsonl").exists());
        assert!(temp_dir.path().join("topic-2.jsonl").exists());
    }

    #[tokio::test]
    async fn test_health_check_closed() {
        let temp_dir = TempDir::new().unwrap();
        let backend = FileBackend::new(temp_dir.path().to_path_buf());

        assert!(backend.health_check().await.is_ok());

        backend.close().await.unwrap();

        assert!(matches!(
            backend.health_check().await,
            Err(BackendError::Closed)
        ));
    }

    #[tokio::test]
    async fn test_timeout_no_messages() {
        let temp_dir = TempDir::new().unwrap();
        let backend = FileBackend::new(temp_dir.path().to_path_buf());

        // Create topic file
        let _ = backend.create_producer("empty-topic").await.unwrap();
        let mut consumer = backend
            .create_consumer("empty-topic", "group")
            .await
            .unwrap();

        let result = consumer.next_timeout(Duration::from_millis(50)).await;
        assert!(result.is_none());
    }
}
