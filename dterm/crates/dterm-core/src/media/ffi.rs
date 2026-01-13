//! C FFI bindings for media platform hooks.
//!
//! This module provides C-callable functions that allow platform layers (Swift, C#, etc.)
//! to register native STT/TTS providers with the dterm-core media server.
//!
//! ## Architecture
//!
//! The FFI layer uses a callback-based design:
//!
//! 1. Platform layer registers callbacks for STT/TTS operations
//! 2. dterm-core calls these callbacks when voice I/O is needed
//! 3. Platform layer uses native APIs (Speech framework on macOS, etc.)
//! 4. Results flow back via completion callbacks
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │  Platform Layer (Swift)                                          │
//! │  ┌─────────────────────┐   ┌─────────────────────┐              │
//! │  │ SFSpeechRecognizer  │   │ AVSpeechSynthesizer │              │
//! │  └─────────────────────┘   └─────────────────────┘              │
//! │           │                         │                           │
//! │     Native callbacks          Native callbacks                  │
//! └───────────┼─────────────────────────┼───────────────────────────┘
//!             │                         │
//!             ▼                         ▼
//! ┌───────────────────────────────────────────────────────────────────┐
//! │  dterm-core FFI                                                   │
//! │  ┌─────────────────────┐   ┌─────────────────────┐               │
//! │  │ FfiSttProvider      │   │ FfiTtsProvider      │               │
//! │  │ (callback wrapper)  │   │ (callback wrapper)  │               │
//! │  └─────────────────────┘   └─────────────────────┘               │
//! │           │                         │                            │
//! │     implements                implements                         │
//! │     SttProvider               TtsProvider                        │
//! └───────────┼─────────────────────────┼────────────────────────────┘
//!             │                         │
//!             ▼                         ▼
//! ┌───────────────────────────────────────────────────────────────────┐
//! │  MediaServer                                                      │
//! │  Uses providers through trait interface                          │
//! └───────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage from Swift
//!
//! ```swift
//! // Register STT callbacks
//! dterm_media_register_stt_callbacks(
//!     context,
//!     startCallback,
//!     feedAudioCallback,
//!     stopCallback,
//!     cancelCallback
//! )
//!
//! // Register TTS callbacks
//! dterm_media_register_tts_callbacks(
//!     context,
//!     synthesizeCallback,
//!     streamStartCallback,
//!     streamReadCallback,
//!     streamStopCallback
//! )
//! ```

use super::platform::{SttProvider, SttProviderError, TtsProvider, TtsProviderError, VoiceInfo};
use super::stt::SttResult;
use super::AudioFormat;
use std::ffi::{c_char, c_void, CStr};
use std::ptr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Placeholder client ID used by FFI providers.
/// The actual client ID is tracked at the MediaServer level.
const FFI_PLACEHOLDER_CLIENT: u64 = 0;

// =============================================================================
// FFI Types
// =============================================================================

/// Audio format for FFI (matches AudioFormat enum).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtermAudioFormat {
    /// 16-bit PCM at 16kHz (common for STT).
    Pcm16k = 0,
    /// 16-bit PCM at 44.1kHz (high quality).
    Pcm44k = 1,
    /// Opus codec (efficient for streaming).
    Opus = 2,
    /// AAC codec (Apple ecosystem).
    Aac = 3,
}

impl From<DtermAudioFormat> for AudioFormat {
    fn from(fmt: DtermAudioFormat) -> Self {
        match fmt {
            DtermAudioFormat::Pcm16k => AudioFormat::Pcm16k,
            DtermAudioFormat::Pcm44k => AudioFormat::Pcm44k,
            DtermAudioFormat::Opus => AudioFormat::Opus,
            DtermAudioFormat::Aac => AudioFormat::Aac,
        }
    }
}

impl From<AudioFormat> for DtermAudioFormat {
    fn from(fmt: AudioFormat) -> Self {
        match fmt {
            AudioFormat::Pcm16k => DtermAudioFormat::Pcm16k,
            AudioFormat::Pcm44k => DtermAudioFormat::Pcm44k,
            AudioFormat::Opus => DtermAudioFormat::Opus,
            AudioFormat::Aac => DtermAudioFormat::Aac,
        }
    }
}

