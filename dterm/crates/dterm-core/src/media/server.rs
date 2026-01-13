//! Media server implementation.
//!
//! The MediaServer coordinates STT, TTS, and audio streams according to
//! the state machine defined in `tla/MediaServer.tla`.
//!
//! ## Safety Invariants
//!
//! - **INV-MEDIA-1**: At most one STT session active at a time
//! - **INV-MEDIA-2**: TTS queue depth bounded per client
//! - **INV-MEDIA-3**: Active streams have valid clients
//! - **INV-MEDIA-4**: Latency within bounds (soft constraint)
//! - **INV-MEDIA-5**: No orphaned processing state
//! - **INV-MEDIA-6**: Speaking client has TTS state
//! - **INV-MEDIA-7**: Idle STT has no active client

use super::{
    platform::{
        AudioInputError, AudioInputProvider, NullSttProvider, NullTtsProvider,
        PlatformCapabilities, SttProvider, TtsProvider,
    },
    stream::{StreamDirection, StreamError, StreamId, StreamManager},
    stt::{SttError, SttResult, SttSession, SttState},
    tts::{Priority, TtsError, TtsManager, TtsQueue, TtsState, TtsUtterance},
    AudioFormat,
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Unique identifier for a client (agent or terminal).
pub type ClientId = u64;

/// Media server configuration.
#[derive(Debug, Clone)]
pub struct MediaServerConfig {
    /// Maximum TTS queue depth per client.
    pub max_tts_queue_depth: usize,
    /// Maximum audio stream duration (milliseconds).
    pub max_stream_duration_ms: u64,
    /// Maximum acceptable latency (milliseconds).
    pub max_latency_ms: u64,
}

impl Default for MediaServerConfig {
    fn default() -> Self {
        Self {
            max_tts_queue_depth: 10,
            max_stream_duration_ms: 30_000, // 30 seconds
            max_latency_ms: 100,            // 100ms
        }
    }
}

/// Media server result type.
pub type MediaServerResult<T> = Result<T, MediaServerError>;

/// Shared audio buffer for passing data from audio input callback to STT.
type SharedAudioBuffer = Arc<Mutex<Vec<u8>>>;

/// Media server coordinating voice I/O.
///
/// Implements the state machine from `tla/MediaServer.tla`.
pub struct MediaServer {
    /// Configuration.
    config: MediaServerConfig,
    /// STT session (at most one active - INV-MEDIA-1).
    stt: SttSession,
    /// TTS manager (per-client queues - INV-MEDIA-2).
    tts: TtsManager,
    /// Audio stream manager (INV-MEDIA-3).
    streams: StreamManager,
    /// Pending STT results awaiting delivery.
    pending_results: VecDeque<SttResult>,
    /// Logical clock for latency tracking.
    clock: u64,
    /// Platform STT provider.
    stt_provider: Box<dyn SttProvider>,
    /// Platform TTS provider.
    tts_provider: Box<dyn TtsProvider>,
    /// Platform audio input provider.
    audio_input: Option<Box<dyn AudioInputProvider>>,
    /// Shared buffer for audio data from input callback.
    audio_buffer: SharedAudioBuffer,
}

impl std::fmt::Debug for MediaServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaServer")
            .field("config", &self.config)
            .field("stt_state", &self.stt.state())
            .field("tts_active_clients", &self.tts.active_clients().len())
            .field("active_streams", &self.streams.active_count())
            .field("pending_results", &self.pending_results.len())
            .field("clock", &self.clock)
            .field("has_audio_input", &self.audio_input.is_some())
            .finish()
    }
}

impl MediaServer {
    /// Create a new media server with default (null) providers.
    pub fn new(config: MediaServerConfig) -> Self {
        let max_duration = Duration::from_millis(config.max_stream_duration_ms);
        let max_latency = Duration::from_millis(config.max_latency_ms);

        Self {
            stt: SttSession::new(),
            tts: TtsManager::new(config.max_tts_queue_depth),
            streams: StreamManager::new(max_duration, max_latency),
            pending_results: VecDeque::new(),
            clock: 0,
            stt_provider: Box::new(NullSttProvider),
            tts_provider: Box::new(NullTtsProvider),
            audio_input: None,
            audio_buffer: Arc::new(Mutex::new(Vec::new())),
            config,
        }
    }

