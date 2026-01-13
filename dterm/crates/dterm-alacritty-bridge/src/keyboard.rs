//! Keyboard input encoding for terminal emulators.
//!
//! This module provides key-to-escape-sequence encoding, supporting both legacy
//! terminal encoding and the Kitty keyboard protocol.
//!
//! # Overview
//!
//! Terminal applications communicate with the host through escape sequences.
//! When a user presses a key, the terminal must convert that key press into
//! the appropriate byte sequence that the running application expects.
//!
//! # Encoding Modes
//!
//! 1. **Legacy encoding**: Traditional VT100/xterm-style sequences
//!    - Arrow keys: `ESC [ A/B/C/D` or `ESC O A/B/C/D` (app cursor mode)
//!    - Function keys: `ESC O P/Q/R/S` (F1-F4) or `ESC [ n ~` (F5+)
//!    - Ctrl+letter: Control characters (0x01-0x1A)
//!    - Alt+key: ESC prefix before the key's normal encoding
//!
//! 2. **Kitty keyboard protocol**: Modern unambiguous encoding
//!    - All keys use `CSI unicode ; modifiers u` format
//!    - Supports key repeat/release events
//!    - Reports alternate key codes
//!
//! # Usage
//!
//! ```
//! use dterm_alacritty_bridge::keyboard::{Key, NamedKey, Modifiers, encode_key};
//! use dterm_alacritty_bridge::TermMode;
//!
//! let key = Key::Named(NamedKey::Enter);
//! let modifiers = Modifiers::empty();
//! let mode = TermMode::empty();
//!
//! let bytes = encode_key(&key, modifiers, mode);
//! assert_eq!(bytes, vec![0x0D]); // Carriage return
//! ```

use crate::TermMode;

/// A keyboard key, either a named special key or a character.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    /// A named special key (Enter, Tab, arrows, function keys, etc.)
    Named(NamedKey),
    /// A character key (letters, numbers, symbols)
    Character(char),
}

impl Key {
    /// Create a character key.
    #[must_use]
    pub fn character(c: char) -> Self {
        Key::Character(c)
    }

    /// Create a named key.
    #[must_use]
    pub fn named(key: NamedKey) -> Self {
        Key::Named(key)
    }
}

/// Named special keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum NamedKey {
    // Navigation
    /// Up arrow key
    ArrowUp,
    /// Down arrow key
    ArrowDown,
    /// Left arrow key
    ArrowLeft,
    /// Right arrow key
    ArrowRight,
    /// Home key
    Home,
    /// End key
    End,
    /// Page Up key
    PageUp,
    /// Page Down key
    PageDown,

    // Editing
    /// Backspace key
    Backspace,
    /// Delete key
    Delete,
    /// Insert key
    Insert,
    /// Enter/Return key
    Enter,
    /// Tab key
    Tab,
    /// Escape key
    Escape,
    /// Space key (when sent as named key)
    Space,

    // Function keys
    /// F1 function key
    F1,
    /// F2 function key
    F2,
    /// F3 function key
    F3,
    /// F4 function key
    F4,
    /// F5 function key
    F5,
    /// F6 function key
    F6,
    /// F7 function key
    F7,
    /// F8 function key
    F8,
    /// F9 function key
    F9,
    /// F10 function key
    F10,
    /// F11 function key
    F11,
    /// F12 function key
    F12,
    /// F13 function key
    F13,
    /// F14 function key
    F14,
    /// F15 function key
    F15,
    /// F16 function key
    F16,
    /// F17 function key
    F17,
    /// F18 function key
    F18,
    /// F19 function key
    F19,
    /// F20 function key
    F20,
    /// F21 function key
    F21,
    /// F22 function key
    F22,
    /// F23 function key
    F23,
    /// F24 function key
    F24,

    // Numpad keys (when numlock is off or in app keypad mode)
    /// Numpad 0
    Numpad0,
    /// Numpad 1
    Numpad1,
    /// Numpad 2
    Numpad2,
    /// Numpad 3
    Numpad3,
    /// Numpad 4
    Numpad4,
    /// Numpad 5
    Numpad5,
    /// Numpad 6
    Numpad6,
    /// Numpad 7
    Numpad7,
    /// Numpad 8
    Numpad8,
    /// Numpad 9
    Numpad9,
    /// Numpad decimal point
    NumpadDecimal,
    /// Numpad divide
    NumpadDivide,
    /// Numpad multiply
    NumpadMultiply,
    /// Numpad subtract
    NumpadSubtract,
    /// Numpad add
    NumpadAdd,
    /// Numpad enter
    NumpadEnter,
}

