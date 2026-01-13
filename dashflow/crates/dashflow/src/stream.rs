// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Graph execution streaming
//!
//! Streaming allows consuming graph execution results in real-time
//! as nodes complete, rather than waiting for full execution to finish.

use crate::constants::DEFAULT_STREAM_CHANNEL_CAPACITY;
use crate::state::GraphState;
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;

/// Global counter for dropped stream messages (observability)
/// This counter tracks how many custom stream events were dropped due to channel full.
/// Use `stream_dropped_count()` to read the current value.
static STREAM_DROPPED_COUNT: AtomicU64 = AtomicU64::new(0);

/// Returns the total number of stream messages dropped due to channel full.
///
/// This metric is useful for:
/// - Detecting when stream channel capacity is insufficient
/// - Monitoring backpressure in high-volume custom event scenarios
/// - Alerting when producers outpace consumers
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::stream::stream_dropped_count;
///
/// // After some streaming...
/// let dropped = stream_dropped_count();
/// if dropped > 0 {
///     eprintln!("Warning: {} stream messages were dropped", dropped);
/// }
/// ```
#[must_use]
pub fn stream_dropped_count() -> u64 {
    STREAM_DROPPED_COUNT.load(Ordering::Relaxed)
}

/// Resets the dropped message counter to zero.
///
/// Useful for testing or when starting a new measurement period.
pub fn reset_stream_dropped_count() {
    STREAM_DROPPED_COUNT.store(0, Ordering::Relaxed);
}

// Note: DEFAULT_STREAM_CHANNEL_CAPACITY is imported from crate::constants
// and re-exported for backward compatibility (see lib.rs constants export)

thread_local! {
    /// Thread-local stream writer for custom data emission
    ///
    /// Using RefCell inside thread_local! is safe because each thread gets its own
    /// isolated RefCell instance. This pattern is idiomatic for single-threaded
    /// interior mutability in thread-local storage.
    static STREAM_WRITER: RefCell<Option<mpsc::Sender<serde_json::Value>>> = const { RefCell::new(None) };
}

/// RAII guard that automatically clears the thread-local stream writer on drop.
/// This prevents leaking the writer if a panic occurs during node execution.
///
/// # Safety
///
/// The guard MUST be created on the same thread where the writer will be used.
/// The writer is cleared when the guard is dropped, even during stack unwinding.
///
/// # Example
///
/// ```rust,ignore
/// let (tx, rx) = create_stream_channel();
/// {
///     let _guard = StreamWriterGuard::new(tx);
///     // Even if code here panics, the writer is cleared
///     run_node_execution().await;
/// }
/// // Writer is automatically cleared when guard goes out of scope
/// ```
// Planned for graph execution streaming (see stream_events in runnable.rs).
// Provides RAII guard for panic-safe stream writer cleanup in node execution.
#[allow(dead_code)] // Architectural: Future use in graph streaming - RAII pattern ready for wiring
pub(crate) struct StreamWriterGuard {
    _private: (),
}

#[allow(dead_code)] // Architectural: RAII impl for StreamWriterGuard - part of streaming infrastructure
impl StreamWriterGuard {
    /// Install a stream writer for the current thread and return a guard.
    /// When the guard is dropped (normally or due to panic), the writer is cleared.
    pub(crate) fn new(sender: mpsc::Sender<serde_json::Value>) -> Self {
        set_stream_writer(Some(sender));
        Self { _private: () }
    }
}

impl Drop for StreamWriterGuard {
    fn drop(&mut self) {
        set_stream_writer(None);
    }
}

/// Get the stream writer for emitting custom data from within a node
///
/// This function can be called from within a node to emit custom progress,
/// status, or metrics data. The data will be yielded as `StreamEvent::Custom`
/// when the graph is executed with `StreamMode::Custom`.
///
/// # Example
///
/// ```rust,ignore
/// graph.add_node_from_fn("my_node", |state| {
///     Box::pin(async move {
///         // Emit custom progress data
///         if let Some(writer) = get_stream_writer() {
///             writer.write(serde_json::json!({
///                 "type": "progress",
///                 "percent": 50
///             }));
///         }
///
///         // ... node logic ...
///
///         Ok(state)
///     })
/// });
/// ```
#[must_use]
pub fn get_stream_writer() -> Option<StreamWriter> {
    STREAM_WRITER.with(|w| {
        w.borrow().as_ref().map(|sender| StreamWriter {
            sender: sender.clone(),
        })
    })
}

