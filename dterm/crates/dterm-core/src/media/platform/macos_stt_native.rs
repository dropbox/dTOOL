//! Native macOS speech recognition using objc2 bindings.
//!
//! This module provides native bindings to SFSpeechRecognizer for speech-to-text
//! on macOS and iOS. It uses the objc2 crate family for safe Objective-C interop.
//!
//! ## Usage
//!
//! This module is only compiled when the `macos-speech` feature is enabled.
//!
//! ## Implementation Notes
//!
//! SFSpeechRecognizer provides:
//! - On-device speech recognition (macOS 10.15+, iOS 10+)
//! - Support for multiple languages
//! - Real-time partial results during recognition
//! - Audio buffer input via SFSpeechAudioBufferRecognitionRequest
//!
//! ## Privacy Requirements
//!
//! Speech recognition requires:
//! - `NSSpeechRecognitionUsageDescription` in Info.plist
//! - User authorization via `SFSpeechRecognizer.requestAuthorization()`

use block2::RcBlock;
use objc2::exception::catch;
use objc2::rc::Retained;
use objc2::runtime::{Bool, NSObject};
use objc2::{extern_class, msg_send, AllocAnyThread, ClassType};
use objc2_foundation::{NSArray, NSError, NSLocale, NSSet, NSString};
use std::cell::RefCell;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

// Link the Speech framework
#[link(name = "Speech", kind = "framework")]
extern "C" {}

// Link AVFoundation for audio format support
#[link(name = "AVFoundation", kind = "framework")]
extern "C" {}

// ============================================================================
// Authorization status enum
// ============================================================================

/// Speech recognition authorization status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum SFSpeechRecognizerAuthorizationStatus {
    /// User has not yet been asked for authorization.
    NotDetermined = 0,
    /// User denied access.
    Denied = 1,
    /// Access restricted (e.g., parental controls).
    Restricted = 2,
    /// User authorized access.
    Authorized = 3,
}

impl From<i64> for SFSpeechRecognizerAuthorizationStatus {
    fn from(value: i64) -> Self {
        match value {
            0 => Self::NotDetermined,
            1 => Self::Denied,
            2 => Self::Restricted,
            3 => Self::Authorized,
            _ => Self::NotDetermined,
        }
    }
}

// ============================================================================
// SFSpeechRecognizer bindings
// ============================================================================

extern_class!(
    /// Binding to SFSpeechRecognizer.
    ///
    /// The main class for speech recognition on Apple platforms.
    #[unsafe(super(NSObject))]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct SFSpeechRecognizer;
);

impl SFSpeechRecognizer {
    /// Create a recognizer with the default locale.
    pub fn new() -> Option<Retained<Self>> {
        unsafe { msg_send![Self::class(), new] }
    }

    /// Create a recognizer with a specific locale.
    pub fn new_with_locale(locale: &NSLocale) -> Option<Retained<Self>> {
        unsafe { msg_send![Self::alloc(), initWithLocale: locale] }
    }

    /// Get current authorization status.
    pub fn authorization_status() -> SFSpeechRecognizerAuthorizationStatus {
        let status: i64 = unsafe { msg_send![Self::class(), authorizationStatus] };
        SFSpeechRecognizerAuthorizationStatus::from(status)
    }

    /// Check if the recognizer is available.
    pub fn is_available(&self) -> bool {
        let available: Bool = unsafe { msg_send![self, isAvailable] };
        available.as_bool()
    }

    /// Check if on-device recognition is supported.
    pub fn supports_on_device_recognition(&self) -> bool {
        let supports: Bool = unsafe { msg_send![self, supportsOnDeviceRecognition] };
        supports.as_bool()
    }

    /// Get supported locales for speech recognition.
    ///
    /// Note: Returns an NSSet, not NSArray.
    pub fn supported_locales() -> Retained<NSSet<NSLocale>> {
        unsafe { msg_send![Self::class(), supportedLocales] }
    }

    /// Start a recognition task with the given request.
    ///
    /// The result handler block is called with partial and final results.
    pub fn recognition_task_with_request(
        &self,
        request: &SFSpeechAudioBufferRecognitionRequest,
        result_handler: &RcBlock<dyn Fn(*mut SFSpeechRecognitionResult, *mut NSError) + 'static>,
    ) -> Option<Retained<SFSpeechRecognitionTask>> {
        unsafe {
            msg_send![
                self,
                recognitionTaskWithRequest: request,
                resultHandler: &**result_handler
            ]
        }
    }
}

