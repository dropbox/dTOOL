//! Bounded event queue for plugin processing.
//!
//! This module provides a thread-safe, bounded event queue that allows the host
//! to enqueue events for plugins to process. Overflow handling drops the oldest
//! non-critical events first.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use parking_lot::Mutex;

use super::types::{PluginEvent, PluginId};

/// Priority levels for events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventPriority {
    /// Low priority - can be dropped on overflow (e.g., ticks).
    Low = 0,
    /// Normal priority - dropped after low priority (e.g., output).
    Normal = 1,
    /// High priority - rarely dropped (e.g., key events).
    High = 2,
    /// Critical priority - never dropped (e.g., command complete).
    Critical = 3,
}

impl EventPriority {
    /// Determine priority for an event type.
    pub fn for_event(event: &PluginEvent) -> Self {
        match event {
            PluginEvent::Tick { .. } => Self::Low,
            PluginEvent::Output { .. } => Self::Normal,
            PluginEvent::Key(_) => Self::High,
            PluginEvent::CommandStart { .. } => Self::High,
            PluginEvent::CommandComplete { .. } => Self::Critical,
        }
    }
}

/// A queued event with metadata.
#[derive(Debug)]
pub struct QueuedEvent {
    /// The event payload.
    pub event: PluginEvent,
    /// Event priority.
    pub priority: EventPriority,
    /// When the event was enqueued.
    pub enqueued_at: Instant,
    /// Sequence number for ordering.
    pub sequence: u64,
}

impl QueuedEvent {
    /// Create a new queued event.
    pub fn new(event: PluginEvent, sequence: u64) -> Self {
        Self {
            priority: EventPriority::for_event(&event),
            event,
            enqueued_at: Instant::now(),
            sequence,
        }
    }
}

/// Statistics for the event queue.
#[derive(Debug, Clone, Default)]
pub struct QueueStats {
    /// Total events enqueued.
    pub total_enqueued: u64,
    /// Total events dequeued.
    pub total_dequeued: u64,
    /// Events dropped due to overflow.
    pub dropped_overflow: u64,
    /// Events dropped due to age.
    pub dropped_aged: u64,
    /// Current queue length.
    pub current_length: usize,
    /// Peak queue length observed.
    pub peak_length: usize,
}

/// Configuration for the event queue.
#[derive(Debug, Clone)]
pub struct QueueConfig {
    /// Maximum queue capacity.
    pub max_capacity: usize,
    /// Target capacity to free up during overflow cleanup.
    pub target_capacity: usize,
    /// Maximum age for events before they're considered stale (milliseconds).
    pub max_age_ms: u64,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_capacity: 1000,
            target_capacity: 800,
            max_age_ms: 5000, // 5 seconds
        }
    }
}

/// Result of an enqueue operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnqueueResult {
    /// Event was successfully enqueued.
    Enqueued,
    /// Event was enqueued, but some events were dropped to make room.
    EnqueuedWithDrops(usize),
}

/// A bounded, priority-aware event queue for a single plugin.
pub struct PluginEventQueue {
    /// Plugin this queue belongs to.
    plugin_id: PluginId,
    /// The queue storage.
    queue: Mutex<VecDeque<QueuedEvent>>,
    /// Queue configuration.
    config: QueueConfig,
    /// Monotonic sequence counter.
    sequence: AtomicU64,
    /// Queue statistics.
    stats: Mutex<QueueStats>,
}

impl PluginEventQueue {
    /// Create a new event queue for a plugin.
    pub fn new(plugin_id: PluginId) -> Self {
        Self::with_config(plugin_id, QueueConfig::default())
    }

    /// Create a new event queue with custom configuration.
    pub fn with_config(plugin_id: PluginId, config: QueueConfig) -> Self {
        Self {
            plugin_id,
            queue: Mutex::new(VecDeque::with_capacity(config.max_capacity)),
            config,
            sequence: AtomicU64::new(0),
            stats: Mutex::new(QueueStats::default()),
        }
    }

    /// Get the plugin ID.
    pub fn plugin_id(&self) -> PluginId {
        self.plugin_id
    }

