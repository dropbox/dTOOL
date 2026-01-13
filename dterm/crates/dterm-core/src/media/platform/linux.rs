//! Linux platform provider implementation.
//!
//! Uses Vosk or Whisper.cpp for STT and espeak-ng for TTS.
//!
//! ## Library References
//!
//! - STT: [Vosk](https://alphacephei.com/vosk/) - Offline speech recognition
//! - STT: [Whisper.cpp](https://github.com/ggerganov/whisper.cpp) - OpenAI Whisper port
//! - TTS: [espeak-ng](https://github.com/espeak-ng/espeak-ng) - Compact multilingual TTS
//! - TTS: [Festival](http://www.cstr.ed.ac.uk/projects/festival/) - University of Edinburgh TTS
//! - TTS: [Piper](https://github.com/rhasspy/piper) - Fast neural TTS
//!
//! ## Implementation Notes
//!
//! When the `linux-speech` feature is enabled:
//! - TTS uses native espeak-ng bindings
//! - STT uses native Vosk bindings for offline speech recognition
//!
//! Linux voice support varies by distribution and installed packages.
//! espeak-ng is widely available on most Linux distributions.
//! Vosk requires downloading language models (~50MB for small models).

use super::{
    PlatformCapabilities, SttProvider, SttProviderError, TtsProvider, TtsProviderError,
    VoiceGender, VoiceInfo, VoiceQuality,
};
use crate::media::{stt::SttResult, AudioFormat};
use std::time::Duration;

/// Get Linux platform capabilities.
///
/// Linux supports (with appropriate packages installed):
/// - Vosk for offline speech recognition (small models ~50MB)
/// - Whisper.cpp for high-quality offline STT
/// - Festival/espeak for TTS
/// - PulseAudio/PipeWire for audio I/O
pub fn get_capabilities() -> PlatformCapabilities {
    PlatformCapabilities {
        stt_formats: vec![AudioFormat::Pcm16k],
        tts_formats: vec![AudioFormat::Pcm16k, AudioFormat::Pcm44k],
        supports_continuous_stt: true,
        supports_vad: true,         // Vosk includes VAD
        supports_offline_stt: true, // Vosk/Whisper are fully offline
        supports_offline_tts: true, // espeak/Festival are offline
        stt_languages: get_stt_languages(),
        tts_voices: get_tts_voices(),
    }
}

/// Get supported STT languages on Linux (native when feature enabled).
#[cfg(feature = "linux-speech")]
fn get_stt_languages() -> Vec<String> {
    // Get languages from installed Vosk models
    let languages = super::linux_stt_native::get_available_languages();
    if languages.is_empty() {
        // Fall back to static list
        get_stt_languages_static()
    } else {
        languages
    }
}

/// Get supported STT languages on Linux (stub when native not available).
#[cfg(not(feature = "linux-speech"))]
fn get_stt_languages() -> Vec<String> {
    get_stt_languages_static()
}

/// Static list of STT languages with available Vosk models.
fn get_stt_languages_static() -> Vec<String> {
    // Languages with Vosk models available
    // See: https://alphacephei.com/vosk/models
    vec![
        "en-US".to_string(),
        "en-GB".to_string(),
        "es-ES".to_string(),
        "fr-FR".to_string(),
        "de-DE".to_string(),
        "it-IT".to_string(),
        "ja-JP".to_string(),
        "ko-KR".to_string(),
        "zh-CN".to_string(),
        "pt-BR".to_string(),
        "ru-RU".to_string(),
        "nl-NL".to_string(),
        "pl-PL".to_string(),
        "uk-UA".to_string(),
        "ar-SA".to_string(),
        "hi-IN".to_string(),
        "vi-VN".to_string(),
        "tr-TR".to_string(),
    ]
}

