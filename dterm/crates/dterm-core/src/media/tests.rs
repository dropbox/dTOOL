//! Unit tests for media module.
//!
//! Tests cover all safety invariants from MediaServer.tla.

use super::stream::{StreamDirection, StreamManager, StreamState};
use super::stt::{SttError, SttSession, SttState};
use super::tts::{Priority, TtsError, TtsManager, TtsQueue, TtsState};
use super::*;
use std::time::Duration;

// ============================================================================
// STT Tests
// ============================================================================

#[test]
fn test_stt_initial_state() {
    let stt = SttSession::new();
    assert_eq!(stt.state(), SttState::Idle);
    assert!(stt.active_client().is_none());
    assert!(stt.verify_invariants());
}

#[test]
fn test_stt_start_session() {
    let mut stt = SttSession::new();
    let result = stt.start(1, AudioFormat::Pcm16k, 100);
    assert!(result.is_ok());
    assert_eq!(stt.state(), SttState::Listening);
    assert_eq!(stt.active_client(), Some(1));
    assert_eq!(stt.stream_id(), Some(100));
    assert!(stt.verify_invariants());
}

#[test]
fn test_stt_inv_media_1_single_session() {
    // INV-MEDIA-1: At most one STT session active at a time
    let mut stt = SttSession::new();

    // Start first session
    assert!(stt.start(1, AudioFormat::Pcm16k, 100).is_ok());

    // Attempt to start second session should fail
    let result = stt.start(2, AudioFormat::Pcm16k, 101);
    assert!(matches!(result, Err(SttError::SessionAlreadyActive { .. })));

    // Original session still active
    assert_eq!(stt.active_client(), Some(1));
    assert!(stt.verify_single_session());
}

#[test]
fn test_stt_update_partial() {
    let mut stt = SttSession::new();
    stt.start(1, AudioFormat::Pcm16k, 100).unwrap();

    assert!(stt.update_partial("hello", 75).is_ok());
    assert_eq!(stt.partial_text(), "hello");
    assert_eq!(stt.confidence(), 75);

    // Update again
    assert!(stt.update_partial("hello world", 85).is_ok());
    assert_eq!(stt.partial_text(), "hello world");
}

#[test]
fn test_stt_end_utterance() {
    let mut stt = SttSession::new();
    stt.start(1, AudioFormat::Pcm16k, 100).unwrap();

    assert!(stt.end_utterance().is_ok());
    assert_eq!(stt.state(), SttState::Processing);
    assert!(stt.verify_invariants());
}

#[test]
fn test_stt_deliver_result() {
    let mut stt = SttSession::new();
    stt.start(1, AudioFormat::Pcm16k, 100).unwrap();
    stt.end_utterance().unwrap();

    let result = stt.deliver_result("hello world", 95).unwrap();
    assert_eq!(result.client, 1);
    assert_eq!(result.text, "hello world");
    assert_eq!(result.confidence, 95);
    assert!(result.is_final);

    // Back to idle
    assert_eq!(stt.state(), SttState::Idle);
    assert!(stt.active_client().is_none());
    assert!(stt.verify_invariants());
}

#[test]
fn test_stt_cancel() {
    let mut stt = SttSession::new();
    stt.start(1, AudioFormat::Pcm16k, 100).unwrap();

    let client = stt.cancel();
    assert_eq!(client, Some(1));
    assert_eq!(stt.state(), SttState::Idle);
    assert!(stt.verify_invariants());
}

#[test]
fn test_stt_inv_media_5_no_orphaned_processing() {
    // INV-MEDIA-5: No orphaned processing state
    let mut stt = SttSession::new();
    stt.start(1, AudioFormat::Pcm16k, 100).unwrap();
    stt.end_utterance().unwrap();

    // In processing state, must have active client
    assert_eq!(stt.state(), SttState::Processing);
    assert!(stt.active_client().is_some());
    assert!(stt.verify_no_orphaned_processing());
}