/// Stream writer for emitting custom data from nodes
pub struct StreamWriter {
    sender: mpsc::Sender<serde_json::Value>,
}

impl StreamWriter {
    /// Write custom data to the stream
    ///
    /// The data will be serialized as JSON and yielded as a `StreamEvent::Custom`
    /// event when the graph is executed with `StreamMode::Custom`.
    ///
    /// Uses `try_send` for non-blocking operation. If the channel is full,
    /// the data is dropped with a warning log and the dropped message counter is incremented.
    /// Use [`stream_dropped_count()`] to monitor dropped messages.
    pub fn write(&self, data: serde_json::Value) {
        match self.sender.try_send(data) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                STREAM_DROPPED_COUNT.fetch_add(1, Ordering::Relaxed);
                tracing::warn!(
                    dropped_total = STREAM_DROPPED_COUNT.load(Ordering::Relaxed),
                    "Stream channel full, dropping custom data. Consider increasing capacity with with_stream_channel_capacity()"
                );
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                // Stream receiver dropped - this is expected during shutdown
            }
        }
    }
}

/// Internal function to set the stream writer for the current thread
pub(crate) fn set_stream_writer(sender: Option<mpsc::Sender<serde_json::Value>>) {
    STREAM_WRITER.with(|w| {
        *w.borrow_mut() = sender;
    });
}

/// Create a new bounded stream channel for custom data emission
///
/// Returns a (sender, receiver) pair. The sender should be passed to `set_stream_writer`,
/// and the receiver should be used to collect custom events.
///
/// Uses the default capacity of [`DEFAULT_STREAM_CHANNEL_CAPACITY`] (10,000 messages).
pub(crate) fn create_stream_channel() -> (
    mpsc::Sender<serde_json::Value>,
    mpsc::Receiver<serde_json::Value>,
) {
    mpsc::channel(DEFAULT_STREAM_CHANNEL_CAPACITY)
}

/// Create a new bounded stream channel with custom capacity
///
/// Use this when you need to adjust the buffer size for high-volume custom events
/// or to limit memory usage.
///
/// # Arguments
///
/// * `capacity` - The maximum number of messages that can be buffered
///
/// # Panics
///
/// Panics if `capacity` is 0.
// Used by executor when with_stream_channel_capacity() is set
pub(crate) fn create_stream_channel_with_capacity(
    capacity: usize,
) -> (
    mpsc::Sender<serde_json::Value>,
    mpsc::Receiver<serde_json::Value>,
) {
    mpsc::channel(capacity)
}

/// Stream modes control what data is yielded during execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StreamMode {
    /// Emit full state after each node completes
    #[default]
    Values,
    /// Emit only state updates from each node
    Updates,
    /// Emit node start/end events
    Events,
    /// Emit custom data from nodes via stream writer
    Custom,
}

/// Event yielded during streaming execution
#[derive(Debug, Clone)]
pub enum StreamEvent<S>
where
    S: GraphState,
{
    /// Full state after a node completed (`StreamMode::Values`)
    Values {
        /// Node that just completed
        node: String,
        /// Current state after node execution
        state: S,
    },
    /// State update from a node (`StreamMode::Updates`)
    Update {
        /// Node that produced the update
        node: String,
        /// State update (same as Values for now, could be delta in future)
        state: S,
    },
    /// Node started execution (`StreamMode::Events`)
    NodeStart {
        /// Node name
        node: String,
    },
    /// Node completed execution (`StreamMode::Events`)
    NodeEnd {
        /// Node name
        node: String,
        /// State after node
        state: S,
    },
    /// Custom data emitted from a node (`StreamMode::Custom`)
    Custom {
        /// Node that emitted the data
        node: String,
        /// Custom data as JSON value
        data: serde_json::Value,
    },
    /// Graph execution completed
    Done {
        /// Final state
        state: S,
        /// Execution path
        execution_path: Vec<String>,
    },
}