impl NamedKey {
    /// Get the Kitty keyboard protocol key code for this named key.
    ///
    /// Returns the Unicode code point used in CSI u encoding.
    #[must_use]
    pub fn kitty_code(self) -> u32 {
        // Kitty protocol uses specific code points for functional keys
        // See: https://sw.kovidgoyal.net/kitty/keyboard-protocol/#functional-key-definitions
        match self {
            NamedKey::Escape => 27,
            NamedKey::Enter => 13,
            NamedKey::Tab => 9,
            NamedKey::Backspace => 127,
            NamedKey::Insert => 57348,
            NamedKey::Delete => 57349,
            NamedKey::ArrowLeft => 57350,
            NamedKey::ArrowRight => 57351,
            NamedKey::ArrowUp => 57352,
            NamedKey::ArrowDown => 57353,
            NamedKey::PageUp => 57354,
            NamedKey::PageDown => 57355,
            NamedKey::Home => 57356,
            NamedKey::End => 57357,
            NamedKey::Space => 32,
            NamedKey::F1 => 57364,
            NamedKey::F2 => 57365,
            NamedKey::F3 => 57366,
            NamedKey::F4 => 57367,
            NamedKey::F5 => 57368,
            NamedKey::F6 => 57369,
            NamedKey::F7 => 57370,
            NamedKey::F8 => 57371,
            NamedKey::F9 => 57372,
            NamedKey::F10 => 57373,
            NamedKey::F11 => 57374,
            NamedKey::F12 => 57375,
            NamedKey::F13 => 57376,
            NamedKey::F14 => 57377,
            NamedKey::F15 => 57378,
            NamedKey::F16 => 57379,
            NamedKey::F17 => 57380,
            NamedKey::F18 => 57381,
            NamedKey::F19 => 57382,
            NamedKey::F20 => 57383,
            NamedKey::F21 => 57384,
            NamedKey::F22 => 57385,
            NamedKey::F23 => 57386,
            NamedKey::F24 => 57387,
            // Numpad keys
            NamedKey::Numpad0 => 57399,
            NamedKey::Numpad1 => 57400,
            NamedKey::Numpad2 => 57401,
            NamedKey::Numpad3 => 57402,
            NamedKey::Numpad4 => 57403,
            NamedKey::Numpad5 => 57404,
            NamedKey::Numpad6 => 57405,
            NamedKey::Numpad7 => 57406,
            NamedKey::Numpad8 => 57407,
            NamedKey::Numpad9 => 57408,
            NamedKey::NumpadDecimal => 57409,
            NamedKey::NumpadDivide => 57410,
            NamedKey::NumpadMultiply => 57411,
            NamedKey::NumpadSubtract => 57412,
            NamedKey::NumpadAdd => 57413,
            NamedKey::NumpadEnter => 57414,
        }
    }
}

bitflags::bitflags! {
    /// Keyboard modifier flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct Modifiers: u8 {
        /// Shift modifier
        const SHIFT = 0b0000_0001;
        /// Alt/Option modifier
        const ALT = 0b0000_0010;
        /// Control modifier
        const CTRL = 0b0000_0100;
        /// Super/Cmd/Win modifier
        const SUPER = 0b0000_1000;
    }
}

impl Modifiers {
    /// Get the Kitty keyboard protocol modifier value.
    ///
    /// Kitty uses `modifiers + 1` format (1 = no modifiers).
    #[must_use]
    pub fn kitty_encoded(self) -> u8 {
        self.bits() + 1
    }

    /// Get the legacy xterm modifier value for CSI sequences.
    ///
    /// Used in sequences like `ESC [ 1 ; <mod> A` for arrow keys with modifiers.
    #[must_use]
    pub fn xterm_encoded(self) -> u8 {
        // xterm modifier encoding:
        // 2 = Shift
        // 3 = Alt
        // 4 = Shift+Alt
        // 5 = Control
        // 6 = Shift+Control
        // 7 = Alt+Control
        // 8 = Shift+Alt+Control
        let mut val = 1u8;
        if self.contains(Modifiers::SHIFT) {
            val += 1;
        }
        if self.contains(Modifiers::ALT) {
            val += 2;
        }
        if self.contains(Modifiers::CTRL) {
            val += 4;
        }
        val
    }
}

/// Key event type for Kitty keyboard protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum KeyEventType {
    /// Key press event (default)
    #[default]
    Press,
    /// Key repeat event
    Repeat,
    /// Key release event
    Release,
}

impl KeyEventType {
    /// Get the Kitty protocol event type value.
    #[must_use]
    pub fn kitty_value(self) -> u8 {
        match self {
            KeyEventType::Press => 1,
            KeyEventType::Repeat => 2,
            KeyEventType::Release => 3,
        }
    }
}

