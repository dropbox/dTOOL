# Native Speech Bindings API Plan

**Created:** Iteration 334
**Updated:** Iteration 351
**Status:** Complete (All Platforms, Audio Input Complete)
**Related:** `src/media/platform/`, Phase 12 (Full Integration)

## Overview

This document outlines the implementation plan for native speech bindings on each platform:
- **macOS/iOS:** Using `objc2` crate family for Objective-C runtime bindings
- **Windows:** Using `windows-rs` for Windows Runtime (WinRT) APIs
- **Linux:** Using Vosk for STT and espeak-ng for TTS

---

## Current State

The `src/media/platform/` module defines traits and stub implementations:

| Platform | STT Status | TTS Status | Provider |
|----------|------------|------------|----------|
| macOS | **Native** | **Native** | `MacOsSttProvider`, `MacOsTtsProvider` |
| iOS | **Native** | **Native** | `IosSttProvider`, `IosTtsProvider` |
| Windows | **Native** | **Native** | `WindowsSttProvider`, `WindowsTtsProvider` |
| Linux | **Native** | **Native** | `LinuxSttProvider`, `LinuxTtsProvider` |

The traits (`SttProvider`, `TtsProvider`) are fully specified in `platform/traits.rs`.

### Completed Work

**macOS TTS (Iteration 338-340):**
- Native bindings via `objc2-av-foundation`
- `ThreadSafeSynthesizer` for thread-safe access
- Playback control (pause/resume/stop) via main thread dispatch
- Voice enumeration and quality detection

**macOS STT (Iteration 345):**
- Full native bindings to `SFSpeechRecognizer` via manual `objc2` bindings
- Authorization status checking via `authorizationStatus()`
- Locale enumeration via `supportedLocales()`
- Recognizer availability checking and on-device recognition detection
- `ThreadSafeRecognizer` wrapper with block2 callback support
- `SFSpeechAudioBufferRecognitionRequest` for streaming audio input
- `SFSpeechRecognitionTask` for managing recognition sessions
- `AVAudioFormat` and `AVAudioPCMBuffer` for audio data handling
- Full recognition flow: start → feed audio → get partial/final results → stop

**Windows TTS (Iteration 341):**
- Native bindings via `windows` crate v0.58
- `synthesize_text()`, `synthesize_ssml()`, `synthesize_with_options()`
- SSML-based rate/pitch/volume control
- Voice enumeration via `SpeechSynthesizer::AllVoices()`

**Linux TTS (Iteration 342):**
- Native bindings via `espeakng` crate v0.2
- `synthesize_text()`, `synthesize_with_options()`
- Rate/pitch/volume via espeak-ng Parameter API
- Voice enumeration via `get_voices()`
- Global singleton Speaker with mutex lock
- 22050 Hz mono PCM16 output

**iOS STT & TTS (Iteration 346):**
- Shares native bindings with macOS (same Speech and AVFoundation frameworks)
- `ios-speech` feature flag enables native bindings
- `IosSttProvider` uses shared `ThreadSafeRecognizer` from macOS implementation
- `IosTtsProvider` uses shared `ThreadSafeSynthesizer` from macOS implementation
- Authorization status checking and on-device recognition detection
- Playback control (pause/resume/stop) via GCD main thread dispatch
- Requires Info.plist entries: `NSMicrophoneUsageDescription`, `NSSpeechRecognitionUsageDescription`

**Audio Input (Iteration 347-351):**
- New `AudioInputProvider` trait for platform-agnostic audio capture
- `AudioDataCallback` type alias enables object-safe trait design
- `MacOsAudioInputProvider` using AVAudioEngine with input tap
- Shared between macOS and iOS (same implementation)
- Features:
  - Device enumeration via AVCaptureDevice
  - Authorization status checking
  - Configurable sample rate (16kHz or 44.1kHz)
  - Float32 PCM output via callback
- Factory function `create_audio_input_provider()` for platform detection