impl<S> StreamEvent<S>
where
    S: GraphState,
{
    /// Get the state from this event if available
    pub fn state(&self) -> Option<&S> {
        match self {
            StreamEvent::Values { state, .. } => Some(state),
            StreamEvent::Update { state, .. } => Some(state),
            StreamEvent::NodeEnd { state, .. } => Some(state),
            StreamEvent::Done { state, .. } => Some(state),
            StreamEvent::NodeStart { .. } => None,
            StreamEvent::Custom { .. } => None,
        }
    }

    /// Get the node name if this event is node-related
    pub fn node(&self) -> Option<&str> {
        match self {
            StreamEvent::Values { node, .. } => Some(node),
            StreamEvent::Update { node, .. } => Some(node),
            StreamEvent::NodeStart { node } => Some(node),
            StreamEvent::NodeEnd { node, .. } => Some(node),
            StreamEvent::Custom { node, .. } => Some(node),
            StreamEvent::Done { .. } => None,
        }
    }

    /// Check if this is the final event
    pub fn is_done(&self) -> bool {
        matches!(self, StreamEvent::Done { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AgentState;

    #[test]
    fn test_stream_event_state() {
        let state = AgentState::new();

        let event = StreamEvent::Values {
            node: "test".to_string(),
            state: state.clone(),
        };

        assert!(event.state().is_some());
        assert_eq!(event.node(), Some("test"));
        assert!(!event.is_done());
    }

    #[test]
    fn test_stream_event_done() {
        let state = AgentState::new();

        let event = StreamEvent::Done {
            state,
            execution_path: vec!["node1".to_string(), "node2".to_string()],
        };

        assert!(event.state().is_some());
        assert!(event.node().is_none());
        assert!(event.is_done());
    }

    #[test]
    fn test_stream_mode_default() {
        assert_eq!(StreamMode::default(), StreamMode::Values);
    }

    #[test]
    fn test_stream_event_update() {
        let state = AgentState::new();

        let event = StreamEvent::Update {
            node: "update_node".to_string(),
            state: state.clone(),
        };

        assert!(event.state().is_some());
        assert_eq!(event.node(), Some("update_node"));
        assert!(!event.is_done());
    }

    #[test]
    fn test_stream_event_node_start() {
        let event = StreamEvent::<AgentState>::NodeStart {
            node: "start_node".to_string(),
        };

        assert!(event.state().is_none());
        assert_eq!(event.node(), Some("start_node"));
        assert!(!event.is_done());
    }

    #[test]
    fn test_stream_event_node_end() {
        let state = AgentState::new();

        let event = StreamEvent::NodeEnd {
            node: "end_node".to_string(),
            state: state.clone(),
        };

        assert!(event.state().is_some());
        assert_eq!(event.node(), Some("end_node"));
        assert!(!event.is_done());
    }

    #[test]
    fn test_stream_mode_equality() {
        assert_eq!(StreamMode::Values, StreamMode::Values);
        assert_eq!(StreamMode::Updates, StreamMode::Updates);
        assert_eq!(StreamMode::Events, StreamMode::Events);
        assert_ne!(StreamMode::Values, StreamMode::Updates);
    }

    #[test]
    fn test_stream_mode_clone() {
        let mode1 = StreamMode::Values;
        let mode2 = mode1;
        assert_eq!(mode1, mode2);
    }

    #[test]
    fn test_stream_event_clone() {
        let state = AgentState::new();
        let event1 = StreamEvent::Values {
            node: "test".to_string(),
            state: state.clone(),
        };

        let event2 = event1.clone();
        assert_eq!(event1.node(), event2.node());
        assert!(event1.state().is_some());
        assert!(event2.state().is_some());
    }

    #[test]
    fn test_stream_event_done_execution_path() {
        let state = AgentState::new();
        let path = vec![
            "start".to_string(),
            "process".to_string(),
            "validate".to_string(),
            "end".to_string(),
        ];

        let event = StreamEvent::Done {
            state,
            execution_path: path.clone(),
        };

        if let StreamEvent::Done { execution_path, .. } = event {
            assert_eq!(execution_path.len(), 4);
            assert_eq!(execution_path[0], "start");
            assert_eq!(execution_path[3], "end");
        } else {
            panic!("Expected Done event");
        }
    }

    #[test]
    fn test_stream_event_done_empty_path() {
        let state = AgentState::new();

        let event = StreamEvent::Done {
            state,
            execution_path: vec![],
        };

        if let StreamEvent::Done { execution_path, .. } = event {
            assert_eq!(execution_path.len(), 0);
        } else {
            panic!("Expected Done event");
        }
    }

    #[test]
    fn test_stream_event_empty_node_names() {
        let state = AgentState::new();

        let event = StreamEvent::Values {
            node: "".to_string(),
            state: state.clone(),
        };
        assert_eq!(event.node(), Some(""));

        let event = StreamEvent::Update {
            node: "".to_string(),
            state: state.clone(),
        };
        assert_eq!(event.node(), Some(""));

        let event = StreamEvent::<AgentState>::NodeStart {
            node: "".to_string(),
        };
        assert_eq!(event.node(), Some(""));

        let event = StreamEvent::NodeEnd {
            node: "".to_string(),
            state,
        };
        assert_eq!(event.node(), Some(""));
    }

    #[test]
    fn test_stream_event_special_characters() {
        let state = AgentState::new();
        let special_names = vec![
            "node-with-hyphens",
            "node_with_underscores",
            "node.with.dots",
            "node:with:colons",
            "node/with/slashes",
            "node@with@ats",
        ];

        for name in special_names {
            let event = StreamEvent::Values {
                node: name.to_string(),
                state: state.clone(),
            };
            assert_eq!(event.node(), Some(name));
        }
    }

    #[test]
    fn test_stream_event_unicode_names() {
        let _state = AgentState::new();
        let unicode_names = vec![
            "节点处理器",     // Chinese
            "узел_обработки", // Russian
            "ノード処理",     // Japanese
        ];

        for name in unicode_names {
            let event = StreamEvent::<AgentState>::NodeStart {
                node: name.to_string(),
            };
            assert_eq!(event.node(), Some(name));
        }
    }

    #[test]
    fn test_stream_event_long_node_names() {
        let state = AgentState::new();
        let long_name = "a".repeat(1000);

        let event = StreamEvent::Values {
            node: long_name.clone(),
            state: state.clone(),
        };

        assert_eq!(event.node(), Some(long_name.as_str()));
        assert_eq!(event.node().unwrap().len(), 1000);
    }

    #[test]
    fn test_stream_mode_debug_format() {
        let mode = StreamMode::Values;
        let debug_str = format!("{:?}", mode);
        assert!(debug_str.contains("Values"));

        let mode = StreamMode::Updates;
        let debug_str = format!("{:?}", mode);
        assert!(debug_str.contains("Updates"));

        let mode = StreamMode::Events;
        let debug_str = format!("{:?}", mode);
        assert!(debug_str.contains("Events"));
    }

    #[test]
    fn test_stream_event_debug_format() {
        let state = AgentState::new();

        let event = StreamEvent::Values {
            node: "test".to_string(),
            state: state.clone(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("Values"));
        assert!(debug_str.contains("test"));

        let event = StreamEvent::<AgentState>::NodeStart {
            node: "start".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("NodeStart"));
        assert!(debug_str.contains("start"));
    }

    #[test]
    fn test_stream_event_pattern_matching() {
        let state = AgentState::new();

        let events = vec![
            StreamEvent::Values {
                node: "node1".to_string(),
                state: state.clone(),
            },
            StreamEvent::Update {
                node: "node2".to_string(),
                state: state.clone(),
            },
            StreamEvent::NodeStart {
                node: "node3".to_string(),
            },
            StreamEvent::NodeEnd {
                node: "node4".to_string(),
                state: state.clone(),
            },
            StreamEvent::Done {
                state: state.clone(),
                execution_path: vec!["node1".to_string()],
            },
        ];

        let mut values_count = 0;
        let mut update_count = 0;
        let mut start_count = 0;
        let mut end_count = 0;
        let mut done_count = 0;

        for event in events {
            match event {
                StreamEvent::Values { .. } => values_count += 1,
                StreamEvent::Update { .. } => update_count += 1,
                StreamEvent::NodeStart { .. } => start_count += 1,
                StreamEvent::NodeEnd { .. } => end_count += 1,
                StreamEvent::Custom { .. } => {} // Not expected in this test
                StreamEvent::Done { .. } => done_count += 1,
            }
        }

        assert_eq!(values_count, 1);
        assert_eq!(update_count, 1);
        assert_eq!(start_count, 1);
        assert_eq!(end_count, 1);
        assert_eq!(done_count, 1);
    }

    #[test]
    fn test_stream_event_state_extraction() {
        let state = AgentState::new();

        // Events with state
        let with_state = vec![
            StreamEvent::Values {
                node: "node1".to_string(),
                state: state.clone(),
            },
            StreamEvent::Update {
                node: "node2".to_string(),
                state: state.clone(),
            },
            StreamEvent::NodeEnd {
                node: "node3".to_string(),
                state: state.clone(),
            },
            StreamEvent::Done {
                state: state.clone(),
                execution_path: vec![],
            },
        ];

        for event in with_state {
            assert!(event.state().is_some(), "Expected state for {:?}", event);
        }

        // Events without state
        let without_state = vec![StreamEvent::<AgentState>::NodeStart {
            node: "node".to_string(),
        }];

        for event in without_state {
            assert!(event.state().is_none(), "Expected no state for {:?}", event);
        }
    }

    #[test]
    fn test_stream_event_multiple_sequential_events() {
        let state = AgentState::new();

        let events = [
            StreamEvent::NodeStart {
                node: "node1".to_string(),
            },
            StreamEvent::NodeEnd {
                node: "node1".to_string(),
                state: state.clone(),
            },
            StreamEvent::NodeStart {
                node: "node2".to_string(),
            },
            StreamEvent::NodeEnd {
                node: "node2".to_string(),
                state: state.clone(),
            },
            StreamEvent::Done {
                state: state.clone(),
                execution_path: vec!["node1".to_string(), "node2".to_string()],
            },
        ];

        let node_names: Vec<_> = events
            .iter()
            .filter_map(|e| e.node().map(|n| n.to_string()))
            .collect();

        assert_eq!(node_names.len(), 4);
        assert_eq!(node_names[0], "node1");
        assert_eq!(node_names[1], "node1");
        assert_eq!(node_names[2], "node2");
        assert_eq!(node_names[3], "node2");

        let done_event = events.last().unwrap();
        assert!(done_event.is_done());
    }

    #[test]
    fn test_stream_event_done_long_execution_path() {
        let state = AgentState::new();
        let long_path: Vec<String> = (0..100).map(|i| format!("node_{}", i)).collect();

        let event = StreamEvent::Done {
            state,
            execution_path: long_path.clone(),
        };

        if let StreamEvent::Done { execution_path, .. } = event {
            assert_eq!(execution_path.len(), 100);
            assert_eq!(execution_path[0], "node_0");
            assert_eq!(execution_path[99], "node_99");
        } else {
            panic!("Expected Done event");
        }
    }

    #[test]
    fn test_stream_mode_copy_trait() {
        let mode1 = StreamMode::Values;
        let mode2 = mode1; // Copy
        let mode3 = mode1; // Copy again
        assert_eq!(mode1, mode2);
        assert_eq!(mode1, mode3);
        assert_eq!(mode2, mode3);
    }

    #[test]
    fn test_stream_event_is_done_all_variants() {
        let state = AgentState::new();

        let not_done = vec![
            StreamEvent::Values {
                node: "node".to_string(),
                state: state.clone(),
            },
            StreamEvent::Update {
                node: "node".to_string(),
                state: state.clone(),
            },
            StreamEvent::NodeStart {
                node: "node".to_string(),
            },
            StreamEvent::NodeEnd {
                node: "node".to_string(),
                state: state.clone(),
            },
        ];

        for event in not_done {
            assert!(!event.is_done(), "Event should not be done: {:?}", event);
        }

        let done = StreamEvent::Done {
            state,
            execution_path: vec![],
        };
        assert!(done.is_done(), "Done event should be done");
    }

    #[test]
    fn test_stream_event_node_none_only_for_done() {
        let state = AgentState::new();

        let with_node = vec![
            StreamEvent::Values {
                node: "node".to_string(),
                state: state.clone(),
            },
            StreamEvent::Update {
                node: "node".to_string(),
                state: state.clone(),
            },
            StreamEvent::NodeStart {
                node: "node".to_string(),
            },
            StreamEvent::NodeEnd {
                node: "node".to_string(),
                state: state.clone(),
            },
        ];

        for event in with_node {
            assert!(event.node().is_some(), "Expected node for {:?}", event);
        }

        let done = StreamEvent::Done {
            state,
            execution_path: vec![],
        };
        assert!(done.node().is_none(), "Done event should have no node");
    }

    #[test]
    fn test_stream_writer_basic() {
        // Create bounded channel
        let (tx, mut rx) = create_stream_channel();

        // Set writer
        set_stream_writer(Some(tx));

        // Get writer and write data
        if let Some(writer) = get_stream_writer() {
            writer.write(serde_json::json!({"type": "progress", "percent": 50}));
            writer.write(serde_json::json!({"type": "status", "message": "Processing"}));
        }

        // Clear writer
        set_stream_writer(None);

        // Verify data was received
        assert_eq!(
            rx.try_recv().unwrap(),
            serde_json::json!({"type": "progress", "percent": 50})
        );
        assert_eq!(
            rx.try_recv().unwrap(),
            serde_json::json!({"type": "status", "message": "Processing"})
        );
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_stream_writer_none_when_not_set() {
        set_stream_writer(None);
        assert!(get_stream_writer().is_none());
    }

    #[test]
    fn test_custom_stream_event() {
        let event = StreamEvent::<AgentState>::Custom {
            node: "my_node".to_string(),
            data: serde_json::json!({"progress": 75}),
        };

        assert_eq!(event.node(), Some("my_node"));
        assert!(event.state().is_none());
        assert!(!event.is_done());
    }

    #[test]
    fn test_stream_channel_with_custom_capacity() {
        // Test that custom capacity works
        let (tx, mut rx) = create_stream_channel_with_capacity(5);

        // Send up to capacity
        for i in 0..5 {
            tx.try_send(serde_json::json!({"index": i})).unwrap();
        }

        // Channel should be full now
        assert!(tx.try_send(serde_json::json!({"overflow": true})).is_err());

        // Verify data
        for i in 0..5 {
            assert_eq!(rx.try_recv().unwrap(), serde_json::json!({"index": i}));
        }
    }

    #[test]
    fn test_stream_dropped_counter() {
        // Reset the counter first
        reset_stream_dropped_count();
        assert_eq!(stream_dropped_count(), 0);

        // Create a tiny channel that will overflow
        let (tx, _rx) = create_stream_channel_with_capacity(1);
        set_stream_writer(Some(tx));

        if let Some(writer) = get_stream_writer() {
            // First write succeeds
            writer.write(serde_json::json!({"first": true}));
            // Second write should be dropped (channel full)
            writer.write(serde_json::json!({"second": true}));
            // Third write should also be dropped
            writer.write(serde_json::json!({"third": true}));
        }

        set_stream_writer(None);

        // Should have 2 dropped messages
        assert_eq!(stream_dropped_count(), 2);

        // Reset and verify
        reset_stream_dropped_count();
        assert_eq!(stream_dropped_count(), 0);
    }

    #[test]
    fn test_default_stream_channel_capacity_constant() {
        // Verify the constant is accessible and has expected value
        assert_eq!(DEFAULT_STREAM_CHANNEL_CAPACITY, 10000);
    }
}