#[test]
fn test_stt_inv_media_7_idle_no_client() {
    // INV-MEDIA-7: Idle STT has no active client
    let stt = SttSession::new();
    assert_eq!(stt.state(), SttState::Idle);
    assert!(stt.active_client().is_none());
    assert!(stt.verify_idle_no_client());
}

// ============================================================================
// TTS Tests
// ============================================================================

#[test]
fn test_tts_initial_state() {
    let tts = TtsQueue::new(10);
    assert_eq!(tts.state(), TtsState::Idle);
    assert_eq!(tts.queue_len(), 0);
    assert!(tts.verify_invariants());
}

#[test]
fn test_tts_queue_utterance() {
    let mut tts = TtsQueue::new(10);

    let id = tts
        .queue("Hello", Priority::Normal, AudioFormat::Pcm16k)
        .unwrap();
    assert_eq!(id, 0);
    assert_eq!(tts.queue_len(), 1);
    assert!(tts.verify_invariants());
}

#[test]
fn test_tts_inv_media_2_queue_bounded() {
    // INV-MEDIA-2: TTS queue depth bounded per client
    let mut tts = TtsQueue::new(3);

    // Fill the queue
    assert!(tts
        .queue("one", Priority::Normal, AudioFormat::Pcm16k)
        .is_ok());
    assert!(tts
        .queue("two", Priority::Normal, AudioFormat::Pcm16k)
        .is_ok());
    assert!(tts
        .queue("three", Priority::Normal, AudioFormat::Pcm16k)
        .is_ok());

    // Queue is now full
    let result = tts.queue("four", Priority::Normal, AudioFormat::Pcm16k);
    assert!(matches!(result, Err(TtsError::QueueFull { max_depth: 3 })));

    assert!(tts.verify_queue_bounded());
}

#[test]
fn test_tts_priority_ordering() {
    let mut tts = TtsQueue::new(10);

    // Queue low priority first
    tts.queue("low", Priority::Low, AudioFormat::Pcm16k)
        .unwrap();
    // Queue high priority second - should be at front
    tts.queue("high", Priority::High, AudioFormat::Pcm16k)
        .unwrap();

    // Start should get high priority first
    let utterance = tts.start(100).unwrap();
    assert_eq!(utterance.text, "high");
}

#[test]
fn test_tts_start_speaking() {
    let mut tts = TtsQueue::new(10);
    tts.queue("Hello", Priority::Normal, AudioFormat::Pcm16k)
        .unwrap();

    let utterance = tts.start(100).unwrap();
    assert_eq!(utterance.text, "Hello");
    assert_eq!(tts.state(), TtsState::Speaking);
    assert_eq!(tts.queue_len(), 0);
    assert!(tts.verify_invariants());
}

#[test]
fn test_tts_complete() {
    let mut tts = TtsQueue::new(10);
    tts.queue("Hello", Priority::Normal, AudioFormat::Pcm16k)
        .unwrap();
    tts.start(100).unwrap();

    let completed = tts.complete().unwrap();
    assert_eq!(completed.text, "Hello");
    assert_eq!(tts.state(), TtsState::Idle);
    assert!(tts.verify_invariants());
}

#[test]
fn test_tts_pause_resume() {
    let mut tts = TtsQueue::new(10);
    tts.queue("Hello", Priority::Normal, AudioFormat::Pcm16k)
        .unwrap();
    tts.start(100).unwrap();

    // Pause
    assert!(tts.pause().is_ok());
    assert_eq!(tts.state(), TtsState::Paused);

    // Resume
    assert!(tts.resume().is_ok());
    assert_eq!(tts.state(), TtsState::Speaking);

    assert!(tts.verify_invariants());
}

#[test]
fn test_tts_cancel() {
    let mut tts = TtsQueue::new(10);
    tts.queue("Hello", Priority::Normal, AudioFormat::Pcm16k)
        .unwrap();
    tts.queue("World", Priority::Normal, AudioFormat::Pcm16k)
        .unwrap();
    tts.start(100).unwrap();

    // Cancel without clearing queue
    let cancelled = tts.cancel(false).unwrap();
    assert!(cancelled.is_some());
    assert_eq!(tts.state(), TtsState::Idle);
    assert_eq!(tts.queue_len(), 1); // "World" still in queue
}

