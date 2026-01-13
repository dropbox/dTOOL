//! Native Windows audio input using AudioGraph API.
//!
//! This module provides native microphone capture via Windows Audio Graph API
//! through the `windows` crate WinRT bindings.
//!
//! ## Usage
//!
//! This module is only compiled when the `windows-speech` feature is enabled.
//!
//! ## Implementation Notes
//!
//! We use `Windows.Media.Audio.AudioGraph` with:
//! - `AudioDeviceInputNode` for microphone capture
//! - `AudioFrameOutputNode` for frame-by-frame audio retrieval
//! - `QuantumStarted` event for synchronized audio processing
//!
//! This provides real-time audio capture with configurable sample rates.
//!
//! ## Privacy Requirements
//!
//! Audio input requires the "microphone" capability in the app manifest
//! for UWP apps. For desktop apps, the user may be prompted for permission.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::traits::{AudioDataCallback, AudioInputDevice, AudioInputError, AudioInputProvider};
use crate::media::AudioFormat;

// Windows imports
use windows::core::{implement, Result as WinResult, HSTRING};
use windows::Devices::Enumeration::{DeviceClass, DeviceInformation};
use windows::Foundation::TypedEventHandler;
use windows::Media::Audio::{
    AudioDeviceInputNode, AudioDeviceNodeCreationStatus, AudioFrameOutputNode, AudioGraph,
    AudioGraphCreationStatus, AudioGraphSettings, QuantumSizeSelectionMode,
};
use windows::Media::AudioBuffer;
use windows::Media::AudioBufferAccessMode;
use windows::Media::Capture::MediaCategory;
use windows::Media::MediaProperties::{AudioEncodingProperties, MediaEncodingSubtypes};
use windows::Media::Render::AudioRenderCategory;

// ============================================================================
// AudioGraph State
// ============================================================================

/// State for the audio graph capture.
struct AudioGraphState {
    /// The audio graph instance.
    graph: AudioGraph,
    /// The device input node (microphone).
    input_node: AudioDeviceInputNode,
    /// The frame output node for capturing audio data.
    output_node: AudioFrameOutputNode,
    /// Event registration token for QuantumStarted.
    quantum_token: Option<windows::Foundation::EventRegistrationToken>,
}

impl Drop for AudioGraphState {
    fn drop(&mut self) {
        // Unregister the event handler
        if let Some(token) = self.quantum_token.take() {
            let _ = self.graph.RemoveQuantumStarted(token);
        }
        // Stop the graph
        let _ = self.graph.Stop();
    }
}

/// Shared state for audio callback communication.
struct CallbackState {
    /// User callback to receive audio data.
    callback: AudioDataCallback,
    /// Flag to signal the callback is active.
    active: bool,
}

// ============================================================================
// WindowsAudioInputProvider implementation
// ============================================================================

/// Windows audio input provider using AudioGraph.
///
/// Captures audio from the system microphone and delivers it via callback.
///
/// ## Example
///
/// ```ignore
/// let mut provider = WindowsAudioInputProvider::new();
///
/// // List available devices
/// for device in provider.available_devices() {
///     println!("{}: {}", device.id, device.name);
/// }
///
/// // Start capturing
/// provider.start(AudioFormat::Pcm16k, None, Box::new(|data| {
///     // Process audio data (f32 samples, mono, 16kHz)
///     println!("Received {} samples", data.len() / 4);
/// }))?;
///
/// // ... capture audio ...
///
/// // Stop capturing
/// provider.stop();
/// ```
pub struct WindowsAudioInputProvider {
    /// The audio graph state (when capturing).
    graph_state: Option<AudioGraphState>,
    /// Whether we're currently capturing.
    capturing: Arc<AtomicBool>,
    /// Shared state with the audio callback.
    callback_state: Arc<Mutex<Option<CallbackState>>>,
    /// Target sample rate.
    target_sample_rate: u32,
    /// Supported formats.
    supported_formats: Vec<AudioFormat>,
    /// Cached list of available devices.
    cached_devices: Vec<AudioInputDevice>,
}

impl WindowsAudioInputProvider {
    /// Create a new audio input provider.
    pub fn new() -> Self {
        let mut provider = Self {
            graph_state: None,
            capturing: Arc::new(AtomicBool::new(false)),
            callback_state: Arc::new(Mutex::new(None)),
            target_sample_rate: 16000,
            supported_formats: vec![AudioFormat::Pcm16k, AudioFormat::Pcm44k],
            cached_devices: Vec::new(),
        };

        // Cache available devices
        provider.cached_devices = provider.enumerate_devices();
        provider
    }

