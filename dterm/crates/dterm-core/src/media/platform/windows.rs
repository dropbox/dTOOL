//! Windows platform provider implementation.
//!
//! Uses Windows.Media.SpeechRecognition for STT and
//! Windows.Media.SpeechSynthesis for TTS.
//!
//! ## WinRT API References
//!
//! - STT: [`SpeechRecognizer`](https://docs.microsoft.com/en-us/uwp/api/windows.media.speechrecognition.speechrecognizer)
//! - TTS: [`SpeechSynthesizer`](https://docs.microsoft.com/en-us/uwp/api/windows.media.speechsynthesis.speechsynthesizer)
//!
//! ## Implementation Notes
//!
//! When the `windows-speech` feature is enabled, both TTS and STT use native WinRT
//! bindings via the `windows` crate. Otherwise, stub implementations are used.

use super::{
    PlatformCapabilities, SttProvider, SttProviderError, TtsProvider, TtsProviderError,
    VoiceGender, VoiceInfo, VoiceQuality,
};
use crate::media::{stt::SttResult, AudioFormat};
use std::time::Duration;

/// Get Windows platform capabilities.
///
/// Windows supports:
/// - Online and offline STT via Windows.Media.SpeechRecognition
/// - High-quality TTS via Windows.Media.SpeechSynthesis with SSML support
/// - Continuous dictation mode
/// - Grammar-based recognition
pub fn get_capabilities() -> PlatformCapabilities {
    PlatformCapabilities {
        stt_formats: vec![AudioFormat::Pcm16k, AudioFormat::Pcm44k],
        tts_formats: vec![AudioFormat::Pcm16k, AudioFormat::Pcm44k],
        supports_continuous_stt: true,
        supports_vad: true,
        supports_offline_stt: true, // Windows 10+ with language packs
        supports_offline_tts: true,
        stt_languages: get_stt_languages(),
        tts_voices: get_tts_voices(),
    }
}

/// Get supported STT languages on Windows (native when feature enabled).
#[cfg(feature = "windows-speech")]
fn get_stt_languages() -> Vec<String> {
    super::windows_stt_native::get_supported_languages()
}

/// Get supported STT languages on Windows (stub when native not available).
#[cfg(not(feature = "windows-speech"))]
fn get_stt_languages() -> Vec<String> {
    // Languages available with Windows Speech Recognition
    // Actual availability depends on installed language packs
    vec![
        "en-US".to_string(),
        "en-GB".to_string(),
        "en-AU".to_string(),
        "en-CA".to_string(),
        "en-IN".to_string(),
        "es-ES".to_string(),
        "es-MX".to_string(),
        "fr-FR".to_string(),
        "fr-CA".to_string(),
        "de-DE".to_string(),
        "it-IT".to_string(),
        "ja-JP".to_string(),
        "ko-KR".to_string(),
        "zh-CN".to_string(),
        "zh-TW".to_string(),
        "pt-BR".to_string(),
        "ru-RU".to_string(),
        "nl-NL".to_string(),
        "pl-PL".to_string(),
    ]
}

/// Get available TTS voices on Windows.
#[cfg(feature = "windows-speech")]
fn get_tts_voices() -> Vec<VoiceInfo> {
    super::windows_native::get_voice_info_list()
}