    /// Enqueue an event.
    pub fn enqueue(&self, event: PluginEvent) -> EnqueueResult {
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
        let queued = QueuedEvent::new(event, sequence);

        let mut queue = self.queue.lock();
        let mut stats = self.stats.lock();

        stats.total_enqueued += 1;

        // Check if we need to make room
        let dropped = if queue.len() >= self.config.max_capacity {
            self.cleanup_locked(&mut queue, &mut stats)
        } else {
            0
        };

        queue.push_back(queued);

        // Update stats
        stats.current_length = queue.len();
        if stats.current_length > stats.peak_length {
            stats.peak_length = stats.current_length;
        }

        if dropped > 0 {
            EnqueueResult::EnqueuedWithDrops(dropped)
        } else {
            EnqueueResult::Enqueued
        }
    }

    /// Dequeue the next event.
    pub fn dequeue(&self) -> Option<QueuedEvent> {
        let mut queue = self.queue.lock();
        let event = queue.pop_front();

        if event.is_some() {
            let mut stats = self.stats.lock();
            stats.total_dequeued += 1;
            stats.current_length = queue.len();
        }

        event
    }

    /// Peek at the next event without removing it.
    pub fn peek(&self) -> Option<EventPriority> {
        self.queue.lock().front().map(|e| e.priority)
    }

    /// Get the current queue length.
    pub fn len(&self) -> usize {
        self.queue.lock().len()
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.lock().is_empty()
    }

    /// Clear all events from the queue.
    pub fn clear(&self) {
        let mut queue = self.queue.lock();
        let dropped = queue.len();
        queue.clear();

        let mut stats = self.stats.lock();
        stats.dropped_overflow += dropped as u64;
        stats.current_length = 0;
    }

    /// Get queue statistics.
    pub fn stats(&self) -> QueueStats {
        self.stats.lock().clone()
    }

    /// Clean up old/low-priority events to make room.
    /// Must be called with locks held.
    fn cleanup_locked(
        &self,
        queue: &mut VecDeque<QueuedEvent>,
        stats: &mut QueueStats,
    ) -> usize {
        let target_len = self.config.target_capacity;
        let now = Instant::now();
        let mut dropped = 0;

        // First pass: remove aged events (except critical)
        queue.retain(|e| {
            if e.priority == EventPriority::Critical {
                return true;
            }
            // Saturate age to u64::MAX for extremely long durations (theoretical only)
            #[allow(clippy::cast_possible_truncation)]
            let age_ms = now.duration_since(e.enqueued_at).as_millis().min(u128::from(u64::MAX)) as u64;
            if age_ms > self.config.max_age_ms {
                dropped += 1;
                stats.dropped_aged += 1;
                false
            } else {
                true
            }
        });

        // Second pass: if still over capacity, drop by priority
        if queue.len() > target_len {
            // Drop Low priority first
            let to_drop = queue.len() - target_len;
            let mut dropped_this_pass = 0;

            queue.retain(|e| {
                if dropped_this_pass >= to_drop {
                    return true;
                }
                if e.priority == EventPriority::Low {
                    dropped_this_pass += 1;
                    false
                } else {
                    true
                }
            });
            dropped += dropped_this_pass;
            stats.dropped_overflow += dropped_this_pass as u64;
        }

        // Third pass: if still over, drop Normal priority
        if queue.len() > target_len {
            let to_drop = queue.len() - target_len;
            let mut dropped_this_pass = 0;

            queue.retain(|e| {
                if dropped_this_pass >= to_drop {
                    return true;
                }
                if e.priority == EventPriority::Normal {
                    dropped_this_pass += 1;
                    false
                } else {
                    true
                }
            });
            dropped += dropped_this_pass;
            stats.dropped_overflow += dropped_this_pass as u64;
        }

        dropped
    }
}