// ============================================================================
// SFSpeechAudioBufferRecognitionRequest bindings
// ============================================================================

extern_class!(
    /// Binding to SFSpeechAudioBufferRecognitionRequest.
    ///
    /// A request for recognizing speech from audio buffers.
    #[unsafe(super(NSObject))]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct SFSpeechAudioBufferRecognitionRequest;
);

impl SFSpeechAudioBufferRecognitionRequest {
    /// Create a new audio buffer recognition request.
    pub fn new() -> Retained<Self> {
        unsafe { msg_send![Self::class(), new] }
    }

    /// Set whether to report partial results.
    pub fn set_should_report_partial_results(&self, value: bool) {
        unsafe {
            let _: () = msg_send![self, setShouldReportPartialResults: Bool::new(value)];
        }
    }

    /// Set whether to require on-device recognition.
    pub fn set_requires_on_device_recognition(&self, value: bool) {
        unsafe {
            let _: () = msg_send![self, setRequiresOnDeviceRecognition: Bool::new(value)];
        }
    }

    /// Append audio data in PCM buffer format.
    ///
    /// Returns an error if the Speech framework throws an exception.
    pub fn append_audio_pcm_buffer(&self, buffer: &AVAudioPCMBuffer) -> Result<(), String> {
        let result = catch(AssertUnwindSafe(|| unsafe {
            let _: () = msg_send![self, appendAudioPCMBuffer: buffer];
        }));
        result.map_err(|e| {
            if let Some(exc) = e {
                format!("Objective-C exception: {:?}", exc)
            } else {
                "Unknown Objective-C exception".to_string()
            }
        })
    }

    /// End the audio input (signals that no more audio will be provided).
    ///
    /// Returns an error if the Speech framework throws an exception.
    pub fn end_audio(&self) -> Result<(), String> {
        let result = catch(AssertUnwindSafe(|| unsafe {
            let _: () = msg_send![self, endAudio];
        }));
        result.map_err(|e| {
            if let Some(exc) = e {
                format!("Objective-C exception: {:?}", exc)
            } else {
                "Unknown Objective-C exception".to_string()
            }
        })
    }
}

// ============================================================================
// SFSpeechRecognitionTask bindings
// ============================================================================

/// Task state for speech recognition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
#[allow(dead_code)]
pub enum SFSpeechRecognitionTaskState {
    /// Task is starting.
    Starting = 0,
    /// Task is running.
    Running = 1,
    /// Task is finishing.
    Finishing = 2,
    /// Task has been cancelled.
    Cancelling = 3,
    /// Task has completed.
    Completed = 4,
}

extern_class!(
    /// Binding to SFSpeechRecognitionTask.
    ///
    /// Represents an ongoing speech recognition task.
    #[unsafe(super(NSObject))]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct SFSpeechRecognitionTask;
);

impl SFSpeechRecognitionTask {
    /// Get the current state of the task.
    #[allow(dead_code)]
    pub fn state(&self) -> SFSpeechRecognitionTaskState {
        let state: i64 = unsafe { msg_send![self, state] };
        match state {
            0 => SFSpeechRecognitionTaskState::Starting,
            1 => SFSpeechRecognitionTaskState::Running,
            2 => SFSpeechRecognitionTaskState::Finishing,
            3 => SFSpeechRecognitionTaskState::Cancelling,
            4 => SFSpeechRecognitionTaskState::Completed,
            _ => SFSpeechRecognitionTaskState::Starting,
        }
    }

    /// Check if the task has been cancelled.
    #[allow(dead_code)]
    pub fn is_cancelled(&self) -> bool {
        let cancelled: Bool = unsafe { msg_send![self, isCancelled] };
        cancelled.as_bool()
    }

    /// Cancel the recognition task.
    pub fn cancel(&self) {
        unsafe {
            let _: () = msg_send![self, cancel];
        }
    }

    /// Finish the recognition task (wait for final result).
    pub fn finish(&self) {
        unsafe {
            let _: () = msg_send![self, finish];
        }
    }
}

// ============================================================================
// SFSpeechRecognitionResult bindings
// ============================================================================

