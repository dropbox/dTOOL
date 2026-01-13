//! Native Linux audio input using ALSA (Advanced Linux Sound Architecture).
//!
//! This module provides native microphone capture on Linux via ALSA.
//!
//! ## Usage
//!
//! This module is only compiled when the `linux-speech` feature is enabled.
//!
//! ## Implementation Notes
//!
//! Uses the `alsa` crate for PCM capture with the following configuration:
//! - Format: S16_LE (16-bit signed little-endian)
//! - Channels: 1 (mono)
//! - Sample rates: 16000 Hz or 44100 Hz
//! - Access: RW Interleaved
//!
//! ## Device Names
//!
//! Common ALSA device names:
//! - `default` - The default audio device
//! - `hw:0,0` - First hardware device
//! - `plughw:0,0` - Plugin layer on first device (handles format conversion)
//! - `pulse` - PulseAudio plugin (if available)
//!
//! ## Requirements
//!
//! - ALSA development libraries (`libasound2-dev` on Debian/Ubuntu)
//! - A working audio input device

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use super::traits::{AudioDataCallback, AudioInputDevice, AudioInputError, AudioInputProvider};
use crate::media::AudioFormat;

#[cfg(feature = "linux-speech")]
use alsa::pcm::{Access, Format, HwParams, State, PCM};
#[cfg(feature = "linux-speech")]
use alsa::{Direction, ValueOr};

// ============================================================================
// LinuxAudioInputProvider implementation
// ============================================================================

/// State shared with the audio capture thread.
struct CaptureThreadState {
    /// User callback to receive audio data.
    callback: AudioDataCallback,
    /// Target sample rate.
    sample_rate: u32,
    /// Device name to capture from.
    device: String,
    /// Stop signal.
    should_stop: Arc<AtomicBool>,
}

/// Linux audio input provider using ALSA.
///
/// Captures audio from the system microphone and delivers it via callback.
///
/// ## Example
///
/// ```ignore
/// let mut provider = LinuxAudioInputProvider::new();
///
/// // Start capturing
/// provider.start(AudioFormat::Pcm16k, None, Box::new(|data| {
///     // Process audio data
///     println!("Received {} bytes", data.len());
/// }))?;
///
/// // ... do something ...
///
/// // Stop capturing
/// provider.stop();
/// ```
pub struct LinuxAudioInputProvider {
    /// Whether we're currently capturing.
    capturing: Arc<AtomicBool>,
    /// Stop signal for the capture thread.
    should_stop: Arc<AtomicBool>,
    /// Capture thread handle.
    capture_thread: Option<thread::JoinHandle<()>>,
    /// Target sample rate.
    target_sample_rate: u32,
    /// Supported formats.
    supported_formats: Vec<AudioFormat>,
    /// Cached list of available devices.
    cached_devices: Vec<AudioInputDevice>,
}

impl LinuxAudioInputProvider {
    /// Create a new audio input provider.
    pub fn new() -> Self {
        let mut provider = Self {
            capturing: Arc::new(AtomicBool::new(false)),
            should_stop: Arc::new(AtomicBool::new(false)),
            capture_thread: None,
            target_sample_rate: 16000,
            supported_formats: vec![AudioFormat::Pcm16k, AudioFormat::Pcm44k],
            cached_devices: Vec::new(),
        };

        // Cache available devices
        provider.cached_devices = provider.enumerate_devices();
        provider
    }

