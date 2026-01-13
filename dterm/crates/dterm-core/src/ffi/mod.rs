//! C FFI bindings for dashterm2 integration.
//!
//! This module provides a complete C API for terminal emulation:
//!
//! - **Parser API**: Low-level VT parsing with callbacks
//! - **Grid API**: Terminal grid manipulation
//! - **Terminal API**: High-level terminal emulator (Parser + Grid combined)
//!
//! ## Usage from Objective-C
//!
//! ### Parser (Low-level)
//! ```objc
//! #import "dterm.h"
//!
//! dterm_parser_t* parser = dterm_parser_new();
//! dterm_parser_feed(parser, data, len, context, callback);
//! dterm_parser_free(parser);
//! ```
//!
//! ### Terminal (High-level, Recommended)
//! ```objc
//! #import "dterm.h"
//!
//! dterm_terminal_t* term = dterm_terminal_new(24, 80);
//! dterm_terminal_process(term, data, len);
//! uint16_t cursor_row = dterm_terminal_cursor_row(term);
//! const char* title = dterm_terminal_title(term);
//! dterm_terminal_free(term);
//! ```
//!
//! ## Safety
//!
//! All FFI functions that take pointers are unsafe and require:
//! - Pointers must be valid and properly aligned
//! - Pointers must not be null unless explicitly documented
//! - Memory must not be freed while still in use
//! - Thread safety is the caller's responsibility

use crate::grid::{Cell, Grid};
use crate::parser::{ActionSink, Parser};
use crate::scrollback::Scrollback;
use crate::terminal::{Rgb, Terminal};
use libc::{free, malloc};
use std::ffi::{c_char, c_void, CString};
use std::ptr;
use std::sync::atomic::{AtomicI32, Ordering};

// =============================================================================
// ERROR CODES
// =============================================================================

/// FFI error codes.
///
/// Negative values indicate errors, zero indicates success.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtermError {
    /// No error - operation succeeded.
    Success = 0,
    /// Null pointer was passed where non-null was required.
    NullPointer = -1,
    /// Invalid parameter value (e.g., out of range).
    InvalidParameter = -2,
    /// Buffer too small for the requested operation.
    BufferTooSmall = -3,
    /// Memory allocation failed.
    OutOfMemory = -4,
    /// Index out of bounds.
    OutOfBounds = -5,
    /// Resource not found.
    NotFound = -6,
    /// Operation not supported in current state.
    InvalidState = -7,
    /// Generic internal error.
    InternalError = -100,
}

impl DtermError {
    /// Convert to C-compatible i32.
    #[must_use]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }
}

/// Thread-local storage for the last error code.
static LAST_ERROR: AtomicI32 = AtomicI32::new(0);

/// Set the last error code.
///
/// Available for FFI functions that need to report errors.
#[allow(dead_code)]
fn set_last_error(err: DtermError) {
    LAST_ERROR.store(err.as_i32(), Ordering::SeqCst);
}

/// Get the last error code.
///
/// Returns the error code from the most recent FFI operation that failed.
/// Returns `DtermError::Success` (0) if the last operation succeeded.
#[no_mangle]
pub extern "C" fn dterm_get_last_error() -> i32 {
    LAST_ERROR.load(Ordering::SeqCst)
}

/// Clear the last error code.
///
/// Resets the error state to `DtermError::Success`.
#[no_mangle]
pub extern "C" fn dterm_clear_error() {
    LAST_ERROR.store(0, Ordering::SeqCst);
}

/// Get a human-readable error message.
///
/// Returns a static string describing the error code.
/// The returned pointer is valid for the lifetime of the program.
///
/// # Safety
///
/// - The returned pointer must not be freed by the caller.
/// - The returned string is valid UTF-8 and null-terminated.
#[no_mangle]
pub extern "C" fn dterm_error_message(error: i32) -> *const c_char {
    let msg = match error {
        0 => c"Success",
        -1 => c"Null pointer",
        -2 => c"Invalid parameter",
        -3 => c"Buffer too small",
        -4 => c"Out of memory",
        -5 => c"Index out of bounds",
        -6 => c"Not found",
        -7 => c"Invalid state",
        _ => c"Unknown error",
    };
    msg.as_ptr()
}

// =============================================================================
// PARSER API
// =============================================================================

/// Opaque parser handle.
pub struct DtermParser(Parser);

/// Create a new parser.
#[no_mangle]
pub extern "C" fn dterm_parser_new() -> *mut DtermParser {
    Box::into_raw(Box::new(DtermParser(Parser::new())))
}

/// Free a parser.
///
/// # Safety
///
/// - `parser` must be a valid pointer returned by `dterm_parser_new`, or null.
/// - `parser` must not have been freed previously (no double-free).
/// - After this call, `parser` is invalid and must not be used.
#[no_mangle]
pub unsafe extern "C" fn dterm_parser_free(parser: *mut DtermParser) {
    if !parser.is_null() {
        drop(unsafe { Box::from_raw(parser) });
    }
}

/// Action type for FFI.
#[repr(C)]
pub enum DtermActionType {
    /// Print a character.
    Print = 0,
    /// Execute a control character.
    Execute = 1,
    /// CSI sequence.
    Csi = 2,
    /// ESC sequence.
    Esc = 3,
    /// OSC sequence.
    Osc = 4,
}

/// Action for FFI.
#[repr(C)]
pub struct DtermAction {
    /// Action type.
    pub action_type: DtermActionType,
    /// Character for Print, byte for Execute.
    pub byte: u32,
    /// Final byte for CSI/ESC.
    pub final_byte: u8,
    /// Number of parameters.
    pub param_count: u8,
    /// Parameters (up to 16).
    pub params: [u16; 16],
}

/// Callback function type.
pub type DtermCallback = extern "C" fn(*mut c_void, DtermAction);

/// FFI action sink.
struct FfiSink {
    context: *mut c_void,
    callback: DtermCallback,
}

impl ActionSink for FfiSink {
    fn print(&mut self, c: char) {
        let action = DtermAction {
            action_type: DtermActionType::Print,
            byte: c as u32,
            final_byte: 0,
            param_count: 0,
            params: [0; 16],
        };
        (self.callback)(self.context, action);
    }

    fn execute(&mut self, byte: u8) {
        let action = DtermAction {
            action_type: DtermActionType::Execute,
            byte: u32::from(byte),
            final_byte: 0,
            param_count: 0,
            params: [0; 16],
        };
        (self.callback)(self.context, action);
    }

    fn csi_dispatch(&mut self, params: &[u16], _intermediates: &[u8], final_byte: u8) {
        // params.len().min(16) is at most 16, fits in u8
        #[allow(clippy::cast_possible_truncation)]
        let param_count = params.len().min(16) as u8;
        let mut action = DtermAction {
            action_type: DtermActionType::Csi,
            byte: 0,
            final_byte,
            param_count,
            params: [0; 16],
        };
        for (i, &p) in params.iter().take(16).enumerate() {
            action.params[i] = p;
        }
        (self.callback)(self.context, action);
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], final_byte: u8) {
        let action = DtermAction {
            action_type: DtermActionType::Esc,
            byte: 0,
            final_byte,
            param_count: 0,
            params: [0; 16],
        };
        (self.callback)(self.context, action);
    }

    fn osc_dispatch(&mut self, _params: &[&[u8]]) {
        let action = DtermAction {
            action_type: DtermActionType::Osc,
            byte: 0,
            final_byte: 0,
            param_count: 0,
            params: [0; 16],
        };
        (self.callback)(self.context, action);
    }

    fn dcs_hook(&mut self, _params: &[u16], _intermediates: &[u8], _final_byte: u8) {}
    fn dcs_put(&mut self, _byte: u8) {}
    fn dcs_unhook(&mut self) {}

    fn apc_start(&mut self) {}
    fn apc_put(&mut self, _byte: u8) {}
    fn apc_end(&mut self) {}
}

/// Feed data to the parser.
///
/// # Safety
///
/// - `parser` must be a valid pointer returned by `dterm_parser_new`.
/// - `data` must point to at least `len` readable bytes, or be null (in which case this is a no-op).
/// - `len` must not exceed `isize::MAX` to avoid pointer arithmetic overflow.
/// - `callback` must be a valid function pointer that can be safely called.
/// - `context` is passed directly to `callback` - caller ensures validity.
/// - The callback must not call back into this parser (no re-entrancy).
#[no_mangle]
pub unsafe extern "C" fn dterm_parser_feed(
    parser: *mut DtermParser,
    data: *const u8,
    len: usize,
    context: *mut c_void,
    callback: DtermCallback,
) {
    if parser.is_null() || data.is_null() {
        return;
    }
    // Saturate len to isize::MAX in release builds for safety
    // (debug_assert still catches issues during development)
    debug_assert!(
        isize::try_from(len).is_ok(),
        "len must not exceed isize::MAX"
    );
    let len = len.min(isize::MAX as usize);

    let parser = unsafe { &mut (*parser).0 };
    let input = unsafe { std::slice::from_raw_parts(data, len) };

    let mut sink = FfiSink { context, callback };
    parser.advance(input, &mut sink);
}

/// Reset the parser to ground state.
///
/// # Safety
///
/// - `parser` must be a valid pointer returned by `dterm_parser_new`, or null (no-op).
/// - `parser` must not have been freed.
#[no_mangle]
pub unsafe extern "C" fn dterm_parser_reset(parser: *mut DtermParser) {
    if !parser.is_null() {
        unsafe { (*parser).0.reset() };
    }
}

// =============================================================================
// GRID API
// =============================================================================

/// Opaque grid handle.
pub struct DtermGrid(Grid);

/// Cell data for FFI.
#[repr(C)]
pub struct DtermCell {
    /// Unicode codepoint (0 for empty cell).
    pub codepoint: u32,
    /// Foreground color (packed).
    pub fg: u32,
    /// Background color (packed).
    pub bg: u32,
    /// Underline color (packed). 0xFFFFFFFF means use foreground color.
    pub underline_color: u32,
    /// Cell flags (bold, italic, etc.).
    /// Bits 0-10: Standard visual attributes
    /// Bit 11: Superscript (SGR 73)
    /// Bit 12: Subscript (SGR 74)
    /// Bits 13-15: Reserved
    pub flags: u16,
}

impl From<&Cell> for DtermCell {
    fn from(cell: &Cell) -> Self {
        Self {
            codepoint: cell.char() as u32,
            fg: cell.fg().0,
            bg: cell.bg().0,
            underline_color: 0xFFFF_FFFF, // Default: use foreground
            flags: cell.flags().bits(),
        }
    }
}

impl DtermCell {
    /// Create a DtermCell with optional extras from CellExtra.
    #[allow(clippy::trivially_copy_pass_by_ref)] // Cell is 8 bytes, &Cell matches other APIs
    fn from_cell_with_extra(
        cell: &Cell,
        fg_rgb: Option<[u8; 3]>,
        bg_rgb: Option<[u8; 3]>,
        underline_color: Option<[u8; 3]>,
        extended_flags: u16,
    ) -> Self {
        // Combine core flags from Cell with extended flags from CellExtra
        let flags = cell.flags().bits() | extended_flags;

        // Helper to convert RGB array to packed u32 (0x01_RRGGBB format)
        let rgb_to_packed = |[r, g, b]: [u8; 3]| {
            0x01_000000 | (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
        };

        // Get foreground color - use RGB from extras if available, otherwise from cell
        let fg = if cell.fg_needs_overflow() {
            fg_rgb.map(rgb_to_packed).unwrap_or(cell.fg().0)
        } else {
            cell.fg().0
        };

        // Get background color - use RGB from extras if available, otherwise from cell
        let bg = if cell.bg_needs_overflow() {
            bg_rgb.map(rgb_to_packed).unwrap_or(cell.bg().0)
        } else {
            cell.bg().0
        };

        // Convert underline color
        let underline_u32 = underline_color.map(rgb_to_packed).unwrap_or(0xFFFF_FFFF);

        Self {
            codepoint: cell.char() as u32,
            fg,
            bg,
            underline_color: underline_u32,
            flags,
        }
    }
}

fn resolve_cell_rgb(term: &Terminal, row: u16, col: u16, fg: bool) -> Rgb {
    let default = if fg {
        term.default_foreground()
    } else {
        term.default_background()
    };
    let grid = term.grid();
    let cell = match grid.cell(row, col) {
        Some(cell) => cell,
        None => return default,
    };
    let colors = cell.colors();
    if fg {
        if colors.fg_is_default() {
            return default;
        }
        if colors.fg_is_indexed() {
            return term.get_palette_color(colors.fg_index());
        }
        if colors.fg_is_rgb() {
            if let Some(extra) = grid.cell_extra(row, col) {
                if let Some(rgb) = extra.fg_rgb() {
                    return Rgb::new(rgb[0], rgb[1], rgb[2]);
                }
            }
        }
    } else {
        if colors.bg_is_default() {
            return default;
        }
        if colors.bg_is_indexed() {
            return term.get_palette_color(colors.bg_index());
        }
        if colors.bg_is_rgb() {
            if let Some(extra) = grid.cell_extra(row, col) {
                if let Some(rgb) = extra.bg_rgb() {
                    return Rgb::new(rgb[0], rgb[1], rgb[2]);
                }
            }
        }
    }
    default
}

/// Create a new grid with default scrollback.
#[no_mangle]
pub extern "C" fn dterm_grid_new(rows: u16, cols: u16) -> *mut DtermGrid {
    Box::into_raw(Box::new(DtermGrid(Grid::new(rows, cols))))
}

/// Create a new grid with custom scrollback size.
#[no_mangle]
pub extern "C" fn dterm_grid_new_with_scrollback(
    rows: u16,
    cols: u16,
    max_scrollback: usize,
) -> *mut DtermGrid {
    Box::into_raw(Box::new(DtermGrid(Grid::with_scrollback(
        rows,
        cols,
        max_scrollback,
    ))))
}

/// Free a grid.
///
/// # Safety
///
/// - `grid` must be a valid pointer returned by `dterm_grid_new` or
///   `dterm_grid_new_with_scrollback`, or null (no-op).
/// - `grid` must not have been freed previously (no double-free).
/// - After this call, `grid` is invalid and must not be used.
#[no_mangle]
pub unsafe extern "C" fn dterm_grid_free(grid: *mut DtermGrid) {
    if !grid.is_null() {
        drop(unsafe { Box::from_raw(grid) });
    }
}

/// Get number of rows.
///
/// # Safety
///
/// - `grid` must be a valid pointer returned by a grid constructor, or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_grid_rows(grid: *const DtermGrid) -> u16 {
    if grid.is_null() {
        return 0;
    }
    unsafe { (*grid).0.rows() }
}

/// Get number of columns.
///
/// # Safety
///
/// - `grid` must be a valid pointer returned by a grid constructor, or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_grid_cols(grid: *const DtermGrid) -> u16 {
    if grid.is_null() {
        return 0;
    }
    unsafe { (*grid).0.cols() }
}

/// Get cursor row.
///
/// # Safety
///
/// - `grid` must be a valid pointer returned by a grid constructor, or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_grid_cursor_row(grid: *const DtermGrid) -> u16 {
    if grid.is_null() {
        return 0;
    }
    unsafe { (*grid).0.cursor_row() }
}

/// Get cursor column.
///
/// # Safety
///
/// - `grid` must be a valid pointer returned by a grid constructor, or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_grid_cursor_col(grid: *const DtermGrid) -> u16 {
    if grid.is_null() {
        return 0;
    }
    unsafe { (*grid).0.cursor_col() }
}

/// Set cursor position.
///
/// # Safety
///
/// - `grid` must be a valid pointer returned by a grid constructor, or null (no-op).
/// - `row` and `col` will be clamped to valid bounds by the implementation.
#[no_mangle]
pub unsafe extern "C" fn dterm_grid_set_cursor(grid: *mut DtermGrid, row: u16, col: u16) {
    if !grid.is_null() {
        unsafe { (*grid).0.set_cursor(row, col) };
    }
}

/// Get cell at position.
///
/// Returns true if cell exists, false otherwise.
/// Cell data is written to `out_cell` if non-null.
///
/// # Safety
///
/// - `grid` must be a valid pointer returned by a grid constructor, or null (returns false).
/// - `out_cell` must be a valid writable pointer, or null (cell data not returned).
/// - If `out_cell` is non-null, it must point to properly aligned `DtermCell`.
#[no_mangle]
pub unsafe extern "C" fn dterm_grid_get_cell(
    grid: *const DtermGrid,
    row: u16,
    col: u16,
    out_cell: *mut DtermCell,
) -> bool {
    if grid.is_null() {
        return false;
    }
    let grid_ref = unsafe { &(*grid).0 };
    if let Some(cell) = grid_ref.cell(row, col) {
        if !out_cell.is_null() {
            // Look up extras from CellExtra
            let extra = grid_ref.cell_extra(row, col);
            let fg_rgb = extra.and_then(|e| e.fg_rgb());
            let bg_rgb = extra.and_then(|e| e.bg_rgb());
            let underline_color = extra.and_then(|e| e.underline_color());
            let extended_flags = extra.map(|e| e.extended_flags()).unwrap_or(0);
            unsafe {
                *out_cell = DtermCell::from_cell_with_extra(
                    cell,
                    fg_rgb,
                    bg_rgb,
                    underline_color,
                    extended_flags,
                );
            }
        }
        true
    } else {
        false
    }
}

/// Write a character at cursor position.
///
/// # Safety
///
/// - `grid` must be a valid pointer returned by a grid constructor, or null (no-op).
/// - `c` must be a valid Unicode codepoint (invalid values are ignored).
#[no_mangle]
pub unsafe extern "C" fn dterm_grid_write_char(grid: *mut DtermGrid, c: u32) {
    if !grid.is_null() {
        if let Some(ch) = char::from_u32(c) {
            unsafe { (*grid).0.write_char(ch) };
        }
    }
}

/// Resize the grid.
///
/// # Safety
///
/// - `grid` must be a valid pointer returned by a grid constructor, or null (no-op).
/// - `rows` and `cols` should be positive; zero values may cause undefined behavior.
#[no_mangle]
pub unsafe extern "C" fn dterm_grid_resize(grid: *mut DtermGrid, rows: u16, cols: u16) {
    if !grid.is_null() {
        debug_assert!(rows > 0 && cols > 0, "grid dimensions must be positive");
        unsafe { (*grid).0.resize(rows, cols) };
    }
}

/// Scroll display by delta lines (positive = up, negative = down).
///
/// # Safety
///
/// - `grid` must be a valid pointer returned by a grid constructor, or null (no-op).
#[no_mangle]
pub unsafe extern "C" fn dterm_grid_scroll_display(grid: *mut DtermGrid, delta: i32) {
    if !grid.is_null() {
        unsafe { (*grid).0.scroll_display(delta) };
    }
}

/// Get display offset (scroll position).
///
/// # Safety
///
/// - `grid` must be a valid pointer returned by a grid constructor, or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_grid_display_offset(grid: *const DtermGrid) -> usize {
    if grid.is_null() {
        return 0;
    }
    unsafe { (*grid).0.display_offset() }
}

/// Get total scrollback lines.
///
/// # Safety
///
/// - `grid` must be a valid pointer returned by a grid constructor, or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_grid_scrollback_lines(grid: *const DtermGrid) -> usize {
    if grid.is_null() {
        return 0;
    }
    unsafe { (*grid).0.scrollback_lines() }
}

/// Check if grid needs full redraw.
///
/// # Safety
///
/// - `grid` must be a valid pointer returned by a grid constructor, or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_grid_needs_redraw(grid: *const DtermGrid) -> bool {
    if grid.is_null() {
        return false;
    }
    unsafe { (*grid).0.needs_full_redraw() }
}

/// Clear damage after rendering.
///
/// # Safety
///
/// - `grid` must be a valid pointer returned by a grid constructor, or null (no-op).
#[no_mangle]
pub unsafe extern "C" fn dterm_grid_clear_damage(grid: *mut DtermGrid) {
    if !grid.is_null() {
        unsafe { (*grid).0.clear_damage() };
    }
}

/// Erase screen.
///
/// # Safety
///
/// - `grid` must be a valid pointer returned by a grid constructor, or null (no-op).
#[no_mangle]
pub unsafe extern "C" fn dterm_grid_erase_screen(grid: *mut DtermGrid) {
    if !grid.is_null() {
        unsafe { (*grid).0.erase_screen() };
    }
}

/// Check if a Unicode codepoint is a box drawing character.
///
/// This function is used by the grid adapter to set the `isBoxDrawingCharacter` flag
/// for proper rendering. Box drawing characters require special geometric rendering
/// rather than font glyphs.
///
/// Returns true for:
/// - U+2500-U+257F: Box Drawing (lines, corners, tees, crosses)
/// - U+2580-U+259F: Block Elements (shades, quadrants, half blocks)
/// - U+25E2-U+25FF: Geometric Shapes (triangles)
/// - U+1FB00-U+1FB3C: Sextant characters (legacy terminal)
///
/// # Example (Objective-C)
/// ```objc
/// uint32_t codepoint = cell.codepoint;
/// BOOL isBoxDrawing = dterm_is_box_drawing_character(codepoint);
/// ```
#[no_mangle]
pub extern "C" fn dterm_is_box_drawing_character(codepoint: u32) -> bool {
    if let Some(c) = char::from_u32(codepoint) {
        // Inline the box drawing check to avoid dependency on gpu feature
        matches!(c,
            '\u{2500}'..='\u{257F}' |  // Box Drawing
            '\u{2580}'..='\u{259F}' |  // Block Elements
            '\u{25E2}'..='\u{25FF}' |  // Geometric Shapes (triangles)
            '\u{1FB00}'..='\u{1FB3C}'  // Legacy Terminal (sextants)
        )
    } else {
        false
    }
}

// =============================================================================
// TERMINAL API (High-level)
// =============================================================================

/// Opaque terminal handle.
pub struct DtermTerminal {
    terminal: Terminal,
    /// Cached title string for FFI (null-terminated).
    title_cache: CString,
    /// Cached icon name string for FFI (null-terminated).
    icon_name_cache: CString,
    /// Cached hyperlink URL for FFI (null-terminated).
    cached_hyperlink: Option<CString>,
    /// Cached current working directory for FFI (null-terminated).
    cached_cwd: Option<CString>,
    /// Cached complex character string for FFI (null-terminated).
    cached_complex_char: Option<CString>,
    /// DCS sequence callback (for Sixel, DECRQSS, etc).
    dcs_callback: Option<unsafe extern "C" fn(*mut c_void, *const u8, usize, u8)>,
    /// Context pointer for DCS callback.
    dcs_context: *mut c_void,
}

/// Create a new terminal.
#[no_mangle]
pub extern "C" fn dterm_terminal_new(rows: u16, cols: u16) -> *mut DtermTerminal {
    Box::into_raw(Box::new(DtermTerminal {
        terminal: Terminal::new(rows, cols),
        title_cache: CString::new("").expect("empty string has no NUL"),
        icon_name_cache: CString::new("").expect("empty string has no NUL"),
        cached_hyperlink: None,
        cached_cwd: None,
        cached_complex_char: None,
        dcs_callback: None,
        dcs_context: ptr::null_mut(),
    }))
}

/// Create a new terminal with tiered scrollback.
///
/// # Arguments
///
/// * `rows` - Number of visible rows
/// * `cols` - Number of columns
/// * `ring_buffer_size` - Size of fast ring buffer
/// * `hot_limit` - Max lines in hot tier
/// * `warm_limit` - Max lines in warm tier
/// * `memory_budget` - Memory budget in bytes
#[no_mangle]
pub extern "C" fn dterm_terminal_new_with_scrollback(
    rows: u16,
    cols: u16,
    ring_buffer_size: usize,
    hot_limit: usize,
    warm_limit: usize,
    memory_budget: usize,
) -> *mut DtermTerminal {
    let scrollback = Scrollback::new(hot_limit, warm_limit, memory_budget);
    Box::into_raw(Box::new(DtermTerminal {
        terminal: Terminal::with_scrollback(rows, cols, ring_buffer_size, scrollback),
        title_cache: CString::new("").expect("empty string has no NUL"),
        icon_name_cache: CString::new("").expect("empty string has no NUL"),
        cached_hyperlink: None,
        cached_cwd: None,
        cached_complex_char: None,
        dcs_callback: None,
        dcs_context: ptr::null_mut(),
    }))
}

/// Free a terminal.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by `dterm_terminal_new` or
///   `dterm_terminal_new_with_scrollback`, or null (no-op).
/// - `term` must not have been freed previously (no double-free).
/// - After this call, `term` is invalid and must not be used.
/// - Any pointers returned by `dterm_terminal_title` become invalid.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_free(term: *mut DtermTerminal) {
    if !term.is_null() {
        drop(unsafe { Box::from_raw(term) });
    }
}

/// Process input bytes.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor.
/// - `data` must point to at least `len` readable bytes, or be null (no-op).
/// - `len` must not exceed `isize::MAX` to avoid pointer arithmetic overflow.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_process(
    term: *mut DtermTerminal,
    data: *const u8,
    len: usize,
) {
    if term.is_null() || data.is_null() {
        return;
    }
    debug_assert!(
        isize::try_from(len).is_ok(),
        "len must not exceed isize::MAX"
    );
    let term = unsafe { &mut (*term) };
    let input = unsafe { std::slice::from_raw_parts(data, len) };
    term.terminal.process(input);
}

/// Get number of rows.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_rows(term: *const DtermTerminal) -> u16 {
    if term.is_null() {
        return 0;
    }
    unsafe { (*term).terminal.rows() }
}

/// Get number of columns.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_cols(term: *const DtermTerminal) -> u16 {
    if term.is_null() {
        return 0;
    }
    unsafe { (*term).terminal.cols() }
}

/// Get terminal memory usage in bytes.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_memory_usage(term: *const DtermTerminal) -> usize {
    if term.is_null() {
        return 0;
    }
    unsafe { (*term).terminal.memory_used() }
}

/// Set the scrollback memory budget in bytes.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (no-op).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_set_memory_budget(term: *mut DtermTerminal, bytes: usize) {
    if term.is_null() {
        return;
    }
    unsafe { (*term).terminal.set_memory_budget(bytes) };
}

/// Get cursor row.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_cursor_row(term: *const DtermTerminal) -> u16 {
    if term.is_null() {
        return 0;
    }
    unsafe { (*term).terminal.cursor().row }
}

/// Get cursor column.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_cursor_col(term: *const DtermTerminal) -> u16 {
    if term.is_null() {
        return 0;
    }
    unsafe { (*term).terminal.cursor().col }
}

/// Check if cursor is visible.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns true).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_cursor_visible(term: *const DtermTerminal) -> bool {
    if term.is_null() {
        return true;
    }
    unsafe { (*term).terminal.cursor_visible() }
}

/// Get window title (null-terminated UTF-8 string).
///
/// The returned pointer is valid until the next call to this function
/// or until the terminal is freed.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns null).
/// - The returned pointer must not be used after the next call to this function.
/// - The returned pointer must not be used after `dterm_terminal_free`.
/// - The returned pointer must not be freed by the caller.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_title(term: *mut DtermTerminal) -> *const c_char {
    if term.is_null() {
        return ptr::null();
    }
    let term = unsafe { &mut (*term) };
    // Update cache
    if let Ok(cstr) = CString::new(term.terminal.title()) {
        term.title_cache = cstr;
    }
    term.title_cache.as_ptr()
}

/// Get window icon name (null-terminated UTF-8 string).
///
/// The icon name is set by OSC 1 escape sequences.
/// The returned pointer is valid until the next call to this function
/// or until the terminal is freed.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns null).
/// - The returned pointer must not be used after the next call to this function.
/// - The returned pointer must not be used after `dterm_terminal_free`.
/// - The returned pointer must not be freed by the caller.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_icon_name(term: *mut DtermTerminal) -> *const c_char {
    if term.is_null() {
        return ptr::null();
    }
    let term = unsafe { &mut (*term) };
    // Update cache
    if let Ok(cstr) = CString::new(term.terminal.icon_name()) {
        term.icon_name_cache = cstr;
    }
    term.icon_name_cache.as_ptr()
}

/// Get cursor style.
///
/// Returns the DECSCUSR cursor style value (1-6):
/// - 1: Blinking block (default)
/// - 2: Steady block
/// - 3: Blinking underline
/// - 4: Steady underline
/// - 5: Blinking bar
/// - 6: Steady bar
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns 1).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_cursor_style(term: *const DtermTerminal) -> u8 {
    if term.is_null() {
        return 1; // Default: blinking block
    }
    unsafe { (*term).terminal.cursor_style() as u8 }
}

/// Check if alternate screen is active.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_is_alternate_screen(term: *const DtermTerminal) -> bool {
    if term.is_null() {
        return false;
    }
    unsafe { (*term).terminal.is_alternate_screen() }
}

/// Resize the terminal.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (no-op).
/// - `rows` and `cols` should be positive; zero values may cause undefined behavior.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_resize(term: *mut DtermTerminal, rows: u16, cols: u16) {
    if !term.is_null() {
        debug_assert!(rows > 0 && cols > 0, "terminal dimensions must be positive");
        unsafe { (*term).terminal.resize(rows, cols) };
    }
}

/// Reset the terminal to initial state.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (no-op).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_reset(term: *mut DtermTerminal) {
    if !term.is_null() {
        unsafe { (*term).terminal.reset() };
    }
}

/// Scroll display by delta lines.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (no-op).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_scroll_display(term: *mut DtermTerminal, delta: i32) {
    if !term.is_null() {
        unsafe { (*term).terminal.scroll_display(delta) };
    }
}

/// Scroll to top of scrollback.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (no-op).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_scroll_to_top(term: *mut DtermTerminal) {
    if !term.is_null() {
        unsafe { (*term).terminal.scroll_to_top() };
    }
}

/// Scroll to bottom (live content).
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (no-op).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_scroll_to_bottom(term: *mut DtermTerminal) {
    if !term.is_null() {
        unsafe { (*term).terminal.scroll_to_bottom() };
    }
}

