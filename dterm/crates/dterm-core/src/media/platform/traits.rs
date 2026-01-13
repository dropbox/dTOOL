//! Platform abstraction traits for voice I/O.
//!
//! These traits define the interface between the media server and
//! platform-specific voice APIs.
//!
//! ## Platform Support
//!
//! | Platform | STT Provider | TTS Provider |
//! |----------|--------------|--------------|
//! | macOS/iOS | Speech framework | AVSpeechSynthesizer |
//! | Windows | Windows.Media.SpeechRecognition | Windows.Media.SpeechSynthesis |
//! | Linux | Vosk, Whisper.cpp | Festival, espeak |
//! | Web | Web Speech API | Web Speech API |

use crate::media::{stt::SttResult, AudioFormat};
use std::time::Duration;

/// Capabilities of a platform's voice support.
#[derive(Debug, Clone, Default)]
pub struct PlatformCapabilities {
    /// Supported STT audio formats.
    pub stt_formats: Vec<AudioFormat>,
    /// Supported TTS audio formats.
    pub tts_formats: Vec<AudioFormat>,
    /// Whether continuous/streaming STT is supported.
    pub supports_continuous_stt: bool,
    /// Whether voice activity detection is available.
    pub supports_vad: bool,
    /// Whether offline STT is available.
    pub supports_offline_stt: bool,
    /// Whether offline TTS is available.
    pub supports_offline_tts: bool,
    /// Available STT languages.
    pub stt_languages: Vec<String>,
    /// Available TTS voices.
    pub tts_voices: Vec<VoiceInfo>,
}

impl PlatformCapabilities {
    /// Create capabilities for a platform with no voice support.
    pub fn none() -> Self {
        Self::default()
    }

    /// Check if STT is available.
    pub fn has_stt(&self) -> bool {
        !self.stt_formats.is_empty()
    }

    /// Check if TTS is available.
    pub fn has_tts(&self) -> bool {
        !self.tts_formats.is_empty()
    }

    /// Check if a specific STT format is supported.
    pub fn supports_stt_format(&self, format: AudioFormat) -> bool {
        self.stt_formats.contains(&format)
    }

    /// Check if a specific TTS format is supported.
    pub fn supports_tts_format(&self, format: AudioFormat) -> bool {
        self.tts_formats.contains(&format)
    }
}

/// Information about a TTS voice.
#[derive(Debug, Clone)]
pub struct VoiceInfo {
    /// Unique voice identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Language code (e.g., "en-US").
    pub language: String,
    /// Voice gender (if known).
    pub gender: Option<VoiceGender>,
    /// Voice quality level.
    pub quality: VoiceQuality,
}

/// Voice gender.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceGender {
    /// Male voice.
    Male,
    /// Female voice.
    Female,
    /// Non-binary/neutral voice.
    Neutral,
}

/// Voice quality level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VoiceQuality {
    /// Basic quality (fast, small).
    Basic,
    /// Standard quality.
    #[default]
    Standard,
    /// High quality (neural/enhanced).
    High,
    /// Premium quality (best available).
    Premium,
}

/// Speech-to-text provider trait.
///
/// Implement this trait for platform-specific STT backends.
pub trait SttProvider: Send + Sync {
    /// Start a new recognition session.
    ///
    /// # Errors
    ///
    /// Returns error if initialization fails.
    fn start(
        &mut self,
        format: AudioFormat,
        language: Option<&str>,
    ) -> Result<(), SttProviderError>;

    /// Feed audio data for recognition.
    ///
    /// # Errors
    ///
    /// Returns error if processing fails.
    fn feed_audio(&mut self, data: &[u8]) -> Result<(), SttProviderError>;

    /// Get available partial results.
    ///
    /// Returns partial recognition results if available.
    fn get_partial(&mut self) -> Option<SttResult>;

    /// Stop the session and get final result.
    ///
    /// # Errors
    ///
    /// Returns error if finalization fails.
    fn stop(&mut self) -> Result<Option<SttResult>, SttProviderError>;

    /// Cancel the session without getting results.
    fn cancel(&mut self);

    /// Check if voice activity is detected.
    ///
    /// Returns `None` if VAD is not supported.
    fn is_voice_active(&self) -> Option<bool>;

    /// Get supported audio formats.
    fn supported_formats(&self) -> &[AudioFormat];

    /// Get supported languages.
    fn supported_languages(&self) -> &[String];
}

/// Text-to-speech provider trait.
///
/// Implement this trait for platform-specific TTS backends.
pub trait TtsProvider: Send + Sync {
    /// Synthesize speech from text.
    ///
    /// Returns the audio data in the specified format.
    ///
    /// # Errors
    ///
    /// Returns error if synthesis fails.
    fn synthesize(
        &mut self,
        text: &str,
        format: AudioFormat,
        voice: Option<&str>,
    ) -> Result<Vec<u8>, TtsProviderError>;