/// STT result for FFI.
#[repr(C)]
pub struct DtermSttResult {
    /// Recognized text (null-terminated UTF-8, caller-owned).
    pub text: *mut c_char,
    /// Confidence score (0-100).
    pub confidence: u8,
    /// Whether this is a final result.
    pub is_final: bool,
}

/// Error code for FFI callbacks.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtermMediaError {
    /// Operation succeeded.
    Ok = 0,
    /// Provider not initialized.
    NotInitialized = 1,
    /// Audio format not supported.
    UnsupportedFormat = 2,
    /// Language not supported.
    UnsupportedLanguage = 3,
    /// Speech recognition failed.
    RecognitionFailed = 4,
    /// Voice not found.
    VoiceNotFound = 5,
    /// Speech synthesis failed.
    SynthesisFailed = 6,
    /// Provider-specific error.
    ProviderError = 7,
    /// Operation was cancelled.
    Cancelled = 8,
}

// =============================================================================
// STT Callback Types
// =============================================================================

/// Callback to start STT session.
/// Returns DtermMediaError::Ok on success.
pub type DtermSttStartCallback = extern "C" fn(
    context: *mut c_void,
    format: DtermAudioFormat,
    language: *const c_char, // null = default
) -> DtermMediaError;

/// Callback to feed audio data to STT.
/// Returns DtermMediaError::Ok on success.
pub type DtermSttFeedCallback =
    extern "C" fn(context: *mut c_void, data: *const u8, len: usize) -> DtermMediaError;

/// Callback to get partial STT result.
/// Returns null if no partial result available.
pub type DtermSttPartialCallback = extern "C" fn(context: *mut c_void) -> *mut DtermSttResult;

/// Callback to stop STT and get final result.
/// Returns null on error.
pub type DtermSttStopCallback = extern "C" fn(context: *mut c_void) -> *mut DtermSttResult;

/// Callback to cancel STT session.
pub type DtermSttCancelCallback = extern "C" fn(context: *mut c_void);

/// Callback to check voice activity.
/// Returns 0 = inactive, 1 = active, -1 = unsupported.
pub type DtermSttVadCallback = extern "C" fn(context: *mut c_void) -> i8;

// =============================================================================
// TTS Callback Types
// =============================================================================

/// Callback to synthesize speech (blocking).
/// Returns audio data length, writes to buffer. Returns 0 on error.
pub type DtermTtsSynthesizeCallback = extern "C" fn(
    context: *mut c_void,
    text: *const c_char,
    format: DtermAudioFormat,
    voice: *const c_char, // null = default
    out_buffer: *mut u8,
    buffer_len: usize,
) -> usize;

/// Callback to start streaming TTS.
/// Returns DtermMediaError::Ok on success.
pub type DtermTtsStreamStartCallback = extern "C" fn(
    context: *mut c_void,
    text: *const c_char,
    format: DtermAudioFormat,
    voice: *const c_char, // null = default
) -> DtermMediaError;

/// Callback to read TTS stream chunk.
/// Returns bytes written, or 0 if complete, or -1 on error.
pub type DtermTtsStreamReadCallback =
    extern "C" fn(context: *mut c_void, buffer: *mut u8, buffer_len: usize) -> isize;

/// Callback to stop TTS streaming.
pub type DtermTtsStreamStopCallback = extern "C" fn(context: *mut c_void);

// =============================================================================
// FFI Provider State
// =============================================================================

/// STT callbacks registered by platform layer.
struct SttCallbacks {
    context: *mut c_void,
    start: DtermSttStartCallback,
    feed: DtermSttFeedCallback,
    partial: DtermSttPartialCallback,
    stop: DtermSttStopCallback,
    cancel: DtermSttCancelCallback,
    vad: Option<DtermSttVadCallback>,
    /// Reserved for future use (query supported formats).
    #[allow(dead_code)]
    supported_formats: Vec<AudioFormat>,
    /// Reserved for future use (query supported languages).
    #[allow(dead_code)]
    supported_languages: Vec<String>,
}

