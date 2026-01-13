//! Speech-to-Text (STT) session management.
//!
//! Implements the STT portion of the MediaServer.tla specification.
//!
//! ## State Machine
//!
//! ```text
//! ┌──────┐  StartSTT   ┌───────────┐  EndUtterance  ┌────────────┐
//! │ Idle │ ──────────▶ │ Listening │ ─────────────▶ │ Processing │
//! └──────┘             └───────────┘                └────────────┘
//!    ▲                      │                             │
//!    │     Cancel/Error     │                             │
//!    ├──────────────────────┘                             │
//!    │                      DeliverResult                 │
//!    └────────────────────────────────────────────────────┘
//! ```

use super::{AudioFormat, ClientId, StreamId};
use std::time::Instant;

/// STT session states (from TLA+ spec: STTStates).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SttState {
    /// No active STT session.
    #[default]
    Idle,
    /// Actively listening for voice input.
    Listening,
    /// Processing captured audio for recognition.
    Processing,
    /// Error state (recognition failed, timeout, etc.).
    Error,
}

impl SttState {
    /// Returns true if the STT session is active (listening or processing).
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Listening | Self::Processing)
    }
}

/// A recognized speech result.
#[derive(Debug, Clone)]
pub struct SttResult {
    /// The client that initiated the STT session.
    pub client: ClientId,
    /// The recognized text.
    pub text: String,
    /// Recognition confidence (0-100%).
    pub confidence: u8,
    /// Whether this is a final result (vs partial/interim).
    pub is_final: bool,
}

impl SttResult {
    /// Create a new STT result.
    pub fn new(client: ClientId, text: impl Into<String>, confidence: u8, is_final: bool) -> Self {
        Self {
            client,
            text: text.into(),
            confidence: confidence.min(100),
            is_final,
        }
    }

    /// Create a final result with high confidence.
    pub fn final_result(client: ClientId, text: impl Into<String>) -> Self {
        Self::new(client, text, 95, true)
    }

    /// Create a partial/interim result.
    pub fn partial(client: ClientId, text: impl Into<String>, confidence: u8) -> Self {
        Self::new(client, text, confidence, false)
    }
}

/// STT session tracking.
///
/// Enforces INV-MEDIA-1: At most one STT session active at a time.
#[derive(Debug)]
pub struct SttSession {
    /// Current session state.
    state: SttState,
    /// Currently active client (None when Idle).
    active_client: Option<ClientId>,
    /// Audio format being used.
    audio_format: AudioFormat,
    /// When the session started.
    start_time: Option<Instant>,
    /// Associated input audio stream.
    stream_id: Option<StreamId>,
    /// Partial recognition text (updated during listening).
    partial_text: String,
    /// Current confidence level.
    confidence: u8,
}

impl Default for SttSession {
    fn default() -> Self {
        Self::new()
    }
}

impl SttSession {
    /// Create a new idle STT session.
    pub fn new() -> Self {
        Self {
            state: SttState::Idle,
            active_client: None,
            audio_format: AudioFormat::default(),
            start_time: None,
            stream_id: None,
            partial_text: String::new(),
            confidence: 0,
        }
    }

    /// Get the current state.
    pub fn state(&self) -> SttState {
        self.state
    }

    /// Get the active client, if any.
    pub fn active_client(&self) -> Option<ClientId> {
        self.active_client
    }

    /// Get the audio format.
    pub fn audio_format(&self) -> AudioFormat {
        self.audio_format
    }

    /// Get the associated stream ID.
    pub fn stream_id(&self) -> Option<StreamId> {
        self.stream_id
    }

    /// Get the partial recognition text.
    pub fn partial_text(&self) -> &str {
        &self.partial_text
    }

    /// Get current confidence level.
    pub fn confidence(&self) -> u8 {
        self.confidence
    }

    /// Get session duration since start.
    pub fn duration(&self) -> Option<std::time::Duration> {
        self.start_time.map(|t| t.elapsed())
    }

    /// Start a new STT session.
    ///
    /// # Errors
    ///
    /// Returns error if another session is already active (INV-MEDIA-1).
    pub fn start(
        &mut self,
        client: ClientId,
        format: AudioFormat,
        stream_id: StreamId,
    ) -> Result<(), SttError> {
        // INV-MEDIA-1: At most one STT session active at a time
        if self.state != SttState::Idle {
            return Err(SttError::SessionAlreadyActive {
                current_client: self.active_client,
            });
        }

        self.state = SttState::Listening;
        self.active_client = Some(client);
        self.audio_format = format;
        self.start_time = Some(Instant::now());
        self.stream_id = Some(stream_id);
        self.partial_text.clear();
        self.confidence = 0;

        Ok(())
    }

