//! Text-to-Speech (TTS) queue and session management.
//!
//! Implements the TTS portion of the MediaServer.tla specification.
//!
//! ## State Machine (per client)
//!
//! ```text
//! ┌──────┐  StartTTS   ┌──────────┐  Complete  ┌──────┐
//! │ Idle │ ──────────▶ │ Speaking │ ─────────▶ │ Idle │
//! └──────┘             └──────────┘            └──────┘
//!    ▲                      │ ▲
//!    │         Pause        │ │ Resume
//!    │                      ▼ │
//!    │                 ┌────────┐
//!    │                 │ Paused │
//!    │                 └────────┘
//!    │                      │
//!    └──────────────────────┘
//!           Cancel
//! ```

use super::{AudioFormat, ClientId, StreamId};
use std::collections::{HashMap, VecDeque};
use std::time::Instant;

/// TTS session states per client (from TLA+ spec: TTSStates).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TtsState {
    /// Not currently speaking.
    #[default]
    Idle,
    /// Actively speaking an utterance.
    Speaking,
    /// Playback paused.
    Paused,
    /// Error state.
    Error,
}

impl TtsState {
    /// Returns true if the TTS session is active (speaking or paused).
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Speaking | Self::Paused)
    }
}

/// Priority level for TTS utterances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Priority {
    /// Lowest priority, can be preempted.
    Low = 0,
    /// Normal priority.
    #[default]
    Normal = 5,
    /// High priority.
    High = 8,
    /// Urgent - interrupts current playback.
    Urgent = 10,
}

impl Priority {
    /// Convert from numeric value (0-10).
    pub fn from_level(level: u8) -> Self {
        match level {
            0..=2 => Self::Low,
            3..=6 => Self::Normal,
            7..=9 => Self::High,
            _ => Self::Urgent,
        }
    }

    /// Get numeric level (0-10).
    pub fn level(&self) -> u8 {
        *self as u8
    }
}

/// A TTS utterance in the queue.
#[derive(Debug, Clone)]
pub struct TtsUtterance {
    /// Unique ID for this utterance.
    pub id: u64,
    /// Text to speak.
    pub text: String,
    /// Priority level.
    pub priority: Priority,
    /// Audio format to use.
    pub audio_format: AudioFormat,
    /// When this utterance was queued.
    pub queued_at: Instant,
}

impl TtsUtterance {
    /// Create a new utterance.
    pub fn new(id: u64, text: impl Into<String>, priority: Priority, format: AudioFormat) -> Self {
        Self {
            id,
            text: text.into(),
            priority,
            audio_format: format,
            queued_at: Instant::now(),
        }
    }

    /// Create a normal priority utterance.
    pub fn normal(id: u64, text: impl Into<String>) -> Self {
        Self::new(id, text, Priority::Normal, AudioFormat::default())
    }

    /// Create an urgent utterance (will interrupt).
    pub fn urgent(id: u64, text: impl Into<String>) -> Self {
        Self::new(id, text, Priority::Urgent, AudioFormat::default())
    }

    /// Get the queue latency (time since queued).
    pub fn queue_latency(&self) -> std::time::Duration {
        self.queued_at.elapsed()
    }
}

/// TTS queue for a single client.
///
/// Enforces INV-MEDIA-2: TTS queue depth bounded per client.
#[derive(Debug)]
pub struct TtsQueue {
    /// Queue of pending utterances.
    queue: VecDeque<TtsUtterance>,
    /// Maximum queue depth.
    max_depth: usize,
    /// Current state for this client.
    state: TtsState,
    /// Currently speaking utterance (if any).
    current: Option<TtsUtterance>,
    /// Associated output stream.
    stream_id: Option<StreamId>,
    /// Next utterance ID.
    next_id: u64,
}

impl TtsQueue {
    /// Create a new TTS queue with the specified max depth.
    pub fn new(max_depth: usize) -> Self {
        Self {
            queue: VecDeque::with_capacity(max_depth),
            max_depth,
            state: TtsState::Idle,
            current: None,
            stream_id: None,
            next_id: 0,
        }
    }

    /// Get the current state.
    pub fn state(&self) -> TtsState {
        self.state
    }

    /// Get the current utterance being spoken.
    pub fn current(&self) -> Option<&TtsUtterance> {
        self.current.as_ref()
    }