// SAFETY: The context pointer is only dereferenced in callback functions
// that are provided by the platform layer, which guarantees thread safety.
unsafe impl Send for SttCallbacks {}
unsafe impl Sync for SttCallbacks {}

/// TTS callbacks registered by platform layer.
struct TtsCallbacks {
    context: *mut c_void,
    synthesize: DtermTtsSynthesizeCallback,
    stream_start: DtermTtsStreamStartCallback,
    stream_read: DtermTtsStreamReadCallback,
    stream_stop: DtermTtsStreamStopCallback,
    /// Reserved for future use (query supported formats).
    #[allow(dead_code)]
    supported_formats: Vec<AudioFormat>,
    /// Reserved for future use (query available voices).
    #[allow(dead_code)]
    voices: Vec<VoiceInfo>,
    rate: f32,
    pitch: f32,
    volume: f32,
}

// SAFETY: Same as SttCallbacks
unsafe impl Send for TtsCallbacks {}
unsafe impl Sync for TtsCallbacks {}

// =============================================================================
// FFI STT Provider Implementation
// =============================================================================

/// FFI-based STT provider that delegates to platform callbacks.
pub struct FfiSttProvider {
    callbacks: Arc<Mutex<Option<SttCallbacks>>>,
    active: bool,
}

impl FfiSttProvider {
    /// Create a new FFI STT provider (uninitialized until callbacks registered).
    pub fn new() -> Self {
        Self {
            callbacks: Arc::new(Mutex::new(None)),
            active: false,
        }
    }

    /// Register callbacks from the platform layer.
    ///
    /// # Safety
    ///
    /// - `context` must remain valid for the lifetime of the provider
    /// - All callback function pointers must be valid
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn register_callbacks(
        &self,
        context: *mut c_void,
        start: DtermSttStartCallback,
        feed: DtermSttFeedCallback,
        partial: DtermSttPartialCallback,
        stop: DtermSttStopCallback,
        cancel: DtermSttCancelCallback,
        vad: Option<DtermSttVadCallback>,
        formats: &[AudioFormat],
        languages: &[&str],
    ) {
        let mut guard = self.callbacks.lock().unwrap();
        *guard = Some(SttCallbacks {
            context,
            start,
            feed,
            partial,
            stop,
            cancel,
            vad,
            supported_formats: formats.to_vec(),
            supported_languages: languages.iter().map(|s| (*s).to_string()).collect(),
        });
    }

    /// Check if callbacks are registered.
    pub fn is_initialized(&self) -> bool {
        self.callbacks.lock().unwrap().is_some()
    }
}

impl Default for FfiSttProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SttProvider for FfiSttProvider {
    fn start(
        &mut self,
        format: AudioFormat,
        language: Option<&str>,
    ) -> Result<(), SttProviderError> {
        let guard = self.callbacks.lock().unwrap();
        let callbacks = guard.as_ref().ok_or(SttProviderError::NotInitialized)?;

        let lang_cstring;
        let lang_ptr = match language {
            Some(l) => {
                lang_cstring = std::ffi::CString::new(l).map_err(|_| {
                    SttProviderError::Provider("Invalid language string".to_string())
                })?;
                lang_cstring.as_ptr()
            }
            None => ptr::null(),
        };

        let result = (callbacks.start)(callbacks.context, format.into(), lang_ptr);

        match result {
            DtermMediaError::Ok => {
                drop(guard);
                self.active = true;
                Ok(())
            }
            DtermMediaError::UnsupportedFormat => Err(SttProviderError::UnsupportedFormat(format)),
            DtermMediaError::UnsupportedLanguage => Err(SttProviderError::UnsupportedLanguage(
                language.unwrap_or("").to_string(),
            )),
            _ => Err(SttProviderError::Provider(format!(
                "Start failed: {:?}",
                result
            ))),
        }
    }

