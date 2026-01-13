//! Platform-specific voice I/O provider implementations.
//!
//! This module contains platform abstraction traits and implementations
//! for different operating systems.
//!
//! ## Platform Support
//!
//! | Platform | STT Provider | TTS Provider | Module |
//! |----------|--------------|--------------|--------|
//! | macOS | Speech framework | AVSpeechSynthesizer | `macos` |
//! | iOS/iPadOS | Speech framework | AVSpeechSynthesizer | `ios` |
//! | Windows | Windows.Media.SpeechRecognition | Windows.Media.SpeechSynthesis | `windows` |
//! | Linux | Vosk, Whisper.cpp | Festival, espeak | `linux` |
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dterm_core::media::platform::{create_stt_provider, create_tts_provider};
//!
//! // Get platform-appropriate providers
//! let stt = create_stt_provider();
//! let tts = create_tts_provider();
//!
//! // Check capabilities
//! let caps = get_platform_capabilities();
//! if caps.has_stt() {
//!     // STT available
//! }
//! ```

mod traits;

#[cfg(target_os = "macos")]
mod macos;

// Apple platform native bindings (shared between macOS and iOS)
// The actual bindings are in macos_native.rs and macos_stt_native.rs but work on both platforms
#[cfg(all(
    any(target_os = "macos", target_os = "ios"),
    any(feature = "macos-speech", feature = "ios-speech")
))]
pub mod macos_native;

#[cfg(all(
    any(target_os = "macos", target_os = "ios"),
    any(feature = "macos-speech", feature = "ios-speech")
))]
pub mod macos_stt_native;

#[cfg(all(
    any(target_os = "macos", target_os = "ios"),
    any(feature = "macos-speech", feature = "ios-speech")
))]
pub mod macos_audio_input;

// Re-export Apple bindings under iOS-specific module names for consistency
#[cfg(all(target_os = "ios", feature = "ios-speech"))]
pub mod ios_native {
    //! iOS native speech bindings (re-exports from shared Apple implementation).
    pub use super::macos_native::*;
}

#[cfg(all(target_os = "ios", feature = "ios-speech"))]
pub mod ios_stt_native {
    //! iOS native STT bindings (re-exports from shared Apple implementation).
    pub use super::macos_stt_native::*;
}

#[cfg(all(target_os = "ios", feature = "ios-speech"))]
pub mod ios_audio_input {
    //! iOS native audio input bindings (re-exports from shared Apple implementation).
    pub use super::macos_audio_input::*;
}

#[cfg(all(target_os = "windows", feature = "windows-speech"))]
pub mod windows_native;

#[cfg(all(target_os = "windows", feature = "windows-speech"))]
pub mod windows_stt_native;

#[cfg(all(target_os = "linux", feature = "linux-speech"))]
pub mod linux_native;

#[cfg(all(target_os = "linux", feature = "linux-speech"))]
pub mod linux_stt_native;

// Audio input modules
#[cfg(all(target_os = "windows", feature = "windows-speech"))]
pub mod windows_audio_input;

#[cfg(all(target_os = "linux", feature = "linux-speech"))]
pub mod linux_audio_input;

#[cfg(target_os = "ios")]
mod ios;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
mod linux;

// Re-export traits from the traits module
pub use traits::{
    AudioDataCallback, AudioInputDevice, AudioInputError, AudioInputProvider, PlatformCapabilities,
    SttProvider, SttProviderError, TtsProvider, TtsProviderError, VoiceGender, VoiceInfo,
    VoiceQuality,
};

// Re-export null providers
pub use traits::{NullAudioInputProvider, NullSttProvider, NullTtsProvider};

// Re-export platform-specific providers
#[cfg(target_os = "macos")]
pub use macos::{MacOsSttProvider, MacOsTtsProvider};

#[cfg(all(target_os = "macos", feature = "macos-speech"))]
pub use macos_audio_input::MacOsAudioInputProvider;

#[cfg(target_os = "ios")]
pub use ios::{IosSttProvider, IosTtsProvider};

#[cfg(all(target_os = "ios", feature = "ios-speech"))]
pub use macos_audio_input::MacOsAudioInputProvider as IosAudioInputProvider;

#[cfg(target_os = "windows")]
pub use windows::{WindowsSttProvider, WindowsTtsProvider};

#[cfg(target_os = "linux")]
pub use linux::{LinuxSttProvider, LinuxTtsProvider};

// Audio input provider exports
#[cfg(all(target_os = "windows", feature = "windows-speech"))]
pub use windows_audio_input::WindowsAudioInputProvider;

#[cfg(all(target_os = "linux", feature = "linux-speech"))]
pub use linux_audio_input::LinuxAudioInputProvider;

/// Create a platform-appropriate STT provider.
///
/// Returns the best available STT provider for the current platform,
/// or a null provider if no native support is available.
pub fn create_stt_provider() -> Box<dyn SttProvider> {
    #[cfg(target_os = "macos")]
    {
        Box::new(MacOsSttProvider::new())
    }

    #[cfg(target_os = "ios")]
    {
        Box::new(IosSttProvider::new())
    }

    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsSttProvider::new())
    }

    #[cfg(target_os = "linux")]
    {
        Box::new(LinuxSttProvider::new())
    }

    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        target_os = "linux"
    )))]
    {
        Box::new(NullSttProvider)
    }
}