#[test]
fn test_tts_cancel_clear_queue() {
    let mut tts = TtsQueue::new(10);
    tts.queue("Hello", Priority::Normal, AudioFormat::Pcm16k)
        .unwrap();
    tts.queue("World", Priority::Normal, AudioFormat::Pcm16k)
        .unwrap();
    tts.start(100).unwrap();

    // Cancel and clear queue
    tts.cancel(true).unwrap();
    assert_eq!(tts.queue_len(), 0);
}

#[test]
fn test_tts_interrupt() {
    let mut tts = TtsQueue::new(10);
    tts.queue("Hello", Priority::Normal, AudioFormat::Pcm16k)
        .unwrap();
    tts.start(100).unwrap();

    // Interrupt with urgent message
    let id = tts.interrupt("Urgent!", AudioFormat::Pcm16k).unwrap();
    assert_eq!(tts.state(), TtsState::Idle);
    assert_eq!(tts.queue_len(), 1);

    // Next start should get the interrupt
    let utterance = tts.start(101).unwrap();
    assert_eq!(utterance.text, "Urgent!");
    assert_eq!(utterance.id, id);
}

#[test]
fn test_tts_manager() {
    let mut manager = TtsManager::new(10);

    // Get or create for new client
    let queue = manager.get_or_create(1);
    assert_eq!(queue.state(), TtsState::Idle);

    // Queue something
    queue
        .queue("Hello", Priority::Normal, AudioFormat::Pcm16k)
        .unwrap();

    // Get existing
    let queue = manager.get(1).unwrap();
    assert_eq!(queue.queue_len(), 1);

    // Verify all bounded
    assert!(manager.verify_all_queues_bounded());
}

// ============================================================================
// Stream Tests
// ============================================================================

#[test]
fn test_stream_create() {
    let mut manager = StreamManager::new(Duration::from_secs(30), Duration::from_millis(100));

    let id = manager.create(1, StreamDirection::Input, AudioFormat::Pcm16k);
    let stream = manager.get(id).unwrap();

    assert_eq!(stream.client(), 1);
    assert_eq!(stream.direction(), StreamDirection::Input);
    assert_eq!(stream.state(), StreamState::Active);
    assert!(manager.verify_invariants());
}

#[test]
fn test_stream_pause_resume() {
    let mut manager = StreamManager::new(Duration::from_secs(30), Duration::from_millis(100));

    let id = manager.create(1, StreamDirection::Output, AudioFormat::Pcm16k);

    // Pause
    manager.get_mut(id).unwrap().pause().unwrap();
    assert_eq!(manager.get(id).unwrap().state(), StreamState::Paused);

    // Resume
    manager.get_mut(id).unwrap().resume().unwrap();
    assert_eq!(manager.get(id).unwrap().state(), StreamState::Active);
}

#[test]
fn test_stream_close() {
    let mut manager = StreamManager::new(Duration::from_secs(30), Duration::from_millis(100));

    let id = manager.create(1, StreamDirection::Input, AudioFormat::Pcm16k);
    manager.close(id);

    assert_eq!(manager.get(id).unwrap().state(), StreamState::Closed);
}

#[test]
fn test_stream_close_all_for_client() {
    let mut manager = StreamManager::new(Duration::from_secs(30), Duration::from_millis(100));

    // Create multiple streams for client 1
    let id1 = manager.create(1, StreamDirection::Input, AudioFormat::Pcm16k);
    let id2 = manager.create(1, StreamDirection::Output, AudioFormat::Pcm16k);
    // Create stream for client 2
    let id3 = manager.create(2, StreamDirection::Input, AudioFormat::Pcm16k);

    manager.close_all_for_client(1);

    assert_eq!(manager.get(id1).unwrap().state(), StreamState::Closed);
    assert_eq!(manager.get(id2).unwrap().state(), StreamState::Closed);
    assert_eq!(manager.get(id3).unwrap().state(), StreamState::Active);
}