    fn feed_audio(&mut self, data: &[u8]) -> Result<(), SttProviderError> {
        if !self.active {
            return Err(SttProviderError::NotInitialized);
        }

        let guard = self.callbacks.lock().unwrap();
        let callbacks = guard.as_ref().ok_or(SttProviderError::NotInitialized)?;

        let result = (callbacks.feed)(callbacks.context, data.as_ptr(), data.len());

        match result {
            DtermMediaError::Ok => Ok(()),
            DtermMediaError::RecognitionFailed => Err(SttProviderError::RecognitionFailed(
                "Feed failed".to_string(),
            )),
            _ => Err(SttProviderError::Provider(format!(
                "Feed failed: {:?}",
                result
            ))),
        }
    }

    fn get_partial(&mut self) -> Option<SttResult> {
        if !self.active {
            return None;
        }

        let guard = self.callbacks.lock().unwrap();
        let callbacks = guard.as_ref()?;

        let result_ptr = (callbacks.partial)(callbacks.context);
        if result_ptr.is_null() {
            return None;
        }

        // SAFETY: The platform layer allocated this result
        let ffi_result = unsafe { &*result_ptr };
        let text = if ffi_result.text.is_null() {
            String::new()
        } else {
            // SAFETY: Platform guarantees valid UTF-8 null-terminated string
            unsafe { CStr::from_ptr(ffi_result.text) }
                .to_string_lossy()
                .into_owned()
        };

        let result = SttResult {
            client: FFI_PLACEHOLDER_CLIENT,
            text,
            confidence: ffi_result.confidence,
            is_final: ffi_result.is_final,
        };

        // Free the FFI result
        unsafe { dterm_media_free_stt_result(result_ptr) };

        Some(result)
    }

    fn stop(&mut self) -> Result<Option<SttResult>, SttProviderError> {
        if !self.active {
            return Err(SttProviderError::NotInitialized);
        }

        let guard = self.callbacks.lock().unwrap();
        let callbacks = guard.as_ref().ok_or(SttProviderError::NotInitialized)?;

        let result_ptr = (callbacks.stop)(callbacks.context);

        drop(guard);
        self.active = false;

        if result_ptr.is_null() {
            return Ok(None);
        }

        // SAFETY: The platform layer allocated this result
        let ffi_result = unsafe { &*result_ptr };
        let text = if ffi_result.text.is_null() {
            String::new()
        } else {
            // SAFETY: Platform guarantees valid UTF-8 null-terminated string
            unsafe { CStr::from_ptr(ffi_result.text) }
                .to_string_lossy()
                .into_owned()
        };

        let result = SttResult {
            client: FFI_PLACEHOLDER_CLIENT,
            text,
            confidence: ffi_result.confidence,
            is_final: true,
        };

        // Free the FFI result
        unsafe { dterm_media_free_stt_result(result_ptr) };

        Ok(Some(result))
    }

    fn cancel(&mut self) {
        if !self.active {
            return;
        }

        let guard = self.callbacks.lock().unwrap();
        if let Some(callbacks) = guard.as_ref() {
            (callbacks.cancel)(callbacks.context);
        }

        drop(guard);
        self.active = false;
    }

    fn is_voice_active(&self) -> Option<bool> {
        if !self.active {
            return None;
        }

        let guard = self.callbacks.lock().unwrap();
        let callbacks = guard.as_ref()?;
        let vad = callbacks.vad?;

        let result = (vad)(callbacks.context);
        match result {
            0 => Some(false),
            1 => Some(true),
            _ => None, // -1 or other = unsupported
        }
    }

    fn supported_formats(&self) -> &[AudioFormat] {
        // This is a limitation - we can't return a reference to data behind a mutex
        // For now, return a static default
        static DEFAULT_FORMATS: &[AudioFormat] = &[AudioFormat::Pcm16k, AudioFormat::Pcm44k];
        DEFAULT_FORMATS
    }