extern_class!(
    /// Binding to SFSpeechRecognitionResult.
    ///
    /// The result of a speech recognition request.
    #[unsafe(super(NSObject))]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct SFSpeechRecognitionResult;
);

impl SFSpeechRecognitionResult {
    /// Get the best transcription.
    pub fn best_transcription(&self) -> Retained<SFTranscription> {
        unsafe { msg_send![self, bestTranscription] }
    }

    /// Check if this is the final result.
    pub fn is_final(&self) -> bool {
        let is_final: Bool = unsafe { msg_send![self, isFinal] };
        is_final.as_bool()
    }

    /// Get all transcriptions (sorted by confidence, highest first).
    #[allow(dead_code)]
    pub fn transcriptions(&self) -> Retained<NSArray<SFTranscription>> {
        unsafe { msg_send![self, transcriptions] }
    }
}

// ============================================================================
// SFTranscription bindings
// ============================================================================

extern_class!(
    /// Binding to SFTranscription.
    ///
    /// A transcribed string with segment information.
    #[unsafe(super(NSObject))]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct SFTranscription;
);

impl SFTranscription {
    /// Get the transcribed text.
    pub fn formatted_string(&self) -> Retained<NSString> {
        unsafe { msg_send![self, formattedString] }
    }

    /// Get the individual segments.
    pub fn segments(&self) -> Retained<NSArray<SFTranscriptionSegment>> {
        unsafe { msg_send![self, segments] }
    }
}

// ============================================================================
// SFTranscriptionSegment bindings
// ============================================================================

extern_class!(
    /// Binding to SFTranscriptionSegment.
    ///
    /// A segment of transcribed audio with timing and confidence.
    #[unsafe(super(NSObject))]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct SFTranscriptionSegment;
);

impl SFTranscriptionSegment {
    /// Get the substring for this segment.
    pub fn substring(&self) -> Retained<NSString> {
        unsafe { msg_send![self, substring] }
    }

    /// Get the confidence (0.0 to 1.0).
    pub fn confidence(&self) -> f32 {
        unsafe { msg_send![self, confidence] }
    }

    /// Get the timestamp in seconds.
    pub fn timestamp(&self) -> f64 {
        unsafe { msg_send![self, timestamp] }
    }

    /// Get the duration in seconds.
    pub fn duration(&self) -> f64 {
        unsafe { msg_send![self, duration] }
    }
}

// ============================================================================
// AVAudioFormat and AVAudioPCMBuffer bindings
// ============================================================================

extern_class!(
    /// Binding to AVAudioFormat.
    ///
    /// Audio format specification.
    #[unsafe(super(NSObject))]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct AVAudioFormat;
);

impl AVAudioFormat {
    /// Create a standard PCM format.
    ///
    /// This creates a non-interleaved float format suitable for speech recognition.
    pub fn new_standard_format(sample_rate: f64, channels: u32) -> Option<Retained<Self>> {
        unsafe {
            msg_send![Self::alloc(), initStandardFormatWithSampleRate: sample_rate, channels: channels]
        }
    }

    /// Create a PCM format with specific settings.
    ///
    /// Common format for speech: 16-bit signed integer, mono, 16kHz.
    #[allow(dead_code)]
    pub fn new_with_common_format(
        sample_rate: f64,
        channels: u32,
        interleaved: bool,
    ) -> Option<Retained<Self>> {
        // AVAudioCommonFormat: 1 = PCMFormatFloat32, 3 = PCMFormatInt16
        let format: u32 = 1; // PCMFormatFloat32
        unsafe {
            msg_send![
                Self::alloc(),
                initWithCommonFormat: format,
                sampleRate: sample_rate,
                channels: channels,
                interleaved: Bool::new(interleaved)
            ]
        }
    }

    /// Get the sample rate.
    #[allow(dead_code)]
    pub fn sample_rate(&self) -> f64 {
        unsafe { msg_send![self, sampleRate] }
    }

    /// Get the number of channels.
    #[allow(dead_code)]
    pub fn channel_count(&self) -> u32 {
        unsafe { msg_send![self, channelCount] }
    }
}

extern_class!(
    /// Binding to AVAudioPCMBuffer.
    ///
    /// A buffer of audio samples in PCM format.
    #[unsafe(super(NSObject))]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct AVAudioPCMBuffer;
);