/// Encode a key press into terminal escape sequence bytes.
///
/// This is the main entry point for keyboard input encoding. It automatically
/// selects between legacy encoding and Kitty keyboard protocol based on the
/// terminal mode flags.
///
/// # Arguments
///
/// * `key` - The key that was pressed
/// * `modifiers` - Active modifier keys
/// * `mode` - Current terminal mode flags
///
/// # Returns
///
/// A `Vec<u8>` containing the escape sequence bytes to send to the terminal.
///
/// # Example
///
/// ```
/// use dterm_alacritty_bridge::keyboard::{Key, NamedKey, Modifiers, encode_key};
/// use dterm_alacritty_bridge::TermMode;
///
/// // Simple Enter key
/// let bytes = encode_key(&Key::Named(NamedKey::Enter), Modifiers::empty(), TermMode::empty());
/// assert_eq!(bytes, vec![0x0D]);
///
/// // Ctrl+C
/// let bytes = encode_key(&Key::Character('c'), Modifiers::CTRL, TermMode::empty());
/// assert_eq!(bytes, vec![0x03]);
/// ```
#[must_use]
pub fn encode_key(key: &Key, modifiers: Modifiers, mode: TermMode) -> Vec<u8> {
    encode_key_with_event(key, modifiers, mode, KeyEventType::Press)
}

/// Encode a key event with event type information.
///
/// This extends `encode_key` to support key repeat and release events
/// when using the Kitty keyboard protocol.
#[must_use]
pub fn encode_key_with_event(
    key: &Key,
    modifiers: Modifiers,
    mode: TermMode,
    event_type: KeyEventType,
) -> Vec<u8> {
    // Check if Kitty keyboard protocol is active
    if mode.contains(TermMode::DISAMBIGUATE_ESC_CODES) {
        return encode_kitty(key, modifiers, mode, event_type);
    }

    // For release events without Kitty protocol, return nothing
    if event_type == KeyEventType::Release {
        return Vec::new();
    }

    // Use legacy encoding
    encode_legacy(key, modifiers, mode)
}

/// Encode using the Kitty keyboard protocol.
///
/// Format: CSI unicode [; modifiers [: event-type]] u
fn encode_kitty(
    key: &Key,
    modifiers: Modifiers,
    mode: TermMode,
    event_type: KeyEventType,
) -> Vec<u8> {
    let code = match key {
        Key::Named(named) => named.kitty_code(),
        Key::Character(c) => *c as u32,
    };

    let mod_value = modifiers.kitty_encoded();
    let report_events = mode.contains(TermMode::REPORT_EVENT_TYPES);

    let mut buf = Vec::with_capacity(16);
    buf.extend_from_slice(b"\x1b[");

    // Write the key code
    write_u32(&mut buf, code);

    // Write modifiers and event type if needed
    if mod_value > 1 || (report_events && event_type != KeyEventType::Press) {
        buf.push(b';');
        write_u8(&mut buf, mod_value);

        // Include event type if reporting events and not a simple press
        if report_events && event_type != KeyEventType::Press {
            buf.push(b':');
            write_u8(&mut buf, event_type.kitty_value());
        }
    }

    buf.push(b'u');
    buf
}

/// Encode using legacy terminal sequences.
fn encode_legacy(key: &Key, modifiers: Modifiers, mode: TermMode) -> Vec<u8> {
    match key {
        Key::Character(c) => encode_character_legacy(*c, modifiers),
        Key::Named(named) => encode_named_legacy(*named, modifiers, mode),
    }
}

/// Encode a character key using legacy encoding.
fn encode_character_legacy(c: char, modifiers: Modifiers) -> Vec<u8> {
    // Handle Ctrl+letter combinations
    if modifiers.contains(Modifiers::CTRL) {
        if let Some(ctrl_char) = ctrl_character(c) {
            if modifiers.contains(Modifiers::ALT) {
                // Alt+Ctrl+letter: ESC followed by control character
                return vec![0x1b, ctrl_char];
            }
            return vec![ctrl_char];
        }
    }

    // Handle Alt combinations
    if modifiers.contains(Modifiers::ALT) {
        // Alt+key: ESC prefix
        let mut buf = vec![0x1b];
        let lower = if modifiers.contains(Modifiers::SHIFT) {
            c.to_ascii_uppercase()
        } else {
            c.to_ascii_lowercase()
        };
        let mut char_buf = [0u8; 4];
        let encoded = lower.encode_utf8(&mut char_buf);
        buf.extend_from_slice(encoded.as_bytes());
        return buf;
    }

    // Plain character (possibly with shift for uppercase)
    let output = if modifiers.contains(Modifiers::SHIFT) {
        c.to_ascii_uppercase()
    } else {
        c
    };

    let mut buf = [0u8; 4];
    let encoded = output.encode_utf8(&mut buf);
    encoded.as_bytes().to_vec()
}