    fn supported_languages(&self) -> &[String] {
        // Same limitation as above
        static DEFAULT_LANGUAGES: &[String] = &[];
        DEFAULT_LANGUAGES
    }
}

// =============================================================================
// FFI TTS Provider Implementation
// =============================================================================

/// FFI-based TTS provider that delegates to platform callbacks.
pub struct FfiTtsProvider {
    callbacks: Arc<Mutex<Option<TtsCallbacks>>>,
    streaming: bool,
}

impl FfiTtsProvider {
    /// Create a new FFI TTS provider (uninitialized until callbacks registered).
    pub fn new() -> Self {
        Self {
            callbacks: Arc::new(Mutex::new(None)),
            streaming: false,
        }
    }

    /// Register callbacks from the platform layer.
    ///
    /// # Safety
    ///
    /// - `context` must remain valid for the lifetime of the provider
    /// - All callback function pointers must be valid
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn register_callbacks(
        &self,
        context: *mut c_void,
        synthesize: DtermTtsSynthesizeCallback,
        stream_start: DtermTtsStreamStartCallback,
        stream_read: DtermTtsStreamReadCallback,
        stream_stop: DtermTtsStreamStopCallback,
        formats: &[AudioFormat],
        voices: &[VoiceInfo],
    ) {
        let mut guard = self.callbacks.lock().unwrap();
        *guard = Some(TtsCallbacks {
            context,
            synthesize,
            stream_start,
            stream_read,
            stream_stop,
            supported_formats: formats.to_vec(),
            voices: voices.to_vec(),
            rate: 1.0,
            pitch: 1.0,
            volume: 1.0,
        });
    }

    /// Check if callbacks are registered.
    pub fn is_initialized(&self) -> bool {
        self.callbacks.lock().unwrap().is_some()
    }
}

impl Default for FfiTtsProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TtsProvider for FfiTtsProvider {
    fn synthesize(
        &mut self,
        text: &str,
        format: AudioFormat,
        voice: Option<&str>,
    ) -> Result<Vec<u8>, TtsProviderError> {
        let guard = self.callbacks.lock().unwrap();
        let callbacks = guard.as_ref().ok_or(TtsProviderError::NotInitialized)?;

        let text_cstring = std::ffi::CString::new(text)
            .map_err(|_| TtsProviderError::Provider("Invalid text string".to_string()))?;

        let voice_cstring;
        let voice_ptr = match voice {
            Some(v) => {
                voice_cstring = std::ffi::CString::new(v)
                    .map_err(|_| TtsProviderError::Provider("Invalid voice string".to_string()))?;
                voice_cstring.as_ptr()
            }
            None => ptr::null(),
        };

        // Allocate a reasonable buffer (5 seconds of 16kHz 16-bit audio)
        let mut buffer = vec![0u8; 16_000 * 2 * 5];
        let bytes_written = (callbacks.synthesize)(
            callbacks.context,
            text_cstring.as_ptr(),
            format.into(),
            voice_ptr,
            buffer.as_mut_ptr(),
            buffer.len(),
        );

        if bytes_written == 0 {
            return Err(TtsProviderError::SynthesisFailed(
                "Synthesis returned no data".to_string(),
            ));
        }

        buffer.truncate(bytes_written);
        Ok(buffer)
    }

    fn start_stream(
        &mut self,
        text: &str,
        format: AudioFormat,
        voice: Option<&str>,
    ) -> Result<(), TtsProviderError> {
        if self.streaming {
            return Err(TtsProviderError::Provider("Already streaming".to_string()));
        }

        let guard = self.callbacks.lock().unwrap();
        let callbacks = guard.as_ref().ok_or(TtsProviderError::NotInitialized)?;

        let text_cstring = std::ffi::CString::new(text)
            .map_err(|_| TtsProviderError::Provider("Invalid text string".to_string()))?;

        let voice_cstring;
        let voice_ptr = match voice {
            Some(v) => {
                voice_cstring = std::ffi::CString::new(v)
                    .map_err(|_| TtsProviderError::Provider("Invalid voice string".to_string()))?;
                voice_cstring.as_ptr()
            }
            None => ptr::null(),
        };

        let result = (callbacks.stream_start)(
            callbacks.context,
            text_cstring.as_ptr(),
            format.into(),
            voice_ptr,
        );

        match result {
            DtermMediaError::Ok => {
                drop(guard);
                self.streaming = true;
                Ok(())
            }
            DtermMediaError::UnsupportedFormat => Err(TtsProviderError::UnsupportedFormat(format)),
            DtermMediaError::VoiceNotFound => Err(TtsProviderError::VoiceNotFound(
                voice.unwrap_or("").to_string(),
            )),
            _ => Err(TtsProviderError::Provider(format!(
                "Stream start failed: {:?}",
                result
            ))),
        }
    }

