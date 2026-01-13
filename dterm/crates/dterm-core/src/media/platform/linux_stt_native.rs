//! Native Linux speech recognition using Vosk bindings.
//!
//! This module provides native bindings to Vosk for speech-to-text
//! on Linux. It uses the `vosk` crate for offline speech recognition.
//!
//! ## Usage
//!
//! This module is only compiled when the `linux-speech` feature is enabled
//! and the Vosk library is installed.
//!
//! ## Implementation Notes
//!
//! Vosk provides:
//! - Fully offline speech recognition
//! - Small models (~50MB for basic recognition)
//! - Support for multiple languages
//! - Real-time partial results
//! - Voice activity detection
//!
//! ## Requirements
//!
//! - Vosk library installed on the system
//! - Language model downloaded (e.g., vosk-model-small-en-us)
//!
//! ## Vosk API References
//!
//! - [Vosk API](https://alphacephei.com/vosk/)
//! - [Vosk Models](https://alphacephei.com/vosk/models)

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use vosk::{Model, Recognizer};

// ============================================================================
// Recognition result types
// ============================================================================

/// Recognition result from Vosk.
#[derive(Debug, Clone)]
pub struct RecognitionResult {
    /// The transcribed text.
    pub text: String,
    /// Confidence score (0.0 - 1.0, if available).
    pub confidence: Option<f32>,
    /// Whether this is a final result.
    pub is_final: bool,
}

/// Parsed Vosk JSON result.
#[derive(Debug)]
struct VoskResult {
    /// The recognized text.
    text: String,
}

impl VoskResult {
    /// Parse from Vosk JSON output.
    fn from_json(json: &str) -> Option<Self> {
        // Vosk returns JSON like: {"text": "hello world"}
        // or for partial: {"partial": "hello"}

        // Simple JSON parsing (avoid serde dependency)
        let text_key = "\"text\"";
        let partial_key = "\"partial\"";

        let key = if json.contains(text_key) {
            text_key
        } else if json.contains(partial_key) {
            partial_key
        } else {
            return None;
        };

        // Find the value after the key
        if let Some(key_pos) = json.find(key) {
            let after_key = &json[key_pos + key.len()..];
            // Skip whitespace and colon
            if let Some(colon_pos) = after_key.find(':') {
                let after_colon = &after_key[colon_pos + 1..];
                // Find the opening quote
                if let Some(quote_start) = after_colon.find('"') {
                    let after_quote = &after_colon[quote_start + 1..];
                    // Find the closing quote
                    if let Some(quote_end) = after_quote.find('"') {
                        let text = &after_quote[..quote_end];
                        return Some(VoskResult {
                            text: text.to_string(),
                        });
                    }
                }
            }
        }

        None
    }
}

// ============================================================================
// Model management
// ============================================================================

/// Default model paths to search for Vosk models.
const DEFAULT_MODEL_PATHS: &[&str] = &[
    // XDG data directories
    "~/.local/share/vosk/models",
    "/usr/share/vosk/models",
    "/usr/local/share/vosk/models",
    // Common installation paths
    "/opt/vosk/models",
    "models/vosk",
    // Current directory
    "./vosk-model",
];

/// Find a Vosk model for the given language.
///
/// Searches in common model directories for a matching model.
pub fn find_model_path(language: &str) -> Option<String> {
    let lang_code = language.replace('-', "_").to_lowercase();

    for base_path in DEFAULT_MODEL_PATHS {
        let expanded = shellexpand::tilde(base_path);
        let base = Path::new(expanded.as_ref());

        if !base.exists() {
            continue;
        }

        // Look for exact match first
        let exact_path = base.join(format!("vosk-model-{}", lang_code));
        if exact_path.exists() {
            return Some(exact_path.to_string_lossy().to_string());
        }

        // Try small model
        let small_path = base.join(format!("vosk-model-small-{}", lang_code));
        if small_path.exists() {
            return Some(small_path.to_string_lossy().to_string());
        }

        // Search for any model starting with the language code
        if let Ok(entries) = std::fs::read_dir(base) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(&format!("vosk-model")) && name.contains(&lang_code) {
                    return Some(entry.path().to_string_lossy().to_string());
                }
            }
        }
    }

    None
}

/// Check if Vosk is available (model exists for default language).
pub fn is_available() -> bool {
    find_model_path("en-US").is_some()
}

