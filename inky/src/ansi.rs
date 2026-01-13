//! ANSI escape sequence parsing for styled text.
//!
//! This module provides parsing of ANSI SGR (Select Graphic Rendition) escape sequences
//! into [`style::StyledSpan`](crate::style::StyledSpan) arrays that can be rendered by inky.
//!
//! # Supported Sequences
//!
//! - Reset: `\x1b[0m`
//! - Bold: `\x1b[1m`
//! - Dim: `\x1b[2m`
//! - Italic: `\x1b[3m`
//! - Underline: `\x1b[4m`
//! - Inverse: `\x1b[7m`
//! - Strikethrough: `\x1b[9m`
//! - Standard colors (30-37, 40-47): `\x1b[31m` (red foreground)
//! - Bright colors (90-97, 100-107): `\x1b[91m` (bright red foreground)
//! - 256-color mode: `\x1b[38;5;123m` (foreground), `\x1b[48;5;123m` (background)
//! - True color (24-bit): `\x1b[38;2;R;G;Bm` (foreground), `\x1b[48;2;R;G;Bm` (background)
//!
//! # Example
//!
//! ```
//! use inky::ansi::parse_ansi;
//!
//! let spans = parse_ansi("\x1b[31mError:\x1b[0m file not found");
//! assert_eq!(spans.len(), 2);
//! assert_eq!(spans[0].text, "Error:");
//! assert_eq!(spans[1].text, " file not found");
//! ```

use crate::style::{Color, StyledSpan, StyledSpanOwned};
use std::borrow::Cow;

/// Current style state during parsing.
#[derive(Clone, Default)]
struct StyleState {
    fg: Option<Color>,
    bg: Option<Color>,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    inverse: bool,
    strikethrough: bool,
}

impl StyleState {
    /// Reset all attributes to default.
    fn reset(&mut self) {
        *self = Self::default();
    }

    /// Convert current state to a StyledSpanOwned with given text.
    fn to_span(&self, text: String) -> StyledSpanOwned {
        StyledSpan {
            text: Cow::Owned(text),
            color: self.fg,
            background_color: self.bg,
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            strikethrough: self.strikethrough,
            dim: self.dim,
            inverse: self.inverse,
        }
    }
}

/// Parse ANSI-escaped text into styled spans.
///
/// Converts a string containing ANSI escape sequences into a vector of
/// [`StyledSpanOwned`] objects that can be rendered by inky.
///
/// # Example
///
/// ```
/// use inky::ansi::parse_ansi;
/// use inky::style::Color;
///
/// let spans = parse_ansi("\x1b[1;31mBold Red\x1b[0m Normal");
/// assert_eq!(spans.len(), 2);
/// assert_eq!(spans[0].text, "Bold Red");
/// assert!(spans[0].bold);
/// assert_eq!(spans[0].color, Some(Color::Red));
/// ```
pub fn parse_ansi(input: &str) -> Vec<StyledSpanOwned> {
    let mut spans = Vec::new();
    let mut state = StyleState::default();
    let mut current_text = String::new();
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Check for CSI sequence (ESC [)
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['

                // Flush current text as a span
                if !current_text.is_empty() {
                    spans.push(state.to_span(std::mem::take(&mut current_text)));
                }

                // Parse SGR parameters
                let mut params = Vec::new();
                let mut current_param = String::new();

                loop {
                    match chars.next() {
                        Some(c) if c.is_ascii_digit() => {
                            current_param.push(c);
                        }
                        Some(';') => {
                            if !current_param.is_empty() {
                                if let Ok(n) = current_param.parse::<u8>() {
                                    params.push(n);
                                }
                                current_param.clear();
                            } else {
                                params.push(0); // Empty parameter defaults to 0
                            }
                        }
                        Some('m') => {
                            // SGR sequence complete
                            if !current_param.is_empty() {
                                if let Ok(n) = current_param.parse::<u8>() {
                                    params.push(n);
                                }
                            }
                            apply_sgr_params(&mut state, &params);
                            break;
                        }
                        Some(c) if c.is_ascii_alphabetic() => {
                            // Non-SGR sequence - skip it
                            break;
                        }
                        _ => {
                            // Malformed sequence - skip
                            break;
                        }
                    }
                }
            } else {
                // Not a CSI sequence, treat as literal
                current_text.push(c);
            }
        } else {
            current_text.push(c);
        }
    }

    // Flush remaining text
    if !current_text.is_empty() {
        spans.push(state.to_span(current_text));
    }

    spans
}