    fn read_chunk(&mut self, buffer: &mut [u8]) -> Result<Option<usize>, TtsProviderError> {
        if !self.streaming {
            return Err(TtsProviderError::NotInitialized);
        }

        let guard = self.callbacks.lock().unwrap();
        let callbacks = guard.as_ref().ok_or(TtsProviderError::NotInitialized)?;

        let result = (callbacks.stream_read)(callbacks.context, buffer.as_mut_ptr(), buffer.len());

        if result < 0 {
            drop(guard);
            self.streaming = false;
            return Err(TtsProviderError::SynthesisFailed(
                "Stream read error".to_string(),
            ));
        }

        if result == 0 {
            // Stream complete
            drop(guard);
            self.streaming = false;
            return Ok(None);
        }

        // result is positive, fits in usize
        #[allow(clippy::cast_sign_loss)]
        Ok(Some(result as usize))
    }

    fn stop_stream(&mut self) {
        if !self.streaming {
            return;
        }

        let guard = self.callbacks.lock().unwrap();
        if let Some(callbacks) = guard.as_ref() {
            (callbacks.stream_stop)(callbacks.context);
        }

        drop(guard);
        self.streaming = false;
    }

    fn estimate_duration(&self, text: &str) -> Duration {
        // Rough estimate: ~150 words per minute
        let word_count = text.split_whitespace().count();
        #[allow(clippy::cast_precision_loss)]
        let seconds = (word_count as f64) / 2.5;
        Duration::from_secs_f64(seconds.max(0.1))
    }

    fn supported_formats(&self) -> &[AudioFormat] {
        static DEFAULT_FORMATS: &[AudioFormat] = &[AudioFormat::Pcm16k, AudioFormat::Pcm44k];
        DEFAULT_FORMATS
    }

    fn available_voices(&self) -> &[VoiceInfo] {
        static DEFAULT_VOICES: &[VoiceInfo] = &[];
        DEFAULT_VOICES
    }

    fn set_rate(&mut self, rate: f32) {
        if let Ok(mut guard) = self.callbacks.lock() {
            if let Some(ref mut callbacks) = *guard {
                callbacks.rate = rate.clamp(0.5, 2.0);
            }
        }
    }

    fn set_pitch(&mut self, pitch: f32) {
        if let Ok(mut guard) = self.callbacks.lock() {
            if let Some(ref mut callbacks) = *guard {
                callbacks.pitch = pitch.clamp(0.5, 2.0);
            }
        }
    }

    fn set_volume(&mut self, volume: f32) {
        if let Ok(mut guard) = self.callbacks.lock() {
            if let Some(ref mut callbacks) = *guard {
                callbacks.volume = volume.clamp(0.0, 1.0);
            }
        }
    }
}

// =============================================================================
// Global Provider Storage
// =============================================================================

use std::sync::OnceLock;

static GLOBAL_STT_PROVIDER: OnceLock<FfiSttProvider> = OnceLock::new();
static GLOBAL_TTS_PROVIDER: OnceLock<FfiTtsProvider> = OnceLock::new();

/// Get or create the global FFI STT provider.
pub fn global_stt_provider() -> &'static FfiSttProvider {
    GLOBAL_STT_PROVIDER.get_or_init(FfiSttProvider::new)
}

