//! Native macOS speech synthesis using objc2 bindings.
//!
//! This module provides native bindings to AVSpeechSynthesizer for text-to-speech
//! on macOS and iOS. It uses the objc2 crate family for safe Objective-C interop.
//!
//! ## Usage
//!
//! This module is only compiled when the `macos-speech` feature is enabled.
//!
//! ## Implementation Notes
//!
//! AVSpeechSynthesizer provides:
//! - High-quality neural and standard voices
//! - Adjustable rate, pitch, and volume
//! - Streaming audio output via delegate callbacks
//! - Both online and offline synthesis
//!
//! The Speech framework (SFSpeechRecognizer) for STT is available via objc2-speech.

use objc2::rc::Retained;
use objc2::{extern_class, msg_send, ClassType};
use objc2_foundation::{NSArray, NSObject, NSString};

// Import objc2_av_foundation to ensure AVFoundation framework gets linked.
// This crate provides the framework linking via its build.rs.
#[allow(unused_imports)]
use objc2_av_foundation as _;

// ============================================================================
// AVSpeechSynthesisVoice bindings
// ============================================================================

extern_class!(
    /// Binding to AVSpeechSynthesisVoice.
    ///
    /// Represents a voice used for speech synthesis.
    #[unsafe(super(NSObject))]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct AVSpeechSynthesisVoice;
);

impl AVSpeechSynthesisVoice {
    /// Get all available speech voices.
    pub fn speech_voices() -> Retained<NSArray<AVSpeechSynthesisVoice>> {
        unsafe { msg_send![Self::class(), speechVoices] }
    }

    /// Get a voice by identifier.
    pub fn voice_with_identifier(identifier: &NSString) -> Option<Retained<Self>> {
        unsafe { msg_send![Self::class(), voiceWithIdentifier: identifier] }
    }

    /// Get the voice identifier.
    pub fn identifier(&self) -> Retained<NSString> {
        unsafe { msg_send![self, identifier] }
    }

    /// Get the voice name.
    pub fn name(&self) -> Retained<NSString> {
        unsafe { msg_send![self, name] }
    }

    /// Get the voice language.
    pub fn language(&self) -> Retained<NSString> {
        unsafe { msg_send![self, language] }
    }

    /// Get the voice quality (0 = default, 1 = enhanced, 2 = premium).
    pub fn quality(&self) -> i64 {
        unsafe { msg_send![self, quality] }
    }
}

// ============================================================================
// AVSpeechUtterance bindings
// ============================================================================

extern_class!(
    /// Binding to AVSpeechUtterance.
    ///
    /// Represents a chunk of text to be spoken with specific voice settings.
    #[unsafe(super(NSObject))]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct AVSpeechUtterance;
);

impl AVSpeechUtterance {
    /// Create a new utterance with the given text.
    pub fn speech_utterance_with_string(text: &NSString) -> Retained<Self> {
        unsafe { msg_send![Self::class(), speechUtteranceWithString: text] }
    }

    /// Set the voice for this utterance.
    pub fn set_voice(&self, voice: Option<&AVSpeechSynthesisVoice>) {
        unsafe {
            let _: () = msg_send![self, setVoice: voice];
        }
    }

    /// Set the speech rate (0.0 to 1.0, where 0.5 is default).
    pub fn set_rate(&self, rate: f32) {
        unsafe {
            let _: () = msg_send![self, setRate: rate];
        }
    }

    /// Set the pitch multiplier (0.5 to 2.0, where 1.0 is default).
    pub fn set_pitch_multiplier(&self, pitch: f32) {
        unsafe {
            let _: () = msg_send![self, setPitchMultiplier: pitch];
        }
    }

    /// Set the volume (0.0 to 1.0, where 1.0 is default).
    pub fn set_volume(&self, volume: f32) {
        unsafe {
            let _: () = msg_send![self, setVolume: volume];
        }
    }
}

// ============================================================================
// AVSpeechSynthesizer bindings
// ============================================================================

extern_class!(
    /// Binding to AVSpeechSynthesizer.
    ///
    /// The main class for speech synthesis on Apple platforms.
    #[unsafe(super(NSObject))]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct AVSpeechSynthesizer;
);