**Windows Audio Input (Iteration 351):**
- Full native implementation using `Windows.Media.Audio.AudioGraph` API
- `WindowsAudioInputProvider` with complete `AudioInputProvider` trait implementation
- Components:
  - `AudioGraph` with `AudioRenderCategory::Speech` for speech capture optimization
  - `AudioDeviceInputNode` for microphone input
  - `AudioFrameOutputNode` for frame-by-frame audio retrieval
  - `QuantumStarted` event handler for synchronized audio processing
- Features:
  - Device enumeration via `DeviceInformation::FindAllAsyncDeviceClass`
  - Configurable sample rate (16kHz or 44.1kHz)
  - Float32 PCM output via callback
  - Low-latency quantum size selection
  - Proper COM interface usage for buffer access (`IMemoryBufferByteAccess`)
- Added `Media_Audio` and `Media_Render` features to windows crate dependencies

---

## macOS/iOS Implementation (objc2)

### Dependencies

```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.5"
objc2-foundation = "0.2"
objc2-av-foundation = "0.2"  # For AVSpeechSynthesizer
block2 = "0.5"  # For block callbacks

[target.'cfg(target_os = "ios")'.dependencies]
objc2 = "0.5"
objc2-foundation = "0.2"
objc2-av-foundation = "0.2"
block2 = "0.5"

[build-dependencies]
# May need for Speech.framework bindings (not in objc2 crates yet)
bindgen = "0.69"  # If manual bindings needed
```

### Speech Framework Bindings

The Speech framework (`SFSpeechRecognizer`) is not yet in the objc2 crate ecosystem. Options:

1. **Manual bindings** - Use `objc2::declare_class!` and `objc2::extern_class!`
2. **Bindgen** - Generate bindings from Speech.framework headers
3. **Wait for upstream** - Request/contribute `objc2-speech` crate

#### Manual Binding Approach

```rust
use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{extern_class, ClassType, msg_send_id, msg_send};
use objc2_foundation::{NSString, NSLocale, NSError};

// Speech.framework classes (not in objc2 ecosystem yet)
extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct SFSpeechRecognizer;

    unsafe impl ClassType for SFSpeechRecognizer {
        type Super = NSObject;
        const NAME: &'static str = "SFSpeechRecognizer";
    }
);

impl SFSpeechRecognizer {
    pub fn new_with_locale(locale: &NSLocale) -> Option<Retained<Self>> {
        unsafe { msg_send_id![Self::class(), alloc, initWithLocale: locale] }
    }

    pub fn is_available(&self) -> bool {
        unsafe { msg_send![self, isAvailable] }
    }

    pub fn supports_on_device_recognition(&self) -> bool {
        unsafe { msg_send![self, supportsOnDeviceRecognition] }
    }
}

extern_class!(
    pub struct SFSpeechAudioBufferRecognitionRequest;

    unsafe impl ClassType for SFSpeechAudioBufferRecognitionRequest {
        type Super = NSObject;
        const NAME: &'static str = "SFSpeechAudioBufferRecognitionRequest";
    }
);
```

### AVSpeechSynthesizer (TTS)

AVFoundation is available via `objc2-av-foundation`:

```rust
use objc2_av_foundation::{
    AVSpeechSynthesizer,
    AVSpeechUtterance,
    AVSpeechSynthesisVoice,
};

impl MacOsTtsProvider {
    pub fn synthesize_native(&mut self, text: &str, voice: Option<&str>) -> Result<Vec<u8>, TtsProviderError> {
        let synthesizer = AVSpeechSynthesizer::new();
        let utterance = AVSpeechUtterance::speechUtteranceWithString(
            &NSString::from_str(text)
        );

        if let Some(voice_id) = voice {
            if let Some(voice) = AVSpeechSynthesisVoice::voiceWithIdentifier(
                &NSString::from_str(voice_id)
            ) {
                utterance.setVoice(Some(&voice));
            }
        }

        utterance.setRate(self.rate);
        utterance.setPitchMultiplier(self.pitch);
        utterance.setVolume(self.volume);

        // Use write(toBufferCallback:) for audio data
        // This requires setting up a delegate

        synthesizer.speakUtterance(&utterance);

        // ...
    }
}
```

### Implementation Phases

