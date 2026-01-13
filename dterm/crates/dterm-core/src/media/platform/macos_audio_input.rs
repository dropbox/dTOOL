//! Native macOS/iOS audio input using AVAudioEngine.
//!
//! This module provides native microphone capture via AVAudioEngine.
//! The captured audio is converted to the requested format and delivered
//! via callback to be fed to STT providers.
//!
//! ## Usage
//!
//! This module is only compiled when the `macos-speech` or `ios-speech` feature is enabled.
//!
//! ## Privacy Requirements
//!
//! Audio input requires:
//! - `NSMicrophoneUsageDescription` in Info.plist
//! - User authorization for microphone access
//!
//! ## Thread Safety
//!
//! The audio callback is invoked on the audio render thread. Data is
//! collected and forwarded to the user callback on a background thread.

use block2::RcBlock;
use objc2::exception::catch;
use objc2::rc::Retained;
use objc2::runtime::{Bool, NSObject};
use objc2::{extern_class, msg_send, ClassType};
use objc2_foundation::{NSArray, NSError, NSString};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::macos_stt_native::{AVAudioFormat, AVAudioPCMBuffer};
use super::traits::{AudioInputDevice, AudioInputError, AudioInputProvider};
use crate::media::AudioFormat;

// Link AVFoundation for AVAudioEngine
#[link(name = "AVFoundation", kind = "framework")]
extern "C" {}

// ============================================================================
// AVAudioEngine bindings
// ============================================================================

extern_class!(
    /// Binding to AVAudioEngine.
    ///
    /// Audio processing graph for recording and playback.
    #[unsafe(super(NSObject))]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct AVAudioEngine;
);

impl AVAudioEngine {
    /// Create a new audio engine.
    pub fn new() -> Retained<Self> {
        unsafe { msg_send![Self::class(), new] }
    }

    /// Get the input node for microphone access.
    ///
    /// # Safety
    /// This may throw an Objective-C exception if there's no audio input
    /// hardware available. Use `try_input_node` for a safe version.
    pub fn input_node(&self) -> Retained<AVAudioInputNode> {
        unsafe { msg_send![self, inputNode] }
    }

    /// Safely get the input node for microphone access.
    ///
    /// Returns `None` if accessing the input node would throw an exception
    /// (e.g., no audio input hardware available).
    pub fn try_input_node(&self) -> Option<Retained<AVAudioInputNode>> {
        // SAFETY: We're catching any Objective-C exception that might be thrown
        // when accessing the inputNode property. AssertUnwindSafe is safe here
        // because we don't capture any mutable state that would be corrupted.
        let result = catch(AssertUnwindSafe(|| self.input_node()));
        result.ok()
    }

    /// Get the main mixer node.
    #[allow(dead_code)]
    pub fn main_mixer_node(&self) -> Retained<AVAudioMixerNode> {
        unsafe { msg_send![self, mainMixerNode] }
    }

    /// Start the audio engine.
    pub fn start(&self) -> Result<(), String> {
        let mut error: *mut NSError = std::ptr::null_mut();
        let success: Bool = unsafe { msg_send![self, startAndReturnError: &mut error] };

        if success.as_bool() {
            Ok(())
        } else if !error.is_null() {
            let err: &NSError = unsafe { &*error };
            let desc: Retained<NSString> = unsafe { msg_send![err, localizedDescription] };
            Err(desc.to_string())
        } else {
            Err("Failed to start audio engine".to_string())
        }
    }

    /// Stop the audio engine.
    pub fn stop(&self) {
        unsafe {
            let _: () = msg_send![self, stop];
        }
    }

    /// Check if the engine is running.
    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        let running: Bool = unsafe { msg_send![self, isRunning] };
        running.as_bool()
    }

    /// Prepare the audio engine for starting.
    pub fn prepare(&self) {
        unsafe {
            let _: () = msg_send![self, prepare];
        }
    }
}

// ============================================================================
// AVAudioInputNode bindings
// ============================================================================