/// Create a platform-appropriate audio input provider.
///
/// Returns the best available audio input provider for the current platform,
/// or a null provider if no native support is available.
pub fn create_audio_input_provider() -> Box<dyn AudioInputProvider> {
    #[cfg(all(target_os = "macos", feature = "macos-speech"))]
    {
        Box::new(MacOsAudioInputProvider::new())
    }

    #[cfg(all(target_os = "ios", feature = "ios-speech"))]
    {
        Box::new(IosAudioInputProvider::new())
    }

    #[cfg(all(target_os = "windows", feature = "windows-speech"))]
    {
        Box::new(WindowsAudioInputProvider::new())
    }

    #[cfg(all(target_os = "linux", feature = "linux-speech"))]
    {
        Box::new(LinuxAudioInputProvider::new())
    }

    #[cfg(not(any(
        all(target_os = "macos", feature = "macos-speech"),
        all(target_os = "ios", feature = "ios-speech"),
        all(target_os = "windows", feature = "windows-speech"),
        all(target_os = "linux", feature = "linux-speech"),
    )))]
    {
        Box::new(NullAudioInputProvider)
    }
}

/// Create a platform-appropriate TTS provider.
///
/// Returns the best available TTS provider for the current platform,
/// or a null provider if no native support is available.
pub fn create_tts_provider() -> Box<dyn TtsProvider> {
    #[cfg(target_os = "macos")]
    {
        Box::new(MacOsTtsProvider::new())
    }

    #[cfg(target_os = "ios")]
    {
        Box::new(IosTtsProvider::new())
    }

    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsTtsProvider::new())
    }

    #[cfg(target_os = "linux")]
    {
        Box::new(LinuxTtsProvider::new())
    }

    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        target_os = "linux"
    )))]
    {
        Box::new(NullTtsProvider)
    }
}

/// Get the platform capabilities for the current platform.
pub fn get_platform_capabilities() -> PlatformCapabilities {
    #[cfg(target_os = "macos")]
    {
        macos::get_capabilities()
    }

    #[cfg(target_os = "ios")]
    {
        ios::get_capabilities()
    }

    #[cfg(target_os = "windows")]
    {
        windows::get_capabilities()
    }

    #[cfg(target_os = "linux")]
    {
        linux::get_capabilities()
    }

    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        target_os = "linux"
    )))]
    {
        PlatformCapabilities::none()
    }
}

/// Check if the current platform has any voice I/O support.
pub fn has_voice_support() -> bool {
    let caps = get_platform_capabilities();
    caps.has_stt() || caps.has_tts()
}

/// Get human-readable platform name.
pub fn platform_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "macOS"
    }

    #[cfg(target_os = "ios")]
    {
        "iOS"
    }

    #[cfg(target_os = "windows")]
    {
        "Windows"
    }

    #[cfg(target_os = "linux")]
    {
        "Linux"
    }

    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        target_os = "linux"
    )))]
    {
        "Unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::AudioFormat;

    #[test]
    fn test_platform_name() {
        let name = platform_name();
        assert!(!name.is_empty());
        #[cfg(target_os = "macos")]
        assert_eq!(name, "macOS");
        #[cfg(target_os = "ios")]
        assert_eq!(name, "iOS");
        #[cfg(target_os = "windows")]
        assert_eq!(name, "Windows");
        #[cfg(target_os = "linux")]
        assert_eq!(name, "Linux");
    }

    #[test]
    fn test_create_providers() {
        let _stt = create_stt_provider();
        let _tts = create_tts_provider();
    }

    #[test]
    fn test_get_capabilities() {
        let caps = get_platform_capabilities();
        // Capabilities should be valid
        assert!(caps.stt_formats.len() <= 10);
        assert!(caps.tts_formats.len() <= 10);
    }

    #[test]
    fn test_null_providers() {
        let mut stt = NullSttProvider;
        let mut tts = NullTtsProvider;

        // Null STT always returns errors
        assert!(stt.start(AudioFormat::Pcm16k, None).is_err());
        assert!(stt.feed_audio(&[]).is_err());
        assert!(stt.get_partial().is_none());
        assert!(stt.stop().is_err());
        assert!(stt.is_voice_active().is_none());
        assert!(stt.supported_formats().is_empty());
        assert!(stt.supported_languages().is_empty());

        // Null TTS always returns errors
        assert!(tts.synthesize("test", AudioFormat::Pcm16k, None).is_err());
        assert!(tts.start_stream("test", AudioFormat::Pcm16k, None).is_err());
        let mut buf = [0u8; 10];
        assert!(tts.read_chunk(&mut buf).is_err());
        assert_eq!(tts.estimate_duration("test"), std::time::Duration::ZERO);
        assert!(tts.supported_formats().is_empty());
        assert!(tts.available_voices().is_empty());
    }
}
