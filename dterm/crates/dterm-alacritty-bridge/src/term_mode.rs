//! Terminal mode bitflags for Alacritty compatibility.
//!
//! This module provides a bitflags-style `TermMode` type that mirrors Alacritty's
//! mode tracking. It can be constructed from dterm-core's `TerminalModes` struct.

use bitflags::bitflags;
use dterm_core::terminal::{KittyKeyboardFlags, MouseMode, TerminalModes};

bitflags! {
    /// Terminal mode flags matching Alacritty's `TermMode`.
    ///
    /// These flags represent various terminal modes that can be queried
    /// efficiently using bitwise operations.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct TermMode: u32 {
        /// Cursor is visible (DECTCEM).
        const SHOW_CURSOR = 1 << 0;
        /// Application cursor keys mode (DECCKM).
        const APP_CURSOR = 1 << 1;
        /// Application keypad mode (DECKPAM).
        const APP_KEYPAD = 1 << 2;
        /// Any mouse tracking mode is enabled.
        const MOUSE_REPORT_CLICK = 1 << 3;
        /// Bracketed paste mode.
        const BRACKETED_PASTE = 1 << 4;
        /// SGR mouse mode (1006).
        const SGR_MOUSE = 1 << 5;
        /// Mouse motion tracking (1003).
        const MOUSE_MOTION = 1 << 6;
        /// Auto-wrap mode (DECAWM).
        const LINE_WRAP = 1 << 7;
        /// LNM mode - LF also does CR.
        const LINE_FEED_NEW_LINE = 1 << 8;
        /// Origin mode (DECOM).
        const ORIGIN = 1 << 9;
        /// Insert mode (IRM).
        const INSERT = 1 << 10;
        /// Focus in/out reporting (1004).
        const FOCUS_IN_OUT = 1 << 11;
        /// Alternate screen buffer active.
        const ALT_SCREEN = 1 << 12;
        /// Mouse button and drag tracking (1002).
        const MOUSE_DRAG = 1 << 13;
        /// UTF-8 mouse encoding (1005).
        const UTF8_MOUSE = 1 << 14;
        /// Alternate scroll mode (1007).
        const ALTERNATE_SCROLL = 1 << 15;
        /// Vi mode is active.
        const VI = 1 << 16;
        /// Urgency hints mode.
        const URGENCY_HINTS = 1 << 17;
        /// Synchronized output mode (2026).
        const SYNCHRONIZED_OUTPUT = 1 << 18;
        /// Reverse video mode (DECSET 5).
        const REVERSE_VIDEO = 1 << 19;
        /// Cursor blink mode (DECSET 12).
        const CURSOR_BLINK = 1 << 20;
        /// 132 column mode (DECSET 3).
        const COLUMN_MODE_132 = 1 << 21;
        /// Reverse wraparound mode (DECSET 45).
        const REVERSE_WRAPAROUND = 1 << 22;
        /// VT52 compatibility mode.
        const VT52_MODE = 1 << 23;

        // Kitty keyboard protocol flags (CSI > u).
        /// Disambiguate escape codes - send Esc, Alt+key, Ctrl+key using CSI u.
        const DISAMBIGUATE_ESC_CODES = 1 << 24;
        /// Report key repeat and release events.
        const REPORT_EVENT_TYPES = 1 << 25;
        /// Report alternate key values (shifted keys, base layout key).
        const REPORT_ALTERNATE_KEYS = 1 << 26;
        /// Report all keys as escape sequences (including Enter, Tab, Backspace).
        const REPORT_ALL_KEYS_AS_ESC = 1 << 27;
        /// Report associated text with key events.
        const REPORT_ASSOCIATED_TEXT = 1 << 28;

        /// Aggregate: any Kitty keyboard protocol flag is active.
        const KITTY_KEYBOARD_PROTOCOL = Self::DISAMBIGUATE_ESC_CODES.bits()
            | Self::REPORT_EVENT_TYPES.bits()
            | Self::REPORT_ALTERNATE_KEYS.bits()
            | Self::REPORT_ALL_KEYS_AS_ESC.bits()
            | Self::REPORT_ASSOCIATED_TEXT.bits();

        /// Aggregate: any mouse mode is active.
        const MOUSE_MODE = Self::MOUSE_REPORT_CLICK.bits()
            | Self::MOUSE_MOTION.bits()
            | Self::MOUSE_DRAG.bits();

        /// All flags.
        const ANY = !0;
    }
}

impl TermMode {
    /// Create `TermMode` from dterm-core's `TerminalModes`.
    ///
    /// Note: This does not include Kitty keyboard protocol flags.
    /// Use `from_terminal_modes_with_keyboard` for full mode conversion.
    pub fn from_terminal_modes(modes: &TerminalModes, vi_mode: bool) -> Self {
        Self::from_terminal_modes_with_keyboard(modes, vi_mode, KittyKeyboardFlags::none())
    }

