//! iOS platform provider implementation.
//!
//! Uses Apple's Speech framework for STT and AVSpeechSynthesizer for TTS.
//! iOS shares the same APIs as macOS but with some platform-specific considerations.
//!
//! ## Framework References
//!
//! - STT: [`SFSpeechRecognizer`](https://developer.apple.com/documentation/speech/sfspeechrecognizer)
//! - TTS: [`AVSpeechSynthesizer`](https://developer.apple.com/documentation/avfoundation/avspeechsynthesizer)
//!
//! ## iOS-Specific Considerations
//!
//! - Requires `NSMicrophoneUsageDescription` in Info.plist for STT
//! - Requires `NSSpeechRecognitionUsageDescription` in Info.plist for STT
//! - Audio session must be properly configured for background audio
//! - Some voices may require download on iOS
//!
//! ## Implementation Notes
//!
//! When the `ios-speech` feature is enabled:
//! - TTS uses native AVSpeechSynthesizer bindings via objc2
//! - STT uses native SFSpeechRecognizer bindings via objc2
//!
//! Without this feature, stubbed implementations are used.
//!
//! ## Feature Flag
//!
//! - `ios-speech`: Enables native speech bindings via objc2.

use super::{
    PlatformCapabilities, SttProvider, SttProviderError, TtsProvider, TtsProviderError, VoiceInfo,
    VoiceQuality,
};

#[cfg(not(feature = "ios-speech"))]
use super::VoiceGender;
use crate::media::{stt::SttResult, AudioFormat};
use std::time::Duration;

#[cfg(feature = "ios-speech")]
use super::macos_native;

#[cfg(feature = "ios-speech")]
use super::macos_stt_native;

/// Get iOS platform capabilities.
///
/// iOS supports:
/// - Continuous on-device STT via Speech framework (iOS 10+)
/// - High-quality TTS via AVSpeechSynthesizer with multiple voices
/// - Voice Activity Detection (VAD)
/// - Offline STT (with downloaded language models)
/// - Offline TTS (with downloaded voices)
///
/// Note: Some features may require iOS 13+ or iOS 15+ for enhanced voices.
pub fn get_capabilities() -> PlatformCapabilities {
    PlatformCapabilities {
        stt_formats: vec![AudioFormat::Pcm16k, AudioFormat::Pcm44k],
        tts_formats: vec![AudioFormat::Pcm16k, AudioFormat::Pcm44k, AudioFormat::Aac],
        supports_continuous_stt: true,
        supports_vad: true,
        supports_offline_stt: true, // With downloaded models (iOS 13+)
        supports_offline_tts: true, // With downloaded voices
        stt_languages: get_stt_languages(),
        tts_voices: get_tts_voices(),
    }
}

/// Get supported STT languages on iOS.
///
/// When `ios-speech` feature is enabled, returns actual supported locales
/// from SFSpeechRecognizer. Otherwise returns a static list.
fn get_stt_languages() -> Vec<String> {
    #[cfg(feature = "ios-speech")]
    {
        macos_stt_native::get_supported_locales()
    }

    #[cfg(not(feature = "ios-speech"))]
    {
        // Common languages supported by Speech framework on iOS
        // Full list available at runtime via SFSpeechRecognizer.supportedLocales()
        vec![
            "en-US".to_string(),
            "en-GB".to_string(),
            "en-AU".to_string(),
            "es-ES".to_string(),
            "es-MX".to_string(),
            "fr-FR".to_string(),
            "de-DE".to_string(),
            "it-IT".to_string(),
            "ja-JP".to_string(),
            "ko-KR".to_string(),
            "zh-CN".to_string(),
            "zh-TW".to_string(),
            "pt-BR".to_string(),
            "ru-RU".to_string(),
            "ar-SA".to_string(),
            "hi-IN".to_string(),
            "th-TH".to_string(),
            "vi-VN".to_string(),
        ]
    }
}

/// Get available TTS voices on iOS.
///
/// When `ios-speech` feature is enabled, returns actual system voices
/// via AVSpeechSynthesisVoice.speechVoices(). Otherwise returns a static list.
fn get_tts_voices() -> Vec<VoiceInfo> {
    #[cfg(feature = "ios-speech")]
    {
        get_native_tts_voices()
    }

    #[cfg(not(feature = "ios-speech"))]
    {
        get_static_tts_voices()
    }
}

