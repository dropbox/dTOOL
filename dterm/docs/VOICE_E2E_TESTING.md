# Voice End-to-End Microphone Testing

Purpose: manual validation of native microphone capture + STT pipeline on each platform.

## Prerequisites

- Build `dterm-core` with the platform speech feature enabled:
  - macOS: `--features macos-speech`
  - iOS: `--features ios-speech`
  - Windows: `--features windows-speech`
  - Linux: `--features linux-speech`
- Ensure microphone and speech recognition permissions are granted for the process.
- Linux: install `espeak-ng` and place a Vosk model in one of:
  `~/.local/share/vosk/models`, `/usr/share/vosk/models`, `/usr/local/share/vosk/models`,
  `/opt/vosk/models`, `models/vosk`, or `./vosk-model` (see `docs/SPEECH_BINDINGS_PLAN.md`).

## Test Harness

A built-in test example is provided in `crates/dterm-core/examples/mic_test.rs`.

### Running the Test

```bash
cargo run --example mic_test --package dterm-core --features macos-speech --release
```

Platform examples:
- macOS: `cargo run --example mic_test --package dterm-core --features macos-speech --release`
- Windows: `cargo run --example mic_test --package dterm-core --features windows-speech --release`
- Linux: `cargo run --example mic_test --package dterm-core --features linux-speech --release`

Note: iOS requires running from a signed app target; the CLI example is macOS/Windows/Linux only.

The test will:
1. Check and request speech recognition authorization
2. Check microphone authorization status
3. Start microphone capture for 5 seconds
4. Display partial STT results in real-time
5. Show the final recognition result

### Expected Output (when properly authorized)

```
=== dterm-core Microphone E2E Test ===

Platform: macOS
Speech recognition authorization: Authorized
Speech recognition: AUTHORIZED

Microphone authorization: authorized

Creating providers...
STT formats: [Pcm16k, Pcm44k]
TTS formats: [Pcm16k, Pcm44k, Aac]
Supports VAD: true
STT languages: 63 languages supported

Has audio input: true

--- Starting microphone capture ---
Speak into your microphone for 5 seconds...
(Say something like: 'Hello, this is a test')

Stream started: StreamId(0)
[1.2s] Partial 1: "Hello" (confidence: 85%)
[2.5s] Partial 2: "Hello this is" (confidence: 87%)
[4.1s] Partial 3: "Hello this is a test" (confidence: 91%)


--- Stopping capture ---

Final result: "Hello, this is a test"
Confidence: 95%

--- State verification ---
STT state: Idle
Is capturing: false
Invariants valid: true

=== Test Summary ===
Partial results received: 3
Duration: 5.02s
Test completed!
```

## Authorization Setup

### macOS Speech Recognition

1. **First Run**: The test will attempt to trigger the authorization dialog
2. **CLI limitation**: Terminal/CLI binaries often cannot trigger the prompt;
   grant permission manually in System Settings if no dialog appears
3. **If dialog doesn't appear**: Manually enable in System Settings:
   - System Settings > Privacy & Security > Speech Recognition
   - Add Terminal.app (or your IDE) to allowed apps

### macOS Microphone Access

1. **First Run**: The system should prompt for microphone access when AVAudioEngine starts
2. **CLI limitation**: Terminal/CLI binaries may not surface the prompt; grant
   access manually in System Settings if needed
3. **If not prompted**: Manually enable in System Settings:
   - System Settings > Privacy & Security > Microphone
   - Add Terminal.app (or your IDE) to allowed apps

### Troubleshooting

| Error | Solution |
|-------|----------|
| "Speech recognition not authorized" | Enable in System Settings > Privacy & Security > Speech Recognition |
| "Microphone permission denied" | Enable in System Settings > Privacy & Security > Microphone |
| "Recognizer not available" | Ensure network connectivity for non-on-device recognition |
| No dialog appears | CLI tools may be blocked; run from an app bundle or grant permissions manually |

## Validation Checklist

- Audio input provider starts and stops cleanly (no hangs, no panics).
- Partial STT results arrive during capture.
- Final result returns after stop.
- `MediaServer` returns to `Idle` and `is_capturing_audio()` is false.
- Stream stats update without errors.

## Platform Notes

### macOS

- Expect a microphone permission prompt on first run.
- Verify AVAudioEngine start/stop logs (if enabled).

### iOS

- Run from a signed app target; confirm microphone permissions in device settings.

### Windows

- Verify microphone privacy settings allow desktop apps.
- Confirm AudioGraph creates `AudioDeviceInputNode` successfully.

### Linux

- Ensure ALSA default device is available.
- Confirm Vosk model path is configured and accessible.

## Results Log

### Run 1: macOS (2024-12-31, Iteration 366)

- **Platform:** macOS (Darwin 24.6.0)
- **Device:** Default audio input
- **Sample rate:** Not logged (default)
- **Language:** en-US (63 languages supported)
- **STT provider:** Native macOS SFSpeechRecognizer
- **Authorization:** Both speech recognition and microphone authorized ✓
- **Provider creation:** STT, TTS, and audio input providers created successfully ✓
- **Audio formats:** Pcm16k, Pcm44k (STT); Pcm16k, Pcm44k, Aac (TTS)
- **VAD support:** true
- **Stream started:** Yes (StreamId: 0) ✓
- **Observed partials:** 0 (no speech input provided during test)
- **Final result:** None (expected - CLI test without actual speech)
- **State after stop:** STT Idle, is_capturing=false, invariants valid ✓
- **Duration:** 11.64s
- **Issues:** None - test passed all validation checks