#### Phase 1: TTS (Lower Risk)
1. Add `objc2-av-foundation` dependency
2. Implement `MacOsTtsProvider::synthesize()` using `AVSpeechSynthesizer`
3. Implement streaming via delegate callbacks
4. Test on macOS and iOS

#### Phase 2: STT (Higher Complexity)
1. Create manual bindings for `SFSpeechRecognizer`
2. Handle authorization flow
3. Implement audio buffer recognition
4. Test on macOS and iOS

### iOS-Specific Considerations

- **Info.plist keys required:**
  - `NSSpeechRecognitionUsageDescription` - STT permission
  - `NSMicrophoneUsageDescription` - Microphone access
- **Background modes:** May need `audio` background mode for continuous STT
- **Memory constraints:** iOS has stricter memory limits

---

## Windows Implementation (windows-rs)

### Dependencies

```toml
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.58", features = [
    "Media_SpeechRecognition",
    "Media_SpeechSynthesis",
    "Storage_Streams",
    "Foundation_Collections",
] }
```

### WinRT Speech APIs

Windows provides speech via WinRT (Windows Runtime):

- **STT:** `Windows.Media.SpeechRecognition.SpeechRecognizer`
- **TTS:** `Windows.Media.SpeechSynthesis.SpeechSynthesizer`

#### STT Implementation

```rust
use windows::Media::SpeechRecognition::{
    SpeechRecognizer,
    SpeechContinuousRecognitionSession,
    SpeechRecognitionResult,
};
use windows::Foundation::{TypedEventHandler, IAsyncOperation};

impl WindowsSttProvider {
    pub async fn start_recognition(&mut self, language: &str) -> Result<(), SttProviderError> {
        // Create recognizer with language
        let language = windows::Globalization::Language::CreateLanguage(
            &windows::core::HSTRING::from(language)
        )?;

        let recognizer = SpeechRecognizer::CreateWithLanguage(&language)?;

        // Enable continuous recognition
        let session = recognizer.ContinuousRecognitionSession()?;

        // Set up result handler
        session.ResultGenerated(&TypedEventHandler::new(
            |_session, args| {
                if let Some(args) = args {
                    let result = args.Result()?;
                    let text = result.Text()?.to_string();
                    // Emit result to channel
                }
                Ok(())
            }
        ))?;

        // Start recognition
        session.StartAsync()?.await?;

        Ok(())
    }
}
```

#### TTS Implementation

```rust
use windows::Media::SpeechSynthesis::{
    SpeechSynthesizer,
    SpeechSynthesisStream,
    VoiceInformation,
};

impl WindowsTtsProvider {
    pub async fn synthesize(&mut self, text: &str, voice: Option<&str>) -> Result<Vec<u8>, TtsProviderError> {
        let synthesizer = SpeechSynthesizer::new()?;

        // Set voice if specified
        if let Some(voice_id) = voice {
            let voices = SpeechSynthesizer::AllVoices()?;
            for i in 0..voices.Size()? {
                let voice = voices.GetAt(i)?;
                if voice.Id()?.to_string() == voice_id {
                    synthesizer.SetVoice(&voice)?;
                    break;
                }
            }
        }

        // Synthesize to stream
        let stream: SpeechSynthesisStream = synthesizer
            .SynthesizeTextToStreamAsync(&HSTRING::from(text))?
            .await?;

        // Read stream to bytes
        let reader = DataReader::CreateDataReader(&stream)?;
        let size = stream.Size()? as u32;
        reader.LoadAsync(size)?.await?;

        let mut buffer = vec![0u8; size as usize];
        reader.ReadBytes(&mut buffer)?;

        Ok(buffer)
    }
}
```

### Implementation Phases

#### Phase 1: TTS (Simpler API)
1. Add `windows` crate with `Media_SpeechSynthesis` feature
2. Implement `WindowsTtsProvider::synthesize()`
3. Implement voice enumeration
4. Test on Windows 10/11

#### Phase 2: STT (More Complex)
1. Add `Media_SpeechRecognition` feature
2. Handle microphone permissions
3. Implement continuous recognition with callbacks
4. Test on Windows 10/11

