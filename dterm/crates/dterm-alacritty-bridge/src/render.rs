//! Rendering integration types for Alacritty-style frontends.
//!
//! These types bridge dterm-core cell/grid data to renderer-friendly formats
//! that match Alacritty's rendering API expectations.

use crate::grid::{CellCoord, CellExtras, Grid, GridIteratorExt};
use crate::index::{Column, Line, Point};
use std::sync::Arc;

pub use dterm_core::grid::{CellFlags, PackedColor, PackedColors};
pub use dterm_core::terminal::{CursorStyle, Rgb};

/// A cell prepared for rendering with resolved colors and flags.
///
/// This is the bridge equivalent of Alacritty's `RenderableCell`.
/// It contains all information needed to render a single cell.
#[derive(Debug, Clone)]
pub struct RenderableCell {
    /// Position in the grid.
    pub point: Point,
    /// The character to render.
    pub character: char,
    /// Foreground color (resolved RGB).
    pub fg: Rgb,
    /// Background color (resolved RGB).
    pub bg: Rgb,
    /// Cell flags (bold, italic, etc.).
    pub flags: CellFlags,
    /// Hyperlink URL (OSC 8), if any.
    pub hyperlink: Option<Arc<str>>,
    /// Whether this is a wide character.
    pub is_wide: bool,
    /// Whether this is the continuation of a wide character.
    pub is_wide_continuation: bool,
}

impl RenderableCell {
    /// Check if the cell is empty (space with default styling).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        (self.character == ' ' || self.character == '\0')
            && !self.has_underline()
            && !self.flags.contains(CellFlags::STRIKETHROUGH)
    }

    /// Check if this cell should render as bold.
    #[must_use]
    pub fn is_bold(&self) -> bool {
        self.flags.contains(CellFlags::BOLD)
    }

    /// Check if this cell should render as italic.
    #[must_use]
    pub fn is_italic(&self) -> bool {
        self.flags.contains(CellFlags::ITALIC)
    }

    /// Check if this cell has underline.
    #[must_use]
    pub fn has_underline(&self) -> bool {
        self.flags.contains(CellFlags::UNDERLINE)
            || self.flags.contains(CellFlags::DOUBLE_UNDERLINE)
            || self.flags.contains(CellFlags::CURLY_UNDERLINE)
    }

    /// Check if this cell has strikethrough.
    #[must_use]
    pub fn is_strikethrough(&self) -> bool {
        self.flags.contains(CellFlags::STRIKETHROUGH)
    }
}

/// Cursor rendering information.
#[derive(Debug, Clone, Copy)]
pub struct RenderableCursor {
    /// Cursor position.
    pub point: Point,
    /// Cursor style/shape.
    pub style: CursorStyle,
    /// Whether cursor is visible.
    pub visible: bool,
    /// Whether cursor is blinking.
    pub blinking: bool,
}

/// Color type for rendering (matches Alacritty's color system).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderColor {
    /// Default foreground/background.
    Default,
    /// Indexed color (0-255).
    Indexed(u8),
    /// RGB color.
    Rgb(Rgb),
}

impl RenderColor {
    /// Resolve this color to RGB using the given palette.
    #[must_use]
    pub fn to_rgb(self, palette: &dterm_core::terminal::ColorPalette, default: Rgb) -> Rgb {
        match self {
            RenderColor::Default => default,
            RenderColor::Indexed(idx) => palette.get(idx),
            RenderColor::Rgb(rgb) => rgb,
        }
    }
}

/// Content ready for rendering.
///
/// This aggregates all renderable content including cells, cursor, and
/// selection highlighting.
pub struct RenderableContent<'a> {
    /// Reference to the grid.
    grid: &'a Grid,
    /// Reference to cell extras for RGB color lookup.
    extras: &'a CellExtras,
    /// Cursor information.
    pub cursor: RenderableCursor,
    /// Color palette for resolving indexed colors.
    palette: &'a dterm_core::terminal::ColorPalette,
    /// Default foreground color.
    default_fg: Rgb,
    /// Default background color.
    default_bg: Rgb,
    /// Selection range (if any).
    selection: Option<crate::selection::SelectionRange>,
    /// Display offset.
    display_offset: usize,
}

impl<'a> RenderableContent<'a> {
    /// Create renderable content from a Term.
    pub fn new<T>(term: &'a crate::term::Term<T>) -> Self {
        let grid = term.terminal().grid();
        let extras = grid.extras();
        let cursor_pos = grid.cursor();
        let modes = term.terminal().modes();

        let cursor = RenderableCursor {
            point: Point::new(Line(cursor_pos.row as i32), Column(cursor_pos.col as usize)),
            style: modes.cursor_style,
            visible: modes.cursor_visible,
            blinking: matches!(
                modes.cursor_style,
                CursorStyle::BlinkingBlock
                    | CursorStyle::BlinkingUnderline
                    | CursorStyle::BlinkingBar
            ),
        };

        Self {
            grid,
            extras,
            cursor,
            palette: term.terminal().color_palette(),
            default_fg: term.terminal().default_foreground(),
            default_bg: term.terminal().default_background(),
            selection: term.selection_range(),
            display_offset: grid.display_offset(),
        }
    }