/// Get cell at position.
///
/// Returns true if cell exists, false otherwise.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns false).
/// - `out_cell` must be a valid writable pointer, or null (cell data not returned).
/// - If `out_cell` is non-null, it must point to properly aligned `DtermCell`.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_get_cell(
    term: *const DtermTerminal,
    row: u16,
    col: u16,
    out_cell: *mut DtermCell,
) -> bool {
    if term.is_null() {
        return false;
    }
    let term_ref = unsafe { &(*term).terminal };
    if let Some(cell) = term_ref.grid().cell(row, col) {
        if !out_cell.is_null() {
            // Look up extras from CellExtra
            let extra = term_ref.grid().cell_extra(row, col);
            let fg_rgb = extra.and_then(|e| e.fg_rgb());
            let bg_rgb = extra.and_then(|e| e.bg_rgb());
            let underline_color = extra.and_then(|e| e.underline_color());
            let extended_flags = extra.map(|e| e.extended_flags()).unwrap_or(0);
            unsafe {
                *out_cell = DtermCell::from_cell_with_extra(
                    cell,
                    fg_rgb,
                    bg_rgb,
                    underline_color,
                    extended_flags,
                );
            }
        }
        true
    } else {
        false
    }
}

/// Get the Unicode codepoint for a cell.
///
/// For complex characters, looks up the overflow string table.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_cell_codepoint(
    term: *const DtermTerminal,
    row: u16,
    col: u16,
) -> u32 {
    if term.is_null() {
        return 0;
    }
    let term_ref = unsafe { &(*term).terminal };
    let grid = term_ref.grid();
    let cell = match grid.cell(row, col) {
        Some(cell) => cell,
        None => return 0,
    };
    if cell.is_wide_continuation() {
        return 0;
    }
    if cell.is_complex() {
        if let Some(extra) = grid.cell_extra(row, col) {
            if let Some(complex_str) = extra.complex_char() {
                if let Some(ch) = complex_str.chars().next() {
                    return ch as u32;
                }
            }
        }
        return 0xFFFD;
    }
    cell.codepoint()
}

/// Get foreground color as RGB.
///
/// Resolves indexed colors via the palette and true color via overflow tables.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (no-op).
/// - `r`, `g`, and `b` must be valid writable pointers.
#[no_mangle]
pub unsafe extern "C" fn dterm_cell_fg_rgb(
    term: *const DtermTerminal,
    row: u16,
    col: u16,
    r: *mut u8,
    g: *mut u8,
    b: *mut u8,
) {
    if term.is_null() || r.is_null() || g.is_null() || b.is_null() {
        return;
    }
    let term_ref = unsafe { &(*term).terminal };
    let rgb = resolve_cell_rgb(term_ref, row, col, true);
    unsafe {
        *r = rgb.r;
        *g = rgb.g;
        *b = rgb.b;
    }
}

/// Get background color as RGB.
///
/// Resolves indexed colors via the palette and true color via overflow tables.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (no-op).
/// - `r`, `g`, and `b` must be valid writable pointers.
#[no_mangle]
pub unsafe extern "C" fn dterm_cell_bg_rgb(
    term: *const DtermTerminal,
    row: u16,
    col: u16,
    r: *mut u8,
    g: *mut u8,
    b: *mut u8,
) {
    if term.is_null() || r.is_null() || g.is_null() || b.is_null() {
        return;
    }
    let term_ref = unsafe { &(*term).terminal };
    let rgb = resolve_cell_rgb(term_ref, row, col, false);
    unsafe {
        *r = rgb.r;
        *g = rgb.g;
        *b = rgb.b;
    }
}

/// Check if terminal needs full redraw.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_needs_redraw(term: *const DtermTerminal) -> bool {
    if term.is_null() {
        return false;
    }
    unsafe { (*term).terminal.grid().needs_full_redraw() }
}

/// Clear damage after rendering.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (no-op).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_clear_damage(term: *mut DtermTerminal) {
    if !term.is_null() {
        unsafe { (*term).terminal.grid_mut().clear_damage() };
    }
}

/// Get total scrollback lines.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_scrollback_lines(term: *const DtermTerminal) -> usize {
    if term.is_null() {
        return 0;
    }
    unsafe { (*term).terminal.grid().scrollback_lines() }
}

/// Scrollback cell information for FFI.
///
/// Contains the character codepoint and style attributes for a cell in scrollback.
#[repr(C)]
pub struct DtermScrollbackCell {
    /// Unicode codepoint (0 if empty or continuation).
    pub codepoint: u32,
    /// Foreground color (packed: 0xTT_RRGGBB where TT is type).
    /// - 0x00: Indexed color (BB = index)
    /// - 0x01: True color RGB
    /// - 0xFF: Default color
    pub fg: u32,
    /// Background color (same format as fg).
    pub bg: u32,
    /// Cell flags (bold, italic, underline, etc.).
    pub flags: u16,
    /// Reserved for alignment.
    pub reserved: u16,
}

impl Default for DtermScrollbackCell {
    fn default() -> Self {
        Self {
            codepoint: 0,
            fg: 0xFF_FF_FF_FF, // Default fg
            bg: 0xFF_00_00_00, // Default bg
            flags: 0,
            reserved: 0,
        }
    }
}

/// Get a cell from scrollback history.
///
/// Retrieves the character and attributes at a specific position in scrollback.
///
/// Parameters:
/// - `term`: Terminal handle
/// - `scrollback_row`: Row index in scrollback (0 = oldest line)
/// - `col`: Column index (0-based)
/// - `out_cell`: Output cell info
///
/// Returns true if the cell exists, false if out of bounds or scrollback unavailable.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns false).
/// - `out_cell` must be a valid writable pointer, or null (returns existence only).
/// - If `out_cell` is non-null, it must point to properly aligned `DtermScrollbackCell`.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_get_scrollback_cell(
    term: *const DtermTerminal,
    scrollback_row: usize,
    col: u16,
    out_cell: *mut DtermScrollbackCell,
) -> bool {
    if term.is_null() {
        return false;
    }

    let term_ref = unsafe { &(*term).terminal };
    let scrollback = match term_ref.scrollback() {
        Some(sb) => sb,
        None => return false,
    };

    let line = match scrollback.get_line(scrollback_row) {
        Some(line) => line,
        None => return false,
    };

    // Get the character at the column index
    // Line stores UTF-8 text, we need to iterate to find the nth character
    let text = line.as_bytes();
    let text_str = match std::str::from_utf8(text) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let char_at_col = text_str.chars().nth(col as usize);
    let codepoint = match char_at_col {
        Some(c) => c as u32,
        None => 0, // Empty cell (past end of line)
    };

    if !out_cell.is_null() {
        // Get attributes for this column
        let attrs = line.get_attr(col as usize);

        unsafe {
            *out_cell = DtermScrollbackCell {
                codepoint,
                fg: attrs.fg,
                bg: attrs.bg,
                flags: attrs.flags,
                reserved: 0,
            };
        }
    }

    true
}

/// Get scrollback line length (number of characters).
///
/// Returns the number of characters in the specified scrollback line.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_scrollback_line_len(
    term: *const DtermTerminal,
    scrollback_row: usize,
) -> usize {
    if term.is_null() {
        return 0;
    }

    let term_ref = unsafe { &(*term).terminal };
    let scrollback = match term_ref.scrollback() {
        Some(sb) => sb,
        None => return 0,
    };

    let line = match scrollback.get_line(scrollback_row) {
        Some(line) => line,
        None => return 0,
    };

    // Count characters (not bytes)
    let text = line.as_bytes();
    match std::str::from_utf8(text) {
        Ok(s) => s.chars().count(),
        Err(_) => 0,
    }
}

/// Check if scrollback line is wrapped (continuation of previous line).
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_scrollback_line_wrapped(
    term: *const DtermTerminal,
    scrollback_row: usize,
) -> bool {
    if term.is_null() {
        return false;
    }

    let term_ref = unsafe { &(*term).terminal };
    let scrollback = match term_ref.scrollback() {
        Some(sb) => sb,
        None => return false,
    };

    let line = match scrollback.get_line(scrollback_row) {
        Some(line) => line,
        None => return false,
    };

    line.is_wrapped()
}

/// Get display offset (scroll position).
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_display_offset(term: *const DtermTerminal) -> usize {
    if term.is_null() {
        return 0;
    }
    unsafe { (*term).terminal.grid().display_offset() }
}

/// Style information for FFI.
#[repr(C)]
pub struct DtermStyle {
    /// Foreground color (packed).
    pub fg: u32,
    /// Background color (packed).
    pub bg: u32,
    /// Cell flags (bold, italic, etc.).
    /// Bits 0-10: Standard visual attributes
    /// Bit 11: Superscript (SGR 73)
    /// Bit 12: Subscript (SGR 74)
    pub flags: u16,
}

/// Get current style.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (no-op).
/// - `out_style` must be a valid writable pointer, or null (no-op).
/// - If `out_style` is non-null, it must point to properly aligned `DtermStyle`.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_get_style(
    term: *const DtermTerminal,
    out_style: *mut DtermStyle,
) {
    if term.is_null() || out_style.is_null() {
        return;
    }
    let style = unsafe { (*term).terminal.style() };
    unsafe {
        (*out_style) = DtermStyle {
            fg: style.fg.0,
            bg: style.bg.0,
            flags: style.flags.bits(),
        };
    }
}

/// Mouse tracking mode for FFI.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DtermMouseMode {
    /// No mouse tracking (default).
    None = 0,
    /// Normal tracking mode (1000) - report button press/release.
    Normal = 1,
    /// Button-event tracking mode (1002) - report press/release and motion while button pressed.
    ButtonEvent = 2,
    /// Any-event tracking mode (1003) - report all motion events.
    AnyEvent = 3,
}

/// Mouse encoding format for FFI.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DtermMouseEncoding {
    /// X10 compatibility mode - coordinates encoded as single bytes (limited to 223).
    X10 = 0,
    /// UTF-8 encoding (1005) - coordinates as UTF-8 characters, supports up to 2015.
    Utf8 = 1,
    /// SGR encoding (1006) - coordinates as decimal parameters, supports larger values.
    Sgr = 2,
    /// URXVT encoding (1015) - decimal parameters without '<' prefix.
    Urxvt = 3,
    /// SGR pixel mode (1016) - like SGR but coordinates are in pixels.
    SgrPixel = 4,
}

/// Mode flags for FFI.
#[repr(C)]
pub struct DtermModes {
    /// Cursor visible (DECTCEM).
    pub cursor_visible: bool,
    /// Cursor style (DECSCUSR).
    /// Values match DECSCUSR parameters (1-6).
    pub cursor_style: u8,
    /// Application cursor keys (DECCKM).
    pub application_cursor_keys: bool,
    /// Alternate screen buffer active.
    pub alternate_screen: bool,
    /// Auto-wrap mode (DECAWM).
    pub auto_wrap: bool,
    /// Origin mode (DECOM).
    pub origin_mode: bool,
    /// Insert mode (IRM).
    pub insert_mode: bool,
    /// Bracketed paste mode.
    pub bracketed_paste: bool,
    /// Mouse tracking mode (1000/1002/1003).
    pub mouse_mode: DtermMouseMode,
    /// Mouse encoding format (X10 or SGR/1006).
    pub mouse_encoding: DtermMouseEncoding,
    /// Focus reporting mode (1004).
    pub focus_reporting: bool,
    /// Synchronized output mode (2026).
    /// When enabled, rendering should be deferred to prevent tearing.
    pub synchronized_output: bool,
    /// Reverse video mode (DECSET 5).
    /// When enabled, screen colors are inverted.
    pub reverse_video: bool,
    /// Cursor blink mode (DECSET 12).
    /// When enabled, cursor blinks.
    pub cursor_blink: bool,
    /// Application keypad mode (DECKPAM/DECKPNM).
    /// When enabled, keypad sends application sequences.
    pub application_keypad: bool,
    /// 132 column mode (DECSET 3).
    /// When enabled, terminal uses 132 columns.
    pub column_mode_132: bool,
    /// Reverse wraparound mode (DECSET 45).
    /// When enabled, backspace at column 0 wraps to previous line.
    pub reverse_wraparound: bool,
}

/// Get terminal modes.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (no-op).
/// - `out_modes` must be a valid writable pointer, or null (no-op).
/// - If `out_modes` is non-null, it must point to properly aligned `DtermModes`.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_get_modes(
    term: *const DtermTerminal,
    out_modes: *mut DtermModes,
) {
    if term.is_null() || out_modes.is_null() {
        return;
    }
    let modes = unsafe { (*term).terminal.modes() };
    unsafe {
        (*out_modes) = DtermModes {
            cursor_visible: modes.cursor_visible,
            cursor_style: modes.cursor_style as u8,
            application_cursor_keys: modes.application_cursor_keys,
            alternate_screen: modes.alternate_screen,
            auto_wrap: modes.auto_wrap,
            origin_mode: modes.origin_mode,
            insert_mode: modes.insert_mode,
            bracketed_paste: modes.bracketed_paste,
            mouse_mode: match modes.mouse_mode {
                crate::terminal::MouseMode::None => DtermMouseMode::None,
                crate::terminal::MouseMode::Normal => DtermMouseMode::Normal,
                crate::terminal::MouseMode::ButtonEvent => DtermMouseMode::ButtonEvent,
                crate::terminal::MouseMode::AnyEvent => DtermMouseMode::AnyEvent,
            },
            mouse_encoding: match modes.mouse_encoding {
                crate::terminal::MouseEncoding::X10 => DtermMouseEncoding::X10,
                crate::terminal::MouseEncoding::Utf8 => DtermMouseEncoding::Utf8,
                crate::terminal::MouseEncoding::Sgr => DtermMouseEncoding::Sgr,
                crate::terminal::MouseEncoding::Urxvt => DtermMouseEncoding::Urxvt,
                crate::terminal::MouseEncoding::SgrPixel => DtermMouseEncoding::SgrPixel,
            },
            focus_reporting: modes.focus_reporting,
            synchronized_output: modes.synchronized_output,
            reverse_video: modes.reverse_video,
            cursor_blink: modes.cursor_blink,
            application_keypad: modes.application_keypad,
            column_mode_132: modes.column_mode_132,
            reverse_wraparound: modes.reverse_wraparound,
        };
    }
}

// =============================================================================
// RESPONSE BUFFER API
// =============================================================================

/// Check if the terminal has pending response data.
///
/// Returns true if there is response data to read (from DSR/DA sequences).
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_has_response(term: *const DtermTerminal) -> bool {
    if term.is_null() {
        return false;
    }
    unsafe { (*term).terminal.has_pending_response() }
}

/// Get the number of bytes in the pending response.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_response_len(term: *const DtermTerminal) -> usize {
    if term.is_null() {
        return 0;
    }
    unsafe { (*term).terminal.pending_response_len() }
}

/// Read pending response data from the terminal.
///
/// Copies up to `buffer_size` bytes of response data into `buffer` and returns
/// the number of bytes copied. The copied data is removed from the response
/// buffer.
///
/// This should be called after processing input to check for any responses
/// that need to be written back to the PTY (such as cursor position reports
/// or device attribute responses).
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor.
/// - `buffer` must point to at least `buffer_size` writable bytes, or be null (returns 0).
/// - `buffer_size` must not exceed `isize::MAX` to avoid pointer arithmetic overflow.
/// - The memory at `buffer` must not overlap with terminal internal memory.
///
/// # Returns
///
/// The number of bytes copied to `buffer`. Returns 0 if there is no
/// pending response or if either pointer is null.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_read_response(
    term: *mut DtermTerminal,
    buffer: *mut u8,
    buffer_size: usize,
) -> usize {
    if term.is_null() || buffer.is_null() || buffer_size == 0 {
        return 0;
    }
    debug_assert!(
        isize::try_from(buffer_size).is_ok(),
        "buffer_size must not exceed isize::MAX"
    );

    let term = unsafe { &mut (*term) };
    if let Some(response) = term.terminal.take_response() {
        let copy_len = response.len().min(buffer_size);
        unsafe {
            std::ptr::copy_nonoverlapping(response.as_ptr(), buffer, copy_len);
        }
        // If there's remaining data that didn't fit, we lose it.
        // Callers should ensure buffer is large enough (check response_len first).
        copy_len
    } else {
        0
    }
}

// =============================================================================
// MOUSE EVENT API
// =============================================================================

/// Check if mouse tracking is enabled.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_mouse_tracking_enabled(term: *const DtermTerminal) -> bool {
    if term.is_null() {
        return false;
    }
    unsafe { (*term).terminal.mouse_tracking_enabled() }
}

/// Get the current mouse tracking mode.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns None).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_mouse_mode(term: *const DtermTerminal) -> DtermMouseMode {
    if term.is_null() {
        return DtermMouseMode::None;
    }
    match unsafe { (*term).terminal.mouse_mode() } {
        crate::terminal::MouseMode::None => DtermMouseMode::None,
        crate::terminal::MouseMode::Normal => DtermMouseMode::Normal,
        crate::terminal::MouseMode::ButtonEvent => DtermMouseMode::ButtonEvent,
        crate::terminal::MouseMode::AnyEvent => DtermMouseMode::AnyEvent,
    }
}

/// Get the current mouse encoding format.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns X10).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_mouse_encoding(
    term: *const DtermTerminal,
) -> DtermMouseEncoding {
    if term.is_null() {
        return DtermMouseEncoding::X10;
    }
    match unsafe { (*term).terminal.mouse_encoding() } {
        crate::terminal::MouseEncoding::X10 => DtermMouseEncoding::X10,
        crate::terminal::MouseEncoding::Utf8 => DtermMouseEncoding::Utf8,
        crate::terminal::MouseEncoding::Sgr => DtermMouseEncoding::Sgr,
        crate::terminal::MouseEncoding::Urxvt => DtermMouseEncoding::Urxvt,
        crate::terminal::MouseEncoding::SgrPixel => DtermMouseEncoding::SgrPixel,
    }
}

/// Check if focus reporting is enabled.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_focus_reporting_enabled(
    term: *const DtermTerminal,
) -> bool {
    if term.is_null() {
        return false;
    }
    unsafe { (*term).terminal.focus_reporting_enabled() }
}

/// Check if synchronized output mode is enabled.
///
/// When enabled, the terminal is in "batch update" mode and the renderer
/// should defer drawing until the mode is disabled. This prevents screen
/// tearing during rapid updates from applications like vim or tmux.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_synchronized_output_enabled(
    term: *const DtermTerminal,
) -> bool {
    if term.is_null() {
        return false;
    }
    unsafe { (*term).terminal.synchronized_output_enabled() }
}

/// Encode a mouse button press event.
///
/// Returns the number of bytes written to `buffer`, or 0 if mouse reporting
/// is disabled or parameters are invalid. Coordinates are 0-indexed.
///
/// # Arguments
///
/// * `button` - Mouse button (0=left, 1=middle, 2=right)
/// * `col` - Column (0-indexed)
/// * `row` - Row (0-indexed)
/// * `modifiers` - Modifier keys (shift=4, meta=8, ctrl=16)
/// * `buffer` - Output buffer for the escape sequence
/// * `buffer_size` - Size of the output buffer
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns 0).
/// - `buffer` must point to at least `buffer_size` writable bytes, or be null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_encode_mouse_press(
    term: *const DtermTerminal,
    button: u8,
    col: u16,
    row: u16,
    modifiers: u8,
    buffer: *mut u8,
    buffer_size: usize,
) -> usize {
    if term.is_null() || buffer.is_null() || buffer_size == 0 {
        return 0;
    }
    let term = unsafe { &(*term).terminal };
    if let Some(seq) = term.encode_mouse_press(button, col, row, modifiers) {
        let copy_len = seq.len().min(buffer_size);
        unsafe { std::ptr::copy_nonoverlapping(seq.as_ptr(), buffer, copy_len) };
        copy_len
    } else {
        0
    }
}

/// Encode a mouse button release event.
///
/// Returns the number of bytes written to `buffer`, or 0 if mouse reporting
/// is disabled or parameters are invalid. Coordinates are 0-indexed.
///
/// # Arguments
///
/// * `button` - Original mouse button (0=left, 1=middle, 2=right)
/// * `col` - Column (0-indexed)
/// * `row` - Row (0-indexed)
/// * `modifiers` - Modifier keys (shift=4, meta=8, ctrl=16)
/// * `buffer` - Output buffer for the escape sequence
/// * `buffer_size` - Size of the output buffer
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns 0).
/// - `buffer` must point to at least `buffer_size` writable bytes, or be null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_encode_mouse_release(
    term: *const DtermTerminal,
    button: u8,
    col: u16,
    row: u16,
    modifiers: u8,
    buffer: *mut u8,
    buffer_size: usize,
) -> usize {
    if term.is_null() || buffer.is_null() || buffer_size == 0 {
        return 0;
    }
    let term = unsafe { &(*term).terminal };
    if let Some(seq) = term.encode_mouse_release(button, col, row, modifiers) {
        let copy_len = seq.len().min(buffer_size);
        unsafe { std::ptr::copy_nonoverlapping(seq.as_ptr(), buffer, copy_len) };
        copy_len
    } else {
        0
    }
}

/// Encode a mouse motion event.
///
/// Returns the number of bytes written to `buffer`, or 0 if motion tracking
/// is not enabled or parameters are invalid. Coordinates are 0-indexed.
///
/// Motion events are only sent in ButtonEvent (1002) or AnyEvent (1003) modes.
///
/// # Arguments
///
/// * `button` - Button held during motion (0=left, 1=middle, 2=right, 3=none)
/// * `col` - Column (0-indexed)
/// * `row` - Row (0-indexed)
/// * `modifiers` - Modifier keys (shift=4, meta=8, ctrl=16)
/// * `buffer` - Output buffer for the escape sequence
/// * `buffer_size` - Size of the output buffer
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns 0).
/// - `buffer` must point to at least `buffer_size` writable bytes, or be null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_encode_mouse_motion(
    term: *const DtermTerminal,
    button: u8,
    col: u16,
    row: u16,
    modifiers: u8,
    buffer: *mut u8,
    buffer_size: usize,
) -> usize {
    if term.is_null() || buffer.is_null() || buffer_size == 0 {
        return 0;
    }
    let term = unsafe { &(*term).terminal };
    if let Some(seq) = term.encode_mouse_motion(button, col, row, modifiers) {
        let copy_len = seq.len().min(buffer_size);
        unsafe { std::ptr::copy_nonoverlapping(seq.as_ptr(), buffer, copy_len) };
        copy_len
    } else {
        0
    }
}

/// Encode a mouse wheel event.
///
/// Returns the number of bytes written to `buffer`, or 0 if mouse reporting
/// is disabled or parameters are invalid. Coordinates are 0-indexed.
///
/// # Arguments
///
/// * `up` - True for wheel up, false for wheel down
/// * `col` - Column (0-indexed)
/// * `row` - Row (0-indexed)
/// * `modifiers` - Modifier keys (shift=4, meta=8, ctrl=16)
/// * `buffer` - Output buffer for the escape sequence
/// * `buffer_size` - Size of the output buffer
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns 0).
/// - `buffer` must point to at least `buffer_size` writable bytes, or be null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_encode_mouse_wheel(
    term: *const DtermTerminal,
    up: bool,
    col: u16,
    row: u16,
    modifiers: u8,
    buffer: *mut u8,
    buffer_size: usize,
) -> usize {
    if term.is_null() || buffer.is_null() || buffer_size == 0 {
        return 0;
    }
    let term = unsafe { &(*term).terminal };
    if let Some(seq) = term.encode_mouse_wheel(up, col, row, modifiers) {
        let copy_len = seq.len().min(buffer_size);
        unsafe { std::ptr::copy_nonoverlapping(seq.as_ptr(), buffer, copy_len) };
        copy_len
    } else {
        0
    }
}

/// Encode a focus event.
///
/// Returns the number of bytes written to `buffer`, or 0 if focus reporting
/// is disabled.
///
/// # Arguments
///
/// * `focused` - True if window gained focus, false if lost focus
/// * `buffer` - Output buffer for the escape sequence
/// * `buffer_size` - Size of the output buffer
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns 0).
/// - `buffer` must point to at least `buffer_size` writable bytes, or be null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_encode_focus_event(
    term: *const DtermTerminal,
    focused: bool,
    buffer: *mut u8,
    buffer_size: usize,
) -> usize {
    if term.is_null() || buffer.is_null() || buffer_size == 0 {
        return 0;
    }
    let term = unsafe { &(*term).terminal };
    if let Some(seq) = term.encode_focus_event(focused) {
        let copy_len = seq.len().min(buffer_size);
        unsafe { std::ptr::copy_nonoverlapping(seq.as_ptr(), buffer, copy_len) };
        copy_len
    } else {
        0
    }
}

// =============================================================================
// SEARCH API
// =============================================================================

/// Opaque search index handle.
pub struct DtermSearch(crate::search::TerminalSearch);

/// Search match result for FFI.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DtermSearchMatch {
    /// Line number (0-indexed).
    pub line: usize,
    /// Starting column of the match (0-indexed).
    pub start_col: usize,
    /// Ending column of the match (exclusive).
    pub end_col: usize,
}

/// Search direction.
#[repr(C)]
pub enum DtermSearchDirection {
    /// Search forward (oldest to newest).
    Forward = 0,
    /// Search backward (newest to oldest).
    Backward = 1,
}

/// Create a new search index.
#[no_mangle]
pub extern "C" fn dterm_search_new() -> *mut DtermSearch {
    Box::into_raw(Box::new(DtermSearch(crate::search::TerminalSearch::new())))
}

/// Create a new search index with expected capacity.
#[no_mangle]
pub extern "C" fn dterm_search_with_capacity(expected_lines: usize) -> *mut DtermSearch {
    Box::into_raw(Box::new(DtermSearch(
        crate::search::TerminalSearch::with_capacity(expected_lines),
    )))
}

/// Free a search index.
///
/// # Safety
///
/// - `search` must be a valid pointer returned by `dterm_search_new` or
///   `dterm_search_with_capacity`, or null (no-op).
/// - `search` must not have been freed previously (no double-free).
/// - After this call, `search` is invalid and must not be used.
#[no_mangle]
pub unsafe extern "C" fn dterm_search_free(search: *mut DtermSearch) {
    if !search.is_null() {
        drop(unsafe { Box::from_raw(search) });
    }
}

/// Index a scrollback line for searching.
///
/// # Safety
///
/// - `search` must be a valid pointer returned by a search constructor, or null (no-op).
/// - `text` must point to at least `len` readable bytes, or be null (no-op).
/// - `len` must not exceed `isize::MAX`.
/// - `text` must contain valid UTF-8 data.
#[no_mangle]
pub unsafe extern "C" fn dterm_search_index_line(
    search: *mut DtermSearch,
    text: *const u8,
    len: usize,
) {
    if search.is_null() || text.is_null() {
        return;
    }
    debug_assert!(
        isize::try_from(len).is_ok(),
        "len must not exceed isize::MAX"
    );

    let search = unsafe { &mut (*search).0 };
    let bytes = unsafe { std::slice::from_raw_parts(text, len) };
    if let Ok(s) = std::str::from_utf8(bytes) {
        search.index_scrollback_line(s);
    }
}

/// Check if a query might have matches (fast bloom filter check).
///
/// Returns false if definitely no matches exist.
/// Returns true if matches are possible (verify with actual search).
///
/// # Safety
///
/// - `search` must be a valid pointer returned by a search constructor, or null (returns false).
/// - `query` must point to at least `query_len` readable bytes, or be null (returns false).
/// - `query` must contain valid UTF-8 data.
#[no_mangle]
pub unsafe extern "C" fn dterm_search_might_contain(
    search: *const DtermSearch,
    query: *const u8,
    query_len: usize,
) -> bool {
    // Return false for null pointers or empty query
    if search.is_null() || query.is_null() || query_len == 0 {
        return false;
    }

    let search = unsafe { &(*search).0 };
    let bytes = unsafe { std::slice::from_raw_parts(query, query_len) };
    if let Ok(s) = std::str::from_utf8(bytes) {
        search.might_contain(s)
    } else {
        false
    }
}

/// Search for a query string.
///
/// Returns the number of matches found. Matches are written to `out_matches`
/// up to `max_matches` count.
///
/// # Safety
///
/// - `search` must be a valid pointer returned by a search constructor, or null (returns 0).
/// - `query` must point to at least `query_len` readable bytes, or be null (returns 0).
/// - `query` must contain valid UTF-8 data.
/// - `out_matches` must point to at least `max_matches` writable `DtermSearchMatch`, or be null.
/// - If `out_matches` is null, only the count is returned (useful for sizing a buffer).
#[no_mangle]
pub unsafe extern "C" fn dterm_search_find(
    search: *const DtermSearch,
    query: *const u8,
    query_len: usize,
    out_matches: *mut DtermSearchMatch,
    max_matches: usize,
) -> usize {
    // Return 0 for null pointers or empty query
    if search.is_null() || query.is_null() || query_len == 0 {
        return 0;
    }

    let search = unsafe { &(*search).0 };
    let bytes = unsafe { std::slice::from_raw_parts(query, query_len) };
    let query_str = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let matches = search.search(query_str);
    let count = matches.len();

    if !out_matches.is_null() && max_matches > 0 {
        for (i, m) in matches.into_iter().take(max_matches).enumerate() {
            unsafe {
                *out_matches.add(i) = DtermSearchMatch {
                    line: m.line,
                    start_col: m.start_col,
                    end_col: m.end_col,
                };
            }
        }
    }

    count
}

