//! System clipboard integration via OSC 52.
//!
//! This module provides clipboard access using the OSC 52 escape sequence,
//! which is supported by most modern terminals including:
//! - iTerm2
//! - Alacritty
//! - Kitty
//! - WezTerm
//! - Terminal.app (macOS)
//! - Windows Terminal
//!
//! # Example
//!
//! ```rust,ignore
//! use inky::clipboard::Clipboard;
//!
//! // Copy text to clipboard
//! Clipboard::copy("Hello, World!").expect("clipboard copy failed");
//!
//! // Request paste (requires terminal support)
//! if let Ok(text) = Clipboard::paste() {
//!     println!("Pasted: {}", text);
//! }
//! ```
//!
//! # Security Considerations
//!
//! Some terminals restrict OSC 52 for security reasons:
//! - xterm requires explicit configuration (`allowWindowOps`)
//! - Some terminals only allow copying, not pasting
//! - SSH sessions may have different permissions
//!
//! Always handle errors gracefully as clipboard operations may fail.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use std::io;

/// System clipboard interface via OSC 52.
///
/// OSC 52 is a terminal escape sequence that allows programs to copy
/// text to and from the system clipboard without requiring platform-specific
/// clipboard libraries.
///
/// # Supported Terminals
///
/// | Terminal | Copy | Paste |
/// |----------|------|-------|
/// | iTerm2 | Yes | Yes |
/// | Alacritty | Yes | Yes |
/// | Kitty | Yes | Yes |
/// | WezTerm | Yes | Yes |
/// | Terminal.app | Yes | No |
/// | Windows Terminal | Yes | Yes |
/// | xterm | Config | Config |
///
/// # Example
///
/// ```rust,ignore
/// use inky::clipboard::Clipboard;
///
/// // Copy text
/// Clipboard::copy("Hello")?;
///
/// // Copy from selection (primary)
/// Clipboard::copy_to_selection("Selected text", ClipboardSelection::Primary)?;
/// ```
pub struct Clipboard;

/// Clipboard selection target.
///
/// X11 has multiple selection buffers. On other platforms, only `Clipboard`
/// is typically used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClipboardSelection {
    /// The system clipboard (Ctrl+C/Ctrl+V).
    #[default]
    Clipboard,
    /// Primary selection (X11 middle-click paste).
    Primary,
    /// Secondary selection (rarely used).
    Secondary,
    /// Both clipboard and primary selection.
    Both,
}

impl ClipboardSelection {
    /// Get the OSC 52 selection character(s).
    fn to_osc52_char(self) -> &'static str {
        match self {
            Self::Clipboard => "c",
            Self::Primary => "p",
            Self::Secondary => "s",
            Self::Both => "cp",
        }
    }
}

impl Clipboard {
    /// Copy text to the system clipboard.
    ///
    /// Uses OSC 52 escape sequence to set clipboard content.
    /// This works in most modern terminals.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use inky::clipboard::Clipboard;
    ///
    /// Clipboard::copy("Hello, World!")?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if writing to stdout fails.
    pub fn copy(text: &str) -> io::Result<()> {
        Self::copy_to_selection(text, ClipboardSelection::Clipboard)
    }

    /// Copy text to a specific selection buffer.
    ///
    /// Useful for X11 systems where you may want to copy to the primary
    /// selection (middle-click paste) instead of or in addition to the
    /// system clipboard.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use inky::clipboard::{Clipboard, ClipboardSelection};
    ///
    /// // Copy to both clipboard and primary selection
    /// Clipboard::copy_to_selection("Hello", ClipboardSelection::Both)?;
    /// ```
    pub fn copy_to_selection(text: &str, selection: ClipboardSelection) -> io::Result<()> {
        use std::io::Write;
        let encoded = BASE64.encode(text);
        let mut stdout = io::stdout().lock();
        write!(
            stdout,
            "\x1b]52;{};{}\x07",
            selection.to_osc52_char(),
            encoded
        )?;
        stdout.flush()
    }

    /// Clear the clipboard.
    ///
    /// Sets the clipboard content to empty.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use inky::clipboard::Clipboard;
    ///
    /// Clipboard::clear()?;
    /// ```
    pub fn clear() -> io::Result<()> {
        Self::clear_selection(ClipboardSelection::Clipboard)
    }

