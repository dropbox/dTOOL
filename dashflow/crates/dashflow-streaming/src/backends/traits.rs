// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
// DashFlow Streaming - Backend Traits

//! # Streaming Backend Traits
//!
//! Abstract traits for streaming backends, allowing interchangeable backends
//! for different deployment scenarios.

use crate::errors::Error;
use crate::{Checkpoint, DashStreamMessage, Event, Metrics, StateDiff, TokenChunk, ToolExecution};
use async_trait::async_trait;
use std::time::Duration;
use thiserror::Error;

/// Errors from streaming backends
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum BackendError {
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Deserialization error
    #[error("Deserialization error: {0}")]
    Deserialization(String),

    /// Topic not found
    #[error("Topic not found: {0}")]
    TopicNotFound(String),

    /// Consumer group error
    #[error("Consumer group error: {0}")]
    ConsumerGroup(String),

    /// Database error (for SQLite backend)
    #[error("Database error: {0}")]
    Database(String),

    /// Backend closed
    #[error("Backend closed")]
    Closed,

    /// Channel communication error (preserves context about which operation failed)
    #[error("Channel error: {0}")]
    ChannelError(String),

    /// Timeout error
    #[error("Operation timed out")]
    Timeout,

    /// General backend error
    #[error("Backend error: {0}")]
    Other(String),
}

impl From<BackendError> for Error {
    fn from(e: BackendError) -> Self {
        match e {
            BackendError::Io(io) => Error::Io(io),
            BackendError::Serialization(msg) => Error::InvalidFormat(msg),
            BackendError::Deserialization(msg) => Error::InvalidFormat(msg),
            _ => Error::InvalidFormat(e.to_string()),
        }
    }
}

/// Result type for backend operations
pub type BackendResult<T> = Result<T, BackendError>;

/// Abstract streaming backend trait
///
/// Backends implement this trait to provide producer and consumer creation.
/// This allows swapping between Kafka, in-memory, file-based, and SQLite backends.
#[async_trait]
pub trait StreamBackend: Send + Sync {
    /// Producer type for this backend
    type Producer: StreamProducer;

    /// Consumer type for this backend
    type Consumer: StreamConsumer;

    /// Create a producer for the given topic
    async fn create_producer(&self, topic: &str) -> BackendResult<Self::Producer>;

    /// Create a consumer for the given topic and consumer group
    async fn create_consumer(&self, topic: &str, group_id: &str) -> BackendResult<Self::Consumer>;

    /// Check if the backend is healthy/connected
    async fn health_check(&self) -> BackendResult<()>;

    /// Close the backend and release resources
    async fn close(&self) -> BackendResult<()>;
}

/// Abstract streaming producer trait
///
/// Producers send messages to a topic. All DashFlow Streaming message types
/// are supported through type-specific methods.
#[async_trait]
pub trait StreamProducer: Send + Sync {
    /// Send a raw DashStreamMessage
    async fn send(&self, message: DashStreamMessage) -> BackendResult<()>;

    /// Send an Event message
    async fn send_event(&self, event: Event) -> BackendResult<()> {
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Event(event)),
        };
        self.send(message).await
    }

    /// Send a StateDiff message
    async fn send_state_diff(&self, diff: StateDiff) -> BackendResult<()> {
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::StateDiff(diff)),
        };
        self.send(message).await
    }

    /// Send a TokenChunk message
    async fn send_token_chunk(&self, chunk: TokenChunk) -> BackendResult<()> {
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::TokenChunk(chunk)),
        };
        self.send(message).await
    }

    /// Send a ToolExecution message
    async fn send_tool_execution(&self, tool: ToolExecution) -> BackendResult<()> {
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::ToolExecution(tool)),
        };
        self.send(message).await
    }

    /// Send a Checkpoint message
    async fn send_checkpoint(&self, checkpoint: Checkpoint) -> BackendResult<()> {
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Checkpoint(checkpoint)),
        };
        self.send(message).await
    }

    /// Send a Metrics message
    async fn send_metrics(&self, metrics: Metrics) -> BackendResult<()> {
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Metrics(metrics)),
        };
        self.send(message).await
    }

    /// Send an Error message
    async fn send_error(&self, error: crate::Error) -> BackendResult<()> {
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Error(error)),
        };
        self.send(message).await
    }

    /// Flush pending messages
    async fn flush(&self) -> BackendResult<()>;

    /// Get the topic this producer is sending to
    fn topic(&self) -> &str;
}

/// Abstract streaming consumer trait
///
/// Consumers receive messages from a topic. Supports both polling and
/// timeout-based consumption.
#[async_trait]
pub trait StreamConsumer: Send + Sync {
    /// Receive the next message (blocking until available or end of stream)
    async fn next(&mut self) -> Option<BackendResult<DashStreamMessage>>;

    /// Receive the next message with a timeout
    async fn next_timeout(&mut self, timeout: Duration)
        -> Option<BackendResult<DashStreamMessage>>;

    /// Commit the current offset (for backends that support it)
    async fn commit(&mut self) -> BackendResult<()>;

    /// Get the topic this consumer is reading from
    fn topic(&self) -> &str;

    /// Get the consumer group ID
    fn group_id(&self) -> &str;

    /// Get the current offset
    fn current_offset(&self) -> i64;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_error_display() {
        let err = BackendError::TopicNotFound("test-topic".to_string());
        assert_eq!(err.to_string(), "Topic not found: test-topic");

        let err = BackendError::Timeout;
        assert_eq!(err.to_string(), "Operation timed out");
    }

    #[test]
    fn test_backend_error_conversion() {
        let backend_err = BackendError::Serialization("test error".to_string());
        let err: Error = backend_err.into();
        assert!(matches!(err, Error::InvalidFormat(_)));
    }
}