    /// Get the cursor information.
    #[must_use]
    pub fn cursor(&self) -> &RenderableCursor {
        &self.cursor
    }

    /// Check if a point is within the current selection.
    #[must_use]
    pub fn is_selected(&self, point: Point) -> bool {
        self.selection
            .as_ref()
            .map(|s| s.contains(point))
            .unwrap_or(false)
    }

    /// Get the display offset (scrollback position).
    #[must_use]
    pub fn display_offset(&self) -> usize {
        self.display_offset
    }

    /// Iterate over renderable cells in the visible area.
    pub fn iter_cells(&self) -> impl Iterator<Item = RenderableCell> + '_ {
        RenderableCellIterator {
            inner: self.grid.display_iter(),
            extras: self.extras,
            palette: self.palette,
            default_fg: self.default_fg,
            default_bg: self.default_bg,
            selection: self.selection,
        }
    }
}

/// Iterator over renderable cells.
struct RenderableCellIterator<'a> {
    inner: crate::grid::GridIterator<'a>,
    extras: &'a CellExtras,
    palette: &'a dterm_core::terminal::ColorPalette,
    default_fg: Rgb,
    default_bg: Rgb,
    selection: Option<crate::selection::SelectionRange>,
}

impl<'a> Iterator for RenderableCellIterator<'a> {
    type Item = RenderableCell;

    fn next(&mut self) -> Option<Self::Item> {
        let indexed = self.inner.next()?;
        let cell = indexed.cell;
        let point = indexed.point;

        // Only look up extras for visible area cells (line >= 0)
        let extras = if point.line.0 >= 0 {
            let coord = CellCoord::new(point.line.0 as u16, point.column.0 as u16);
            self.extras.get(coord)
        } else {
            None
        };

        // Resolve colors from the cell
        let (fg, bg) = resolve_cell_colors(
            cell,
            extras,
            self.palette,
            self.default_fg,
            self.default_bg,
            self.selection
                .as_ref()
                .map(|s| s.contains(point))
                .unwrap_or(false),
        );

        // Get character from cell
        let character = cell.char();

        // Get flags
        let flags = cell.flags();

        let hyperlink = extras.and_then(|extra| extra.hyperlink()).cloned();

        // Check for wide character
        let is_wide = flags.contains(CellFlags::WIDE);
        let is_wide_continuation = flags.contains(CellFlags::WIDE_CONTINUATION);

        Some(RenderableCell {
            point,
            character,
            fg,
            bg,
            flags,
            hyperlink,
            is_wide,
            is_wide_continuation,
        })
    }
}

/// Resolve cell colors to RGB values.
///
/// This function handles all color modes:
/// - Default colors
/// - Indexed colors (0-255, resolved via palette)
/// - True RGB colors (looked up from CellExtras overflow)
fn resolve_cell_colors(
    cell: &dterm_core::grid::Cell,
    extras: Option<&dterm_core::grid::CellExtra>,
    palette: &dterm_core::terminal::ColorPalette,
    default_fg: Rgb,
    default_bg: Rgb,
    is_selected: bool,
) -> (Rgb, Rgb) {
    let colors = cell.colors();
    let flags = cell.flags();

    // Resolve foreground
    let mut fg = if colors.fg_is_default() {
        default_fg
    } else if colors.fg_is_indexed() {
        palette.get(colors.fg_index())
    } else {
        // RGB - look up from extras overflow table
        extras
            .and_then(|e| e.fg_rgb())
            .map(|[r, g, b]| Rgb { r, g, b })
            .unwrap_or(default_fg)
    };

    // Resolve background
    let mut bg = if colors.bg_is_default() {
        default_bg
    } else if colors.bg_is_indexed() {
        palette.get(colors.bg_index())
    } else {
        // RGB - look up from extras overflow table
        extras
            .and_then(|e| e.bg_rgb())
            .map(|[r, g, b]| Rgb { r, g, b })
            .unwrap_or(default_bg)
    };

    // Handle inverse/reverse video
    if flags.contains(CellFlags::INVERSE) {
        std::mem::swap(&mut fg, &mut bg);
    }

    // Handle selection (invert colors for selected cells)
    if is_selected {
        std::mem::swap(&mut fg, &mut bg);
    }

    // Handle dim/faint attribute
    if flags.contains(CellFlags::DIM) {
        fg = Rgb {
            r: fg.r / 2,
            g: fg.g / 2,
            b: fg.b / 2,
        };
    }

    // Handle invisible attribute
    if flags.contains(CellFlags::HIDDEN) {
        fg = bg;
    }

    (fg, bg)
}

/// Get all renderable cells as a vector.
///
/// This is a convenience function for renderers that need all cells at once.
pub fn get_renderable_cells<T>(term: &crate::term::Term<T>) -> Vec<RenderableCell> {
    RenderableContent::new(term).iter_cells().collect()
}