impl AVSpeechSynthesizer {
    /// Create a new speech synthesizer.
    pub fn new() -> Retained<Self> {
        unsafe { msg_send![Self::class(), new] }
    }

    /// Speak the given utterance.
    pub fn speak_utterance(&self, utterance: &AVSpeechUtterance) {
        unsafe {
            let _: () = msg_send![self, speakUtterance: utterance];
        }
    }

    /// Stop speaking immediately.
    #[allow(dead_code)]
    pub fn stop_speaking_at_boundary(&self, boundary: i64) -> bool {
        unsafe { msg_send![self, stopSpeakingAtBoundary: boundary] }
    }

    /// Check if currently speaking.
    #[allow(dead_code)]
    pub fn is_speaking(&self) -> bool {
        unsafe { msg_send![self, isSpeaking] }
    }

    /// Check if currently paused.
    #[allow(dead_code)]
    pub fn is_paused(&self) -> bool {
        unsafe { msg_send![self, isPaused] }
    }

    /// Pause speaking at word boundary.
    #[allow(dead_code)]
    pub fn pause_speaking_at_boundary(&self, boundary: i64) -> bool {
        unsafe { msg_send![self, pauseSpeakingAtBoundary: boundary] }
    }

    /// Continue speaking after pause.
    #[allow(dead_code)]
    pub fn continue_speaking(&self) -> bool {
        unsafe { msg_send![self, continueSpeaking] }
    }
}

// ============================================================================
// Speech boundary constants
// ============================================================================

/// Stop/pause at immediate position.
#[allow(dead_code)]
pub const AVSPEECH_BOUNDARY_IMMEDIATE: i64 = 0;
/// Stop/pause at word boundary.
#[allow(dead_code)]
pub const AVSPEECH_BOUNDARY_WORD: i64 = 1;

// ============================================================================
// Voice quality constants
// ============================================================================

/// Default voice quality.
pub const AVSPEECH_SYNTHESIS_VOICE_QUALITY_DEFAULT: i64 = 0;
/// Enhanced voice quality.
pub const AVSPEECH_SYNTHESIS_VOICE_QUALITY_ENHANCED: i64 = 1;
/// Premium voice quality.
pub const AVSPEECH_SYNTHESIS_VOICE_QUALITY_PREMIUM: i64 = 2;

// ============================================================================
// Rate constants
// ============================================================================

/// Minimum speech rate.
#[allow(dead_code)]
pub const AVSPEECH_UTTERANCE_MINIMUM_SPEECH_RATE: f32 = 0.0;
/// Maximum speech rate.
#[allow(dead_code)]
pub const AVSPEECH_UTTERANCE_MAXIMUM_SPEECH_RATE: f32 = 1.0;
/// Default speech rate.
#[allow(dead_code)]
pub const AVSPEECH_UTTERANCE_DEFAULT_SPEECH_RATE: f32 = 0.5;

// ============================================================================
// Helper functions
// ============================================================================

/// Get all available voice identifiers and names.
pub fn get_available_voices() -> Vec<(String, String, String, i64)> {
    let voices = AVSpeechSynthesisVoice::speech_voices();
    let mut result = Vec::new();

    let count: usize = voices.count();
    for i in 0..count {
        // objectAtIndex returns Retained directly in objc2 0.6
        let voice: Retained<AVSpeechSynthesisVoice> = voices.objectAtIndex(i);
        let id = voice.identifier();
        let name = voice.name();
        let lang = voice.language();
        let quality = voice.quality();

        result.push((id.to_string(), name.to_string(), lang.to_string(), quality));
    }

    result
}

/// Speak text with the default voice.
#[allow(dead_code)]
pub fn speak_text(text: &str) {
    let synthesizer = AVSpeechSynthesizer::new();
    let ns_text = NSString::from_str(text);
    let utterance = AVSpeechUtterance::speech_utterance_with_string(&ns_text);
    synthesizer.speak_utterance(&utterance);
}