extern_class!(
    /// Binding to AVAudioInputNode.
    ///
    /// Node representing the system's audio input (microphone).
    #[unsafe(super(NSObject))]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct AVAudioInputNode;
);

impl AVAudioInputNode {
    /// Get the output format of the input node.
    pub fn output_format_for_bus(&self, bus: u32) -> Retained<AVAudioFormat> {
        unsafe { msg_send![self, outputFormatForBus: bus] }
    }

    /// Install a tap on the input node to receive audio data.
    ///
    /// The tap block is called with each buffer of audio data.
    pub fn install_tap_on_bus(
        &self,
        bus: u32,
        buffer_size: u32,
        format: Option<&AVAudioFormat>,
        tap_block: &RcBlock<dyn Fn(*mut AVAudioPCMBuffer, *mut AVAudioTime) + 'static>,
    ) {
        unsafe {
            let _: () = msg_send![
                self,
                installTapOnBus: bus,
                bufferSize: buffer_size,
                format: format.map_or(std::ptr::null(), std::ptr::from_ref),
                block: &**tap_block
            ];
        }
    }

    /// Remove a previously installed tap.
    pub fn remove_tap_on_bus(&self, bus: u32) {
        unsafe {
            let _: () = msg_send![self, removeTapOnBus: bus];
        }
    }
}

// ============================================================================
// AVAudioMixerNode bindings
// ============================================================================

extern_class!(
    /// Binding to AVAudioMixerNode.
    #[unsafe(super(NSObject))]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct AVAudioMixerNode;
);

// ============================================================================
// AVAudioTime bindings
// ============================================================================

extern_class!(
    /// Binding to AVAudioTime.
    ///
    /// Represents a moment in time.
    #[unsafe(super(NSObject))]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct AVAudioTime;
);

// ============================================================================
// AVAudioSession bindings (for iOS audio session management)
// ============================================================================

#[cfg(target_os = "ios")]
extern_class!(
    /// Binding to AVAudioSession.
    ///
    /// Manages audio session configuration on iOS.
    #[unsafe(super(NSObject))]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct AVAudioSession;
);

#[cfg(target_os = "ios")]
impl AVAudioSession {
    /// Get the shared audio session instance.
    pub fn shared_instance() -> Retained<Self> {
        unsafe { msg_send![Self::class(), sharedInstance] }
    }

    /// Set the audio session category.
    pub fn set_category(&self, category: &NSString) -> Result<(), String> {
        let mut error: *mut NSError = std::ptr::null_mut();
        let success: Bool = unsafe { msg_send![self, setCategory: category, error: &mut error] };

        if success.as_bool() {
            Ok(())
        } else if !error.is_null() {
            let err: &NSError = unsafe { &*error };
            let desc: Retained<NSString> = unsafe { msg_send![err, localizedDescription] };
            Err(desc.to_string())
        } else {
            Err("Failed to set audio session category".to_string())
        }
    }

    /// Activate or deactivate the audio session.
    pub fn set_active(&self, active: bool) -> Result<(), String> {
        let mut error: *mut NSError = std::ptr::null_mut();
        let success: Bool =
            unsafe { msg_send![self, setActive: Bool::new(active), error: &mut error] };

        if success.as_bool() {
            Ok(())
        } else if !error.is_null() {
            let err: &NSError = unsafe { &*error };
            let desc: Retained<NSString> = unsafe { msg_send![err, localizedDescription] };
            Err(desc.to_string())
        } else {
            Err("Failed to set audio session active state".to_string())
        }
    }
}

// ============================================================================
// AVCaptureDevice bindings (for device enumeration)
// ============================================================================

extern_class!(
    /// Binding to AVCaptureDevice.
    ///
    /// Represents a capture device (camera, microphone).
    #[unsafe(super(NSObject))]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct AVCaptureDevice;
);

