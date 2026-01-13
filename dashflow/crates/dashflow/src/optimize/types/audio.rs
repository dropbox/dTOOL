// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Audio type for audio-capable LLMs (GPT-4o-audio, Gemini, etc.)

use super::{LlmContent, ToLlmContent};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Supported audio formats for audio-capable LLMs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    /// WAV - Waveform Audio File Format (uncompressed, high quality)
    Wav,
    /// MP3 - MPEG Audio Layer III (lossy compression, widely supported)
    Mp3,
    /// FLAC - Free Lossless Audio Codec (lossless compression)
    Flac,
    /// OGG - Ogg Vorbis (lossy compression, open format)
    Ogg,
    /// WebM - WebM audio (lossy compression, web-optimized)
    Webm,
    /// M4A - MPEG-4 Audio (AAC codec, Apple ecosystem)
    M4a,
}

impl AudioFormat {
    /// Get MIME type for this format
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Wav => "audio/wav",
            Self::Mp3 => "audio/mpeg",
            Self::Flac => "audio/flac",
            Self::Ogg => "audio/ogg",
            Self::Webm => "audio/webm",
            Self::M4a => "audio/mp4",
        }
    }

    /// Get file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Mp3 => "mp3",
            Self::Flac => "flac",
            Self::Ogg => "ogg",
            Self::Webm => "webm",
            Self::M4a => "m4a",
        }
    }

    /// Detect format from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "wav" => Some(Self::Wav),
            "mp3" => Some(Self::Mp3),
            "flac" => Some(Self::Flac),
            "ogg" => Some(Self::Ogg),
            "webm" => Some(Self::Webm),
            "m4a" => Some(Self::M4a),
            _ => None,
        }
    }

    /// Detect format from MIME type
    pub fn from_mime(mime: &str) -> Option<Self> {
        match mime.to_lowercase().as_str() {
            "audio/wav" | "audio/x-wav" => Some(Self::Wav),
            "audio/mpeg" | "audio/mp3" => Some(Self::Mp3),
            "audio/flac" | "audio/x-flac" => Some(Self::Flac),
            "audio/ogg" => Some(Self::Ogg),
            "audio/webm" => Some(Self::Webm),
            "audio/mp4" | "audio/m4a" => Some(Self::M4a),
            _ => None,
        }
    }
}

/// Audio input for audio-capable LLMs
///
/// Supports base64-encoded audio for use with models like GPT-4o-audio
/// and Gemini that can process audio directly.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::optimize::types::Audio;
///
/// // From file
/// let audio = Audio::from_file("recording.wav")?;
///
/// // From bytes
/// let audio = Audio::from_bytes(&bytes, AudioFormat::Mp3);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Audio {
    /// Base64-encoded audio data
    data: String,

    /// Audio format
    format: AudioFormat,

    /// Optional duration in seconds (for metadata)
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_seconds: Option<f64>,

    /// Optional sample rate in Hz (for metadata)
    #[serde(skip_serializing_if = "Option::is_none")]
    sample_rate: Option<u32>,

    /// Optional transcript (if pre-transcribed)
    #[serde(skip_serializing_if = "Option::is_none")]
    transcript: Option<String>,
}

impl Audio {
    /// Create audio from base64-encoded data
    ///
    /// # Arguments
    /// * `data` - Base64-encoded audio data
    /// * `format` - Audio format
    pub fn from_base64(data: impl Into<String>, format: AudioFormat) -> Self {
        Self {
            data: data.into(),
            format,
            duration_seconds: None,
            sample_rate: None,
            transcript: None,
        }
    }

    /// Create audio from raw bytes
    ///
    /// # Arguments
    /// * `bytes` - Raw audio bytes
    /// * `format` - Audio format
    pub fn from_bytes(bytes: &[u8], format: AudioFormat) -> Self {
        let data = BASE64.encode(bytes);
        Self::from_base64(data, format)
    }

