//! Text wrapping utilities for TUI rendering
//!
//! Provides word-aware text wrapping for chat messages.

use ratatui::text::{Line, Span, Text};

/// Wrap text to fit within a given width.
///
/// This function performs word-aware wrapping, breaking at word boundaries
/// when possible. If a single word exceeds the width, it will be broken
/// at character boundaries.
///
/// # Arguments
/// * `text` - The Text to wrap
/// * `width` - Maximum width in characters
///
/// # Returns
/// A new Text with lines wrapped to fit the width
pub fn wrap_text(text: &Text<'_>, width: usize) -> Text<'static> {
    if width == 0 {
        return Text::default();
    }

    let mut wrapped_lines: Vec<Line<'static>> = Vec::new();

    for line in &text.lines {
        let wrapped = wrap_line(line, width);
        wrapped_lines.extend(wrapped);
    }

    Text::from(wrapped_lines)
}

/// Wrap a single Line to fit within a given width.
///
/// Preserves styling from spans when wrapping.
fn wrap_line(line: &Line<'_>, width: usize) -> Vec<Line<'static>> {
    // First, flatten the line into a sequence of (char, style) pairs
    let mut chars_with_styles: Vec<(char, ratatui::style::Style)> = Vec::new();

    for span in &line.spans {
        for c in span.content.chars() {
            chars_with_styles.push((c, span.style));
        }
    }

    if chars_with_styles.is_empty() {
        return vec![Line::default()];
    }

    // Simple word-aware wrapping
    let mut result: Vec<Line<'static>> = Vec::new();
    let mut current_line_chars: Vec<(char, ratatui::style::Style)> = Vec::new();
    let mut current_width = 0;
    let mut word_start = 0;
    let mut in_word = false;

    for (i, &(c, style)) in chars_with_styles.iter().enumerate() {
        if c == '\n' {
            // Explicit newline - flush current line
            result.push(chars_to_line(&current_line_chars));
            current_line_chars.clear();
            current_width = 0;
            in_word = false;
            word_start = i + 1;
            continue;
        }

        let char_width = unicode_display_width(c);

        if c.is_whitespace() {
            in_word = false;
            word_start = i + 1;
        } else if !in_word {
            in_word = true;
            word_start = i;
        }

        // Check if adding this char would exceed width
        if current_width + char_width > width {
            if in_word && word_start > 0 && word_start <= current_line_chars.len() {
                // Try to break at word boundary
                let word_len = current_line_chars.len() - word_start;
                if word_len < width / 2 {
                    // Word is short enough to move to next line
                    let word_chars: Vec<_> = current_line_chars.drain(word_start..).collect();
                    // Remove trailing whitespace from current line
                    while current_line_chars
                        .last()
                        .is_some_and(|(c, _)| c.is_whitespace())
                    {
                        current_line_chars.pop();
                    }
                    result.push(chars_to_line(&current_line_chars));
                    current_line_chars = word_chars;
                    current_width = current_line_chars
                        .iter()
                        .map(|(c, _)| unicode_display_width(*c))
                        .sum();
                } else {
                    // Word too long, break at character boundary
                    result.push(chars_to_line(&current_line_chars));
                    current_line_chars.clear();
                    current_width = 0;
                }
            } else {
                // Break at character boundary
                result.push(chars_to_line(&current_line_chars));
                current_line_chars.clear();
                current_width = 0;
            }
            word_start = 0;
        }

        current_line_chars.push((c, style));
        current_width += char_width;
    }

    // Flush remaining characters
    if !current_line_chars.is_empty() {
        result.push(chars_to_line(&current_line_chars));
    }

    if result.is_empty() {
        result.push(Line::default());
    }

    result
}

/// Convert a sequence of (char, style) pairs into a Line, merging consecutive
/// chars with the same style into spans.
fn chars_to_line(chars: &[(char, ratatui::style::Style)]) -> Line<'static> {
    if chars.is_empty() {
        return Line::default();
    }

    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut current_text = String::new();
    let mut current_style = chars[0].1;

    for &(c, style) in chars {
        if style == current_style {
            current_text.push(c);
        } else {
            if !current_text.is_empty() {
                spans.push(Span::styled(current_text, current_style));
                current_text = String::new();
            }
            current_style = style;
            current_text.push(c);
        }
    }

    if !current_text.is_empty() {
        spans.push(Span::styled(current_text, current_style));
    }

    Line::from(spans)
}

/// Calculate the display width of a character.
///
/// Most characters are 1 cell wide, but some (like CJK characters) are 2 cells.
fn unicode_display_width(c: char) -> usize {
    // Simple approximation: CJK characters are 2 cells wide
    // This covers the most common cases without pulling in unicode-width crate
    if is_wide_char(c) {
        2
    } else {
        1
    }
}

/// Check if a character is a wide (2-cell) character.
fn is_wide_char(c: char) -> bool {
    let cp = c as u32;
    // CJK Unified Ideographs and related ranges
    (0x1100..=0x115F).contains(&cp) // Hangul Jamo
        || (0x2E80..=0x9FFF).contains(&cp) // CJK
        || (0xAC00..=0xD7A3).contains(&cp) // Hangul Syllables
        || (0xF900..=0xFAFF).contains(&cp) // CJK Compatibility Ideographs
        || (0xFE10..=0xFE1F).contains(&cp) // Vertical Forms
        || (0xFE30..=0xFE6F).contains(&cp) // CJK Compatibility Forms
        || (0xFF00..=0xFF60).contains(&cp) // Fullwidth Forms
        || (0xFFE0..=0xFFE6).contains(&cp) // Fullwidth Forms
        || (0x20000..=0x2FFFD).contains(&cp) // CJK Extension B+
        || (0x30000..=0x3FFFD).contains(&cp) // CJK Extension G+
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Style};

    #[test]
    fn test_wrap_empty_text() {
        let text = Text::default();
        let wrapped = wrap_text(&text, 80);
        assert!(wrapped.lines.is_empty() || wrapped.lines.len() == 1);
    }

    #[test]
    fn test_wrap_short_text() {
        let text = Text::raw("Hello");
        let wrapped = wrap_text(&text, 80);
        assert_eq!(wrapped.lines.len(), 1);
    }

    #[test]
    fn test_wrap_at_word_boundary() {
        let text = Text::raw("Hello world this is a test");
        let wrapped = wrap_text(&text, 12);
        // Should wrap between words
        assert!(wrapped.lines.len() >= 2);
    }

    #[test]
    fn test_wrap_preserves_style() {
        let line = Line::from(vec![
            Span::styled("Hello ", Style::default().fg(Color::Red)),
            Span::styled("world", Style::default().fg(Color::Blue)),
        ]);
        let text = Text::from(vec![line]);
        let wrapped = wrap_text(&text, 80);

        // Check that styling is preserved
        assert_eq!(wrapped.lines.len(), 1);
        // The spans should still have colors
        let total_spans: usize = wrapped.lines.iter().map(|l| l.spans.len()).sum();
        assert!(total_spans >= 1);
    }

    #[test]
    fn test_wrap_long_word() {
        let text = Text::raw("Supercalifragilisticexpialidocious");
        let wrapped = wrap_text(&text, 10);
        // Long word should be broken at character boundary
        assert!(wrapped.lines.len() >= 2);
    }

    #[test]
    fn test_wrap_multiline_input() {
        let text = Text::raw("Line one\nLine two\nLine three");
        let wrapped = wrap_text(&text, 80);
        assert_eq!(wrapped.lines.len(), 3);
    }

    #[test]
    fn test_wrap_zero_width() {
        let text = Text::raw("Hello");
        let wrapped = wrap_text(&text, 0);
        assert!(wrapped.lines.is_empty());
    }

    #[test]
    fn test_wrap_exact_width() {
        let text = Text::raw("Hello");
        let wrapped = wrap_text(&text, 5);
        assert_eq!(wrapped.lines.len(), 1);
    }

    #[test]
    fn test_unicode_display_width_ascii() {
        assert_eq!(unicode_display_width('a'), 1);
        assert_eq!(unicode_display_width(' '), 1);
        assert_eq!(unicode_display_width('!'), 1);
    }

    #[test]
    fn test_unicode_display_width_cjk() {
        assert_eq!(unicode_display_width('中'), 2);
        assert_eq!(unicode_display_width('日'), 2);
        assert_eq!(unicode_display_width('本'), 2);
    }

    #[test]
    fn test_is_wide_char() {
        assert!(!is_wide_char('a'));
        assert!(!is_wide_char(' '));
        assert!(is_wide_char('中'));
        assert!(is_wide_char('日'));
    }
}