### Windows-Specific Considerations

- **Async:** WinRT APIs are async, need tokio/async runtime
- **Privacy:** Speech recognition requires user consent
- **Offline:** On-device recognition requires downloading language packs
- **Audio format:** WinRT speech APIs typically use PCM 16kHz

---

## Linux Implementation

Linux has no built-in speech APIs. Options:

### STT Options

| Library | Type | Quality | Offline | License |
|---------|------|---------|---------|---------|
| Vosk | C API | Good | Yes | Apache 2.0 |
| Whisper.cpp | C++ | Excellent | Yes | MIT |
| CMU Sphinx | C | Fair | Yes | BSD |

**Recommended:** Vosk - best balance of quality, size, and license.

```toml
[target.'cfg(target_os = "linux")'.dependencies]
vosk = "0.4"  # Rust bindings for Vosk
```

### TTS Options (Implemented)

| Library | Type | Quality | Offline | License |
|---------|------|---------|---------|---------|
| **espeak-ng** | C | Basic | Yes | GPL-3.0 |
| Festival | C++ | Good | Yes | BSD-like |
| Piper | C++ | Excellent | Yes | MIT |

**Implemented:** espeak-ng via `espeakng` crate - safe Rust bindings.

```toml
[target.'cfg(target_os = "linux")'.dependencies]
espeakng = { version = "0.2", optional = true }
```

### TTS Implementation (Completed)

```rust
// TTS with espeak-ng (implemented in linux_native.rs)
use espeakng::{initialise, Speaker, Parameter};

/// Synthesize text to audio samples
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
        locked.set_voice_raw(voice_id)?;
    }

    // Set parameters (rate in WPM, pitch 0-100, volume 0-200)
    let wpm = ((rate * 175.0).clamp(80.0, 450.0)) as i32;
    locked.set_parameter(Parameter::Rate, wpm, 0)?;

    let espeak_pitch = (((pitch - 0.5) / 1.5) * 100.0).clamp(0.0, 100.0) as i32;
    locked.set_parameter(Parameter::Pitch, espeak_pitch, 0)?;

    let espeak_volume = (volume * 200.0).clamp(0.0, 200.0) as i32;
    locked.set_parameter(Parameter::Volume, espeak_volume, 0)?;

    // Synthesize to PCM samples (22050 Hz mono)
    locked.synthesize(text)
}
```

### STT Implementation (Pending)

```rust
// STT with Vosk (not yet implemented)
use vosk::{Model, Recognizer};

impl LinuxSttProvider {
    pub fn new(model_path: &str) -> Result<Self, SttProviderError> {
        let model = Model::new(model_path)?;
        Ok(Self { model: Some(model), recognizer: None })
    }

    pub fn start(&mut self, format: AudioFormat) -> Result<(), SttProviderError> {
        let sample_rate = match format {
            AudioFormat::Pcm16k => 16000.0,
            AudioFormat::Pcm44k => 44100.0,
            _ => return Err(SttProviderError::UnsupportedFormat(format)),
        };

        self.recognizer = Some(Recognizer::new(
            self.model.as_ref().unwrap(),
            sample_rate
        )?);

        Ok(())
    }

    pub fn feed_audio(&mut self, data: &[u8]) -> Result<(), SttProviderError> {
        if let Some(ref mut recognizer) = self.recognizer {
            recognizer.accept_waveform(data);
        }
        Ok(())
    }
}
```

---

## Feature Flags

Cargo features control platform bindings:

```toml
[features]
default = []
macos-speech = ["objc2", "objc2-foundation", "block2", "objc2-av-foundation", "dispatch2"]
ios-speech = ["objc2", "objc2-foundation", "block2", "objc2-av-foundation", "dispatch2"]
windows-speech = ["windows"]  # Media_SpeechSynthesis feature
linux-speech = ["espeakng"]   # espeak-ng bindings

# All native speech
native-speech = [
    "macos-speech",
    "ios-speech",
    "windows-speech",
    "linux-speech"
]
```

### Current Implementation Status