#[test]
fn test_stream_cleanup_closed() {
    let mut manager = StreamManager::new(Duration::from_secs(30), Duration::from_millis(100));

    let id1 = manager.create(1, StreamDirection::Input, AudioFormat::Pcm16k);
    let id2 = manager.create(1, StreamDirection::Output, AudioFormat::Pcm16k);

    manager.close(id1);

    let removed = manager.cleanup_closed();
    assert_eq!(removed, 1);
    assert!(manager.get(id1).is_none());
    assert!(manager.get(id2).is_some());
}

#[test]
fn test_stream_inv_media_3_valid_clients() {
    // INV-MEDIA-3: Active streams have valid clients
    let manager = StreamManager::new(Duration::from_secs(30), Duration::from_millis(100));

    // All streams created with valid client IDs
    assert!(manager.verify_streams_have_clients());
}

// ============================================================================
// MediaServer Integration Tests
// ============================================================================

#[test]
fn test_media_server_new() {
    let server = MediaServer::new(MediaServerConfig::default());
    assert_eq!(server.stt_state(), SttState::Idle);
    assert!(server.verify_invariants());
}

#[test]
fn test_media_server_stt_flow() {
    let mut server = MediaServer::new(MediaServerConfig::default());

    // Start STT
    let stream_id = server.start_stt(1, AudioFormat::Pcm16k).unwrap();
    assert_eq!(server.stt_state(), SttState::Listening);
    assert_eq!(server.stt_active_client(), Some(1));

    // Stream created
    let stream = server.stream(stream_id).unwrap();
    assert_eq!(stream.direction(), StreamDirection::Input);

    // End utterance
    server.stt_end_utterance().unwrap();
    assert_eq!(server.stt_state(), SttState::Processing);

    // Deliver result
    let result = server.stt_deliver_result("test", 95).unwrap();
    assert_eq!(result.text, "test");
    assert_eq!(server.stt_state(), SttState::Idle);

    // Result available for consumption
    let consumed = server.consume_result(1).unwrap();
    assert_eq!(consumed.text, "test");

    assert!(server.verify_invariants());
}

#[test]
fn test_media_server_tts_flow() {
    let mut server = MediaServer::new(MediaServerConfig::default());

    // Queue TTS
    let id = server
        .queue_tts(1, "Hello world", Priority::Normal)
        .unwrap();
    assert_eq!(id, 0);

    // Start speaking
    let utterance = server.start_tts(1).unwrap();
    assert_eq!(utterance.text, "Hello world");
    assert_eq!(server.tts_state(1), TtsState::Speaking);

    // Complete
    let completed = server.complete_tts(1).unwrap();
    assert_eq!(completed.text, "Hello world");
    assert_eq!(server.tts_state(1), TtsState::Idle);

    assert!(server.verify_invariants());
}

#[test]
fn test_media_server_client_disconnect() {
    let mut server = MediaServer::new(MediaServerConfig::default());

    // Set up STT for client 1
    server.start_stt(1, AudioFormat::Pcm16k).unwrap();

    // Set up TTS for client 1
    server.queue_tts(1, "Hello", Priority::Normal).unwrap();

    // Disconnect client
    server.client_disconnect(1);

    // STT should be reset
    assert_eq!(server.stt_state(), SttState::Idle);
    assert!(server.stt_active_client().is_none());

    // TTS should be reset
    assert_eq!(server.tts_state(1), TtsState::Idle);

    assert!(server.verify_invariants());
}

#[test]
fn test_media_server_inv_media_6_speaking_has_stream() {
    // INV-MEDIA-6: Speaking client has TTS state
    let mut server = MediaServer::new(MediaServerConfig::default());

    // Queue and start TTS
    server.queue_tts(1, "Hello", Priority::Normal).unwrap();
    server.start_tts(1).unwrap();

    // Speaking should have output stream
    assert!(server.verify_speaking_has_stream());
}