    /// Update partial recognition result during listening.
    ///
    /// # Errors
    ///
    /// Returns error if not in Listening state.
    pub fn update_partial(
        &mut self,
        text: impl Into<String>,
        confidence: u8,
    ) -> Result<(), SttError> {
        if self.state != SttState::Listening {
            return Err(SttError::InvalidState {
                expected: SttState::Listening,
                actual: self.state,
            });
        }

        self.partial_text = text.into();
        self.confidence = confidence.min(100);
        Ok(())
    }

    /// End the utterance and begin processing.
    ///
    /// # Errors
    ///
    /// Returns error if not in Listening state.
    pub fn end_utterance(&mut self) -> Result<(), SttError> {
        if self.state != SttState::Listening {
            return Err(SttError::InvalidState {
                expected: SttState::Listening,
                actual: self.state,
            });
        }

        // INV-MEDIA-5: No orphaned processing state
        if self.active_client.is_none() {
            return Err(SttError::NoActiveClient);
        }

        self.state = SttState::Processing;
        Ok(())
    }

    /// Deliver the final recognition result and return to idle.
    ///
    /// # Errors
    ///
    /// Returns error if not in Processing state.
    pub fn deliver_result(
        &mut self,
        text: impl Into<String>,
        confidence: u8,
    ) -> Result<SttResult, SttError> {
        if self.state != SttState::Processing {
            return Err(SttError::InvalidState {
                expected: SttState::Processing,
                actual: self.state,
            });
        }

        // INV-MEDIA-5: No orphaned processing state
        let client = self.active_client.ok_or(SttError::NoActiveClient)?;

        let result = SttResult::new(client, text, confidence, true);

        // Reset to idle
        self.reset();

        Ok(result)
    }

    /// Cancel the current session and return to idle.
    ///
    /// Returns the client that was using the session, if any.
    pub fn cancel(&mut self) -> Option<ClientId> {
        let client = self.active_client;
        self.reset();
        client
    }

    /// Handle an error and return to idle.
    ///
    /// Returns the client that was using the session, if any.
    pub fn error(&mut self) -> Option<ClientId> {
        let client = self.active_client;
        self.state = SttState::Error;
        // Brief error state, then reset
        self.reset();
        client
    }

    /// Reset the session to idle state.
    fn reset(&mut self) {
        self.state = SttState::Idle;
        self.active_client = None;
        self.start_time = None;
        self.stream_id = None;
        self.partial_text.clear();
        self.confidence = 0;
    }

    // ========================================================================
    // Safety Invariant Verification (TLA+ spec)
    // ========================================================================

    /// Verify INV-MEDIA-1: At most one STT session active at a time.
    ///
    /// This is enforced structurally by having a single SttSession.
    pub fn verify_single_session(&self) -> bool {
        // If state is active, we must have a client
        if self.state.is_active() {
            self.active_client.is_some()
        } else {
            true
        }
    }

    /// Verify INV-MEDIA-5: No orphaned processing state.
    pub fn verify_no_orphaned_processing(&self) -> bool {
        if self.state == SttState::Processing {
            self.active_client.is_some()
        } else {
            true
        }
    }

    /// Verify INV-MEDIA-7: Idle STT has no active client.
    pub fn verify_idle_no_client(&self) -> bool {
        if self.state == SttState::Idle {
            self.active_client.is_none()
        } else {
            true
        }
    }

    /// Verify all STT-related safety invariants.
    pub fn verify_invariants(&self) -> bool {
        self.verify_single_session()
            && self.verify_no_orphaned_processing()
            && self.verify_idle_no_client()
    }
}

/// STT-specific errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SttError {
    /// Another STT session is already active.
    SessionAlreadyActive {
        /// The client currently using the session.
        current_client: Option<ClientId>,
    },
    /// Invalid state transition.
    InvalidState {
        /// Expected state.
        expected: SttState,
        /// Actual state.
        actual: SttState,
    },
    /// No active client for the operation.
    NoActiveClient,
}

impl std::fmt::Display for SttError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SessionAlreadyActive { current_client } => {
                write!(
                    f,
                    "STT session already active for client {:?}",
                    current_client
                )
            }
            Self::InvalidState { expected, actual } => {
                write!(
                    f,
                    "Invalid STT state: expected {:?}, got {:?}",
                    expected, actual
                )
            }
            Self::NoActiveClient => {
                write!(f, "No active client for STT operation")
            }
        }
    }
}

impl std::error::Error for SttError {}