    /// Create with platform providers.
    pub fn with_providers(
        config: MediaServerConfig,
        stt_provider: Box<dyn SttProvider>,
        tts_provider: Box<dyn TtsProvider>,
    ) -> Self {
        let mut server = Self::new(config);
        server.stt_provider = stt_provider;
        server.tts_provider = tts_provider;
        server
    }

    /// Create with all platform providers including audio input.
    pub fn with_all_providers(
        config: MediaServerConfig,
        stt_provider: Box<dyn SttProvider>,
        tts_provider: Box<dyn TtsProvider>,
        audio_input: Box<dyn AudioInputProvider>,
    ) -> Self {
        let mut server = Self::with_providers(config, stt_provider, tts_provider);
        server.audio_input = Some(audio_input);
        server
    }

    /// Set the audio input provider.
    pub fn set_audio_input(&mut self, provider: Box<dyn AudioInputProvider>) {
        self.audio_input = Some(provider);
    }

    /// Check if audio input is available.
    pub fn has_audio_input(&self) -> bool {
        self.audio_input.is_some()
    }

    /// Get platform capabilities.
    pub fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            stt_formats: self.stt_provider.supported_formats().to_vec(),
            tts_formats: self.tts_provider.supported_formats().to_vec(),
            supports_continuous_stt: true,
            supports_vad: self.stt_provider.is_voice_active().is_some(),
            supports_offline_stt: false,
            supports_offline_tts: false,
            stt_languages: self.stt_provider.supported_languages().to_vec(),
            tts_voices: self.tts_provider.available_voices().to_vec(),
        }
    }

    // ========================================================================
    // STT Operations
    // ========================================================================

    /// Start an STT session for a client.
    ///
    /// # Errors
    ///
    /// Returns error if another session is already active (INV-MEDIA-1).
    pub fn start_stt(
        &mut self,
        client: ClientId,
        format: AudioFormat,
    ) -> MediaServerResult<StreamId> {
        // Create input stream
        let stream_id = self.streams.create(client, StreamDirection::Input, format);

        // Start STT session (enforces INV-MEDIA-1)
        self.stt.start(client, format, stream_id).map_err(|e| {
            // Clean up stream on failure
            self.streams.close(stream_id);
            MediaServerError::Stt(e)
        })?;

        Ok(stream_id)
    }

    /// Feed audio data to the STT session.
    ///
    /// # Errors
    ///
    /// Returns error if no session is active.
    pub fn stt_feed_audio(&mut self, data: &[u8]) -> MediaServerResult<Option<SttResult>> {
        if self.stt.state() != SttState::Listening {
            return Err(MediaServerError::Stt(SttError::InvalidState {
                expected: SttState::Listening,
                actual: self.stt.state(),
            }));
        }

        // Update stream bytes
        if let Some(stream_id) = self.stt.stream_id() {
            if let Some(stream) = self.streams.get_mut(stream_id) {
                #[allow(clippy::cast_possible_truncation)]
                stream.record_transfer(data.len() as u64);
            }
        }

        // Get partial result if available
        if let Some(result) = self.stt_provider.get_partial() {
            let _ = self.stt.update_partial(&result.text, result.confidence);
            if !result.is_final {
                return Ok(Some(result));
            }
        }

        Ok(None)
    }

    /// End the STT utterance and begin processing.
    ///
    /// # Errors
    ///
    /// Returns error if not listening.
    pub fn stt_end_utterance(&mut self) -> MediaServerResult<()> {
        self.stt.end_utterance().map_err(MediaServerError::Stt)?;

        // Begin closing the input stream
        if let Some(stream_id) = self.stt.stream_id() {
            if let Some(stream) = self.streams.get_mut(stream_id) {
                stream.begin_close();
            }
        }

        Ok(())
    }

    /// Deliver the STT result and return to idle.
    ///
    /// # Errors
    ///
    /// Returns error if not processing.
    pub fn stt_deliver_result(
        &mut self,
        text: impl Into<String>,
        confidence: u8,
    ) -> MediaServerResult<SttResult> {
        let result = self
            .stt
            .deliver_result(text, confidence)
            .map_err(MediaServerError::Stt)?;

        // Queue result for delivery
        self.pending_results.push_back(result.clone());

        Ok(result)
    }

    /// Cancel the STT session.
    pub fn stt_cancel(&mut self) -> Option<ClientId> {
        // Close associated stream
        if let Some(stream_id) = self.stt.stream_id() {
            self.streams.close(stream_id);
        }

        self.stt.cancel()
    }

    /// Handle STT error.
    pub fn stt_error(&mut self) -> Option<ClientId> {
        // Close associated stream
        if let Some(stream_id) = self.stt.stream_id() {
            self.streams.close(stream_id);
        }

        self.stt.error()
    }

    /// Get the current STT state.
    pub fn stt_state(&self) -> SttState {
        self.stt.state()
    }

    /// Get the active STT client.
    pub fn stt_active_client(&self) -> Option<ClientId> {
        self.stt.active_client()
    }

    // ========================================================================
    // Audio Input Operations
    // ========================================================================

    /// Start an STT session with automatic microphone capture.
    ///
    /// This combines `start_stt` with audio input capture. Audio is captured
    /// from the microphone and automatically fed to the STT provider.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Another session is already active (INV-MEDIA-1)
    /// - No audio input provider is configured
    /// - Audio capture fails to start
    pub fn start_stt_with_microphone(
        &mut self,
        client: ClientId,
        format: AudioFormat,
        language: Option<&str>,
    ) -> MediaServerResult<StreamId> {
        // Check if audio input is available
        if self.audio_input.is_none() {
            return Err(MediaServerError::Provider(
                "No audio input provider configured".to_string(),
            ));
        }

        // Start STT session
        let stream_id = self.start_stt(client, format)?;

        // Start the STT provider
        if let Err(e) = self.stt_provider.start(format, language) {
            // Clean up on failure
            self.stt.cancel();
            self.streams.close(stream_id);
            return Err(MediaServerError::Provider(format!(
                "STT provider error: {}",
                e
            )));
        }

        // Clear the audio buffer
        if let Ok(mut buffer) = self.audio_buffer.lock() {
            buffer.clear();
        }

        // Start audio capture
        let buffer_clone = Arc::clone(&self.audio_buffer);
        let callback = Box::new(move |data: &[u8]| {
            if let Ok(mut buffer) = buffer_clone.lock() {
                buffer.extend_from_slice(data);
            }
        });

        if let Some(ref mut audio_input) = self.audio_input {
            if let Err(e) = audio_input.start(format, None, callback) {
                // Clean up on failure
                self.stt_provider.cancel();
                self.stt.cancel();
                self.streams.close(stream_id);
                return Err(MediaServerError::AudioInput(e));
            }
        }

        Ok(stream_id)
    }

    /// Process pending audio data from the microphone.
    ///
    /// Call this periodically (e.g., in a tick loop) to process captured audio
    /// and get partial recognition results.
    ///
    /// # Errors
    ///
    /// Returns error if no STT session is active.
    pub fn process_audio(&mut self) -> MediaServerResult<Option<SttResult>> {
        if self.stt.state() != SttState::Listening {
            return Err(MediaServerError::Stt(SttError::InvalidState {
                expected: SttState::Listening,
                actual: self.stt.state(),
            }));
        }

        // Get pending audio data
        let data = {
            let mut buffer = self.audio_buffer.lock().unwrap();
            if buffer.is_empty() {
                return Ok(None);
            }
            std::mem::take(&mut *buffer)
        };

        // Feed audio to STT provider
        if let Err(e) = self.stt_provider.feed_audio(&data) {
            return Err(MediaServerError::Provider(format!("STT feed error: {}", e)));
        }

        // Update stream stats
        if let Some(stream_id) = self.stt.stream_id() {
            if let Some(stream) = self.streams.get_mut(stream_id) {
                #[allow(clippy::cast_possible_truncation)]
                stream.record_transfer(data.len() as u64);
            }
        }

        // Get partial result if available
        if let Some(result) = self.stt_provider.get_partial() {
            let _ = self.stt.update_partial(&result.text, result.confidence);
            return Ok(Some(result));
        }

        Ok(None)
    }

    /// Stop microphone capture and get the final STT result.
    ///
    /// Stops audio capture, processes any remaining audio, and returns
    /// the final recognition result.
    ///
    /// # Errors
    ///
    /// Returns error if no STT session is active.
    pub fn stop_stt_with_microphone(&mut self) -> MediaServerResult<Option<SttResult>> {
        // Stop audio capture first
        if let Some(ref mut audio_input) = self.audio_input {
            audio_input.stop();
        }

        // Process any remaining audio
        let _ = self.process_audio();

        // Stop the STT provider and get final result
        let final_result = match self.stt_provider.stop() {
            Ok(result) => result,
            Err(e) => {
                // Still clean up even on error
                self.stt.cancel();
                return Err(MediaServerError::Provider(format!("STT stop error: {}", e)));
            }
        };

        // Transition to processing state
        if self.stt.state() == SttState::Listening {
            let _ = self.stt.end_utterance();
        }

        // Deliver the result
        if let Some(result) = final_result {
            let delivered = self
                .stt
                .deliver_result(&result.text, result.confidence)
                .map_err(MediaServerError::Stt)?;
            self.pending_results.push_back(delivered.clone());
            Ok(Some(delivered))
        } else {
            // No result, just cancel
            self.stt.cancel();
            Ok(None)
        }
    }

    /// Cancel microphone capture and STT session.
    pub fn cancel_stt_with_microphone(&mut self) -> Option<ClientId> {
        // Stop audio capture
        if let Some(ref mut audio_input) = self.audio_input {
            audio_input.stop();
        }

        // Cancel STT provider
        self.stt_provider.cancel();

        // Clear audio buffer
        if let Ok(mut buffer) = self.audio_buffer.lock() {
            buffer.clear();
        }

        // Cancel STT session
        self.stt_cancel()
    }

    /// Check if the audio input provider is currently capturing.
    pub fn is_capturing_audio(&self) -> bool {
        self.audio_input
            .as_ref()
            .is_some_and(|ai| ai.is_capturing())
    }

    /// Check if voice activity is detected (if VAD is supported).
    pub fn is_voice_active(&self) -> Option<bool> {
        self.stt_provider.is_voice_active()
    }

    // ========================================================================
    // TTS Operations
    // ========================================================================

    /// Queue a TTS utterance for a client.
    ///
    /// # Errors
    ///
    /// Returns error if queue is full (INV-MEDIA-2).
    pub fn queue_tts(
        &mut self,
        client: ClientId,
        text: impl Into<String>,
        priority: Priority,
    ) -> MediaServerResult<u64> {
        let queue = self.tts.get_or_create(client);
        let id = queue
            .queue(text, priority, AudioFormat::default())
            .map_err(MediaServerError::Tts)?;
        Ok(id)
    }

    /// Start speaking the next queued utterance.
    ///
    /// # Errors
    ///
    /// Returns error if not idle or queue is empty.
    pub fn start_tts(&mut self, client: ClientId) -> MediaServerResult<&TtsUtterance> {
        // Create output stream
        let format = AudioFormat::default();
        let stream_id = self.streams.create(client, StreamDirection::Output, format);

        let queue = self.tts.get_or_create(client);
        match queue.start(stream_id) {
            Ok(utterance) => Ok(utterance),
            Err(e) => {
                // Clean up stream on failure
                self.streams.close(stream_id);
                Err(MediaServerError::Tts(e))
            }
        }
    }

    /// Complete the current TTS utterance.
    ///
    /// # Errors
    ///
    /// Returns error if not speaking.
    pub fn complete_tts(&mut self, client: ClientId) -> MediaServerResult<TtsUtterance> {
        let queue = self
            .tts
            .get_mut(client)
            .ok_or(MediaServerError::ClientNotFound(client))?;

        // Close the stream
        if let Some(stream_id) = queue.stream_id() {
            self.streams.close(stream_id);
        }

        queue.complete().map_err(MediaServerError::Tts)
    }

    /// Pause TTS playback.
    ///
    /// # Errors
    ///
    /// Returns error if not speaking.
    pub fn pause_tts(&mut self, client: ClientId) -> MediaServerResult<()> {
        let queue = self
            .tts
            .get_mut(client)
            .ok_or(MediaServerError::ClientNotFound(client))?;

        // Pause the stream
        if let Some(stream_id) = queue.stream_id() {
            if let Some(stream) = self.streams.get_mut(stream_id) {
                let _ = stream.pause();
            }
        }

        queue.pause().map_err(MediaServerError::Tts)
    }

    /// Resume TTS playback.
    ///
    /// # Errors
    ///
    /// Returns error if not paused.
    pub fn resume_tts(&mut self, client: ClientId) -> MediaServerResult<()> {
        let queue = self
            .tts
            .get_mut(client)
            .ok_or(MediaServerError::ClientNotFound(client))?;

        // Resume the stream
        if let Some(stream_id) = queue.stream_id() {
            if let Some(stream) = self.streams.get_mut(stream_id) {
                let _ = stream.resume();
            }
        }

        queue.resume().map_err(MediaServerError::Tts)
    }

    /// Cancel TTS playback.
    ///
    /// # Errors
    ///
    /// Returns error if not speaking or paused.
    pub fn cancel_tts(
        &mut self,
        client: ClientId,
        clear_queue: bool,
    ) -> MediaServerResult<Option<TtsUtterance>> {
        let queue = self
            .tts
            .get_mut(client)
            .ok_or(MediaServerError::ClientNotFound(client))?;

        // Close the stream
        if let Some(stream_id) = queue.stream_id() {
            self.streams.close(stream_id);
        }

        queue.cancel(clear_queue).map_err(MediaServerError::Tts)
    }

    /// Interrupt TTS with a high-priority utterance.
    ///
    /// # Errors
    ///
    /// Returns error if not speaking.
    pub fn interrupt_tts(
        &mut self,
        client: ClientId,
        text: impl Into<String>,
    ) -> MediaServerResult<u64> {
        let queue = self
            .tts
            .get_mut(client)
            .ok_or(MediaServerError::ClientNotFound(client))?;

        // Close the current stream
        if let Some(stream_id) = queue.stream_id() {
            self.streams.close(stream_id);
        }

        queue
            .interrupt(text, AudioFormat::default())
            .map_err(MediaServerError::Tts)
    }

    /// Get the TTS state for a client.
    pub fn tts_state(&self, client: ClientId) -> TtsState {
        self.tts
            .get(client)
            .map(|q| q.state())
            .unwrap_or(TtsState::Idle)
    }

    /// Get the TTS queue for a client.
    pub fn tts_queue(&self, client: ClientId) -> Option<&TtsQueue> {
        self.tts.get(client)
    }

    // ========================================================================
    // Client Operations
    // ========================================================================

    /// Handle client disconnect - cleanup all resources.
    pub fn client_disconnect(&mut self, client: ClientId) {
        // Close all streams for client
        self.streams.close_all_for_client(client);

        // Reset STT if this client was active
        if self.stt.active_client() == Some(client) {
            self.stt.cancel();
        }

        // Reset TTS state
        if let Some(queue) = self.tts.get_mut(client) {
            queue.reset();
        }

        // Remove pending results for client
        self.pending_results.retain(|r| r.client != client);
    }

    /// Consume the next pending STT result for a client.
    pub fn consume_result(&mut self, client: ClientId) -> Option<SttResult> {
        if let Some(pos) = self.pending_results.iter().position(|r| r.client == client) {
            self.pending_results.remove(pos)
        } else {
            None
        }
    }

    /// Get pending results count for a client.
    pub fn pending_results_count(&self, client: ClientId) -> usize {
        self.pending_results
            .iter()
            .filter(|r| r.client == client)
            .count()
    }

    // ========================================================================
    // Stream Operations
    // ========================================================================

    /// Get a stream by ID.
    pub fn stream(&self, id: StreamId) -> Option<&super::stream::AudioStream> {
        self.streams.get(id)
    }

    /// Check for stream timeouts.
    ///
    /// Returns the IDs of streams that were timed out.
    pub fn check_stream_timeouts(&mut self) -> Vec<StreamId> {
        let timed_out = self.streams.check_timeouts();

        // Handle any STT/TTS state cleanup for timed-out streams
        for &stream_id in &timed_out {
            if Some(stream_id) == self.stt.stream_id() {
                self.stt.cancel();
            }
            // TTS cleanup is handled by stream manager
        }

        timed_out
    }

    /// Garbage collect closed streams.
    pub fn cleanup_streams(&mut self) -> usize {
        self.streams.cleanup_closed()
    }

    // ========================================================================
    // Clock Operations
    // ========================================================================

    /// Advance the logical clock.
    pub fn tick(&mut self) {
        self.clock += 1;
    }

    /// Get the current clock value.
    pub fn clock(&self) -> u64 {
        self.clock
    }

    // ========================================================================
    // Safety Invariant Verification
    // ========================================================================

    /// Verify INV-MEDIA-1: At most one STT session active at a time.
    pub fn verify_single_stt_session(&self) -> bool {
        self.stt.verify_single_session()
    }

    /// Verify INV-MEDIA-2: TTS queue depth bounded per client.
    pub fn verify_tts_queues_bounded(&self) -> bool {
        self.tts.verify_all_queues_bounded()
    }

    /// Verify INV-MEDIA-3: Active streams have valid clients.
    pub fn verify_stream_clients_valid(&self) -> bool {
        self.streams.verify_streams_have_clients()
    }

    /// Verify INV-MEDIA-5: No orphaned processing state.
    pub fn verify_no_orphaned_processing(&self) -> bool {
        self.stt.verify_no_orphaned_processing()
    }

    /// Verify INV-MEDIA-6: Speaking client has TTS state.
    ///
    /// When a client is speaking, there should be an active output stream.
    pub fn verify_speaking_has_stream(&self) -> bool {
        for client in self.tts.active_clients() {
            if let Some(queue) = self.tts.get(client) {
                if queue.state() == TtsState::Speaking {
                    // Should have an output stream
                    if self.streams.output_stream_for_client(client).is_none() {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Verify INV-MEDIA-7: Idle STT has no active client.
    pub fn verify_idle_stt_no_client(&self) -> bool {
        self.stt.verify_idle_no_client()
    }

    /// Verify all safety invariants.
    pub fn verify_invariants(&self) -> bool {
        self.verify_single_stt_session()
            && self.verify_tts_queues_bounded()
            && self.verify_stream_clients_valid()
            && self.verify_no_orphaned_processing()
            && self.verify_speaking_has_stream()
            && self.verify_idle_stt_no_client()
    }

    /// Get streams with high latency (for monitoring).
    pub fn high_latency_streams(&self) -> Vec<StreamId> {
        self.streams.high_latency_streams()
    }
}

/// Media server errors.
#[derive(Debug)]
pub enum MediaServerError {
    /// STT operation error.
    Stt(SttError),
    /// TTS operation error.
    Tts(TtsError),
    /// Stream operation error.
    Stream(StreamError),
    /// Audio input error.
    AudioInput(AudioInputError),
    /// Client not found.
    ClientNotFound(ClientId),
    /// Platform provider error.
    Provider(String),
}

impl std::fmt::Display for MediaServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stt(e) => write!(f, "STT error: {}", e),
            Self::Tts(e) => write!(f, "TTS error: {}", e),
            Self::Stream(e) => write!(f, "Stream error: {}", e),
            Self::AudioInput(e) => write!(f, "Audio input error: {}", e),
            Self::ClientNotFound(id) => write!(f, "Client not found: {}", id),
            Self::Provider(msg) => write!(f, "Provider error: {}", msg),
        }
    }
}

impl std::error::Error for MediaServerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Stt(e) => Some(e),
            Self::Tts(e) => Some(e),
            Self::Stream(e) => Some(e),
            Self::AudioInput(e) => Some(e),
            _ => None,
        }
    }
}

impl From<SttError> for MediaServerError {
    fn from(e: SttError) -> Self {
        Self::Stt(e)
    }
}

impl From<TtsError> for MediaServerError {
    fn from(e: TtsError) -> Self {
        Self::Tts(e)
    }
}

impl From<StreamError> for MediaServerError {
    fn from(e: StreamError) -> Self {
        Self::Stream(e)
    }
}

impl From<AudioInputError> for MediaServerError {
    fn from(e: AudioInputError) -> Self {
        Self::AudioInput(e)
    }
}