/// Get the control character for a letter (Ctrl+A = 0x01, Ctrl+Z = 0x1A).
fn ctrl_character(c: char) -> Option<u8> {
    let c_upper = c.to_ascii_uppercase();
    if c_upper.is_ascii_uppercase() {
        Some(c_upper as u8 - b'A' + 1)
    } else {
        match c {
            '@' => Some(0x00),  // Ctrl+@
            '[' => Some(0x1b),  // Ctrl+[ (ESC)
            '\\' => Some(0x1c), // Ctrl+\
            ']' => Some(0x1d),  // Ctrl+]
            '^' => Some(0x1e),  // Ctrl+^
            '_' => Some(0x1f),  // Ctrl+_
            '?' => Some(0x7f),  // Ctrl+? (DEL)
            _ => None,
        }
    }
}

/// Encode a named key using legacy encoding.
fn encode_named_legacy(key: NamedKey, modifiers: Modifiers, mode: TermMode) -> Vec<u8> {
    let app_cursor = mode.contains(TermMode::APP_CURSOR);
    let app_keypad = mode.contains(TermMode::APP_KEYPAD);

    // Check if we need to include modifiers in the sequence
    let has_modifiers = !modifiers.is_empty();

    match key {
        // Simple keys (single byte)
        NamedKey::Enter | NamedKey::NumpadEnter => {
            if modifiers.contains(Modifiers::ALT) {
                vec![0x1b, 0x0d]
            } else {
                vec![0x0d]
            }
        }
        NamedKey::Tab => {
            if modifiers.contains(Modifiers::SHIFT) {
                vec![0x1b, b'[', b'Z'] // Shift+Tab = CSI Z (backtab)
            } else if modifiers.contains(Modifiers::ALT) {
                vec![0x1b, 0x09]
            } else {
                vec![0x09]
            }
        }
        NamedKey::Escape => {
            if modifiers.contains(Modifiers::ALT) {
                vec![0x1b, 0x1b]
            } else {
                vec![0x1b]
            }
        }
        NamedKey::Backspace => {
            if modifiers.contains(Modifiers::CTRL) {
                vec![0x08] // Ctrl+Backspace = BS
            } else if modifiers.contains(Modifiers::ALT) {
                vec![0x1b, 0x7f]
            } else {
                vec![0x7f] // DEL
            }
        }
        NamedKey::Space => {
            if modifiers.contains(Modifiers::CTRL) {
                vec![0x00] // Ctrl+Space = NUL
            } else if modifiers.contains(Modifiers::ALT) {
                vec![0x1b, 0x20]
            } else {
                vec![0x20]
            }
        }

        // Arrow keys
        NamedKey::ArrowUp => encode_arrow(b'A', app_cursor, modifiers, has_modifiers),
        NamedKey::ArrowDown => encode_arrow(b'B', app_cursor, modifiers, has_modifiers),
        NamedKey::ArrowRight => encode_arrow(b'C', app_cursor, modifiers, has_modifiers),
        NamedKey::ArrowLeft => encode_arrow(b'D', app_cursor, modifiers, has_modifiers),

        // Navigation keys
        NamedKey::Home => encode_home_end(b'H', modifiers, has_modifiers),
        NamedKey::End => encode_home_end(b'F', modifiers, has_modifiers),
        NamedKey::PageUp => encode_tilde_key(5, modifiers, has_modifiers),
        NamedKey::PageDown => encode_tilde_key(6, modifiers, has_modifiers),
        NamedKey::Insert => encode_tilde_key(2, modifiers, has_modifiers),
        NamedKey::Delete => encode_tilde_key(3, modifiers, has_modifiers),

        // Function keys F1-F4 (SS3 or CSI with special handling)
        NamedKey::F1 => encode_f1_f4(b'P', modifiers, has_modifiers),
        NamedKey::F2 => encode_f1_f4(b'Q', modifiers, has_modifiers),
        NamedKey::F3 => encode_f1_f4(b'R', modifiers, has_modifiers),
        NamedKey::F4 => encode_f1_f4(b'S', modifiers, has_modifiers),

        // Function keys F5-F12 (tilde sequences)
        NamedKey::F5 => encode_tilde_key(15, modifiers, has_modifiers),
        NamedKey::F6 => encode_tilde_key(17, modifiers, has_modifiers),
        NamedKey::F7 => encode_tilde_key(18, modifiers, has_modifiers),
        NamedKey::F8 => encode_tilde_key(19, modifiers, has_modifiers),
        NamedKey::F9 => encode_tilde_key(20, modifiers, has_modifiers),
        NamedKey::F10 => encode_tilde_key(21, modifiers, has_modifiers),
        NamedKey::F11 => encode_tilde_key(23, modifiers, has_modifiers),
        NamedKey::F12 => encode_tilde_key(24, modifiers, has_modifiers),

        // Extended function keys F13-F24
        NamedKey::F13 => encode_tilde_key(25, modifiers, has_modifiers),
        NamedKey::F14 => encode_tilde_key(26, modifiers, has_modifiers),
        NamedKey::F15 => encode_tilde_key(28, modifiers, has_modifiers),
        NamedKey::F16 => encode_tilde_key(29, modifiers, has_modifiers),
        NamedKey::F17 => encode_tilde_key(31, modifiers, has_modifiers),
        NamedKey::F18 => encode_tilde_key(32, modifiers, has_modifiers),
        NamedKey::F19 => encode_tilde_key(33, modifiers, has_modifiers),
        NamedKey::F20 => encode_tilde_key(34, modifiers, has_modifiers),
        NamedKey::F21 => encode_tilde_key(35, modifiers, has_modifiers),
        NamedKey::F22 => encode_tilde_key(36, modifiers, has_modifiers),
        NamedKey::F23 => encode_tilde_key(37, modifiers, has_modifiers),
        NamedKey::F24 => encode_tilde_key(38, modifiers, has_modifiers),

        // Numpad keys - in app keypad mode use SS3 sequences
        NamedKey::Numpad0 => encode_numpad(b'p', '0', app_keypad, modifiers),
        NamedKey::Numpad1 => encode_numpad(b'q', '1', app_keypad, modifiers),
        NamedKey::Numpad2 => encode_numpad(b'r', '2', app_keypad, modifiers),
        NamedKey::Numpad3 => encode_numpad(b's', '3', app_keypad, modifiers),
        NamedKey::Numpad4 => encode_numpad(b't', '4', app_keypad, modifiers),
        NamedKey::Numpad5 => encode_numpad(b'u', '5', app_keypad, modifiers),
        NamedKey::Numpad6 => encode_numpad(b'v', '6', app_keypad, modifiers),
        NamedKey::Numpad7 => encode_numpad(b'w', '7', app_keypad, modifiers),
        NamedKey::Numpad8 => encode_numpad(b'x', '8', app_keypad, modifiers),
        NamedKey::Numpad9 => encode_numpad(b'y', '9', app_keypad, modifiers),
        NamedKey::NumpadDecimal => encode_numpad(b'n', '.', app_keypad, modifiers),
        NamedKey::NumpadDivide => encode_numpad(b'o', '/', app_keypad, modifiers),
        NamedKey::NumpadMultiply => encode_numpad(b'j', '*', app_keypad, modifiers),
        NamedKey::NumpadSubtract => encode_numpad(b'm', '-', app_keypad, modifiers),
        NamedKey::NumpadAdd => encode_numpad(b'k', '+', app_keypad, modifiers),
    }
}