    /// Enumerate available audio input devices.
    fn enumerate_devices(&self) -> Vec<AudioInputDevice> {
        let mut devices = Vec::new();

        // Use DeviceInformation to enumerate audio capture devices
        if let Ok(device_info_collection) =
            DeviceInformation::FindAllAsyncDeviceClass(DeviceClass::AudioCapture)
        {
            if let Ok(collection) = device_info_collection.get() {
                if let Ok(count) = collection.Size() {
                    for i in 0..count {
                        if let Ok(device) = collection.GetAt(i) {
                            if let (Ok(id), Ok(name)) = (device.Id(), device.Name()) {
                                let is_default = i == 0; // First device is typically default
                                devices.push(AudioInputDevice {
                                    id: id.to_string(),
                                    name: name.to_string(),
                                    is_default,
                                    supported_sample_rates: vec![16000, 44100, 48000],
                                });
                            }
                        }
                    }
                }
            }
        }

        devices
    }

    /// Check if microphone access is available.
    ///
    /// On Windows, this checks if there are any audio capture devices.
    pub fn is_authorized() -> bool {
        // Check if we can enumerate any audio capture devices
        if let Ok(device_info_collection) =
            DeviceInformation::FindAllAsyncDeviceClass(DeviceClass::AudioCapture)
        {
            if let Ok(collection) = device_info_collection.get() {
                if let Ok(count) = collection.Size() {
                    return count > 0;
                }
            }
        }
        false
    }

    /// Create the AudioGraph with the specified configuration.
    fn create_audio_graph(
        &self,
        sample_rate: u32,
        device_id: Option<&str>,
    ) -> Result<AudioGraphState, AudioInputError> {
        // Create AudioGraph settings
        let settings = AudioGraphSettings::Create(AudioRenderCategory::Speech).map_err(|e| {
            AudioInputError::Provider(format!("Failed to create AudioGraphSettings: {}", e))
        })?;

        // Set quantum size for lower latency (480 samples at 48kHz = 10ms)
        settings
            .SetQuantumSizeSelectionMode(QuantumSizeSelectionMode::LowestLatency)
            .map_err(|e| {
                AudioInputError::Provider(format!("Failed to set quantum size mode: {}", e))
            })?;

        // Set desired sample rate
        let encoding_props =
            AudioEncodingProperties::CreatePcm(sample_rate, 1, 32).map_err(|e| {
                AudioInputError::Provider(format!("Failed to create encoding properties: {}", e))
            })?;

        // Set subtype to Float
        encoding_props
            .SetSubtype(&MediaEncodingSubtypes::Float().map_err(|e| {
                AudioInputError::Provider(format!("Failed to get Float subtype: {}", e))
            })?)
            .map_err(|e| {
                AudioInputError::Provider(format!("Failed to set encoding subtype: {}", e))
            })?;

        settings
            .SetEncodingProperties(&encoding_props)
            .map_err(|e| {
                AudioInputError::Provider(format!("Failed to set encoding properties: {}", e))
            })?;

        // Create the AudioGraph asynchronously
        let create_result = AudioGraph::CreateAsync(&settings)
            .map_err(|e| AudioInputError::Provider(format!("Failed to start CreateAsync: {}", e)))?
            .get()
            .map_err(|e| AudioInputError::Provider(format!("CreateAsync failed: {}", e)))?;

        if create_result.Status() != Ok(AudioGraphCreationStatus::Success) {
            let status = create_result
                .Status()
                .unwrap_or(AudioGraphCreationStatus::UnknownFailure);
            return Err(AudioInputError::Provider(format!(
                "AudioGraph creation failed with status: {:?}",
                status
            )));
        }

        let graph = create_result.Graph().map_err(|e| {
            AudioInputError::Provider(format!("Failed to get AudioGraph from result: {}", e))
        })?;

        // Create device input node (microphone)
        let input_result = if let Some(device_id) = device_id {
            // Find the specified device
            let device_async = DeviceInformation::CreateFromIdAsync(&HSTRING::from(device_id))
                .map_err(|e| {
                    AudioInputError::Provider(format!("Failed to get device info: {}", e))
                })?;

            let device_info = device_async.get().map_err(|e| {
                AudioInputError::Provider(format!("Failed to retrieve device: {}", e))
            })?;

            graph
                .CreateDeviceInputNodeWithFormatAndDeviceOnCategoryAsync(
                    MediaCategory::Speech,
                    &encoding_props,
                    &device_info,
                )
                .map_err(|e| {
                    AudioInputError::Provider(format!("Failed to create input node: {}", e))
                })?
                .get()
                .map_err(|e| {
                    AudioInputError::Provider(format!("Input node creation failed: {}", e))
                })?
        } else {
            // Use default device
            graph
                .CreateDeviceInputNodeWithFormatOnCategoryAsync(
                    MediaCategory::Speech,
                    &encoding_props,
                )
                .map_err(|e| {
                    AudioInputError::Provider(format!("Failed to create input node: {}", e))
                })?
                .get()
                .map_err(|e| {
                    AudioInputError::Provider(format!("Input node creation failed: {}", e))
                })?
        };

        let input_status = input_result.Status().map_err(|e| {
            AudioInputError::Provider(format!("Failed to get input node status: {}", e))
        })?;

        if input_status != AudioDeviceNodeCreationStatus::Success {
            return Err(match input_status {
                AudioDeviceNodeCreationStatus::AccessDenied => AudioInputError::PermissionDenied,
                AudioDeviceNodeCreationStatus::DeviceNotAvailable => {
                    AudioInputError::DeviceNotFound("No audio capture device available".to_string())
                }
                AudioDeviceNodeCreationStatus::FormatNotSupported => {
                    AudioInputError::UnsupportedFormat(AudioFormat::Pcm16k)
                }
                _ => AudioInputError::Provider(format!(
                    "Input node creation failed with status: {:?}",
                    input_status
                )),
            });
        }

        let input_node = input_result
            .DeviceInputNode()
            .map_err(|e| AudioInputError::Provider(format!("Failed to get input node: {}", e)))?;

        // Create frame output node for capturing audio data
        let output_node = graph.CreateFrameOutputNode().map_err(|e| {
            AudioInputError::Provider(format!("Failed to create frame output node: {}", e))
        })?;

        // Connect input node to output node
        input_node
            .AddOutgoingConnection(&output_node)
            .map_err(|e| AudioInputError::Provider(format!("Failed to connect nodes: {}", e)))?;

        Ok(AudioGraphState {
            graph,
            input_node,
            output_node,
            quantum_token: None,
        })
    }