impl AVCaptureDevice {
    /// Get all devices of a specific media type.
    #[allow(dead_code)]
    pub fn devices_with_media_type(media_type: &NSString) -> Retained<NSArray<AVCaptureDevice>> {
        unsafe { msg_send![Self::class(), devicesWithMediaType: media_type] }
    }

    /// Get the default device for a media type.
    pub fn default_device_with_media_type(media_type: &NSString) -> Option<Retained<Self>> {
        unsafe { msg_send![Self::class(), defaultDeviceWithMediaType: media_type] }
    }

    /// Get the unique device ID.
    pub fn unique_id(&self) -> Retained<NSString> {
        unsafe { msg_send![self, uniqueID] }
    }

    /// Get the localized device name.
    pub fn localized_name(&self) -> Retained<NSString> {
        unsafe { msg_send![self, localizedName] }
    }

    /// Check authorization status for a media type.
    pub fn authorization_status(media_type: &NSString) -> i64 {
        unsafe { msg_send![Self::class(), authorizationStatusForMediaType: media_type] }
    }

    /// Request authorization for a media type.
    #[allow(dead_code)]
    pub fn request_access(media_type: &NSString, completion: &RcBlock<dyn Fn(Bool) + 'static>) {
        unsafe {
            let _: () = msg_send![
                Self::class(),
                requestAccessForMediaType: media_type,
                completionHandler: &**completion
            ];
        }
    }
}

// Media type constants
fn audio_media_type() -> Retained<NSString> {
    // AVMediaTypeAudio = @"soun"
    NSString::from_str("soun")
}

// Authorization status constants
#[allow(dead_code)]
const AV_AUTHORIZATION_STATUS_NOT_DETERMINED: i64 = 0;
#[allow(dead_code)]
const AV_AUTHORIZATION_STATUS_RESTRICTED: i64 = 1;
#[allow(dead_code)]
const AV_AUTHORIZATION_STATUS_DENIED: i64 = 2;
const AV_AUTHORIZATION_STATUS_AUTHORIZED: i64 = 3;

// ============================================================================
// MacOsAudioInputProvider implementation
// ============================================================================

/// State shared with the audio callback.
struct AudioCallbackState {
    /// User callback to receive audio data.
    callback: super::traits::AudioDataCallback,
}

/// macOS/iOS audio input provider using AVAudioEngine.
///
/// Captures audio from the system microphone and delivers it via callback.
///
/// ## Example
///
/// ```ignore
/// let mut provider = MacOsAudioInputProvider::new();
///
/// // Start capturing
/// provider.start(AudioFormat::Pcm16k, None, |data| {
///     // Process audio data
///     println!("Received {} bytes", data.len());
/// })?;
///
/// // ... do something ...
///
/// // Stop capturing
/// provider.stop();
/// ```
pub struct MacOsAudioInputProvider {
    /// The audio engine.
    engine: Option<Retained<AVAudioEngine>>,
    /// Whether we're currently capturing.
    capturing: Arc<AtomicBool>,
    /// Shared state with the audio callback.
    callback_state: Arc<Mutex<Option<AudioCallbackState>>>,
    /// Target sample rate.
    target_sample_rate: f64,
    /// Supported formats.
    supported_formats: Vec<AudioFormat>,
}

impl MacOsAudioInputProvider {
    /// Create a new audio input provider.
    pub fn new() -> Self {
        Self {
            engine: None,
            capturing: Arc::new(AtomicBool::new(false)),
            callback_state: Arc::new(Mutex::new(None)),
            target_sample_rate: 16000.0,
            supported_formats: vec![AudioFormat::Pcm16k, AudioFormat::Pcm44k],
        }
    }

    /// Check if microphone access is authorized.
    pub fn is_authorized() -> bool {
        let media_type = audio_media_type();
        let status = AVCaptureDevice::authorization_status(&media_type);
        status == AV_AUTHORIZATION_STATUS_AUTHORIZED
    }

