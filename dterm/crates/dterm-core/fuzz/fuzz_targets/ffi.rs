//! FFI boundary fuzzer - tests C FFI functions for safety.
//!
//! This fuzzer exercises the FFI surface to ensure:
//! 1. No panics escape across FFI boundary
//! 2. Invalid parameters don't cause undefined behavior
//! 3. Memory is properly managed
//!
//! ## Running
//!
//! ```bash
//! cd crates/dterm-core
//! cargo +nightly fuzz run ffi -- -max_total_time=600
//! ```

#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use std::ffi::c_void;
use std::ptr;

use dterm_core::ffi::*;

/// FFI operation to perform
#[derive(Debug, Arbitrary)]
enum FfiOperation {
    // Terminal lifecycle
    CreateTerminal { rows: u16, cols: u16 },
    CreateTerminalWithScrollback { rows: u16, cols: u16, max_lines: u32 },

    // Terminal data processing
    ProcessData { data: Vec<u8> },

    // Terminal queries
    GetRows,
    GetCols,
    GetCursorRow,
    GetCursorCol,
    GetCursorVisible,
    GetTitle,
    GetIconName,
    GetCursorStyle,
    IsAlternateScreen,
    NeedsRedraw,
    GetDisplayOffset,
    GetTotalLines,
    MemoryUsage,
    HasResponse,
    ResponseLen,

    // Terminal mutations
    Resize { rows: u16, cols: u16 },
    Reset,
    ScrollDisplay { delta: i32 },
    ScrollToTop,
    ScrollToBottom,
    ClearDamage,
    SetMemoryBudget { bytes: u32 },

    // Kitty graphics
    KittyHasImages,
    KittyIsDirty,
    KittyClearDirty,
    KittyImageCount,
    KittyTotalBytes,
    KittyQuota,

    // Sixel
    HasSixelImage,

    // Shell integration
    ShellState,
    BlockCount,
    HasWorkingDirectory,

    // Mode queries
    MouseTrackingEnabled,
    MouseMode,
    MouseEncoding,
    FocusReportingEnabled,
    SynchronizedOutputEnabled,
    IsSecureKeyboardEntry,

    // Secure keyboard entry
    SetSecureKeyboardEntry { enabled: bool },

    // Palette
    GetPaletteColor { index: u8 },
    SetPaletteColor { index: u8, r: u8, g: u8, b: u8 },
    ResetPalette,
    ResetPaletteColor { index: u8 },

    // Selection operations
    SelectionStart { row: i32, col: u16 },
    SelectionUpdate { row: i32, col: u16 },
    SelectionEnd,
    SelectionClear,
    HasSelection,

    // Parser operations
    ParserCreate,
    ParserReset,
    ParserFeed { data: Vec<u8> },

    // Grid operations
    GridCreate { rows: u16, cols: u16 },
    GridResize { rows: u16, cols: u16 },
    GridWriteChar { c: u32 },
    GridScrollDisplay { delta: i32 },
    GridEraseScreen,

    // Search operations
    SearchCreate,
    SearchClear,
}

/// Test context holding created handles
struct FuzzContext {
    terminal: *mut DtermTerminal,
    parser: *mut DtermParser,
    grid: *mut DtermGrid,
    search: *mut DtermSearch,
}

impl FuzzContext {
    fn new() -> Self {
        Self {
            terminal: ptr::null_mut(),
            parser: ptr::null_mut(),
            grid: ptr::null_mut(),
            search: ptr::null_mut(),
        }
    }
}

impl Drop for FuzzContext {
    fn drop(&mut self) {
        // Clean up all handles
        unsafe {
            if !self.terminal.is_null() {
                dterm_terminal_free(self.terminal);
            }
            if !self.parser.is_null() {
                dterm_parser_free(self.parser);
            }
            if !self.grid.is_null() {
                dterm_grid_free(self.grid);
            }
            if !self.search.is_null() {
                dterm_search_free(self.search);
            }
        }
    }
}