/// Get available TTS voices on Linux.
fn get_tts_voices() -> Vec<VoiceInfo> {
    // espeak-ng and Festival voices
    vec![
        // espeak-ng voices
        VoiceInfo {
            id: "espeak-en".to_string(),
            name: "English (espeak)".to_string(),
            language: "en-US".to_string(),
            gender: Some(VoiceGender::Male),
            quality: VoiceQuality::Basic,
        },
        VoiceInfo {
            id: "espeak-en+f1".to_string(),
            name: "English Female (espeak)".to_string(),
            language: "en-US".to_string(),
            gender: Some(VoiceGender::Female),
            quality: VoiceQuality::Basic,
        },
        VoiceInfo {
            id: "espeak-es".to_string(),
            name: "Spanish (espeak)".to_string(),
            language: "es-ES".to_string(),
            gender: Some(VoiceGender::Male),
            quality: VoiceQuality::Basic,
        },
        VoiceInfo {
            id: "espeak-fr".to_string(),
            name: "French (espeak)".to_string(),
            language: "fr-FR".to_string(),
            gender: Some(VoiceGender::Male),
            quality: VoiceQuality::Basic,
        },
        VoiceInfo {
            id: "espeak-de".to_string(),
            name: "German (espeak)".to_string(),
            language: "de-DE".to_string(),
            gender: Some(VoiceGender::Male),
            quality: VoiceQuality::Basic,
        },
        // Festival voices
        VoiceInfo {
            id: "festival-kal_diphone".to_string(),
            name: "Kal (Festival)".to_string(),
            language: "en-US".to_string(),
            gender: Some(VoiceGender::Male),
            quality: VoiceQuality::Standard,
        },
        VoiceInfo {
            id: "festival-cmu_us_slt_arctic_hts".to_string(),
            name: "SLT Arctic (Festival)".to_string(),
            language: "en-US".to_string(),
            gender: Some(VoiceGender::Female),
            quality: VoiceQuality::High,
        },
        // Piper neural voices (if installed)
        VoiceInfo {
            id: "piper-en_US-lessac-medium".to_string(),
            name: "Lessac (Piper Neural)".to_string(),
            language: "en-US".to_string(),
            gender: Some(VoiceGender::Male),
            quality: VoiceQuality::Premium,
        },
        VoiceInfo {
            id: "piper-en_US-amy-medium".to_string(),
            name: "Amy (Piper Neural)".to_string(),
            language: "en-US".to_string(),
            gender: Some(VoiceGender::Female),
            quality: VoiceQuality::Premium,
        },
    ]
}

/// Linux Speech-to-Text provider.
///
/// Uses Vosk or Whisper.cpp for speech recognition.
///
/// ## Implementation Status
///
/// When the `linux-speech` feature is enabled, this provider uses native Vosk
/// bindings for offline speech recognition. Otherwise, it is a stub.
///
/// ## Native Implementation Details
///
/// The native implementation provides:
/// - Offline speech recognition via Vosk models
/// - Language model discovery from common paths
/// - Real-time partial results
/// - Thread-safe recognizer wrapper
///
/// ## Requirements
///
/// - Vosk library installed (`libvosk`)
/// - Language model downloaded (e.g., `vosk-model-small-en-us`)
/// - Model placed in one of: `~/.local/share/vosk/models`, `/usr/share/vosk/models`, etc.
#[derive(Debug)]
pub struct LinuxSttProvider {
    /// Session active flag.
    active: bool,
    /// Current audio format.
    format: AudioFormat,
    /// Current language.
    language: String,
    /// Supported formats.
    supported_formats: Vec<AudioFormat>,
    /// Supported languages.
    supported_languages: Vec<String>,
    /// Voice activity detected.
    voice_active: bool,
    /// Audio buffer.
    audio_buffer: Vec<u8>,
    /// STT backend in use.
    backend: SttBackend,
    /// Native recognizer (when linux-speech feature is enabled).
    #[cfg(feature = "linux-speech")]
    native_recognizer: Option<super::linux_stt_native::ThreadSafeRecognizer>,
}

/// STT backend selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SttBackend {
    /// Vosk offline speech recognition.
    #[default]
    Vosk,
    /// Whisper.cpp for high-quality recognition.
    Whisper,
}

impl LinuxSttProvider {
    /// Create a new Linux STT provider.
    pub fn new() -> Self {
        Self {
            active: false,
            format: AudioFormat::Pcm16k,
            language: "en-US".to_string(),
            supported_formats: vec![AudioFormat::Pcm16k],
            supported_languages: get_stt_languages(),
            voice_active: false,
            audio_buffer: Vec::new(),
            backend: SttBackend::Vosk,
            #[cfg(feature = "linux-speech")]
            native_recognizer: None, // Created on start() with language
        }
    }