| Feature | TTS | STT |
|---------|-----|-----|
| `macos-speech` | Native (AVSpeechSynthesizer) | Native (SFSpeechRecognizer) |
| `ios-speech` | Native (shared with macOS) | Native (shared with macOS) |
| `windows-speech` | Native (WinRT SpeechSynthesizer) | Native (WinRT SpeechRecognizer) |
| `linux-speech` | Native (espeak-ng) | Native (Vosk) |

---

## FFI Callback Integration

The existing FFI callback layer in `media/ffi.rs` should work with native providers. The flow:

1. Platform layer registers FFI callbacks
2. Native providers call through FFI to platform
3. Platform invokes system speech APIs
4. Results flow back through FFI

For native Rust implementations, skip FFI and call directly:

```rust
pub fn create_stt_provider() -> Box<dyn SttProvider> {
    #[cfg(all(target_os = "macos", feature = "macos-speech"))]
    {
        Box::new(native::macos::MacOsSttProviderNative::new())
    }

    #[cfg(all(target_os = "windows", feature = "windows-speech"))]
    {
        Box::new(native::windows::WindowsSttProviderNative::new())
    }

    #[cfg(not(any(
        all(target_os = "macos", feature = "macos-speech"),
        all(target_os = "windows", feature = "windows-speech"),
    )))]
    {
        // Fall back to FFI-based providers or null
        Box::new(NullSttProvider)
    }
}
```

---

## Testing Strategy

### Unit Tests
- Mock audio data for STT
- Verify TTS output format
- Test error handling

### Integration Tests
- Requires device with microphone
- Test actual recognition
- Test voice synthesis playback
- Manual end-to-end checklist: see `docs/VOICE_E2E_TESTING.md`

### CI Testing
- Unit tests on all platforms
- Integration tests on macOS (GitHub Actions)
- Windows integration requires self-hosted runner

---

## Timeline

| Phase | Platform | Component | Complexity | Status |
|-------|----------|-----------|------------|--------|
| 1.1 | macOS | TTS (AVSpeechSynthesizer) | Medium | **Done** |
| 1.2 | Windows | TTS (SpeechSynthesizer) | Medium | **Done** |
| 1.3 | Linux | TTS (espeak-ng) | Low | **Done** |
| 2.1 | macOS | STT (SFSpeechRecognizer) | High | **Done** |
| 2.2 | Windows | STT (SpeechRecognizer) | High | **Done** |
| 2.3 | Linux | STT (Vosk) | Medium | **Done** |
| 3 | iOS | Both (shared with macOS) | Medium | **Done** |
| 4.1 | macOS/iOS | Audio Input (AVAudioEngine) | Medium | **Done** |
| 4.2 | Windows | Audio Input (AudioGraph) | Medium | **Done** |
| 4.3 | Linux | Audio Input (ALSA) | Medium | **Done** |

**All speech bindings and audio input complete on all platforms!**

Next steps:
- Real microphone end-to-end testing
- Integration with MediaServer for voice-to-text pipeline

---

## Open Questions

1. **objc2-speech crate:** Should we contribute this upstream or maintain locally?
2. **Async runtime:** Windows-rs requires async - how to integrate with sync API?
3. **Audio routing:** How to handle audio capture from terminal PTY?
4. **Model distribution:** How to distribute Vosk models for Linux?

---

## References

- [objc2 documentation](https://docs.rs/objc2/latest/objc2/)
- [windows-rs documentation](https://docs.rs/windows/latest/windows/)
- [Apple Speech Framework](https://developer.apple.com/documentation/speech)
- [AVSpeechSynthesizer](https://developer.apple.com/documentation/avfoundation/avspeechsynthesizer)
- [Windows SpeechRecognizer](https://learn.microsoft.com/en-us/uwp/api/windows.media.speechrecognition.speechrecognizer)
- [Windows SpeechSynthesizer](https://learn.microsoft.com/en-us/uwp/api/windows.media.speechsynthesis.speechsynthesizer)
- [Vosk API](https://alphacephei.com/vosk/)
- [espeak-ng](https://github.com/espeak-ng/espeak-ng)