#[test]
fn test_media_server_all_invariants() {
    let mut server = MediaServer::new(MediaServerConfig::default());

    // Run through various operations
    server.start_stt(1, AudioFormat::Pcm16k).unwrap();
    assert!(server.verify_invariants());

    server.stt_cancel();
    assert!(server.verify_invariants());

    server.queue_tts(1, "Hello", Priority::Normal).unwrap();
    server.queue_tts(1, "World", Priority::High).unwrap();
    assert!(server.verify_invariants());

    server.start_tts(1).unwrap();
    assert!(server.verify_invariants());

    server.pause_tts(1).unwrap();
    assert!(server.verify_invariants());

    server.resume_tts(1).unwrap();
    assert!(server.verify_invariants());

    server.complete_tts(1).unwrap();
    assert!(server.verify_invariants());

    server.client_disconnect(1);
    assert!(server.verify_invariants());
}

// ============================================================================
// Platform Trait Tests
// ============================================================================

#[test]
fn test_null_stt_provider() {
    let mut provider = platform::NullSttProvider;

    assert!(provider.start(AudioFormat::Pcm16k, None).is_err());
    assert!(provider.feed_audio(&[0, 1, 2]).is_err());
    assert!(provider.get_partial().is_none());
    assert!(provider.stop().is_err());
    assert!(provider.is_voice_active().is_none());
    assert!(provider.supported_formats().is_empty());
    assert!(provider.supported_languages().is_empty());
}

#[test]
fn test_null_tts_provider() {
    let mut provider = platform::NullTtsProvider;

    assert!(provider
        .synthesize("test", AudioFormat::Pcm16k, None)
        .is_err());
    assert!(provider
        .start_stream("test", AudioFormat::Pcm16k, None)
        .is_err());
    let mut buf = [0u8; 1024];
    assert!(provider.read_chunk(&mut buf).is_err());
    assert_eq!(provider.estimate_duration("test"), Duration::ZERO);
    assert!(provider.supported_formats().is_empty());
    assert!(provider.available_voices().is_empty());
}

#[test]
fn test_platform_capabilities() {
    let caps = PlatformCapabilities::none();
    assert!(!caps.has_stt());
    assert!(!caps.has_tts());

    let caps = PlatformCapabilities {
        stt_formats: vec![AudioFormat::Pcm16k],
        tts_formats: vec![AudioFormat::Pcm16k, AudioFormat::Opus],
        ..Default::default()
    };
    assert!(caps.has_stt());
    assert!(caps.has_tts());
    assert!(caps.supports_stt_format(AudioFormat::Pcm16k));
    assert!(!caps.supports_stt_format(AudioFormat::Opus));
    assert!(caps.supports_tts_format(AudioFormat::Opus));
}

// ============================================================================
// Priority Tests
// ============================================================================

#[test]
fn test_priority_ordering() {
    assert!(Priority::Low < Priority::Normal);
    assert!(Priority::Normal < Priority::High);
    assert!(Priority::High < Priority::Urgent);
}

#[test]
fn test_priority_from_level() {
    assert_eq!(Priority::from_level(0), Priority::Low);
    assert_eq!(Priority::from_level(1), Priority::Low);
    assert_eq!(Priority::from_level(5), Priority::Normal);
    assert_eq!(Priority::from_level(8), Priority::High);
    assert_eq!(Priority::from_level(10), Priority::Urgent);
    assert_eq!(Priority::from_level(11), Priority::Urgent);
}

// ============================================================================
// Audio Format Tests
// ============================================================================

#[test]
fn test_audio_format_default() {
    assert_eq!(AudioFormat::default(), AudioFormat::Pcm16k);
}

// ============================================================================
// Audio Input Integration Tests
// ============================================================================

#[test]
fn test_media_server_no_audio_input_by_default() {
    let server = MediaServer::new(MediaServerConfig::default());
    assert!(!server.has_audio_input());
    assert!(!server.is_capturing_audio());
}

#[test]
fn test_media_server_start_stt_without_audio_input() {
    let mut server = MediaServer::new(MediaServerConfig::default());

    // Should fail because no audio input provider is configured
    let result = server.start_stt_with_microphone(1, AudioFormat::Pcm16k, None);
    assert!(result.is_err());
    match result.unwrap_err() {
        MediaServerError::Provider(msg) => {
            assert!(msg.contains("No audio input provider"));
        }
        e => panic!("Unexpected error type: {:?}", e),
    }
}