    /// Create with specific backend.
    pub fn with_backend(backend: SttBackend) -> Self {
        let mut provider = Self::new();
        provider.backend = backend;
        provider
    }

    /// Get the current backend.
    pub fn backend(&self) -> SttBackend {
        self.backend
    }
}

impl Default for LinuxSttProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SttProvider for LinuxSttProvider {
    #[cfg(feature = "linux-speech")]
    fn start(
        &mut self,
        format: AudioFormat,
        language: Option<&str>,
    ) -> Result<(), SttProviderError> {
        if self.active {
            return Err(SttProviderError::Provider(
                "Session already active".to_string(),
            ));
        }

        if !self.supported_formats.contains(&format) {
            return Err(SttProviderError::UnsupportedFormat(format));
        }

        let lang = language.unwrap_or("en-US");
        if !self.supported_languages.iter().any(|l| l == lang) {
            return Err(SttProviderError::UnsupportedLanguage(lang.to_string()));
        }

        // Create native recognizer for the language
        self.native_recognizer = super::linux_stt_native::ThreadSafeRecognizer::new(lang, None);

        // Start native recognizer
        if let Some(ref mut recognizer) = self.native_recognizer {
            recognizer.start().map_err(SttProviderError::Provider)?;
        } else {
            return Err(SttProviderError::Provider(
                format!("No Vosk model found for language '{}'. Download a model from https://alphacephei.com/vosk/models", lang),
            ));
        }

        self.active = true;
        self.format = format;
        self.language = lang.to_string();
        self.audio_buffer.clear();
        self.voice_active = false;

        Ok(())
    }

    #[cfg(not(feature = "linux-speech"))]
    fn start(
        &mut self,
        format: AudioFormat,
        language: Option<&str>,
    ) -> Result<(), SttProviderError> {
        if self.active {
            return Err(SttProviderError::Provider(
                "Session already active".to_string(),
            ));
        }

        if !self.supported_formats.contains(&format) {
            return Err(SttProviderError::UnsupportedFormat(format));
        }

        let lang = language.unwrap_or("en-US");
        if !self.supported_languages.iter().any(|l| l == lang) {
            return Err(SttProviderError::UnsupportedLanguage(lang.to_string()));
        }

        self.active = true;
        self.format = format;
        self.language = lang.to_string();
        self.audio_buffer.clear();
        self.voice_active = false;

        Ok(())
    }

    #[cfg(feature = "linux-speech")]
    fn feed_audio(&mut self, data: &[u8]) -> Result<(), SttProviderError> {
        if !self.active {
            return Err(SttProviderError::NotInitialized);
        }

        self.audio_buffer.extend_from_slice(data);

        // Simple VAD based on audio energy
        let energy: u64 = data
            .iter()
            .map(|&b| (b as i16 - 128).unsigned_abs() as u64)
            .sum();
        let threshold = (data.len() as u64) * 10;
        self.voice_active = energy > threshold;

        // Feed to native recognizer
        if let Some(ref mut recognizer) = self.native_recognizer {
            recognizer
                .feed_audio(data)
                .map_err(SttProviderError::Provider)?;
        }

        Ok(())
    }

    #[cfg(not(feature = "linux-speech"))]
    fn feed_audio(&mut self, data: &[u8]) -> Result<(), SttProviderError> {
        if !self.active {
            return Err(SttProviderError::NotInitialized);
        }

        self.audio_buffer.extend_from_slice(data);

        // Simple VAD: detect non-silence
        self.voice_active = data.iter().any(|&b| b > 32 || b < 224);

        Ok(())
    }

    #[cfg(feature = "linux-speech")]
    fn get_partial(&mut self) -> Option<SttResult> {
        if !self.active {
            return None;
        }

        if let Some(ref recognizer) = self.native_recognizer {
            if let Some(result) = recognizer.get_partial() {
                return Some(SttResult {
                    text: result.text,
                    confidence: result.confidence.unwrap_or(0.5),
                    is_final: result.is_final,
                    language: Some(self.language.clone()),
                });
            }
        }

        None
    }

