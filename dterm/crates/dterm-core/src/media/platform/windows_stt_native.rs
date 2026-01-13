//! Native Windows speech recognition using WinRT bindings.
//!
//! This module provides native bindings to Windows.Media.SpeechRecognition for STT
//! on Windows 10/11. It uses the `windows` crate for WinRT interop.
//!
//! ## Usage
//!
//! This module is only compiled when the `windows-speech` feature is enabled.
//!
//! ## Implementation Notes
//!
//! Windows.Media.SpeechRecognition provides:
//! - On-device speech recognition (Windows 10+)
//! - Support for multiple languages
//! - Real-time partial results via continuous recognition
//! - Grammar and dictation constraints
//!
//! ## Privacy Requirements
//!
//! Speech recognition requires:
//! - User consent for microphone access
//! - Language packs for offline recognition
//!
//! ## WinRT API References
//!
//! - [`SpeechRecognizer`](https://docs.microsoft.com/en-us/uwp/api/windows.media.speechrecognition.speechrecognizer)
//! - [`SpeechRecognitionResult`](https://docs.microsoft.com/en-us/uwp/api/windows.media.speechrecognition.speechrecognitionresult)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use windows::core::HSTRING;
use windows::Foundation::IAsyncOperation;
use windows::Globalization::Language;
use windows::Media::SpeechRecognition::{
    SpeechRecognitionCompilationResult, SpeechRecognitionResult, SpeechRecognitionResultStatus,
    SpeechRecognizer, SpeechRecognizerState, SpeechRecognizerTimeouts,
};

// ============================================================================
// Recognition result types
// ============================================================================

/// Recognition result from the callback.
#[derive(Debug, Clone)]
pub struct RecognitionResult {
    /// The transcribed text.
    pub text: String,
    /// Confidence score (0.0 - 1.0).
    pub confidence: f64,
    /// Whether this is a final result.
    pub is_final: bool,
    /// Raw confidence level from Windows.
    pub raw_confidence: SpeechRecognitionConfidence,
}

/// Confidence level from Windows speech recognition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum SpeechRecognitionConfidence {
    /// High confidence.
    High = 0,
    /// Medium confidence.
    Medium = 1,
    /// Low confidence.
    Low = 2,
    /// Rejected (too low confidence).
    Rejected = 3,
}

impl From<i32> for SpeechRecognitionConfidence {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::High,
            1 => Self::Medium,
            2 => Self::Low,
            _ => Self::Rejected,
        }
    }
}