    /// Get the current authorization status.
    #[allow(dead_code)]
    pub fn authorization_status() -> &'static str {
        let media_type = audio_media_type();
        let status = AVCaptureDevice::authorization_status(&media_type);
        match status {
            AV_AUTHORIZATION_STATUS_NOT_DETERMINED => "not_determined",
            AV_AUTHORIZATION_STATUS_RESTRICTED => "restricted",
            AV_AUTHORIZATION_STATUS_DENIED => "denied",
            AV_AUTHORIZATION_STATUS_AUTHORIZED => "authorized",
            _ => "unknown",
        }
    }

    /// Request microphone authorization.
    ///
    /// The callback receives `true` if access was granted.
    #[allow(dead_code)]
    pub fn request_authorization<F>(callback: F)
    where
        F: FnOnce(bool) + Send + 'static,
    {
        let media_type = audio_media_type();

        // Wrap callback in a mutex to allow FnOnce
        let callback = Arc::new(Mutex::new(Some(callback)));

        let block = RcBlock::new(move |granted: Bool| {
            if let Some(cb) = callback.lock().unwrap().take() {
                cb(granted.as_bool());
            }
        });

        AVCaptureDevice::request_access(&media_type, &block);
    }

    /// Request microphone authorization synchronously.
    ///
    /// This blocks the current thread until the user responds to the
    /// authorization dialog. Returns `true` if access was granted.
    ///
    /// Note: The authorization dialog may not appear for CLI tools.
    /// In that case, users must manually grant permission in System Settings.
    pub fn request_authorization_sync() -> bool {
        use std::sync::mpsc;
        use std::time::Duration;

        // Check if already authorized
        let media_type = audio_media_type();
        let status = AVCaptureDevice::authorization_status(&media_type);
        if status == 3 {
            // AVAuthorizationStatusAuthorized
            return true;
        }
        if status == 1 || status == 2 {
            // Restricted or Denied - can't change programmatically
            return false;
        }

        // Status is NotDetermined (0), request authorization
        let (tx, rx) = mpsc::channel();

        let block = RcBlock::new(move |granted: Bool| {
            let _ = tx.send(granted.as_bool());
        });

        AVCaptureDevice::request_access(&media_type, &block);

        // Wait for response with timeout
        rx.recv_timeout(Duration::from_secs(60)).unwrap_or_default()
    }

    /// Configure iOS audio session for recording.
    #[cfg(target_os = "ios")]
    fn configure_audio_session() -> Result<(), String> {
        let session = AVAudioSession::shared_instance();

        // AVAudioSessionCategoryPlayAndRecord = @"AVAudioSessionCategoryPlayAndRecord"
        let category = NSString::from_str("AVAudioSessionCategoryPlayAndRecord");
        session.set_category(&category)?;
        session.set_active(true)?;

        Ok(())
    }

    #[cfg(not(target_os = "ios"))]
    fn configure_audio_session() {
        // No audio session configuration needed on macOS
    }

    /// Convert audio buffer to the target format (float32 PCM at target sample rate).
    fn convert_buffer(buffer: &AVAudioPCMBuffer, target_sample_rate: f64) -> Vec<u8> {
        let frame_length = buffer.frame_length() as usize;
        if frame_length == 0 {
            return Vec::new();
        }

        let channel_data = buffer.float_channel_data();
        if channel_data.is_null() {
            return Vec::new();
        }

        // Get the first channel (mono)
        let float_ptr = unsafe { *channel_data };
        if float_ptr.is_null() {
            return Vec::new();
        }

        // Read float samples
        let samples: Vec<f32> =
            unsafe { std::slice::from_raw_parts(float_ptr, frame_length).to_vec() };

        // TODO: Implement resampling if input sample rate differs from target
        // For now, we assume the tap format matches our target
        let _ = target_sample_rate;

        // Convert float32 samples to bytes
        let bytes: Vec<u8> = samples.iter().flat_map(|&f| f.to_le_bytes()).collect();

        bytes
    }
}