    /// Load audio from file path
    ///
    /// Automatically detects format from file extension.
    ///
    /// # Arguments
    /// * `path` - Path to the audio file
    ///
    /// # Returns
    /// Result with Audio or error if file cannot be read or format unknown
    pub fn from_file(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();

        let format = AudioFormat::from_extension(ext).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Unknown audio format: {}", ext),
            )
        })?;

        let bytes = std::fs::read(path)?;
        Ok(Self::from_bytes(&bytes, format))
    }

    /// Set duration metadata
    ///
    /// # Arguments
    /// * `seconds` - Duration in seconds
    #[must_use]
    pub fn with_duration(mut self, seconds: f64) -> Self {
        self.duration_seconds = Some(seconds);
        self
    }

    /// Set sample rate metadata
    ///
    /// # Arguments
    /// * `rate` - Sample rate in Hz
    #[must_use]
    pub fn with_sample_rate(mut self, rate: u32) -> Self {
        self.sample_rate = Some(rate);
        self
    }

    /// Set transcript (pre-transcribed text)
    ///
    /// # Arguments
    /// * `transcript` - Transcript text
    #[must_use]
    pub fn with_transcript(mut self, transcript: impl Into<String>) -> Self {
        self.transcript = Some(transcript.into());
        self
    }

    /// Get the base64-encoded audio data
    pub fn data(&self) -> &str {
        &self.data
    }

    /// Get the audio format
    pub fn format(&self) -> AudioFormat {
        self.format
    }

    /// Get duration if set
    pub fn duration(&self) -> Option<f64> {
        self.duration_seconds
    }

    /// Get sample rate if set
    pub fn sample_rate(&self) -> Option<u32> {
        self.sample_rate
    }

    /// Get transcript if set
    pub fn transcript(&self) -> Option<&str> {
        self.transcript.as_deref()
    }

    /// Get MIME type for this audio
    pub fn mime_type(&self) -> &'static str {
        self.format.mime_type()
    }

    /// Decode the base64 data to raw bytes
    pub fn decode(&self) -> Result<Vec<u8>, base64::DecodeError> {
        BASE64.decode(&self.data)
    }
}

impl ToLlmContent for Audio {
    fn to_content(&self) -> LlmContent {
        LlmContent::Audio {
            data: self.data.clone(),
            format: self.format.extension().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_from_base64() {
        let audio = Audio::from_base64("dGVzdA==", AudioFormat::Wav);
        assert_eq!(audio.data(), "dGVzdA==");
        assert_eq!(audio.format(), AudioFormat::Wav);
    }

    #[test]
    fn test_audio_from_bytes() {
        let bytes = b"fake audio data";
        let audio = Audio::from_bytes(bytes, AudioFormat::Mp3);
        assert_eq!(audio.format(), AudioFormat::Mp3);
        // Verify it decodes back
        let decoded = audio.decode().unwrap();
        assert_eq!(decoded, bytes);
    }

    #[test]
    fn test_audio_with_metadata() {
        let audio = Audio::from_base64("dGVzdA==", AudioFormat::Flac)
            .with_duration(120.5)
            .with_sample_rate(44100)
            .with_transcript("Hello, world!");

        assert_eq!(audio.duration(), Some(120.5));
        assert_eq!(audio.sample_rate(), Some(44100));
        assert_eq!(audio.transcript(), Some("Hello, world!"));
    }

    #[test]
    fn test_audio_format_detection() {
        assert_eq!(AudioFormat::from_extension("wav"), Some(AudioFormat::Wav));
        assert_eq!(AudioFormat::from_extension("mp3"), Some(AudioFormat::Mp3));
        assert_eq!(AudioFormat::from_extension("flac"), Some(AudioFormat::Flac));
        assert_eq!(AudioFormat::from_extension("ogg"), Some(AudioFormat::Ogg));
        assert_eq!(AudioFormat::from_extension("webm"), Some(AudioFormat::Webm));
        assert_eq!(AudioFormat::from_extension("m4a"), Some(AudioFormat::M4a));
        assert_eq!(AudioFormat::from_extension("aac"), None);
    }

    #[test]
    fn test_audio_format_mime() {
        assert_eq!(AudioFormat::Wav.mime_type(), "audio/wav");
        assert_eq!(AudioFormat::Mp3.mime_type(), "audio/mpeg");
        assert_eq!(AudioFormat::Flac.mime_type(), "audio/flac");
        assert_eq!(AudioFormat::Ogg.mime_type(), "audio/ogg");
        assert_eq!(AudioFormat::Webm.mime_type(), "audio/webm");
        assert_eq!(AudioFormat::M4a.mime_type(), "audio/mp4");
    }

    #[test]
    fn test_to_llm_content() {
        let audio = Audio::from_base64("dGVzdA==", AudioFormat::Wav);
        let content = audio.to_content();
        match content {
            LlmContent::Audio { data, format } => {
                assert_eq!(data, "dGVzdA==");
                assert_eq!(format, "wav");
            }
            _ => panic!("Expected Audio variant"),
        }
    }

    #[test]
    fn test_serialization() {
        let audio = Audio::from_base64("dGVzdA==", AudioFormat::Mp3).with_duration(60.0);

        let json = serde_json::to_string(&audio).unwrap();
        assert!(json.contains("dGVzdA=="));
        assert!(json.contains("mp3"));
        assert!(json.contains("60"));

        let deserialized: Audio = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.data(), "dGVzdA==");
        assert_eq!(deserialized.format(), AudioFormat::Mp3);
        assert_eq!(deserialized.duration(), Some(60.0));
    }
}