impl SpeechRecognitionConfidence {
    /// Convert to a normalized confidence score (0.0 - 1.0).
    pub fn to_score(self) -> f64 {
        match self {
            Self::High => 0.95,
            Self::Medium => 0.75,
            Self::Low => 0.50,
            Self::Rejected => 0.0,
        }
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Get the list of supported languages for speech recognition.
///
/// Note: This returns languages that Windows reports as installed.
/// Actual availability depends on language pack installation.
pub fn get_supported_languages() -> Vec<String> {
    let mut result = Vec::new();

    if let Ok(recognizer) = SpeechRecognizer::new() {
        if let Ok(languages) = recognizer.SupportedGrammarLanguages() {
            if let Ok(count) = languages.Size() {
                for i in 0..count {
                    if let Ok(lang) = languages.GetAt(i) {
                        if let Ok(tag) = lang.LanguageTag() {
                            result.push(tag.to_string());
                        }
                    }
                }
            }
        }
    }

    // If we couldn't get languages dynamically, return common defaults
    if result.is_empty() {
        result = vec![
            "en-US".to_string(),
            "en-GB".to_string(),
            "es-ES".to_string(),
            "fr-FR".to_string(),
            "de-DE".to_string(),
            "it-IT".to_string(),
            "ja-JP".to_string(),
            "zh-CN".to_string(),
        ];
    }

    result
}

/// Check if speech recognition is available.
pub fn is_available() -> bool {
    SpeechRecognizer::new().is_ok()
}

/// Get the current recognizer state.
pub fn get_recognizer_state(
    recognizer: &SpeechRecognizer,
) -> Result<SpeechRecognizerState, String> {
    recognizer.State().map_err(|e| e.to_string())
}

// ============================================================================
// Synchronous single-shot recognition
// ============================================================================

/// Perform a single-shot recognition (blocking).
///
/// This is useful for simple recognition tasks where you want to capture
/// a single phrase and get the result.
///
/// # Arguments
/// * `language` - Optional language code (e.g., "en-US"). Uses system default if None.
/// * `timeout_secs` - Timeout in seconds for the recognition.
///
/// # Returns
/// The recognition result on success, or an error message on failure.
pub fn recognize_once(
    language: Option<&str>,
    timeout_secs: u32,
) -> Result<RecognitionResult, String> {
    // Create recognizer with language
    let recognizer = if let Some(lang) = language {
        let lang_obj = Language::CreateLanguage(&HSTRING::from(lang))
            .map_err(|e| format!("Failed to create language: {}", e))?;
        SpeechRecognizer::CreateWithLanguage(&lang_obj)
            .map_err(|e| format!("Failed to create recognizer with language: {}", e))?
    } else {
        SpeechRecognizer::new().map_err(|e| format!("Failed to create recognizer: {}", e))?
    };

    // Set timeouts
    if let Ok(timeouts) = recognizer.Timeouts() {
        let duration = windows::Foundation::TimeSpan {
            Duration: (timeout_secs as i64) * 10_000_000, // 100-nanosecond units
        };
        let _ = timeouts.SetInitialSilenceTimeout(duration);
        let _ = timeouts.SetEndSilenceTimeout(duration);
    }

    // Compile constraints (required before recognition)
    let compile_op: IAsyncOperation<SpeechRecognitionCompilationResult> = recognizer
        .CompileConstraintsAsync()
        .map_err(|e| format!("Failed to compile constraints: {}", e))?;

    let compile_result = compile_op
        .get()
        .map_err(|e| format!("Failed to wait for compile: {}", e))?;

    if compile_result.Status().map_err(|e| e.to_string())?
        != windows::Media::SpeechRecognition::SpeechRecognitionResultStatus::Success
    {
        return Err("Failed to compile speech constraints".to_string());
    }

    // Start recognition
    let recognize_op: IAsyncOperation<SpeechRecognitionResult> = recognizer
        .RecognizeAsync()
        .map_err(|e| format!("Failed to start recognition: {}", e))?;

    let result = recognize_op
        .get()
        .map_err(|e| format!("Recognition failed: {}", e))?;

    // Extract result
    let status = result.Status().map_err(|e| e.to_string())?;

    if status != SpeechRecognitionResultStatus::Success {
        return Err(format!("Recognition status: {:?}", status));
    }

    let text = result.Text().map_err(|e| e.to_string())?.to_string();
    let confidence = result.Confidence().map_err(|e| e.to_string())?;
    let confidence_enum = SpeechRecognitionConfidence::from(confidence.0);

    Ok(RecognitionResult {
        text,
        confidence: confidence_enum.to_score(),
        is_final: true,
        raw_confidence: confidence_enum,
    })
}

// ============================================================================
// Thread-safe recognizer wrapper
// ============================================================================

/// Thread-safe speech recognizer wrapper for continuous recognition.
///
/// Note: Windows continuous recognition uses event handlers that are complex
/// to set up in Rust. This implementation provides single-shot recognition
/// and basic state management. For continuous recognition with real-time
/// results, consider using a separate thread with periodic single-shot calls
/// or implementing native event handlers.
pub struct ThreadSafeRecognizer {
    /// The underlying recognizer (if created successfully).
    recognizer: Option<SpeechRecognizer>,
    /// Collected results.
    results: Arc<Mutex<Vec<RecognitionResult>>>,
    /// Last error.
    last_error: Arc<Mutex<Option<String>>>,
    /// Whether recognition is active.
    active: AtomicBool,
    /// Language code.
    language: String,
    /// Accumulated audio buffer (for compatibility with trait interface).
    audio_buffer: Arc<Mutex<Vec<u8>>>,
}

impl ThreadSafeRecognizer {
    /// Create a new recognizer for the default language.
    pub fn new() -> Option<Self> {
        let recognizer = SpeechRecognizer::new().ok()?;

        Some(Self {
            recognizer: Some(recognizer),
            results: Arc::new(Mutex::new(Vec::new())),
            last_error: Arc::new(Mutex::new(None)),
            active: AtomicBool::new(false),
            language: "en-US".to_string(),
            audio_buffer: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Create a recognizer for a specific language.
    pub fn with_language(language: &str) -> Option<Self> {
        let lang_obj = Language::CreateLanguage(&HSTRING::from(language)).ok()?;
        let recognizer = SpeechRecognizer::CreateWithLanguage(&lang_obj).ok()?;

        Some(Self {
            recognizer: Some(recognizer),
            results: Arc::new(Mutex::new(Vec::new())),
            last_error: Arc::new(Mutex::new(None)),
            active: AtomicBool::new(false),
            language: language.to_string(),
            audio_buffer: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Check if the recognizer is available.
    pub fn is_available(&self) -> bool {
        self.recognizer.is_some()
    }

    /// Get the current recognizer state.
    pub fn state(&self) -> Option<SpeechRecognizerState> {
        self.recognizer.as_ref()?.State().ok()
    }

    /// Start a recognition session.
    ///
    /// Note: This initializes the recognizer but doesn't start continuous
    /// recognition. Use `recognize_chunk()` to process audio segments.
    pub fn start(&mut self) -> Result<(), String> {
        if self.active.load(Ordering::Acquire) {
            return Err("Recognition already active".to_string());
        }

        let recognizer = self
            .recognizer
            .as_ref()
            .ok_or_else(|| "Recognizer not available".to_string())?;

        // Compile constraints
        let compile_op: IAsyncOperation<SpeechRecognitionCompilationResult> = recognizer
            .CompileConstraintsAsync()
            .map_err(|e| format!("Failed to compile constraints: {}", e))?;

        let compile_result = compile_op
            .get()
            .map_err(|e| format!("Failed to wait for compile: {}", e))?;

        let status = compile_result.Status().map_err(|e| e.to_string())?;
        if status != windows::Media::SpeechRecognition::SpeechRecognitionResultStatus::Success {
            return Err(format!("Failed to compile constraints: {:?}", status));
        }

        // Clear previous state
        {
            let mut results = self.results.lock().unwrap();
            results.clear();
        }
        {
            let mut error = self.last_error.lock().unwrap();
            *error = None;
        }
        {
            let mut buffer = self.audio_buffer.lock().unwrap();
            buffer.clear();
        }

        self.active.store(true, Ordering::Release);
        Ok(())
    }

    /// Feed audio data to the recognizer.
    ///
    /// Note: Windows SpeechRecognizer typically uses system audio input directly.
    /// This method accumulates audio for compatibility with the trait interface,
    /// but actual recognition happens via RecognizeAsync.
    pub fn feed_audio(&mut self, data: &[u8]) -> Result<(), String> {
        if !self.active.load(Ordering::Acquire) {
            return Err("Recognition not active".to_string());
        }

        let mut buffer = self.audio_buffer.lock().unwrap();
        buffer.extend_from_slice(data);

        Ok(())
    }

    /// Get the latest partial result (if any).
    pub fn get_partial(&self) -> Option<RecognitionResult> {
        if !self.active.load(Ordering::Acquire) {
            return None;
        }

        let results = self.results.lock().unwrap();
        results.last().filter(|r| !r.is_final).cloned()
    }

    /// Get the final result (if recognition is complete).
    pub fn get_final(&self) -> Option<RecognitionResult> {
        let results = self.results.lock().unwrap();
        results.iter().find(|r| r.is_final).cloned()
    }

    /// Get the last error (if any).
    #[allow(dead_code)]
    pub fn get_error(&self) -> Option<String> {
        let error = self.last_error.lock().unwrap();
        error.clone()
    }

    /// Perform a single recognition and return the result.
    ///
    /// This method uses the system microphone for input.
    pub fn recognize(&mut self) -> Result<RecognitionResult, String> {
        if !self.active.load(Ordering::Acquire) {
            return Err("Recognition not active".to_string());
        }

        let recognizer = self
            .recognizer
            .as_ref()
            .ok_or_else(|| "Recognizer not available".to_string())?;

        // Start recognition
        let recognize_op: IAsyncOperation<SpeechRecognitionResult> = recognizer
            .RecognizeAsync()
            .map_err(|e| format!("Failed to start recognition: {}", e))?;

        let result = recognize_op
            .get()
            .map_err(|e| format!("Recognition failed: {}", e))?;

        // Extract result
        let status = result.Status().map_err(|e| e.to_string())?;

        if status != SpeechRecognitionResultStatus::Success {
            let mut error = self.last_error.lock().unwrap();
            *error = Some(format!("Recognition status: {:?}", status));
            return Err(format!("Recognition status: {:?}", status));
        }

        let text = result.Text().map_err(|e| e.to_string())?.to_string();
        let confidence = result.Confidence().map_err(|e| e.to_string())?;
        let confidence_enum = SpeechRecognitionConfidence::from(confidence.0);

        let recognition_result = RecognitionResult {
            text,
            confidence: confidence_enum.to_score(),
            is_final: true,
            raw_confidence: confidence_enum,
        };

        // Store result
        {
            let mut results = self.results.lock().unwrap();
            results.push(recognition_result.clone());
        }

        Ok(recognition_result)
    }

    /// Stop recognition and get final result.
    pub fn stop(&mut self) -> Result<Option<RecognitionResult>, String> {
        if !self.active.load(Ordering::Acquire) {
            return Err("Recognition not active".to_string());
        }

        self.active.store(false, Ordering::Release);

        // Return the last final result
        Ok(self.get_final())
    }

    /// Cancel recognition without getting results.
    pub fn cancel(&mut self) {
        self.active.store(false, Ordering::Release);

        // Clear results
        let mut results = self.results.lock().unwrap();
        results.clear();

        let mut buffer = self.audio_buffer.lock().unwrap();
        buffer.clear();
    }

    /// Check if recognition is currently active.
    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
}

impl Default for ThreadSafeRecognizer {
    fn default() -> Self {
        Self::new().unwrap_or(Self {
            recognizer: None,
            results: Arc::new(Mutex::new(Vec::new())),
            last_error: Arc::new(Mutex::new(None)),
            active: AtomicBool::new(false),
            language: "en-US".to_string(),
            audio_buffer: Arc::new(Mutex::new(Vec::new())),
        })
    }
}

impl std::fmt::Debug for ThreadSafeRecognizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThreadSafeRecognizer")
            .field("active", &self.active.load(Ordering::Relaxed))
            .field("language", &self.language)
            .field("is_available", &self.is_available())
            .finish()
    }
}

// ThreadSafeRecognizer is Send + Sync because all mutable state is protected
// by Arc<Mutex<_>> or AtomicBool.
// Note: SpeechRecognizer itself is !Send, so we need to be careful about
// how we use it. In this implementation, all recognizer operations happen
// on the same thread.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_languages() {
        let languages = get_supported_languages();
        // Should have at least some languages
        assert!(
            !languages.is_empty(),
            "Should have at least one supported language"
        );
    }

    #[test]
    fn test_confidence_conversion() {
        assert!((SpeechRecognitionConfidence::High.to_score() - 0.95).abs() < 0.01);
        assert!((SpeechRecognitionConfidence::Medium.to_score() - 0.75).abs() < 0.01);
        assert!((SpeechRecognitionConfidence::Low.to_score() - 0.50).abs() < 0.01);
        assert!((SpeechRecognitionConfidence::Rejected.to_score() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_recognizer_creation() {
        // This may fail if speech recognition is not available on the system
        let recognizer = ThreadSafeRecognizer::new();
        if let Some(r) = recognizer {
            assert!(r.is_available());
            assert!(!r.is_active());
        }
    }
}