/// Apply SGR (Select Graphic Rendition) parameters to style state.
fn apply_sgr_params(state: &mut StyleState, params: &[u8]) {
    if params.is_empty() {
        state.reset();
        return;
    }

    let mut i = 0;
    while i < params.len() {
        match params[i] {
            0 => state.reset(),
            1 => state.bold = true,
            2 => state.dim = true,
            3 => state.italic = true,
            4 => state.underline = true,
            7 => state.inverse = true,
            9 => state.strikethrough = true,

            // Disable attributes
            21 | 22 => {
                state.bold = false;
                state.dim = false;
            }
            23 => state.italic = false,
            24 => state.underline = false,
            27 => state.inverse = false,
            29 => state.strikethrough = false,

            // Standard foreground colors (30-37)
            30 => state.fg = Some(Color::Black),
            31 => state.fg = Some(Color::Red),
            32 => state.fg = Some(Color::Green),
            33 => state.fg = Some(Color::Yellow),
            34 => state.fg = Some(Color::Blue),
            35 => state.fg = Some(Color::Magenta),
            36 => state.fg = Some(Color::Cyan),
            37 => state.fg = Some(Color::White),
            38 => {
                // Extended foreground color
                if let Some(color) = parse_extended_color(params, &mut i) {
                    state.fg = Some(color);
                }
            }
            39 => state.fg = None, // Default foreground

            // Standard background colors (40-47)
            40 => state.bg = Some(Color::Black),
            41 => state.bg = Some(Color::Red),
            42 => state.bg = Some(Color::Green),
            43 => state.bg = Some(Color::Yellow),
            44 => state.bg = Some(Color::Blue),
            45 => state.bg = Some(Color::Magenta),
            46 => state.bg = Some(Color::Cyan),
            47 => state.bg = Some(Color::White),
            48 => {
                // Extended background color
                if let Some(color) = parse_extended_color(params, &mut i) {
                    state.bg = Some(color);
                }
            }
            49 => state.bg = None, // Default background

            // Bright foreground colors (90-97)
            90 => state.fg = Some(Color::BrightBlack),
            91 => state.fg = Some(Color::BrightRed),
            92 => state.fg = Some(Color::BrightGreen),
            93 => state.fg = Some(Color::BrightYellow),
            94 => state.fg = Some(Color::BrightBlue),
            95 => state.fg = Some(Color::BrightMagenta),
            96 => state.fg = Some(Color::BrightCyan),
            97 => state.fg = Some(Color::BrightWhite),

            // Bright background colors (100-107)
            100 => state.bg = Some(Color::BrightBlack),
            101 => state.bg = Some(Color::BrightRed),
            102 => state.bg = Some(Color::BrightGreen),
            103 => state.bg = Some(Color::BrightYellow),
            104 => state.bg = Some(Color::BrightBlue),
            105 => state.bg = Some(Color::BrightMagenta),
            106 => state.bg = Some(Color::BrightCyan),
            107 => state.bg = Some(Color::BrightWhite),

            _ => {} // Unknown code, ignore
        }
        i += 1;
    }
}