/// Get TTS voices from the native AVSpeechSynthesisVoice API.
#[cfg(feature = "ios-speech")]
fn get_native_tts_voices() -> Vec<VoiceInfo> {
    use super::macos_native::{
        AVSPEECH_SYNTHESIS_VOICE_QUALITY_DEFAULT, AVSPEECH_SYNTHESIS_VOICE_QUALITY_ENHANCED,
        AVSPEECH_SYNTHESIS_VOICE_QUALITY_PREMIUM,
    };

    let native_voices = macos_native::get_available_voices();

    native_voices
        .into_iter()
        .map(|(id, name, language, quality)| {
            let voice_quality = match quality {
                q if q == AVSPEECH_SYNTHESIS_VOICE_QUALITY_DEFAULT => VoiceQuality::Standard,
                q if q == AVSPEECH_SYNTHESIS_VOICE_QUALITY_ENHANCED => VoiceQuality::High,
                q if q == AVSPEECH_SYNTHESIS_VOICE_QUALITY_PREMIUM => VoiceQuality::Premium,
                _ => VoiceQuality::Standard,
            };

            VoiceInfo {
                id,
                name,
                language,
                gender: None, // AVSpeechSynthesisVoice doesn't expose gender directly
                quality: voice_quality,
            }
        })
        .collect()
}

/// Static TTS voice list (used when native bindings unavailable).
#[cfg(not(feature = "ios-speech"))]
fn get_static_tts_voices() -> Vec<VoiceInfo> {
    // Subset of voices available on iOS
    // Full list available at runtime via AVSpeechSynthesisVoice.speechVoices()
    // Note: Enhanced/Premium voices may require download
    vec![
        VoiceInfo {
            id: "com.apple.voice.compact.en-US.Samantha".to_string(),
            name: "Samantha".to_string(),
            language: "en-US".to_string(),
            gender: Some(VoiceGender::Female),
            quality: VoiceQuality::Standard,
        },
        VoiceInfo {
            id: "com.apple.voice.enhanced.en-US.Samantha".to_string(),
            name: "Samantha (Enhanced)".to_string(),
            language: "en-US".to_string(),
            gender: Some(VoiceGender::Female),
            quality: VoiceQuality::High,
        },
        VoiceInfo {
            id: "com.apple.voice.compact.en-US.Tom".to_string(),
            name: "Tom".to_string(),
            language: "en-US".to_string(),
            gender: Some(VoiceGender::Male),
            quality: VoiceQuality::Standard,
        },
        VoiceInfo {
            id: "com.apple.voice.compact.en-GB.Daniel".to_string(),
            name: "Daniel".to_string(),
            language: "en-GB".to_string(),
            gender: Some(VoiceGender::Male),
            quality: VoiceQuality::Standard,
        },
        VoiceInfo {
            id: "com.apple.voice.compact.es-ES.Monica".to_string(),
            name: "Monica".to_string(),
            language: "es-ES".to_string(),
            gender: Some(VoiceGender::Female),
            quality: VoiceQuality::Standard,
        },
        VoiceInfo {
            id: "com.apple.voice.compact.fr-FR.Thomas".to_string(),
            name: "Thomas".to_string(),
            language: "fr-FR".to_string(),
            gender: Some(VoiceGender::Male),
            quality: VoiceQuality::Standard,
        },
        VoiceInfo {
            id: "com.apple.voice.compact.de-DE.Anna".to_string(),
            name: "Anna".to_string(),
            language: "de-DE".to_string(),
            gender: Some(VoiceGender::Female),
            quality: VoiceQuality::Standard,
        },
        VoiceInfo {
            id: "com.apple.voice.compact.ja-JP.Kyoko".to_string(),
            name: "Kyoko".to_string(),
            language: "ja-JP".to_string(),
            gender: Some(VoiceGender::Female),
            quality: VoiceQuality::Standard,
        },
        VoiceInfo {
            id: "com.apple.voice.compact.zh-CN.Tingting".to_string(),
            name: "Tingting".to_string(),
            language: "zh-CN".to_string(),
            gender: Some(VoiceGender::Female),
            quality: VoiceQuality::Standard,
        },
    ]
}