impl AVAudioPCMBuffer {
    /// Create a new PCM buffer with the given format and capacity.
    pub fn new_with_format(format: &AVAudioFormat, frame_capacity: u32) -> Option<Retained<Self>> {
        unsafe {
            msg_send![Self::alloc(), initWithPCMFormat: format, frameCapacity: frame_capacity]
        }
    }

    /// Get the frame length.
    #[allow(dead_code)]
    pub fn frame_length(&self) -> u32 {
        unsafe { msg_send![self, frameLength] }
    }

    /// Set the frame length.
    pub fn set_frame_length(&self, length: u32) {
        unsafe {
            let _: () = msg_send![self, setFrameLength: length];
        }
    }

    /// Get the frame capacity.
    #[allow(dead_code)]
    pub fn frame_capacity(&self) -> u32 {
        unsafe { msg_send![self, frameCapacity] }
    }

    /// Get pointer to float channel data.
    ///
    /// Returns a pointer to an array of float pointers, one per channel.
    /// For mono audio, use index 0.
    pub fn float_channel_data(&self) -> *mut *mut f32 {
        unsafe { msg_send![self, floatChannelData] }
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Get the list of supported locale identifiers for speech recognition.
pub fn get_supported_locales() -> Vec<String> {
    let locales = SFSpeechRecognizer::supported_locales();
    let mut result = Vec::new();

    // Iterate over NSSet using allObjects to get an NSArray
    let all_objects: Retained<objc2_foundation::NSArray<NSLocale>> =
        unsafe { msg_send![&*locales, allObjects] };
    let count: usize = all_objects.count();
    for i in 0..count {
        let locale: Retained<NSLocale> = all_objects.objectAtIndex(i);
        let identifier: Retained<NSString> = unsafe { msg_send![&*locale, localeIdentifier] };
        result.push(identifier.to_string());
    }

    result
}

/// Check if speech recognition is authorized.
pub fn is_authorized() -> bool {
    matches!(
        SFSpeechRecognizer::authorization_status(),
        SFSpeechRecognizerAuthorizationStatus::Authorized
    )
}

/// Request speech recognition authorization synchronously.
///
/// Triggers the system authorization dialog if not already determined,
/// and blocks until authorization is granted or denied.
/// Returns true if authorized, false otherwise.
///
/// Note: The app's Info.plist must include `NSSpeechRecognitionUsageDescription`.
pub fn request_authorization_sync() -> bool {
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::Duration;

    // Check if already authorized
    if is_authorized() {
        return true;
    }

    // Check if status is not_determined - only then can we request
    let current_status = SFSpeechRecognizer::authorization_status();
    if current_status != SFSpeechRecognizerAuthorizationStatus::NotDetermined {
        // Already denied or restricted - can't change via request
        return false;
    }

    let pair = Arc::new((Mutex::new(None::<bool>), Condvar::new()));
    let pair_clone = Arc::clone(&pair);

    // Create a block that can be called multiple times (Fn, not FnOnce)
    // We use Arc<Mutex<Option<_>>> to ensure only the first call takes effect
    let block = RcBlock::new(move |status: i64| {
        let auth_status = SFSpeechRecognizerAuthorizationStatus::from(status);
        let (lock, cvar) = &*pair_clone;
        let mut authorized = lock.lock().unwrap();
        if authorized.is_none() {
            *authorized = Some(matches!(
                auth_status,
                SFSpeechRecognizerAuthorizationStatus::Authorized
            ));
            cvar.notify_one();
        }
    });

    unsafe {
        let _: () = msg_send![
            SFSpeechRecognizer::class(),
            requestAuthorization: &*block
        ];
    }

    // Wait for result with timeout
    let (lock, cvar) = &*pair;
    let authorized = lock.lock().unwrap();

    // Wait up to 60 seconds for user to respond to authorization dialog
    let result =
        cvar.wait_timeout_while(authorized, Duration::from_secs(60), |auth| auth.is_none());

    match result {
        Ok((auth, _)) => auth.unwrap_or(false),
        Err(_) => false,
    }
}

// ============================================================================
// Recognition result types (for API compatibility)
// ============================================================================

/// Recognition result from the callback.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RecognitionResult {
    /// The transcribed text.
    pub text: String,
    /// Confidence score (0.0 - 1.0, if available).
    pub confidence: Option<f32>,
    /// Whether this is a final result.
    pub is_final: bool,
    /// Individual word segments with timing.
    pub segments: Vec<RecognitionSegment>,
}

/// A word segment with timing information.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RecognitionSegment {
    /// The word or phrase.
    pub text: String,
    /// Confidence score.
    pub confidence: f32,
    /// Start time in seconds.
    pub timestamp: f64,
    /// Duration in seconds.
    pub duration: f64,
}