#[test]
fn test_media_server_process_audio_when_not_listening() {
    let mut server = MediaServer::new(MediaServerConfig::default());

    // Should fail because no STT session is active
    let result = server.process_audio();
    assert!(result.is_err());
}

#[test]
fn test_media_server_cancel_stt_with_microphone() {
    let mut server = MediaServer::new(MediaServerConfig::default());

    // Should return None when no session is active
    let result = server.cancel_stt_with_microphone();
    assert!(result.is_none());
}

#[test]
fn test_media_server_with_audio_input() {
    use super::platform::NullAudioInputProvider;

    let mut server = MediaServer::new(MediaServerConfig::default());
    assert!(!server.has_audio_input());

    server.set_audio_input(Box::new(NullAudioInputProvider));
    assert!(server.has_audio_input());
}

#[test]
fn test_media_server_with_all_providers() {
    use super::platform::{NullAudioInputProvider, NullSttProvider, NullTtsProvider};

    let server = MediaServer::with_all_providers(
        MediaServerConfig::default(),
        Box::new(NullSttProvider),
        Box::new(NullTtsProvider),
        Box::new(NullAudioInputProvider),
    );

    assert!(server.has_audio_input());
    assert!(server.verify_invariants());
}

#[test]
fn test_media_server_is_voice_active() {
    let server = MediaServer::new(MediaServerConfig::default());
    // Null provider returns None for VAD
    assert!(server.is_voice_active().is_none());
}

// ============================================================================
// Mock Providers for End-to-End Testing
// ============================================================================

/// Mock STT provider that returns predefined results.
struct MockSttProvider {
    partial_text: Option<String>,
    final_text: Option<String>,
    started: bool,
    audio_received: usize,
    supported_formats: Vec<AudioFormat>,
    supported_languages: Vec<String>,
}

impl MockSttProvider {
    fn new(partial: Option<&str>, final_text: Option<&str>) -> Self {
        Self {
            partial_text: partial.map(String::from),
            final_text: final_text.map(String::from),
            started: false,
            audio_received: 0,
            supported_formats: vec![AudioFormat::Pcm16k],
            supported_languages: vec!["en-US".to_string()],
        }
    }
}

impl platform::SttProvider for MockSttProvider {
    fn start(
        &mut self,
        _format: AudioFormat,
        _language: Option<&str>,
    ) -> Result<(), platform::SttProviderError> {
        self.started = true;
        self.audio_received = 0;
        Ok(())
    }

    fn feed_audio(&mut self, data: &[u8]) -> Result<(), platform::SttProviderError> {
        if !self.started {
            return Err(platform::SttProviderError::NotInitialized);
        }
        self.audio_received += data.len();
        Ok(())
    }

    fn get_partial(&mut self) -> Option<stt::SttResult> {
        if self.audio_received > 0 {
            self.partial_text.as_ref().map(|text| stt::SttResult {
                client: 0,
                text: text.clone(),
                confidence: 50,
                is_final: false,
            })
        } else {
            None
        }
    }

    fn stop(&mut self) -> Result<Option<stt::SttResult>, platform::SttProviderError> {
        if !self.started {
            return Err(platform::SttProviderError::NotInitialized);
        }
        self.started = false;
        Ok(self.final_text.as_ref().map(|text| stt::SttResult {
            client: 0,
            text: text.clone(),
            confidence: 95,
            is_final: true,
        }))
    }

    fn cancel(&mut self) {
        self.started = false;
        self.audio_received = 0;
    }

    fn is_voice_active(&self) -> Option<bool> {
        Some(self.audio_received > 0)
    }

    fn supported_formats(&self) -> &[AudioFormat] {
        &self.supported_formats
    }

    fn supported_languages(&self) -> &[String] {
        &self.supported_languages
    }
}

/// Mock audio input provider that generates fake audio data.
struct MockAudioInputProvider {
    capturing: bool,
    callback: Option<platform::AudioDataCallback>,
}