    /// Enumerate available audio input devices.
    ///
    /// This checks for common ALSA devices and probes for sound cards.
    fn enumerate_devices(&self) -> Vec<AudioInputDevice> {
        let mut devices = Vec::new();

        // Always include the default device
        devices.push(AudioInputDevice {
            id: "default".to_string(),
            name: "Default Audio Input".to_string(),
            is_default: true,
            supported_sample_rates: vec![16000, 44100, 48000],
        });

        // Enumerate hardware devices by checking /proc/asound
        for card_num in 0..8 {
            let card_path = format!("/proc/asound/card{}", card_num);
            if std::path::Path::new(&card_path).exists() {
                // Read card name from /proc/asound/cardX/id
                let id_path = format!("/proc/asound/card{}/id", card_num);
                let name = std::fs::read_to_string(&id_path)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|_| format!("Card {}", card_num));

                // plughw provides format conversion (recommended)
                devices.push(AudioInputDevice {
                    id: format!("plughw:{}", card_num),
                    name: format!("{} (plughw)", name),
                    is_default: false,
                    supported_sample_rates: vec![16000, 44100, 48000],
                });

                // hw is direct hardware access
                devices.push(AudioInputDevice {
                    id: format!("hw:{}", card_num),
                    name: format!("{} (hw)", name),
                    is_default: false,
                    supported_sample_rates: vec![16000, 44100, 48000],
                });
            }
        }

        // Check for PulseAudio/PipeWire
        if Self::has_pulse_audio() {
            devices.push(AudioInputDevice {
                id: "pulse".to_string(),
                name: "PulseAudio/PipeWire".to_string(),
                is_default: false,
                supported_sample_rates: vec![16000, 44100, 48000],
            });
        }

        devices
    }

    /// Check if PulseAudio/PipeWire is available.
    fn has_pulse_audio() -> bool {
        // Check for pulse socket in common locations
        if let Ok(uid) = std::env::var("XDG_RUNTIME_DIR") {
            let pulse_path = format!("{}/pulse/native", uid);
            if std::path::Path::new(&pulse_path).exists() {
                return true;
            }
        }

        // Fallback: check /run/user/*/pulse/native
        if let Ok(entries) = std::fs::read_dir("/run/user") {
            for entry in entries.flatten() {
                let pulse_path = entry.path().join("pulse/native");
                if pulse_path.exists() {
                    return true;
                }
            }
        }

        false
    }

    /// Check if ALSA is available on this system.
    pub fn is_authorized() -> bool {
        // Check if /dev/snd exists (ALSA is available)
        std::path::Path::new("/dev/snd").exists()
    }

    /// Capture audio in a loop using ALSA.
    #[cfg(feature = "linux-speech")]
    fn capture_loop(state: CaptureThreadState) {
        // Open PCM device for capture
        let pcm = match PCM::new(&state.device, Direction::Capture, false) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Failed to open ALSA device '{}': {}", state.device, e);
                return;
            }
        };

        // Configure hardware parameters
        let hwp = match HwParams::any(&pcm) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("Failed to get ALSA HwParams: {}", e);
                return;
            }
        };

        // Set access type (interleaved read/write)
        if let Err(e) = hwp.set_access(Access::RWInterleaved) {
            eprintln!("Failed to set access type: {}", e);
            return;
        }

        // Set format to 16-bit signed little-endian
        if let Err(e) = hwp.set_format(Format::s16()) {
            eprintln!("Failed to set format: {}", e);
            return;
        }

        // Set mono channel
        if let Err(e) = hwp.set_channels(1) {
            eprintln!("Failed to set channels: {}", e);
            return;
        }

        // Set sample rate
        if let Err(e) = hwp.set_rate(state.sample_rate, ValueOr::Nearest) {
            eprintln!("Failed to set sample rate: {}", e);
            return;
        }

        // Set period size (~100ms of audio)
        let period_frames = state.sample_rate / 10; // 100ms
        if let Err(e) = hwp.set_period_size_near(period_frames as i64, ValueOr::Nearest) {
            // Non-fatal, use default period
            eprintln!("Warning: Could not set period size: {}", e);
        }

        // Set buffer size (4 periods)
        let buffer_frames = period_frames * 4;
        if let Err(e) = hwp.set_buffer_size_near(buffer_frames as i64) {
            // Non-fatal, use default buffer
            eprintln!("Warning: Could not set buffer size: {}", e);
        }

        // Apply hardware parameters
        if let Err(e) = pcm.hw_params(&hwp) {
            eprintln!("Failed to apply ALSA HwParams: {}", e);
            return;
        }

        // Get the actual period size
        let actual_period = hwp.get_period_size().unwrap_or(period_frames as i64) as usize;

        // Allocate buffer for one period of i16 samples
        let mut buffer: Vec<i16> = vec![0; actual_period];

        // Get I/O interface
        let io = match pcm.io_i16() {
            Ok(io) => io,
            Err(e) => {
                eprintln!("Failed to get ALSA I/O interface: {}", e);
                return;
            }
        };

        // Start the PCM
        if let Err(e) = pcm.start() {
            // Some devices may auto-start, so this might fail - try to continue
            eprintln!("Warning: PCM start returned: {}", e);
        }

        // Capture loop
        while !state.should_stop.load(Ordering::Acquire) {
            // Check PCM state
            match pcm.state() {
                State::XRun => {
                    // Buffer overrun, recover
                    if let Err(e) = pcm.prepare() {
                        eprintln!("Failed to recover from xrun: {}", e);
                        break;
                    }
                }
                State::Suspended => {
                    // Try to resume
                    if let Err(e) = pcm.resume() {
                        eprintln!("Failed to resume: {}", e);
                        break;
                    }
                }
                State::Disconnected => {
                    eprintln!("PCM disconnected");
                    break;
                }
                _ => {}
            }

            // Read audio data
            match io.readi(&mut buffer) {
                Ok(frames_read) => {
                    if frames_read > 0 && !state.should_stop.load(Ordering::Acquire) {
                        // Convert i16 samples to bytes (little-endian)
                        let bytes: Vec<u8> = buffer[..frames_read]
                            .iter()
                            .flat_map(|&sample| sample.to_le_bytes())
                            .collect();

                        // Call the callback
                        (state.callback)(&bytes);
                    }
                }
                Err(e) => {
                    // Check if it's a recoverable error
                    // EPIPE = 32, EAGAIN = 11 on Linux
                    let errno = e.errno();
                    if errno == 32 {
                        // EPIPE: Buffer overrun
                        if let Err(e) = pcm.prepare() {
                            eprintln!("Failed to recover from overrun: {}", e);
                            break;
                        }
                    } else if errno == 11 {
                        // EAGAIN: Would block, just continue
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    } else {
                        eprintln!("ALSA read error (errno {}): {}", errno, e);
                        break;
                    }
                }
            }
        }

        // Clean up
        let _ = pcm.drain();
    }

    /// Stub capture loop when linux-speech feature is not enabled.
    #[cfg(not(feature = "linux-speech"))]
    fn capture_loop(_state: CaptureThreadState) {
        // No-op when feature is not enabled
    }
}