// ============================================================================
// Thread-safe recognizer wrapper with full recognition support
// ============================================================================

/// Shared state for the recognition callback.
struct RecognitionState {
    results: Vec<RecognitionResult>,
    error: Option<String>,
    completed: bool,
}

/// Thread-safe speech recognizer wrapper.
///
/// This implementation uses `block2` for Objective-C block callbacks and
/// provides full speech recognition functionality via SFSpeechRecognizer.
///
/// ## Usage
///
/// ```ignore
/// let mut recognizer = ThreadSafeRecognizer::new()?;
/// recognizer.start(true, true)?;  // on-device, partial results
///
/// // Feed audio data (16kHz mono float32)
/// recognizer.feed_audio(&audio_samples)?;
///
/// // Get partial results as they become available
/// if let Some(partial) = recognizer.get_partial() {
///     println!("Partial: {}", partial.text);
/// }
///
/// // Stop and get final result
/// let final_result = recognizer.stop()?;
/// ```
pub struct ThreadSafeRecognizer {
    /// The underlying recognizer.
    recognizer: Option<Retained<SFSpeechRecognizer>>,
    /// Current recognition request.
    request: Option<Retained<SFSpeechAudioBufferRecognitionRequest>>,
    /// Current recognition task.
    task: Option<Retained<SFSpeechRecognitionTask>>,
    /// Audio format for buffers.
    audio_format: Option<Retained<AVAudioFormat>>,
    /// Shared state for callback results.
    state: Arc<Mutex<RecognitionState>>,
    /// Whether recognition is active (shared with callback).
    active: Arc<AtomicBool>,
    /// Sample rate for audio conversion.
    sample_rate: f64,
}

impl ThreadSafeRecognizer {
    /// Create a new recognizer for the default locale.
    pub fn new() -> Option<Self> {
        let recognizer = SFSpeechRecognizer::new()?;

        Some(Self {
            recognizer: Some(recognizer),
            request: None,
            task: None,
            audio_format: None,
            state: Arc::new(Mutex::new(RecognitionState {
                results: Vec::new(),
                error: None,
                completed: false,
            })),
            active: Arc::new(AtomicBool::new(false)),
            sample_rate: 16000.0,
        })
    }

    /// Create a recognizer for a specific locale.
    pub fn with_locale(locale_id: &str) -> Option<Self> {
        // Create NSLocale from identifier
        let ns_locale_id = NSString::from_str(locale_id);
        let locale: Retained<NSLocale> =
            unsafe { msg_send![NSLocale::alloc(), initWithLocaleIdentifier: &*ns_locale_id] };

        let recognizer = SFSpeechRecognizer::new_with_locale(&locale)?;

        Some(Self {
            recognizer: Some(recognizer),
            request: None,
            task: None,
            audio_format: None,
            state: Arc::new(Mutex::new(RecognitionState {
                results: Vec::new(),
                error: None,
                completed: false,
            })),
            active: Arc::new(AtomicBool::new(false)),
            sample_rate: 16000.0,
        })
    }

    /// Set the sample rate for audio conversion.
    pub fn set_sample_rate(&mut self, rate: f64) {
        self.sample_rate = rate;
    }

    /// Check if the recognizer is available.
    pub fn is_available(&self) -> bool {
        self.recognizer
            .as_ref()
            .map(|r| r.is_available())
            .unwrap_or(false)
    }

    /// Check if on-device recognition is supported.
    pub fn supports_on_device(&self) -> bool {
        self.recognizer
            .as_ref()
            .map(|r| r.supports_on_device_recognition())
            .unwrap_or(false)
    }