/// iOS Speech-to-Text provider.
///
/// Uses SFSpeechRecognizer for on-device speech recognition.
///
/// ## Implementation Status
///
/// When the `ios-speech` feature is enabled, this provider uses native
/// SFSpeechRecognizer bindings for real speech recognition. Without the feature,
/// the implementation is stubbed.
///
/// ## Authorization
///
/// Speech recognition requires user authorization. Call
/// `IosSttProvider::request_authorization()` before using the provider.
/// The app's Info.plist must include `NSSpeechRecognitionUsageDescription`.
///
/// ## Privacy Requirements
///
/// iOS requires the following Info.plist entries:
/// - `NSMicrophoneUsageDescription`: Explain microphone usage
/// - `NSSpeechRecognitionUsageDescription`: Explain speech recognition usage
///
/// ## Thread Safety
///
/// When `ios-speech` feature is enabled, the provider uses a thread-safe
/// recognizer wrapper that handles result collection internally.
pub struct IosSttProvider {
    /// Session active flag.
    active: bool,
    /// Current audio format.
    format: AudioFormat,
    /// Current language.
    language: String,
    /// Supported formats (static).
    supported_formats: Vec<AudioFormat>,
    /// Supported languages (from Speech framework).
    supported_languages: Vec<String>,
    /// Voice activity detection state.
    voice_active: bool,
    /// Accumulated audio data (for stub/VAD simulation).
    audio_buffer: Vec<u8>,
    /// Native recognizer (only with ios-speech feature).
    #[cfg(feature = "ios-speech")]
    recognizer: Option<macos_stt_native::ThreadSafeRecognizer>,
    /// Last partial result (cached for retrieval).
    #[cfg(feature = "ios-speech")]
    last_partial: Option<SttResult>,
}

impl IosSttProvider {
    /// Create a new iOS STT provider.
    pub fn new() -> Self {
        Self {
            active: false,
            format: AudioFormat::Pcm16k,
            language: "en-US".to_string(),
            supported_formats: vec![AudioFormat::Pcm16k, AudioFormat::Pcm44k],
            supported_languages: get_stt_languages(),
            voice_active: false,
            audio_buffer: Vec::new(),
            #[cfg(feature = "ios-speech")]
            recognizer: None,
            #[cfg(feature = "ios-speech")]
            last_partial: None,
        }
    }

    /// Check if speech recognition is authorized.
    ///
    /// Returns `true` if the user has granted permission.
    #[cfg(feature = "ios-speech")]
    pub fn is_authorized() -> bool {
        macos_stt_native::is_authorized()
    }

    /// Check if speech recognition is authorized (stub when native bindings unavailable).
    #[allow(dead_code)]
    #[cfg(not(feature = "ios-speech"))]
    pub fn is_authorized() -> bool {
        false
    }

    /// Get the current authorization status.
    #[allow(dead_code)]
    #[cfg(feature = "ios-speech")]
    pub fn authorization_status() -> &'static str {
        match macos_stt_native::SFSpeechRecognizer::authorization_status() {
            macos_stt_native::SFSpeechRecognizerAuthorizationStatus::NotDetermined => {
                "not_determined"
            }
            macos_stt_native::SFSpeechRecognizerAuthorizationStatus::Denied => "denied",
            macos_stt_native::SFSpeechRecognizerAuthorizationStatus::Restricted => "restricted",
            macos_stt_native::SFSpeechRecognizerAuthorizationStatus::Authorized => "authorized",
        }
    }

    /// Get the current authorization status (stub when native bindings unavailable).
    #[allow(dead_code)]
    #[cfg(not(feature = "ios-speech"))]
    pub fn authorization_status() -> &'static str {
        "not_available"
    }

    /// Check if the provider is using native bindings.
    #[allow(dead_code)]
    pub fn is_native(&self) -> bool {
        cfg!(feature = "ios-speech")
    }

    /// Check if on-device recognition is supported.
    #[allow(dead_code)]
    #[cfg(feature = "ios-speech")]
    pub fn supports_on_device(&self) -> bool {
        self.recognizer
            .as_ref()
            .map(|r| r.supports_on_device())
            .unwrap_or(false)
    }

    /// Check if on-device recognition is supported (stub).
    #[allow(dead_code)]
    #[cfg(not(feature = "ios-speech"))]
    pub fn supports_on_device(&self) -> bool {
        false
    }

    /// Get sample rate for the current format.
    #[allow(dead_code)]
    fn sample_rate(&self) -> f64 {
        match self.format {
            AudioFormat::Pcm16k => 16000.0,
            AudioFormat::Pcm44k => 44100.0,
            _ => 16000.0,
        }
    }

    /// Simple VAD based on audio energy.
    fn detect_voice_activity(&mut self, data: &[u8]) {
        // Convert PCM16 bytes to samples and calculate RMS energy
        if data.len() < 2 {
            return;
        }

        let samples: Vec<i16> = data
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();

        if samples.is_empty() {
            return;
        }

        // Calculate RMS energy
        let sum_squares: f64 = samples.iter().map(|&s| f64::from(s).powi(2)).sum();
        #[allow(clippy::cast_precision_loss)]
        let rms = (sum_squares / samples.len() as f64).sqrt();

        // Threshold for voice activity (roughly -40dB)
        const VOICE_THRESHOLD: f64 = 300.0;
        self.voice_active = rms > VOICE_THRESHOLD;
    }
}