    /// Set up the QuantumStarted event handler.
    fn setup_quantum_handler(&mut self) -> Result<(), AudioInputError> {
        let state = self
            .graph_state
            .as_mut()
            .ok_or_else(|| AudioInputError::Provider("No graph state".to_string()))?;

        let callback_state = Arc::clone(&self.callback_state);
        let capturing = Arc::clone(&self.capturing);
        let output_node = state.output_node.clone();

        // Create the event handler
        let handler =
            TypedEventHandler::new(move |_graph: &Option<AudioGraph>, _args: &Option<_>| {
                // Check if we're still capturing
                if !capturing.load(Ordering::Acquire) {
                    return Ok(());
                }

                // Get the audio frame
                let frame = match output_node.GetFrame() {
                    Ok(f) => f,
                    Err(_) => return Ok(()),
                };

                // Lock the buffer for reading
                let buffer = match frame.LockBuffer(AudioBufferAccessMode::Read) {
                    Ok(b) => b,
                    Err(_) => return Ok(()),
                };

                // Get buffer length
                let length = match buffer.Length() {
                    Ok(l) => l as usize,
                    Err(_) => return Ok(()),
                };

                if length == 0 {
                    return Ok(());
                }

                // Create reference to access data
                let reference = match buffer.CreateReference() {
                    Ok(r) => r,
                    Err(_) => return Ok(()),
                };

                // Get the raw bytes using IMemoryBufferByteAccess
                // Note: This requires unsafe access to the COM interface
                let data = unsafe { get_buffer_data(&reference, length) };

                if let Some(data) = data {
                    // Call the user callback
                    if let Ok(mut state_guard) = callback_state.lock() {
                        if let Some(ref mut state) = *state_guard {
                            if state.active {
                                (state.callback)(&data);
                            }
                        }
                    }
                }

                Ok(())
            });

        // Register the event handler
        let token = state.graph.QuantumStarted(&handler).map_err(|e| {
            AudioInputError::Provider(format!("Failed to register QuantumStarted handler: {}", e))
        })?;

        state.quantum_token = Some(token);

        Ok(())
    }
}