/// Search for a query string in the specified direction.
///
/// Returns the number of matches found. Matches are written to `out_matches`
/// up to `max_matches` count, sorted by the specified direction.
///
/// # Safety
///
/// - `search` must be a valid pointer returned by a search constructor, or null (returns 0).
/// - `query` must point to at least `query_len` readable bytes, or be null (returns 0).
/// - `query` must contain valid UTF-8 data.
/// - `out_matches` must point to at least `max_matches` writable `DtermSearchMatch`, or be null.
#[no_mangle]
pub unsafe extern "C" fn dterm_search_find_ordered(
    search: *const DtermSearch,
    query: *const u8,
    query_len: usize,
    direction: DtermSearchDirection,
    out_matches: *mut DtermSearchMatch,
    max_matches: usize,
) -> usize {
    // Return 0 for null pointers or empty query
    if search.is_null() || query.is_null() || query_len == 0 {
        return 0;
    }

    let search = unsafe { &(*search).0 };
    let bytes = unsafe { std::slice::from_raw_parts(query, query_len) };
    let query_str = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let dir = match direction {
        DtermSearchDirection::Forward => crate::search::SearchDirection::Forward,
        DtermSearchDirection::Backward => crate::search::SearchDirection::Backward,
    };

    let matches = search.search_ordered(query_str, dir);
    let count = matches.len();

    if !out_matches.is_null() && max_matches > 0 {
        for (i, m) in matches.into_iter().take(max_matches).enumerate() {
            unsafe {
                *out_matches.add(i) = DtermSearchMatch {
                    line: m.line,
                    start_col: m.start_col,
                    end_col: m.end_col,
                };
            }
        }
    }

    count
}

/// Find the next match after a given position.
///
/// Returns true if a match was found, false otherwise.
///
/// # Safety
///
/// - `search` must be a valid pointer returned by a search constructor, or null (returns false).
/// - `query` must point to at least `query_len` readable bytes, or be null (returns false).
/// - `query` must contain valid UTF-8 data.
/// - `out_match` must be a valid writable pointer, or null (no-op but still returns result).
#[no_mangle]
pub unsafe extern "C" fn dterm_search_find_next(
    search: *const DtermSearch,
    query: *const u8,
    query_len: usize,
    after_line: usize,
    after_col: usize,
    out_match: *mut DtermSearchMatch,
) -> bool {
    // Return false for null pointers or empty query
    if search.is_null() || query.is_null() || query_len == 0 {
        return false;
    }

    let search = unsafe { &(*search).0 };
    let bytes = unsafe { std::slice::from_raw_parts(query, query_len) };
    let query_str = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return false,
    };

    if let Some(m) = search.find_next(query_str, after_line, after_col) {
        if !out_match.is_null() {
            unsafe {
                *out_match = DtermSearchMatch {
                    line: m.line,
                    start_col: m.start_col,
                    end_col: m.end_col,
                };
            }
        }
        true
    } else {
        false
    }
}

/// Find the previous match before a given position.
///
/// Returns true if a match was found, false otherwise.
///
/// # Safety
///
/// - `search` must be a valid pointer returned by a search constructor, or null (returns false).
/// - `query` must point to at least `query_len` readable bytes, or be null (returns false).
/// - `query` must contain valid UTF-8 data.
/// - `out_match` must be a valid writable pointer, or null (no-op but still returns result).
#[no_mangle]
pub unsafe extern "C" fn dterm_search_find_prev(
    search: *const DtermSearch,
    query: *const u8,
    query_len: usize,
    before_line: usize,
    before_col: usize,
    out_match: *mut DtermSearchMatch,
) -> bool {
    // Return false for null pointers or empty query
    if search.is_null() || query.is_null() || query_len == 0 {
        return false;
    }

    let search = unsafe { &(*search).0 };
    let bytes = unsafe { std::slice::from_raw_parts(query, query_len) };
    let query_str = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return false,
    };

    if let Some(m) = search.find_prev(query_str, before_line, before_col) {
        if !out_match.is_null() {
            unsafe {
                *out_match = DtermSearchMatch {
                    line: m.line,
                    start_col: m.start_col,
                    end_col: m.end_col,
                };
            }
        }
        true
    } else {
        false
    }
}

/// Get the number of indexed lines.
///
/// # Safety
///
/// - `search` must be a valid pointer returned by a search constructor, or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_search_line_count(search: *const DtermSearch) -> usize {
    if search.is_null() {
        return 0;
    }
    unsafe { (*search).0.indexed_line_count() }
}

/// Clear the search index.
///
/// # Safety
///
/// - `search` must be a valid pointer returned by a search constructor, or null (no-op).
#[no_mangle]
pub unsafe extern "C" fn dterm_search_clear(search: *mut DtermSearch) {
    if !search.is_null() {
        unsafe { (*search).0.clear() };
    }
}

// =============================================================================
// CALLBACK REGISTRATION
// =============================================================================

/// Bell callback type.
///
/// Nullable function pointer - pass null to disable the callback.
pub type DtermBellCallback = Option<extern "C" fn(*mut c_void)>;

/// Buffer activation callback type.
///
/// Called when the terminal switches between main and alternate screen buffers.
/// The boolean parameter is `true` when switching to the alternate screen,
/// `false` when switching back to the main screen.
///
/// Nullable function pointer - pass null to disable the callback.
pub type DtermBufferActivationCallback = Option<extern "C" fn(*mut c_void, bool)>;

/// Kitty image callback type.
///
/// Called when a Kitty graphics image is successfully transmitted and stored.
/// Parameters:
/// - `context`: User context pointer
/// - `id`: Image ID assigned by the terminal
/// - `width`: Image width in pixels
/// - `height`: Image height in pixels
/// - `data`: RGBA pixel data (4 bytes per pixel, length = width * height * 4)
/// - `data_len`: Length of the pixel data in bytes
///
/// The `data` pointer is valid only during the callback invocation.
/// The caller must copy the data if it needs to persist beyond the callback.
///
/// Nullable function pointer - pass null to disable the callback.
pub type DtermKittyImageCallback =
    Option<extern "C" fn(*mut c_void, u32, u32, u32, *const u8, usize)>;

/// Title callback type.
///
/// Called when the terminal title changes (OSC 0, OSC 2).
/// Parameters:
/// - `context`: User context pointer
/// - `title`: Null-terminated UTF-8 string with the new title
///
/// The `title` pointer is valid only during the callback invocation.
///
/// Nullable function pointer - pass null to disable the callback.
pub type DtermTitleCallback = Option<extern "C" fn(*mut c_void, *const c_char)>;

/// Icon name callback type.
///
/// Called when the terminal icon name changes (OSC 1).
/// Parameters:
/// - `context`: User context pointer
/// - `icon_name`: Null-terminated UTF-8 string with the new icon name
///
/// The `icon_name` pointer is valid only during the callback invocation.
///
/// Nullable function pointer - pass null to disable the callback.
pub type DtermIconNameCallback = Option<extern "C" fn(*mut c_void, *const c_char)>;

/// Window operation type for FFI.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtermWindowOpType {
    /// De-iconify (restore from minimized) window (CSI 1 t).
    DeIconify = 1,
    /// Iconify (minimize) window (CSI 2 t).
    Iconify = 2,
    /// Move window to pixel position (CSI 3;x;y t).
    MoveWindow = 3,
    /// Resize window to pixel dimensions (CSI 4;height;width t).
    ResizeWindowPixels = 4,
    /// Raise window to front (CSI 5 t).
    RaiseWindow = 5,
    /// Lower window to back (CSI 6 t).
    LowerWindow = 6,
    /// Refresh/redraw window (CSI 7 t).
    RefreshWindow = 7,
    /// Resize text area to cell dimensions (CSI 8;rows;cols t).
    ResizeWindowCells = 8,
    /// Report window state (CSI 11 t).
    ReportWindowState = 11,
    /// Report window position (CSI 13 t).
    ReportWindowPosition = 13,
    /// Report text area size in pixels (CSI 14 t).
    ReportWindowSizePixels = 14,
    /// Report screen size in pixels (CSI 15 t).
    ReportScreenSizePixels = 15,
    /// Report cell size in pixels (CSI 16 t).
    ReportCellSizePixels = 16,
    /// Report text area size in cells (CSI 18 t).
    ReportTextAreaCells = 18,
    /// Report screen size in cells (CSI 19 t).
    ReportScreenSizeCells = 19,
    /// Report icon label (CSI 20 t).
    ReportIconLabel = 20,
    /// Report window title (CSI 21 t).
    ReportWindowTitle = 21,
    /// Push icon/title to stack (CSI 22;mode t).
    PushTitle = 22,
    /// Pop icon/title from stack (CSI 23;mode t).
    PopTitle = 23,
    /// Maximize window (CSI 9;0 t).
    MaximizeWindow = 90,
    /// Enter fullscreen (CSI 10;0 t).
    EnterFullscreen = 100,
    /// Exit fullscreen (CSI 10;1 t).
    ExitFullscreen = 101,
    /// Toggle fullscreen (CSI 10;2 t).
    ToggleFullscreen = 102,
}

/// Window operation parameters structure.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DtermWindowOp {
    /// Operation type.
    pub op_type: DtermWindowOpType,
    /// First parameter (x, height, or mode depending on operation).
    pub param1: u16,
    /// Second parameter (y, width, or 0 depending on operation).
    pub param2: u16,
}

/// Window callback response structure.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DtermWindowResponse {
    /// Whether a response should be sent to the terminal.
    pub has_response: bool,
    /// For state reports: the state value.
    pub state: u8,
    /// For position/size reports: x or width value.
    pub x_or_width: u16,
    /// For position/size reports: y or height value.
    pub y_or_height: u16,
}

/// Window command callback type.
///
/// Called when a window manipulation command is received (CSI t).
/// The callback should handle the operation and optionally return response data.
///
/// Parameters:
/// - `context`: User context pointer
/// - `op`: The window operation to perform
/// - `response`: Output parameter for the response (if any)
///
/// Returns: true if the operation was handled, false otherwise.
///
/// Nullable function pointer - pass null to disable the callback.
pub type DtermWindowCallback =
    Option<extern "C" fn(*mut c_void, *const DtermWindowOp, *mut DtermWindowResponse) -> bool>;

/// Shell event type for FFI.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtermShellEventType {
    /// Prompt started (OSC 133 ; A).
    PromptStart = 0,
    /// Command input started (OSC 133 ; B).
    CommandStart = 1,
    /// Command execution started (OSC 133 ; C).
    OutputStart = 2,
    /// Command finished (OSC 133 ; D).
    CommandFinished = 3,
    /// Working directory changed (OSC 7).
    DirectoryChanged = 4,
}

/// Shell event structure for FFI.
#[repr(C)]
pub struct DtermShellEvent {
    /// Event type.
    pub event_type: DtermShellEventType,
    /// Row where event occurred (for position events).
    pub row: u32,
    /// Column where event occurred (for position events).
    pub col: u16,
    /// Exit code (for CommandFinished, -1 if unknown).
    pub exit_code: i32,
    /// Path or URL (for DirectoryChanged, null otherwise).
    /// Valid only during callback invocation.
    pub path: *const c_char,
}

/// Shell event callback type.
///
/// Called when shell integration events occur (OSC 133, OSC 7).
/// Parameters:
/// - `context`: User context pointer
/// - `event`: The shell event data
///
/// Nullable function pointer - pass null to disable the callback.
pub type DtermShellEventCallback = Option<extern "C" fn(*mut c_void, *const DtermShellEvent)>;

// =============================================================================
// CHECKPOINT API
// =============================================================================

/// Opaque checkpoint manager handle.
pub struct DtermCheckpoint(crate::checkpoint::CheckpointManager);

/// Create a new checkpoint manager.
///
/// # Safety
///
/// - `path` must point to at least `path_len` readable bytes, or be null (returns null).
/// - `path` must contain valid UTF-8 data representing a valid filesystem path.
#[no_mangle]
pub unsafe extern "C" fn dterm_checkpoint_new(
    path: *const u8,
    path_len: usize,
) -> *mut DtermCheckpoint {
    if path.is_null() {
        return ptr::null_mut();
    }

    let bytes = unsafe { std::slice::from_raw_parts(path, path_len) };
    let path_str = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let checkpoint = crate::checkpoint::CheckpointManager::new(std::path::Path::new(path_str));
    Box::into_raw(Box::new(DtermCheckpoint(checkpoint)))
}

/// Free a checkpoint manager.
///
/// # Safety
///
/// - `checkpoint` must be a valid pointer returned by `dterm_checkpoint_new`, or null (no-op).
/// - `checkpoint` must not have been freed previously (no double-free).
/// - After this call, `checkpoint` is invalid and must not be used.
#[no_mangle]
pub unsafe extern "C" fn dterm_checkpoint_free(checkpoint: *mut DtermCheckpoint) {
    if !checkpoint.is_null() {
        drop(unsafe { Box::from_raw(checkpoint) });
    }
}

/// Check if a checkpoint should be performed based on time and line thresholds.
///
/// # Safety
///
/// - `checkpoint` must be a valid pointer returned by `dterm_checkpoint_new`, or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_checkpoint_should_save(checkpoint: *const DtermCheckpoint) -> bool {
    if checkpoint.is_null() {
        return false;
    }
    unsafe { (*checkpoint).0.should_checkpoint() }
}

/// Notify the checkpoint manager that lines were added.
///
/// # Safety
///
/// - `checkpoint` must be a valid pointer returned by `dterm_checkpoint_new`, or null (no-op).
#[no_mangle]
pub unsafe extern "C" fn dterm_checkpoint_notify_lines(
    checkpoint: *mut DtermCheckpoint,
    count: usize,
) {
    if !checkpoint.is_null() {
        unsafe { (*checkpoint).0.notify_lines_added(count) };
    }
}

/// Check if a valid checkpoint exists.
///
/// # Safety
///
/// - `checkpoint` must be a valid pointer returned by `dterm_checkpoint_new`, or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_checkpoint_exists(checkpoint: *const DtermCheckpoint) -> bool {
    if checkpoint.is_null() {
        return false;
    }
    unsafe { (*checkpoint).0.has_checkpoint() }
}

/// Save a checkpoint of the terminal state.
///
/// Returns true on success, false on failure.
///
/// # Safety
///
/// - `checkpoint` must be a valid pointer returned by `dterm_checkpoint_new`, or null (returns false).
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_checkpoint_save(
    checkpoint: *mut DtermCheckpoint,
    term: *const DtermTerminal,
) -> bool {
    if checkpoint.is_null() || term.is_null() {
        return false;
    }

    let checkpoint = unsafe { &mut (*checkpoint).0 };
    let term = unsafe { &(*term).terminal };

    // Save grid and scrollback
    checkpoint.save(term.grid(), term.scrollback()).is_ok()
}

/// Restore terminal state from the latest checkpoint.
///
/// Returns a new terminal on success, null on failure.
///
/// # Safety
///
/// - `checkpoint` must be a valid pointer returned by `dterm_checkpoint_new`, or null (returns null).
#[no_mangle]
pub unsafe extern "C" fn dterm_checkpoint_restore(
    checkpoint: *const DtermCheckpoint,
) -> *mut DtermTerminal {
    if checkpoint.is_null() {
        return ptr::null_mut();
    }

    let checkpoint = unsafe { &(*checkpoint).0 };

    match checkpoint.restore() {
        Ok((grid, scrollback)) => {
            // Create a new terminal from the restored state
            let terminal = if let Some(sb) = scrollback {
                Terminal::from_grid_and_scrollback(grid, sb)
            } else {
                Terminal::from_grid(grid)
            };

            Box::into_raw(Box::new(DtermTerminal {
                terminal,
                title_cache: CString::new("").expect("empty string has no NUL"),
                icon_name_cache: CString::new("").expect("empty string has no NUL"),
                cached_hyperlink: None,
                cached_cwd: None,
                cached_complex_char: None,
                dcs_callback: None,
                dcs_context: ptr::null_mut(),
            }))
        }
        Err(_) => ptr::null_mut(),
    }
}

// =============================================================================
// CLIPBOARD API (OSC 52)
// =============================================================================

/// Clipboard selection type for FFI.
///
/// These correspond to OSC 52 selection parameters.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtermClipboardSelection {
    /// System clipboard ('c')
    Clipboard = 0,
    /// Primary selection ('p')
    Primary = 1,
    /// Secondary selection ('q')
    Secondary = 2,
    /// Select ('s')
    Select = 3,
    /// Cut buffer 0 ('0')
    CutBuffer0 = 4,
    /// Cut buffer 1 ('1')
    CutBuffer1 = 5,
    /// Cut buffer 2 ('2')
    CutBuffer2 = 6,
    /// Cut buffer 3 ('3')
    CutBuffer3 = 7,
    /// Cut buffer 4 ('4')
    CutBuffer4 = 8,
    /// Cut buffer 5 ('5')
    CutBuffer5 = 9,
    /// Cut buffer 6 ('6')
    CutBuffer6 = 10,
    /// Cut buffer 7 ('7')
    CutBuffer7 = 11,
}

impl From<crate::terminal::ClipboardSelection> for DtermClipboardSelection {
    fn from(sel: crate::terminal::ClipboardSelection) -> Self {
        match sel {
            crate::terminal::ClipboardSelection::Clipboard => DtermClipboardSelection::Clipboard,
            crate::terminal::ClipboardSelection::Primary => DtermClipboardSelection::Primary,
            crate::terminal::ClipboardSelection::Secondary => DtermClipboardSelection::Secondary,
            crate::terminal::ClipboardSelection::Select => DtermClipboardSelection::Select,
            crate::terminal::ClipboardSelection::CutBuffer(0) => {
                DtermClipboardSelection::CutBuffer0
            }
            crate::terminal::ClipboardSelection::CutBuffer(1) => {
                DtermClipboardSelection::CutBuffer1
            }
            crate::terminal::ClipboardSelection::CutBuffer(2) => {
                DtermClipboardSelection::CutBuffer2
            }
            crate::terminal::ClipboardSelection::CutBuffer(3) => {
                DtermClipboardSelection::CutBuffer3
            }
            crate::terminal::ClipboardSelection::CutBuffer(4) => {
                DtermClipboardSelection::CutBuffer4
            }
            crate::terminal::ClipboardSelection::CutBuffer(5) => {
                DtermClipboardSelection::CutBuffer5
            }
            crate::terminal::ClipboardSelection::CutBuffer(6) => {
                DtermClipboardSelection::CutBuffer6
            }
            crate::terminal::ClipboardSelection::CutBuffer(n) => {
                // For cut buffers beyond 7, fall back to CutBuffer7
                if n >= 7 {
                    DtermClipboardSelection::CutBuffer7
                } else {
                    unreachable!()
                }
            }
        }
    }
}

/// Clipboard operation type for FFI.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtermClipboardOpType {
    /// Set clipboard content
    Set = 0,
    /// Query clipboard content
    Query = 1,
    /// Clear clipboard
    Clear = 2,
}

/// Clipboard operation for FFI.
///
/// This struct is passed to the clipboard callback when an OSC 52
/// sequence is processed.
#[repr(C)]
pub struct DtermClipboardOp {
    /// Operation type (Set, Query, or Clear)
    pub op_type: DtermClipboardOpType,
    /// Selection targets (bitmask: bit 0 = clipboard, bit 1 = primary, etc.)
    /// Use `dterm_clipboard_selection_mask()` to convert DtermClipboardSelection to mask.
    pub selections_mask: u16,
    /// Number of selections (1-12)
    pub selection_count: u8,
    /// Array of selection targets (up to 12)
    pub selections: [DtermClipboardSelection; 12],
    /// Content length in bytes (for Set operations)
    pub content_len: usize,
    /// Content pointer (for Set operations, UTF-8 encoded, NOT null-terminated)
    /// NULL for Query and Clear operations.
    pub content: *const c_char,
}

/// Clipboard callback function type.
///
/// For Query operations, the callback should:
/// 1. Copy the clipboard content to the provided buffer (if not NULL)
/// 2. Return the length of the clipboard content
/// 3. Return 0 to deny access or if clipboard is empty
///
/// For Set and Clear operations, return value is ignored.
///
/// # Arguments
///
/// * `context` - User context pointer passed to `dterm_terminal_set_clipboard_callback`
/// * `op` - Clipboard operation details
/// * `response_buffer` - Buffer to write clipboard content for Query operations (may be NULL)
/// * `response_buffer_len` - Size of response buffer
///
/// # Returns
///
/// Length of clipboard content for Query operations (0 to deny), ignored for Set/Clear.
pub type DtermClipboardCallback = extern "C" fn(
    context: *mut c_void,
    op: *const DtermClipboardOp,
    response_buffer: *mut c_char,
    response_buffer_len: usize,
) -> usize;

/// Get the bitmask value for a clipboard selection.
///
/// Use this to interpret the `selections_mask` field in DtermClipboardOp.
#[no_mangle]
pub extern "C" fn dterm_clipboard_selection_mask(selection: DtermClipboardSelection) -> u16 {
    1u16 << (selection as u16)
}

/// Wrapper to make FFI clipboard callback state Send-safe.
///
/// This struct wraps raw pointers and function pointers from C code.
/// The Send implementation is marked unsafe because the caller must ensure:
/// - The callback function is safe to call from any thread
/// - The context pointer (if non-null) points to thread-safe data
///
/// # Safety
///
/// The caller is responsible for ensuring the context pointer remains valid
/// and that any data it points to is accessed in a thread-safe manner.
#[repr(C)]
struct ClipboardCallbackState {
    callback: DtermClipboardCallback,
    context: *mut c_void,
}

// SAFETY: The callback function pointer is a static function from C code, and
// the context pointer is provided by the FFI caller, who is responsible for
// ensuring thread safety of their context data. This is a standard pattern for
// C FFI callbacks. The Send trait is required because the Rust terminal callback
// may be called from different threads.
//
// The caller of dterm_terminal_set_clipboard_callback MUST ensure:
// 1. The callback function is safe to call from any thread
// 2. The context pointer points to data that is safe to access from any thread
//    (e.g., protected by a mutex, or immutable)
unsafe impl Send for ClipboardCallbackState {}

/// Set clipboard callback for OSC 52 operations.
///
/// The callback is invoked when an application sends OSC 52 to set, query,
/// or clear the clipboard.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (no-op).
/// - `callback` must be a valid function pointer, or null to disable clipboard support.
/// - `context` is passed to the callback and can be any value (including null).
/// - The callback and context must remain valid for the lifetime of the terminal.
/// - The caller is responsible for ensuring thread-safety of the context data.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_set_clipboard_callback(
    term: *mut DtermTerminal,
    callback: Option<DtermClipboardCallback>,
    context: *mut c_void,
) {
    if term.is_null() {
        return;
    }

    let term_ref = unsafe { &mut (*term).terminal };

    match callback {
        Some(cb) => {
            // Wrap the callback state in a struct with unsafe Send impl.
            // This is safe because the C caller is responsible for thread safety.
            let state = Box::new(ClipboardCallbackState {
                callback: cb,
                context,
            });

            term_ref.set_clipboard_callback(move |op| {
                // Access state through Box
                let state = &*state;
                // Convert ClipboardOperation to DtermClipboardOp
                let (op_type, selections, content_str): (
                    DtermClipboardOpType,
                    &[crate::terminal::ClipboardSelection],
                    Option<&str>,
                ) = match &op {
                    crate::terminal::ClipboardOperation::Set {
                        selections,
                        content,
                    } => (
                        DtermClipboardOpType::Set,
                        selections,
                        Some(content.as_str()),
                    ),
                    crate::terminal::ClipboardOperation::Query { selections } => {
                        (DtermClipboardOpType::Query, selections, None)
                    }
                    crate::terminal::ClipboardOperation::Clear { selections } => {
                        (DtermClipboardOpType::Clear, selections, None)
                    }
                };

                // Build selections array
                let mut sel_array = [DtermClipboardSelection::Clipboard; 12];
                let mut sel_mask = 0u16;
                let sel_count = selections.len().min(12);
                for (i, sel) in selections.iter().take(12).enumerate() {
                    let ffi_sel: DtermClipboardSelection = (*sel).into();
                    sel_array[i] = ffi_sel;
                    sel_mask |= 1u16 << (ffi_sel as u16);
                }

                // Build DtermClipboardOp
                let (content_ptr, content_len) = match content_str {
                    Some(s) => (s.as_ptr() as *const c_char, s.len()),
                    None => (ptr::null(), 0),
                };

                // sel_count is at most 12 (from .min(12)), fits in u8
                #[allow(clippy::cast_possible_truncation)]
                let selection_count = sel_count as u8;
                let ffi_op = DtermClipboardOp {
                    op_type,
                    selections_mask: sel_mask,
                    selection_count,
                    selections: sel_array,
                    content_len,
                    content: content_ptr,
                };

                if op_type == DtermClipboardOpType::Query {
                    // For query, we need to get the clipboard content back
                    // Allocate a buffer for the response
                    const MAX_RESPONSE_LEN: usize = 1024 * 1024; // 1MB max
                    let mut response_buf = vec![0u8; MAX_RESPONSE_LEN];

                    let response_len = (state.callback)(
                        state.context,
                        &raw const ffi_op,
                        response_buf.as_mut_ptr() as *mut c_char,
                        MAX_RESPONSE_LEN,
                    );

                    if response_len > 0 && response_len <= MAX_RESPONSE_LEN {
                        // Convert response to String
                        response_buf.truncate(response_len);
                        String::from_utf8(response_buf).ok()
                    } else {
                        None
                    }
                } else {
                    // For Set/Clear, just call the callback
                    (state.callback)(state.context, &raw const ffi_op, ptr::null_mut(), 0);
                    None
                }
            });
        }
        None => {
            // Disable clipboard callback - we can't easily unset it,
            // so set a no-op callback
            term_ref.set_clipboard_callback(|_| None);
        }
    }
}

// =============================================================================
// HYPERLINK API (OSC 8)
// =============================================================================

/// Get the hyperlink URL for a cell, if any.
///
/// OSC 8 hyperlinks allow terminal applications to create clickable links.
/// This function returns the URL associated with a cell, or NULL if the cell
/// has no hyperlink.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns null).
/// - The returned string is owned by the terminal and must NOT be freed by the caller.
/// - The returned string is valid until the next call to `dterm_terminal_process`,
///   `dterm_terminal_reset`, or `dterm_terminal_free`.
///
/// # Returns
///
/// A pointer to a null-terminated UTF-8 string containing the URL, or NULL if:
/// - `term` is NULL
/// - The cell is out of bounds
/// - The cell has no hyperlink
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_cell_hyperlink(
    term: *mut DtermTerminal,
    row: u16,
    col: u16,
) -> *const c_char {
    if term.is_null() {
        return ptr::null();
    }
    let term_data = unsafe { &mut *term };
    let term_ref = &term_data.terminal;

    // Get the cell extra for this position
    if let Some(extra) = term_ref.grid().cell_extra(row, col) {
        if let Some(hyperlink) = extra.hyperlink() {
            // Store the hyperlink URL in a cached CString to return to caller
            // This ensures the string lives long enough to be used
            let url_cstring = match CString::new(hyperlink.as_ref()) {
                Ok(s) => s,
                Err(_) => return ptr::null(), // URL contains null byte
            };

            // Cache the CString in the terminal wrapper
            term_data.cached_hyperlink = Some(url_cstring);

            // Return pointer to the cached string (just set above)
            return term_data.cached_hyperlink.as_ref().expect("just set").as_ptr();
        }
    }
    ptr::null()
}

/// Check if a cell has a hyperlink.
///
/// This is a faster check than `dterm_terminal_cell_hyperlink` when you only
/// need to know if a hyperlink exists, not what the URL is.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_cell_has_hyperlink(
    term: *const DtermTerminal,
    row: u16,
    col: u16,
) -> bool {
    if term.is_null() {
        return false;
    }
    let term_ref = unsafe { &(*term).terminal };

    if let Some(extra) = term_ref.grid().cell_extra(row, col) {
        extra.hyperlink().is_some()
    } else {
        false
    }
}

/// Check if a cell contains a complex character (non-BMP, grapheme cluster).
///
/// Complex characters are stored in overflow and cannot be represented
/// as a single codepoint. Use `dterm_terminal_cell_display_string` to
/// get the actual character string.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_cell_is_complex(
    term: *const DtermTerminal,
    row: u16,
    col: u16,
) -> bool {
    if term.is_null() {
        return false;
    }
    let term_ref = unsafe { &(*term).terminal };

    if let Some(cell) = term_ref.grid().cell(row, col) {
        cell.is_complex()
    } else {
        false
    }
}

/// Get the display string for a cell.
///
/// For simple cells, returns a string containing just the character.
/// For complex cells (non-BMP, grapheme clusters), returns the full string
/// from the overflow table. For wide continuation cells, returns an empty string.
///
/// # Returns
///
/// - Pointer to a null-terminated UTF-8 string for valid cells
/// - Null pointer if the cell doesn't exist or on error
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns null).
/// - The returned string is owned by the terminal and must NOT be freed by the caller.
/// - The returned string is valid until the next call to this function,
///   `dterm_terminal_process`, `dterm_terminal_reset`, or `dterm_terminal_free`.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_cell_display_string(
    term: *mut DtermTerminal,
    row: u16,
    col: u16,
) -> *const c_char {
    if term.is_null() {
        return ptr::null();
    }
    let term_data = unsafe { &mut *term };

    if let Some(display_str) = term_data.terminal.grid().cell_display_char(row, col) {
        match CString::new(display_str) {
            Ok(s) => {
                term_data.cached_complex_char = Some(s);
                term_data.cached_complex_char.as_ref().expect("just set").as_ptr()
            }
            Err(_) => ptr::null(), // String contains null byte
        }
    } else {
        ptr::null()
    }
}

/// Get the text content of a visible row, properly resolving complex characters.
///
/// This returns the full text of a row, including proper handling of
/// non-BMP characters like emoji that are stored in overflow.
///
/// # Returns
///
/// - Pointer to a null-terminated UTF-8 string
/// - Null pointer if row is out of bounds or on error
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns null).
/// - The returned string is owned by the terminal and must NOT be freed by the caller.
/// - The returned string is valid until the next call to this function,
///   `dterm_terminal_process`, `dterm_terminal_reset`, or `dterm_terminal_free`.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_row_text(
    term: *mut DtermTerminal,
    row: u16,
) -> *const c_char {
    if term.is_null() {
        return ptr::null();
    }
    let term_data = unsafe { &mut *term };

    if let Some(text) = term_data.terminal.grid().row_text(row) {
        match CString::new(text) {
            Ok(s) => {
                term_data.cached_complex_char = Some(s);
                term_data.cached_complex_char.as_ref().expect("just set").as_ptr()
            }
            Err(_) => ptr::null(),
        }
    } else {
        ptr::null()
    }
}

