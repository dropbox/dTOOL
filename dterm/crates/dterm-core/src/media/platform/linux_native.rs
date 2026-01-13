//! Native Linux speech synthesis using espeak-ng bindings.
//!
//! This module provides native bindings to espeak-ng for text-to-speech
//! on Linux. It uses the `espeakng` crate for safe Rust bindings.
//!
//! ## Usage
//!
//! This module is only compiled when the `linux-speech` feature is enabled.
//!
//! ## Implementation Notes
//!
//! espeak-ng provides:
//! - Fast, lightweight TTS with many voices
//! - Offline synthesis (no network required)
//! - Support for 100+ languages
//! - Adjustable rate, pitch, and volume
//!
//! ## espeak-ng References
//!
//! - [espeak-ng GitHub](https://github.com/espeak-ng/espeak-ng)
//! - [espeakng crate](https://docs.rs/espeakng)

use std::sync::OnceLock;

use espeakng::{initialise, Speaker};
use parking_lot::Mutex;

use super::{VoiceGender, VoiceInfo, VoiceQuality};

// ============================================================================
// Global speaker singleton
// ============================================================================

/// Global speaker instance.
///
/// espeak-ng uses global state, so we maintain a single locked instance.
static SPEAKER: OnceLock<Mutex<Speaker>> = OnceLock::new();

/// Get or initialize the global speaker.
fn get_speaker() -> Result<&'static Mutex<Speaker>, String> {
    SPEAKER.get_or_try_init(|| {
        let speaker =
            initialise(None).map_err(|e| format!("Failed to initialize espeak-ng: {e}"))?;
        Ok(speaker)
    })
}

// ============================================================================
// Voice enumeration
// ============================================================================

/// Get all available TTS voices from espeak-ng.
///
/// Returns a vector of (id, name, language, gender) tuples.
pub fn get_available_voices() -> Vec<(String, String, String, Option<VoiceGender>)> {
    let mut result = Vec::new();

    let speaker = match get_speaker() {
        Ok(s) => s,
        Err(_) => return result,
    };

    let locked = speaker.lock();
    if let Ok(voices) = locked.get_voices() {
        for voice in voices {
            let id = voice.identifier.clone();
            let name = voice.name.clone();
            let lang = voice.languages.first().cloned().unwrap_or_default();

            // espeak-ng uses 1 for male, 2 for female
            let gender = match voice.gender {
                1 => Some(VoiceGender::Male),
                2 => Some(VoiceGender::Female),
                _ => None,
            };

            result.push((id, name, lang, gender));
        }
    }

    result
}

/// Convert espeak-ng voices to VoiceInfo structs.
pub fn get_voice_info_list() -> Vec<VoiceInfo> {
    get_available_voices()
        .into_iter()
        .map(|(id, name, lang, gender)| VoiceInfo {
            id,
            name,
            language: lang,
            gender,
            // espeak-ng voices are basic quality (formant synthesis)
            quality: VoiceQuality::Basic,
        })
        .collect()
}

// ============================================================================
// Speech synthesis
// ============================================================================

/// Synthesize text to audio samples using espeak-ng.
///
/// # Arguments
/// * `text` - The text to synthesize
/// * `voice_id` - Optional voice identifier to use
///
/// # Returns
/// Audio data as PCM 16-bit samples (22050 Hz mono) on success, or error message on failure.
pub fn synthesize_text(text: &str, voice_id: Option<&str>) -> Result<Vec<i16>, String> {
    let speaker = get_speaker()?;
    let mut locked = speaker.lock();

    // Set voice if specified
    if let Some(voice_id) = voice_id {
        locked
            .set_voice_raw(voice_id)
            .map_err(|e| format!("Failed to set voice: {e}"))?;
    }

    // Synthesize text to audio samples
    let samples = locked
        .synthesize(text)
        .map_err(|e| format!("Failed to synthesize: {e}"))?;

    Ok(samples)
}

/// Synthesize text with voice and options.
///
/// # Arguments
/// * `text` - The text to synthesize
/// * `voice_id` - Optional voice identifier to use
/// * `rate` - Speech rate (0.5-2.0, where 1.0 is normal)
/// * `pitch` - Pitch multiplier (0.5-2.0, where 1.0 is normal)
/// * `volume` - Volume (0.0-1.0)
///
/// # Returns
/// Audio data as PCM 16-bit samples (22050 Hz mono) on success, or error message on failure.
pub fn synthesize_with_options(
    text: &str,
    voice_id: Option<&str>,
    rate: f32,
    pitch: f32,
    volume: f32,
) -> Result<Vec<i16>, String> {
    let speaker = get_speaker()?;
    let mut locked = speaker.lock();

    // Set voice if specified
    if let Some(voice_id) = voice_id {
        locked
            .set_voice_raw(voice_id)
            .map_err(|e| format!("Failed to set voice: {e}"))?;
    }

    // Set parameters using espeak-ng parameter indices
    // Rate: espeak-ng uses words-per-minute (80-450, default 175)
    // Convert from 0.5-2.0 range to WPM
    let wpm = ((rate * 175.0).clamp(80.0, 450.0)) as i32;
    locked
        .set_parameter(espeakng::Parameter::Rate, wpm, 0)
        .map_err(|e| format!("Failed to set rate: {e}"))?;

    // Pitch: espeak-ng uses 0-100, default 50
    // Convert from 0.5-2.0 range to 0-100
    let espeak_pitch = (((pitch - 0.5) / 1.5) * 100.0).clamp(0.0, 100.0) as i32;
    locked
        .set_parameter(espeakng::Parameter::Pitch, espeak_pitch, 0)
        .map_err(|e| format!("Failed to set pitch: {e}"))?;

    // Volume: espeak-ng uses 0-200, default 100
    // Convert from 0.0-1.0 range to 0-200
    let espeak_volume = (volume * 200.0).clamp(0.0, 200.0) as i32;
    locked
        .set_parameter(espeakng::Parameter::Volume, espeak_volume, 0)
        .map_err(|e| format!("Failed to set volume: {e}"))?;

    // Synthesize text to audio samples
    let samples = locked
        .synthesize(text)
        .map_err(|e| format!("Failed to synthesize: {e}"))?;

    Ok(samples)
}