/// Get the raw buffer data from an IMemoryBufferReference.
///
/// # Safety
/// This uses COM interfaces to access the raw memory buffer.
unsafe fn get_buffer_data(
    reference: &windows::Foundation::IMemoryBufferReference,
    length: usize,
) -> Option<Vec<u8>> {
    use windows::core::Interface;

    // Query for IMemoryBufferByteAccess interface
    #[windows::core::interface("5B0D3235-4DBA-4D44-865E-8F1D0E4FD04D")]
    unsafe trait IMemoryBufferByteAccess: windows::core::IUnknown {
        fn GetBuffer(&self, value: *mut *mut u8, capacity: *mut u32) -> windows::core::HRESULT;
    }

    let byte_access: IMemoryBufferByteAccess = match reference.cast() {
        Ok(ba) => ba,
        Err(_) => return None,
    };

    let mut data_ptr: *mut u8 = std::ptr::null_mut();
    let mut capacity: u32 = 0;

    if byte_access.GetBuffer(&mut data_ptr, &mut capacity).is_err() {
        return None;
    }

    if data_ptr.is_null() || capacity == 0 {
        return None;
    }

    // Copy the data to a Vec
    let actual_len = std::cmp::min(length, capacity as usize);
    let mut result = vec![0u8; actual_len];
    std::ptr::copy_nonoverlapping(data_ptr, result.as_mut_ptr(), actual_len);

    Some(result)
}

impl Default for WindowsAudioInputProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioInputProvider for WindowsAudioInputProvider {
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

        // Validate device if specified
        if let Some(device_id) = device {
            if !self.cached_devices.iter().any(|d| d.id == device_id) {
                return Err(AudioInputError::DeviceNotFound(device_id.to_string()));
            }
        }

        // Create the audio graph
        let graph_state = self.create_audio_graph(self.target_sample_rate, device)?;
        self.graph_state = Some(graph_state);

        // Store callback state
        {
            let mut state = self.callback_state.lock().unwrap();
            *state = Some(CallbackState {
                callback,
                active: true,
            });
        }

        // Set up the quantum handler
        self.setup_quantum_handler()?;

        // Start the graph
        if let Some(ref state) = self.graph_state {
            state.graph.Start().map_err(|e| {
                AudioInputError::Provider(format!("Failed to start AudioGraph: {}", e))
            })?;
        }

        // Mark as capturing
        self.capturing.store(true, Ordering::Release);

        Ok(())
    }

    fn stop(&mut self) {
        if !self.capturing.load(Ordering::Acquire) {
            return;
        }

        self.capturing.store(false, Ordering::Release);

        // Deactivate callback first
        if let Ok(mut state) = self.callback_state.lock() {
            if let Some(ref mut s) = *state {
                s.active = false;
            }
        }

        // Drop the graph state (stops and cleans up the graph)
        self.graph_state = None;

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

impl std::fmt::Debug for WindowsAudioInputProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowsAudioInputProvider")
            .field("capturing", &self.capturing.load(Ordering::Relaxed))
            .field("target_sample_rate", &self.target_sample_rate)
            .field("supported_formats", &self.supported_formats)
            .field("device_count", &self.cached_devices.len())
            .field("has_graph", &self.graph_state.is_some())
            .finish()
    }
}

// SAFETY: WindowsAudioInputProvider is Send because:
// - graph_state is Option<AudioGraphState> which contains WinRT types
// - WinRT types are designed to be thread-safe when properly synchronized
// - capturing is Arc<AtomicBool> which is Send + Sync
// - callback_state is Arc<Mutex<...>> which is Send + Sync
// - All Windows COM operations go through proper synchronization
unsafe impl Send for WindowsAudioInputProvider {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = WindowsAudioInputProvider::new();
        assert!(!provider.is_capturing());
    }

    #[test]
    fn test_supported_formats() {
        let provider = WindowsAudioInputProvider::new();
        let formats = provider.supported_formats();
        assert!(formats.contains(&AudioFormat::Pcm16k));
        assert!(formats.contains(&AudioFormat::Pcm44k));
    }

    #[test]
    fn test_stop_when_not_capturing() {
        let mut provider = WindowsAudioInputProvider::new();
        // Should not panic when stopping while not capturing
        provider.stop();
        assert!(!provider.is_capturing());
    }

    #[test]
    fn test_default_device() {
        let provider = WindowsAudioInputProvider::new();
        // Default device may or may not exist depending on hardware
        let _default = provider.default_device();
    }

    #[test]
    fn test_start_unsupported_format() {
        let mut provider = WindowsAudioInputProvider::new();
        let result = provider.start(AudioFormat::Opus, None, Box::new(|_| {}));
        assert!(matches!(result, Err(AudioInputError::UnsupportedFormat(_))));
    }

    #[test]
    fn test_start_invalid_device() {
        let mut provider = WindowsAudioInputProvider::new();
        let result = provider.start(
            AudioFormat::Pcm16k,
            Some("invalid-device-id"),
            Box::new(|_| {}),
        );
        assert!(matches!(result, Err(AudioInputError::DeviceNotFound(_))));
    }
}