    /// Start a recognition session.
    ///
    /// - `on_device`: Whether to require on-device recognition (no network).
    /// - `partial_results`: Whether to report partial results during recognition.
    pub fn start(&mut self, on_device: bool, partial_results: bool) -> Result<(), String> {
        if !is_authorized() {
            return Err("Speech recognition not authorized".to_string());
        }

        if self.active.load(Ordering::Acquire) {
            return Err("Recognition already active".to_string());
        }

        if !self.is_available() {
            return Err("Recognizer not available".to_string());
        }

        let recognizer = self.recognizer.as_ref().ok_or("No recognizer available")?;

        // Clear previous state
        {
            let mut state = self.state.lock().unwrap();
            state.results.clear();
            state.error = None;
            state.completed = false;
        }

        // Create audio format (float32, mono, at configured sample rate)
        let format = AVAudioFormat::new_standard_format(self.sample_rate, 1)
            .ok_or("Failed to create audio format")?;
        self.audio_format = Some(format);

        // Create recognition request
        let request = SFSpeechAudioBufferRecognitionRequest::new();
        request.set_should_report_partial_results(partial_results);
        request.set_requires_on_device_recognition(on_device);

        // Create callback block for results
        let state_clone = Arc::clone(&self.state);
        let active_clone = self.active.clone();

        // Use RefCell for interior mutability in the callback
        let state_cell = RefCell::new(state_clone);

        let result_handler = RcBlock::new(
            move |result: *mut SFSpeechRecognitionResult, error: *mut NSError| {
                let state = state_cell.borrow();
                let mut locked_state = state.lock().unwrap();

                if !error.is_null() {
                    let err: &NSError = unsafe { &*error };
                    let desc: Retained<NSString> = unsafe { msg_send![err, localizedDescription] };
                    locked_state.error = Some(desc.to_string());
                    locked_state.completed = true;
                    active_clone.store(false, Ordering::Release);
                    return;
                }

                if !result.is_null() {
                    let res: &SFSpeechRecognitionResult = unsafe { &*result };
                    let transcription = res.best_transcription();
                    let text = transcription.formatted_string().to_string();

                    // Get segments with timing info
                    let segments_array = transcription.segments();
                    let mut segments = Vec::new();
                    let count = segments_array.count();
                    for i in 0..count {
                        let seg: Retained<SFTranscriptionSegment> = segments_array.objectAtIndex(i);
                        segments.push(RecognitionSegment {
                            text: seg.substring().to_string(),
                            confidence: seg.confidence(),
                            timestamp: seg.timestamp(),
                            duration: seg.duration(),
                        });
                    }

                    // Calculate average confidence from segments
                    #[allow(clippy::cast_precision_loss)]
                    let avg_confidence = if segments.is_empty() {
                        None
                    } else {
                        let sum: f32 = segments.iter().map(|s| s.confidence).sum();
                        Some(sum / segments.len() as f32)
                    };

                    let recognition_result = RecognitionResult {
                        text,
                        confidence: avg_confidence,
                        is_final: res.is_final(),
                        segments,
                    };

                    locked_state.results.push(recognition_result);

                    if res.is_final() {
                        locked_state.completed = true;
                        active_clone.store(false, Ordering::Release);
                    }
                }
            },
        );

        // Start recognition task
        // The recognition task callbacks will be delivered on the Speech framework's internal queue
        let task = recognizer
            .recognition_task_with_request(&request, &result_handler)
            .ok_or("Failed to create recognition task")?;

        self.request = Some(request);
        self.task = Some(task);
        self.active.store(true, Ordering::Release);

        Ok(())
    }