/// Encode arrow key.
fn encode_arrow(
    suffix: u8,
    app_cursor: bool,
    modifiers: Modifiers,
    has_modifiers: bool,
) -> Vec<u8> {
    if has_modifiers {
        // With modifiers: CSI 1 ; <mod> <suffix>
        let mut buf = vec![0x1b, b'[', b'1', b';'];
        write_u8(&mut buf, modifiers.xterm_encoded());
        buf.push(suffix);
        buf
    } else if app_cursor {
        // Application cursor mode: SS3 <suffix>
        vec![0x1b, b'O', suffix]
    } else {
        // Normal mode: CSI <suffix>
        vec![0x1b, b'[', suffix]
    }
}

/// Encode Home/End keys.
fn encode_home_end(suffix: u8, modifiers: Modifiers, has_modifiers: bool) -> Vec<u8> {
    if has_modifiers {
        // With modifiers: CSI 1 ; <mod> <suffix>
        let mut buf = vec![0x1b, b'[', b'1', b';'];
        write_u8(&mut buf, modifiers.xterm_encoded());
        buf.push(suffix);
        buf
    } else {
        // Normal: CSI <suffix>
        vec![0x1b, b'[', suffix]
    }
}

/// Encode tilde-style keys (PgUp, PgDn, Insert, Delete, F5-F24).
fn encode_tilde_key(num: u8, modifiers: Modifiers, has_modifiers: bool) -> Vec<u8> {
    let mut buf = vec![0x1b, b'['];
    write_u8(&mut buf, num);

    if has_modifiers {
        buf.push(b';');
        write_u8(&mut buf, modifiers.xterm_encoded());
    }

    buf.push(b'~');
    buf
}

/// Encode F1-F4 keys (SS3 style).
fn encode_f1_f4(suffix: u8, modifiers: Modifiers, has_modifiers: bool) -> Vec<u8> {
    if has_modifiers {
        // With modifiers: CSI 1 ; <mod> <suffix>
        let mut buf = vec![0x1b, b'[', b'1', b';'];
        write_u8(&mut buf, modifiers.xterm_encoded());
        buf.push(suffix);
        buf
    } else {
        // Without modifiers: SS3 <suffix>
        vec![0x1b, b'O', suffix]
    }
}