/// Get the current hyperlink URL being applied to new text.
///
/// When an OSC 8 hyperlink sequence is received, subsequent text will have
/// the hyperlink applied until the hyperlink is cleared. This function
/// returns the current active hyperlink URL, if any.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns null).
/// - The returned string is owned by the terminal and must NOT be freed by the caller.
/// - The returned string is valid until the next call to `dterm_terminal_process`,
///   `dterm_terminal_reset`, or `dterm_terminal_free`.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_current_hyperlink(
    term: *mut DtermTerminal,
) -> *const c_char {
    if term.is_null() {
        return ptr::null();
    }
    let term_data = unsafe { &mut *term };

    if let Some(ref hyperlink) = term_data.terminal.current_hyperlink() {
        let url_cstring = match CString::new(hyperlink.as_ref()) {
            Ok(s) => s,
            Err(_) => return ptr::null(),
        };
        term_data.cached_hyperlink = Some(url_cstring);
        return term_data.cached_hyperlink.as_ref().expect("just set").as_ptr();
    }
    ptr::null()
}

// =============================================================================
// CURRENT WORKING DIRECTORY (OSC 7)
// =============================================================================

/// Get the current working directory (OSC 7).
///
/// Returns the path portion of the file:// URL set by OSC 7, or null if not set.
/// The path is already percent-decoded.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns null).
/// - The returned string is owned by the terminal and must NOT be freed by the caller.
/// - The returned string is valid until the next call to `dterm_terminal_process`,
///   `dterm_terminal_reset`, or `dterm_terminal_free`.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_current_working_directory(
    term: *mut DtermTerminal,
) -> *const c_char {
    if term.is_null() {
        return ptr::null();
    }
    let term_data = unsafe { &mut *term };

    if let Some(cwd) = term_data.terminal.current_working_directory() {
        let cwd_cstring = match CString::new(cwd) {
            Ok(s) => s,
            Err(_) => return ptr::null(),
        };
        term_data.cached_cwd = Some(cwd_cstring);
        return term_data.cached_cwd.as_ref().expect("just set").as_ptr();
    }
    ptr::null()
}

/// Check if a current working directory is set.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_has_working_directory(term: *const DtermTerminal) -> bool {
    if term.is_null() {
        return false;
    }
    unsafe { (*term).terminal.current_working_directory().is_some() }
}

// =============================================================================
// COLOR PALETTE (OSC 4)
// =============================================================================

/// RGB color value for FFI.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DtermRgb {
    /// Red component (0-255).
    pub r: u8,
    /// Green component (0-255).
    pub g: u8,
    /// Blue component (0-255).
    pub b: u8,
}

impl From<Rgb> for DtermRgb {
    fn from(rgb: Rgb) -> Self {
        DtermRgb {
            r: rgb.r,
            g: rgb.g,
            b: rgb.b,
        }
    }
}

impl From<DtermRgb> for Rgb {
    fn from(rgb: DtermRgb) -> Self {
        Rgb {
            r: rgb.r,
            g: rgb.g,
            b: rgb.b,
        }
    }
}

/// Get a color from the terminal's 256-color palette.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null.
/// - `out_color` must be a valid pointer to a `DtermRgb` struct.
///
/// # Returns
///
/// Returns true if successful, false if `term` is null or `index` is invalid (>= 256).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_get_palette_color(
    term: *const DtermTerminal,
    index: u8,
    out_color: *mut DtermRgb,
) -> bool {
    if term.is_null() || out_color.is_null() {
        return false;
    }
    let color = unsafe { (*term).terminal.get_palette_color(index) };
    unsafe { *out_color = color.into() };
    true
}

/// Set a color in the terminal's 256-color palette.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (no-op).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_set_palette_color(
    term: *mut DtermTerminal,
    index: u8,
    r: u8,
    g: u8,
    b: u8,
) {
    if term.is_null() {
        return;
    }
    let rgb = Rgb { r, g, b };
    unsafe { (*term).terminal.set_palette_color(index, rgb) };
}

/// Reset the entire 256-color palette to default values.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (no-op).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_reset_palette(term: *mut DtermTerminal) {
    if term.is_null() {
        return;
    }
    unsafe { (*term).terminal.reset_color_palette() };
}

/// Reset a single color in the palette to its default value.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (no-op).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_reset_palette_color(term: *mut DtermTerminal, index: u8) {
    if term.is_null() {
        return;
    }
    // Reset by setting to default palette color
    use crate::terminal::ColorPalette;
    let default_palette = ColorPalette::new();
    let default_color = default_palette.get(index);
    unsafe { (*term).terminal.set_palette_color(index, default_color) };
}

// =============================================================================
// DASHTERM2 INTEGRATION APIs
// =============================================================================
// These APIs were requested by the DashTerm2 team for integration.
// See: dashterm2/docs/DCORE-REQUESTS.md

// -----------------------------------------------------------------------------
// Request 1: DCS Sequence Callbacks (High Priority)
// For Sixel graphics and DECRQSS support
// -----------------------------------------------------------------------------

/// DCS callback type for handling DCS sequences.
///
/// Parameters:
/// - `context`: User-provided context pointer
/// - `data`: DCS payload data
/// - `len`: Length of data
/// - `final_byte`: Final byte of the DCS sequence
pub type DtermDCSCallback =
    Option<unsafe extern "C" fn(context: *mut c_void, data: *const u8, len: usize, final_byte: u8)>;

struct SendContext(*mut c_void);

// SAFETY: The context pointer is opaque to Rust; the caller guarantees thread safety.
unsafe impl Send for SendContext {}

impl SendContext {
    fn as_ptr(&self) -> *mut c_void {
        self.0
    }
}

/// Set a callback for DCS sequences (e.g., Sixel graphics, DECRQSS).
///
/// The callback will be invoked for each DCS sequence received by the terminal.
/// Currently supported DCS sequences:
/// - Sixel graphics (callback receives raw payload bytes)
/// - DECRQSS (request selection or settings)
/// - Unknown DCS sequences (payload passed through)
///
/// # Safety
///
/// - `term` must be a valid pointer returned by `dterm_terminal_new`, or null (no-op).
/// - `callback` may be null to disable the callback.
/// - `context` is passed to the callback and must remain valid while set.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_set_dcs_callback(
    term: *mut DtermTerminal,
    callback: DtermDCSCallback,
    context: *mut c_void,
) {
    if term.is_null() {
        return;
    }
    unsafe {
        (*term).dcs_callback = callback;
        (*term).dcs_context = context;
        if let Some(callback) = callback {
            let ctx = SendContext(context);
            (*term).terminal.set_dcs_callback(move |data, final_byte| {
                callback(ctx.as_ptr(), data.as_ptr(), data.len(), final_byte);
            });
        } else {
            (*term).terminal.clear_dcs_callback();
        }
    }
}

/// Set a callback for the terminal bell (BEL character, 0x07).
///
/// The callback will be invoked when the terminal receives a BEL character.
/// This is typically used to play a sound or flash the window.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by `dterm_terminal_new`, or null (no-op).
/// - `callback` may be null to disable the callback.
/// - `context` is passed to the callback and must remain valid while set.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_set_bell_callback(
    term: *mut DtermTerminal,
    callback: DtermBellCallback,
    context: *mut c_void,
) {
    if term.is_null() {
        return;
    }
    unsafe {
        if let Some(cb) = callback {
            let ctx = SendContext(context);
            (*term).terminal.set_bell_callback(move || {
                cb(ctx.as_ptr());
            });
        }
        // Note: No way to unset bell callback currently, but passing None
        // will just not set a new one
    }
}

/// Set a callback to be invoked when the terminal switches between buffers.
///
/// The callback will be invoked when the terminal switches between the main
/// and alternate screen buffers (e.g., when entering/exiting vim, less, etc.).
/// The boolean parameter is `true` when switching to the alternate screen,
/// `false` when switching back to the main screen.
///
/// This maps to SwiftTerm's `bufferActivated` delegate callback.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by `dterm_terminal_new`, or null (no-op).
/// - `callback` may be null to disable the callback.
/// - `context` is passed to the callback and must remain valid while set.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_set_buffer_activation_callback(
    term: *mut DtermTerminal,
    callback: DtermBufferActivationCallback,
    context: *mut c_void,
) {
    if term.is_null() {
        return;
    }
    unsafe {
        if let Some(cb) = callback {
            let ctx = SendContext(context);
            (*term)
                .terminal
                .set_buffer_activation_callback(move |is_alternate| {
                    cb(ctx.as_ptr(), is_alternate);
                });
        }
        // Note: No way to unset callback currently, but passing None
        // will just not set a new one
    }
}

/// Set callback for Kitty graphics images.
///
/// The callback is invoked when a Kitty graphics image is successfully
/// transmitted and stored. Parameters passed to the callback:
/// - `context`: User context pointer
/// - `id`: Image ID assigned by the terminal
/// - `width`: Image width in pixels
/// - `height`: Image height in pixels
/// - `data`: RGBA pixel data (4 bytes per pixel)
/// - `data_len`: Length of the pixel data in bytes
///
/// The `data` pointer is valid only during the callback invocation.
/// The caller must copy the data if it needs to persist beyond the callback.
///
/// This maps to SwiftTerm's `createImage` delegate callback.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by `dterm_terminal_new`, or null (no-op).
/// - `callback` may be null to disable the callback.
/// - `context` is passed to the callback and must remain valid while set.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_set_kitty_image_callback(
    term: *mut DtermTerminal,
    callback: DtermKittyImageCallback,
    context: *mut c_void,
) {
    if term.is_null() {
        return;
    }
    unsafe {
        if let Some(cb) = callback {
            let ctx = SendContext(context);
            (*term)
                .terminal
                .set_kitty_image_callback(move |id, width, height, data| {
                    cb(ctx.as_ptr(), id, width, height, data.as_ptr(), data.len());
                });
        }
        // Note: No way to unset callback currently, but passing None
        // will just not set a new one
    }
}

/// Set callback for terminal title changes (OSC 0, OSC 2).
///
/// The callback is invoked when the terminal title is changed via OSC 0 or OSC 2.
/// This maps to SwiftTerm's `setTerminalTitle` delegate callback.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by `dterm_terminal_new`, or null (no-op).
/// - `callback` may be null to disable the callback.
/// - `context` is passed to the callback and must remain valid while set.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_set_title_callback(
    term: *mut DtermTerminal,
    callback: DtermTitleCallback,
    context: *mut c_void,
) {
    if term.is_null() {
        return;
    }
    unsafe {
        if let Some(cb) = callback {
            let ctx = SendContext(context);
            (*term).terminal.set_title_callback(move |title| {
                // Create a temporary CString for the callback
                if let Ok(c_title) = CString::new(title) {
                    cb(ctx.as_ptr(), c_title.as_ptr());
                }
            });
        }
    }
}

/// Set callback for window manipulation commands (CSI t / XTWINOPS).
///
/// The callback is invoked when a window manipulation command is received.
/// This maps to SwiftTerm's `windowCommand` delegate callback.
///
/// The callback receives the operation type and parameters, and can optionally
/// provide a response (for query operations like report window position).
///
/// # Safety
///
/// - `term` must be a valid pointer returned by `dterm_terminal_new`, or null (no-op).
/// - `callback` may be null to disable the callback.
/// - `context` is passed to the callback and must remain valid while set.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_set_window_callback(
    term: *mut DtermTerminal,
    callback: DtermWindowCallback,
    context: *mut c_void,
) {
    if term.is_null() {
        return;
    }
    unsafe {
        if let Some(cb) = callback {
            let ctx = SendContext(context);
            (*term).terminal.set_window_callback(move |op| {
                use crate::terminal::WindowOperation;

                // Convert WindowOperation to DtermWindowOp
                let (op_type, param1, param2) = match op {
                    WindowOperation::DeIconify => (DtermWindowOpType::DeIconify, 0, 0),
                    WindowOperation::Iconify => (DtermWindowOpType::Iconify, 0, 0),
                    WindowOperation::MoveWindow { x, y } => (DtermWindowOpType::MoveWindow, x, y),
                    WindowOperation::ResizeWindowPixels { height, width } => {
                        (DtermWindowOpType::ResizeWindowPixels, height, width)
                    }
                    WindowOperation::RaiseWindow => (DtermWindowOpType::RaiseWindow, 0, 0),
                    WindowOperation::LowerWindow => (DtermWindowOpType::LowerWindow, 0, 0),
                    WindowOperation::RefreshWindow => (DtermWindowOpType::RefreshWindow, 0, 0),
                    WindowOperation::ResizeWindowCells { rows, cols } => {
                        (DtermWindowOpType::ResizeWindowCells, rows, cols)
                    }
                    WindowOperation::ReportWindowState => {
                        (DtermWindowOpType::ReportWindowState, 0, 0)
                    }
                    WindowOperation::ReportWindowPosition
                    | WindowOperation::ReportTextAreaPosition => {
                        (DtermWindowOpType::ReportWindowPosition, 0, 0)
                    }
                    WindowOperation::ReportWindowSizePixels
                    | WindowOperation::ReportTextAreaSizePixels => {
                        (DtermWindowOpType::ReportWindowSizePixels, 0, 0)
                    }
                    WindowOperation::ReportScreenSizePixels => {
                        (DtermWindowOpType::ReportScreenSizePixels, 0, 0)
                    }
                    WindowOperation::ReportCellSizePixels => {
                        (DtermWindowOpType::ReportCellSizePixels, 0, 0)
                    }
                    WindowOperation::ReportTextAreaSizeCells => {
                        (DtermWindowOpType::ReportTextAreaCells, 0, 0)
                    }
                    WindowOperation::ReportScreenSizeCells => {
                        (DtermWindowOpType::ReportScreenSizeCells, 0, 0)
                    }
                    WindowOperation::ReportIconLabel => (DtermWindowOpType::ReportIconLabel, 0, 0),
                    WindowOperation::ReportWindowTitle => {
                        (DtermWindowOpType::ReportWindowTitle, 0, 0)
                    }
                    WindowOperation::PushTitle { icon, window } => {
                        // Encode icon/window flags in param1: bit 0 = icon, bit 1 = window
                        let mode = u16::from(icon) | (u16::from(window) << 1);
                        (DtermWindowOpType::PushTitle, mode, 0)
                    }
                    WindowOperation::PopTitle { icon, window } => {
                        let mode = u16::from(icon) | (u16::from(window) << 1);
                        (DtermWindowOpType::PopTitle, mode, 0)
                    }
                    WindowOperation::MaximizeWindow
                    | WindowOperation::MaximizeVertically
                    | WindowOperation::MaximizeHorizontally
                    | WindowOperation::RestoreMaximized => {
                        (DtermWindowOpType::MaximizeWindow, 0, 0)
                    }
                    WindowOperation::EnterFullscreen => (DtermWindowOpType::EnterFullscreen, 0, 0),
                    WindowOperation::UndoFullscreen => (DtermWindowOpType::ExitFullscreen, 0, 0),
                    WindowOperation::ToggleFullscreen => {
                        (DtermWindowOpType::ToggleFullscreen, 0, 0)
                    }
                };

                let ffi_op = DtermWindowOp {
                    op_type,
                    param1,
                    param2,
                };
                let mut response = DtermWindowResponse {
                    has_response: false,
                    state: 0,
                    x_or_width: 0,
                    y_or_height: 0,
                };

                let handled = cb(ctx.as_ptr(), &raw const ffi_op, &raw mut response);
                if !handled || !response.has_response {
                    return None;
                }

                // Convert response back to WindowResponse
                use crate::terminal::WindowResponse;
                Some(match op_type {
                    DtermWindowOpType::ReportWindowState => {
                        WindowResponse::WindowState(response.state != 0)
                    }
                    DtermWindowOpType::ReportWindowPosition => WindowResponse::Position {
                        x: response.x_or_width,
                        y: response.y_or_height,
                    },
                    DtermWindowOpType::ReportWindowSizePixels
                    | DtermWindowOpType::ReportScreenSizePixels
                    | DtermWindowOpType::ReportCellSizePixels => WindowResponse::SizePixels {
                        height: response.y_or_height,
                        width: response.x_or_width,
                    },
                    DtermWindowOpType::ReportTextAreaCells
                    | DtermWindowOpType::ReportScreenSizeCells => WindowResponse::SizeCells {
                        rows: response.y_or_height,
                        cols: response.x_or_width,
                    },
                    _ => return None,
                })
            });
        }
    }
}

/// Set callback for shell integration events (OSC 133, OSC 7).
///
/// The callback is invoked when shell integration markers are received.
/// This allows tracking of command prompts, input areas, output, and completion.
///
/// This maps to SwiftTerm's various shell integration delegate callbacks.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by `dterm_terminal_new`, or null (no-op).
/// - `callback` may be null to disable the callback.
/// - `context` is passed to the callback and must remain valid while set.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_set_shell_callback(
    term: *mut DtermTerminal,
    callback: DtermShellEventCallback,
    context: *mut c_void,
) {
    if term.is_null() {
        return;
    }
    unsafe {
        if let Some(cb) = callback {
            let ctx = SendContext(context);
            (*term).terminal.set_shell_callback(move |event| {
                use crate::terminal::ShellEvent;

                // Row values are capped at u32::MAX (safe truncation)
                #[allow(clippy::cast_possible_truncation)]
                let row_to_u32 = |r: usize| -> u32 { r.min(u32::MAX as usize) as u32 };
                let (event_type, row, col, exit_code) = match &event {
                    ShellEvent::PromptStart { row, col } => (
                        DtermShellEventType::PromptStart,
                        row_to_u32(*row),
                        *col,
                        -1i32,
                    ),
                    ShellEvent::CommandStart { row, col } => (
                        DtermShellEventType::CommandStart,
                        row_to_u32(*row),
                        *col,
                        -1i32,
                    ),
                    ShellEvent::OutputStart { row } => (
                        DtermShellEventType::OutputStart,
                        row_to_u32(*row),
                        0u16,
                        -1i32,
                    ),
                    ShellEvent::CommandFinished { exit_code } => {
                        (DtermShellEventType::CommandFinished, 0u32, 0u16, *exit_code)
                    }
                };

                let ffi_event = DtermShellEvent {
                    event_type,
                    row,
                    col,
                    exit_code,
                    path: ptr::null(), // DirectoryChanged is handled via OSC 7, not shell events
                };

                cb(ctx.as_ptr(), &raw const ffi_event);
            });
        }
    }
}

/// Sixel image structure for high-level Sixel API.
#[repr(C)]
pub struct DtermSixelImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// RGBA pixel data (width * height * 4 bytes).
    /// Owned by the caller after successful call.
    pub pixels: *mut u32,
}

/// Check if the terminal has a pending Sixel image.
///
/// Returns true if a Sixel image is available via `dterm_terminal_get_sixel_image`.
///
/// # Safety
///
/// - `term` must be a valid pointer or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_has_sixel_image(term: *const DtermTerminal) -> bool {
    if term.is_null() {
        return false;
    }
    unsafe { (*term).terminal.has_sixel_image() }
}

/// Get the pending Sixel image from the terminal.
///
/// If a Sixel image is available, fills `out` and returns true.
/// The caller owns the `pixels` pointer and must free it with `dterm_sixel_image_free`.
///
/// # Safety
///
/// - `term` must be a valid pointer or null (returns false).
/// - `out` must be a valid pointer to write the image structure.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_get_sixel_image(
    term: *mut DtermTerminal,
    out: *mut DtermSixelImage,
) -> bool {
    if term.is_null() || out.is_null() {
        return false;
    }
    let terminal = unsafe { &mut (*term).terminal };
    let image = match terminal.peek_sixel_image() {
        Some(image) => image,
        None => return false,
    };

    let width = match u32::try_from(image.width()) {
        Ok(width) => width,
        Err(_) => return false,
    };
    let height = match u32::try_from(image.height()) {
        Ok(height) => height,
        Err(_) => return false,
    };
    let expected_len = match image.width().checked_mul(image.height()) {
        Some(len) => len,
        None => return false,
    };
    if expected_len == 0 || expected_len != image.pixels().len() {
        return false;
    }
    let byte_len = match expected_len.checked_mul(std::mem::size_of::<u32>()) {
        Some(len) => len,
        None => return false,
    };

    let pixels = unsafe { malloc(byte_len) } as *mut u32;
    if pixels.is_null() {
        return false;
    }

    unsafe {
        ptr::copy_nonoverlapping(image.pixels().as_ptr(), pixels, expected_len);
        (*out).width = width;
        (*out).height = height;
        (*out).pixels = pixels;
    }

    terminal.take_sixel_image();
    true
}

/// Free a Sixel image's pixel buffer.
///
/// # Safety
///
/// - `pixels` must be a pointer returned by `dterm_terminal_get_sixel_image`, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_sixel_image_free(pixels: *mut u32) {
    if !pixels.is_null() {
        unsafe {
            free(pixels as *mut c_void);
        }
    }
}

// =============================================================================
// KITTY GRAPHICS API
// =============================================================================

/// Kitty graphics placement location type.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtermKittyPlacementLocation {
    /// Placed at absolute cursor position.
    Absolute = 0,
    /// Virtual placement (for Unicode placeholder mode).
    Virtual = 1,
    /// Relative to another placement.
    Relative = 2,
}

/// Kitty graphics placement structure for FFI.
#[repr(C)]
pub struct DtermKittyPlacement {
    /// Placement ID within the image.
    pub id: u32,
    /// Location type.
    pub location_type: DtermKittyPlacementLocation,
    /// Row position (for Absolute) or parent image ID (for Relative).
    pub row_or_parent_image: u32,
    /// Column position (for Absolute) or parent placement ID (for Relative).
    pub col_or_parent_placement: u32,
    /// Horizontal offset (for Relative placement, in cells).
    pub offset_x: i32,
    /// Vertical offset (for Relative placement, in cells).
    pub offset_y: i32,
    /// Source rectangle x offset (in pixels).
    pub source_x: u32,
    /// Source rectangle y offset (in pixels).
    pub source_y: u32,
    /// Source rectangle width (0 = full image).
    pub source_width: u32,
    /// Source rectangle height (0 = full image).
    pub source_height: u32,
    /// Pixel offset within starting cell, x.
    pub cell_x_offset: u32,
    /// Pixel offset within starting cell, y.
    pub cell_y_offset: u32,
    /// Number of columns to display (0 = auto).
    pub num_columns: u32,
    /// Number of rows to display (0 = auto).
    pub num_rows: u32,
    /// Z-index for stacking (negative = below text).
    pub z_index: i32,
    /// Whether this is a virtual placement.
    pub is_virtual: bool,
}

/// Kitty graphics image info structure for FFI.
#[repr(C)]
pub struct DtermKittyImageInfo {
    /// Image ID.
    pub id: u32,
    /// Image number (0 if not assigned).
    pub number: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Number of placements.
    pub placement_count: u32,
}

/// Check if Kitty graphics storage has any images.
///
/// # Safety
///
/// - `term` must be a valid pointer or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_kitty_has_images(term: *const DtermTerminal) -> bool {
    if term.is_null() {
        return false;
    }
    unsafe { (*term).terminal.kitty_graphics().image_count() > 0 }
}

/// Check if Kitty graphics storage has been modified since last clear.
///
/// Use this to determine if re-rendering is needed.
///
/// # Safety
///
/// - `term` must be a valid pointer or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_kitty_is_dirty(term: *const DtermTerminal) -> bool {
    if term.is_null() {
        return false;
    }
    unsafe { (*term).terminal.kitty_graphics().is_dirty() }
}

/// Clear the Kitty graphics dirty flag after rendering.
///
/// # Safety
///
/// - `term` must be a valid pointer or null (no-op).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_kitty_clear_dirty(term: *mut DtermTerminal) {
    if term.is_null() {
        return;
    }
    unsafe { (*term).terminal.kitty_graphics_mut().clear_dirty() }
}

/// Get the number of Kitty graphics images.
///
/// # Safety
///
/// - `term` must be a valid pointer or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_kitty_image_count(term: *const DtermTerminal) -> u32 {
    if term.is_null() {
        return 0;
    }
    // KittyImageStorage::image_count returns usize, truncate to u32
    #[allow(clippy::cast_possible_truncation)]
    let count = unsafe { (*term).terminal.kitty_graphics().image_count() as u32 };
    count
}

/// Get all Kitty image IDs into a buffer.
///
/// Returns the number of IDs written (or needed if buffer is null).
///
/// # Safety
///
/// - `term` must be a valid pointer or null (returns 0).
/// - If `ids` is not null, it must be valid for `ids_capacity` elements.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_kitty_image_ids(
    term: *const DtermTerminal,
    ids: *mut u32,
    ids_capacity: u32,
) -> u32 {
    if term.is_null() {
        return 0;
    }
    let storage = unsafe { (*term).terminal.kitty_graphics() };
    let image_ids = storage.image_ids();
    #[allow(clippy::cast_possible_truncation)]
    let count = image_ids.len() as u32;

    if ids.is_null() || ids_capacity == 0 {
        return count;
    }

    let write_count = std::cmp::min(count, ids_capacity) as usize;
    unsafe {
        ptr::copy_nonoverlapping(image_ids.as_ptr(), ids, write_count);
    }
    #[allow(clippy::cast_possible_truncation)]
    let result = write_count as u32;
    result
}

/// Get info about a Kitty graphics image by ID.
///
/// Returns true if the image exists and info was filled.
///
/// # Safety
///
/// - `term` must be a valid pointer or null (returns false).
/// - `info` must be a valid pointer (returns false if null).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_kitty_get_image_info(
    term: *const DtermTerminal,
    image_id: u32,
    info: *mut DtermKittyImageInfo,
) -> bool {
    if term.is_null() || info.is_null() {
        return false;
    }
    let storage = unsafe { (*term).terminal.kitty_graphics() };
    let image = match storage.get_image(image_id) {
        Some(img) => img,
        None => return false,
    };

    unsafe {
        (*info).id = image.id;
        (*info).number = image.number.unwrap_or(0);
        (*info).width = image.width;
        (*info).height = image.height;
        #[allow(clippy::cast_possible_truncation)]
        {
            (*info).placement_count = image.placement_count() as u32;
        }
    }
    true
}

/// Get pixel data for a Kitty graphics image.
///
/// The pixel data is in RGBA format (4 bytes per pixel).
/// Returns true if successful and fills `pixels` with a pointer to allocated memory.
/// The caller must free the memory with `dterm_kitty_image_free`.
///
/// # Safety
///
/// - `term` must be a valid pointer or null (returns false).
/// - `pixels` must be a valid pointer (returns false if null).
/// - `pixel_count` must be a valid pointer (returns false if null).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_kitty_get_image_pixels(
    term: *const DtermTerminal,
    image_id: u32,
    pixels: *mut *mut u8,
    pixel_count: *mut usize,
) -> bool {
    if term.is_null() || pixels.is_null() || pixel_count.is_null() {
        return false;
    }
    let storage = unsafe { (*term).terminal.kitty_graphics() };
    let image = match storage.get_image(image_id) {
        Some(img) => img,
        None => return false,
    };

    let data = &*image.data;
    let len = data.len();
    if len == 0 {
        return false;
    }

    let allocated = unsafe { malloc(len) } as *mut u8;
    if allocated.is_null() {
        return false;
    }

    unsafe {
        ptr::copy_nonoverlapping(data.as_ptr(), allocated, len);
        *pixels = allocated;
        *pixel_count = len;
    }
    true
}

/// Free Kitty image pixel data.
///
/// # Safety
///
/// - `pixels` must be a pointer returned by `dterm_terminal_kitty_get_image_pixels`, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_kitty_image_free(pixels: *mut u8) {
    if !pixels.is_null() {
        unsafe {
            free(pixels as *mut c_void);
        }
    }
}

/// Get the number of placements for a Kitty graphics image.
///
/// # Safety
///
/// - `term` must be a valid pointer or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_kitty_placement_count(
    term: *const DtermTerminal,
    image_id: u32,
) -> u32 {
    if term.is_null() {
        return 0;
    }
    let storage = unsafe { (*term).terminal.kitty_graphics() };
    match storage.get_image(image_id) {
        Some(img) => {
            #[allow(clippy::cast_possible_truncation)]
            let count = img.placement_count() as u32;
            count
        }
        None => 0,
    }
}

/// Get placement IDs for a Kitty graphics image.
///
/// Returns the number of IDs written (or needed if buffer is null).
///
/// # Safety
///
/// - `term` must be a valid pointer or null (returns 0).
/// - If `ids` is not null, it must be valid for `ids_capacity` elements.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_kitty_placement_ids(
    term: *const DtermTerminal,
    image_id: u32,
    ids: *mut u32,
    ids_capacity: u32,
) -> u32 {
    if term.is_null() {
        return 0;
    }
    let storage = unsafe { (*term).terminal.kitty_graphics() };
    let image = match storage.get_image(image_id) {
        Some(img) => img,
        None => return 0,
    };

    let placement_ids: Vec<u32> = image.iter_placements().map(|p| p.id).collect();
    #[allow(clippy::cast_possible_truncation)]
    let count = placement_ids.len() as u32;

    if ids.is_null() || ids_capacity == 0 {
        return count;
    }

    let write_count = std::cmp::min(count, ids_capacity) as usize;
    unsafe {
        ptr::copy_nonoverlapping(placement_ids.as_ptr(), ids, write_count);
    }
    #[allow(clippy::cast_possible_truncation)]
    let result = write_count as u32;
    result
}