    /// Start streaming synthesis.
    ///
    /// Use `read_chunk` to get audio data incrementally.
    ///
    /// # Errors
    ///
    /// Returns error if initialization fails.
    fn start_stream(
        &mut self,
        text: &str,
        format: AudioFormat,
        voice: Option<&str>,
    ) -> Result<(), TtsProviderError>;

    /// Read the next chunk of audio data.
    ///
    /// Returns `None` when synthesis is complete.
    ///
    /// # Errors
    ///
    /// Returns error if reading fails.
    fn read_chunk(&mut self, buffer: &mut [u8]) -> Result<Option<usize>, TtsProviderError>;

    /// Stop streaming and discard remaining audio.
    fn stop_stream(&mut self);

    /// Get estimated duration for text.
    fn estimate_duration(&self, text: &str) -> Duration;

    /// Get supported audio formats.
    fn supported_formats(&self) -> &[AudioFormat];

    /// Get available voices.
    fn available_voices(&self) -> &[VoiceInfo];

    /// Set speech rate (0.5 = half speed, 2.0 = double speed).
    fn set_rate(&mut self, rate: f32);

    /// Set pitch adjustment (0.5 = lower, 2.0 = higher).
    fn set_pitch(&mut self, pitch: f32);

    /// Set volume (0.0 = silent, 1.0 = full).
    fn set_volume(&mut self, volume: f32);
}

/// STT provider error.
#[derive(Debug, Clone)]
pub enum SttProviderError {
    /// Provider not initialized.
    NotInitialized,
    /// Audio format not supported.
    UnsupportedFormat(AudioFormat),
    /// Language not supported.
    UnsupportedLanguage(String),
    /// Recognition failed.
    RecognitionFailed(String),
    /// Provider-specific error.
    Provider(String),
}

impl std::fmt::Display for SttProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInitialized => write!(f, "STT provider not initialized"),
            Self::UnsupportedFormat(fmt) => write!(f, "Unsupported audio format: {:?}", fmt),
            Self::UnsupportedLanguage(lang) => write!(f, "Unsupported language: {}", lang),
            Self::RecognitionFailed(msg) => write!(f, "Recognition failed: {}", msg),
            Self::Provider(msg) => write!(f, "STT provider error: {}", msg),
        }
    }
}

impl std::error::Error for SttProviderError {}

/// TTS provider error.
#[derive(Debug, Clone)]
pub enum TtsProviderError {
    /// Provider not initialized.
    NotInitialized,
    /// Audio format not supported.
    UnsupportedFormat(AudioFormat),
    /// Voice not found.
    VoiceNotFound(String),
    /// Synthesis failed.
    SynthesisFailed(String),
    /// Provider-specific error.
    Provider(String),
}

impl std::fmt::Display for TtsProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInitialized => write!(f, "TTS provider not initialized"),
            Self::UnsupportedFormat(fmt) => write!(f, "Unsupported audio format: {:?}", fmt),
            Self::VoiceNotFound(voice) => write!(f, "Voice not found: {}", voice),
            Self::SynthesisFailed(msg) => write!(f, "Synthesis failed: {}", msg),
            Self::Provider(msg) => write!(f, "TTS provider error: {}", msg),
        }
    }
}

impl std::error::Error for TtsProviderError {}

/// Null STT provider (no-op implementation).
///
/// Use when no platform STT is available.
#[derive(Debug, Default)]
pub struct NullSttProvider;

impl SttProvider for NullSttProvider {
    fn start(
        &mut self,
        _format: AudioFormat,
        _language: Option<&str>,
    ) -> Result<(), SttProviderError> {
        Err(SttProviderError::NotInitialized)
    }

    fn feed_audio(&mut self, _data: &[u8]) -> Result<(), SttProviderError> {
        Err(SttProviderError::NotInitialized)
    }

    fn get_partial(&mut self) -> Option<SttResult> {
        None
    }

    fn stop(&mut self) -> Result<Option<SttResult>, SttProviderError> {
        Err(SttProviderError::NotInitialized)
    }

    fn cancel(&mut self) {}

    fn is_voice_active(&self) -> Option<bool> {
        None
    }

    fn supported_formats(&self) -> &[AudioFormat] {
        &[]
    }

    fn supported_languages(&self) -> &[String] {
        &[]
    }
}

// ============================================================================
// Audio Input Provider
// ============================================================================

/// Information about an audio input device.
#[derive(Debug, Clone)]
pub struct AudioInputDevice {
    /// Unique device identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Whether this is the default input device.
    pub is_default: bool,
    /// Sample rates supported by this device.
    pub supported_sample_rates: Vec<u32>,
}