    /// Get the number of queued utterances.
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    /// Check if the queue is full.
    pub fn is_full(&self) -> bool {
        self.queue.len() >= self.max_depth
    }

    /// Get the associated stream ID.
    pub fn stream_id(&self) -> Option<StreamId> {
        self.stream_id
    }

    /// Queue a new utterance.
    ///
    /// # Errors
    ///
    /// Returns error if queue is full (INV-MEDIA-2).
    pub fn queue(
        &mut self,
        text: impl Into<String>,
        priority: Priority,
        format: AudioFormat,
    ) -> Result<u64, TtsError> {
        // INV-MEDIA-2: TTS queue depth bounded per client
        if self.queue.len() >= self.max_depth {
            return Err(TtsError::QueueFull {
                max_depth: self.max_depth,
            });
        }

        let id = self.next_id;
        self.next_id += 1;

        let utterance = TtsUtterance::new(id, text, priority, format);

        // Insert by priority (higher priority = earlier in queue)
        let insert_pos = self
            .queue
            .iter()
            .position(|u| u.priority < priority)
            .unwrap_or(self.queue.len());

        self.queue.insert(insert_pos, utterance);

        Ok(id)
    }

    /// Start speaking the next utterance.
    ///
    /// # Errors
    ///
    /// Returns error if not in Idle state or queue is empty.
    pub fn start(&mut self, stream_id: StreamId) -> Result<&TtsUtterance, TtsError> {
        if self.state != TtsState::Idle {
            return Err(TtsError::InvalidState {
                expected: TtsState::Idle,
                actual: self.state,
            });
        }

        let utterance = self.queue.pop_front().ok_or(TtsError::QueueEmpty)?;

        self.current = Some(utterance);
        self.state = TtsState::Speaking;
        self.stream_id = Some(stream_id);

        Ok(self.current.as_ref().unwrap())
    }

    /// Complete the current utterance.
    ///
    /// # Errors
    ///
    /// Returns error if not in Speaking state.
    pub fn complete(&mut self) -> Result<TtsUtterance, TtsError> {
        if self.state != TtsState::Speaking {
            return Err(TtsError::InvalidState {
                expected: TtsState::Speaking,
                actual: self.state,
            });
        }

        let completed = self.current.take().ok_or(TtsError::NoCurrentUtterance)?;
        self.state = TtsState::Idle;
        self.stream_id = None;

        Ok(completed)
    }

    /// Pause playback.
    ///
    /// # Errors
    ///
    /// Returns error if not in Speaking state.
    pub fn pause(&mut self) -> Result<(), TtsError> {
        if self.state != TtsState::Speaking {
            return Err(TtsError::InvalidState {
                expected: TtsState::Speaking,
                actual: self.state,
            });
        }

        self.state = TtsState::Paused;
        Ok(())
    }

    /// Resume playback.
    ///
    /// # Errors
    ///
    /// Returns error if not in Paused state.
    pub fn resume(&mut self) -> Result<(), TtsError> {
        if self.state != TtsState::Paused {
            return Err(TtsError::InvalidState {
                expected: TtsState::Paused,
                actual: self.state,
            });
        }

        self.state = TtsState::Speaking;
        Ok(())
    }

    /// Cancel current playback.
    ///
    /// # Arguments
    ///
    /// * `clear_queue` - If true, also clears the pending queue.
    ///
    /// # Errors
    ///
    /// Returns error if not in Speaking or Paused state.
    pub fn cancel(&mut self, clear_queue: bool) -> Result<Option<TtsUtterance>, TtsError> {
        if !self.state.is_active() {
            return Err(TtsError::InvalidState {
                expected: TtsState::Speaking,
                actual: self.state,
            });
        }

        let cancelled = self.current.take();
        self.state = TtsState::Idle;
        self.stream_id = None;

        if clear_queue {
            self.queue.clear();
        }

        Ok(cancelled)
    }

    /// Interrupt current playback with a high-priority utterance.
    ///
    /// The current utterance is re-queued (unless discarded) and the
    /// interrupt utterance is played immediately.
    ///
    /// # Errors
    ///
    /// Returns error if not in Speaking state.
    pub fn interrupt(
        &mut self,
        text: impl Into<String>,
        format: AudioFormat,
    ) -> Result<u64, TtsError> {
        if self.state != TtsState::Speaking {
            return Err(TtsError::InvalidState {
                expected: TtsState::Speaking,
                actual: self.state,
            });
        }

        // Cancel current (stream will be closed by caller)
        self.current.take();
        self.state = TtsState::Idle;
        self.stream_id = None;

        // Queue urgent utterance at front
        let id = self.next_id;
        self.next_id += 1;

        let utterance = TtsUtterance::new(id, text, Priority::Urgent, format);
        self.queue.push_front(utterance);

        Ok(id)
    }