impl Default for LinuxAudioInputProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioInputProvider for LinuxAudioInputProvider {
    fn available_devices(&self) -> Vec<AudioInputDevice> {
        self.cached_devices.clone()
    }

    fn default_device(&self) -> Option<AudioInputDevice> {
        self.cached_devices.first().cloned()
    }

    fn start(
        &mut self,
        format: AudioFormat,
        device: Option<&str>,
        callback: AudioDataCallback,
    ) -> Result<(), AudioInputError> {
        // Check if already capturing
        if self.capturing.load(Ordering::Acquire) {
            return Err(AudioInputError::AlreadyCapturing);
        }

        // Check format support
        if !self.supported_formats.contains(&format) {
            return Err(AudioInputError::UnsupportedFormat(format));
        }

        // Set target sample rate based on format
        self.target_sample_rate = match format {
            AudioFormat::Pcm16k => 16000,
            AudioFormat::Pcm44k => 44100,
            _ => return Err(AudioInputError::UnsupportedFormat(format)),
        };

        // Get device name
        let device_name = device.unwrap_or("default").to_string();

        // Validate device if specified
        if device.is_some() && !self.cached_devices.iter().any(|d| d.id == device_name) {
            return Err(AudioInputError::DeviceNotFound(device_name));
        }

        // Check if ALSA is available
        if !Self::is_authorized() {
            return Err(AudioInputError::Provider(
                "ALSA not available. Install libasound2-dev.".to_string(),
            ));
        }

        // Reset stop signal
        self.should_stop.store(false, Ordering::Release);

        // Create capture state for the thread
        let state = CaptureThreadState {
            callback,
            sample_rate: self.target_sample_rate,
            device: device_name,
            should_stop: Arc::clone(&self.should_stop),
        };

        // Mark as capturing
        self.capturing.store(true, Ordering::Release);

        // Start capture thread
        let capturing = Arc::clone(&self.capturing);
        self.capture_thread = Some(thread::spawn(move || {
            Self::capture_loop(state);
            capturing.store(false, Ordering::Release);
        }));

        Ok(())
    }