    /// Clear a specific selection buffer.
    pub fn clear_selection(selection: ClipboardSelection) -> io::Result<()> {
        use std::io::Write;
        let mut stdout = io::stdout().lock();
        write!(stdout, "\x1b]52;{};\x07", selection.to_osc52_char())?;
        stdout.flush()
    }

    /// Request clipboard contents (query).
    ///
    /// Sends an OSC 52 query to request clipboard contents. The response
    /// will come as terminal input that needs to be parsed.
    ///
    /// **Note**: This is an asynchronous operation. The response arrives
    /// as terminal input and must be parsed from the event stream.
    /// Use [`Clipboard::parse_paste_response`] to extract the content from the
    /// terminal's response.
    ///
    /// # Support
    ///
    /// Not all terminals support clipboard queries:
    /// - iTerm2: Requires "Allow clipboard access" in settings
    /// - Kitty: Supported
    /// - Alacritty: Supported with configuration
    /// - Terminal.app: Not supported
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use inky::clipboard::Clipboard;
    ///
    /// // Send query request
    /// Clipboard::request_paste()?;
    ///
    /// // Response arrives as terminal input...
    /// // Parse it with Clipboard::parse_paste_response()
    /// ```
    pub fn request_paste() -> io::Result<()> {
        Self::request_paste_from(ClipboardSelection::Clipboard)
    }

    /// Request contents from a specific selection buffer.
    pub fn request_paste_from(selection: ClipboardSelection) -> io::Result<()> {
        use std::io::Write;
        let mut stdout = io::stdout().lock();
        write!(stdout, "\x1b]52;{};?\x07", selection.to_osc52_char())?;
        stdout.flush()
    }