/// Get list of available languages based on installed models.
pub fn get_available_languages() -> Vec<String> {
    let mut languages = Vec::new();

    for base_path in DEFAULT_MODEL_PATHS {
        let expanded = shellexpand::tilde(base_path);
        let base = Path::new(expanded.as_ref());

        if !base.exists() {
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(base) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("vosk-model") {
                    // Extract language code from model name
                    // e.g., "vosk-model-small-en-us" -> "en-US"
                    if let Some(lang) = extract_language_from_model_name(&name) {
                        if !languages.contains(&lang) {
                            languages.push(lang);
                        }
                    }
                }
            }
        }
    }

    // Always include English as it's the default
    if !languages.iter().any(|l| l.starts_with("en")) {
        languages.push("en-US".to_string());
    }

    languages.sort();
    languages
}

/// Extract language code from Vosk model name.
fn extract_language_from_model_name(name: &str) -> Option<String> {
    // Model names are like: vosk-model-small-en-us, vosk-model-de-de
    let parts: Vec<&str> = name.split('-').collect();

    // Find the language code (typically last 2 parts like "en", "us")
    if parts.len() >= 2 {
        let last = parts[parts.len() - 1];
        let second_last = parts[parts.len() - 2];

        // Check if it looks like a language code
        if last.len() == 2 && second_last.len() == 2 {
            return Some(format!("{}-{}", second_last, last.to_uppercase()));
        }

        // Single language code (e.g., "en")
        if last.len() == 2 {
            return Some(format!("{}-{}", last, last.to_uppercase()));
        }
    }

    None
}

// ============================================================================
// Thread-safe recognizer wrapper
// ============================================================================

/// Thread-safe Vosk recognizer wrapper.
///
/// This wrapper provides a thread-safe interface to the Vosk recognizer,
/// handling model loading, audio processing, and result extraction.
pub struct ThreadSafeRecognizer {
    /// The Vosk model.
    model: Option<Arc<Model>>,
    /// The Vosk recognizer.
    recognizer: Option<Arc<Mutex<Recognizer>>>,
    /// Collected results.
    results: Arc<Mutex<Vec<RecognitionResult>>>,
    /// Last error.
    last_error: Arc<Mutex<Option<String>>>,
    /// Whether recognition is active.
    active: AtomicBool,
    /// Sample rate.
    sample_rate: f32,
    /// Language code.
    language: String,
}

impl ThreadSafeRecognizer {
    /// Create a new recognizer for the given language.
    ///
    /// # Arguments
    /// * `language` - Language code (e.g., "en-US")
    /// * `model_path` - Optional path to the model. If None, searches default paths.
    ///
    /// # Returns
    /// The recognizer on success, or None if the model couldn't be loaded.
    pub fn new(language: &str, model_path: Option<&str>) -> Option<Self> {
        let path = model_path
            .map(String::from)
            .or_else(|| find_model_path(language))?;

        let model = Model::new(&path).ok()?;

        Some(Self {
            model: Some(Arc::new(model)),
            recognizer: None,
            results: Arc::new(Mutex::new(Vec::new())),
            last_error: Arc::new(Mutex::new(None)),
            active: AtomicBool::new(false),
            sample_rate: 16000.0,
            language: language.to_string(),
        })
    }

    /// Create with default language (en-US).
    pub fn new_default() -> Option<Self> {
        Self::new("en-US", None)
    }

    /// Set the sample rate for audio processing.
    pub fn set_sample_rate(&mut self, rate: f32) {
        self.sample_rate = rate;
    }

    /// Check if the recognizer is available.
    pub fn is_available(&self) -> bool {
        self.model.is_some()
    }

    /// Start a recognition session.
    pub fn start(&mut self) -> Result<(), String> {
        if self.active.load(Ordering::Acquire) {
            return Err("Recognition already active".to_string());
        }

        let model = self
            .model
            .as_ref()
            .ok_or_else(|| "Model not loaded".to_string())?;

        // Create a new recognizer for this session
        let recognizer = Recognizer::new(model, self.sample_rate)
            .map_err(|e| format!("Failed to create recognizer: {:?}", e))?;

        self.recognizer = Some(Arc::new(Mutex::new(recognizer)));

        // Clear previous state
        {
            let mut results = self.results.lock().unwrap();
            results.clear();
        }
        {
            let mut error = self.last_error.lock().unwrap();
            *error = None;
        }

        self.active.store(true, Ordering::Release);
        Ok(())
    }

    /// Feed audio data to the recognizer.
    ///
    /// # Arguments
    /// * `data` - PCM audio data (16-bit signed little-endian)
    pub fn feed_audio(&mut self, data: &[u8]) -> Result<(), String> {
        if !self.active.load(Ordering::Acquire) {
            return Err("Recognition not active".to_string());
        }

        let recognizer = self
            .recognizer
            .as_ref()
            .ok_or_else(|| "Recognizer not initialized".to_string())?;

        // Convert bytes to i16 samples
        let samples: Vec<i16> = data
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();

        // Feed to recognizer
        let mut rec = recognizer.lock().unwrap();

        // accept_waveform returns the recognition state
        let _state = rec.accept_waveform(&samples);

        Ok(())
    }

