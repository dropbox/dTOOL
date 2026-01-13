//! Mouse input encoding for terminal emulators.
//!
//! This module mirrors common terminal mouse reporting modes, including
//! X10/normal tracking, SGR (1006), and URXVT (1015) encodings.

use crate::keyboard::Modifiers;
use dterm_core::terminal::{MouseEncoding, MouseMode};

/// Mouse buttons used for press/release/motion events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    /// Left mouse button.
    Left,
    /// Middle mouse button.
    Middle,
    /// Right mouse button.
    Right,
}

impl MouseButton {
    #[must_use]
    fn code(self) -> u8 {
        match self {
            MouseButton::Left => 0,
            MouseButton::Middle => 1,
            MouseButton::Right => 2,
        }
    }
}

const SHIFT_MASK: u8 = 4;
const ALT_MASK: u8 = 8;
const CTRL_MASK: u8 = 16;

#[must_use]
fn modifiers_bits(modifiers: Modifiers) -> u8 {
    let mut bits = 0;
    if modifiers.contains(Modifiers::SHIFT) {
        bits |= SHIFT_MASK;
    }
    if modifiers.contains(Modifiers::ALT) {
        bits |= ALT_MASK;
    }
    if modifiers.contains(Modifiers::CTRL) {
        bits |= CTRL_MASK;
    }
    bits
}

#[must_use]
fn encode_x10(cb: u8, col: u16, row: u16) -> Vec<u8> {
    let cx = ((col + 1).min(223) as u8).saturating_add(32);
    let cy = ((row + 1).min(223) as u8).saturating_add(32);
    vec![0x1b, b'[', b'M', cb.saturating_add(32), cx, cy]
}

#[allow(clippy::cast_possible_truncation)]
fn encode_utf8_coord(coord: u16, output: &mut Vec<u8>) {
    let c = coord.saturating_add(32);
    if c < 128 {
        output.push(c as u8);
    } else {
        let c = c.min(2047);
        output.push(0xC0 | ((c >> 6) as u8));
        output.push(0x80 | ((c & 0x3F) as u8));
    }
}

#[must_use]
fn encode_utf8(cb: u8, col: u16, row: u16) -> Vec<u8> {
    let mut result = vec![0x1b, b'[', b'M', cb.saturating_add(32)];
    encode_utf8_coord(col + 1, &mut result);
    encode_utf8_coord(row + 1, &mut result);
    result
}

#[must_use]
fn encode_sgr(cb: u8, col: u16, row: u16, release: bool) -> Vec<u8> {
    let terminator = if release { 'm' } else { 'M' };
    format!("\x1b[<{};{};{}{}", cb, col + 1, row + 1, terminator).into_bytes()
}

#[must_use]
fn encode_urxvt(cb: u8, col: u16, row: u16) -> Vec<u8> {
    format!(
        "\x1b[{};{};{}M",
        u16::from(cb.saturating_add(32)),
        col + 1,
        row + 1
    )
    .into_bytes()
}

/// Encode a mouse button press event.
#[must_use]
pub fn encode_mouse_press(
    button: MouseButton,
    col: u16,
    row: u16,
    modifiers: Modifiers,
    mouse_mode: MouseMode,
    mouse_encoding: MouseEncoding,
) -> Option<Vec<u8>> {
    if mouse_mode == MouseMode::None {
        return None;
    }

    let cb = button.code() | modifiers_bits(modifiers);

    Some(match mouse_encoding {
        MouseEncoding::X10 => encode_x10(cb, col, row),
        MouseEncoding::Utf8 => encode_utf8(cb, col, row),
        MouseEncoding::Sgr | MouseEncoding::SgrPixel => encode_sgr(cb, col, row, false),
        MouseEncoding::Urxvt => encode_urxvt(cb, col, row),
    })
}

/// Encode a mouse button release event.
#[must_use]
pub fn encode_mouse_release(
    button: MouseButton,
    col: u16,
    row: u16,
    modifiers: Modifiers,
    mouse_mode: MouseMode,
    mouse_encoding: MouseEncoding,
) -> Option<Vec<u8>> {
    if mouse_mode == MouseMode::None {
        return None;
    }

    let modifiers = modifiers_bits(modifiers);

    Some(match mouse_encoding {
        MouseEncoding::X10 => encode_x10(3 | modifiers, col, row),
        MouseEncoding::Utf8 => encode_utf8(3 | modifiers, col, row),
        MouseEncoding::Sgr | MouseEncoding::SgrPixel => {
            encode_sgr(button.code() | modifiers, col, row, true)
        }
        MouseEncoding::Urxvt => encode_urxvt(3 | modifiers, col, row),
    })
}