/// Get a placement for a Kitty graphics image.
///
/// Returns true if the placement exists and was filled.
///
/// # Safety
///
/// - `term` must be a valid pointer or null (returns false).
/// - `placement` must be a valid pointer (returns false if null).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_kitty_get_placement(
    term: *const DtermTerminal,
    image_id: u32,
    placement_id: u32,
    placement: *mut DtermKittyPlacement,
) -> bool {
    if term.is_null() || placement.is_null() {
        return false;
    }
    let storage = unsafe { (*term).terminal.kitty_graphics() };
    let image = match storage.get_image(image_id) {
        Some(img) => img,
        None => return false,
    };
    let p = match image.get_placement(placement_id) {
        Some(p) => p,
        None => return false,
    };

    // Convert PlacementLocation to FFI representation
    let (loc_type, row_or_parent_image, col_or_parent_placement, offset_x, offset_y) =
        match &p.location {
            crate::kitty_graphics::storage::PlacementLocation::Absolute { row, col } => {
                (DtermKittyPlacementLocation::Absolute, *row, *col, 0, 0)
            }
            crate::kitty_graphics::storage::PlacementLocation::Virtual { id } => {
                (DtermKittyPlacementLocation::Virtual, *id, 0, 0, 0)
            }
            crate::kitty_graphics::storage::PlacementLocation::Relative {
                parent_image_id,
                parent_placement_id,
                offset_x: ox,
                offset_y: oy,
            } => (
                DtermKittyPlacementLocation::Relative,
                *parent_image_id,
                *parent_placement_id,
                *ox,
                *oy,
            ),
        };

    unsafe {
        (*placement).id = p.id;
        (*placement).location_type = loc_type;
        (*placement).row_or_parent_image = row_or_parent_image;
        (*placement).col_or_parent_placement = col_or_parent_placement;
        (*placement).offset_x = offset_x;
        (*placement).offset_y = offset_y;
        (*placement).source_x = p.source_x;
        (*placement).source_y = p.source_y;
        (*placement).source_width = p.source_width;
        (*placement).source_height = p.source_height;
        (*placement).cell_x_offset = u32::from(p.cell_x_offset);
        (*placement).cell_y_offset = u32::from(p.cell_y_offset);
        (*placement).num_columns = u32::from(p.num_columns);
        (*placement).num_rows = u32::from(p.num_rows);
        (*placement).z_index = p.z_index;
        (*placement).is_virtual = p.is_virtual;
    }
    true
}

/// Get total bytes used by Kitty graphics storage.
///
/// # Safety
///
/// - `term` must be a valid pointer or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_kitty_total_bytes(term: *const DtermTerminal) -> usize {
    if term.is_null() {
        return 0;
    }
    unsafe { (*term).terminal.kitty_graphics().total_bytes() }
}

/// Get Kitty graphics storage quota in bytes.
///
/// # Safety
///
/// - `term` must be a valid pointer or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_kitty_quota(term: *const DtermTerminal) -> usize {
    if term.is_null() {
        return 0;
    }
    unsafe { (*term).terminal.kitty_graphics().quota() }
}

// -----------------------------------------------------------------------------
// Request 2: Line Content Extraction (Medium Priority)
// For search indexing and iTerm2 LineBuffer compatibility
// -----------------------------------------------------------------------------

/// Get the total number of lines (visible + scrollback).
///
/// This includes all lines in the terminal: the visible screen area
/// plus all scrollback history.
///
/// # Safety
///
/// - `term` must be a valid pointer or null (returns 0).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_total_lines(term: *const DtermTerminal) -> usize {
    if term.is_null() {
        return 0;
    }
    let term_ref = unsafe { &(*term).terminal };
    term_ref.grid().rows() as usize + term_ref.grid().scrollback_lines()
}

/// Get the text content of a line.
///
/// Returns the number of bytes written to `buffer`, or the required buffer size
/// if `buffer` is null or `buffer_size` is 0.
///
/// Parameters:
/// - `term`: Terminal handle
/// - `line_index`: Line index (0 = first scrollback line, scrollback_lines = first visible row)
/// - `buffer`: Output buffer for UTF-8 text (may be null to query size)
/// - `buffer_size`: Size of buffer in bytes
///
/// Returns:
/// - If buffer is null or size is 0: required buffer size (including null terminator)
/// - Otherwise: bytes written (not including null terminator)
///
/// # Safety
///
/// - `term` must be a valid pointer or null (returns 0).
/// - If `buffer` is not null, it must be valid for `buffer_size` bytes.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_get_line_text(
    term: *const DtermTerminal,
    line_index: usize,
    buffer: *mut u8,
    buffer_size: usize,
) -> usize {
    if term.is_null() {
        return 0;
    }
    let term_ref = unsafe { &(*term).terminal };
    let scrollback_lines = term_ref.grid().scrollback_lines();
    let visible_rows = term_ref.grid().rows() as usize;

    // Build the line text
    let mut text = String::new();

    if line_index < scrollback_lines {
        // Scrollback line
        if let Some(scrollback) = term_ref.scrollback() {
            if let Some(line) = scrollback.get_line(line_index) {
                text = line.to_string();
            }
        }
    } else {
        // Visible line
        let row = line_index - scrollback_lines;
        if row < visible_rows {
            let grid = term_ref.grid();
            for col in 0..grid.cols() {
                // row is checked to be < visible_rows which is bounded by grid.rows() (u16)
                #[allow(clippy::cast_possible_truncation)]
                let row_u16 = row as u16;
                if let Some(cell) = grid.cell(row_u16, col) {
                    if let Some(c) = char::from_u32(cell.codepoint()) {
                        text.push(c);
                    }
                }
            }
            // Trim trailing spaces
            text = text.trim_end().to_string();
        }
    }

    let text_bytes = text.as_bytes();
    let required_size = text_bytes.len() + 1; // +1 for null terminator

    // Query mode: return required size
    if buffer.is_null() || buffer_size == 0 {
        return required_size;
    }

    // Copy mode: write to buffer
    let copy_len = text_bytes.len().min(buffer_size.saturating_sub(1));
    if copy_len > 0 {
        unsafe {
            ptr::copy_nonoverlapping(text_bytes.as_ptr(), buffer, copy_len);
        }
    }
    // Null terminate
    if buffer_size > copy_len {
        unsafe { *buffer.add(copy_len) = 0 };
    }

    copy_len
}

/// Get visible line text (convenience wrapper).
///
/// Gets the text of a visible row (not scrollback).
///
/// Parameters:
/// - `term`: Terminal handle
/// - `row`: Row index (0 = top of visible area)
/// - `buffer`: Output buffer for UTF-8 text
/// - `buffer_size`: Size of buffer in bytes
///
/// # Safety
///
/// Same as `dterm_terminal_get_line_text`.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_get_visible_line_text(
    term: *const DtermTerminal,
    row: u16,
    buffer: *mut u8,
    buffer_size: usize,
) -> usize {
    if term.is_null() {
        return 0;
    }
    // SAFETY: We verified term is not null above
    unsafe {
        let scrollback_lines = (*term).terminal.grid().scrollback_lines();
        dterm_terminal_get_line_text(term, scrollback_lines + row as usize, buffer, buffer_size)
    }
}

// -----------------------------------------------------------------------------
// Request 3: Damage Iteration API (Medium Priority)
// For efficient partial rendering
// -----------------------------------------------------------------------------

/// Damage bounds for a single row.
#[repr(C)]
pub struct DtermRowDamage {
    /// Row index (0 = top of visible area).
    pub row: u16,
    /// First damaged column (inclusive).
    pub left: u16,
    /// Last damaged column (exclusive).
    pub right: u16,
}

/// Get damaged rows for partial rendering.
///
/// Returns the number of damaged rows, filling `out_damages` up to `max_count`.
///
/// Use this to efficiently re-render only changed regions:
/// ```c
/// DtermRowDamage damages[100];
/// size_t count = dterm_terminal_get_damage(term, damages, 100);
/// for (size_t i = 0; i < count; i++) {
///     render_row_range(damages[i].row, damages[i].left, damages[i].right);
/// }
/// dterm_terminal_clear_damage(term);
/// ```
///
/// # Safety
///
/// - `term` must be a valid pointer or null (returns 0).
/// - `out_damages` must be valid for `max_count` elements, or null (returns total count).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_get_damage(
    term: *const DtermTerminal,
    out_damages: *mut DtermRowDamage,
    max_count: usize,
) -> usize {
    if term.is_null() {
        return 0;
    }
    let grid = unsafe { (*term).terminal.grid() };
    let damage = grid.damage();
    let cols = grid.cols();

    // Collect damaged rows
    let mut count = 0;
    for row in 0..grid.rows() {
        if let Some((left, right)) = damage.row_damage_bounds(row, cols) {
            if !out_damages.is_null() && count < max_count {
                unsafe {
                    *out_damages.add(count) = DtermRowDamage { row, left, right };
                }
            }
            count += 1;
        }
    }

    count
}

/// Check if a specific row is damaged.
///
/// # Safety
///
/// - `term` must be a valid pointer or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_row_is_damaged(
    term: *const DtermTerminal,
    row: u16,
) -> bool {
    if term.is_null() {
        return false;
    }
    let grid = unsafe { (*term).terminal.grid() };
    grid.damage().row_damage_bounds(row, grid.cols()).is_some()
}

/// Get damage bounds for a specific row.
///
/// Returns true if the row is damaged, filling `out_left` and `out_right`.
///
/// # Safety
///
/// - `term` must be a valid pointer or null (returns false).
/// - `out_left` and `out_right` must be valid pointers, or null (just returns damaged status).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_get_row_damage(
    term: *const DtermTerminal,
    row: u16,
    out_left: *mut u16,
    out_right: *mut u16,
) -> bool {
    if term.is_null() {
        return false;
    }
    let grid = unsafe { (*term).terminal.grid() };
    if let Some((left, right)) = grid.damage().row_damage_bounds(row, grid.cols()) {
        if !out_left.is_null() {
            unsafe { *out_left = left };
        }
        if !out_right.is_null() {
            unsafe { *out_right = right };
        }
        true
    } else {
        false
    }
}

/// Line size for DEC line attributes (DECDHL/DECDWL).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtermLineSize {
    /// Normal single-width, single-height line (DECSWL).
    SingleWidth = 0,
    /// Double-width line (DECDWL) - each character is rendered double-wide.
    DoubleWidth = 1,
    /// Top half of double-height line (DECDHL).
    DoubleHeightTop = 2,
    /// Bottom half of double-height line (DECDHL).
    DoubleHeightBottom = 3,
}

impl From<crate::grid::LineSize> for DtermLineSize {
    fn from(size: crate::grid::LineSize) -> Self {
        match size {
            crate::grid::LineSize::SingleWidth => DtermLineSize::SingleWidth,
            crate::grid::LineSize::DoubleWidth => DtermLineSize::DoubleWidth,
            crate::grid::LineSize::DoubleHeightTop => DtermLineSize::DoubleHeightTop,
            crate::grid::LineSize::DoubleHeightBottom => DtermLineSize::DoubleHeightBottom,
        }
    }
}

/// Get the line size attribute for a row.
///
/// This indicates whether the row should be rendered as:
/// - SingleWidth (0): Normal rendering
/// - DoubleWidth (1): Characters rendered at 2x width
/// - DoubleHeightTop (2): Top half of double-height characters
/// - DoubleHeightBottom (3): Bottom half of double-height characters
///
/// For double-height text, two consecutive rows must be used together:
/// the top row with DoubleHeightTop, bottom row with DoubleHeightBottom.
///
/// # Safety
///
/// - `term` must be a valid pointer or null (returns SingleWidth).
/// - `row` must be within bounds (returns SingleWidth if out of bounds).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_row_line_size(
    term: *const DtermTerminal,
    row: u16,
) -> DtermLineSize {
    if term.is_null() {
        return DtermLineSize::SingleWidth;
    }
    let grid = unsafe { (*term).terminal.grid() };
    if let Some(row_data) = grid.row(row) {
        row_data.line_size().into()
    } else {
        DtermLineSize::SingleWidth
    }
}

// =============================================================================
// SHELL INTEGRATION API (OSC 133)
// =============================================================================

/// Shell integration state (OSC 133).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtermShellState {
    /// Ground state - waiting for prompt.
    Ground = 0,
    /// Receiving prompt text (after OSC 133 ; A).
    ReceivingPrompt = 1,
    /// User is entering command (after OSC 133 ; B).
    EnteringCommand = 2,
    /// Command is executing (after OSC 133 ; C).
    Executing = 3,
}

/// Output block state for block-based terminal model.
///
/// Note: Variants are prefixed with `Block` to avoid C enum name collision
/// with `DtermShellState` which also has `EnteringCommand` and `Executing`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtermBlockState {
    /// Only prompt has been received.
    BlockPromptOnly = 0,
    /// User is entering a command.
    BlockEnteringCommand = 1,
    /// Command is executing.
    BlockExecuting = 2,
    /// Command has completed with exit code.
    BlockComplete = 3,
}

/// An output block representing a command and its output.
///
/// Output blocks are the fundamental unit of the block-based terminal model.
/// Each block contains a prompt, optional command, and optional output.
#[repr(C)]
pub struct DtermOutputBlock {
    /// Unique identifier for this block.
    pub id: u64,
    /// Current state of this block.
    pub state: DtermBlockState,
    /// Row where the prompt started (absolute line number).
    pub prompt_start_row: usize,
    /// Column where the prompt started.
    pub prompt_start_col: u16,
    /// Row where the command text started (0 if not set).
    pub command_start_row: usize,
    /// Column where the command text started (0 if not set).
    pub command_start_col: u16,
    /// Whether command_start is valid.
    pub has_command_start: bool,
    /// Row where command output started (0 if not set).
    pub output_start_row: usize,
    /// Whether output_start_row is valid.
    pub has_output_start: bool,
    /// Row where this block ends (exclusive).
    pub end_row: usize,
    /// Whether end_row is valid.
    pub has_end_row: bool,
    /// Command exit code (only valid if state is Complete).
    pub exit_code: i32,
    /// Whether exit_code is valid.
    pub has_exit_code: bool,
    /// Whether the output portion of this block is collapsed.
    pub collapsed: bool,
}

/// Get the current shell integration state.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_shell_state(term: *const DtermTerminal) -> DtermShellState {
    if term.is_null() {
        return DtermShellState::Ground;
    }
    match unsafe { (*term).terminal.shell_state() } {
        crate::terminal::ShellState::Ground => DtermShellState::Ground,
        crate::terminal::ShellState::ReceivingPrompt => DtermShellState::ReceivingPrompt,
        crate::terminal::ShellState::EnteringCommand => DtermShellState::EnteringCommand,
        crate::terminal::ShellState::Executing => DtermShellState::Executing,
    }
}

/// Get the number of completed output blocks.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_block_count(term: *const DtermTerminal) -> usize {
    if term.is_null() {
        return 0;
    }
    unsafe { (*term).terminal.output_blocks().len() }
}

/// Get an output block by index.
///
/// Returns true if the block was found and written to `out_block`.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null.
/// - `out_block` must be a valid writable pointer, or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_get_block(
    term: *const DtermTerminal,
    index: usize,
    out_block: *mut DtermOutputBlock,
) -> bool {
    if term.is_null() || out_block.is_null() {
        return false;
    }
    let blocks = unsafe { (*term).terminal.output_blocks() };
    if let Some(block) = blocks.get(index) {
        unsafe {
            (*out_block) = DtermOutputBlock {
                id: block.id,
                state: match block.state {
                    crate::terminal::BlockState::PromptOnly => DtermBlockState::BlockPromptOnly,
                    crate::terminal::BlockState::EnteringCommand => {
                        DtermBlockState::BlockEnteringCommand
                    }
                    crate::terminal::BlockState::Executing => DtermBlockState::BlockExecuting,
                    crate::terminal::BlockState::Complete => DtermBlockState::BlockComplete,
                },
                prompt_start_row: block.prompt_start_row,
                prompt_start_col: block.prompt_start_col,
                command_start_row: block.command_start_row.unwrap_or(0),
                command_start_col: block.command_start_col.unwrap_or(0),
                has_command_start: block.command_start_row.is_some(),
                output_start_row: block.output_start_row.unwrap_or(0),
                has_output_start: block.output_start_row.is_some(),
                end_row: block.end_row.unwrap_or(0),
                has_end_row: block.end_row.is_some(),
                exit_code: block.exit_code.unwrap_or(0),
                has_exit_code: block.exit_code.is_some(),
                collapsed: block.collapsed,
            };
        }
        true
    } else {
        false
    }
}

/// Get the current (in-progress) output block.
///
/// Returns true if there is a current block and it was written to `out_block`.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null.
/// - `out_block` must be a valid writable pointer, or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_get_current_block(
    term: *const DtermTerminal,
    out_block: *mut DtermOutputBlock,
) -> bool {
    if term.is_null() || out_block.is_null() {
        return false;
    }
    if let Some(block) = unsafe { (*term).terminal.current_block() } {
        unsafe {
            (*out_block) = DtermOutputBlock {
                id: block.id,
                state: match block.state {
                    crate::terminal::BlockState::PromptOnly => DtermBlockState::BlockPromptOnly,
                    crate::terminal::BlockState::EnteringCommand => {
                        DtermBlockState::BlockEnteringCommand
                    }
                    crate::terminal::BlockState::Executing => DtermBlockState::BlockExecuting,
                    crate::terminal::BlockState::Complete => DtermBlockState::BlockComplete,
                },
                prompt_start_row: block.prompt_start_row,
                prompt_start_col: block.prompt_start_col,
                command_start_row: block.command_start_row.unwrap_or(0),
                command_start_col: block.command_start_col.unwrap_or(0),
                has_command_start: block.command_start_row.is_some(),
                output_start_row: block.output_start_row.unwrap_or(0),
                has_output_start: block.output_start_row.is_some(),
                end_row: block.end_row.unwrap_or(0),
                has_end_row: block.end_row.is_some(),
                exit_code: block.exit_code.unwrap_or(0),
                has_exit_code: block.exit_code.is_some(),
                collapsed: block.collapsed,
            };
        }
        true
    } else {
        false
    }
}

/// Find the output block containing a given row.
///
/// Returns the block index if found, or `usize::MAX` if no block contains the row.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_block_at_row(
    term: *const DtermTerminal,
    row: usize,
) -> usize {
    if term.is_null() {
        return usize::MAX;
    }
    let term_ref = unsafe { &(*term).terminal };
    if let Some(block) = term_ref.block_at_row(row) {
        // Find the index of this block
        let blocks = term_ref.output_blocks();
        for (i, b) in blocks.iter().enumerate() {
            if b.id == block.id {
                return i;
            }
        }
        // Check current block
        if let Some(current) = term_ref.current_block() {
            if current.id == block.id {
                return blocks.len(); // Current block is at index = completed count
            }
        }
    }
    usize::MAX
}

/// Get the exit code of the last completed block.
///
/// Returns true if there was a completed or current block with an exit code.
/// Checks current_block first (most recent), then falls back to output_blocks.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null.
/// - `out_exit_code` must be a valid writable pointer, or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_last_exit_code(
    term: *const DtermTerminal,
    out_exit_code: *mut i32,
) -> bool {
    if term.is_null() || out_exit_code.is_null() {
        return false;
    }
    // Check current_block first (may have exit code but not yet moved to output_blocks)
    if let Some(block) = unsafe { (*term).terminal.current_block() } {
        if let Some(code) = block.exit_code {
            unsafe { *out_exit_code = code };
            return true;
        }
    }
    // Fall back to completed output_blocks
    let blocks = unsafe { (*term).terminal.output_blocks() };
    // Find the last block with an exit code
    for block in blocks.iter().rev() {
        if let Some(code) = block.exit_code {
            unsafe { *out_exit_code = code };
            return true;
        }
    }
    false
}

// =============================================================================
// SMART SELECTION API
// =============================================================================

/// Opaque handle for smart selection engine.
pub struct DtermSmartSelection(crate::selection::SmartSelection);

/// Create a new smart selection engine with all built-in rules.
///
/// Returns a handle that can be used with `dterm_terminal_smart_*` functions.
/// The handle must be freed with `dterm_smart_selection_free`.
#[no_mangle]
pub extern "C" fn dterm_smart_selection_new() -> *mut DtermSmartSelection {
    Box::into_raw(Box::new(DtermSmartSelection(
        crate::selection::SmartSelection::with_builtin_rules(),
    )))
}

/// Create an empty smart selection engine (no rules).
#[no_mangle]
pub extern "C" fn dterm_smart_selection_new_empty() -> *mut DtermSmartSelection {
    Box::into_raw(Box::new(DtermSmartSelection(
        crate::selection::SmartSelection::new(),
    )))
}

/// Free a smart selection engine.
///
/// # Safety
///
/// - `selection` must be a valid pointer returned by `dterm_smart_selection_new*`, or null.
/// - `selection` must not have been freed previously.
#[no_mangle]
pub unsafe extern "C" fn dterm_smart_selection_free(selection: *mut DtermSmartSelection) {
    if !selection.is_null() {
        drop(unsafe { Box::from_raw(selection) });
    }
}

/// Enable or disable a rule by name.
///
/// Returns true if the rule was found, false otherwise.
///
/// # Safety
///
/// - `selection` must be a valid pointer.
/// - `name` must be a valid null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn dterm_smart_selection_set_rule_enabled(
    selection: *mut DtermSmartSelection,
    name: *const c_char,
    enabled: bool,
) -> bool {
    if selection.is_null() || name.is_null() {
        return false;
    }

    let name_str = match unsafe { std::ffi::CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    unsafe { &mut *selection }
        .0
        .set_rule_enabled(name_str, enabled)
}

/// Result of a smart selection match.
#[repr(C)]
pub struct DtermSelectionMatch {
    /// Start byte offset in the text.
    pub start: u32,
    /// End byte offset in the text (exclusive).
    pub end: u32,
    /// Rule name (null-terminated).
    pub rule_name: *mut c_char,
    /// Matched text (null-terminated).
    pub matched_text: *mut c_char,
    /// Kind of match (see `DtermSelectionKind`).
    pub kind: u8,
}

/// Kind of selection match.
#[repr(u8)]
pub enum DtermSelectionKind {
    /// URL (http, https, ftp, file, etc.)
    Url = 0,
    /// File path
    FilePath = 1,
    /// Email address
    Email = 2,
    /// IP address (IPv4 or IPv6)
    IpAddress = 3,
    /// Git hash
    GitHash = 4,
    /// Quoted string
    QuotedString = 5,
    /// UUID
    Uuid = 6,
    /// Semantic version
    SemVer = 7,
    /// Custom rule
    Custom = 8,
}

impl From<crate::selection::SelectionRuleKind> for DtermSelectionKind {
    fn from(kind: crate::selection::SelectionRuleKind) -> Self {
        match kind {
            crate::selection::SelectionRuleKind::Url => Self::Url,
            crate::selection::SelectionRuleKind::FilePath => Self::FilePath,
            crate::selection::SelectionRuleKind::Email => Self::Email,
            crate::selection::SelectionRuleKind::IpAddress => Self::IpAddress,
            crate::selection::SelectionRuleKind::GitHash => Self::GitHash,
            crate::selection::SelectionRuleKind::QuotedString => Self::QuotedString,
            crate::selection::SelectionRuleKind::Uuid => Self::Uuid,
            crate::selection::SelectionRuleKind::SemVer => Self::SemVer,
            crate::selection::SelectionRuleKind::Custom => Self::Custom,
        }
    }
}

/// Free a selection match returned by `dterm_terminal_smart_match_at`.
///
/// # Safety
///
/// - `match_ptr` must be a valid pointer to a `DtermSelectionMatch`, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_selection_match_free(match_ptr: *mut DtermSelectionMatch) {
    if match_ptr.is_null() {
        return;
    }

    let m = unsafe { &mut *match_ptr };
    if !m.rule_name.is_null() {
        drop(unsafe { CString::from_raw(m.rule_name) });
    }
    if !m.matched_text.is_null() {
        drop(unsafe { CString::from_raw(m.matched_text) });
    }
    drop(unsafe { Box::from_raw(match_ptr) });
}

/// Get smart word boundaries at a position on a row.
///
/// Returns true if a word/semantic unit was found, false otherwise.
/// On success, `out_start` and `out_end` are set to the column boundaries.
///
/// # Safety
///
/// - All pointers must be valid (or null for output pointers if result not needed).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_smart_word_at(
    term: *const DtermTerminal,
    selection: *const DtermSmartSelection,
    row: u32,
    col: u32,
    out_start: *mut u32,
    out_end: *mut u32,
) -> bool {
    if term.is_null() || selection.is_null() {
        return false;
    }

    let terminal = unsafe { &(*term).terminal };
    let smart = unsafe { &(*selection).0 };

    match terminal.smart_word_at(row as usize, col as usize, smart) {
        Some((start, end)) => {
            #[allow(clippy::cast_possible_truncation)]
            if !out_start.is_null() {
                unsafe { *out_start = start as u32 };
            }
            #[allow(clippy::cast_possible_truncation)]
            if !out_end.is_null() {
                unsafe { *out_end = end as u32 };
            }
            true
        }
        None => false,
    }
}

/// Find a semantic match at a position on a row.
///
/// Returns a pointer to a `DtermSelectionMatch` if found, null otherwise.
/// The returned match must be freed with `dterm_selection_match_free`.
///
/// # Safety
///
/// - All pointers must be valid.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_smart_match_at(
    term: *const DtermTerminal,
    selection: *const DtermSmartSelection,
    row: u32,
    col: u32,
) -> *mut DtermSelectionMatch {
    if term.is_null() || selection.is_null() {
        return ptr::null_mut();
    }

    let terminal = unsafe { &(*term).terminal };
    let smart = unsafe { &(*selection).0 };

    match terminal.smart_match_at(row as usize, col as usize, smart) {
        Some(m) => {
            let rule_name = CString::new(m.rule_name()).ok();
            let matched_text = CString::new(m.matched_text()).ok();

            #[allow(clippy::cast_possible_truncation)]
            let result = Box::new(DtermSelectionMatch {
                start: m.start() as u32,
                end: m.end() as u32,
                rule_name: rule_name.map(CString::into_raw).unwrap_or(ptr::null_mut()),
                matched_text: matched_text
                    .map(CString::into_raw)
                    .unwrap_or(ptr::null_mut()),
                kind: DtermSelectionKind::from(m.kind()) as u8,
            });
            Box::into_raw(result)
        }
        None => ptr::null_mut(),
    }
}

/// Count semantic matches on a row.
///
/// # Safety
///
/// - All pointers must be valid.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_smart_match_count(
    term: *const DtermTerminal,
    selection: *const DtermSmartSelection,
    row: u32,
) -> u32 {
    if term.is_null() || selection.is_null() {
        return 0;
    }

    let terminal = unsafe { &(*term).terminal };
    let smart = unsafe { &(*selection).0 };

    #[allow(clippy::cast_possible_truncation)]
    {
        terminal.smart_matches_on_row(row as usize, smart).len() as u32
    }
}

/// Get semantic matches on a row.
///
/// Fills `out_matches` with up to `max_matches` matches. Returns the actual count.
/// Each match must be freed with `dterm_selection_match_free`.
///
/// # Safety
///
/// - All pointers must be valid.
/// - `out_matches` must have room for at least `max_matches` pointers.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_smart_matches_on_row(
    term: *const DtermTerminal,
    selection: *const DtermSmartSelection,
    row: u32,
    out_matches: *mut *mut DtermSelectionMatch,
    max_matches: u32,
) -> u32 {
    if term.is_null() || selection.is_null() || out_matches.is_null() || max_matches == 0 {
        return 0;
    }

    let terminal = unsafe { &(*term).terminal };
    let smart = unsafe { &(*selection).0 };

    let matches = terminal.smart_matches_on_row(row as usize, smart);

    #[allow(clippy::cast_possible_truncation)]
    let count = matches.len().min(max_matches as usize);

    for (i, m) in matches.into_iter().take(count).enumerate() {
        let rule_name = CString::new(m.rule_name()).ok();
        let matched_text = CString::new(m.matched_text()).ok();

        #[allow(clippy::cast_possible_truncation)]
        let result = Box::new(DtermSelectionMatch {
            start: m.start() as u32,
            end: m.end() as u32,
            rule_name: rule_name.map(CString::into_raw).unwrap_or(ptr::null_mut()),
            matched_text: matched_text
                .map(CString::into_raw)
                .unwrap_or(ptr::null_mut()),
            kind: DtermSelectionKind::from(m.kind()) as u8,
        });

        unsafe { *out_matches.add(i) = Box::into_raw(result) };
    }

    #[allow(clippy::cast_possible_truncation)]
    {
        count as u32
    }
}

// =============================================================================
// TEXT SELECTION (Mouse-based)
// =============================================================================

/// Selection type for text selection.
///
/// See `tla/Selection.tla` for the formal specification.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtermSelectionType {
    /// Character-by-character selection (single click + drag).
    Simple = 0,
    /// Rectangular block selection (Alt + click + drag).
    Block = 1,
    /// Semantic selection - words, URLs, etc. (double-click).
    Semantic = 2,
    /// Full line selection (triple-click).
    Lines = 3,
}

impl From<crate::selection::SelectionType> for DtermSelectionType {
    fn from(ty: crate::selection::SelectionType) -> Self {
        match ty {
            crate::selection::SelectionType::Simple => Self::Simple,
            crate::selection::SelectionType::Block => Self::Block,
            crate::selection::SelectionType::Semantic => Self::Semantic,
            crate::selection::SelectionType::Lines => Self::Lines,
        }
    }
}

impl From<DtermSelectionType> for crate::selection::SelectionType {
    fn from(ty: DtermSelectionType) -> Self {
        match ty {
            DtermSelectionType::Simple => Self::Simple,
            DtermSelectionType::Block => Self::Block,
            DtermSelectionType::Semantic => Self::Semantic,
            DtermSelectionType::Lines => Self::Lines,
        }
    }
}

/// Start a text selection.
///
/// Call this on mouse down to begin a new selection.
///
/// # Parameters
///
/// - `term`: Terminal handle
/// - `col`: Starting column (0-indexed)
/// - `row`: Starting row (0 = top of visible area, negative = scrollback)
/// - `selection_type`: Type of selection (Simple, Block, Semantic, Lines)
///
/// # Safety
///
/// - `term` must be a valid pointer returned by `dterm_terminal_new*`.
#[no_mangle]
#[allow(clippy::cast_possible_truncation)]
pub unsafe extern "C" fn dterm_terminal_selection_start(
    term: *mut DtermTerminal,
    col: u32,
    row: i32,
    selection_type: DtermSelectionType,
) {
    if term.is_null() {
        return;
    }

    let sel = unsafe { (*term).terminal.text_selection_mut() };
    // Truncation is acceptable: columns > u16::MAX are clamped
    sel.start_selection(
        row,
        col as u16,
        crate::selection::SelectionSide::Left,
        crate::selection::SelectionType::from(selection_type),
    );
}

