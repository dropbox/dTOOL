//! Native Windows speech synthesis using WinRT bindings.
//!
//! This module provides native bindings to Windows.Media.SpeechSynthesis for TTS
//! on Windows 10/11. It uses the `windows` crate for WinRT interop.
//!
//! ## Usage
//!
//! This module is only compiled when the `windows-speech` feature is enabled.
//!
//! ## Implementation Notes
//!
//! Windows.Media.SpeechSynthesis provides:
//! - High-quality neural and standard voices
//! - SSML support for advanced control
//! - Streaming audio output
//! - Both online (neural) and offline voices
//!
//! ## WinRT API References
//!
//! - [`SpeechSynthesizer`](https://docs.microsoft.com/en-us/uwp/api/windows.media.speechsynthesis.speechsynthesizer)
//! - [`VoiceInformation`](https://docs.microsoft.com/en-us/uwp/api/windows.media.speechsynthesis.voiceinformation)

use windows::core::HSTRING;
use windows::Foundation::IAsyncOperation;
use windows::Media::SpeechSynthesis::{
    SpeechSynthesisStream, SpeechSynthesizer, VoiceGender as WinVoiceGender, VoiceInformation,
};
use windows::Storage::Streams::{DataReader, IRandomAccessStream, InputStreamOptions};

use super::{VoiceGender, VoiceInfo, VoiceQuality};

// ============================================================================
// Voice enumeration
// ============================================================================

/// Get all available TTS voices from Windows.
///
/// Returns a vector of (id, name, language, gender, is_neural) tuples.
pub fn get_available_voices() -> Vec<(String, String, String, Option<VoiceGender>, bool)> {
    let mut result = Vec::new();

    if let Ok(voices) = SpeechSynthesizer::AllVoices() {
        if let Ok(count) = voices.Size() {
            for i in 0..count {
                if let Ok(voice) = voices.GetAt(i) {
                    if let (Ok(id), Ok(name), Ok(lang), Ok(gender)) = (
                        voice.Id(),
                        voice.DisplayName(),
                        voice.Language(),
                        voice.Gender(),
                    ) {
                        let gender = match gender {
                            WinVoiceGender::Male => Some(VoiceGender::Male),
                            WinVoiceGender::Female => Some(VoiceGender::Female),
                            _ => None,
                        };

                        // Neural voices typically have "Online" or "Natural" in the name
                        let is_neural = name.to_string().contains("Online")
                            || name.to_string().contains("Natural")
                            || name.to_string().contains("Neural");

                        result.push((
                            id.to_string(),
                            name.to_string(),
                            lang.to_string(),
                            gender,
                            is_neural,
                        ));
                    }
                }
            }
        }
    }

    result
}

/// Convert Windows voices to VoiceInfo structs.
pub fn get_voice_info_list() -> Vec<VoiceInfo> {
    get_available_voices()
        .into_iter()
        .map(|(id, name, lang, gender, is_neural)| VoiceInfo {
            id,
            name,
            language: lang,
            gender,
            quality: if is_neural {
                VoiceQuality::Premium
            } else {
                VoiceQuality::Standard
            },
        })
        .collect()
}

// ============================================================================
// Speech synthesis
// ============================================================================