    /// Get the latest partial result (if any).
    pub fn get_partial(&self) -> Option<RecognitionResult> {
        if !self.active.load(Ordering::Acquire) {
            return None;
        }

        let recognizer = self.recognizer.as_ref()?;
        let rec = recognizer.lock().unwrap();

        let json = rec.partial_result().partial;
        if json.is_empty() {
            return None;
        }

        Some(RecognitionResult {
            text: json.to_string(),
            confidence: None,
            is_final: false,
        })
    }

    /// Get the final result.
    pub fn get_final(&self) -> Option<RecognitionResult> {
        let recognizer = self.recognizer.as_ref()?;
        let mut rec = recognizer.lock().unwrap();

        let result = rec.final_result();

        // Handle different result types
        match result {
            vosk::CompleteResult::Single(single) => {
                if single.text.is_empty() {
                    return None;
                }
                Some(RecognitionResult {
                    text: single.text.to_string(),
                    confidence: None,
                    is_final: true,
                })
            }
            vosk::CompleteResult::Multiple(multiple) => {
                // Get the best alternative
                multiple.alternatives.first().map(|alt| RecognitionResult {
                    text: alt.text.to_string(),
                    confidence: Some(alt.confidence),
                    is_final: true,
                })
            }
        }
    }

    /// Stop recognition and get final result.
    pub fn stop(&mut self) -> Result<Option<RecognitionResult>, String> {
        if !self.active.load(Ordering::Acquire) {
            return Err("Recognition not active".to_string());
        }

        let result = self.get_final();

        // Store result
        if let Some(ref r) = result {
            let mut results = self.results.lock().unwrap();
            results.push(r.clone());
        }

        self.active.store(false, Ordering::Release);
        self.recognizer = None;

        Ok(result)
    }

    /// Cancel recognition without getting results.
    pub fn cancel(&mut self) {
        self.active.store(false, Ordering::Release);
        self.recognizer = None;

        let mut results = self.results.lock().unwrap();
        results.clear();
    }

    /// Check if recognition is currently active.
    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    /// Get the last error (if any).
    #[allow(dead_code)]
    pub fn get_error(&self) -> Option<String> {
        let error = self.last_error.lock().unwrap();
        error.clone()
    }
}

impl Default for ThreadSafeRecognizer {
    fn default() -> Self {
        Self::new_default().unwrap_or(Self {
            model: None,
            recognizer: None,
            results: Arc::new(Mutex::new(Vec::new())),
            last_error: Arc::new(Mutex::new(None)),
            active: AtomicBool::new(false),
            sample_rate: 16000.0,
            language: "en-US".to_string(),
        })
    }
}

impl std::fmt::Debug for ThreadSafeRecognizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThreadSafeRecognizer")
            .field("active", &self.active.load(Ordering::Relaxed))
            .field("sample_rate", &self.sample_rate)
            .field("language", &self.language)
            .field("is_available", &self.is_available())
            .finish()
    }
}

// ThreadSafeRecognizer is Send + Sync because all mutable state is protected
// by Arc<Mutex<_>> or AtomicBool.
unsafe impl Send for ThreadSafeRecognizer {}
unsafe impl Sync for ThreadSafeRecognizer {}

// ============================================================================
// Shell expansion helper (simple tilde expansion)
// ============================================================================

mod shellexpand {
    use std::borrow::Cow;
    use std::env;

    pub fn tilde(path: &str) -> Cow<'_, str> {
        if path.starts_with('~') {
            if let Ok(home) = env::var("HOME") {
                return Cow::Owned(path.replacen('~', &home, 1));
            }
        }
        Cow::Borrowed(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vosk_result_parsing() {
        let json = r#"{"text": "hello world"}"#;
        let result = VoskResult::from_json(json);
        assert!(result.is_some());
        assert_eq!(result.unwrap().text, "hello world");

        let partial = r#"{"partial": "hello"}"#;
        let result = VoskResult::from_json(partial);
        assert!(result.is_some());
        assert_eq!(result.unwrap().text, "hello");
    }

    #[test]
    fn test_language_extraction() {
        assert_eq!(
            extract_language_from_model_name("vosk-model-small-en-us"),
            Some("en-US".to_string())
        );
        assert_eq!(
            extract_language_from_model_name("vosk-model-de-de"),
            Some("de-DE".to_string())
        );
    }

    #[test]
    fn test_tilde_expansion() {
        let expanded = shellexpand::tilde("~/test");
        if std::env::var("HOME").is_ok() {
            assert!(!expanded.starts_with('~'));
        }
    }
}