    /// Create `TermMode` from dterm-core's `TerminalModes` and Kitty keyboard flags.
    pub fn from_terminal_modes_with_keyboard(
        modes: &TerminalModes,
        vi_mode: bool,
        kitty_keyboard: KittyKeyboardFlags,
    ) -> Self {
        let mut flags = Self::empty();

        if modes.cursor_visible {
            flags |= Self::SHOW_CURSOR;
        }
        if modes.application_cursor_keys {
            flags |= Self::APP_CURSOR;
        }
        if modes.application_keypad {
            flags |= Self::APP_KEYPAD;
        }
        if modes.bracketed_paste {
            flags |= Self::BRACKETED_PASTE;
        }
        if modes.auto_wrap {
            flags |= Self::LINE_WRAP;
        }
        if modes.new_line_mode {
            flags |= Self::LINE_FEED_NEW_LINE;
        }
        if modes.origin_mode {
            flags |= Self::ORIGIN;
        }
        if modes.insert_mode {
            flags |= Self::INSERT;
        }
        if modes.focus_reporting {
            flags |= Self::FOCUS_IN_OUT;
        }
        if modes.alternate_screen {
            flags |= Self::ALT_SCREEN;
        }
        if modes.synchronized_output {
            flags |= Self::SYNCHRONIZED_OUTPUT;
        }
        if modes.reverse_video {
            flags |= Self::REVERSE_VIDEO;
        }
        if modes.cursor_blink {
            flags |= Self::CURSOR_BLINK;
        }
        if modes.column_mode_132 {
            flags |= Self::COLUMN_MODE_132;
        }
        if modes.reverse_wraparound {
            flags |= Self::REVERSE_WRAPAROUND;
        }
        if modes.vt52_mode {
            flags |= Self::VT52_MODE;
        }
        if vi_mode {
            flags |= Self::VI;
        }

        // Mouse mode flags
        match modes.mouse_mode {
            MouseMode::None => {}
            MouseMode::Normal => {
                flags |= Self::MOUSE_REPORT_CLICK;
            }
            MouseMode::ButtonEvent => {
                flags |= Self::MOUSE_REPORT_CLICK | Self::MOUSE_DRAG;
            }
            MouseMode::AnyEvent => {
                flags |= Self::MOUSE_REPORT_CLICK | Self::MOUSE_MOTION;
            }
        }

        // Mouse encoding flags
        match modes.mouse_encoding {
            dterm_core::terminal::MouseEncoding::X10 => {}
            dterm_core::terminal::MouseEncoding::Utf8 => {
                flags |= Self::UTF8_MOUSE;
            }
            dterm_core::terminal::MouseEncoding::Sgr => {
                flags |= Self::SGR_MOUSE;
            }
            dterm_core::terminal::MouseEncoding::Urxvt => {
                // URXVT doesn't set any specific flags
            }
            dterm_core::terminal::MouseEncoding::SgrPixel => {
                flags |= Self::SGR_MOUSE;
            }
        }

        // Kitty keyboard protocol flags
        if kitty_keyboard.disambiguate() {
            flags |= Self::DISAMBIGUATE_ESC_CODES;
        }
        if kitty_keyboard.report_events() {
            flags |= Self::REPORT_EVENT_TYPES;
        }
        if kitty_keyboard.report_alternates() {
            flags |= Self::REPORT_ALTERNATE_KEYS;
        }
        if kitty_keyboard.report_all_keys() {
            flags |= Self::REPORT_ALL_KEYS_AS_ESC;
        }
        if kitty_keyboard.report_text() {
            flags |= Self::REPORT_ASSOCIATED_TEXT;
        }

        flags
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dterm_core::terminal::{MouseEncoding, TerminalModes};

    #[test]
    fn term_mode_default() {
        let modes = TerminalModes::new();
        let term_mode = TermMode::from_terminal_modes(&modes, false);

        // Default should have cursor visible and line wrap
        assert!(term_mode.contains(TermMode::SHOW_CURSOR));
        assert!(term_mode.contains(TermMode::LINE_WRAP));

        // Should not have vi mode
        assert!(!term_mode.contains(TermMode::VI));
    }

    #[test]
    fn term_mode_vi_mode() {
        let modes = TerminalModes::new();
        let term_mode = TermMode::from_terminal_modes(&modes, true);

        assert!(term_mode.contains(TermMode::VI));
    }

    #[test]
    fn term_mode_mouse_modes() {
        let mut modes = TerminalModes::new();

        // Test X10 mouse mode
        modes.mouse_mode = MouseMode::Normal;
        let term_mode = TermMode::from_terminal_modes(&modes, false);
        assert!(term_mode.contains(TermMode::MOUSE_REPORT_CLICK));
        assert!(!term_mode.contains(TermMode::MOUSE_MOTION));

        // Test button event mode
        modes.mouse_mode = MouseMode::ButtonEvent;
        let term_mode = TermMode::from_terminal_modes(&modes, false);
        assert!(term_mode.contains(TermMode::MOUSE_REPORT_CLICK));
        assert!(term_mode.contains(TermMode::MOUSE_DRAG));

        // Test any event mode
        modes.mouse_mode = MouseMode::AnyEvent;
        let term_mode = TermMode::from_terminal_modes(&modes, false);
        assert!(term_mode.contains(TermMode::MOUSE_REPORT_CLICK));
        assert!(term_mode.contains(TermMode::MOUSE_MOTION));
    }

    #[test]
    fn term_mode_sgr_mouse() {
        let mut modes = TerminalModes::new();
        modes.mouse_encoding = MouseEncoding::Sgr;
        let term_mode = TermMode::from_terminal_modes(&modes, false);
        assert!(term_mode.contains(TermMode::SGR_MOUSE));
    }

    #[test]
    fn term_mode_bracketed_paste() {
        let mut modes = TerminalModes::new();
        modes.bracketed_paste = true;
        let term_mode = TermMode::from_terminal_modes(&modes, false);
        assert!(term_mode.contains(TermMode::BRACKETED_PASTE));
    }

    #[test]
    fn term_mode_alt_screen() {
        let mut modes = TerminalModes::new();
        modes.alternate_screen = true;
        let term_mode = TermMode::from_terminal_modes(&modes, false);
        assert!(term_mode.contains(TermMode::ALT_SCREEN));
    }

    #[test]
    fn term_mode_contains_any_mouse() {
        let mut modes = TerminalModes::new();
        modes.mouse_mode = MouseMode::Normal;
        let term_mode = TermMode::from_terminal_modes(&modes, false);

        // Check the aggregate mouse mode flag
        assert!(term_mode.intersects(TermMode::MOUSE_MODE));
    }

    #[test]
    fn term_mode_kitty_keyboard_none() {
        let modes = TerminalModes::new();
        let kitty = KittyKeyboardFlags::none();
        let term_mode = TermMode::from_terminal_modes_with_keyboard(&modes, false, kitty);

        // No Kitty keyboard flags should be set
        assert!(!term_mode.intersects(TermMode::KITTY_KEYBOARD_PROTOCOL));
        assert!(!term_mode.contains(TermMode::DISAMBIGUATE_ESC_CODES));
        assert!(!term_mode.contains(TermMode::REPORT_EVENT_TYPES));
        assert!(!term_mode.contains(TermMode::REPORT_ALTERNATE_KEYS));
        assert!(!term_mode.contains(TermMode::REPORT_ALL_KEYS_AS_ESC));
        assert!(!term_mode.contains(TermMode::REPORT_ASSOCIATED_TEXT));
    }

    #[test]
    fn term_mode_kitty_keyboard_disambiguate() {
        let modes = TerminalModes::new();
        let kitty = KittyKeyboardFlags::from_bits(KittyKeyboardFlags::DISAMBIGUATE);
        let term_mode = TermMode::from_terminal_modes_with_keyboard(&modes, false, kitty);

        assert!(term_mode.contains(TermMode::DISAMBIGUATE_ESC_CODES));
        assert!(!term_mode.contains(TermMode::REPORT_EVENT_TYPES));
        assert!(term_mode.intersects(TermMode::KITTY_KEYBOARD_PROTOCOL));
    }

    #[test]
    fn term_mode_kitty_keyboard_all_flags() {
        let modes = TerminalModes::new();
        let kitty = KittyKeyboardFlags::from_bits(0x1F); // All 5 flags
        let term_mode = TermMode::from_terminal_modes_with_keyboard(&modes, false, kitty);

        assert!(term_mode.contains(TermMode::DISAMBIGUATE_ESC_CODES));
        assert!(term_mode.contains(TermMode::REPORT_EVENT_TYPES));
        assert!(term_mode.contains(TermMode::REPORT_ALTERNATE_KEYS));
        assert!(term_mode.contains(TermMode::REPORT_ALL_KEYS_AS_ESC));
        assert!(term_mode.contains(TermMode::REPORT_ASSOCIATED_TEXT));
        assert!(term_mode.intersects(TermMode::KITTY_KEYBOARD_PROTOCOL));
    }

    #[test]
    fn term_mode_kitty_keyboard_report_events_only() {
        let modes = TerminalModes::new();
        let kitty = KittyKeyboardFlags::from_bits(KittyKeyboardFlags::REPORT_EVENTS);
        let term_mode = TermMode::from_terminal_modes_with_keyboard(&modes, false, kitty);

        assert!(!term_mode.contains(TermMode::DISAMBIGUATE_ESC_CODES));
        assert!(term_mode.contains(TermMode::REPORT_EVENT_TYPES));
        assert!(!term_mode.contains(TermMode::REPORT_ALTERNATE_KEYS));
        assert!(!term_mode.contains(TermMode::REPORT_ALL_KEYS_AS_ESC));
        assert!(!term_mode.contains(TermMode::REPORT_ASSOCIATED_TEXT));
    }
}