impl std::fmt::Debug for IosSttProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IosSttProvider")
            .field("active", &self.active)
            .field("format", &self.format)
            .field("language", &self.language)
            .field("voice_active", &self.voice_active)
            .field("is_native", &self.is_native())
            .finish()
    }
}

impl Default for IosSttProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SttProvider for IosSttProvider {
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

        self.format = format;
        self.language = lang.to_string();
        self.audio_buffer.clear();
        self.voice_active = false;

        #[cfg(feature = "ios-speech")]
        {
            // Check authorization
            if !Self::is_authorized() {
                return Err(SttProviderError::Provider(
                    "Speech recognition not authorized. Call request_authorization() first."
                        .to_string(),
                ));
            }

            // Create recognizer for the requested locale
            let recognizer = macos_stt_native::ThreadSafeRecognizer::with_locale(lang)
                .or_else(macos_stt_native::ThreadSafeRecognizer::new)
                .ok_or_else(|| {
                    SttProviderError::Provider("Failed to create speech recognizer".to_string())
                })?;

            // Check if recognizer is available
            if !recognizer.is_available() {
                return Err(SttProviderError::Provider(
                    "Speech recognizer not available".to_string(),
                ));
            }

            // Get sample rate before storing recognizer
            let sample_rate = self.sample_rate();
            let mut recognizer = recognizer;
            recognizer.set_sample_rate(sample_rate);

            // Check on-device support
            let on_device = recognizer.supports_on_device();

            // Start recognition with partial results enabled
            recognizer.start(on_device, true).map_err(|e| {
                SttProviderError::Provider(format!("Failed to start recognition: {}", e))
            })?;

            self.recognizer = Some(recognizer);
            self.last_partial = None;
        }