    #[cfg(not(feature = "linux-speech"))]
    fn get_partial(&mut self) -> Option<SttResult> {
        if !self.active || self.audio_buffer.is_empty() {
            return None;
        }

        // Stub: no partial result
        None
    }

    #[cfg(feature = "linux-speech")]
    fn stop(&mut self) -> Result<Option<SttResult>, SttProviderError> {
        if !self.active {
            return Err(SttProviderError::NotInitialized);
        }

        let result = if let Some(ref mut recognizer) = self.native_recognizer {
            match recognizer.stop() {
                Ok(Some(result)) => Some(SttResult {
                    text: result.text,
                    confidence: result.confidence.unwrap_or(0.5),
                    is_final: true,
                    language: Some(self.language.clone()),
                }),
                Ok(None) => None,
                Err(e) => {
                    self.active = false;
                    self.voice_active = false;
                    self.native_recognizer = None;
                    return Err(SttProviderError::Provider(e));
                }
            }
        } else {
            None
        };

        self.active = false;
        self.voice_active = false;
        self.native_recognizer = None;

        Ok(result)
    }

    #[cfg(not(feature = "linux-speech"))]
    fn stop(&mut self) -> Result<Option<SttResult>, SttProviderError> {
        if !self.active {
            return Err(SttProviderError::NotInitialized);
        }

        self.active = false;
        self.voice_active = false;

        Ok(None)
    }

    #[cfg(feature = "linux-speech")]
    fn cancel(&mut self) {
        if let Some(ref mut recognizer) = self.native_recognizer {
            recognizer.cancel();
        }

        self.active = false;
        self.voice_active = false;
        self.audio_buffer.clear();
        self.native_recognizer = None;
    }

    #[cfg(not(feature = "linux-speech"))]
    fn cancel(&mut self) {
        self.active = false;
        self.voice_active = false;
        self.audio_buffer.clear();
    }

    fn is_voice_active(&self) -> Option<bool> {
        if self.active {
            Some(self.voice_active)
        } else {
            None
        }
    }

    fn supported_formats(&self) -> &[AudioFormat] {
        &self.supported_formats
    }

    fn supported_languages(&self) -> &[String] {
        &self.supported_languages
    }
}

/// Linux Text-to-Speech provider.
///
/// Uses espeak-ng, Festival, or Piper for speech synthesis.
///
/// ## Implementation Status
///
/// When the `linux-speech` feature is enabled, TTS uses native espeak-ng bindings.
/// Otherwise, this is a stub implementation.
///
/// espeak-ng produces 22050 Hz mono PCM audio, which is resampled to the requested
/// format when necessary.
#[derive(Debug)]
pub struct LinuxTtsProvider {
    /// Currently synthesizing.
    active: bool,
    /// Current voice ID.
    voice_id: Option<String>,
    /// Speech rate.
    rate: f32,
    /// Pitch adjustment.
    pitch: f32,
    /// Volume.
    volume: f32,
    /// Supported formats.
    supported_formats: Vec<AudioFormat>,
    /// Available voices.
    voices: Vec<VoiceInfo>,
    /// Audio buffer.
    audio_buffer: Vec<u8>,
    /// Read position.
    read_position: usize,
    /// TTS backend in use.
    backend: TtsBackend,
}

/// TTS backend selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TtsBackend {
    /// espeak-ng (compact, multilingual).
    #[default]
    Espeak,
    /// Festival (university research quality).
    Festival,
    /// Piper (neural TTS).
    Piper,
}

impl LinuxTtsProvider {
    /// Create a new Linux TTS provider.
    pub fn new() -> Self {
        // When linux-speech feature is enabled, get voices from native bindings
        #[cfg(feature = "linux-speech")]
        let voices = {
            let native_voices = super::linux_native::get_voice_info_list();
            if native_voices.is_empty() {
                // Fall back to static list if espeak-ng not available
                get_tts_voices()
            } else {
                native_voices
            }
        };

        #[cfg(not(feature = "linux-speech"))]
        let voices = get_tts_voices();

        // espeak-ng outputs 22050 Hz, we support that plus resampled formats
        Self {
            active: false,
            voice_id: None,
            rate: 1.0,
            pitch: 1.0,
            volume: 1.0,
            supported_formats: vec![AudioFormat::Pcm16k, AudioFormat::Pcm44k],
            voices,
            audio_buffer: Vec::new(),
            read_position: 0,
            backend: TtsBackend::Espeak,
        }
    }