/// Get or create the global FFI TTS provider.
pub fn global_tts_provider() -> &'static FfiTtsProvider {
    GLOBAL_TTS_PROVIDER.get_or_init(FfiTtsProvider::new)
}

// =============================================================================
// C FFI Functions
// =============================================================================

/// Register STT callbacks from the platform layer.
///
/// This must be called before any STT operations can succeed.
///
/// # Safety
///
/// - `context` must remain valid for the duration of all STT operations
/// - All callback function pointers must be valid
/// - `formats` must point to `formats_len` valid `DtermAudioFormat` values
/// - `languages` must point to `languages_len` valid null-terminated C strings
#[no_mangle]
pub unsafe extern "C" fn dterm_media_register_stt_callbacks(
    context: *mut c_void,
    start: DtermSttStartCallback,
    feed: DtermSttFeedCallback,
    partial: DtermSttPartialCallback,
    stop: DtermSttStopCallback,
    cancel: DtermSttCancelCallback,
    vad: DtermSttVadCallback,
    formats: *const DtermAudioFormat,
    formats_len: usize,
    languages: *const *const c_char,
    languages_len: usize,
) {
    // SAFETY: Caller guarantees formats points to formats_len valid values
    let formats_slice = if formats.is_null() || formats_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(formats, formats_len) }
    };
    let formats_vec: Vec<AudioFormat> = formats_slice.iter().map(|f| (*f).into()).collect();

    // SAFETY: Caller guarantees languages points to languages_len valid pointers
    let languages_vec: Vec<&str> = if languages.is_null() || languages_len == 0 {
        vec![]
    } else {
        let lang_ptrs = unsafe { std::slice::from_raw_parts(languages, languages_len) };
        lang_ptrs
            .iter()
            .filter_map(|&ptr| {
                if ptr.is_null() {
                    None
                } else {
                    // SAFETY: Caller guarantees ptr is a valid null-terminated string
                    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
                }
            })
            .collect()
    };

    let vad_opt = if vad as usize == 0 { None } else { Some(vad) };

    // SAFETY: Caller guarantees context and callbacks remain valid
    unsafe {
        global_stt_provider().register_callbacks(
            context,
            start,
            feed,
            partial,
            stop,
            cancel,
            vad_opt,
            &formats_vec,
            &languages_vec,
        );
    }
}

/// Register TTS callbacks from the platform layer.
///
/// This must be called before any TTS operations can succeed.
///
/// # Safety
///
/// - `context` must remain valid for the duration of all TTS operations
/// - All callback function pointers must be valid
/// - `formats` must point to `formats_len` valid `DtermAudioFormat` values
#[no_mangle]
pub unsafe extern "C" fn dterm_media_register_tts_callbacks(
    context: *mut c_void,
    synthesize: DtermTtsSynthesizeCallback,
    stream_start: DtermTtsStreamStartCallback,
    stream_read: DtermTtsStreamReadCallback,
    stream_stop: DtermTtsStreamStopCallback,
    formats: *const DtermAudioFormat,
    formats_len: usize,
) {
    // SAFETY: Caller guarantees formats points to formats_len valid values
    let formats_slice = if formats.is_null() || formats_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(formats, formats_len) }
    };
    let formats_vec: Vec<AudioFormat> = formats_slice.iter().map(|f| (*f).into()).collect();

    // No voice info via this simple API - platform can set up voices separately
    let voices: Vec<VoiceInfo> = vec![];

    // SAFETY: Caller guarantees context and callbacks remain valid
    unsafe {
        global_tts_provider().register_callbacks(
            context,
            synthesize,
            stream_start,
            stream_read,
            stream_stop,
            &formats_vec,
            &voices,
        );
    }
}

/// Check if STT callbacks are registered.
#[no_mangle]
pub extern "C" fn dterm_media_stt_initialized() -> bool {
    global_stt_provider().is_initialized()
}

/// Check if TTS callbacks are registered.
#[no_mangle]
pub extern "C" fn dterm_media_tts_initialized() -> bool {
    global_tts_provider().is_initialized()
}

