//! Terminal cell representation
//!
//! Each cell in the terminal grid contains a character and its attributes.

use serde::{Deserialize, Serialize};

/// ANSI color representation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Color {
    /// Default foreground/background
    Default,
    /// Named ANSI color (0-15)
    Named(u8),
    /// 256-color palette index
    Indexed(u8),
    /// True color RGB
    Rgb(u8, u8, u8),
}

impl Default for Color {
    fn default() -> Self {
        Color::Default
    }
}

/// Cell display attributes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CellAttributes {
    pub foreground: Color,
    pub background: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub inverse: bool,
    pub hidden: bool,
    pub dim: bool,
    pub blink: bool,
}

/// A single terminal cell
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    /// The character(s) in this cell (may be multiple for wide chars/grapheme clusters)
    pub content: String,
    /// Display width of this cell (1 for normal, 2 for wide chars, 0 for continuation)
    pub width: u8,
    /// Cell attributes
    pub attrs: CellAttributes,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            content: String::from(" "),
            width: 1,
            attrs: CellAttributes::default(),
        }
    }
}

impl Cell {
    pub fn new(c: char) -> Self {
        use unicode_width::UnicodeWidthChar;
        let width = c.width().unwrap_or(1) as u8;
        Self {
            content: c.to_string(),
            width,
            attrs: CellAttributes::default(),
        }
    }

    pub fn with_attrs(mut self, attrs: CellAttributes) -> Self {
        self.attrs = attrs;
        self
    }

    pub fn is_empty(&self) -> bool {
        self.content == " " || self.content.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Color tests
    #[test]
    fn test_color_default() {
        let color = Color::default();
        assert_eq!(color, Color::Default);
    }

    #[test]
    fn test_color_named() {
        let red = Color::Named(1);
        let green = Color::Named(2);
        assert_ne!(red, green);
        assert_eq!(red, Color::Named(1));
    }

    #[test]
    fn test_color_indexed() {
        let color = Color::Indexed(128);
        assert_eq!(color, Color::Indexed(128));
    }

    #[test]
    fn test_color_rgb() {
        let color = Color::Rgb(255, 128, 64);
        if let Color::Rgb(r, g, b) = color {
            assert_eq!(r, 255);
            assert_eq!(g, 128);
            assert_eq!(b, 64);
        } else {
            panic!("Expected Rgb color");
        }
    }

    // CellAttributes tests
    #[test]
    fn test_cell_attributes_default() {
        let attrs = CellAttributes::default();
        assert_eq!(attrs.foreground, Color::Default);
        assert_eq!(attrs.background, Color::Default);
        assert!(!attrs.bold);
        assert!(!attrs.italic);
        assert!(!attrs.underline);
        assert!(!attrs.strikethrough);
        assert!(!attrs.inverse);
        assert!(!attrs.hidden);
        assert!(!attrs.dim);
        assert!(!attrs.blink);
    }

    #[test]
    fn test_cell_attributes_with_styles() {
        let attrs = CellAttributes {
            foreground: Color::Named(1),
            background: Color::Named(0),
            bold: true,
            italic: true,
            underline: true,
            strikethrough: false,
            inverse: false,
            hidden: false,
            dim: false,
            blink: false,
        };
        assert_eq!(attrs.foreground, Color::Named(1));
        assert!(attrs.bold);
        assert!(attrs.italic);
        assert!(attrs.underline);
    }

    // Cell tests
    #[test]
    fn test_cell_default() {
        let cell = Cell::default();
        assert_eq!(cell.content, " ");
        assert_eq!(cell.width, 1);
        assert_eq!(cell.attrs, CellAttributes::default());
    }

    #[test]
    fn test_cell_new_ascii() {
        let cell = Cell::new('A');
        assert_eq!(cell.content, "A");
        assert_eq!(cell.width, 1);
    }

    #[test]
    fn test_cell_new_wide_char() {
        // Chinese character (width 2)
        let cell = Cell::new('中');
        assert_eq!(cell.content, "中");
        assert_eq!(cell.width, 2);
    }

    #[test]
    fn test_cell_new_emoji() {
        // Some emojis are wide characters
        let cell = Cell::new('😀');
        assert_eq!(cell.content, "😀");
        // Emoji width can vary, but typically 2
        assert!(cell.width >= 1);
    }

    #[test]
    fn test_cell_with_attrs() {
        let attrs = CellAttributes {
            foreground: Color::Named(2),
            background: Color::Default,
            bold: true,
            ..Default::default()
        };
        let cell = Cell::new('X').with_attrs(attrs);
        assert_eq!(cell.content, "X");
        assert_eq!(cell.attrs.foreground, Color::Named(2));
        assert!(cell.attrs.bold);
    }

    #[test]
    fn test_cell_is_empty_space() {
        let cell = Cell::default();
        assert!(cell.is_empty());
    }

    #[test]
    fn test_cell_is_empty_empty_string() {
        let cell = Cell {
            content: String::new(),
            width: 1,
            attrs: CellAttributes::default(),
        };
        assert!(cell.is_empty());
    }

    #[test]
    fn test_cell_not_empty() {
        let cell = Cell::new('A');
        assert!(!cell.is_empty());
    }

    #[test]
    fn test_cell_equality() {
        let cell1 = Cell::new('A');
        let cell2 = Cell::new('A');
        let cell3 = Cell::new('B');
        assert_eq!(cell1, cell2);
        assert_ne!(cell1, cell3);
    }

    #[test]
    fn test_cell_clone() {
        let original = Cell::new('Z').with_attrs(CellAttributes {
            bold: true,
            ..Default::default()
        });
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }
}