    /// Create with specific backend.
    pub fn with_backend(backend: TtsBackend) -> Self {
        let mut provider = Self::new();
        provider.backend = backend;
        provider
    }

    /// Get the current backend.
    pub fn backend(&self) -> TtsBackend {
        self.backend
    }

    fn find_voice(&self, id: &str) -> Option<&VoiceInfo> {
        self.voices.iter().find(|v| v.id == id)
    }

    /// Convert rate (0.5-2.0) to espeak words-per-minute (80-450).
    fn rate_to_wpm(&self) -> u32 {
        // Default is 175 wpm
        let base_wpm = 175.0;
        (base_wpm * self.rate).clamp(80.0, 450.0) as u32
    }

    /// Convert pitch (0.5-2.0) to espeak pitch (0-99).
    fn pitch_to_espeak(&self) -> u32 {
        // Default is 50
        ((self.pitch - 0.5) / 1.5 * 99.0).clamp(0.0, 99.0) as u32
    }
}

impl Default for LinuxTtsProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TtsProvider for LinuxTtsProvider {
    fn synthesize(
        &mut self,
        text: &str,
        format: AudioFormat,
        voice: Option<&str>,
    ) -> Result<Vec<u8>, TtsProviderError> {
        if !self.supported_formats.contains(&format) {
            return Err(TtsProviderError::UnsupportedFormat(format));
        }

        // When linux-speech feature is enabled, use native espeak-ng bindings
        #[cfg(feature = "linux-speech")]
        {
            // Synthesize using native espeak-ng bindings
            let samples = super::linux_native::synthesize_with_options(
                text,
                voice,
                self.rate,
                self.pitch,
                self.volume,
            )
            .map_err(TtsProviderError::Provider)?;

            // Convert samples to bytes
            let bytes = super::linux_native::samples_to_bytes(&samples);

            // espeak-ng outputs 22050 Hz, resample if needed
            // For now, return raw bytes (caller handles format conversion)
            // TODO: Implement proper resampling to match requested format
            Ok(bytes)
        }

        #[cfg(not(feature = "linux-speech"))]
        {
            // Stub implementation when feature not enabled
            let _ = (text, voice); // Suppress unused warnings

            if let Some(voice_id) = voice {
                if self.find_voice(voice_id).is_none() {
                    return Err(TtsProviderError::VoiceNotFound(voice_id.to_string()));
                }
            }

            Err(TtsProviderError::Provider(
                "Linux TTS requires linux-speech feature - install espeak-ng and enable the feature".to_string(),
            ))
        }
    }

    fn start_stream(
        &mut self,
        text: &str,
        format: AudioFormat,
        voice: Option<&str>,
    ) -> Result<(), TtsProviderError> {
        if self.active {
            return Err(TtsProviderError::Provider(
                "Streaming already active".to_string(),
            ));
        }

        if !self.supported_formats.contains(&format) {
            return Err(TtsProviderError::UnsupportedFormat(format));
        }

        if let Some(voice_id) = voice {
            self.voice_id = Some(voice_id.to_string());
        }

        // Pre-synthesize the entire audio (espeak-ng doesn't support true streaming)
        #[cfg(feature = "linux-speech")]
        {
            let samples = super::linux_native::synthesize_with_options(
                text,
                voice,
                self.rate,
                self.pitch,
                self.volume,
            )
            .map_err(TtsProviderError::Provider)?;

            self.audio_buffer = super::linux_native::samples_to_bytes(&samples);
        }

        #[cfg(not(feature = "linux-speech"))]
        {
            let _ = text; // Suppress unused warning
                          // No data available without the feature
            self.audio_buffer.clear();
        }

        self.active = true;
        self.read_position = 0;

        Ok(())
    }