/// Convert PCM i16 samples to bytes (little-endian).
///
/// espeak-ng outputs 22050 Hz mono PCM data. This function converts
/// the i16 samples to raw bytes for compatibility with the TTS trait.
pub fn samples_to_bytes(samples: &[i16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

/// Get the default voice for the current system.
pub fn get_default_voice() -> Option<VoiceInfo> {
    let speaker = get_speaker().ok()?;
    let locked = speaker.lock();

    if let Ok(voice) = locked.get_current_voice() {
        let lang = voice.languages.first().cloned().unwrap_or_default();
        let gender = match voice.gender {
            1 => Some(VoiceGender::Male),
            2 => Some(VoiceGender::Female),
            _ => None,
        };

        return Some(VoiceInfo {
            id: voice.identifier.clone(),
            name: voice.name.clone(),
            language: lang,
            gender,
            quality: VoiceQuality::Basic,
        });
    }

    None
}

/// Get library info (version and voice path).
pub fn get_library_info() -> Option<(String, String)> {
    let speaker = get_speaker().ok()?;
    let locked = speaker.lock();

    if let Ok(info) = locked.info() {
        Some((info.version, info.voice_path))
    } else {
        None
    }
}

/// espeak-ng output sample rate (Hz).
pub const ESPEAK_SAMPLE_RATE: u32 = 22050;

/// espeak-ng output format is mono.
pub const ESPEAK_CHANNELS: u32 = 1;

/// espeak-ng output bits per sample.
pub const ESPEAK_BITS_PER_SAMPLE: u32 = 16;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_samples_to_bytes() {
        let samples: Vec<i16> = vec![0, 1, -1, i16::MAX, i16::MIN];
        let bytes = samples_to_bytes(&samples);

        assert_eq!(bytes.len(), samples.len() * 2);

        // Check first sample (0)
        assert_eq!(bytes[0], 0);
        assert_eq!(bytes[1], 0);

        // Check second sample (1)
        assert_eq!(bytes[2], 1);
        assert_eq!(bytes[3], 0);

        // Check third sample (-1 = 0xFFFF in two's complement)
        assert_eq!(bytes[4], 0xFF);
        assert_eq!(bytes[5], 0xFF);
    }

    #[test]
    fn test_constants() {
        assert_eq!(ESPEAK_SAMPLE_RATE, 22050);
        assert_eq!(ESPEAK_CHANNELS, 1);
        assert_eq!(ESPEAK_BITS_PER_SAMPLE, 16);
    }

    // Note: Tests that require espeak-ng installed are marked with #[ignore]
    // Run with: cargo test --features linux-speech -- --ignored

    #[test]
    #[ignore = "requires espeak-ng installed"]
    fn test_get_available_voices() {
        let voices = get_available_voices();
        assert!(!voices.is_empty(), "Should have at least one voice");

        for (id, name, _lang, _gender) in &voices {
            assert!(!id.is_empty(), "Voice ID should not be empty");
            assert!(!name.is_empty(), "Voice name should not be empty");
        }
    }

    #[test]
    #[ignore = "requires espeak-ng installed"]
    fn test_get_voice_info_list() {
        let voices = get_voice_info_list();
        assert!(!voices.is_empty(), "Should have at least one voice");

        for voice in &voices {
            assert!(!voice.id.is_empty());
            assert!(!voice.name.is_empty());
            assert_eq!(voice.quality, VoiceQuality::Basic);
        }
    }

    #[test]
    #[ignore = "requires espeak-ng installed"]
    fn test_synthesize_text() {
        let samples = synthesize_text("Hello", None);
        assert!(samples.is_ok(), "Synthesis should succeed");

        let samples = samples.unwrap();
        assert!(!samples.is_empty(), "Should produce audio samples");
    }

    #[test]
    #[ignore = "requires espeak-ng installed"]
    fn test_synthesize_with_options() {
        let samples = synthesize_with_options("Hello world", None, 1.0, 1.0, 1.0);
        assert!(samples.is_ok(), "Synthesis with options should succeed");

        let samples = samples.unwrap();
        assert!(!samples.is_empty(), "Should produce audio samples");
    }

    #[test]
    #[ignore = "requires espeak-ng installed"]
    fn test_get_default_voice() {
        let voice = get_default_voice();
        assert!(voice.is_some(), "Should have a default voice");

        let voice = voice.unwrap();
        assert!(!voice.id.is_empty());
        assert!(!voice.name.is_empty());
    }

    #[test]
    #[ignore = "requires espeak-ng installed"]
    fn test_get_library_info() {
        let info = get_library_info();
        assert!(info.is_some(), "Should get library info");

        let (version, path) = info.unwrap();
        assert!(!version.is_empty(), "Version should not be empty");
        assert!(!path.is_empty(), "Voice path should not be empty");
    }
}