    /// Parse an OSC 52 paste response.
    ///
    /// When the terminal responds to a paste request, it sends an OSC 52
    /// sequence containing base64-encoded clipboard content. This function
    /// parses that response.
    ///
    /// # Arguments
    ///
    /// * `response` - The raw OSC 52 response string (e.g., `"\x1b]52;c;SGVsbG8=\x07"`)
    ///
    /// # Returns
    ///
    /// Returns `Ok(String)` with the decoded clipboard content, or an error
    /// if the response is malformed.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::clipboard::Clipboard;
    ///
    /// let response = "\x1b]52;c;SGVsbG8=\x07";
    /// let text = Clipboard::parse_paste_response(response).unwrap();
    /// assert_eq!(text, "Hello");
    /// ```
    pub fn parse_paste_response(response: &str) -> io::Result<String> {
        // OSC 52 response format: ESC ] 52 ; <selection> ; <base64> BEL
        // Or: ESC ] 52 ; <selection> ; <base64> ESC \
        let stripped = response
            .strip_prefix("\x1b]52;")
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing OSC 52 prefix"))?;
        let stripped = stripped
            .strip_suffix("\x07")
            .or_else(|| stripped.strip_suffix("\x1b\\"))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "missing OSC 52 terminator")
            })?;

        // Find the base64 part (after the selection character and semicolon)
        if let Some((_selection, base64_data)) = stripped.split_once(';') {
            match BASE64.decode(base64_data) {
                Ok(bytes) => String::from_utf8(bytes).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("clipboard data is not valid UTF-8: {}", e),
                    )
                }),
                Err(e) => Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid base64 in clipboard response: {}", e),
                )),
            }
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "malformed OSC 52 response",
            ))
        }
    }

    /// Check if a string looks like an OSC 52 response.
    ///
    /// Returns `true` if the string appears to be an OSC 52 clipboard response
    /// that can be parsed with [`Clipboard::parse_paste_response`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::clipboard::Clipboard;
    ///
    /// assert!(Clipboard::is_paste_response("\x1b]52;c;SGVsbG8=\x07"));
    /// assert!(!Clipboard::is_paste_response("Hello"));
    /// ```
    pub fn is_paste_response(s: &str) -> bool {
        s.starts_with("\x1b]52;") && (s.ends_with("\x07") || s.ends_with("\x1b\\"))
    }

    /// Create an OSC 52 copy sequence without sending it.
    ///
    /// This is useful when you need to queue clipboard operations or
    /// send them through a custom writer.
    ///
    /// # Example
    ///
    /// ```rust
    /// use inky::clipboard::Clipboard;
    ///
    /// let seq = Clipboard::copy_sequence("Hello");
    /// assert!(seq.starts_with("\x1b]52;c;"));
    /// ```
    pub fn copy_sequence(text: &str) -> String {
        Self::copy_sequence_to(text, ClipboardSelection::Clipboard)
    }

    /// Create an OSC 52 copy sequence for a specific selection.
    pub fn copy_sequence_to(text: &str, selection: ClipboardSelection) -> String {
        let encoded = BASE64.encode(text);
        format!("\x1b]52;{};{}\x07", selection.to_osc52_char(), encoded)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_copy_sequence() {
        let seq = Clipboard::copy_sequence("Hello");
        assert!(seq.starts_with("\x1b]52;c;"));
        assert!(seq.ends_with("\x07"));
        // "Hello" in base64 is "SGVsbG8="
        assert!(seq.contains("SGVsbG8="));
    }

    #[test]
    fn test_copy_sequence_to_primary() {
        let seq = Clipboard::copy_sequence_to("Test", ClipboardSelection::Primary);
        assert!(seq.starts_with("\x1b]52;p;"));
    }

    #[test]
    fn test_copy_sequence_to_both() {
        let seq = Clipboard::copy_sequence_to("Test", ClipboardSelection::Both);
        assert!(seq.starts_with("\x1b]52;cp;"));
    }

    #[test]
    fn test_parse_paste_response() {
        // "Hello" encoded in base64 is "SGVsbG8="
        let response = "\x1b]52;c;SGVsbG8=\x07";
        let result = Clipboard::parse_paste_response(response).unwrap();
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_parse_paste_response_st_terminator() {
        // Some terminals use ESC \ instead of BEL
        let response = "\x1b]52;c;SGVsbG8=\x1b\\";
        let result = Clipboard::parse_paste_response(response).unwrap();
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_parse_paste_response_primary() {
        let response = "\x1b]52;p;V29ybGQ=\x07"; // "World"
        let result = Clipboard::parse_paste_response(response).unwrap();
        assert_eq!(result, "World");
    }

    #[test]
    fn test_is_paste_response() {
        assert!(Clipboard::is_paste_response("\x1b]52;c;SGVsbG8=\x07"));
        assert!(Clipboard::is_paste_response("\x1b]52;p;SGVsbG8=\x1b\\"));
        assert!(!Clipboard::is_paste_response("Hello"));
        assert!(!Clipboard::is_paste_response("\x1b[52;c;SGVsbG8=\x07"));
    }

    #[test]
    fn test_parse_invalid_base64() {
        let response = "\x1b]52;c;!!!invalid!!!\x07";
        assert!(Clipboard::parse_paste_response(response).is_err());
    }

    #[test]
    fn test_parse_malformed_response() {
        let response = "\x1b]52;\x07"; // Missing selection and data
        assert!(Clipboard::parse_paste_response(response).is_err());
    }

    #[test]
    fn test_parse_missing_prefix() {
        let response = "52;c;SGVsbG8=\x07";
        assert!(Clipboard::parse_paste_response(response).is_err());
    }

    #[test]
    fn test_parse_missing_terminator() {
        let response = "\x1b]52;c;SGVsbG8=";
        assert!(Clipboard::parse_paste_response(response).is_err());
    }

    #[test]
    fn test_selection_to_osc52_char() {
        assert_eq!(ClipboardSelection::Clipboard.to_osc52_char(), "c");
        assert_eq!(ClipboardSelection::Primary.to_osc52_char(), "p");
        assert_eq!(ClipboardSelection::Secondary.to_osc52_char(), "s");
        assert_eq!(ClipboardSelection::Both.to_osc52_char(), "cp");
    }

    #[test]
    fn test_unicode_clipboard() {
        let text = "Hello, 世界! 🦀";
        let seq = Clipboard::copy_sequence(text);

        // Extract base64 portion
        let b64 = seq
            .trim_start_matches("\x1b]52;c;")
            .trim_end_matches("\x07");

        // Decode and verify
        let decoded = BASE64.decode(b64).unwrap();
        let result = String::from_utf8(decoded).unwrap();
        assert_eq!(result, text);
    }

    #[test]
    fn test_empty_clipboard() {
        let seq = Clipboard::copy_sequence("");
        // Empty string encodes as empty base64 (no data)
        assert_eq!(seq, "\x1b]52;c;\x07");
    }
}