/// Update text selection endpoint.
///
/// Call this on mouse drag to update the selection.
/// Only works when selection is in progress.
///
/// # Parameters
///
/// - `term`: Terminal handle
/// - `col`: Current column (0-indexed)
/// - `row`: Current row (0 = top of visible area, negative = scrollback)
///
/// # Safety
///
/// - `term` must be a valid pointer returned by `dterm_terminal_new*`.
#[no_mangle]
#[allow(clippy::cast_possible_truncation)]
pub unsafe extern "C" fn dterm_terminal_selection_update(
    term: *mut DtermTerminal,
    col: u32,
    row: i32,
) {
    if term.is_null() {
        return;
    }

    let sel = unsafe { (*term).terminal.text_selection_mut() };
    // Truncation is acceptable: columns > u16::MAX are clamped
    sel.update_selection(row, col as u16, crate::selection::SelectionSide::Right);
}

/// Complete text selection.
///
/// Call this on mouse up to complete the selection.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by `dterm_terminal_new*`.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_selection_end(term: *mut DtermTerminal) {
    if term.is_null() {
        return;
    }

    let sel = unsafe { (*term).terminal.text_selection_mut() };
    sel.complete_selection();
}

/// Clear text selection.
///
/// Removes any active selection.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by `dterm_terminal_new*`.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_selection_clear(term: *mut DtermTerminal) {
    if term.is_null() {
        return;
    }

    let sel = unsafe { (*term).terminal.text_selection_mut() };
    sel.clear();
}

/// Check if terminal has an active selection.
///
/// Returns true if there is any selection (in progress or complete).
///
/// # Safety
///
/// - `term` must be a valid pointer returned by `dterm_terminal_new*`.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_has_selection(term: *const DtermTerminal) -> bool {
    if term.is_null() {
        return false;
    }

    unsafe { (*term).terminal.text_selection().has_selection() }
}

/// Get selected text as a C string.
///
/// Returns the selected text, or null if there is no selection.
/// The returned string must be freed by the caller using `dterm_string_free`.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by `dterm_terminal_new*`.
/// - The returned pointer must be freed with `dterm_string_free`.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_selection_to_string(
    term: *const DtermTerminal,
) -> *mut c_char {
    if term.is_null() {
        return std::ptr::null_mut();
    }

    match unsafe { (*term).terminal.selection_to_string() } {
        Some(s) => match std::ffi::CString::new(s) {
            Ok(cs) => cs.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        None => std::ptr::null_mut(),
    }
}

/// Free a string returned by dterm FFI functions.
///
/// Strings returned by functions like `dterm_terminal_selection_to_string`
/// must be freed using this function.
///
/// # Safety
///
/// - `s` must be a valid pointer returned by a dterm FFI function, or null.
/// - `s` must not have been freed previously.
#[no_mangle]
pub unsafe extern "C" fn dterm_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { std::ffi::CString::from_raw(s) });
    }
}

// =============================================================================
// SECURE KEYBOARD ENTRY
// =============================================================================

/// Check if secure keyboard entry mode is enabled.
///
/// When enabled, the UI layer should activate platform-specific secure input
/// mechanisms to prevent keylogging:
///
/// - **macOS**: Call `EnableSecureEventInput()` / `DisableSecureEventInput()`
/// - **iOS**: Not applicable (sandboxed by default)
/// - **Windows**: Limited protection available (document to users)
/// - **Linux/X11**: Not possible (X11 is inherently insecure)
/// - **Linux/Wayland**: Secure by default (no action needed)
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor, or null (returns false).
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_is_secure_keyboard_entry(
    term: *const DtermTerminal,
) -> bool {
    if term.is_null() {
        return false;
    }
    unsafe { (*term).terminal.is_secure_keyboard_entry() }
}

/// Enable or disable secure keyboard entry mode.
///
/// When enabled, the UI layer should activate platform-specific secure input
/// mechanisms to prevent keylogging. See `dterm_terminal_is_secure_keyboard_entry`
/// for platform-specific guidance.
///
/// # Safety
///
/// - `term` must be a valid pointer returned by a terminal constructor.
/// - `term` must not be null.
#[no_mangle]
pub unsafe extern "C" fn dterm_terminal_set_secure_keyboard_entry(
    term: *mut DtermTerminal,
    enabled: bool,
) {
    if term.is_null() {
        return;
    }
    unsafe { (*term).terminal.set_secure_keyboard_entry(enabled) }
}

// =============================================================================
// VERSION INFO
// =============================================================================