/// Synthesize text to audio bytes using Windows TTS.
///
/// # Arguments
/// * `text` - The text to synthesize
/// * `voice_id` - Optional voice ID to use
///
/// # Returns
/// Audio data as WAV bytes on success, or error message on failure.
pub fn synthesize_text(text: &str, voice_id: Option<&str>) -> Result<Vec<u8>, String> {
    // Create synthesizer
    let synth = SpeechSynthesizer::new().map_err(|e| e.to_string())?;

    // Set voice if specified
    if let Some(voice_id) = voice_id {
        if let Ok(voices) = SpeechSynthesizer::AllVoices() {
            if let Ok(count) = voices.Size() {
                for i in 0..count {
                    if let Ok(voice) = voices.GetAt(i) {
                        if let Ok(id) = voice.Id() {
                            if id.to_string() == voice_id {
                                let _ = synth.SetVoice(&voice);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    // Synthesize text to stream
    let text_hstring = HSTRING::from(text);
    let stream_op: IAsyncOperation<SpeechSynthesisStream> = synth
        .SynthesizeTextToStreamAsync(&text_hstring)
        .map_err(|e| e.to_string())?;

    // Wait for synthesis to complete (blocking)
    let stream = stream_op.get().map_err(|e| e.to_string())?;

    // Read stream to bytes
    read_stream_to_bytes(&stream)
}

/// Synthesize SSML to audio bytes using Windows TTS.
///
/// SSML allows more control over pronunciation, prosody, etc.
///
/// # Arguments
/// * `ssml` - The SSML markup to synthesize
///
/// # Returns
/// Audio data as WAV bytes on success, or error message on failure.
pub fn synthesize_ssml(ssml: &str) -> Result<Vec<u8>, String> {
    let synth = SpeechSynthesizer::new().map_err(|e| e.to_string())?;

    let ssml_hstring = HSTRING::from(ssml);
    let stream_op: IAsyncOperation<SpeechSynthesisStream> = synth
        .SynthesizeSsmlToStreamAsync(&ssml_hstring)
        .map_err(|e| e.to_string())?;

    let stream = stream_op.get().map_err(|e| e.to_string())?;

    read_stream_to_bytes(&stream)
}

/// Read a stream to bytes.
fn read_stream_to_bytes(stream: &SpeechSynthesisStream) -> Result<Vec<u8>, String> {
    let size = stream.Size().map_err(|e| e.to_string())? as u32;

    if size == 0 {
        return Ok(Vec::new());
    }

    // Get the input stream from the speech synthesis stream
    let input_stream: IRandomAccessStream = stream.cast().map_err(|e| e.to_string())?;

    let reader = DataReader::CreateDataReader(&input_stream).map_err(|e| e.to_string())?;

    // Set input stream options for sequential read
    reader
        .SetInputStreamOptions(InputStreamOptions::ReadAhead)
        .map_err(|e| e.to_string())?;

    // Load data from stream
    let load_op = reader.LoadAsync(size).map_err(|e| e.to_string())?;
    let bytes_loaded = load_op.get().map_err(|e| e.to_string())?;

    if bytes_loaded == 0 {
        return Ok(Vec::new());
    }

    // Read bytes from reader
    let mut buffer = vec![0u8; bytes_loaded as usize];
    reader.ReadBytes(&mut buffer).map_err(|e| e.to_string())?;

    Ok(buffer)
}

// ============================================================================
// Synthesizer options
// ============================================================================

/// Set synthesizer options for rate, pitch, and volume.
///
/// Note: Windows TTS options are set via SpeechSynthesizerOptions on the
/// SpeechSynthesizer. For more control, use SSML with prosody elements.
pub fn create_ssml_with_options(text: &str, rate: f32, pitch: f32, volume: f32) -> String {
    // Convert rate from 0.5-2.0 to percentage (-50% to +100%)
    let rate_percent = ((rate - 1.0) * 100.0) as i32;
    let rate_str = if rate_percent >= 0 {
        format!("+{}%", rate_percent)
    } else {
        format!("{}%", rate_percent)
    };

    // Convert pitch from 0.5-2.0 to percentage (-50% to +100%)
    let pitch_percent = ((pitch - 1.0) * 100.0) as i32;
    let pitch_str = if pitch_percent >= 0 {
        format!("+{}%", pitch_percent)
    } else {
        format!("{}%", pitch_percent)
    };

    // Convert volume from 0.0-1.0 to "x-soft" to "x-loud"
    let volume_str = match (volume * 100.0) as u32 {
        0..=20 => "x-soft",
        21..=40 => "soft",
        41..=60 => "medium",
        61..=80 => "loud",
        _ => "x-loud",
    };

    // Escape XML special characters in text
    let escaped_text = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;");

    format!(
        r#"<speak version="1.0" xmlns="http://www.w3.org/2001/10/synthesis" xml:lang="en-US">
    <prosody rate="{}" pitch="{}" volume="{}">
        {}
    </prosody>
</speak>"#,
        rate_str, pitch_str, volume_str, escaped_text
    )
}

/// Synthesize text with voice and options.
pub fn synthesize_with_options(
    text: &str,
    voice_id: Option<&str>,
    rate: f32,
    pitch: f32,
    volume: f32,
) -> Result<Vec<u8>, String> {
    // If options are default, use simple synthesis
    let is_default =
        (rate - 1.0).abs() < 0.01 && (pitch - 1.0).abs() < 0.01 && (volume - 1.0).abs() < 0.01;

    if is_default {
        return synthesize_text(text, voice_id);
    }

    // Create synthesizer with voice
    let synth = SpeechSynthesizer::new().map_err(|e| e.to_string())?;

    if let Some(voice_id) = voice_id {
        if let Ok(voices) = SpeechSynthesizer::AllVoices() {
            if let Ok(count) = voices.Size() {
                for i in 0..count {
                    if let Ok(voice) = voices.GetAt(i) {
                        if let Ok(id) = voice.Id() {
                            if id.to_string() == voice_id {
                                let _ = synth.SetVoice(&voice);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    // Use SSML for rate/pitch/volume control
    let ssml = create_ssml_with_options(text, rate, pitch, volume);
    let ssml_hstring = HSTRING::from(ssml);

    let stream_op: IAsyncOperation<SpeechSynthesisStream> = synth
        .SynthesizeSsmlToStreamAsync(&ssml_hstring)
        .map_err(|e| e.to_string())?;

    let stream = stream_op.get().map_err(|e| e.to_string())?;

    read_stream_to_bytes(&stream)
}

// ============================================================================
// Default voice
// ============================================================================

/// Get the default voice for the current system.
pub fn get_default_voice() -> Option<VoiceInfo> {
    if let Ok(synth) = SpeechSynthesizer::new() {
        if let Ok(voice) = synth.Voice() {
            if let (Ok(id), Ok(name), Ok(lang), Ok(gender)) = (
                voice.Id(),
                voice.DisplayName(),
                voice.Language(),
                voice.Gender(),
            ) {
                let gender = match gender {
                    WinVoiceGender::Male => Some(VoiceGender::Male),
                    WinVoiceGender::Female => Some(VoiceGender::Female),
                    _ => None,
                };

                let is_neural =
                    name.to_string().contains("Online") || name.to_string().contains("Natural");

                return Some(VoiceInfo {
                    id: id.to_string(),
                    name: name.to_string(),
                    language: lang.to_string(),
                    gender,
                    quality: if is_neural {
                        VoiceQuality::Premium
                    } else {
                        VoiceQuality::Standard
                    },
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssml_generation() {
        let ssml = create_ssml_with_options("Hello world", 1.5, 0.8, 0.7);
        assert!(ssml.contains("rate=\"+50%\""));
        assert!(ssml.contains("pitch=\"-20%\""));
        assert!(ssml.contains("volume=\"loud\""));
        assert!(ssml.contains("Hello world"));
    }

    #[test]
    fn test_ssml_escaping() {
        let ssml = create_ssml_with_options("Test <>&\"' chars", 1.0, 1.0, 1.0);
        assert!(ssml.contains("&lt;"));
        assert!(ssml.contains("&gt;"));
        assert!(ssml.contains("&amp;"));
        assert!(ssml.contains("&quot;"));
        assert!(ssml.contains("&apos;"));
    }
}