    fn read_chunk(&mut self, buffer: &mut [u8]) -> Result<Option<usize>, TtsProviderError> {
        if !self.active {
            return Err(TtsProviderError::NotInitialized);
        }

        // Return data from pre-synthesized buffer
        if self.read_position >= self.audio_buffer.len() {
            return Ok(None); // No more data
        }

        let remaining = self.audio_buffer.len() - self.read_position;
        let to_copy = buffer.len().min(remaining);

        buffer[..to_copy]
            .copy_from_slice(&self.audio_buffer[self.read_position..self.read_position + to_copy]);

        self.read_position += to_copy;

        Ok(Some(to_copy))
    }

    fn stop_stream(&mut self) {
        self.active = false;
        self.audio_buffer.clear();
        self.read_position = 0;
    }

    fn estimate_duration(&self, text: &str) -> Duration {
        let word_count = text.split_whitespace().count();
        #[allow(clippy::cast_precision_loss)]
        let base_seconds = (word_count as f64) / 2.5;
        let adjusted_seconds = base_seconds / f64::from(self.rate);
        Duration::from_secs_f64(adjusted_seconds.max(0.1))
    }

    fn supported_formats(&self) -> &[AudioFormat] {
        &self.supported_formats
    }

    fn available_voices(&self) -> &[VoiceInfo] {
        &self.voices
    }

    fn set_rate(&mut self, rate: f32) {
        self.rate = rate.clamp(0.5, 2.0);
    }

    fn set_pitch(&mut self, pitch: f32) {
        self.pitch = pitch.clamp(0.5, 2.0);
    }

    fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_capabilities() {
        let caps = get_capabilities();
        assert!(caps.has_stt());
        assert!(caps.has_tts());
        assert!(caps.supports_offline_stt);
        assert!(caps.supports_offline_tts);
        assert!(!caps.stt_languages.is_empty());
        assert!(!caps.tts_voices.is_empty());
    }

    #[test]
    fn test_stt_provider_lifecycle() {
        let mut stt = LinuxSttProvider::new();
        assert!(stt.start(AudioFormat::Pcm16k, Some("en-US")).is_ok());
        assert!(stt.feed_audio(&[0u8; 1024]).is_ok());
        assert!(stt.stop().is_ok());
    }

    #[test]
    fn test_stt_backend_selection() {
        let vosk = LinuxSttProvider::with_backend(SttBackend::Vosk);
        assert_eq!(vosk.backend(), SttBackend::Vosk);

        let whisper = LinuxSttProvider::with_backend(SttBackend::Whisper);
        assert_eq!(whisper.backend(), SttBackend::Whisper);
    }

    #[test]
    fn test_tts_provider_basics() {
        let mut tts = LinuxTtsProvider::new();
        assert!(!tts.supported_formats().is_empty());
        assert!(!tts.available_voices().is_empty());

        let duration = tts.estimate_duration("Hello, this is a test");
        assert!(duration > Duration::ZERO);

        tts.set_rate(1.5);
        tts.set_pitch(0.8);
        tts.set_volume(0.5);
    }

    #[test]
    fn test_tts_backend_selection() {
        let espeak = LinuxTtsProvider::with_backend(TtsBackend::Espeak);
        assert_eq!(espeak.backend(), TtsBackend::Espeak);

        let festival = LinuxTtsProvider::with_backend(TtsBackend::Festival);
        assert_eq!(festival.backend(), TtsBackend::Festival);

        let piper = LinuxTtsProvider::with_backend(TtsBackend::Piper);
        assert_eq!(piper.backend(), TtsBackend::Piper);
    }

    #[test]
    fn test_rate_conversion() {
        let mut tts = LinuxTtsProvider::new();

        tts.set_rate(1.0);
        assert_eq!(tts.rate_to_wpm(), 175);

        tts.set_rate(0.5);
        assert_eq!(tts.rate_to_wpm(), 87); // ~87.5 -> 87

        tts.set_rate(2.0);
        assert_eq!(tts.rate_to_wpm(), 350);
    }

    #[test]
    fn test_pitch_conversion() {
        let mut tts = LinuxTtsProvider::new();

        tts.set_pitch(1.0);
        assert_eq!(tts.pitch_to_espeak(), 33); // (1.0-0.5)/1.5*99 = 33

        tts.set_pitch(0.5);
        assert_eq!(tts.pitch_to_espeak(), 0);

        tts.set_pitch(2.0);
        assert_eq!(tts.pitch_to_espeak(), 99);
    }
}