impl Default for MacOsAudioInputProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioInputProvider for MacOsAudioInputProvider {
    fn available_devices(&self) -> Vec<AudioInputDevice> {
        // Get the default audio input device
        // AVAudioEngine uses the system default, so we report that
        if let Some(device) = self.default_device() {
            vec![device]
        } else {
            Vec::new()
        }
    }

    fn default_device(&self) -> Option<AudioInputDevice> {
        let media_type = audio_media_type();
        let device = AVCaptureDevice::default_device_with_media_type(&media_type)?;

        Some(AudioInputDevice {
            id: device.unique_id().to_string(),
            name: device.localized_name().to_string(),
            is_default: true,
            supported_sample_rates: vec![16000, 44100, 48000],
        })
    }

    fn start(
        &mut self,
        format: AudioFormat,
        _device: Option<&str>,
        callback: super::traits::AudioDataCallback,
    ) -> Result<(), AudioInputError> {
        // Check if already capturing
        if self.capturing.load(Ordering::Acquire) {
            return Err(AudioInputError::AlreadyCapturing);
        }

        // Check authorization
        if !Self::is_authorized() {
            return Err(AudioInputError::PermissionDenied);
        }

        // Check format support
        if !self.supported_formats.contains(&format) {
            return Err(AudioInputError::UnsupportedFormat(format));
        }

        // Set target sample rate based on format
        self.target_sample_rate = match format {
            AudioFormat::Pcm16k => 16000.0,
            AudioFormat::Pcm44k => 44100.0,
            _ => return Err(AudioInputError::UnsupportedFormat(format)),
        };

        // Configure iOS audio session
        #[cfg(target_os = "ios")]
        Self::configure_audio_session().map_err(AudioInputError::Provider)?;
        #[cfg(not(target_os = "ios"))]
        Self::configure_audio_session();

        // Create and configure the audio engine
        let engine = AVAudioEngine::new();

        // Safely get the input node - this can throw an exception if no audio input is available
        let input_node = engine.try_input_node().ok_or_else(|| {
            AudioInputError::Provider(
                "No audio input available. Check that a microphone is connected and permissions are granted.".to_string()
            )
        })?;

        // Get the hardware format - we must use the hardware's native format for the tap
        // Apple's documentation states: "If a non-nil format is specified, the format must be
        // deinterleaved and match the sample rate and number of channels of the node's output format."
        let hardware_format = input_node.output_format_for_bus(0);

        // Store the actual sample rate for later use
        let actual_sample_rate = hardware_format.sample_rate();

        // Store callback state
        {
            let mut state = self.callback_state.lock().unwrap();
            *state = Some(AudioCallbackState { callback });
        }

        // Create the tap block
        let callback_state = Arc::clone(&self.callback_state);
        let capturing = Arc::clone(&self.capturing);
        // Note: We use the actual hardware sample rate, not the target, since we must
        // use the hardware format for the tap. Resampling would need to be done later if needed.
        // Store the hardware sample rate for potential future resampling
        self.target_sample_rate = actual_sample_rate;

        let tap_block = RcBlock::new(
            move |buffer: *mut AVAudioPCMBuffer, _time: *mut AVAudioTime| {
                // Wrap the entire callback in exception handling since we're called from the audio thread
                let result = catch(AssertUnwindSafe(|| {
                    if !capturing.load(Ordering::Acquire) {
                        return;
                    }

                    if buffer.is_null() {
                        return;
                    }

                    let buffer_ref: &AVAudioPCMBuffer = unsafe { &*buffer };

                    // Convert buffer to bytes
                    // Note: We pass 0.0 for sample rate since we don't do resampling currently
                    let data = Self::convert_buffer(buffer_ref, 0.0);

                    if !data.is_empty() {
                        // Call the user callback
                        if let Ok(mut state) = callback_state.lock() {
                            if let Some(ref mut callback_state) = *state {
                                (callback_state.callback)(&data);
                            }
                        }
                    }
                }));

                if let Err(e) = result {
                    // Log the exception but don't propagate (we're in a callback)
                    eprintln!("Exception in audio callback: {:?}", e);
                }
            },
        );

        // Buffer size: ~100ms of audio at hardware sample rate
        // Using 4096 samples is a reasonable default for most hardware sample rates
        let buffer_size: u32 = 4096;

        // Install the tap with format=None to use the hardware's native format
        // Apple's documentation: "If a non-nil format is specified, the format must be
        // deinterleaved and match the sample rate and number of channels of the node's output format."
        // Using None lets AVAudioEngine handle the format automatically.
        input_node.install_tap_on_bus(0, buffer_size, None, &tap_block);

        // Prepare and start the engine
        engine.prepare();
        engine.start().map_err(AudioInputError::Provider)?;

        // Store the engine and mark as capturing
        self.engine = Some(engine);
        self.capturing.store(true, Ordering::Release);

        // Keep references alive
        let _ = hardware_format;

        Ok(())
    }