    /// Feed audio data to the recognizer.
    ///
    /// Audio should be float32 samples at the configured sample rate (default 16kHz).
    pub fn feed_audio(&mut self, data: &[u8]) -> Result<(), String> {
        if !self.active.load(Ordering::Acquire) {
            return Err("Recognition not active".to_string());
        }

        let request = self.request.as_ref().ok_or("No active request")?;
        let format = self.audio_format.as_ref().ok_or("No audio format")?;

        // Convert bytes to float32 samples
        // Assuming data is already float32 samples (4 bytes per sample)
        let num_samples = data.len() / 4;
        if num_samples == 0 {
            return Ok(());
        }

        // Limit buffer size to prevent truncation issues on 64-bit systems
        let num_samples_u32 = u32::try_from(num_samples).map_err(|_| "Audio buffer too large")?;

        // Create PCM buffer
        let buffer = AVAudioPCMBuffer::new_with_format(format, num_samples_u32)
            .ok_or("Failed to create audio buffer")?;

        buffer.set_frame_length(num_samples_u32);

        // Copy data to buffer
        // The data must be properly aligned to f32 boundary
        let channel_data = buffer.float_channel_data();
        if channel_data.is_null() {
            return Err("Failed to get buffer channel data".to_string());
        }

        // Verify alignment and copy safely
        if data.as_ptr().align_offset(std::mem::align_of::<f32>()) != 0 {
            // Data is not aligned, copy byte by byte through a temp buffer
            let float_samples: Vec<f32> = data
                .chunks_exact(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();

            unsafe {
                let float_ptr = *channel_data;
                if !float_ptr.is_null() {
                    std::ptr::copy_nonoverlapping(float_samples.as_ptr(), float_ptr, num_samples);
                }
            }
        } else {
            // Data is properly aligned, copy directly
            #[allow(clippy::cast_ptr_alignment)]
            unsafe {
                let float_ptr = *channel_data;
                if !float_ptr.is_null() {
                    std::ptr::copy_nonoverlapping(
                        data.as_ptr().cast::<f32>(),
                        float_ptr,
                        num_samples,
                    );
                }
            }
        }

        // Append to recognition request
        request.append_audio_pcm_buffer(&buffer)?;

        Ok(())
    }

    /// Feed audio data as i16 PCM samples.
    ///
    /// Converts 16-bit signed integer samples to float32 for the recognizer.
    #[allow(dead_code)]
    pub fn feed_audio_i16(&mut self, samples: &[i16]) -> Result<(), String> {
        if samples.is_empty() {
            return Ok(());
        }

        // Convert i16 to f32 (normalize to -1.0 to 1.0)
        let float_samples: Vec<f32> = samples.iter().map(|&s| f32::from(s) / 32768.0).collect();

        // Convert to bytes
        let bytes: Vec<u8> = float_samples
            .iter()
            .flat_map(|&f| f.to_le_bytes())
            .collect();

        self.feed_audio(&bytes)
    }

    /// Get the latest partial result (if any).
    pub fn get_partial(&self) -> Option<RecognitionResult> {
        let state = self.state.lock().unwrap();
        state.results.last().filter(|r| !r.is_final).cloned()
    }

    /// Get all collected results.
    #[allow(dead_code)]
    pub fn get_results(&self) -> Vec<RecognitionResult> {
        let state = self.state.lock().unwrap();
        state.results.clone()
    }

    /// Get the final result (if recognition is complete).
    #[allow(dead_code)]
    pub fn get_final(&self) -> Option<RecognitionResult> {
        let state = self.state.lock().unwrap();
        state.results.iter().find(|r| r.is_final).cloned()
    }

    /// Get the last error (if any).
    #[allow(dead_code)]
    pub fn get_error(&self) -> Option<String> {
        let state = self.state.lock().unwrap();
        state.error.clone()
    }

    /// Stop recognition and get final result.
    pub fn stop(&mut self) -> Result<Option<RecognitionResult>, String> {
        if !self.active.load(Ordering::Acquire) {
            // Check if we have results from a completed session
            let state = self.state.lock().unwrap();
            if state.completed {
                return Ok(state.results.iter().find(|r| r.is_final).cloned());
            }
            return Err("Recognition not active".to_string());
        }

        // Signal end of audio
        if let Some(ref request) = self.request {
            if let Err(e) = request.end_audio() {
                // Log but don't fail - we still want to try getting results
                eprintln!("Warning: end_audio failed: {}", e);
            }
        }

        // Wait briefly for final result to arrive
        // In a real implementation, you'd want to use a proper async mechanism
        for _ in 0..50 {
            {
                let state = self.state.lock().unwrap();
                if state.completed || state.error.is_some() {
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        self.active.store(false, Ordering::Release);

        // Clean up
        if let Some(ref task) = self.task {
            task.finish();
        }
        self.task = None;
        self.request = None;

        // Return final result or error
        let state = self.state.lock().unwrap();
        if let Some(ref error) = state.error {
            return Err(error.clone());
        }

        Ok(state.results.iter().find(|r| r.is_final).cloned())
    }

    /// Cancel recognition without getting results.
    pub fn cancel(&mut self) {
        self.active.store(false, Ordering::Release);

        // Cancel the task
        if let Some(ref task) = self.task {
            task.cancel();
        }

        // Clean up
        self.task = None;
        self.request = None;

        // Clear results
        let mut state = self.state.lock().unwrap();
        state.results.clear();
        state.error = None;
        state.completed = false;
    }

    /// Check if recognition is currently active.
    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    /// Check if recognition has completed.
    #[allow(dead_code)]
    pub fn is_completed(&self) -> bool {
        let state = self.state.lock().unwrap();
        state.completed
    }
}

impl Default for ThreadSafeRecognizer {
    fn default() -> Self {
        Self::new().unwrap_or(Self {
            recognizer: None,
            request: None,
            task: None,
            audio_format: None,
            state: Arc::new(Mutex::new(RecognitionState {
                results: Vec::new(),
                error: None,
                completed: false,
            })),
            active: Arc::new(AtomicBool::new(false)),
            sample_rate: 16000.0,
        })
    }
}

impl std::fmt::Debug for ThreadSafeRecognizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThreadSafeRecognizer")
            .field("active", &self.active.load(Ordering::Relaxed))
            .field("sample_rate", &self.sample_rate)
            .field("is_available", &self.is_available())
            .field("has_task", &self.task.is_some())
            .finish()
    }
}

// SAFETY: ThreadSafeRecognizer's mutable state is protected by:
// - Arc<Mutex<RecognitionState>> for shared callback state
// - AtomicBool for active flag
// - Objective-C objects are retained and thread-safe when accessed properly
unsafe impl Send for ThreadSafeRecognizer {}
unsafe impl Sync for ThreadSafeRecognizer {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authorization_status() {
        let status = SFSpeechRecognizer::authorization_status();
        // Status should be a valid value
        assert!(matches!(
            status,
            SFSpeechRecognizerAuthorizationStatus::NotDetermined
                | SFSpeechRecognizerAuthorizationStatus::Denied
                | SFSpeechRecognizerAuthorizationStatus::Restricted
                | SFSpeechRecognizerAuthorizationStatus::Authorized
        ));
    }

    #[test]
    fn test_supported_locales() {
        let locales = get_supported_locales();
        // Should have at least some locales
        assert!(
            !locales.is_empty(),
            "Should have at least one supported locale"
        );

        // en-US should be supported
        assert!(
            locales.iter().any(|l| l.starts_with("en")),
            "English should be supported"
        );
    }

    #[test]
    fn test_recognizer_creation() {
        let recognizer = SFSpeechRecognizer::new();
        // Just check that it doesn't crash
        if let Some(r) = recognizer {
            let _ = r.is_available();
            let _ = r.supports_on_device_recognition();
        }
    }

    #[test]
    fn test_thread_safe_recognizer() {
        let recognizer = ThreadSafeRecognizer::new();
        // May be None if Speech framework not available
        if let Some(r) = recognizer {
            // Check basic properties
            let _ = r.is_available();
            let _ = r.supports_on_device();
            assert!(!r.is_active());
        }
    }

    #[test]
    fn test_audio_format_creation() {
        let format = AVAudioFormat::new_standard_format(16000.0, 1);
        assert!(format.is_some(), "Should create audio format");

        if let Some(f) = format {
            assert!((f.sample_rate() - 16000.0).abs() < f64::EPSILON);
            assert_eq!(f.channel_count(), 1);
        }
    }

    #[test]
    fn test_recognition_request_creation() {
        let request = SFSpeechAudioBufferRecognitionRequest::new();
        request.set_should_report_partial_results(true);
        request.set_requires_on_device_recognition(false);
        // Should not panic
    }

    #[test]
    fn test_pcm_buffer_creation() {
        let format = AVAudioFormat::new_standard_format(16000.0, 1);
        if let Some(f) = format {
            let buffer = AVAudioPCMBuffer::new_with_format(&f, 1024);
            assert!(buffer.is_some(), "Should create PCM buffer");

            if let Some(b) = buffer {
                assert_eq!(b.frame_capacity(), 1024);
                b.set_frame_length(512);
                assert_eq!(b.frame_length(), 512);
            }
        }
    }
}