/// Get available TTS voices on Windows (stub when native not available).
#[cfg(not(feature = "windows-speech"))]
fn get_tts_voices() -> Vec<VoiceInfo> {
    // Default voices available on Windows 10/11
    // Additional voices available via Settings > Time & Language > Speech
    vec![
        VoiceInfo {
            id: "Microsoft David Desktop".to_string(),
            name: "David".to_string(),
            language: "en-US".to_string(),
            gender: Some(VoiceGender::Male),
            quality: VoiceQuality::Standard,
        },
        VoiceInfo {
            id: "Microsoft Zira Desktop".to_string(),
            name: "Zira".to_string(),
            language: "en-US".to_string(),
            gender: Some(VoiceGender::Female),
            quality: VoiceQuality::Standard,
        },
        VoiceInfo {
            id: "Microsoft Mark Desktop".to_string(),
            name: "Mark".to_string(),
            language: "en-US".to_string(),
            gender: Some(VoiceGender::Male),
            quality: VoiceQuality::Standard,
        },
        VoiceInfo {
            id: "Microsoft Jenny Online (Natural)".to_string(),
            name: "Jenny (Neural)".to_string(),
            language: "en-US".to_string(),
            gender: Some(VoiceGender::Female),
            quality: VoiceQuality::Premium,
        },
        VoiceInfo {
            id: "Microsoft Aria Online (Natural)".to_string(),
            name: "Aria (Neural)".to_string(),
            language: "en-US".to_string(),
            gender: Some(VoiceGender::Female),
            quality: VoiceQuality::Premium,
        },
        VoiceInfo {
            id: "Microsoft Hazel Desktop".to_string(),
            name: "Hazel".to_string(),
            language: "en-GB".to_string(),
            gender: Some(VoiceGender::Female),
            quality: VoiceQuality::Standard,
        },
        VoiceInfo {
            id: "Microsoft Helena Desktop".to_string(),
            name: "Helena".to_string(),
            language: "es-ES".to_string(),
            gender: Some(VoiceGender::Female),
            quality: VoiceQuality::Standard,
        },
        VoiceInfo {
            id: "Microsoft Hortense Desktop".to_string(),
            name: "Hortense".to_string(),
            language: "fr-FR".to_string(),
            gender: Some(VoiceGender::Female),
            quality: VoiceQuality::Standard,
        },
        VoiceInfo {
            id: "Microsoft Hedda Desktop".to_string(),
            name: "Hedda".to_string(),
            language: "de-DE".to_string(),
            gender: Some(VoiceGender::Female),
            quality: VoiceQuality::Standard,
        },
        VoiceInfo {
            id: "Microsoft Haruka Desktop".to_string(),
            name: "Haruka".to_string(),
            language: "ja-JP".to_string(),
            gender: Some(VoiceGender::Female),
            quality: VoiceQuality::Standard,
        },
    ]
}

/// Windows Speech-to-Text provider.
///
/// Uses Windows.Media.SpeechRecognition for speech recognition.
///
/// ## Implementation Status
///
/// When the `windows-speech` feature is enabled, this provider uses native
/// WinRT bindings to Windows.Media.SpeechRecognition. Otherwise, it is a stub.
///
/// ## Native Implementation Details
///
/// The native implementation provides:
/// - Language enumeration from installed language packs
/// - Single-shot recognition via system microphone
/// - Thread-safe recognizer wrapper
///
/// Note: Windows SpeechRecognizer typically uses system audio input directly.
/// The `feed_audio()` method accumulates audio for compatibility with the trait
/// interface, but actual recognition uses the system microphone.
#[derive(Debug)]
pub struct WindowsSttProvider {
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
    /// Audio buffer for recognition.
    audio_buffer: Vec<u8>,
    /// Native recognizer (when windows-speech feature is enabled).
    #[cfg(feature = "windows-speech")]
    native_recognizer: Option<super::windows_stt_native::ThreadSafeRecognizer>,
}

impl WindowsSttProvider {
    /// Create a new Windows STT provider.
    pub fn new() -> Self {
        Self {
            active: false,
            format: AudioFormat::Pcm16k,
            language: "en-US".to_string(),
            supported_formats: vec![AudioFormat::Pcm16k, AudioFormat::Pcm44k],
            supported_languages: get_stt_languages(),
            voice_active: false,
            audio_buffer: Vec::new(),
            #[cfg(feature = "windows-speech")]
            native_recognizer: super::windows_stt_native::ThreadSafeRecognizer::new(),
        }
    }
}

impl Default for WindowsSttProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SttProvider for WindowsSttProvider {
    #[cfg(feature = "windows-speech")]
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

        // Create language-specific recognizer if different from default
        if lang != "en-US" && self.native_recognizer.is_some() {
            self.native_recognizer =
                super::windows_stt_native::ThreadSafeRecognizer::with_language(lang);
        }