/// Encode numpad key.
fn encode_numpad(
    ss3_suffix: u8,
    char_val: char,
    app_keypad: bool,
    modifiers: Modifiers,
) -> Vec<u8> {
    if modifiers.contains(Modifiers::ALT) {
        let mut buf = vec![0x1b];
        buf.push(char_val as u8);
        buf
    } else if app_keypad {
        vec![0x1b, b'O', ss3_suffix]
    } else {
        vec![char_val as u8]
    }
}

/// Write a u8 as decimal digits to a buffer.
fn write_u8(buf: &mut Vec<u8>, val: u8) {
    if val >= 100 {
        buf.push(b'0' + val / 100);
    }
    if val >= 10 {
        buf.push(b'0' + (val / 10) % 10);
    }
    buf.push(b'0' + val % 10);
}

/// Write a u32 as decimal digits to a buffer.
fn write_u32(buf: &mut Vec<u8>, val: u32) {
    if val == 0 {
        buf.push(b'0');
        return;
    }

    let mut divisor = 1u32;
    while divisor * 10 <= val {
        divisor *= 10;
    }

    while divisor > 0 {
        buf.push(b'0' + (val / divisor % 10) as u8);
        divisor /= 10;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enter_key() {
        let bytes = encode_key(
            &Key::Named(NamedKey::Enter),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x0d]);
    }

    #[test]
    fn test_alt_enter() {
        let bytes = encode_key(
            &Key::Named(NamedKey::Enter),
            Modifiers::ALT,
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, 0x0d]);
    }

    #[test]
    fn test_escape_key() {
        let bytes = encode_key(
            &Key::Named(NamedKey::Escape),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b]);
    }

    #[test]
    fn test_tab_key() {
        let bytes = encode_key(
            &Key::Named(NamedKey::Tab),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x09]);
    }

    #[test]
    fn test_shift_tab() {
        let bytes = encode_key(
            &Key::Named(NamedKey::Tab),
            Modifiers::SHIFT,
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, b'[', b'Z']);
    }

    #[test]
    fn test_backspace() {
        let bytes = encode_key(
            &Key::Named(NamedKey::Backspace),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x7f]);
    }

    #[test]
    fn test_ctrl_c() {
        let bytes = encode_key(&Key::Character('c'), Modifiers::CTRL, TermMode::empty());
        assert_eq!(bytes, vec![0x03]);
    }

    #[test]
    fn test_ctrl_a() {
        let bytes = encode_key(&Key::Character('a'), Modifiers::CTRL, TermMode::empty());
        assert_eq!(bytes, vec![0x01]);
    }

    #[test]
    fn test_ctrl_z() {
        let bytes = encode_key(&Key::Character('z'), Modifiers::CTRL, TermMode::empty());
        assert_eq!(bytes, vec![0x1a]);
    }

    #[test]
    fn test_alt_x() {
        let bytes = encode_key(&Key::Character('x'), Modifiers::ALT, TermMode::empty());
        assert_eq!(bytes, vec![0x1b, b'x']);
    }

    #[test]
    fn test_alt_ctrl_c() {
        let bytes = encode_key(
            &Key::Character('c'),
            Modifiers::ALT | Modifiers::CTRL,
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, 0x03]);
    }

    #[test]
    fn test_arrow_up_normal() {
        let bytes = encode_key(
            &Key::Named(NamedKey::ArrowUp),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, b'[', b'A']);
    }

    #[test]
    fn test_arrow_up_app_cursor() {
        let bytes = encode_key(
            &Key::Named(NamedKey::ArrowUp),
            Modifiers::empty(),
            TermMode::APP_CURSOR,
        );
        assert_eq!(bytes, vec![0x1b, b'O', b'A']);
    }

    #[test]
    fn test_arrow_down_normal() {
        let bytes = encode_key(
            &Key::Named(NamedKey::ArrowDown),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, b'[', b'B']);
    }

    #[test]
    fn test_arrow_left_normal() {
        let bytes = encode_key(
            &Key::Named(NamedKey::ArrowLeft),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, b'[', b'D']);
    }

    #[test]
    fn test_arrow_right_normal() {
        let bytes = encode_key(
            &Key::Named(NamedKey::ArrowRight),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, b'[', b'C']);
    }

    #[test]
    fn test_shift_arrow_up() {
        let bytes = encode_key(
            &Key::Named(NamedKey::ArrowUp),
            Modifiers::SHIFT,
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, b'[', b'1', b';', b'2', b'A']);
    }

    #[test]
    fn test_ctrl_arrow_up() {
        let bytes = encode_key(
            &Key::Named(NamedKey::ArrowUp),
            Modifiers::CTRL,
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, b'[', b'1', b';', b'5', b'A']);
    }

    #[test]
    fn test_home_key() {
        let bytes = encode_key(
            &Key::Named(NamedKey::Home),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, b'[', b'H']);
    }

    #[test]
    fn test_end_key() {
        let bytes = encode_key(
            &Key::Named(NamedKey::End),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, b'[', b'F']);
    }

    #[test]
    fn test_page_up() {
        let bytes = encode_key(
            &Key::Named(NamedKey::PageUp),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, b'[', b'5', b'~']);
    }

    #[test]
    fn test_page_down() {
        let bytes = encode_key(
            &Key::Named(NamedKey::PageDown),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, b'[', b'6', b'~']);
    }

    #[test]
    fn test_insert() {
        let bytes = encode_key(
            &Key::Named(NamedKey::Insert),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, b'[', b'2', b'~']);
    }

    #[test]
    fn test_delete() {
        let bytes = encode_key(
            &Key::Named(NamedKey::Delete),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, b'[', b'3', b'~']);
    }

    #[test]
    fn test_f1() {
        let bytes = encode_key(
            &Key::Named(NamedKey::F1),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, b'O', b'P']);
    }

    #[test]
    fn test_f2() {
        let bytes = encode_key(
            &Key::Named(NamedKey::F2),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, b'O', b'Q']);
    }

    #[test]
    fn test_f3() {
        let bytes = encode_key(
            &Key::Named(NamedKey::F3),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, b'O', b'R']);
    }

    #[test]
    fn test_f4() {
        let bytes = encode_key(
            &Key::Named(NamedKey::F4),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, b'O', b'S']);
    }

    #[test]
    fn test_f5() {
        let bytes = encode_key(
            &Key::Named(NamedKey::F5),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, b'[', b'1', b'5', b'~']);
    }

    #[test]
    fn test_f12() {
        let bytes = encode_key(
            &Key::Named(NamedKey::F12),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, b'[', b'2', b'4', b'~']);
    }

    #[test]
    fn test_shift_f1() {
        let bytes = encode_key(
            &Key::Named(NamedKey::F1),
            Modifiers::SHIFT,
            TermMode::empty(),
        );
        // With modifiers, F1-F4 use CSI format
        assert_eq!(bytes, vec![0x1b, b'[', b'1', b';', b'2', b'P']);
    }

    #[test]
    fn test_shift_f5() {
        let bytes = encode_key(
            &Key::Named(NamedKey::F5),
            Modifiers::SHIFT,
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x1b, b'[', b'1', b'5', b';', b'2', b'~']);
    }

    #[test]
    fn test_plain_character() {
        let bytes = encode_key(&Key::Character('a'), Modifiers::empty(), TermMode::empty());
        assert_eq!(bytes, vec![b'a']);
    }

    #[test]
    fn test_shift_character() {
        let bytes = encode_key(&Key::Character('a'), Modifiers::SHIFT, TermMode::empty());
        assert_eq!(bytes, vec![b'A']);
    }

    #[test]
    fn test_space_key() {
        let bytes = encode_key(
            &Key::Named(NamedKey::Space),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x20]);
    }

    #[test]
    fn test_ctrl_space() {
        let bytes = encode_key(
            &Key::Named(NamedKey::Space),
            Modifiers::CTRL,
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![0x00]);
    }

    // Kitty protocol tests

    #[test]
    fn test_kitty_enter() {
        let bytes = encode_key(
            &Key::Named(NamedKey::Enter),
            Modifiers::empty(),
            TermMode::DISAMBIGUATE_ESC_CODES,
        );
        assert_eq!(bytes, vec![0x1b, b'[', b'1', b'3', b'u']);
    }

    #[test]
    fn test_kitty_character_a() {
        let bytes = encode_key(
            &Key::Character('a'),
            Modifiers::empty(),
            TermMode::DISAMBIGUATE_ESC_CODES,
        );
        assert_eq!(bytes, vec![0x1b, b'[', b'9', b'7', b'u']);
    }

    #[test]
    fn test_kitty_shift_a() {
        let bytes = encode_key(
            &Key::Character('a'),
            Modifiers::SHIFT,
            TermMode::DISAMBIGUATE_ESC_CODES,
        );
        // 'a' = 97, shift modifier = 2
        assert_eq!(bytes, vec![0x1b, b'[', b'9', b'7', b';', b'2', b'u']);
    }

    #[test]
    fn test_kitty_ctrl_a() {
        let bytes = encode_key(
            &Key::Character('a'),
            Modifiers::CTRL,
            TermMode::DISAMBIGUATE_ESC_CODES,
        );
        // 'a' = 97, ctrl modifier = 5
        assert_eq!(bytes, vec![0x1b, b'[', b'9', b'7', b';', b'5', b'u']);
    }

    #[test]
    fn test_kitty_arrow_up() {
        let bytes = encode_key(
            &Key::Named(NamedKey::ArrowUp),
            Modifiers::empty(),
            TermMode::DISAMBIGUATE_ESC_CODES,
        );
        // ArrowUp kitty code = 57352
        assert_eq!(bytes, vec![0x1b, b'[', b'5', b'7', b'3', b'5', b'2', b'u']);
    }

    #[test]
    fn test_kitty_f1() {
        let bytes = encode_key(
            &Key::Named(NamedKey::F1),
            Modifiers::empty(),
            TermMode::DISAMBIGUATE_ESC_CODES,
        );
        // F1 kitty code = 57364
        assert_eq!(bytes, vec![0x1b, b'[', b'5', b'7', b'3', b'6', b'4', b'u']);
    }

    #[test]
    fn test_kitty_release_event() {
        let bytes = encode_key_with_event(
            &Key::Character('a'),
            Modifiers::empty(),
            TermMode::DISAMBIGUATE_ESC_CODES | TermMode::REPORT_EVENT_TYPES,
            KeyEventType::Release,
        );
        // 'a' = 97, no modifiers = 1, release = 3
        assert_eq!(
            bytes,
            vec![0x1b, b'[', b'9', b'7', b';', b'1', b':', b'3', b'u']
        );
    }

    #[test]
    fn test_kitty_repeat_event() {
        let bytes = encode_key_with_event(
            &Key::Character('a'),
            Modifiers::empty(),
            TermMode::DISAMBIGUATE_ESC_CODES | TermMode::REPORT_EVENT_TYPES,
            KeyEventType::Repeat,
        );
        // 'a' = 97, no modifiers = 1, repeat = 2
        assert_eq!(
            bytes,
            vec![0x1b, b'[', b'9', b'7', b';', b'1', b':', b'2', b'u']
        );
    }

    #[test]
    fn test_release_without_kitty() {
        // Release events without Kitty protocol should produce nothing
        let bytes = encode_key_with_event(
            &Key::Character('a'),
            Modifiers::empty(),
            TermMode::empty(),
            KeyEventType::Release,
        );
        assert!(bytes.is_empty());
    }

    #[test]
    fn test_modifier_encoding() {
        assert_eq!(Modifiers::empty().xterm_encoded(), 1);
        assert_eq!(Modifiers::SHIFT.xterm_encoded(), 2);
        assert_eq!(Modifiers::ALT.xterm_encoded(), 3);
        assert_eq!((Modifiers::SHIFT | Modifiers::ALT).xterm_encoded(), 4);
        assert_eq!(Modifiers::CTRL.xterm_encoded(), 5);
        assert_eq!((Modifiers::SHIFT | Modifiers::CTRL).xterm_encoded(), 6);
        assert_eq!((Modifiers::ALT | Modifiers::CTRL).xterm_encoded(), 7);
        assert_eq!(
            (Modifiers::SHIFT | Modifiers::ALT | Modifiers::CTRL).xterm_encoded(),
            8
        );
    }

    #[test]
    fn test_kitty_modifier_encoding() {
        assert_eq!(Modifiers::empty().kitty_encoded(), 1);
        assert_eq!(Modifiers::SHIFT.kitty_encoded(), 2);
        assert_eq!(Modifiers::ALT.kitty_encoded(), 3);
        assert_eq!(Modifiers::CTRL.kitty_encoded(), 5);
        assert_eq!((Modifiers::SHIFT | Modifiers::CTRL).kitty_encoded(), 6);
    }

    #[test]
    fn test_numpad_normal_mode() {
        let bytes = encode_key(
            &Key::Named(NamedKey::Numpad5),
            Modifiers::empty(),
            TermMode::empty(),
        );
        assert_eq!(bytes, vec![b'5']);
    }

    #[test]
    fn test_numpad_app_keypad_mode() {
        let bytes = encode_key(
            &Key::Named(NamedKey::Numpad5),
            Modifiers::empty(),
            TermMode::APP_KEYPAD,
        );
        assert_eq!(bytes, vec![0x1b, b'O', b'u']);
    }

    #[test]
    fn test_write_u32() {
        let mut buf = Vec::new();
        write_u32(&mut buf, 0);
        assert_eq!(buf, vec![b'0']);

        buf.clear();
        write_u32(&mut buf, 1);
        assert_eq!(buf, vec![b'1']);

        buf.clear();
        write_u32(&mut buf, 97);
        assert_eq!(buf, vec![b'9', b'7']);

        buf.clear();
        write_u32(&mut buf, 57352);
        assert_eq!(buf, vec![b'5', b'7', b'3', b'5', b'2']);
    }
}