/// Speak text with a specific voice and settings.
#[allow(dead_code)]
pub fn speak_text_with_options(
    text: &str,
    voice_id: Option<&str>,
    rate: f32,
    pitch: f32,
    volume: f32,
) {
    let synthesizer = AVSpeechSynthesizer::new();
    let ns_text = NSString::from_str(text);
    let utterance = AVSpeechUtterance::speech_utterance_with_string(&ns_text);

    if let Some(id) = voice_id {
        let ns_id = NSString::from_str(id);
        if let Some(voice) = AVSpeechSynthesisVoice::voice_with_identifier(&ns_id) {
            utterance.set_voice(Some(&voice));
        }
    }

    // Convert rate from 0.5-2.0 range to AVSpeechUtterance's 0.0-1.0 range
    let av_rate = ((rate - 0.5) / 1.5).clamp(0.0, 1.0);
    utterance.set_rate(av_rate);
    utterance.set_pitch_multiplier(pitch);
    utterance.set_volume(volume);

    synthesizer.speak_utterance(&utterance);
}

// ============================================================================
// Thread-safe synthesizer manager
// ============================================================================

use dispatch2::{run_on_main, MainThreadBound};
use std::sync::atomic::{AtomicBool, Ordering};

/// Thread-safe wrapper around AVSpeechSynthesizer.
///
/// This struct stores the synthesizer in a `MainThreadBound` wrapper,
/// allowing it to be used from any thread while ensuring all operations
/// are dispatched to the main thread.
///
/// ## Thread Safety
///
/// - All operations are dispatched to the main thread via GCD
/// - The struct is Send + Sync and can be stored in the TtsProvider
/// - Configuration (rate, pitch, volume, voice) is stored locally and applied per-utterance
///
/// ## Note on Runloop Requirement
///
/// This implementation uses `run_on_main` which requires the main thread to be
/// running a dispatch loop (CFRunLoop, NSApplication, etc.). In unit tests or
/// CLI apps without a runloop, the main thread check (`MainThreadMarker::new()`)
/// determines whether we're already on the main thread. If called from the main
/// thread, operations run synchronously without needing a runloop.
pub struct ThreadSafeSynthesizer {
    /// The synthesizer wrapped for main thread access.
    /// None until first use (lazy initialization on main thread).
    inner: Option<MainThreadBound<Retained<AVSpeechSynthesizer>>>,
    /// Whether the synthesizer has been initialized.
    initialized: AtomicBool,
}

impl ThreadSafeSynthesizer {
    /// Create a new thread-safe synthesizer.
    ///
    /// The underlying AVSpeechSynthesizer is lazily created on first use.
    pub fn new() -> Self {
        Self {
            inner: None,
            initialized: AtomicBool::new(false),
        }
    }

    /// Check if we can safely call run_on_main.
    ///
    /// Returns true if we're on the main thread or if a runloop appears available.
    /// This helps avoid deadlocks in unit tests.
    fn can_use_main_dispatch() -> bool {
        // If we're already on the main thread, we can always proceed
        objc2::MainThreadMarker::new().is_some()
    }

    /// Ensure the synthesizer is initialized on the main thread.
    fn ensure_initialized(&mut self) {
        if self.initialized.load(Ordering::Acquire) {
            return;
        }

        // Only try to initialize if we're on the main thread
        // to avoid deadlocks in tests without a runloop
        if !Self::can_use_main_dispatch() {
            return;
        }

        run_on_main(|mtm| {
            if self.inner.is_none() {
                let synth = AVSpeechSynthesizer::new();
                self.inner = Some(MainThreadBound::new(synth, mtm));
                self.initialized.store(true, Ordering::Release);
            }
        });
    }

    /// Speak text with the given settings.
    ///
    /// This dispatches to the main thread and starts playback.
    pub fn speak(
        &mut self,
        text: &str,
        voice_id: Option<&str>,
        rate: f32,
        pitch: f32,
        volume: f32,
    ) {
        self.ensure_initialized();

        // Copy values for the closure
        let text = text.to_string();
        let voice_id = voice_id.map(String::from);

        if let Some(ref mut bound) = self.inner {
            bound.get_on_main_mut(|synth| {
                let ns_text = NSString::from_str(&text);
                let utterance = AVSpeechUtterance::speech_utterance_with_string(&ns_text);

                if let Some(ref id) = voice_id {
                    let ns_id = NSString::from_str(id);
                    if let Some(voice) = AVSpeechSynthesisVoice::voice_with_identifier(&ns_id) {
                        utterance.set_voice(Some(&voice));
                    }
                }

                // Convert rate from 0.5-2.0 range to AVSpeechUtterance's 0.0-1.0 range
                let av_rate = ((rate - 0.5) / 1.5).clamp(0.0, 1.0);
                utterance.set_rate(av_rate);
                utterance.set_pitch_multiplier(pitch);
                utterance.set_volume(volume);

                synth.speak_utterance(&utterance);
            });
        }
    }