/// Encode a mouse motion event.
#[must_use]
pub fn encode_mouse_motion(
    button: Option<MouseButton>,
    col: u16,
    row: u16,
    modifiers: Modifiers,
    mouse_mode: MouseMode,
    mouse_encoding: MouseEncoding,
) -> Option<Vec<u8>> {
    match mouse_mode {
        MouseMode::None | MouseMode::Normal => return None,
        MouseMode::ButtonEvent => {
            button?;
        }
        MouseMode::AnyEvent => {}
    }

    let button_code = button.map(MouseButton::code).unwrap_or(3);
    let cb = button_code | modifiers_bits(modifiers) | 32;

    Some(match mouse_encoding {
        MouseEncoding::X10 => encode_x10(cb, col, row),
        MouseEncoding::Utf8 => encode_utf8(cb, col, row),
        MouseEncoding::Sgr | MouseEncoding::SgrPixel => encode_sgr(cb, col, row, false),
        MouseEncoding::Urxvt => encode_urxvt(cb, col, row),
    })
}

/// Encode a mouse wheel event (up/down).
#[must_use]
pub fn encode_mouse_wheel(
    up: bool,
    col: u16,
    row: u16,
    modifiers: Modifiers,
    mouse_mode: MouseMode,
    mouse_encoding: MouseEncoding,
) -> Option<Vec<u8>> {
    if mouse_mode == MouseMode::None {
        return None;
    }

    let button = if up { 64 } else { 65 };
    let cb = button | modifiers_bits(modifiers);

    Some(match mouse_encoding {
        MouseEncoding::X10 => encode_x10(cb, col, row),
        MouseEncoding::Utf8 => encode_utf8(cb, col, row),
        MouseEncoding::Sgr | MouseEncoding::SgrPixel => encode_sgr(cb, col, row, false),
        MouseEncoding::Urxvt => encode_urxvt(cb, col, row),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_press_x10_encoding() {
        let encoded = encode_mouse_press(
            MouseButton::Left,
            10,
            5,
            Modifiers::empty(),
            MouseMode::Normal,
            MouseEncoding::X10,
        )
        .unwrap();
        assert_eq!(encoded, vec![0x1b, b'[', b'M', 32, 43, 38]);
    }

    #[test]
    fn mouse_press_sgr_encoding() {
        let encoded = encode_mouse_press(
            MouseButton::Left,
            10,
            5,
            Modifiers::empty(),
            MouseMode::Normal,
            MouseEncoding::Sgr,
        )
        .unwrap();
        assert_eq!(encoded, b"\x1b[<0;11;6M");
    }

    #[test]
    fn mouse_release_sgr_encoding() {
        let encoded = encode_mouse_release(
            MouseButton::Left,
            10,
            5,
            Modifiers::empty(),
            MouseMode::Normal,
            MouseEncoding::Sgr,
        )
        .unwrap();
        assert_eq!(encoded, b"\x1b[<0;11;6m");
    }

    #[test]
    fn mouse_motion_requires_button_in_button_event_mode() {
        let encoded = encode_mouse_motion(
            None,
            10,
            5,
            Modifiers::empty(),
            MouseMode::ButtonEvent,
            MouseEncoding::X10,
        );
        assert!(encoded.is_none());
    }

    #[test]
    fn mouse_motion_urxvt_encoding() {
        let encoded = encode_mouse_motion(
            Some(MouseButton::Left),
            10,
            5,
            Modifiers::empty(),
            MouseMode::ButtonEvent,
            MouseEncoding::Urxvt,
        )
        .unwrap();
        assert_eq!(encoded, b"\x1b[64;11;6M");
    }

    #[test]
    fn mouse_wheel_urxvt_encoding() {
        let encoded = encode_mouse_wheel(
            true,
            10,
            5,
            Modifiers::empty(),
            MouseMode::Normal,
            MouseEncoding::Urxvt,
        )
        .unwrap();
        assert_eq!(encoded, b"\x1b[96;11;6M");
    }
}