/// Parse extended color (256-color or 24-bit RGB).
fn parse_extended_color(params: &[u8], i: &mut usize) -> Option<Color> {
    if *i + 1 >= params.len() {
        return None;
    }

    match params[*i + 1] {
        5 => {
            // 256-color mode: 38;5;N or 48;5;N
            if *i + 2 < params.len() {
                let color_idx = params[*i + 2];
                *i += 2;
                Some(Color::Ansi256(color_idx))
            } else {
                None
            }
        }
        2 => {
            // 24-bit RGB mode: 38;2;R;G;B or 48;2;R;G;B
            if *i + 4 < params.len() {
                let r = params[*i + 2];
                let g = params[*i + 3];
                let b = params[*i + 4];
                *i += 4;
                Some(Color::Rgb(r, g, b))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Strip ANSI escape sequences from text, returning plain text.
///
/// # Example
///
/// ```
/// use inky::ansi::strip_ansi;
///
/// let plain = strip_ansi("\x1b[31mRed\x1b[0m text");
/// assert_eq!(plain, "Red text");
/// ```
pub fn strip_ansi(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next(); // consume '['
                          // Skip until we hit a letter
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty() {
        let spans = parse_ansi("");
        assert!(spans.is_empty());
    }

    #[test]
    fn test_parse_plain_text() {
        let spans = parse_ansi("Hello, World!");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "Hello, World!");
        assert!(!spans[0].bold);
        assert_eq!(spans[0].color, None);
    }

    #[test]
    fn test_parse_bold() {
        let spans = parse_ansi("\x1b[1mBold\x1b[0m");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "Bold");
        assert!(spans[0].bold);
    }

    #[test]
    fn test_parse_colors() {
        let spans = parse_ansi("\x1b[31mRed\x1b[32mGreen\x1b[0m");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].text, "Red");
        assert_eq!(spans[0].color, Some(Color::Red));
        assert_eq!(spans[1].text, "Green");
        assert_eq!(spans[1].color, Some(Color::Green));
    }

    #[test]
    fn test_parse_bright_colors() {
        let spans = parse_ansi("\x1b[91mBright Red\x1b[0m");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].color, Some(Color::BrightRed));
    }

    #[test]
    fn test_parse_256_color() {
        let spans = parse_ansi("\x1b[38;5;123mCustom\x1b[0m");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].color, Some(Color::Ansi256(123)));
    }

    #[test]
    fn test_parse_rgb_color() {
        let spans = parse_ansi("\x1b[38;2;255;128;64mRGB\x1b[0m");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].color, Some(Color::Rgb(255, 128, 64)));
    }

    #[test]
    fn test_parse_combined_attributes() {
        let spans = parse_ansi("\x1b[1;3;31mBold Italic Red\x1b[0m");
        assert_eq!(spans.len(), 1);
        assert!(spans[0].bold);
        assert!(spans[0].italic);
        assert_eq!(spans[0].color, Some(Color::Red));
    }

    #[test]
    fn test_parse_background_color() {
        let spans = parse_ansi("\x1b[41mRed BG\x1b[0m");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].background_color, Some(Color::Red));
    }

    #[test]
    fn test_parse_mixed_text() {
        let spans = parse_ansi("Normal \x1b[31mRed\x1b[0m Normal");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].text, "Normal ");
        assert_eq!(spans[0].color, None);
        assert_eq!(spans[1].text, "Red");
        assert_eq!(spans[1].color, Some(Color::Red));
        assert_eq!(spans[2].text, " Normal");
        assert_eq!(spans[2].color, None);
    }

    #[test]
    fn test_strip_ansi() {
        let plain = strip_ansi("\x1b[1;31mBold Red\x1b[0m Normal");
        assert_eq!(plain, "Bold Red Normal");
    }

    #[test]
    fn test_strip_ansi_plain() {
        let plain = strip_ansi("No escape codes");
        assert_eq!(plain, "No escape codes");
    }

    #[test]
    fn test_parse_reset() {
        let spans = parse_ansi("\x1b[31mRed\x1b[0mReset");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].color, Some(Color::Red));
        assert_eq!(spans[1].color, None);
        assert!(!spans[1].bold);
    }

    #[test]
    fn test_parse_dim() {
        let spans = parse_ansi("\x1b[2mDim\x1b[0m");
        assert_eq!(spans.len(), 1);
        assert!(spans[0].dim);
    }

    #[test]
    fn test_parse_underline() {
        let spans = parse_ansi("\x1b[4mUnderline\x1b[0m");
        assert_eq!(spans.len(), 1);
        assert!(spans[0].underline);
    }

    #[test]
    fn test_parse_strikethrough() {
        let spans = parse_ansi("\x1b[9mStrikethrough\x1b[0m");
        assert_eq!(spans.len(), 1);
        assert!(spans[0].strikethrough);
    }

    #[test]
    fn test_parse_inverse() {
        let spans = parse_ansi("\x1b[7mInverse\x1b[0m");
        assert_eq!(spans.len(), 1);
        assert!(spans[0].inverse);
    }
}