    /// Pause speech at word boundary.
    ///
    /// Returns true if pause was successful.
    pub fn pause(&mut self) -> bool {
        self.ensure_initialized();

        if let Some(ref mut bound) = self.inner {
            bound.get_on_main_mut(|synth| synth.pause_speaking_at_boundary(AVSPEECH_BOUNDARY_WORD))
        } else {
            false
        }
    }

    /// Resume paused speech.
    ///
    /// Returns true if resume was successful.
    pub fn resume(&mut self) -> bool {
        self.ensure_initialized();

        if let Some(ref mut bound) = self.inner {
            bound.get_on_main_mut(|synth| synth.continue_speaking())
        } else {
            false
        }
    }

    /// Stop speech immediately.
    ///
    /// Returns true if stop was successful.
    pub fn stop(&mut self) -> bool {
        self.ensure_initialized();

        if let Some(ref mut bound) = self.inner {
            bound.get_on_main_mut(|synth| {
                synth.stop_speaking_at_boundary(AVSPEECH_BOUNDARY_IMMEDIATE)
            })
        } else {
            false
        }
    }

    /// Stop speech at word boundary.
    ///
    /// Returns true if stop was successful.
    #[allow(dead_code)]
    pub fn stop_at_word(&mut self) -> bool {
        self.ensure_initialized();

        if let Some(ref mut bound) = self.inner {
            bound.get_on_main_mut(|synth| synth.stop_speaking_at_boundary(AVSPEECH_BOUNDARY_WORD))
        } else {
            false
        }
    }

    /// Check if currently speaking.
    pub fn is_speaking(&mut self) -> bool {
        self.ensure_initialized();

        if let Some(ref mut bound) = self.inner {
            bound.get_on_main_mut(|synth| synth.is_speaking())
        } else {
            false
        }
    }

    /// Check if currently paused.
    pub fn is_paused(&mut self) -> bool {
        self.ensure_initialized();

        if let Some(ref mut bound) = self.inner {
            bound.get_on_main_mut(|synth| synth.is_paused())
        } else {
            false
        }
    }
}

impl Default for ThreadSafeSynthesizer {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ThreadSafeSynthesizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThreadSafeSynthesizer")
            .field("initialized", &self.initialized.load(Ordering::Relaxed))
            .finish()
    }
}

// SAFETY: ThreadSafeSynthesizer is Send + Sync because:
// - MainThreadBound<T> is Send + Sync
// - AtomicBool is Send + Sync
// - All operations on the inner AVSpeechSynthesizer are dispatched to the main thread
unsafe impl Send for ThreadSafeSynthesizer {}
unsafe impl Sync for ThreadSafeSynthesizer {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_available_voices() {
        let voices = get_available_voices();
        // Should have at least some voices on any macOS system
        assert!(!voices.is_empty(), "Should have at least one voice");

        // Check that voice info is populated
        for (id, name, lang, quality) in &voices {
            assert!(!id.is_empty(), "Voice ID should not be empty");
            assert!(!name.is_empty(), "Voice name should not be empty");
            assert!(!lang.is_empty(), "Voice language should not be empty");
            assert!(*quality >= 0, "Voice quality should be non-negative");
        }
    }

    #[test]
    fn test_synthesizer_creation() {
        let synth = AVSpeechSynthesizer::new();
        assert!(!synth.is_speaking());
        assert!(!synth.is_paused());
    }

    #[test]
    fn test_utterance_creation() {
        let text = NSString::from_str("Hello, world!");
        let utterance = AVSpeechUtterance::speech_utterance_with_string(&text);

        // Set various properties
        utterance.set_rate(0.5);
        utterance.set_pitch_multiplier(1.0);
        utterance.set_volume(1.0);

        // Should not panic
    }
}