fn execute_operation(ctx: &mut FuzzContext, op: &FfiOperation) {
    unsafe {
        match op {
            // Terminal lifecycle
            FfiOperation::CreateTerminal { rows, cols } => {
                // Clean up existing terminal
                if !ctx.terminal.is_null() {
                    dterm_terminal_free(ctx.terminal);
                }
                let rows = (*rows).max(1).min(100);
                let cols = (*cols).max(1).min(200);
                ctx.terminal = dterm_terminal_new(rows, cols);
            }
            FfiOperation::CreateTerminalWithScrollback { rows, cols, max_lines } => {
                if !ctx.terminal.is_null() {
                    dterm_terminal_free(ctx.terminal);
                }
                let rows = (*rows).max(1).min(100);
                let cols = (*cols).max(1).min(200);
                let max_lines = (*max_lines as usize).min(1000);
                // ring_buffer_size, hot_limit, warm_limit, memory_budget (10MB limit)
                ctx.terminal = dterm_terminal_new_with_scrollback(rows, cols, 256, max_lines, max_lines, 10 * 1024 * 1024);
            }

            // Terminal data processing
            FfiOperation::ProcessData { data } => {
                if !ctx.terminal.is_null() {
                    dterm_terminal_process(
                        ctx.terminal,
                        data.as_ptr(),
                        data.len(),
                    );
                }
            }

            // Terminal queries - all should be safe even with null
            FfiOperation::GetRows => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_rows(ctx.terminal);
                }
            }
            FfiOperation::GetCols => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_cols(ctx.terminal);
                }
            }
            FfiOperation::GetCursorRow => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_cursor_row(ctx.terminal);
                }
            }
            FfiOperation::GetCursorCol => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_cursor_col(ctx.terminal);
                }
            }
            FfiOperation::GetCursorVisible => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_cursor_visible(ctx.terminal);
                }
            }
            FfiOperation::GetTitle => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_title(ctx.terminal);
                }
            }
            FfiOperation::GetIconName => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_icon_name(ctx.terminal);
                }
            }
            FfiOperation::GetCursorStyle => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_cursor_style(ctx.terminal);
                }
            }
            FfiOperation::IsAlternateScreen => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_is_alternate_screen(ctx.terminal);
                }
            }
            FfiOperation::NeedsRedraw => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_needs_redraw(ctx.terminal);
                }
            }
            FfiOperation::GetDisplayOffset => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_display_offset(ctx.terminal);
                }
            }
            FfiOperation::GetTotalLines => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_total_lines(ctx.terminal);
                }
            }
            FfiOperation::MemoryUsage => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_memory_usage(ctx.terminal);
                }
            }
            FfiOperation::HasResponse => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_has_response(ctx.terminal);
                }
            }
            FfiOperation::ResponseLen => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_response_len(ctx.terminal);
                }
            }

            // Terminal mutations
            FfiOperation::Resize { rows, cols } => {
                if !ctx.terminal.is_null() {
                    let rows = (*rows).max(1).min(100);
                    let cols = (*cols).max(1).min(200);
                    dterm_terminal_resize(ctx.terminal, rows, cols);
                }
            }
            FfiOperation::Reset => {
                if !ctx.terminal.is_null() {
                    dterm_terminal_reset(ctx.terminal);
                }
            }
            FfiOperation::ScrollDisplay { delta } => {
                if !ctx.terminal.is_null() {
                    dterm_terminal_scroll_display(ctx.terminal, *delta);
                }
            }
            FfiOperation::ScrollToTop => {
                if !ctx.terminal.is_null() {
                    dterm_terminal_scroll_to_top(ctx.terminal);
                }
            }
            FfiOperation::ScrollToBottom => {
                if !ctx.terminal.is_null() {
                    dterm_terminal_scroll_to_bottom(ctx.terminal);
                }
            }
            FfiOperation::ClearDamage => {
                if !ctx.terminal.is_null() {
                    dterm_terminal_clear_damage(ctx.terminal);
                }
            }
            FfiOperation::SetMemoryBudget { bytes } => {
                if !ctx.terminal.is_null() {
                    let bytes = (*bytes as usize).min(1 << 30); // 1GB max
                    dterm_terminal_set_memory_budget(ctx.terminal, bytes);
                }
            }

            // Kitty graphics
            FfiOperation::KittyHasImages => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_kitty_has_images(ctx.terminal);
                }
            }
            FfiOperation::KittyIsDirty => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_kitty_is_dirty(ctx.terminal);
                }
            }
            FfiOperation::KittyClearDirty => {
                if !ctx.terminal.is_null() {
                    dterm_terminal_kitty_clear_dirty(ctx.terminal);
                }
            }
            FfiOperation::KittyImageCount => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_kitty_image_count(ctx.terminal);
                }
            }
            FfiOperation::KittyTotalBytes => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_kitty_total_bytes(ctx.terminal);
                }
            }
            FfiOperation::KittyQuota => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_kitty_quota(ctx.terminal);
                }
            }

            // Sixel
            FfiOperation::HasSixelImage => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_has_sixel_image(ctx.terminal);
                }
            }

            // Shell integration
            FfiOperation::ShellState => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_shell_state(ctx.terminal);
                }
            }
            FfiOperation::BlockCount => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_block_count(ctx.terminal);
                }
            }
            FfiOperation::HasWorkingDirectory => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_has_working_directory(ctx.terminal);
                }
            }

            // Mode queries
            FfiOperation::MouseTrackingEnabled => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_mouse_tracking_enabled(ctx.terminal);
                }
            }
            FfiOperation::MouseMode => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_mouse_mode(ctx.terminal);
                }
            }
            FfiOperation::MouseEncoding => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_mouse_encoding(ctx.terminal);
                }
            }
            FfiOperation::FocusReportingEnabled => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_focus_reporting_enabled(ctx.terminal);
                }
            }
            FfiOperation::SynchronizedOutputEnabled => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_synchronized_output_enabled(ctx.terminal);
                }
            }
            FfiOperation::IsSecureKeyboardEntry => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_is_secure_keyboard_entry(ctx.terminal);
                }
            }

            // Secure keyboard entry
            FfiOperation::SetSecureKeyboardEntry { enabled } => {
                if !ctx.terminal.is_null() {
                    dterm_terminal_set_secure_keyboard_entry(ctx.terminal, *enabled);
                }
            }

            // Palette
            FfiOperation::GetPaletteColor { index } => {
                if !ctx.terminal.is_null() {
                    let mut color = DtermRgb { r: 0, g: 0, b: 0 };
                    dterm_terminal_get_palette_color(ctx.terminal, *index, &mut color);
                }
            }
            FfiOperation::SetPaletteColor { index, r, g, b } => {
                if !ctx.terminal.is_null() {
                    dterm_terminal_set_palette_color(ctx.terminal, *index, *r, *g, *b);
                }
            }
            FfiOperation::ResetPalette => {
                if !ctx.terminal.is_null() {
                    dterm_terminal_reset_palette(ctx.terminal);
                }
            }
            FfiOperation::ResetPaletteColor { index } => {
                if !ctx.terminal.is_null() {
                    dterm_terminal_reset_palette_color(ctx.terminal, *index);
                }
            }

            // Selection operations
            FfiOperation::SelectionStart { row, col } => {
                if !ctx.terminal.is_null() {
                    // dterm_terminal_selection_start(term, col: u32, row: i32, selection_type)
                    dterm_terminal_selection_start(ctx.terminal, *col as u32, *row, DtermSelectionType::Simple);
                }
            }
            FfiOperation::SelectionUpdate { row, col } => {
                if !ctx.terminal.is_null() {
                    // dterm_terminal_selection_update(term, col: u32, row: i32)
                    dterm_terminal_selection_update(ctx.terminal, *col as u32, *row);
                }
            }
            FfiOperation::SelectionEnd => {
                if !ctx.terminal.is_null() {
                    dterm_terminal_selection_end(ctx.terminal);
                }
            }
            FfiOperation::SelectionClear => {
                if !ctx.terminal.is_null() {
                    dterm_terminal_selection_clear(ctx.terminal);
                }
            }
            FfiOperation::HasSelection => {
                if !ctx.terminal.is_null() {
                    let _ = dterm_terminal_has_selection(ctx.terminal);
                }
            }

            // Parser operations
            FfiOperation::ParserCreate => {
                if !ctx.parser.is_null() {
                    dterm_parser_free(ctx.parser);
                }
                ctx.parser = dterm_parser_new();
            }
            FfiOperation::ParserReset => {
                if !ctx.parser.is_null() {
                    dterm_parser_reset(ctx.parser);
                }
            }
            FfiOperation::ParserFeed { data } => {
                if !ctx.parser.is_null() {
                    // Use a no-op callback for fuzzing
                    extern "C" fn noop_callback(_ctx: *mut c_void, _action: DtermAction) {}

                    dterm_parser_feed(
                        ctx.parser,
                        data.as_ptr(),
                        data.len(),
                        ptr::null_mut(),
                        noop_callback,
                    );
                }
            }

            // Grid operations
            FfiOperation::GridCreate { rows, cols } => {
                if !ctx.grid.is_null() {
                    dterm_grid_free(ctx.grid);
                }
                let rows = (*rows).max(1).min(100);
                let cols = (*cols).max(1).min(200);
                ctx.grid = dterm_grid_new(rows, cols);
            }
            FfiOperation::GridResize { rows, cols } => {
                if !ctx.grid.is_null() {
                    let rows = (*rows).max(1).min(100);
                    let cols = (*cols).max(1).min(200);
                    dterm_grid_resize(ctx.grid, rows, cols);
                }
            }
            FfiOperation::GridWriteChar { c } => {
                if !ctx.grid.is_null() {
                    dterm_grid_write_char(ctx.grid, *c);
                }
            }
            FfiOperation::GridScrollDisplay { delta } => {
                if !ctx.grid.is_null() {
                    dterm_grid_scroll_display(ctx.grid, *delta);
                }
            }
            FfiOperation::GridEraseScreen => {
                if !ctx.grid.is_null() {
                    dterm_grid_erase_screen(ctx.grid);
                }
            }

            // Search operations
            FfiOperation::SearchCreate => {
                if !ctx.search.is_null() {
                    dterm_search_free(ctx.search);
                }
                ctx.search = dterm_search_new();
            }
            FfiOperation::SearchClear => {
                if !ctx.search.is_null() {
                    dterm_search_clear(ctx.search);
                }
            }
        }
    }
}

fuzz_target!(|data: &[u8]| {
    // Parse the fuzz input into operations
    if let Ok(operations) = <Vec<FfiOperation> as Arbitrary>::arbitrary(
        &mut Unstructured::new(data)
    ) {
        let mut ctx = FuzzContext::new();

        // Always start with a terminal with LIMITED scrollback to prevent OOM
        // ring_buffer_size, hot_limit, warm_limit, memory_budget
        ctx.terminal = dterm_terminal_new_with_scrollback(24, 80, 256, 1000, 1000, 10 * 1024 * 1024);

        // Execute operations
        for op in operations.iter().take(100) {
            execute_operation(&mut ctx, op);
        }

        // ctx drops here, cleaning up all handles
    }
});