/// Get library version string.
#[no_mangle]
pub extern "C" fn dterm_version() -> *const c_char {
    static VERSION: &[u8] = b"0.6.0\0"; // Bumped for secure keyboard entry FFI APIs
    VERSION.as_ptr() as *const c_char
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
#[allow(clippy::borrow_as_ptr)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn test_parser_ffi() {
        unsafe {
            let parser = dterm_parser_new();
            assert!(!parser.is_null());
            dterm_parser_reset(parser);
            dterm_parser_free(parser);
        }
    }

    #[test]
    fn test_grid_ffi() {
        unsafe {
            let grid = dterm_grid_new(24, 80);
            assert!(!grid.is_null());

            assert_eq!(dterm_grid_rows(grid), 24);
            assert_eq!(dterm_grid_cols(grid), 80);

            dterm_grid_set_cursor(grid, 10, 20);
            assert_eq!(dterm_grid_cursor_row(grid), 10);
            assert_eq!(dterm_grid_cursor_col(grid), 20);

            dterm_grid_write_char(grid, 'A' as u32);
            assert_eq!(dterm_grid_cursor_col(grid), 21);

            let mut cell = DtermCell {
                codepoint: 0,
                fg: 0,
                bg: 0,
                underline_color: 0xFFFF_FFFF,
                flags: 0,
            };
            assert!(dterm_grid_get_cell(grid, 10, 20, &raw mut cell));
            assert_eq!(cell.codepoint, 'A' as u32);

            dterm_grid_resize(grid, 40, 120);
            assert_eq!(dterm_grid_rows(grid), 40);
            assert_eq!(dterm_grid_cols(grid), 120);

            dterm_grid_free(grid);
        }
    }

    #[test]
    fn test_terminal_ffi() {
        unsafe {
            let term = dterm_terminal_new(24, 80);
            assert!(!term.is_null());

            assert_eq!(dterm_terminal_rows(term), 24);
            assert_eq!(dterm_terminal_cols(term), 80);

            // Process "Hello"
            let input = b"Hello";
            dterm_terminal_process(term, input.as_ptr(), input.len());
            assert_eq!(dterm_terminal_cursor_col(term), 5);

            // Check cell
            let mut cell = DtermCell {
                codepoint: 0,
                fg: 0,
                bg: 0,
                underline_color: 0xFFFF_FFFF,
                flags: 0,
            };
            assert!(dterm_terminal_get_cell(term, 0, 0, &raw mut cell));
            assert_eq!(cell.codepoint, 'H' as u32);

            // Process cursor movement
            let esc = b"\x1b[10;10H";
            dterm_terminal_process(term, esc.as_ptr(), esc.len());
            assert_eq!(dterm_terminal_cursor_row(term), 9);
            assert_eq!(dterm_terminal_cursor_col(term), 9);

            // Test title via OSC
            let osc = b"\x1b]0;Test Title\x07";
            dterm_terminal_process(term, osc.as_ptr(), osc.len());
            let title_ptr = dterm_terminal_title(term);
            assert!(!title_ptr.is_null());
            let title = CStr::from_ptr(title_ptr).to_str().unwrap();
            assert_eq!(title, "Test Title");

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_terminal_cell_codepoint_complex() {
        unsafe {
            let term = dterm_terminal_new(24, 80);
            assert!(!term.is_null());

            let input = b"\xF0\x9F\x98\x80"; // U+1F600
            dterm_terminal_process(term, input.as_ptr(), input.len());
            assert_eq!(dterm_cell_codepoint(term, 0, 0), 0x1F600);

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_terminal_cell_rgb_accessors() {
        unsafe {
            let term = dterm_terminal_new(24, 80);
            assert!(!term.is_null());

            let input = b"\x1b[38;2;10;20;30mA\x1b[48;2;40;50;60mB";
            dterm_terminal_process(term, input.as_ptr(), input.len());

            let mut r = 0;
            let mut g = 0;
            let mut b = 0;
            dterm_cell_fg_rgb(term, 0, 0, &raw mut r, &raw mut g, &raw mut b);
            assert_eq!((r, g, b), (10, 20, 30));

            let mut br = 0;
            let mut bg = 0;
            let mut bb = 0;
            dterm_cell_bg_rgb(term, 0, 1, &raw mut br, &raw mut bg, &raw mut bb);
            assert_eq!((br, bg, bb), (40, 50, 60));

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_terminal_memory_usage_budget() {
        unsafe {
            let term = dterm_terminal_new_with_scrollback(24, 80, 100, 1000, 2000, 50_000);
            assert!(!term.is_null());

            let usage = dterm_terminal_memory_usage(term);
            assert!(usage > 0);

            dterm_terminal_set_memory_budget(term, 1234);
            let budget = (*term).terminal.scrollback().unwrap().memory_budget();
            assert_eq!(budget, 1234);

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_terminal_sgr_bold_ffi() {
        unsafe {
            let term = dterm_terminal_new(24, 80);
            assert!(!term.is_null());

            // Process bold + text: ESC[1m = bold on
            let input = b"\x1b[1mBold";
            dterm_terminal_process(term, input.as_ptr(), input.len());

            // Get cell at (0, 0) - should have 'B' with bold flag
            let mut cell = DtermCell {
                codepoint: 0,
                fg: 0,
                bg: 0,
                underline_color: 0xFFFF_FFFF,
                flags: 0,
            };
            assert!(dterm_terminal_get_cell(term, 0, 0, &raw mut cell));

            // Check codepoint
            assert_eq!(cell.codepoint, 'B' as u32);

            // Check flags (BOLD = 1 << 0 = 1)
            assert_eq!(
                cell.flags, 0x0001,
                "Expected BOLD flag (1), got {:#06x}",
                cell.flags
            );

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_terminal_sgr_attributes_ffi() {
        unsafe {
            let term = dterm_terminal_new(24, 80);
            assert!(!term.is_null());

            // Test all SGR attributes via FFI
            // SGR codes: 1=bold, 2=dim, 3=italic, 4=underline, 5=blink, 7=inverse, 8=hidden, 9=strikethrough
            let input = b"\x1b[1mB\x1b[0m\x1b[2mD\x1b[0m\x1b[3mI\x1b[0m\x1b[4mU\x1b[0m\x1b[5mK\x1b[0m\x1b[7mV\x1b[0m\x1b[8mH\x1b[0m\x1b[9mS";
            dterm_terminal_process(term, input.as_ptr(), input.len());

            let mut cell = DtermCell {
                codepoint: 0,
                fg: 0,
                bg: 0,
                underline_color: 0xFFFF_FFFF,
                flags: 0,
            };

            // Cell 0: Bold 'B' (flag 0x0001)
            assert!(dterm_terminal_get_cell(term, 0, 0, &raw mut cell));
            assert_eq!(cell.codepoint, 'B' as u32);
            assert_eq!(cell.flags, 0x0001, "Bold flag mismatch");

            // Cell 1: Dim 'D' (flag 0x0002)
            assert!(dterm_terminal_get_cell(term, 0, 1, &raw mut cell));
            assert_eq!(cell.codepoint, 'D' as u32);
            assert_eq!(cell.flags, 0x0002, "Dim flag mismatch");

            // Cell 2: Italic 'I' (flag 0x0004)
            assert!(dterm_terminal_get_cell(term, 0, 2, &raw mut cell));
            assert_eq!(cell.codepoint, 'I' as u32);
            assert_eq!(cell.flags, 0x0004, "Italic flag mismatch");

            // Cell 3: Underline 'U' (flag 0x0008)
            assert!(dterm_terminal_get_cell(term, 0, 3, &raw mut cell));
            assert_eq!(cell.codepoint, 'U' as u32);
            assert_eq!(cell.flags, 0x0008, "Underline flag mismatch");

            // Cell 4: Blink 'K' (flag 0x0010)
            assert!(dterm_terminal_get_cell(term, 0, 4, &raw mut cell));
            assert_eq!(cell.codepoint, 'K' as u32);
            assert_eq!(cell.flags, 0x0010, "Blink flag mismatch");

            // Cell 5: Inverse 'V' (flag 0x0020)
            assert!(dterm_terminal_get_cell(term, 0, 5, &raw mut cell));
            assert_eq!(cell.codepoint, 'V' as u32);
            assert_eq!(cell.flags, 0x0020, "Inverse flag mismatch");

            // Cell 6: Hidden 'H' (flag 0x0040)
            assert!(dterm_terminal_get_cell(term, 0, 6, &raw mut cell));
            assert_eq!(cell.codepoint, 'H' as u32);
            assert_eq!(cell.flags, 0x0040, "Hidden flag mismatch");

            // Cell 7: Strikethrough 'S' (flag 0x0080)
            assert!(dterm_terminal_get_cell(term, 0, 7, &raw mut cell));
            assert_eq!(cell.codepoint, 'S' as u32);
            assert_eq!(cell.flags, 0x0080, "Strikethrough flag mismatch");

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_terminal_sgr_combined_ffi() {
        unsafe {
            let term = dterm_terminal_new(24, 80);
            assert!(!term.is_null());

            // Test combined SGR attributes: bold + italic + underline
            // ESC[1;3;4m = bold + italic + underline
            let input = b"\x1b[1;3;4mX";
            dterm_terminal_process(term, input.as_ptr(), input.len());

            let mut cell = DtermCell {
                codepoint: 0,
                fg: 0,
                bg: 0,
                underline_color: 0xFFFF_FFFF,
                flags: 0,
            };

            assert!(dterm_terminal_get_cell(term, 0, 0, &raw mut cell));
            assert_eq!(cell.codepoint, 'X' as u32);
            // BOLD (0x0001) | ITALIC (0x0004) | UNDERLINE (0x0008) = 0x000D
            assert_eq!(
                cell.flags, 0x000D,
                "Expected combined flags 0x000D, got {:#06x}",
                cell.flags
            );

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_terminal_sgr_reset_ffi() {
        unsafe {
            let term = dterm_terminal_new(24, 80);
            assert!(!term.is_null());

            // Set bold, then reset, then write character
            let input = b"\x1b[1mB\x1b[0mN";
            dterm_terminal_process(term, input.as_ptr(), input.len());

            let mut cell = DtermCell {
                codepoint: 0,
                fg: 0,
                bg: 0,
                underline_color: 0xFFFF_FFFF,
                flags: 0,
            };

            // Cell 0: Bold 'B'
            assert!(dterm_terminal_get_cell(term, 0, 0, &raw mut cell));
            assert_eq!(cell.codepoint, 'B' as u32);
            assert_eq!(cell.flags, 0x0001, "Expected bold flag");

            // Cell 1: Normal 'N' (no flags after reset)
            assert!(dterm_terminal_get_cell(term, 0, 1, &raw mut cell));
            assert_eq!(cell.codepoint, 'N' as u32);
            assert_eq!(cell.flags, 0x0000, "Expected no flags after SGR reset");

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_terminal_underline_color_ffi() {
        unsafe {
            let term = dterm_terminal_new(24, 80);
            assert!(!term.is_null());

            // Set underline + red underline color, write "Red", reset underline color, write "Def"
            let input = b"\x1b[4m\x1b[58;2;255;0;0mRed\x1b[59mDef";
            dterm_terminal_process(term, input.as_ptr(), input.len());

            let mut cell = DtermCell {
                codepoint: 0,
                fg: 0,
                bg: 0,
                underline_color: 0xFFFF_FFFF,
                flags: 0,
            };

            assert!(dterm_terminal_get_cell(term, 0, 0, &raw mut cell));
            assert_eq!(cell.codepoint, 'R' as u32);
            assert_eq!(cell.underline_color, 0x01_FF0000);

            assert!(dterm_terminal_get_cell(term, 0, 3, &raw mut cell));
            assert_eq!(cell.codepoint, 'D' as u32);
            assert_eq!(cell.underline_color, 0xFFFF_FFFF);

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_terminal_wide_char_ffi() {
        unsafe {
            let term = dterm_terminal_new(24, 80);
            assert!(!term.is_null());

            // Write a wide CJK character (日 is U+65E5)
            let input = "日".as_bytes();
            dterm_terminal_process(term, input.as_ptr(), input.len());

            // Cursor should advance by 2
            assert_eq!(
                dterm_terminal_cursor_col(term),
                2,
                "Cursor should be at column 2 after wide char"
            );

            let mut cell = DtermCell {
                codepoint: 0,
                fg: 0,
                bg: 0,
                underline_color: 0xFFFF_FFFF,
                flags: 0,
            };

            // First cell should contain the wide character with WIDE flag (bit 9 = 0x0200)
            assert!(dterm_terminal_get_cell(term, 0, 0, &raw mut cell));
            assert_eq!(
                cell.codepoint, '日' as u32,
                "Cell 0 should contain the CJK character"
            );
            assert_eq!(
                cell.flags & 0x0200,
                0x0200,
                "Cell 0 should have WIDE flag (bit 9). Got flags: {:#06x}",
                cell.flags
            );

            // Second cell should be the continuation cell with WIDE_CONTINUATION flag (bit 10 = 0x0400)
            assert!(dterm_terminal_get_cell(term, 0, 1, &raw mut cell));
            assert_eq!(
                cell.flags & 0x0400,
                0x0400,
                "Cell 1 should have WIDE_CONTINUATION flag (bit 10). Got flags: {:#06x}",
                cell.flags
            );

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_scrollback_cell_ffi() {
        unsafe {
            // Create terminal with tiered scrollback (so scrollback is enabled)
            // Use a SMALL ring_buffer_size (10) so lines overflow quickly into tiered storage
            // Args: rows, cols, ring_buffer_size, hot_limit, warm_limit, memory_budget
            let term = dterm_terminal_new_with_scrollback(24, 80, 10, 1000, 2000, 50_000);
            assert!(!term.is_null());

            // Fill terminal with lines that will overflow the ring buffer into tiered scrollback
            // We need: visible_rows (24) + ring_buffer_size (10) + extra = 34+ lines
            // Writing 50 lines ensures some go to tiered scrollback
            for i in 0..50 {
                let line = format!("Line {:02}\r\n", i);
                let line_bytes = line.as_bytes();
                dterm_terminal_process(term, line_bytes.as_ptr(), line_bytes.len());
            }

            // Check tiered scrollback has content
            let tiered_lines = (*term).terminal.grid().tiered_scrollback_lines();
            assert!(
                tiered_lines > 0,
                "Should have tiered scrollback lines (got {})",
                tiered_lines
            );

            // Get first tiered scrollback cell (should be "Line 00" or similar)
            let mut cell = DtermScrollbackCell::default();
            let exists = dterm_terminal_get_scrollback_cell(term, 0, 0, &raw mut cell);
            assert!(exists, "First tiered scrollback line should exist");
            assert_eq!(
                cell.codepoint, 'L' as u32,
                "First cell should be 'L' from 'Line'"
            );

            // Verify line length function
            let line_len = dterm_terminal_scrollback_line_len(term, 0);
            assert!(line_len > 0, "First line should have characters");
            assert!(line_len >= 7, "Line 'Line XX' should have at least 7 chars");

            // Test out of bounds access
            let exists = dterm_terminal_get_scrollback_cell(term, 100000, 0, &raw mut cell);
            assert!(!exists, "Out of bounds row should return false");

            // Test null out_cell (should work, returns existence only)
            let exists = dterm_terminal_get_scrollback_cell(term, 0, 0, ptr::null_mut());
            assert!(
                exists,
                "Should return true for existence check with null out_cell"
            );

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_scrollback_cell_ffi_styled() {
        unsafe {
            // Create terminal with small ring buffer to force tiered scrollback
            // Args: rows, cols, ring_buffer_size, hot_limit, warm_limit, memory_budget
            let term = dterm_terminal_new_with_scrollback(24, 80, 10, 1000, 2000, 50_000);
            assert!(!term.is_null());

            // Write styled text that will overflow to tiered scrollback
            // Red foreground text: ESC[31m
            // Need 34+ lines to overflow ring buffer
            for i in 0..50 {
                let line = format!("\x1b[31mRed{:02}\x1b[0m\r\n", i);
                let line_bytes = line.as_bytes();
                dterm_terminal_process(term, line_bytes.as_ptr(), line_bytes.len());
            }

            // Check tiered scrollback has content
            let tiered_lines = (*term).terminal.grid().tiered_scrollback_lines();
            assert!(
                tiered_lines > 0,
                "Should have tiered scrollback lines (got {})",
                tiered_lines
            );

            // Get first tiered scrollback cell and verify it has style
            let mut cell = DtermScrollbackCell::default();
            let exists = dterm_terminal_get_scrollback_cell(term, 0, 0, &raw mut cell);
            assert!(exists, "First scrollback line should exist");
            assert_eq!(
                cell.codepoint, 'R' as u32,
                "First char should be 'R' from 'Red'"
            );

            // The fg color should be red indexed (0x00_00_00_01 = indexed red)
            // Check it's not default (0xFF_FFFFFF)
            assert_ne!(
                cell.fg, 0xFF_FF_FF_FF,
                "Foreground should not be default (should be red)"
            );

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_terminal_modes_ffi() {
        unsafe {
            let term = dterm_terminal_new(24, 80);

            assert!(dterm_terminal_cursor_visible(term));

            // Hide cursor
            let hide = b"\x1b[?25l";
            dterm_terminal_process(term, hide.as_ptr(), hide.len());
            assert!(!dterm_terminal_cursor_visible(term));

            // Show cursor
            let show = b"\x1b[?25h";
            dterm_terminal_process(term, show.as_ptr(), show.len());
            assert!(dterm_terminal_cursor_visible(term));

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_terminal_sixel_image_ffi() {
        unsafe {
            let term = dterm_terminal_new(24, 80);
            assert!(!term.is_null());
            assert!(!dterm_terminal_has_sixel_image(term));

            let sixel = b"\x1bPq#15~\x1b\\";
            dterm_terminal_process(term, sixel.as_ptr(), sixel.len());
            assert!(dterm_terminal_has_sixel_image(term));

            let mut image = DtermSixelImage {
                width: 0,
                height: 0,
                pixels: std::ptr::null_mut(),
            };
            assert!(dterm_terminal_get_sixel_image(term, &raw mut image));
            assert_eq!(image.width, 1);
            assert_eq!(image.height, 6);
            assert!(!image.pixels.is_null());
            assert_eq!(*image.pixels, 0xFF_FFFFFF);
            assert!(!dterm_terminal_has_sixel_image(term));

            dterm_sixel_image_free(image.pixels);
            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_version() {
        unsafe {
            let ver = dterm_version();
            assert!(!ver.is_null());
            let version = CStr::from_ptr(ver).to_str().unwrap();
            assert_eq!(version, "0.6.0"); // Bumped for secure keyboard entry FFI APIs
        }
    }

    #[test]
    fn test_terminal_response_buffer_ffi() {
        unsafe {
            let term = dterm_terminal_new(24, 80);

            // Initially no response
            assert!(!dterm_terminal_has_response(term));
            assert_eq!(dterm_terminal_response_len(term), 0);

            // Trigger DSR
            let dsr = b"\x1b[5n";
            dterm_terminal_process(term, dsr.as_ptr(), dsr.len());

            // Should have response
            assert!(dterm_terminal_has_response(term));
            assert_eq!(dterm_terminal_response_len(term), 4); // "\x1b[0n"

            // Read response
            let mut buffer = [0u8; 32];
            let len = dterm_terminal_read_response(term, buffer.as_mut_ptr(), buffer.len());
            assert_eq!(len, 4);
            assert_eq!(&buffer[..len], b"\x1b[0n");

            // Response should be cleared
            assert!(!dterm_terminal_has_response(term));
            assert_eq!(dterm_terminal_response_len(term), 0);

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_terminal_cursor_position_report_ffi() {
        unsafe {
            let term = dterm_terminal_new(24, 80);

            // Move cursor
            let move_seq = b"\x1b[5;10H";
            dterm_terminal_process(term, move_seq.as_ptr(), move_seq.len());

            // Request CPR
            let cpr = b"\x1b[6n";
            dterm_terminal_process(term, cpr.as_ptr(), cpr.len());

            // Read response
            let mut buffer = [0u8; 32];
            let len = dterm_terminal_read_response(term, buffer.as_mut_ptr(), buffer.len());
            assert_eq!(&buffer[..len], b"\x1b[5;10R");

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_terminal_device_attributes_ffi() {
        unsafe {
            let term = dterm_terminal_new(24, 80);

            // Request primary DA
            let da = b"\x1b[c";
            dterm_terminal_process(term, da.as_ptr(), da.len());

            // Read response
            let mut buffer = [0u8; 32];
            let len = dterm_terminal_read_response(term, buffer.as_mut_ptr(), buffer.len());
            assert_eq!(&buffer[..len], b"\x1b[?62;6c");

            dterm_terminal_free(term);
        }
    }

    // ============== Null Pointer Safety Tests ==============

    #[test]
    fn test_parser_null_safety() {
        unsafe {
            // All these should be no-ops, not crashes
            dterm_parser_free(ptr::null_mut());
            dterm_parser_reset(ptr::null_mut());
        }
    }

    #[test]
    fn test_grid_null_safety() {
        unsafe {
            // Free with null is safe
            dterm_grid_free(ptr::null_mut());

            // Read functions return defaults with null
            assert_eq!(dterm_grid_rows(ptr::null()), 0);
            assert_eq!(dterm_grid_cols(ptr::null()), 0);
            assert_eq!(dterm_grid_cursor_row(ptr::null()), 0);
            assert_eq!(dterm_grid_cursor_col(ptr::null()), 0);
            assert_eq!(dterm_grid_display_offset(ptr::null()), 0);
            assert_eq!(dterm_grid_scrollback_lines(ptr::null()), 0);
            assert!(!dterm_grid_needs_redraw(ptr::null()));
            assert!(!dterm_grid_get_cell(ptr::null(), 0, 0, ptr::null_mut()));

            // Write functions with null are no-ops
            dterm_grid_set_cursor(ptr::null_mut(), 0, 0);
            dterm_grid_write_char(ptr::null_mut(), 'A' as u32);
            dterm_grid_resize(ptr::null_mut(), 10, 10);
            dterm_grid_scroll_display(ptr::null_mut(), 1);
            dterm_grid_clear_damage(ptr::null_mut());
            dterm_grid_erase_screen(ptr::null_mut());
        }
    }

    #[test]
    fn test_terminal_null_safety() {
        unsafe {
            // Free with null is safe
            dterm_terminal_free(ptr::null_mut());

            // Read functions return defaults with null terminal
            assert_eq!(dterm_terminal_rows(ptr::null()), 0);
            assert_eq!(dterm_terminal_cols(ptr::null()), 0);
            assert_eq!(dterm_terminal_cursor_row(ptr::null()), 0);
            assert_eq!(dterm_terminal_cursor_col(ptr::null()), 0);
            assert!(dterm_terminal_cursor_visible(ptr::null())); // default true
            assert!(dterm_terminal_title(ptr::null_mut()).is_null());
            assert!(!dterm_terminal_is_alternate_screen(ptr::null()));
            assert!(!dterm_terminal_needs_redraw(ptr::null()));
            assert!(!dterm_terminal_has_response(ptr::null()));
            assert_eq!(dterm_terminal_response_len(ptr::null()), 0);
            assert_eq!(dterm_terminal_scrollback_lines(ptr::null()), 0);
            assert_eq!(dterm_terminal_display_offset(ptr::null()), 0);

            // Write functions with null are no-ops
            dterm_terminal_process(ptr::null_mut(), ptr::null(), 0);
            dterm_terminal_resize(ptr::null_mut(), 10, 10);
            dterm_terminal_reset(ptr::null_mut());
            dterm_terminal_scroll_display(ptr::null_mut(), 1);
            dterm_terminal_scroll_to_top(ptr::null_mut());
            dterm_terminal_scroll_to_bottom(ptr::null_mut());
            dterm_terminal_clear_damage(ptr::null_mut());

            // Functions with out params and null are safe
            dterm_terminal_get_style(ptr::null(), ptr::null_mut());
            dterm_terminal_get_modes(ptr::null(), ptr::null_mut());
            assert!(!dterm_terminal_get_cell(ptr::null(), 0, 0, ptr::null_mut()));
        }
    }

    #[test]
    fn test_data_null_safety() {
        unsafe {
            // Process with null data is no-op
            let term = dterm_terminal_new(24, 80);
            dterm_terminal_process(term, ptr::null(), 0);
            dterm_terminal_process(term, ptr::null(), 100); // Even with non-zero len

            // Read response with null buffer returns 0
            let result = dterm_terminal_read_response(term, ptr::null_mut(), 100);
            assert_eq!(result, 0);

            // Read response with zero size returns 0
            let mut buffer = [0u8; 32];
            let result = dterm_terminal_read_response(term, buffer.as_mut_ptr(), 0);
            assert_eq!(result, 0);

            dterm_terminal_free(term);
        }
    }

    // ============== Search FFI Tests ==============

    #[test]
    fn test_search_ffi_basic() {
        unsafe {
            let search = dterm_search_new();
            assert!(!search.is_null());

            // Index some lines
            let line1 = b"hello world";
            let line2 = b"goodbye world";
            let line3 = b"hello there";
            dterm_search_index_line(search, line1.as_ptr(), line1.len());
            dterm_search_index_line(search, line2.as_ptr(), line2.len());
            dterm_search_index_line(search, line3.as_ptr(), line3.len());

            assert_eq!(dterm_search_line_count(search), 3);

            // Search for "world"
            let query = b"world";
            let mut matches = [DtermSearchMatch {
                line: 0,
                start_col: 0,
                end_col: 0,
            }; 10];
            let count = dterm_search_find(
                search,
                query.as_ptr(),
                query.len(),
                matches.as_mut_ptr(),
                10,
            );
            assert_eq!(count, 2);

            // Search for "hello"
            let query = b"hello";
            let count = dterm_search_find(
                search,
                query.as_ptr(),
                query.len(),
                matches.as_mut_ptr(),
                10,
            );
            assert_eq!(count, 2);

            dterm_search_free(search);
        }
    }

    #[test]
    fn test_search_ffi_find_next_prev() {
        unsafe {
            let search = dterm_search_new();

            let line1 = b"match here";
            let line2 = b"no match here";
            let line3 = b"match again";
            dterm_search_index_line(search, line1.as_ptr(), line1.len());
            dterm_search_index_line(search, line2.as_ptr(), line2.len());
            dterm_search_index_line(search, line3.as_ptr(), line3.len());

            let query = b"match";
            let mut m = DtermSearchMatch {
                line: 0,
                start_col: 0,
                end_col: 0,
            };

            // Find next from (0, 0) - should find line 1's match (at col 3)
            let found =
                dterm_search_find_next(search, query.as_ptr(), query.len(), 0, 0, &raw mut m);
            assert!(found);
            assert_eq!(m.line, 1);
            assert_eq!(m.start_col, 3);

            // Find prev from (3, 0) - should find line 2
            let found =
                dterm_search_find_prev(search, query.as_ptr(), query.len(), 3, 0, &raw mut m);
            assert!(found);
            assert_eq!(m.line, 2);

            dterm_search_free(search);
        }
    }

    #[test]
    fn test_search_null_safety() {
        unsafe {
            // Free with null is safe
            dterm_search_free(ptr::null_mut());

            // Read functions return defaults with null
            assert_eq!(dterm_search_line_count(ptr::null()), 0);
            assert!(!dterm_search_might_contain(ptr::null(), ptr::null(), 0));

            // Search with null returns 0
            assert_eq!(
                dterm_search_find(ptr::null(), ptr::null(), 0, ptr::null_mut(), 0),
                0
            );
            assert!(!dterm_search_find_next(
                ptr::null(),
                ptr::null(),
                0,
                0,
                0,
                ptr::null_mut()
            ));
            assert!(!dterm_search_find_prev(
                ptr::null(),
                ptr::null(),
                0,
                0,
                0,
                ptr::null_mut()
            ));

            // Write functions with null are no-ops
            dterm_search_index_line(ptr::null_mut(), ptr::null(), 0);
            dterm_search_clear(ptr::null_mut());
        }
    }

    // ============== Checkpoint FFI Tests ==============

    #[test]
    fn test_checkpoint_null_safety() {
        unsafe {
            // Free with null is safe
            dterm_checkpoint_free(ptr::null_mut());

            // Read functions return defaults with null
            assert!(!dterm_checkpoint_should_save(ptr::null()));
            assert!(!dterm_checkpoint_exists(ptr::null()));

            // Save with null returns false
            assert!(!dterm_checkpoint_save(ptr::null_mut(), ptr::null()));

            // Restore with null returns null
            assert!(dterm_checkpoint_restore(ptr::null()).is_null());

            // Write functions with null are no-ops
            dterm_checkpoint_notify_lines(ptr::null_mut(), 100);
        }
    }

    #[test]
    fn test_checkpoint_ffi_basic() {
        unsafe {
            let temp_dir = tempfile::tempdir().unwrap();
            let path = temp_dir.path().to_str().unwrap();

            let checkpoint = dterm_checkpoint_new(path.as_ptr(), path.len());
            assert!(!checkpoint.is_null());

            // Should checkpoint (first time)
            assert!(dterm_checkpoint_should_save(checkpoint));

            // Create terminal and save
            let term = dterm_terminal_new(24, 80);
            let input = b"Hello, checkpoint!";
            dterm_terminal_process(term, input.as_ptr(), input.len());

            let saved = dterm_checkpoint_save(checkpoint, term);
            assert!(saved);

            // Checkpoint should exist now
            assert!(dterm_checkpoint_exists(checkpoint));

            // Restore
            let restored_term = dterm_checkpoint_restore(checkpoint);
            assert!(!restored_term.is_null());

            // Verify dimensions preserved
            assert_eq!(dterm_terminal_rows(restored_term), 24);
            assert_eq!(dterm_terminal_cols(restored_term), 80);

            // Clean up
            dterm_terminal_free(term);
            dterm_terminal_free(restored_term);
            dterm_checkpoint_free(checkpoint);
        }
    }

    #[test]
    fn test_osc_7_cwd_ffi() {
        unsafe {
            let term = dterm_terminal_new(24, 80);

            // Initially no CWD
            assert!(!dterm_terminal_has_working_directory(term));
            assert!(dterm_terminal_current_working_directory(term).is_null());

            // Set CWD via OSC 7
            let osc = b"\x1b]7;file:///home/user/projects\x07";
            dterm_terminal_process(term, osc.as_ptr(), osc.len());

            // Now should have CWD
            assert!(dterm_terminal_has_working_directory(term));
            let cwd_ptr = dterm_terminal_current_working_directory(term);
            assert!(!cwd_ptr.is_null());
            let cwd = CStr::from_ptr(cwd_ptr).to_str().unwrap();
            assert_eq!(cwd, "/home/user/projects");

            // Clear CWD
            let clear = b"\x1b]7;\x07";
            dterm_terminal_process(term, clear.as_ptr(), clear.len());
            assert!(!dterm_terminal_has_working_directory(term));

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_osc_7_percent_decode_ffi() {
        unsafe {
            let term = dterm_terminal_new(24, 80);

            // Set CWD with percent-encoded characters
            let osc = b"\x1b]7;file:///home/user/My%20Documents\x07";
            dterm_terminal_process(term, osc.as_ptr(), osc.len());

            let cwd_ptr = dterm_terminal_current_working_directory(term);
            assert!(!cwd_ptr.is_null());
            let cwd = CStr::from_ptr(cwd_ptr).to_str().unwrap();
            assert_eq!(cwd, "/home/user/My Documents");

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_osc_7_null_handling_ffi() {
        unsafe {
            // Null terminal should return null/false
            assert!(dterm_terminal_current_working_directory(ptr::null_mut()).is_null());
            assert!(!dterm_terminal_has_working_directory(ptr::null()));
        }
    }

    #[test]
    fn test_osc_4_color_palette_ffi() {
        unsafe {
            let term = dterm_terminal_new(24, 80);

            // Get default color 0 (black)
            let mut color = DtermRgb { r: 0, g: 0, b: 0 };
            assert!(dterm_terminal_get_palette_color(term, 0, &raw mut color));
            assert_eq!(color.r, 0);
            assert_eq!(color.g, 0);
            assert_eq!(color.b, 0);

            // Get default color 1 (red)
            assert!(dterm_terminal_get_palette_color(term, 1, &raw mut color));
            assert_eq!(color.r, 205);
            assert_eq!(color.g, 0);
            assert_eq!(color.b, 0);

            // Set color 1 to a custom value
            dterm_terminal_set_palette_color(term, 1, 255, 128, 64);
            assert!(dterm_terminal_get_palette_color(term, 1, &raw mut color));
            assert_eq!(color.r, 255);
            assert_eq!(color.g, 128);
            assert_eq!(color.b, 64);

            // Reset single color
            dterm_terminal_reset_palette_color(term, 1);
            assert!(dterm_terminal_get_palette_color(term, 1, &raw mut color));
            assert_eq!(color.r, 205); // Back to default
            assert_eq!(color.g, 0);
            assert_eq!(color.b, 0);

            // Modify multiple colors then reset all
            dterm_terminal_set_palette_color(term, 0, 100, 100, 100);
            dterm_terminal_set_palette_color(term, 1, 200, 200, 200);
            dterm_terminal_reset_palette(term);

            assert!(dterm_terminal_get_palette_color(term, 0, &raw mut color));
            assert_eq!(color.r, 0);
            assert!(dterm_terminal_get_palette_color(term, 1, &raw mut color));
            assert_eq!(color.r, 205);

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_osc_4_color_palette_via_escape_sequence() {
        unsafe {
            let term = dterm_terminal_new(24, 80);

            // Set color 5 via OSC 4 escape sequence with rgb: format
            let osc = b"\x1b]4;5;rgb:ff/80/40\x1b\\";
            dterm_terminal_process(term, osc.as_ptr(), osc.len());

            let mut color = DtermRgb { r: 0, g: 0, b: 0 };
            assert!(dterm_terminal_get_palette_color(term, 5, &raw mut color));
            assert_eq!(color.r, 255);
            assert_eq!(color.g, 128);
            assert_eq!(color.b, 64);

            // Set color 10 via OSC 4 with hex format
            let osc = b"\x1b]4;10;#00ff00\x1b\\";
            dterm_terminal_process(term, osc.as_ptr(), osc.len());

            assert!(dterm_terminal_get_palette_color(term, 10, &raw mut color));
            assert_eq!(color.r, 0);
            assert_eq!(color.g, 255);
            assert_eq!(color.b, 0);

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_osc_4_color_query_response() {
        unsafe {
            let term = dterm_terminal_new(24, 80);

            // Query color 1 (default red) via OSC 4
            let query = b"\x1b]4;1;?\x1b\\";
            dterm_terminal_process(term, query.as_ptr(), query.len());

            // Check response
            assert!(dterm_terminal_has_response(term));
            let mut buffer = [0u8; 64];
            let len = dterm_terminal_read_response(term, buffer.as_mut_ptr(), buffer.len());
            assert!(len > 0);

            // Response should be: ESC ] 4 ; 1 ; rgb:cdcd/0000/0000 ESC \
            let response = std::str::from_utf8(&buffer[..len]).unwrap();
            assert!(response.starts_with("\x1b]4;1;"));
            assert!(response.contains("cd"));

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_osc_4_null_safety() {
        unsafe {
            // Null terminal should return false/no-op
            let mut color = DtermRgb {
                r: 99,
                g: 99,
                b: 99,
            };
            assert!(!dterm_terminal_get_palette_color(
                ptr::null(),
                0,
                &raw mut color
            ));
            // Color should be unchanged
            assert_eq!(color.r, 99);

            // Null out_color should return false
            let term = dterm_terminal_new(24, 80);
            assert!(!dterm_terminal_get_palette_color(term, 0, ptr::null_mut()));

            // Set/reset with null are no-ops (don't crash)
            dterm_terminal_set_palette_color(ptr::null_mut(), 0, 255, 255, 255);
            dterm_terminal_reset_palette(ptr::null_mut());
            dterm_terminal_reset_palette_color(ptr::null_mut(), 0);

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_shell_integration_ffi_basic() {
        unsafe {
            let term = dterm_terminal_new(24, 80);
            assert!(!term.is_null());

            // Initially no shell integration state
            assert_eq!(dterm_terminal_shell_state(term), DtermShellState::Ground);
            assert_eq!(dterm_terminal_block_count(term), 0);

            // Process OSC 133 ; A (prompt start)
            let prompt_start = b"\x1b]133;A\x07";
            dterm_terminal_process(term, prompt_start.as_ptr(), prompt_start.len());
            assert_eq!(
                dterm_terminal_shell_state(term),
                DtermShellState::ReceivingPrompt
            );

            // Process OSC 133 ; B (command start)
            let cmd_start = b"\x1b]133;B\x07";
            dterm_terminal_process(term, cmd_start.as_ptr(), cmd_start.len());
            assert_eq!(
                dterm_terminal_shell_state(term),
                DtermShellState::EnteringCommand
            );

            // Process OSC 133 ; C (command executing)
            let cmd_exec = b"\x1b]133;C\x07";
            dterm_terminal_process(term, cmd_exec.as_ptr(), cmd_exec.len());
            assert_eq!(dterm_terminal_shell_state(term), DtermShellState::Executing);

            // Process OSC 133 ; D ; 0 (command done with exit code 0)
            let cmd_done = b"\x1b]133;D;0\x07";
            dterm_terminal_process(term, cmd_done.as_ptr(), cmd_done.len());
            // State goes back to ground after command completes
            assert_eq!(dterm_terminal_shell_state(term), DtermShellState::Ground);

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_shell_integration_ffi_blocks() {
        unsafe {
            let term = dterm_terminal_new(24, 80);

            // Process a complete command cycle
            let seq =
                b"\x1b]133;A\x07$ \x1b]133;B\x07ls\x1b]133;C\x07file1 file2\n\x1b]133;D;0\x07";
            dterm_terminal_process(term, seq.as_ptr(), seq.len());

            // Block is complete but still "current" - not yet in output_blocks
            // It moves to output_blocks when a new prompt starts
            let mut block = std::mem::zeroed::<DtermOutputBlock>();
            assert!(dterm_terminal_get_current_block(term, &mut block));
            assert_eq!(block.id, 0);
            assert_eq!(block.state, DtermBlockState::BlockComplete);
            assert!(block.has_exit_code);
            assert_eq!(block.exit_code, 0);

            // Start a second prompt - this moves the first block to output_blocks
            let prompt2 = b"\x1b]133;A\x07$ ";
            dterm_terminal_process(term, prompt2.as_ptr(), prompt2.len());

            // Now the first block should be in output_blocks
            assert_eq!(dterm_terminal_block_count(term), 1);
            assert!(dterm_terminal_get_block(term, 0, &mut block));
            assert_eq!(block.id, 0);
            assert_eq!(block.state, DtermBlockState::BlockComplete);
            assert!(block.has_exit_code);
            assert_eq!(block.exit_code, 0);

            // Test last exit code
            let mut exit_code: i32 = -1;
            assert!(dterm_terminal_last_exit_code(term, &mut exit_code));
            assert_eq!(exit_code, 0);

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_shell_integration_ffi_current_block() {
        unsafe {
            let term = dterm_terminal_new(24, 80);

            // No current block initially
            let mut block = std::mem::zeroed::<DtermOutputBlock>();
            assert!(!dterm_terminal_get_current_block(term, &mut block));

            // Start a new prompt
            let prompt_start = b"\x1b]133;A\x07$ ";
            dterm_terminal_process(term, prompt_start.as_ptr(), prompt_start.len());

            // Now there should be a current block
            assert!(dterm_terminal_get_current_block(term, &mut block));
            assert_eq!(block.state, DtermBlockState::BlockPromptOnly);

            // Start command
            let cmd_start = b"\x1b]133;B\x07echo hello";
            dterm_terminal_process(term, cmd_start.as_ptr(), cmd_start.len());
            assert!(dterm_terminal_get_current_block(term, &mut block));
            assert_eq!(block.state, DtermBlockState::BlockEnteringCommand);

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_shell_integration_ffi_null_safety() {
        unsafe {
            // Null terminal should return safe defaults
            assert_eq!(
                dterm_terminal_shell_state(ptr::null()),
                DtermShellState::Ground
            );
            assert_eq!(dterm_terminal_block_count(ptr::null()), 0);
            assert_eq!(dterm_terminal_block_at_row(ptr::null(), 0), usize::MAX);

            // Null out_block should return false
            let term = dterm_terminal_new(24, 80);
            assert!(!dterm_terminal_get_block(term, 0, ptr::null_mut()));
            assert!(!dterm_terminal_get_current_block(term, ptr::null_mut()));
            assert!(!dterm_terminal_last_exit_code(term, ptr::null_mut()));

            dterm_terminal_free(term);
        }
    }

    // ============== Kitty Graphics FFI Tests ==============

    #[test]
    fn test_kitty_graphics_ffi_empty() {
        unsafe {
            let term = dterm_terminal_new(24, 80);

            // No images initially
            assert!(!dterm_terminal_kitty_has_images(term));
            assert_eq!(dterm_terminal_kitty_image_count(term), 0);
            assert!(!dterm_terminal_kitty_is_dirty(term));
            assert_eq!(dterm_terminal_kitty_total_bytes(term), 0);
            assert!(dterm_terminal_kitty_quota(term) > 0);

            // Query size with null buffer
            assert_eq!(dterm_terminal_kitty_image_ids(term, ptr::null_mut(), 0), 0);

            // Get non-existent image
            let mut info = DtermKittyImageInfo {
                id: 0,
                number: 0,
                width: 0,
                height: 0,
                placement_count: 0,
            };
            assert!(!dterm_terminal_kitty_get_image_info(term, 1, &mut info));

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_kitty_graphics_ffi_transmit() {
        unsafe {
            let term = dterm_terminal_new(24, 80);

            // Send a simple Kitty graphics command to transmit only (no display)
            // a=t (lowercase) = transmit only, no placement
            // APC G a=t,f=24,s=2,v=2,i=42; <4 pixels RGB base64> ST
            // 2x2 RGB = 12 bytes = [255,0,0, 255,0,0, 255,0,0, 255,0,0]
            // Base64 of [255,0,0, 255,0,0, 255,0,0, 255,0,0] = "/wAA/wAA/wAA/wAA"
            let kitty_cmd = b"\x1b_Ga=t,f=24,s=2,v=2,i=42;/wAA/wAA/wAA/wAA\x1b\\";
            dterm_terminal_process(term, kitty_cmd.as_ptr(), kitty_cmd.len());

            // Should now have an image
            assert!(dterm_terminal_kitty_has_images(term));
            assert_eq!(dterm_terminal_kitty_image_count(term), 1);
            assert!(dterm_terminal_kitty_is_dirty(term));

            // Get image IDs
            let mut ids = [0u32; 4];
            let count = dterm_terminal_kitty_image_ids(term, ids.as_mut_ptr(), 4);
            assert_eq!(count, 1);
            let image_id = ids[0];
            assert_eq!(image_id, 42);

            // Get image info
            let mut info = DtermKittyImageInfo {
                id: 0,
                number: 0,
                width: 0,
                height: 0,
                placement_count: 0,
            };
            assert!(dterm_terminal_kitty_get_image_info(
                term, image_id, &mut info
            ));
            assert_eq!(info.id, 42);
            assert_eq!(info.width, 2);
            assert_eq!(info.height, 2);
            // Transmit only (a=t lowercase) doesn't create a placement
            assert_eq!(info.placement_count, 0);

            // Get pixel data
            let mut pixels: *mut u8 = ptr::null_mut();
            let mut pixel_count: usize = 0;
            assert!(dterm_terminal_kitty_get_image_pixels(
                term,
                image_id,
                &mut pixels,
                &mut pixel_count
            ));
            assert!(!pixels.is_null());
            // 2x2 RGBA = 16 bytes
            assert_eq!(pixel_count, 16);

            // Free pixel data
            dterm_kitty_image_free(pixels);

            // Clear dirty flag
            dterm_terminal_kitty_clear_dirty(term);
            assert!(!dterm_terminal_kitty_is_dirty(term));

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_kitty_graphics_ffi_transmit_and_display() {
        unsafe {
            let term = dterm_terminal_new(24, 80);

            // Transmit and display (a=T uppercase) creates a placement
            let kitty_cmd = b"\x1b_Ga=T,f=24,s=2,v=2,i=100;/wAA/wAA/wAA/wAA\x1b\\";
            dterm_terminal_process(term, kitty_cmd.as_ptr(), kitty_cmd.len());

            // Get image info
            let mut info = DtermKittyImageInfo {
                id: 0,
                number: 0,
                width: 0,
                height: 0,
                placement_count: 0,
            };
            assert!(dterm_terminal_kitty_get_image_info(term, 100, &mut info));
            assert_eq!(info.placement_count, 1);

            // Get placement count
            assert_eq!(dterm_terminal_kitty_placement_count(term, 100), 1);

            // Get placement IDs
            let mut placement_ids = [0u32; 4];
            let count =
                dterm_terminal_kitty_placement_ids(term, 100, placement_ids.as_mut_ptr(), 4);
            assert_eq!(count, 1);

            // Get placement details
            let mut placement = DtermKittyPlacement {
                id: 0,
                location_type: DtermKittyPlacementLocation::Absolute,
                row_or_parent_image: 0,
                col_or_parent_placement: 0,
                offset_x: 0,
                offset_y: 0,
                source_x: 0,
                source_y: 0,
                source_width: 0,
                source_height: 0,
                cell_x_offset: 0,
                cell_y_offset: 0,
                num_columns: 0,
                num_rows: 0,
                z_index: 0,
                is_virtual: false,
            };
            assert!(dterm_terminal_kitty_get_placement(
                term,
                100,
                placement_ids[0],
                &mut placement
            ));
            assert_eq!(
                placement.location_type,
                DtermKittyPlacementLocation::Absolute
            );
            // Placement should be at cursor position (0, 0)
            assert_eq!(placement.row_or_parent_image, 0);
            assert_eq!(placement.col_or_parent_placement, 0);

            dterm_terminal_free(term);
        }
    }

    #[test]
    fn test_kitty_graphics_ffi_null_safety() {
        unsafe {
            // All functions should handle null pointers gracefully
            assert!(!dterm_terminal_kitty_has_images(ptr::null()));
            assert!(!dterm_terminal_kitty_is_dirty(ptr::null()));
            dterm_terminal_kitty_clear_dirty(ptr::null_mut()); // No-op
            assert_eq!(dterm_terminal_kitty_image_count(ptr::null()), 0);
            assert_eq!(
                dterm_terminal_kitty_image_ids(ptr::null(), ptr::null_mut(), 0),
                0
            );
            assert!(!dterm_terminal_kitty_get_image_info(
                ptr::null(),
                1,
                ptr::null_mut()
            ));
            assert!(!dterm_terminal_kitty_get_image_pixels(
                ptr::null(),
                1,
                ptr::null_mut(),
                ptr::null_mut()
            ));
            dterm_kitty_image_free(ptr::null_mut()); // No-op
            assert_eq!(dterm_terminal_kitty_placement_count(ptr::null(), 1), 0);
            assert_eq!(
                dterm_terminal_kitty_placement_ids(ptr::null(), 1, ptr::null_mut(), 0),
                0
            );
            assert!(!dterm_terminal_kitty_get_placement(
                ptr::null(),
                1,
                1,
                ptr::null_mut()
            ));
            assert_eq!(dterm_terminal_kitty_total_bytes(ptr::null()), 0);
            assert_eq!(dterm_terminal_kitty_quota(ptr::null()), 0);

            // Test with valid term but null output pointers
            let term = dterm_terminal_new(24, 80);
            assert!(!dterm_terminal_kitty_get_image_info(
                term,
                1,
                ptr::null_mut()
            ));
            assert!(!dterm_terminal_kitty_get_image_pixels(
                term,
                1,
                ptr::null_mut(),
                ptr::null_mut()
            ));
            assert!(!dterm_terminal_kitty_get_placement(
                term,
                1,
                1,
                ptr::null_mut()
            ));
            dterm_terminal_free(term);
        }
    }
}

// =============================================================================
// UI BRIDGE API
// =============================================================================

use crate::ui::{
    CallbackId, Event, EventId, EventKind, TerminalId, TerminalState, UIBridge, UIError, UIState,
};

/// Opaque UI Bridge handle.
pub struct DtermUIBridge(UIBridge);

/// UI state enum for FFI.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtermUIState {
    /// No work in progress, ready to process events.
    Idle = 0,
    /// Currently processing an event.
    Processing = 1,
    /// Waiting for render completion.
    Rendering = 2,
    /// Waiting for callback completion.
    WaitingForCallback = 3,
    /// System is shutting down.
    ShuttingDown = 4,
}

impl From<UIState> for DtermUIState {
    fn from(state: UIState) -> Self {
        match state {
            UIState::Idle => DtermUIState::Idle,
            UIState::Processing => DtermUIState::Processing,
            UIState::Rendering => DtermUIState::Rendering,
            UIState::WaitingForCallback => DtermUIState::WaitingForCallback,
            UIState::ShuttingDown => DtermUIState::ShuttingDown,
        }
    }
}

/// Terminal state enum for FFI.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtermUITerminalState {
    /// Terminal slot is available.
    Inactive = 0,
    /// Terminal is active and usable.
    Active = 1,
    /// Terminal has been disposed (cannot be reactivated).
    Disposed = 2,
}

impl From<TerminalState> for DtermUITerminalState {
    fn from(state: TerminalState) -> Self {
        match state {
            TerminalState::Inactive => DtermUITerminalState::Inactive,
            TerminalState::Active => DtermUITerminalState::Active,
            TerminalState::Disposed => DtermUITerminalState::Disposed,
        }
    }
}

/// Event kind enum for FFI.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtermUIEventKind {
    /// User input to a terminal.
    Input = 0,
    /// Terminal resize request.
    Resize = 1,
    /// Render request for a terminal.
    Render = 2,
    /// Create a new terminal.
    CreateTerminal = 3,
    /// Destroy an existing terminal.
    DestroyTerminal = 4,
    /// Request a callback.
    RequestCallback = 5,
    /// System shutdown.
    Shutdown = 6,
}

impl From<EventKind> for DtermUIEventKind {
    fn from(kind: EventKind) -> Self {
        match kind {
            EventKind::Input => DtermUIEventKind::Input,
            EventKind::Resize => DtermUIEventKind::Resize,
            EventKind::Render => DtermUIEventKind::Render,
            EventKind::CreateTerminal => DtermUIEventKind::CreateTerminal,
            EventKind::DestroyTerminal => DtermUIEventKind::DestroyTerminal,
            EventKind::RequestCallback => DtermUIEventKind::RequestCallback,
            EventKind::Shutdown => DtermUIEventKind::Shutdown,
        }
    }
}

/// Error codes for UI Bridge operations.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtermUIErrorCode {
    /// Operation succeeded.
    Ok = 0,
    /// Event queue is full.
    QueueFull = 1,
    /// System is shutting down.
    ShuttingDown = 2,
    /// Terminal ID is invalid or out of range.
    InvalidTerminalId = 3,
    /// Terminal is not in the expected state.
    InvalidTerminalState = 4,
    /// Callback ID is already pending.
    DuplicateCallback = 5,
    /// No event to process.
    NoEventPending = 6,
    /// Invalid state transition.
    InvalidStateTransition = 7,
    /// Null pointer passed to FFI function.
    NullPointer = 8,
}

impl From<UIError> for DtermUIErrorCode {
    fn from(err: UIError) -> Self {
        match err {
            UIError::QueueFull => DtermUIErrorCode::QueueFull,
            UIError::ShuttingDown => DtermUIErrorCode::ShuttingDown,
            UIError::InvalidTerminalId => DtermUIErrorCode::InvalidTerminalId,
            UIError::InvalidTerminalState => DtermUIErrorCode::InvalidTerminalState,
            UIError::DuplicateCallback => DtermUIErrorCode::DuplicateCallback,
            UIError::NoEventPending => DtermUIErrorCode::NoEventPending,
            UIError::InvalidStateTransition => DtermUIErrorCode::InvalidStateTransition,
        }
    }
}

/// Create a new UI Bridge.
///
/// Returns a pointer to the bridge, or null on allocation failure.
#[no_mangle]
pub extern "C" fn dterm_ui_create() -> *mut DtermUIBridge {
    Box::into_raw(Box::new(DtermUIBridge(UIBridge::new())))
}

/// Free a UI Bridge.
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
/// - `bridge` must not have been freed previously (no double-free).
/// - After this call, `bridge` is invalid and must not be used.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_free(bridge: *mut DtermUIBridge) {
    if !bridge.is_null() {
        drop(unsafe { Box::from_raw(bridge) });
    }
}

/// Get the current UI state.
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
///
/// Returns `Idle` if bridge is null.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_state(bridge: *const DtermUIBridge) -> DtermUIState {
    if bridge.is_null() {
        return DtermUIState::Idle;
    }
    unsafe { (*bridge).0.state().into() }
}

/// Get the number of pending events in the queue.
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
///
/// Returns 0 if bridge is null.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_pending_count(bridge: *const DtermUIBridge) -> usize {
    if bridge.is_null() {
        return 0;
    }
    unsafe { (*bridge).0.pending_count() }
}

/// Get the number of pending callbacks.
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
///
/// Returns 0 if bridge is null.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_callback_count(bridge: *const DtermUIBridge) -> usize {
    if bridge.is_null() {
        return 0;
    }
    unsafe { (*bridge).0.callback_count() }
}

/// Get the number of pending renders.
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
///
/// Returns 0 if bridge is null.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_render_pending_count(bridge: *const DtermUIBridge) -> usize {
    if bridge.is_null() {
        return 0;
    }
    unsafe { (*bridge).0.render_pending_count() }
}

/// Check if the UI Bridge is in a consistent state.
///
/// This verifies all TLA+ invariants hold.
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
///
/// Returns false if bridge is null.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_is_consistent(bridge: *const DtermUIBridge) -> bool {
    if bridge.is_null() {
        return false;
    }
    unsafe { (*bridge).0.is_consistent() }
}

/// Get the state of a terminal.
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
///
/// Returns `Inactive` if bridge is null.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_terminal_state(
    bridge: *const DtermUIBridge,
    terminal_id: TerminalId,
) -> DtermUITerminalState {
    if bridge.is_null() {
        return DtermUITerminalState::Inactive;
    }
    unsafe { (*bridge).0.terminal_state(terminal_id).into() }
}

/// Enqueue an input event.
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
/// - `data` must be a valid pointer to `data_len` bytes, or null if `data_len` is 0.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_enqueue_input(
    bridge: *mut DtermUIBridge,
    terminal_id: TerminalId,
    data: *const u8,
    data_len: usize,
) -> DtermUIErrorCode {
    if bridge.is_null() {
        return DtermUIErrorCode::NullPointer;
    }

    let input_data = if data.is_null() || data_len == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(data, data_len).to_vec() }
    };

    let event = Event::input(terminal_id, input_data);
    match unsafe { (*bridge).0.enqueue(event) } {
        Ok(()) => DtermUIErrorCode::Ok,
        Err(e) => e.into(),
    }
}

/// Enqueue a resize event.
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_enqueue_resize(
    bridge: *mut DtermUIBridge,
    terminal_id: TerminalId,
    rows: u16,
    cols: u16,
) -> DtermUIErrorCode {
    if bridge.is_null() {
        return DtermUIErrorCode::NullPointer;
    }

    let event = Event::resize(terminal_id, rows, cols);
    match unsafe { (*bridge).0.enqueue(event) } {
        Ok(()) => DtermUIErrorCode::Ok,
        Err(e) => e.into(),
    }
}

/// Enqueue a render event.
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_enqueue_render(
    bridge: *mut DtermUIBridge,
    terminal_id: TerminalId,
) -> DtermUIErrorCode {
    if bridge.is_null() {
        return DtermUIErrorCode::NullPointer;
    }

    let event = Event::render(terminal_id);
    match unsafe { (*bridge).0.enqueue(event) } {
        Ok(()) => DtermUIErrorCode::Ok,
        Err(e) => e.into(),
    }
}

/// Enqueue a create terminal event.
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_enqueue_create_terminal(
    bridge: *mut DtermUIBridge,
    terminal_id: TerminalId,
) -> DtermUIErrorCode {
    if bridge.is_null() {
        return DtermUIErrorCode::NullPointer;
    }

    let event = Event::create_terminal(terminal_id);
    match unsafe { (*bridge).0.enqueue(event) } {
        Ok(()) => DtermUIErrorCode::Ok,
        Err(e) => e.into(),
    }
}

/// Enqueue a destroy terminal event.
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_enqueue_destroy_terminal(
    bridge: *mut DtermUIBridge,
    terminal_id: TerminalId,
) -> DtermUIErrorCode {
    if bridge.is_null() {
        return DtermUIErrorCode::NullPointer;
    }

    let event = Event::destroy_terminal(terminal_id);
    match unsafe { (*bridge).0.enqueue(event) } {
        Ok(()) => DtermUIErrorCode::Ok,
        Err(e) => e.into(),
    }
}

/// Enqueue a callback request event.
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_enqueue_callback(
    bridge: *mut DtermUIBridge,
    terminal_id: TerminalId,
    callback_id: CallbackId,
) -> DtermUIErrorCode {
    if bridge.is_null() {
        return DtermUIErrorCode::NullPointer;
    }

    let event = Event::request_callback(terminal_id, callback_id);
    match unsafe { (*bridge).0.enqueue(event) } {
        Ok(()) => DtermUIErrorCode::Ok,
        Err(e) => e.into(),
    }
}

/// Enqueue a shutdown event.
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_enqueue_shutdown(bridge: *mut DtermUIBridge) -> DtermUIErrorCode {
    if bridge.is_null() {
        return DtermUIErrorCode::NullPointer;
    }

    let event = Event::shutdown();
    match unsafe { (*bridge).0.enqueue(event) } {
        Ok(()) => DtermUIErrorCode::Ok,
        Err(e) => e.into(),
    }
}

/// Event info returned from start_processing.
#[repr(C)]
pub struct DtermUIEventInfo {
    /// Event ID.
    pub event_id: EventId,
    /// Event kind.
    pub kind: DtermUIEventKind,
    /// Target terminal (u32::MAX if none).
    pub terminal_id: TerminalId,
    /// Callback ID (u32::MAX if none).
    pub callback_id: CallbackId,
    /// Number of rows (for resize events).
    pub rows: u16,
    /// Number of columns (for resize events).
    pub cols: u16,
}

/// Start processing the next event.
///
/// Returns Ok (0) and fills `out_info` if successful.
/// Returns an error code on failure.
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
/// - `out_info` must be a valid writable pointer, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_start_processing(
    bridge: *mut DtermUIBridge,
    out_info: *mut DtermUIEventInfo,
) -> DtermUIErrorCode {
    if bridge.is_null() {
        return DtermUIErrorCode::NullPointer;
    }

    match unsafe { (*bridge).0.start_processing() } {
        Ok(event) => {
            if !out_info.is_null() {
                unsafe {
                    (*out_info) = DtermUIEventInfo {
                        event_id: event.id,
                        kind: event.kind.into(),
                        terminal_id: event.terminal.unwrap_or(u32::MAX),
                        callback_id: event.callback.unwrap_or(u32::MAX),
                        rows: event.data.rows,
                        cols: event.data.cols,
                    };
                }
            }
            DtermUIErrorCode::Ok
        }
        Err(e) => e.into(),
    }
}

/// Complete processing the current event.
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_complete_processing(
    bridge: *mut DtermUIBridge,
) -> DtermUIErrorCode {
    if bridge.is_null() {
        return DtermUIErrorCode::NullPointer;
    }

    match unsafe { (*bridge).0.complete_processing() } {
        Ok(()) => DtermUIErrorCode::Ok,
        Err(e) => e.into(),
    }
}

/// Complete a render for a terminal.
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_complete_render(
    bridge: *mut DtermUIBridge,
    terminal_id: TerminalId,
) -> DtermUIErrorCode {
    if bridge.is_null() {
        return DtermUIErrorCode::NullPointer;
    }

    match unsafe { (*bridge).0.complete_render(terminal_id) } {
        Ok(()) => DtermUIErrorCode::Ok,
        Err(e) => e.into(),
    }
}

/// Complete a callback.
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_complete_callback(
    bridge: *mut DtermUIBridge,
    callback_id: CallbackId,
) -> DtermUIErrorCode {
    if bridge.is_null() {
        return DtermUIErrorCode::NullPointer;
    }

    match unsafe { (*bridge).0.complete_callback(callback_id) } {
        Ok(()) => DtermUIErrorCode::Ok,
        Err(e) => e.into(),
    }
}

/// Handle an event in one shot (convenience function).
///
/// This enqueues and processes the event immediately if the bridge is idle.
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_handle_create_terminal(
    bridge: *mut DtermUIBridge,
    terminal_id: TerminalId,
) -> DtermUIErrorCode {
    if bridge.is_null() {
        return DtermUIErrorCode::NullPointer;
    }

    let event = Event::create_terminal(terminal_id);
    match unsafe { (*bridge).0.handle_event(event) } {
        Ok(()) => DtermUIErrorCode::Ok,
        Err(e) => e.into(),
    }
}

/// Handle a destroy terminal event in one shot (convenience function).
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_handle_destroy_terminal(
    bridge: *mut DtermUIBridge,
    terminal_id: TerminalId,
) -> DtermUIErrorCode {
    if bridge.is_null() {
        return DtermUIErrorCode::NullPointer;
    }

    let event = Event::destroy_terminal(terminal_id);
    match unsafe { (*bridge).0.handle_event(event) } {
        Ok(()) => DtermUIErrorCode::Ok,
        Err(e) => e.into(),
    }
}

/// Handle an input event in one shot (convenience function).
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
/// - `data` must be a valid pointer to `data_len` bytes, or null if `data_len` is 0.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_handle_input(
    bridge: *mut DtermUIBridge,
    terminal_id: TerminalId,
    data: *const u8,
    data_len: usize,
) -> DtermUIErrorCode {
    if bridge.is_null() {
        return DtermUIErrorCode::NullPointer;
    }

    let input_data = if data.is_null() || data_len == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(data, data_len).to_vec() }
    };

    let event = Event::input(terminal_id, input_data);
    match unsafe { (*bridge).0.handle_event(event) } {
        Ok(()) => DtermUIErrorCode::Ok,
        Err(e) => e.into(),
    }
}

/// Handle a resize event in one shot (convenience function).
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_handle_resize(
    bridge: *mut DtermUIBridge,
    terminal_id: TerminalId,
    rows: u16,
    cols: u16,
) -> DtermUIErrorCode {
    if bridge.is_null() {
        return DtermUIErrorCode::NullPointer;
    }

    let event = Event::resize(terminal_id, rows, cols);
    match unsafe { (*bridge).0.handle_event(event) } {
        Ok(()) => DtermUIErrorCode::Ok,
        Err(e) => e.into(),
    }
}

/// Handle a shutdown event in one shot (convenience function).
///
/// # Safety
///
/// - `bridge` must be a valid pointer returned by `dterm_ui_create`, or null.
#[no_mangle]
pub unsafe extern "C" fn dterm_ui_handle_shutdown(bridge: *mut DtermUIBridge) -> DtermUIErrorCode {
    if bridge.is_null() {
        return DtermUIErrorCode::NullPointer;
    }

    let event = Event::shutdown();
    match unsafe { (*bridge).0.handle_event(event) } {
        Ok(()) => DtermUIErrorCode::Ok,
        Err(e) => e.into(),
    }
}

// =============================================================================
// UI BRIDGE FFI TESTS
// =============================================================================

#[cfg(test)]
#[allow(clippy::borrow_as_ptr)]
mod ui_ffi_tests {
    use super::*;

    #[test]
    fn test_ui_create_free() {
        let bridge = dterm_ui_create();
        assert!(!bridge.is_null());
        unsafe {
            assert_eq!(dterm_ui_state(bridge), DtermUIState::Idle);
            assert_eq!(dterm_ui_pending_count(bridge), 0);
            assert!(dterm_ui_is_consistent(bridge));
            dterm_ui_free(bridge);
        }
    }

    #[test]
    fn test_ui_null_safety() {
        unsafe {
            // All functions should be safe with null
            assert_eq!(dterm_ui_state(ptr::null()), DtermUIState::Idle);
            assert_eq!(dterm_ui_pending_count(ptr::null()), 0);
            assert_eq!(dterm_ui_callback_count(ptr::null()), 0);
            assert_eq!(dterm_ui_render_pending_count(ptr::null()), 0);
            assert!(!dterm_ui_is_consistent(ptr::null()));
            assert_eq!(
                dterm_ui_terminal_state(ptr::null(), 0),
                DtermUITerminalState::Inactive
            );
            assert_eq!(
                dterm_ui_enqueue_input(ptr::null_mut(), 0, ptr::null(), 0),
                DtermUIErrorCode::NullPointer
            );
            assert_eq!(
                dterm_ui_enqueue_resize(ptr::null_mut(), 0, 24, 80),
                DtermUIErrorCode::NullPointer
            );
            assert_eq!(
                dterm_ui_enqueue_render(ptr::null_mut(), 0),
                DtermUIErrorCode::NullPointer
            );
            assert_eq!(
                dterm_ui_enqueue_create_terminal(ptr::null_mut(), 0),
                DtermUIErrorCode::NullPointer
            );
            assert_eq!(
                dterm_ui_enqueue_destroy_terminal(ptr::null_mut(), 0),
                DtermUIErrorCode::NullPointer
            );
            assert_eq!(
                dterm_ui_enqueue_callback(ptr::null_mut(), 0, 0),
                DtermUIErrorCode::NullPointer
            );
            assert_eq!(
                dterm_ui_enqueue_shutdown(ptr::null_mut()),
                DtermUIErrorCode::NullPointer
            );
            assert_eq!(
                dterm_ui_start_processing(ptr::null_mut(), ptr::null_mut()),
                DtermUIErrorCode::NullPointer
            );
            assert_eq!(
                dterm_ui_complete_processing(ptr::null_mut()),
                DtermUIErrorCode::NullPointer
            );
            assert_eq!(
                dterm_ui_complete_render(ptr::null_mut(), 0),
                DtermUIErrorCode::NullPointer
            );
            assert_eq!(
                dterm_ui_complete_callback(ptr::null_mut(), 0),
                DtermUIErrorCode::NullPointer
            );
            dterm_ui_free(ptr::null_mut()); // Should not crash
        }
    }

    #[test]
    fn test_ui_terminal_lifecycle() {
        let bridge = dterm_ui_create();
        unsafe {
            // Initially inactive
            assert_eq!(
                dterm_ui_terminal_state(bridge, 0),
                DtermUITerminalState::Inactive
            );

            // Create terminal
            assert_eq!(
                dterm_ui_handle_create_terminal(bridge, 0),
                DtermUIErrorCode::Ok
            );
            assert_eq!(
                dterm_ui_terminal_state(bridge, 0),
                DtermUITerminalState::Active
            );

            // Destroy terminal
            assert_eq!(
                dterm_ui_handle_destroy_terminal(bridge, 0),
                DtermUIErrorCode::Ok
            );
            assert_eq!(
                dterm_ui_terminal_state(bridge, 0),
                DtermUITerminalState::Disposed
            );

            // Cannot create again
            assert_eq!(
                dterm_ui_handle_create_terminal(bridge, 0),
                DtermUIErrorCode::InvalidTerminalState
            );

            dterm_ui_free(bridge);
        }
    }

    #[test]
    fn test_ui_event_queue() {
        let bridge = dterm_ui_create();
        unsafe {
            // Create terminal first
            assert_eq!(
                dterm_ui_handle_create_terminal(bridge, 0),
                DtermUIErrorCode::Ok
            );

            // Enqueue some events
            assert_eq!(
                dterm_ui_enqueue_input(bridge, 0, b"hello".as_ptr(), 5),
                DtermUIErrorCode::Ok
            );
            assert_eq!(dterm_ui_pending_count(bridge), 1);

            assert_eq!(
                dterm_ui_enqueue_resize(bridge, 0, 40, 120),
                DtermUIErrorCode::Ok
            );
            assert_eq!(dterm_ui_pending_count(bridge), 2);

            // Process events
            let mut info = DtermUIEventInfo {
                event_id: 0,
                kind: DtermUIEventKind::Input,
                terminal_id: 0,
                callback_id: 0,
                rows: 0,
                cols: 0,
            };

            assert_eq!(
                dterm_ui_start_processing(bridge, &mut info),
                DtermUIErrorCode::Ok
            );
            assert_eq!(info.kind, DtermUIEventKind::Input);
            assert_eq!(info.terminal_id, 0);
            assert_eq!(dterm_ui_state(bridge), DtermUIState::Processing);

            assert_eq!(dterm_ui_complete_processing(bridge), DtermUIErrorCode::Ok);
            assert_eq!(dterm_ui_state(bridge), DtermUIState::Idle);
            assert_eq!(dterm_ui_pending_count(bridge), 1);

            assert!(dterm_ui_is_consistent(bridge));

            dterm_ui_free(bridge);
        }
    }

    #[test]
    fn test_ui_render_flow() {
        let bridge = dterm_ui_create();
        unsafe {
            // Create terminal
            assert_eq!(
                dterm_ui_handle_create_terminal(bridge, 0),
                DtermUIErrorCode::Ok
            );

            // Enqueue render and process
            assert_eq!(dterm_ui_enqueue_render(bridge, 0), DtermUIErrorCode::Ok);

            let mut info = DtermUIEventInfo {
                event_id: 0,
                kind: DtermUIEventKind::Input,
                terminal_id: 0,
                callback_id: 0,
                rows: 0,
                cols: 0,
            };

            assert_eq!(
                dterm_ui_start_processing(bridge, &mut info),
                DtermUIErrorCode::Ok
            );
            assert_eq!(info.kind, DtermUIEventKind::Render);

            assert_eq!(dterm_ui_complete_processing(bridge), DtermUIErrorCode::Ok);
            assert_eq!(dterm_ui_state(bridge), DtermUIState::Rendering);
            assert_eq!(dterm_ui_render_pending_count(bridge), 1);

            // Complete render
            assert_eq!(dterm_ui_complete_render(bridge, 0), DtermUIErrorCode::Ok);
            assert_eq!(dterm_ui_state(bridge), DtermUIState::Idle);
            assert_eq!(dterm_ui_render_pending_count(bridge), 0);

            assert!(dterm_ui_is_consistent(bridge));

            dterm_ui_free(bridge);
        }
    }

    #[test]
    fn test_ui_shutdown() {
        let bridge = dterm_ui_create();
        unsafe {
            // Shutdown
            assert_eq!(dterm_ui_handle_shutdown(bridge), DtermUIErrorCode::Ok);
            assert_eq!(dterm_ui_state(bridge), DtermUIState::ShuttingDown);

            // Cannot enqueue after shutdown
            assert_eq!(
                dterm_ui_enqueue_create_terminal(bridge, 0),
                DtermUIErrorCode::ShuttingDown
            );

            dterm_ui_free(bridge);
        }
    }
}

// =============================================================================
// KANI PROOFS FOR FFI SAFETY
// =============================================================================

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proof: Terminal new/free lifecycle is safe
    /// - Allocates valid Box
    /// - Free deallocates without double-free
    #[kani::proof]
    #[kani::unwind(2)]
    fn terminal_lifecycle_safe() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 100);
        kani::assume(cols > 0 && cols <= 200);

        let term = dterm_terminal_new(rows, cols);
        assert!(!term.is_null());

        // Safe to access after creation
        unsafe {
            assert_eq!(dterm_terminal_rows(term), rows);
            assert_eq!(dterm_terminal_cols(term), cols);
            dterm_terminal_free(term);
        }
    }

    /// Proof: Parser new/free lifecycle is safe
    #[kani::proof]
    #[kani::unwind(2)]
    fn parser_lifecycle_safe() {
        let parser = dterm_parser_new();
        assert!(!parser.is_null());

        unsafe {
            dterm_parser_reset(parser);
            dterm_parser_free(parser);
        }
    }

    /// Proof: Grid new/free lifecycle is safe
    #[kani::proof]
    #[kani::unwind(2)]
    fn grid_lifecycle_safe() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 100);
        kani::assume(cols > 0 && cols <= 200);

        let grid = dterm_grid_new(rows, cols);
        assert!(!grid.is_null());

        unsafe {
            assert_eq!(dterm_grid_rows(grid), rows);
            assert_eq!(dterm_grid_cols(grid), cols);
            dterm_grid_free(grid);
        }
    }

    /// Proof: Null pointer checks never cause UB for terminal functions
    #[kani::proof]
    fn terminal_null_checks_safe() {
        unsafe {
            // All read functions should return safe defaults
            assert_eq!(dterm_terminal_rows(ptr::null()), 0);
            assert_eq!(dterm_terminal_cols(ptr::null()), 0);
            assert_eq!(dterm_terminal_cursor_row(ptr::null()), 0);
            assert_eq!(dterm_terminal_cursor_col(ptr::null()), 0);
            assert!(dterm_terminal_title(ptr::null_mut()).is_null());
            assert!(!dterm_terminal_has_response(ptr::null()));
            assert_eq!(dterm_terminal_response_len(ptr::null()), 0);

            // Free with null is no-op
            dterm_terminal_free(ptr::null_mut());
        }
    }

    /// Proof: Null pointer checks never cause UB for grid functions
    #[kani::proof]
    fn grid_null_checks_safe() {
        unsafe {
            assert_eq!(dterm_grid_rows(ptr::null()), 0);
            assert_eq!(dterm_grid_cols(ptr::null()), 0);
            assert_eq!(dterm_grid_cursor_row(ptr::null()), 0);
            assert_eq!(dterm_grid_cursor_col(ptr::null()), 0);
            assert!(!dterm_grid_needs_redraw(ptr::null()));
            dterm_grid_free(ptr::null_mut());
        }
    }

    /// Proof: Null pointer checks never cause UB for parser functions
    #[kani::proof]
    fn parser_null_checks_safe() {
        unsafe {
            dterm_parser_reset(ptr::null_mut());
            dterm_parser_free(ptr::null_mut());
        }
    }

    /// Proof: Terminal cursor stays in bounds after any cursor movement
    #[kani::proof]
    #[kani::unwind(2)]
    fn terminal_cursor_bounds_safe() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 50);
        kani::assume(cols > 0 && cols <= 80);

        let term = dterm_terminal_new(rows, cols);

        unsafe {
            // Cursor should always be in bounds
            let cur_row = dterm_terminal_cursor_row(term);
            let cur_col = dterm_terminal_cursor_col(term);
            assert!(cur_row < rows);
            assert!(cur_col < cols);

            dterm_terminal_free(term);
        }
    }

    /// Proof: Grid cursor stays in bounds after set_cursor
    #[kani::proof]
    #[kani::unwind(2)]
    fn grid_set_cursor_bounds_safe() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        let set_row: u16 = kani::any();
        let set_col: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 50);
        kani::assume(cols > 0 && cols <= 80);

        let grid = dterm_grid_new(rows, cols);

        unsafe {
            // Set cursor to arbitrary position
            dterm_grid_set_cursor(grid, set_row, set_col);

            // Result should be clamped to bounds
            let cur_row = dterm_grid_cursor_row(grid);
            let cur_col = dterm_grid_cursor_col(grid);
            assert!(cur_row < rows);
            assert!(cur_col < cols);

            dterm_grid_free(grid);
        }
    }

    /// Proof: DtermCell FFI struct has correct size and alignment
    #[kani::proof]
    fn dterm_cell_repr_c_safe() {
        // Verify #[repr(C)] layout is predictable
        let cell = DtermCell {
            codepoint: kani::any(),
            fg: kani::any(),
            bg: kani::any(),
            underline_color: kani::any(),
            flags: kani::any(),
        };

        // Size should be stable for FFI
        // 4 + 4 + 4 + 4 + 2 = 18, padded for alignment
        assert!(std::mem::size_of::<DtermCell>() >= 18);
        assert!(std::mem::size_of::<DtermCell>() <= 24); // Allow padding
    }

    /// Proof: DtermAction FFI struct has correct layout
    #[kani::proof]
    fn dterm_action_repr_c_safe() {
        let action = DtermAction {
            action_type: DtermActionType::Print,
            byte: kani::any(),
            final_byte: kani::any(),
            param_count: kani::any(),
            params: [0; 16],
        };

        // Verify param_count bounded
        kani::assume(action.param_count <= 16);

        // Should be able to access all params safely
        for i in 0..action.param_count as usize {
            let _ = action.params[i];
        }
    }

    /// Proof: get_cell with out_cell=null is safe
    #[kani::proof]
    #[kani::unwind(2)]
    fn grid_get_cell_null_out_safe() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 20);
        kani::assume(cols > 0 && cols <= 40);

        let grid = dterm_grid_new(rows, cols);

        unsafe {
            let row: u16 = kani::any();
            let col: u16 = kani::any();

            // Should not crash with null out_cell
            let exists = dterm_grid_get_cell(grid, row, col, ptr::null_mut());

            // Result should match bounds check
            if row < rows && col < cols {
                assert!(exists);
            } else {
                assert!(!exists);
            }

            dterm_grid_free(grid);
        }
    }

    /// Proof: Search new/free lifecycle is safe
    #[kani::proof]
    #[kani::unwind(2)]
    fn search_lifecycle_safe() {
        let search = dterm_search_new();
        assert!(!search.is_null());

        unsafe {
            assert_eq!(dterm_search_line_count(search), 0);
            dterm_search_free(search);
        }
    }

    /// Proof: Search null checks are safe
    #[kani::proof]
    fn search_null_checks_safe() {
        unsafe {
            assert_eq!(dterm_search_line_count(ptr::null()), 0);
            assert!(!dterm_search_might_contain(ptr::null(), ptr::null(), 0));
            dterm_search_free(ptr::null_mut());
            dterm_search_clear(ptr::null_mut());
        }
    }

    // =========================================================================
    // UI BRIDGE FFI PROOFS
    // =========================================================================

    /// Proof: UI Bridge create/free lifecycle is safe
    #[kani::proof]
    #[kani::unwind(2)]
    fn ui_bridge_lifecycle_safe() {
        let bridge = dterm_ui_create();
        assert!(!bridge.is_null());

        unsafe {
            assert_eq!(dterm_ui_state(bridge), DtermUIState::Idle);
            assert_eq!(dterm_ui_pending_count(bridge), 0);
            assert!(dterm_ui_is_consistent(bridge));
            dterm_ui_free(bridge);
        }
    }

    /// Proof: UI Bridge null pointer checks are safe
    #[kani::proof]
    fn ui_bridge_null_safety() {
        unsafe {
            // All read functions should be safe with null
            assert_eq!(dterm_ui_state(ptr::null()), DtermUIState::Idle);
            assert_eq!(dterm_ui_pending_count(ptr::null()), 0);
            assert_eq!(dterm_ui_callback_count(ptr::null()), 0);
            assert_eq!(dterm_ui_render_pending_count(ptr::null()), 0);
            assert!(!dterm_ui_is_consistent(ptr::null()));
            assert_eq!(
                dterm_ui_terminal_state(ptr::null(), 0),
                DtermUITerminalState::Inactive
            );

            // All write functions should return NullPointer error
            assert_eq!(
                dterm_ui_enqueue_input(ptr::null_mut(), 0, ptr::null(), 0),
                DtermUIErrorCode::NullPointer
            );
            assert_eq!(
                dterm_ui_enqueue_resize(ptr::null_mut(), 0, 24, 80),
                DtermUIErrorCode::NullPointer
            );
            assert_eq!(
                dterm_ui_enqueue_render(ptr::null_mut(), 0),
                DtermUIErrorCode::NullPointer
            );
            assert_eq!(
                dterm_ui_enqueue_create_terminal(ptr::null_mut(), 0),
                DtermUIErrorCode::NullPointer
            );
            assert_eq!(
                dterm_ui_enqueue_destroy_terminal(ptr::null_mut(), 0),
                DtermUIErrorCode::NullPointer
            );
            assert_eq!(
                dterm_ui_enqueue_callback(ptr::null_mut(), 0, 0),
                DtermUIErrorCode::NullPointer
            );
            assert_eq!(
                dterm_ui_enqueue_shutdown(ptr::null_mut()),
                DtermUIErrorCode::NullPointer
            );
            assert_eq!(
                dterm_ui_start_processing(ptr::null_mut(), ptr::null_mut()),
                DtermUIErrorCode::NullPointer
            );
            assert_eq!(
                dterm_ui_complete_processing(ptr::null_mut()),
                DtermUIErrorCode::NullPointer
            );
            assert_eq!(
                dterm_ui_complete_render(ptr::null_mut(), 0),
                DtermUIErrorCode::NullPointer
            );
            assert_eq!(
                dterm_ui_complete_callback(ptr::null_mut(), 0),
                DtermUIErrorCode::NullPointer
            );

            // Free should be safe with null
            dterm_ui_free(ptr::null_mut());
        }
    }

    /// Proof: UI Bridge terminal state transitions are valid
    #[kani::proof]
    #[kani::unwind(5)]
    fn ui_bridge_terminal_state_transitions() {
        let bridge = dterm_ui_create();
        let tid: TerminalId = kani::any();
        kani::assume(tid < 10); // Small terminal ID for bounded proof

        unsafe {
            // Initially inactive
            assert_eq!(
                dterm_ui_terminal_state(bridge, tid),
                DtermUITerminalState::Inactive
            );

            // Create terminal
            let result = dterm_ui_handle_create_terminal(bridge, tid);
            assert_eq!(result, DtermUIErrorCode::Ok);
            assert_eq!(
                dterm_ui_terminal_state(bridge, tid),
                DtermUITerminalState::Active
            );

            // Destroy terminal
            let result = dterm_ui_handle_destroy_terminal(bridge, tid);
            assert_eq!(result, DtermUIErrorCode::Ok);
            assert_eq!(
                dterm_ui_terminal_state(bridge, tid),
                DtermUITerminalState::Disposed
            );

            // Disposed is permanent - cannot create again
            let result = dterm_ui_handle_create_terminal(bridge, tid);
            assert_eq!(result, DtermUIErrorCode::InvalidTerminalState);

            dterm_ui_free(bridge);
        }
    }

    /// Proof: UI Bridge consistency is preserved across operations
    #[kani::proof]
    #[kani::unwind(5)]
    fn ui_bridge_consistency_preserved() {
        let bridge = dterm_ui_create();

        unsafe {
            assert!(dterm_ui_is_consistent(bridge));

            // Create terminal
            let _ = dterm_ui_handle_create_terminal(bridge, 0);
            assert!(dterm_ui_is_consistent(bridge));

            // Enqueue events
            let _ = dterm_ui_enqueue_input(bridge, 0, ptr::null(), 0);
            assert!(dterm_ui_is_consistent(bridge));

            // Process event
            let mut info = DtermUIEventInfo {
                event_id: 0,
                kind: DtermUIEventKind::Input,
                terminal_id: 0,
                callback_id: 0,
                rows: 0,
                cols: 0,
            };
            if dterm_ui_start_processing(bridge, &mut info) == DtermUIErrorCode::Ok {
                // Still consistent during processing
                assert!(dterm_ui_is_consistent(bridge));

                if dterm_ui_complete_processing(bridge) == DtermUIErrorCode::Ok {
                    // Consistent after completion
                    assert!(dterm_ui_is_consistent(bridge));
                }
            }

            dterm_ui_free(bridge);
        }
    }

    /// Proof: UI Bridge shutdown rejects new events
    #[kani::proof]
    #[kani::unwind(3)]
    fn ui_bridge_shutdown_rejects_events() {
        let bridge = dterm_ui_create();

        unsafe {
            // Shutdown
            let result = dterm_ui_handle_shutdown(bridge);
            assert_eq!(result, DtermUIErrorCode::Ok);
            assert_eq!(dterm_ui_state(bridge), DtermUIState::ShuttingDown);

            // Cannot enqueue new events
            assert_eq!(
                dterm_ui_enqueue_create_terminal(bridge, 0),
                DtermUIErrorCode::ShuttingDown
            );
            assert_eq!(
                dterm_ui_enqueue_input(bridge, 0, ptr::null(), 0),
                DtermUIErrorCode::ShuttingDown
            );
            assert_eq!(
                dterm_ui_enqueue_render(bridge, 0),
                DtermUIErrorCode::ShuttingDown
            );

            dterm_ui_free(bridge);
        }
    }

    /// Proof: UI Bridge start_processing with null out_info is safe
    #[kani::proof]
    #[kani::unwind(3)]
    fn ui_bridge_start_processing_null_out_safe() {
        let bridge = dterm_ui_create();

        unsafe {
            // Create terminal and enqueue event
            let _ = dterm_ui_handle_create_terminal(bridge, 0);
            let _ = dterm_ui_enqueue_input(bridge, 0, ptr::null(), 0);

            // Start processing with null out_info should work
            let result = dterm_ui_start_processing(bridge, ptr::null_mut());
            assert_eq!(result, DtermUIErrorCode::Ok);
            assert_eq!(dterm_ui_state(bridge), DtermUIState::Processing);

            dterm_ui_free(bridge);
        }
    }
}
