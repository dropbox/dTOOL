//! Microphone end-to-end test harness.
//!
//! This tests the full pipeline: microphone capture -> STT recognition.
//!
//! Usage:
//!     cargo run --example mic_test --package dterm-core --features macos-speech --release
//!
//! Expected behavior:
//! 1. Requests speech recognition authorization (first run only)
//! 2. Starts microphone capture
//! 3. Displays partial STT results as you speak
//! 4. After 5 seconds, stops and shows final result
//!
//! Note: Requires both speech recognition and microphone permissions.

use dterm_core::media::{
    create_audio_input_provider, create_stt_provider, create_tts_provider, AudioFormat,
    MediaServer, MediaServerConfig,
};
use std::time::{Duration, Instant};

// Import native bindings for authorization
#[cfg(all(target_os = "macos", feature = "macos-speech"))]
use dterm_core::media::macos_stt_native;

#[cfg(all(target_os = "macos", feature = "macos-speech"))]
use dterm_core::media::macos_audio_input::MacOsAudioInputProvider;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== dterm-core Microphone E2E Test ===\n");

    // Display platform info
    let platform = dterm_core::media::platform_name();
    println!("Platform: {}", platform);

    // Check and request speech recognition authorization
    #[cfg(all(target_os = "macos", feature = "macos-speech"))]
    {
        let status = macos_stt_native::SFSpeechRecognizer::authorization_status();
        println!("Speech recognition authorization: {:?}", status);

        if !macos_stt_native::is_authorized() {
            match status {
                macos_stt_native::SFSpeechRecognizerAuthorizationStatus::NotDetermined => {
                    println!("\nSpeech recognition has not been authorized yet.");
                    println!("To authorize, you have two options:\n");
                    println!("Option 1: Run this test from within an app bundle that has");
                    println!("          NSSpeechRecognitionUsageDescription in its Info.plist\n");
                    println!("Option 2: Manually grant permission in System Settings:");
                    println!("          System Settings > Privacy & Security > Speech Recognition");
                    println!("          Then add Terminal (or your IDE) to the allowed apps.\n");
                    println!("Attempting to trigger authorization dialog...");

                    // Try requesting authorization
                    let authorized = macos_stt_native::request_authorization_sync();
                    if authorized {
                        println!("Authorization granted!");
                    } else {
                        eprintln!(
                            "\nAuthorization dialog may not have appeared (requires UI context)."
                        );
                        eprintln!("Please manually enable speech recognition for this app.\n");
                        eprintln!("After granting permission, run this test again.");
                        return Err(
                            "Speech recognition not authorized - manual setup required".into()
                        );
                    }
                }
                macos_stt_native::SFSpeechRecognizerAuthorizationStatus::Denied => {
                    eprintln!("\nSpeech recognition was DENIED by user.");
                    eprintln!("To fix: System Settings > Privacy & Security > Speech Recognition");
                    eprintln!("        Enable permission for this app (Terminal, IDE, etc.)");
                    return Err("Speech recognition denied - check System Settings".into());
                }
                macos_stt_native::SFSpeechRecognizerAuthorizationStatus::Restricted => {
                    eprintln!("\nSpeech recognition is RESTRICTED (parental controls or MDM).");
                    return Err("Speech recognition restricted by system policy".into());
                }
                _ => {
                    eprintln!("\nUnexpected authorization status.");
                    return Err("Unknown authorization status".into());
                }
            }
        } else {
            println!("Speech recognition: AUTHORIZED");
        }

        // Check microphone authorization
        let mic_status = MacOsAudioInputProvider::authorization_status();
        println!("\nMicrophone authorization: {}", mic_status);

        if !MacOsAudioInputProvider::is_authorized() {
            if mic_status == "not_determined" {
                println!("\nMicrophone access has not been determined yet.");
                println!("Attempting to trigger authorization dialog...");

                let authorized = MacOsAudioInputProvider::request_authorization_sync();
                if authorized {
                    println!("Microphone authorization granted!");
                } else {
                    eprintln!("\nMicrophone authorization dialog may not have appeared (requires UI context).");
                    eprintln!("Please manually enable microphone for this app.\n");
                    eprintln!("To fix: System Settings > Privacy & Security > Microphone");
                    eprintln!("        Enable permission for this app (Terminal, IDE, etc.)");
                    eprintln!("\nAfter granting permission, run this test again.");
                    return Err("Microphone not authorized - manual setup required".into());
                }
            } else {
                eprintln!(
                    "\nMicrophone access is NOT authorized (status: {}).",
                    mic_status
                );
                eprintln!("To fix: System Settings > Privacy & Security > Microphone");
                eprintln!("        Enable permission for this app (Terminal, IDE, etc.)");
                return Err("Microphone not authorized - check System Settings".into());
            }
        }
    }

    // Create providers
    println!("\nCreating providers...");
    let stt = create_stt_provider();
    let tts = create_tts_provider();
    let audio_input = create_audio_input_provider();

    // Check capabilities
    let caps = dterm_core::media::get_platform_capabilities();
    println!("STT formats: {:?}", caps.stt_formats);
    println!("TTS formats: {:?}", caps.tts_formats);
    println!("Supports VAD: {}", caps.supports_vad);
    println!(
        "STT languages: {} languages supported",
        caps.stt_languages.len()
    );
    println!();

    // Create MediaServer with all providers
    let mut server =
        MediaServer::with_all_providers(MediaServerConfig::default(), stt, tts, audio_input);

    println!("Has audio input: {}", server.has_audio_input());

    if !server.has_audio_input() {
        eprintln!("ERROR: No audio input provider available!");
        eprintln!("Make sure you built with --features macos-speech");
        return Err("No audio input".into());
    }

    // Start STT with microphone capture
    println!("\n--- Starting microphone capture ---");
    println!("Speak into your microphone for 5 seconds...");
    println!("(Say something like: 'Hello, this is a test')\n");

    let client_id = 1;
    let format = AudioFormat::Pcm16k;
    let language = Some("en-US");

    match server.start_stt_with_microphone(client_id, format, language) {
        Ok(stream_id) => {
            println!("Stream started: {:?}", stream_id);
        }
        Err(e) => {
            eprintln!("ERROR starting STT: {}", e);
            return Err(e.to_string().into());
        }
    }

    // Process audio for 5 seconds
    let start = Instant::now();
    let duration = Duration::from_secs(5);
    let mut partial_count = 0;

    while start.elapsed() < duration {
        match server.process_audio() {
            Ok(Some(partial)) => {
                partial_count += 1;
                println!(
                    "[{:.1}s] Partial {}: \"{}\" (confidence: {}%)",
                    start.elapsed().as_secs_f32(),
                    partial_count,
                    partial.text,
                    partial.confidence
                );
            }
            Ok(None) => {
                // No new audio to process
            }
            Err(e) => {
                eprintln!("Process error: {}", e);
            }
        }

        // Check if capturing
        if !server.is_capturing_audio() {
            println!("Audio capture stopped unexpectedly!");
            break;
        }

        // Check VAD if available
        if let Some(is_speaking) = server.is_voice_active() {
            if is_speaking && partial_count == 0 {
                print!(".");
                std::io::Write::flush(&mut std::io::stdout())?;
            }
        }

        std::thread::sleep(Duration::from_millis(30));
    }

    println!("\n\n--- Stopping capture ---");

    // Stop and get final result
    match server.stop_stt_with_microphone() {
        Ok(Some(result)) => {
            println!("\nFinal result: \"{}\"", result.text);
            println!("Confidence: {}%", result.confidence);
        }
        Ok(None) => {
            println!("\nNo speech detected or recognition failed.");
        }
        Err(e) => {
            eprintln!("ERROR stopping STT: {}", e);
        }
    }

    // Verify state
    println!("\n--- State verification ---");
    println!("STT state: {:?}", server.stt_state());
    println!("Is capturing: {}", server.is_capturing_audio());
    println!("Invariants valid: {}", server.verify_invariants());

    // Summary
    println!("\n=== Test Summary ===");
    println!("Partial results received: {}", partial_count);
    println!("Duration: {:.2}s", start.elapsed().as_secs_f32());
    println!("Test completed!");

    Ok(())
}