    fn stop(&mut self) {
        if !self.capturing.load(Ordering::Acquire) {
            return;
        }

        // Signal the capture thread to stop
        self.should_stop.store(true, Ordering::Release);

        // Wait for capture thread to finish
        if let Some(thread) = self.capture_thread.take() {
            let _ = thread.join();
        }

        self.capturing.store(false, Ordering::Release);
    }

    fn is_capturing(&self) -> bool {
        self.capturing.load(Ordering::Acquire)
    }

    fn supported_formats(&self) -> &[AudioFormat] {
        &self.supported_formats
    }
}

impl std::fmt::Debug for LinuxAudioInputProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinuxAudioInputProvider")
            .field("capturing", &self.capturing.load(Ordering::Relaxed))
            .field("target_sample_rate", &self.target_sample_rate)
            .field("supported_formats", &self.supported_formats)
            .field("device_count", &self.cached_devices.len())
            .finish()
    }
}

// SAFETY: LinuxAudioInputProvider is Send because all fields are Send-safe:
// - capturing and should_stop are Arc<AtomicBool> which is Send + Sync
// - capture_thread is Option<JoinHandle<()>> which is Send
// - All other fields are simple types
unsafe impl Send for LinuxAudioInputProvider {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = LinuxAudioInputProvider::new();
        assert!(!provider.is_capturing());
    }

    #[test]
    fn test_supported_formats() {
        let provider = LinuxAudioInputProvider::new();
        let formats = provider.supported_formats();
        assert!(formats.contains(&AudioFormat::Pcm16k));
        assert!(formats.contains(&AudioFormat::Pcm44k));
    }

    #[test]
    fn test_default_device() {
        let provider = LinuxAudioInputProvider::new();
        let device = provider.default_device();
        assert!(device.is_some());
        assert_eq!(device.unwrap().id, "default");
    }

    #[test]
    fn test_stop_when_not_capturing() {
        let mut provider = LinuxAudioInputProvider::new();
        // Should not panic when stopping while not capturing
        provider.stop();
        assert!(!provider.is_capturing());
    }

    #[test]
    fn test_available_devices() {
        let provider = LinuxAudioInputProvider::new();
        let devices = provider.available_devices();
        // Should always have at least the default device
        assert!(!devices.is_empty());
        assert!(devices.iter().any(|d| d.id == "default"));
    }

    #[test]
    fn test_cannot_start_twice() {
        let mut provider = LinuxAudioInputProvider::new();
        // Manually set capturing to true to simulate already capturing
        provider.capturing.store(true, Ordering::Release);

        let result = provider.start(AudioFormat::Pcm16k, None, Box::new(|_| {}));
        assert!(matches!(result, Err(AudioInputError::AlreadyCapturing)));

        // Reset for cleanup
        provider.capturing.store(false, Ordering::Release);
    }

    #[test]
    fn test_unsupported_format() {
        let mut provider = LinuxAudioInputProvider::new();

        // Try to use a format that's not in supported_formats
        // We modify the provider to have an empty list to trigger this
        provider.supported_formats = vec![];

        let result = provider.start(AudioFormat::Pcm16k, None, Box::new(|_| {}));
        assert!(matches!(result, Err(AudioInputError::UnsupportedFormat(_))));
    }

    #[test]
    fn test_device_not_found() {
        let mut provider = LinuxAudioInputProvider::new();

        let result = provider.start(
            AudioFormat::Pcm16k,
            Some("nonexistent_device_xyz"),
            Box::new(|_| {}),
        );
        assert!(matches!(result, Err(AudioInputError::DeviceNotFound(_))));
    }
}