impl std::fmt::Debug for PluginEventQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginEventQueue")
            .field("plugin_id", &self.plugin_id)
            .field("len", &self.len())
            .field("config", &self.config)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::types::KeyCode;

    fn make_output_event() -> PluginEvent {
        PluginEvent::Output {
            data: vec![b'a'],
            in_command: false,
        }
    }

    fn make_tick_event() -> PluginEvent {
        PluginEvent::Tick { now_ms: 0 }
    }

    fn make_key_event() -> PluginEvent {
        use crate::plugins::types::{KeyEvent, KeyModifiers};
        PluginEvent::Key(KeyEvent {
            key: KeyCode::Char('a'),
            modifiers: KeyModifiers::empty(),
        })
    }

    fn make_command_complete() -> PluginEvent {
        use std::time::Duration;
        PluginEvent::CommandComplete {
            command: "ls".to_string(),
            exit_code: Some(0),
            duration: Duration::from_millis(100),
        }
    }

    #[test]
    fn test_queue_creation() {
        let queue = PluginEventQueue::new(PluginId(1));
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_enqueue_dequeue() {
        let queue = PluginEventQueue::new(PluginId(1));

        let result = queue.enqueue(make_output_event());
        assert_eq!(result, EnqueueResult::Enqueued);
        assert_eq!(queue.len(), 1);

        let event = queue.dequeue();
        assert!(event.is_some());
        assert!(queue.is_empty());
    }

    #[test]
    fn test_fifo_order() {
        let queue = PluginEventQueue::new(PluginId(1));

        queue.enqueue(PluginEvent::Tick { now_ms: 1 });
        queue.enqueue(PluginEvent::Tick { now_ms: 2 });
        queue.enqueue(PluginEvent::Tick { now_ms: 3 });

        let e1 = queue.dequeue().unwrap();
        let e2 = queue.dequeue().unwrap();
        let e3 = queue.dequeue().unwrap();

        assert_eq!(e1.sequence, 0);
        assert_eq!(e2.sequence, 1);
        assert_eq!(e3.sequence, 2);
    }

    #[test]
    fn test_event_priorities() {
        assert_eq!(EventPriority::for_event(&make_tick_event()), EventPriority::Low);
        assert_eq!(EventPriority::for_event(&make_output_event()), EventPriority::Normal);
        assert_eq!(EventPriority::for_event(&make_key_event()), EventPriority::High);
        assert_eq!(EventPriority::for_event(&make_command_complete()), EventPriority::Critical);
    }

    #[test]
    fn test_overflow_drops_low_priority() {
        let config = QueueConfig {
            max_capacity: 5,
            target_capacity: 3,
            max_age_ms: 60000,
        };
        let queue = PluginEventQueue::with_config(PluginId(1), config);

        // Fill to max capacity with ticks (low priority)
        for _ in 0..5 {
            queue.enqueue(make_tick_event());
        }
        assert_eq!(queue.len(), 5);

        // Add a critical event to trigger overflow cleanup
        let result = queue.enqueue(make_command_complete());
        assert!(matches!(result, EnqueueResult::EnqueuedWithDrops(_)));

        // Critical event should be preserved, low priority dropped
        let stats = queue.stats();
        assert!(stats.dropped_overflow > 0);
        // Queue should be at or below target capacity + 1 (for the new event)
        assert!(queue.len() <= 4);
    }

    #[test]
    fn test_stats_tracking() {
        let queue = PluginEventQueue::new(PluginId(1));

        queue.enqueue(make_output_event());
        queue.enqueue(make_output_event());
        queue.dequeue();

        let stats = queue.stats();
        assert_eq!(stats.total_enqueued, 2);
        assert_eq!(stats.total_dequeued, 1);
        assert_eq!(stats.current_length, 1);
    }

    #[test]
    fn test_clear() {
        let queue = PluginEventQueue::new(PluginId(1));

        for _ in 0..10 {
            queue.enqueue(make_output_event());
        }
        assert_eq!(queue.len(), 10);

        queue.clear();
        assert!(queue.is_empty());
    }

    #[test]
    fn test_peek() {
        let queue = PluginEventQueue::new(PluginId(1));

        assert!(queue.peek().is_none());

        queue.enqueue(make_key_event());
        assert_eq!(queue.peek(), Some(EventPriority::High));

        // Peek doesn't remove
        assert_eq!(queue.len(), 1);
    }
}