        self.active = true;
        Ok(())
    }

    fn feed_audio(&mut self, data: &[u8]) -> Result<(), SttProviderError> {
        if !self.active {
            return Err(SttProviderError::NotInitialized);
        }

        // Accumulate for VAD
        self.audio_buffer.extend_from_slice(data);

        // Detect voice activity
        self.detect_voice_activity(data);

        #[cfg(feature = "ios-speech")]
        {
            if let Some(ref mut recognizer) = self.recognizer {
                recognizer.feed_audio(data).map_err(|e| {
                    SttProviderError::Provider(format!("Failed to feed audio: {}", e))
                })?;

                // Check for partial results and cache them
                if let Some(partial) = recognizer.get_partial() {
                    // Convert confidence from f32 (0.0-1.0) to u8 (0-100)
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let confidence_u8 = partial
                        .confidence
                        .map(|c| (c * 100.0).clamp(0.0, 100.0) as u8)
                        .unwrap_or(0);

                    // Use dummy client ID 0 - will be replaced by the server layer
                    self.last_partial = Some(SttResult::partial(0, partial.text, confidence_u8));
                }
            }
        }

        Ok(())
    }

    fn get_partial(&mut self) -> Option<SttResult> {
        if !self.active {
            return None;
        }

        #[cfg(feature = "ios-speech")]
        {
            // Return cached partial result
            self.last_partial.clone()
        }

        #[cfg(not(feature = "ios-speech"))]
        {
            None
        }
    }

    fn stop(&mut self) -> Result<Option<SttResult>, SttProviderError> {
        if !self.active {
            return Err(SttProviderError::NotInitialized);
        }

        self.active = false;
        self.voice_active = false;

        #[cfg(feature = "ios-speech")]
        {
            let result = if let Some(ref mut recognizer) = self.recognizer {
                match recognizer.stop() {
                    Ok(Some(final_result)) => {
                        // Use dummy client ID 0 - will be replaced by the server layer
                        Some(SttResult::final_result(0, final_result.text))
                    }
                    Ok(None) => self.last_partial.clone(),
                    Err(e) => {
                        return Err(SttProviderError::Provider(format!(
                            "Failed to stop recognition: {}",
                            e
                        )));
                    }
                }
            } else {
                None
            };

            self.recognizer = None;
            self.last_partial = None;

            Ok(result)
        }

        #[cfg(not(feature = "ios-speech"))]
        {
            Ok(None)
        }
    }

    fn cancel(&mut self) {
        #[cfg(feature = "ios-speech")]
        {
            if let Some(ref mut recognizer) = self.recognizer {
                recognizer.cancel();
            }
            self.recognizer = None;
            self.last_partial = None;
        }

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

/// iOS Text-to-Speech provider.
///
/// Uses AVSpeechSynthesizer for high-quality speech synthesis.
///
/// ## Implementation Status
///
/// When the `ios-speech` feature is enabled, this provider uses native
/// AVSpeechSynthesizer bindings for real TTS playback. Without the feature,
/// the implementation is stubbed.
///
/// Note: AVSpeechSynthesizer plays audio directly to the system audio output.
/// The `synthesize()` method that returns audio data is not yet supported
/// (would require AVAudioEngine integration). Use `start_stream()` for playback.
///
/// ## Audio Session Considerations
///
/// iOS requires careful audio session management:
/// - Use `.playback` category for TTS-only
/// - Use `.playAndRecord` if combining with STT
/// - Handle audio interruptions (calls, Siri, etc.)
///
/// ## Thread Safety
///
/// When `ios-speech` feature is enabled, the provider uses a persistent
/// `ThreadSafeSynthesizer` that dispatches operations to the main thread via GCD.
/// This enables pause/resume/stop control for speech playback.
pub struct IosTtsProvider {
    /// Currently synthesizing.
    active: bool,
    /// Current voice ID.
    voice_id: Option<String>,
    /// Speech rate (0.5 - 2.0).
    rate: f32,
    /// Pitch adjustment (0.5 - 2.0).
    pitch: f32,
    /// Volume (0.0 - 1.0).
    volume: f32,
    /// Supported formats.
    supported_formats: Vec<AudioFormat>,
    /// Available voices.
    voices: Vec<VoiceInfo>,
    /// Synthesized audio buffer (for streaming).
    audio_buffer: Vec<u8>,
    /// Read position in audio buffer.
    read_position: usize,
    /// Thread-safe synthesizer (only with ios-speech feature).
    #[cfg(feature = "ios-speech")]
    synthesizer: macos_native::ThreadSafeSynthesizer,
}

impl IosTtsProvider {
    /// Create a new iOS TTS provider.
    pub fn new() -> Self {
        Self {
            active: false,
            voice_id: None,
            rate: 1.0,
            pitch: 1.0,
            volume: 1.0,
            supported_formats: vec![AudioFormat::Pcm16k, AudioFormat::Pcm44k, AudioFormat::Aac],
            voices: get_tts_voices(),
            audio_buffer: Vec::new(),
            read_position: 0,
            #[cfg(feature = "ios-speech")]
            synthesizer: macos_native::ThreadSafeSynthesizer::new(),
        }
    }

    /// Find voice info by ID.
    fn find_voice(&self, id: &str) -> Option<&VoiceInfo> {
        self.voices.iter().find(|v| v.id == id)
    }

    /// Check if the provider is using native bindings.
    #[allow(dead_code)]
    pub fn is_native(&self) -> bool {
        cfg!(feature = "ios-speech")
    }

    /// Speak text using native AVSpeechSynthesizer.
    ///
    /// Uses the persistent thread-safe synthesizer for playback with control.
    /// The audio plays directly to system output.
    #[cfg(feature = "ios-speech")]
    #[allow(dead_code)]
    pub fn speak(&mut self, text: &str) {
        self.synthesizer.speak(
            text,
            self.voice_id.as_deref(),
            self.rate,
            self.pitch,
            self.volume,
        );
    }

    /// Speak text (stub when native bindings unavailable).
    #[cfg(not(feature = "ios-speech"))]
    #[allow(dead_code)]
    pub fn speak(&mut self, _text: &str) {
        // No-op without native bindings
    }

    /// Pause speech playback at word boundary.
    ///
    /// Returns true if pause was successful.
    #[cfg(feature = "ios-speech")]
    #[allow(dead_code)]
    pub fn pause(&mut self) -> bool {
        self.synthesizer.pause()
    }

    /// Pause speech playback (stub when native bindings unavailable).
    #[cfg(not(feature = "ios-speech"))]
    #[allow(dead_code)]
    pub fn pause(&mut self) -> bool {
        false
    }

    /// Resume paused speech playback.
    ///
    /// Returns true if resume was successful.
    #[cfg(feature = "ios-speech")]
    #[allow(dead_code)]
    pub fn resume(&mut self) -> bool {
        self.synthesizer.resume()
    }

    /// Resume speech playback (stub when native bindings unavailable).
    #[cfg(not(feature = "ios-speech"))]
    #[allow(dead_code)]
    pub fn resume(&mut self) -> bool {
        false
    }

    /// Stop speech playback immediately.
    ///
    /// Returns true if stop was successful.
    #[cfg(feature = "ios-speech")]
    #[allow(dead_code)]
    pub fn stop(&mut self) -> bool {
        self.active = false;
        self.synthesizer.stop()
    }

    /// Stop speech playback (stub when native bindings unavailable).
    #[cfg(not(feature = "ios-speech"))]
    #[allow(dead_code)]
    pub fn stop(&mut self) -> bool {
        self.active = false;
        false
    }

    /// Check if currently speaking.
    #[cfg(feature = "ios-speech")]
    #[allow(dead_code)]
    pub fn is_speaking(&mut self) -> bool {
        self.synthesizer.is_speaking()
    }

    /// Check if currently speaking (stub when native bindings unavailable).
    #[cfg(not(feature = "ios-speech"))]
    #[allow(dead_code)]
    pub fn is_speaking(&self) -> bool {
        false
    }

    /// Check if playback is paused.
    #[cfg(feature = "ios-speech")]
    #[allow(dead_code)]
    pub fn is_paused(&mut self) -> bool {
        self.synthesizer.is_paused()
    }

    /// Check if playback is paused (stub when native bindings unavailable).
    #[cfg(not(feature = "ios-speech"))]
    #[allow(dead_code)]
    pub fn is_paused(&self) -> bool {
        false
    }
}

impl std::fmt::Debug for IosTtsProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IosTtsProvider")
            .field("active", &self.active)
            .field("voice_id", &self.voice_id)
            .field("rate", &self.rate)
            .field("pitch", &self.pitch)
            .field("volume", &self.volume)
            .field("supported_formats", &self.supported_formats)
            .field("voices_count", &self.voices.len())
            .field("audio_buffer_len", &self.audio_buffer.len())
            .field("read_position", &self.read_position)
            .finish()
    }
}