impl MockAudioInputProvider {
    fn new() -> Self {
        Self {
            capturing: false,
            callback: None,
        }
    }
}

impl platform::AudioInputProvider for MockAudioInputProvider {
    fn available_devices(&self) -> Vec<platform::AudioInputDevice> {
        vec![platform::AudioInputDevice {
            id: "mock-device".to_string(),
            name: "Mock Microphone".to_string(),
            is_default: true,
            supported_sample_rates: vec![16000, 44100],
        }]
    }

    fn default_device(&self) -> Option<platform::AudioInputDevice> {
        Some(platform::AudioInputDevice {
            id: "mock-device".to_string(),
            name: "Mock Microphone".to_string(),
            is_default: true,
            supported_sample_rates: vec![16000, 44100],
        })
    }

    fn start(
        &mut self,
        _format: AudioFormat,
        _device: Option<&str>,
        callback: platform::AudioDataCallback,
    ) -> Result<(), platform::AudioInputError> {
        self.capturing = true;
        self.callback = Some(callback);
        Ok(())
    }

    fn stop(&mut self) {
        self.capturing = false;
        self.callback = None;
    }

    fn is_capturing(&self) -> bool {
        self.capturing
    }

    fn supported_formats(&self) -> &[AudioFormat] {
        &[AudioFormat::Pcm16k, AudioFormat::Pcm44k]
    }
}

// ============================================================================
// End-to-End Voice Flow Tests (with Mock Providers)
// ============================================================================

#[test]
fn test_end_to_end_voice_flow_with_mocks() {
    // Create mock providers
    let stt_provider = MockSttProvider::new(
        Some("hello"),       // partial result
        Some("hello world"), // final result
    );
    let tts_provider = platform::NullTtsProvider;
    let audio_input = MockAudioInputProvider::new();

    // Create media server with mock providers
    let mut server = MediaServer::with_all_providers(
        MediaServerConfig::default(),
        Box::new(stt_provider),
        Box::new(tts_provider),
        Box::new(audio_input),
    );

    // Start STT with microphone
    let client_id = 1;
    let stream_id = server
        .start_stt_with_microphone(client_id, AudioFormat::Pcm16k, Some("en-US"))
        .unwrap();
    assert_eq!(server.stt_state(), SttState::Listening);
    assert!(server.is_capturing_audio());

    // Verify stream was created
    let stream = server.stream(stream_id).unwrap();
    assert_eq!(stream.direction(), StreamDirection::Input);

    // Verify invariants hold
    assert!(server.verify_invariants());
}

#[test]
fn test_media_server_stop_stt_with_microphone_empty() {
    // Create server with mock providers but no audio to process
    let stt_provider = MockSttProvider::new(None, None);
    let tts_provider = platform::NullTtsProvider;
    let audio_input = MockAudioInputProvider::new();

    let mut server = MediaServer::with_all_providers(
        MediaServerConfig::default(),
        Box::new(stt_provider),
        Box::new(tts_provider),
        Box::new(audio_input),
    );

    // Start and immediately stop
    server
        .start_stt_with_microphone(1, AudioFormat::Pcm16k, None)
        .unwrap();
    let result = server.stop_stt_with_microphone().unwrap();

    // No audio was captured, so no result
    assert!(result.is_none());
    assert_eq!(server.stt_state(), SttState::Idle);
    assert!(!server.is_capturing_audio());
}

#[test]
fn test_media_server_capabilities_with_providers() {
    let stt_provider = MockSttProvider::new(None, None);
    let tts_provider = platform::NullTtsProvider;
    let audio_input = MockAudioInputProvider::new();

    let server = MediaServer::with_all_providers(
        MediaServerConfig::default(),
        Box::new(stt_provider),
        Box::new(tts_provider),
        Box::new(audio_input),
    );

    let caps = server.capabilities();
    assert!(caps.has_stt());
    assert!(!caps.has_tts()); // Null TTS provider
    assert!(caps.supports_stt_format(AudioFormat::Pcm16k));
    assert_eq!(caps.stt_languages, vec!["en-US"]);
    assert!(caps.supports_vad); // Mock provider supports VAD
}