        // Start native recognizer
        if let Some(ref mut recognizer) = self.native_recognizer {
            recognizer.start().map_err(SttProviderError::Provider)?;
        } else {
            return Err(SttProviderError::Provider(
                "Speech recognition not available".to_string(),
            ));
        }

        self.active = true;
        self.format = format;
        self.language = lang.to_string();
        self.audio_buffer.clear();
        self.voice_active = false;

        Ok(())
    }

    #[cfg(not(feature = "windows-speech"))]
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

    #[cfg(feature = "windows-speech")]
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

    #[cfg(not(feature = "windows-speech"))]
    fn feed_audio(&mut self, data: &[u8]) -> Result<(), SttProviderError> {
        if !self.active {
            return Err(SttProviderError::NotInitialized);
        }

        self.audio_buffer.extend_from_slice(data);
        self.voice_active = data.iter().any(|&b| b > 32 || b < 224);

        Ok(())
    }

    #[cfg(feature = "windows-speech")]
    fn get_partial(&mut self) -> Option<SttResult> {
        if !self.active {
            return None;
        }

        if let Some(ref recognizer) = self.native_recognizer {
            if let Some(result) = recognizer.get_partial() {
                return Some(SttResult {
                    text: result.text,
                    confidence: result.confidence as f32,
                    is_final: result.is_final,
                    language: Some(self.language.clone()),
                });
            }
        }

        None
    }

    #[cfg(not(feature = "windows-speech"))]
    fn get_partial(&mut self) -> Option<SttResult> {
        if !self.active || self.audio_buffer.is_empty() {
            return None;
        }

        // Stub: return no partial result
        None
    }

    #[cfg(feature = "windows-speech")]
    fn stop(&mut self) -> Result<Option<SttResult>, SttProviderError> {
        if !self.active {
            return Err(SttProviderError::NotInitialized);
        }

        let result = if let Some(ref mut recognizer) = self.native_recognizer {
            match recognizer.stop() {
                Ok(Some(result)) => Some(SttResult {
                    text: result.text,
                    confidence: result.confidence as f32,
                    is_final: true,
                    language: Some(self.language.clone()),
                }),
                Ok(None) => None,
                Err(e) => {
                    self.active = false;
                    self.voice_active = false;
                    return Err(SttProviderError::Provider(e));
                }
            }
        } else {
            None
        };

        self.active = false;
        self.voice_active = false;

        Ok(result)
    }

    #[cfg(not(feature = "windows-speech"))]
    fn stop(&mut self) -> Result<Option<SttResult>, SttProviderError> {
        if !self.active {
            return Err(SttProviderError::NotInitialized);
        }

        self.active = false;
        self.voice_active = false;

        Ok(None)
    }

    #[cfg(feature = "windows-speech")]
    fn cancel(&mut self) {
        if let Some(ref mut recognizer) = self.native_recognizer {
            recognizer.cancel();
        }

        self.active = false;
        self.voice_active = false;
        self.audio_buffer.clear();
    }

    #[cfg(not(feature = "windows-speech"))]
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

/// Windows Text-to-Speech provider.
///
/// Uses Windows.Media.SpeechSynthesis for speech synthesis.
///
/// ## Implementation Status
///
/// This is currently a stub. Full implementation requires:
/// 1. Create SpeechSynthesizer
/// 2. Enumerate and select voice
/// 3. Configure voice options (rate, pitch, volume)
/// 4. Synthesize text or SSML
/// 5. Stream audio output
#[derive(Debug)]
pub struct WindowsTtsProvider {
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
    /// Synthesized audio buffer.
    audio_buffer: Vec<u8>,
    /// Read position.
    read_position: usize,
}

impl WindowsTtsProvider {
    /// Create a new Windows TTS provider.
    pub fn new() -> Self {
        Self {
            active: false,
            voice_id: None,
            rate: 1.0,
            pitch: 1.0,
            volume: 1.0,
            supported_formats: vec![AudioFormat::Pcm16k, AudioFormat::Pcm44k],
            voices: get_tts_voices(),
            audio_buffer: Vec::new(),
            read_position: 0,
        }
    }

    fn find_voice(&self, id: &str) -> Option<&VoiceInfo> {
        self.voices.iter().find(|v| v.id == id)
    }
}

