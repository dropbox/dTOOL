//! # Media Server Module
//!
//! Voice I/O protocol for AI agent interactions.
//!
//! ## TLA+ Specification
//!
//! This module implements the state machine defined in `tla/MediaServer.tla`.
//!
//! ## Architecture
//!
//! ```text
//! User speaks → Local STT → Agent intent → Execution → TTS response
//!
//! ┌────────────────────────────────────────────────────────────────┐
//! │                      MediaServer                                │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐     │
//! │  │ STT Session │  │ TTS Queue   │  │   Audio Streams     │     │
//! │  │             │  │             │  │                     │     │
//! │  │  Idle       │  │  Client 1   │  │   Stream 1 (in)     │     │
//! │  │  Listening  │  │  Client 2   │  │   Stream 2 (out)    │     │
//! │  │  Processing │  │  ...        │  │   ...               │     │
//! │  └─────────────┘  └─────────────┘  └─────────────────────┘     │
//! └────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Safety Invariants (from TLA+ spec)
//!
//! - **INV-MEDIA-1**: At most one STT session active at a time
//! - **INV-MEDIA-2**: TTS queue depth bounded per client
//! - **INV-MEDIA-3**: Active streams have valid clients
//! - **INV-MEDIA-4**: Latency within bounds (soft constraint)
//! - **INV-MEDIA-5**: No orphaned processing state
//! - **INV-MEDIA-6**: Speaking client has TTS state
//! - **INV-MEDIA-7**: Idle STT has no active client
//!
//! ## Platform APIs
//!
//! - **macOS/iOS**: Speech framework (NSSpeechRecognizer, AVSpeechSynthesizer)
//! - **Windows**: Windows.Media.SpeechRecognition
//! - **Linux**: Vosk, Whisper.cpp
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dterm_core::media::{MediaServer, MediaServerConfig, AudioFormat};
//!
//! // Create media server with configuration
//! let config = MediaServerConfig {
//!     max_tts_queue_depth: 10,
//!     max_stream_duration_ms: 30_000,
//!     max_latency_ms: 100,
//! };
//! let mut server = MediaServer::new(config);
//!
//! // Start STT session for a client
//! server.start_stt(client_id, AudioFormat::Pcm16k)?;
//!
//! // Queue TTS utterance
//! server.queue_tts(client_id, "Hello, how can I help?", Priority::Normal)?;
//! ```

mod platform;
mod server;
mod stream;
mod stt;
mod tts;

#[cfg(feature = "ffi")]
pub mod ffi;

pub use platform::{
    create_audio_input_provider, create_stt_provider, create_tts_provider,
    get_platform_capabilities, has_voice_support, platform_name, AudioDataCallback,
    AudioInputDevice, AudioInputError, AudioInputProvider, NullAudioInputProvider, NullSttProvider,
    NullTtsProvider, PlatformCapabilities, SttProvider, SttProviderError, TtsProvider,
    TtsProviderError, VoiceGender, VoiceInfo, VoiceQuality,
};
pub use server::{ClientId, MediaServer, MediaServerConfig, MediaServerError, MediaServerResult};
pub use stream::{AudioStream, StreamDirection, StreamId, StreamState};
pub use stt::{SttResult, SttSession, SttState};
pub use tts::{Priority, TtsQueue, TtsState, TtsUtterance};

// Re-export platform-specific modules for advanced usage
#[cfg(all(
    any(target_os = "macos", target_os = "ios"),
    any(feature = "macos-speech", feature = "ios-speech")
))]
pub use platform::macos_stt_native;

#[cfg(all(
    any(target_os = "macos", target_os = "ios"),
    any(feature = "macos-speech", feature = "ios-speech")
))]
pub use platform::macos_audio_input;

/// Audio format for voice I/O.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioFormat {
    /// 16-bit PCM at 16kHz (common for STT)
    Pcm16k,
    /// 16-bit PCM at 44.1kHz (high quality)
    Pcm44k,
    /// Opus codec (efficient for streaming)
    Opus,
    /// AAC codec (Apple ecosystem)
    Aac,
}

impl Default for AudioFormat {
    fn default() -> Self {
        Self::Pcm16k
    }
}

#[cfg(test)]
mod tests;