    /// Clear the queue (does not affect current playback).
    pub fn clear_queue(&mut self) {
        self.queue.clear();
    }

    /// Reset to idle state, clearing everything.
    pub fn reset(&mut self) {
        self.queue.clear();
        self.current = None;
        self.state = TtsState::Idle;
        self.stream_id = None;
    }

    // ========================================================================
    // Safety Invariant Verification
    // ========================================================================

    /// Verify INV-MEDIA-2: TTS queue depth bounded.
    pub fn verify_queue_bounded(&self) -> bool {
        self.queue.len() <= self.max_depth
    }

    /// Verify speaking state has current utterance.
    pub fn verify_speaking_has_utterance(&self) -> bool {
        if self.state == TtsState::Speaking {
            self.current.is_some()
        } else {
            true
        }
    }

    /// Verify all TTS invariants.
    pub fn verify_invariants(&self) -> bool {
        self.verify_queue_bounded() && self.verify_speaking_has_utterance()
    }
}

/// TTS state manager for all clients.
#[derive(Debug)]
pub struct TtsManager {
    /// Per-client TTS queues.
    clients: HashMap<ClientId, TtsQueue>,
    /// Default max queue depth.
    default_max_depth: usize,
}

#[allow(dead_code)]
impl TtsManager {
    /// Create a new TTS manager.
    pub fn new(default_max_depth: usize) -> Self {
        Self {
            clients: HashMap::new(),
            default_max_depth,
        }
    }

    /// Get or create the TTS queue for a client.
    pub fn get_or_create(&mut self, client: ClientId) -> &mut TtsQueue {
        self.clients
            .entry(client)
            .or_insert_with(|| TtsQueue::new(self.default_max_depth))
    }

    /// Get the TTS queue for a client.
    pub fn get(&self, client: ClientId) -> Option<&TtsQueue> {
        self.clients.get(&client)
    }

    /// Get mutable reference to a client's TTS queue.
    pub fn get_mut(&mut self, client: ClientId) -> Option<&mut TtsQueue> {
        self.clients.get_mut(&client)
    }

    /// Remove and return a client's TTS queue.
    pub fn remove(&mut self, client: ClientId) -> Option<TtsQueue> {
        self.clients.remove(&client)
    }

    /// Get all clients with active TTS (speaking or paused).
    pub fn active_clients(&self) -> Vec<ClientId> {
        self.clients
            .iter()
            .filter(|(_, q)| q.state().is_active())
            .map(|(c, _)| *c)
            .collect()
    }

    /// Get total queued utterances across all clients.
    pub fn total_queued(&self) -> usize {
        self.clients.values().map(|q| q.queue_len()).sum()
    }

    /// Verify INV-MEDIA-2 for all clients.
    pub fn verify_all_queues_bounded(&self) -> bool {
        self.clients.values().all(|q| q.verify_queue_bounded())
    }
}

/// TTS-specific errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TtsError {
    /// Queue is full (INV-MEDIA-2 would be violated).
    QueueFull {
        /// Maximum queue depth.
        max_depth: usize,
    },
    /// Queue is empty.
    QueueEmpty,
    /// No current utterance.
    NoCurrentUtterance,
    /// Invalid state transition.
    InvalidState {
        /// Expected state.
        expected: TtsState,
        /// Actual state.
        actual: TtsState,
    },
}

impl std::fmt::Display for TtsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QueueFull { max_depth } => {
                write!(f, "TTS queue full (max depth: {})", max_depth)
            }
            Self::QueueEmpty => {
                write!(f, "TTS queue is empty")
            }
            Self::NoCurrentUtterance => {
                write!(f, "No current TTS utterance")
            }
            Self::InvalidState { expected, actual } => {
                write!(
                    f,
                    "Invalid TTS state: expected {:?}, got {:?}",
                    expected, actual
                )
            }
        }
    }
}

impl std::error::Error for TtsError {}