impl Default for IosTtsProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TtsProvider for IosTtsProvider {
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

        // AVSpeechSynthesizer plays audio directly to system output.
        // For buffer-based synthesis, would need AVAudioEngine integration.
        Err(TtsProviderError::Provider(
            "iOS TTS buffer synthesis not supported - use start_stream() for playback".to_string(),
        ))
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
            if self.find_voice(voice_id).is_none() {
                return Err(TtsProviderError::VoiceNotFound(voice_id.to_string()));
            }
            self.voice_id = Some(voice_id.to_string());
        }

        self.active = true;
        self.audio_buffer.clear();
        self.read_position = 0;

        #[cfg(feature = "ios-speech")]
        {
            // Use the persistent thread-safe synthesizer for playback with control
            self.synthesizer.speak(
                text,
                self.voice_id.as_deref(),
                self.rate,
                self.pitch,
                self.volume,
            );
        }

        #[cfg(not(feature = "ios-speech"))]
        {
            // Stub: synthesis not actually started
            let _ = text;
        }

        Ok(())
    }

    fn read_chunk(&mut self, _buffer: &mut [u8]) -> Result<Option<usize>, TtsProviderError> {
        if !self.active {
            return Err(TtsProviderError::NotInitialized);
        }

        // AVSpeechSynthesizer plays audio directly to system output,
        // so there's no audio data to read into a buffer.
        //
        // Check if synthesis is still in progress via the synthesizer.
        #[cfg(feature = "ios-speech")]
        {
            if self.synthesizer.is_speaking() {
                // Still playing, return empty but keep active
                return Ok(Some(0));
            }
        }

        // Synthesis complete
        self.active = false;
        Ok(None)
    }

    fn stop_stream(&mut self) {
        #[cfg(feature = "ios-speech")]
        {
            // Stop playback using the thread-safe synthesizer
            self.synthesizer.stop();
        }
        self.active = false;
        self.audio_buffer.clear();
        self.read_position = 0;
    }

    fn estimate_duration(&self, text: &str) -> Duration {
        // Rough estimate: ~150 words per minute at rate 1.0
        // Average word length ~5 characters
        let word_count = text.split_whitespace().count();
        #[allow(clippy::cast_precision_loss)]
        let base_seconds = (word_count as f64) / 2.5; // ~150 wpm
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
        assert!(caps.supports_vad);
        assert!(!caps.stt_languages.is_empty());
        assert!(!caps.tts_voices.is_empty());
    }

    #[test]
    fn test_stt_provider_lifecycle() {
        let mut stt = IosSttProvider::new();

        // Start session
        assert!(stt.start(AudioFormat::Pcm16k, Some("en-US")).is_ok());

        // Feed audio
        let audio = vec![0u8; 1024];
        assert!(stt.feed_audio(&audio).is_ok());

        // Check VAD
        assert!(stt.is_voice_active().is_some());

        // Stop session
        assert!(stt.stop().is_ok());
    }

    #[test]
    fn test_stt_unsupported_format() {
        let mut stt = IosSttProvider::new();
        let result = stt.start(AudioFormat::Opus, Some("en-US"));
        assert!(matches!(
            result,
            Err(SttProviderError::UnsupportedFormat(_))
        ));
    }

    #[test]
    fn test_stt_unsupported_language() {
        let mut stt = IosSttProvider::new();
        let result = stt.start(AudioFormat::Pcm16k, Some("xx-XX"));
        assert!(matches!(
            result,
            Err(SttProviderError::UnsupportedLanguage(_))
        ));
    }

    #[test]
    fn test_tts_provider_basics() {
        let mut tts = IosTtsProvider::new();

        // Check capabilities
        assert!(!tts.supported_formats().is_empty());
        assert!(!tts.available_voices().is_empty());

        // Test duration estimation
        let duration = tts.estimate_duration("Hello, this is a test");
        assert!(duration > Duration::ZERO);

        // Test rate/pitch/volume setters
        tts.set_rate(1.5);
        tts.set_pitch(0.8);
        tts.set_volume(0.5);
    }

    #[test]
    fn test_tts_voice_not_found() {
        let mut tts = IosTtsProvider::new();
        let result = tts.start_stream("test", AudioFormat::Pcm16k, Some("invalid.voice"));
        assert!(matches!(result, Err(TtsProviderError::VoiceNotFound(_))));
    }

    #[test]
    fn test_tts_streaming_lifecycle() {
        let mut tts = IosTtsProvider::new();

        // Start streaming
        assert!(tts.start_stream("Hello", AudioFormat::Pcm16k, None).is_ok());

        // Read chunks (stub returns None immediately)
        let mut buf = [0u8; 1024];
        let result = tts.read_chunk(&mut buf);
        assert!(result.is_ok());

        // Stop streaming
        tts.stop_stream();
    }

    #[test]
    fn test_ios_has_enhanced_voices() {
        let caps = get_capabilities();
        // iOS should have at least one enhanced voice
        let has_enhanced = caps
            .tts_voices
            .iter()
            .any(|v| v.quality == VoiceQuality::High);
        assert!(has_enhanced);
    }
}