impl Default for WindowsTtsProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TtsProvider for WindowsTtsProvider {
    #[cfg(feature = "windows-speech")]
    fn synthesize(
        &mut self,
        text: &str,
        format: AudioFormat,
        voice: Option<&str>,
    ) -> Result<Vec<u8>, TtsProviderError> {
        if !self.supported_formats.contains(&format) {
            return Err(TtsProviderError::UnsupportedFormat(format));
        }

        if let Some(voice_id) = voice {
            if self.find_voice(voice_id).is_none() {
                return Err(TtsProviderError::VoiceNotFound(voice_id.to_string()));
            }
        }

        // Use native WinRT bindings
        super::windows_native::synthesize_with_options(
            text,
            voice.or(self.voice_id.as_deref()),
            self.rate,
            self.pitch,
            self.volume,
        )
        .map_err(TtsProviderError::Provider)
    }

    #[cfg(not(feature = "windows-speech"))]
    fn synthesize(
        &mut self,
        _text: &str,
        format: AudioFormat,
        voice: Option<&str>,
    ) -> Result<Vec<u8>, TtsProviderError> {
        if !self.supported_formats.contains(&format) {
            return Err(TtsProviderError::UnsupportedFormat(format));
        }

        if let Some(voice_id) = voice {
            if self.find_voice(voice_id).is_none() {
                return Err(TtsProviderError::VoiceNotFound(voice_id.to_string()));
            }
        }

        Err(TtsProviderError::Provider(
            "Windows TTS requires 'windows-speech' feature".to_string(),
        ))
    }

    #[cfg(feature = "windows-speech")]
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
            if self.find_voice(voice_id).is_none() {
                return Err(TtsProviderError::VoiceNotFound(voice_id.to_string()));
            }
            self.voice_id = Some(voice_id.to_string());
        }

        // Pre-synthesize the audio into our buffer
        // Windows TTS doesn't support true streaming, so we synthesize upfront
        self.audio_buffer = super::windows_native::synthesize_with_options(
            text,
            voice.or(self.voice_id.as_deref()),
            self.rate,
            self.pitch,
            self.volume,
        )
        .map_err(TtsProviderError::Provider)?;

        self.active = true;
        self.read_position = 0;

        Ok(())
    }

    #[cfg(not(feature = "windows-speech"))]
    fn start_stream(
        &mut self,
        _text: &str,
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
            if self.find_voice(voice_id).is_none() {
                return Err(TtsProviderError::VoiceNotFound(voice_id.to_string()));
            }
            self.voice_id = Some(voice_id.to_string());
        }

        self.active = true;
        self.audio_buffer.clear();
        self.read_position = 0;

        Ok(())
    }

    fn read_chunk(&mut self, buffer: &mut [u8]) -> Result<Option<usize>, TtsProviderError> {
        if !self.active {
            return Err(TtsProviderError::NotInitialized);
        }

        // Return chunks from the pre-synthesized buffer
        if self.read_position >= self.audio_buffer.len() {
            return Ok(None);
        }

        let remaining = self.audio_buffer.len() - self.read_position;
        let to_copy = remaining.min(buffer.len());

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
        assert!(caps.supports_continuous_stt);
        assert!(!caps.stt_languages.is_empty());
        assert!(!caps.tts_voices.is_empty());
    }

    #[test]
    fn test_stt_provider_lifecycle() {
        let mut stt = WindowsSttProvider::new();
        assert!(stt.start(AudioFormat::Pcm16k, Some("en-US")).is_ok());
        assert!(stt.feed_audio(&[0u8; 1024]).is_ok());
        assert!(stt.stop().is_ok());
    }

    #[test]
    fn test_tts_provider_basics() {
        let mut tts = WindowsTtsProvider::new();
        assert!(!tts.supported_formats().is_empty());
        assert!(!tts.available_voices().is_empty());

        let duration = tts.estimate_duration("Hello, this is a test");
        assert!(duration > Duration::ZERO);

        tts.set_rate(1.5);
        tts.set_pitch(0.8);
        tts.set_volume(0.5);
    }
}