/// Free an STT result returned by callbacks.
///
/// # Safety
///
/// - `result` must be a valid pointer previously returned by an STT callback,
///   or null (in which case this is a no-op)
/// - `result` must not have been freed previously
#[no_mangle]
pub unsafe extern "C" fn dterm_media_free_stt_result(result: *mut DtermSttResult) {
    if result.is_null() {
        return;
    }

    // SAFETY: Caller guarantees result is a valid pointer from dterm_media_alloc_stt_result
    let boxed = unsafe { Box::from_raw(result) };
    if !boxed.text.is_null() {
        // SAFETY: text was created by CString::into_raw in dterm_media_alloc_stt_result
        drop(unsafe { std::ffi::CString::from_raw(boxed.text) });
    }
    // Box is dropped automatically
}

/// Allocate an STT result for returning from callbacks.
///
/// Platform layer should call this to create properly allocated results.
///
/// # Safety
///
/// - `text` must be a valid null-terminated UTF-8 string, or null
/// - The returned pointer must be freed with `dterm_media_free_stt_result`
#[no_mangle]
pub unsafe extern "C" fn dterm_media_alloc_stt_result(
    text: *const c_char,
    confidence: u8,
    is_final: bool,
) -> *mut DtermSttResult {
    let text_owned = if text.is_null() {
        ptr::null_mut()
    } else {
        // SAFETY: Caller guarantees text is a valid null-terminated string
        match unsafe { CStr::from_ptr(text) }.to_str() {
            Ok(s) => match std::ffi::CString::new(s) {
                Ok(cs) => cs.into_raw(),
                Err(_) => ptr::null_mut(),
            },
            Err(_) => ptr::null_mut(),
        }
    };

    Box::into_raw(Box::new(DtermSttResult {
        text: text_owned,
        confidence,
        is_final,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_format_conversion() {
        assert_eq!(
            AudioFormat::from(DtermAudioFormat::Pcm16k),
            AudioFormat::Pcm16k
        );
        assert_eq!(
            AudioFormat::from(DtermAudioFormat::Pcm44k),
            AudioFormat::Pcm44k
        );
        assert_eq!(AudioFormat::from(DtermAudioFormat::Opus), AudioFormat::Opus);
        assert_eq!(AudioFormat::from(DtermAudioFormat::Aac), AudioFormat::Aac);

        assert_eq!(
            DtermAudioFormat::from(AudioFormat::Pcm16k),
            DtermAudioFormat::Pcm16k
        );
        assert_eq!(
            DtermAudioFormat::from(AudioFormat::Pcm44k),
            DtermAudioFormat::Pcm44k
        );
        assert_eq!(
            DtermAudioFormat::from(AudioFormat::Opus),
            DtermAudioFormat::Opus
        );
        assert_eq!(
            DtermAudioFormat::from(AudioFormat::Aac),
            DtermAudioFormat::Aac
        );
    }

    #[test]
    fn test_stt_provider_uninitialized() {
        let mut provider = FfiSttProvider::new();
        assert!(!provider.is_initialized());
        assert!(provider.start(AudioFormat::Pcm16k, None).is_err());
        assert!(provider.feed_audio(&[1, 2, 3]).is_err());
        assert!(provider.get_partial().is_none());
        assert!(provider.stop().is_err());
    }

    #[test]
    fn test_tts_provider_uninitialized() {
        let mut provider = FfiTtsProvider::new();
        assert!(!provider.is_initialized());
        assert!(provider
            .synthesize("test", AudioFormat::Pcm16k, None)
            .is_err());
        assert!(provider
            .start_stream("test", AudioFormat::Pcm16k, None)
            .is_err());
    }

    #[test]
    fn test_tts_rate_pitch_volume() {
        let provider = FfiTtsProvider::new();

        // These should not panic even without initialization
        // (they just do nothing since callbacks aren't set)
        let mut provider = provider;
        provider.set_rate(1.5);
        provider.set_pitch(0.8);
        provider.set_volume(0.5);
    }
}