/// Callback type for receiving audio data.
pub type AudioDataCallback = Box<dyn FnMut(&[u8]) + Send>;

/// Audio input provider trait.
///
/// Implement this trait for platform-specific audio capture backends.
/// The provider captures audio from a microphone and delivers it via callback.
pub trait AudioInputProvider: Send {
    /// Get available input devices.
    fn available_devices(&self) -> Vec<AudioInputDevice>;

    /// Get the default input device.
    fn default_device(&self) -> Option<AudioInputDevice>;

    /// Start capturing audio.
    ///
    /// Audio data will be delivered to the callback as it becomes available.
    /// The callback receives PCM audio data in the specified format.
    ///
    /// # Arguments
    ///
    /// * `format` - The audio format to capture
    /// * `device` - Optional device ID (uses default if None)
    /// * `callback` - Called with audio data chunks
    ///
    /// # Errors
    ///
    /// Returns error if capture cannot be started.
    fn start(
        &mut self,
        format: AudioFormat,
        device: Option<&str>,
        callback: AudioDataCallback,
    ) -> Result<(), AudioInputError>;

    /// Stop capturing audio.
    fn stop(&mut self);

    /// Check if currently capturing.
    fn is_capturing(&self) -> bool;

    /// Get supported audio formats.
    fn supported_formats(&self) -> &[AudioFormat];
}

/// Audio input error.
#[derive(Debug, Clone)]
pub enum AudioInputError {
    /// No input device available.
    NoDevice,
    /// Device not found.
    DeviceNotFound(String),
    /// Audio format not supported.
    UnsupportedFormat(AudioFormat),
    /// Permission denied (microphone access).
    PermissionDenied,
    /// Already capturing.
    AlreadyCapturing,
    /// Not capturing.
    NotCapturing,
    /// Provider-specific error.
    Provider(String),
}

impl std::fmt::Display for AudioInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoDevice => write!(f, "No audio input device available"),
            Self::DeviceNotFound(id) => write!(f, "Audio input device not found: {}", id),
            Self::UnsupportedFormat(fmt) => write!(f, "Unsupported audio format: {:?}", fmt),
            Self::PermissionDenied => write!(f, "Microphone permission denied"),
            Self::AlreadyCapturing => write!(f, "Already capturing audio"),
            Self::NotCapturing => write!(f, "Not currently capturing audio"),
            Self::Provider(msg) => write!(f, "Audio input error: {}", msg),
        }
    }
}

impl std::error::Error for AudioInputError {}

/// Null audio input provider (no-op implementation).
///
/// Use when no platform audio input is available.
#[derive(Debug, Default)]
pub struct NullAudioInputProvider;

impl AudioInputProvider for NullAudioInputProvider {
    fn available_devices(&self) -> Vec<AudioInputDevice> {
        Vec::new()
    }

    fn default_device(&self) -> Option<AudioInputDevice> {
        None
    }

    fn start(
        &mut self,
        _format: AudioFormat,
        _device: Option<&str>,
        _callback: AudioDataCallback,
    ) -> Result<(), AudioInputError> {
        Err(AudioInputError::NoDevice)
    }

    fn stop(&mut self) {}

    fn is_capturing(&self) -> bool {
        false
    }

    fn supported_formats(&self) -> &[AudioFormat] {
        &[]
    }
}

/// Null TTS provider (no-op implementation).
///
/// Use when no platform TTS is available.
#[derive(Debug, Default)]
pub struct NullTtsProvider;

impl TtsProvider for NullTtsProvider {
    fn synthesize(
        &mut self,
        _text: &str,
        _format: AudioFormat,
        _voice: Option<&str>,
    ) -> Result<Vec<u8>, TtsProviderError> {
        Err(TtsProviderError::NotInitialized)
    }

    fn start_stream(
        &mut self,
        _text: &str,
        _format: AudioFormat,
        _voice: Option<&str>,
    ) -> Result<(), TtsProviderError> {
        Err(TtsProviderError::NotInitialized)
    }

    fn read_chunk(&mut self, _buffer: &mut [u8]) -> Result<Option<usize>, TtsProviderError> {
        Err(TtsProviderError::NotInitialized)
    }

    fn stop_stream(&mut self) {}

    fn estimate_duration(&self, _text: &str) -> Duration {
        Duration::ZERO
    }

    fn supported_formats(&self) -> &[AudioFormat] {
        &[]
    }

    fn available_voices(&self) -> &[VoiceInfo] {
        &[]
    }

    fn set_rate(&mut self, _rate: f32) {}
    fn set_pitch(&mut self, _pitch: f32) {}
    fn set_volume(&mut self, _volume: f32) {}
}