    fn stop(&mut self) {
        if !self.capturing.load(Ordering::Acquire) {
            return;
        }

        self.capturing.store(false, Ordering::Release);

        if let Some(ref engine) = self.engine {
            // Remove the tap before stopping
            // Use try_input_node to avoid exception if hardware was disconnected
            if let Some(input_node) = engine.try_input_node() {
                input_node.remove_tap_on_bus(0);
            }

            // Stop the engine
            engine.stop();
        }

        self.engine = None;

        // Clear the callback state
        if let Ok(mut state) = self.callback_state.lock() {
            *state = None;
        }
    }

    fn is_capturing(&self) -> bool {
        self.capturing.load(Ordering::Acquire)
    }

    fn supported_formats(&self) -> &[AudioFormat] {
        &self.supported_formats
    }
}

impl std::fmt::Debug for MacOsAudioInputProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MacOsAudioInputProvider")
            .field("capturing", &self.capturing.load(Ordering::Relaxed))
            .field("target_sample_rate", &self.target_sample_rate)
            .field("supported_formats", &self.supported_formats)
            .finish()
    }
}

// SAFETY: MacOsAudioInputProvider is Send because:
// - engine is Option<Retained<AVAudioEngine>> which is Send when properly synchronized
// - capturing is Arc<AtomicBool> which is Send + Sync
// - callback_state is Arc<Mutex<...>> which is Send + Sync
// - All Objective-C operations go through proper synchronization
unsafe impl Send for MacOsAudioInputProvider {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = MacOsAudioInputProvider::new();
        assert!(!provider.is_capturing());
    }

    #[test]
    fn test_authorization_status() {
        let status = MacOsAudioInputProvider::authorization_status();
        assert!([
            "not_determined",
            "restricted",
            "denied",
            "authorized",
            "unknown"
        ]
        .contains(&status));
    }

    #[test]
    fn test_supported_formats() {
        let provider = MacOsAudioInputProvider::new();
        let formats = provider.supported_formats();
        assert!(formats.contains(&AudioFormat::Pcm16k));
        assert!(formats.contains(&AudioFormat::Pcm44k));
    }

    #[test]
    fn test_default_device() {
        let provider = MacOsAudioInputProvider::new();
        // May or may not have a default device depending on the system
        let device = provider.default_device();
        if let Some(d) = device {
            assert!(!d.id.is_empty());
            assert!(!d.name.is_empty());
            assert!(d.is_default);
        }
    }

    #[test]
    fn test_available_devices() {
        let provider = MacOsAudioInputProvider::new();
        let devices = provider.available_devices();
        // Should match default device behavior
        if let Some(default) = provider.default_device() {
            assert!(!devices.is_empty());
            assert_eq!(devices[0].id, default.id);
        }
    }

    #[test]
    fn test_stop_when_not_capturing() {
        let mut provider = MacOsAudioInputProvider::new();
        // Should not panic when stopping while not capturing
        provider.stop();
        assert!(!provider.is_capturing());
    }
}
