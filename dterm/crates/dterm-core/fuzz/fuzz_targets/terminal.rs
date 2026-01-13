//! Terminal integration fuzzer - tests the complete terminal stack.
//!
//! This fuzzer exercises the full Terminal API, not just the parser.
//! It tests grid operations, modes, and all terminal state transitions.
//!
//! ## Running
//!
//! ```bash
//! cd crates/dterm-core
//! cargo +nightly fuzz run terminal -- -max_total_time=600
//! ```
//!
//! ## Properties Tested
//!
//! 1. Terminal state machine consistency
//! 2. Grid bounds are never violated
//! 3. Cursor position is always valid
//! 4. No panics from any API call sequence
//! 5. Memory usage remains bounded

#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use dterm_core::terminal::Terminal;

/// Commands that can be issued to a terminal.
#[derive(Debug, Arbitrary)]
enum TerminalCommand {
    /// Feed raw bytes
    Feed { data: Vec<u8> },
    /// Feed a known escape sequence
    FeedEscape { escape_type: EscapeType },
    /// Resize the terminal
    Resize { rows: u16, cols: u16 },
    /// Get cursor position
    GetCursor,
    /// Get terminal title
    GetTitle,
    /// Get terminal modes
    GetModes,
    /// Reset the terminal
    Reset,
    /// Scroll down
    ScrollDown { lines: u16 },
    /// Scroll up
    ScrollUp { lines: u16 },
}

/// Known escape sequence types to inject.
#[derive(Debug, Arbitrary)]
enum EscapeType {
    // Cursor movement
    CursorUp,
    CursorDown,
    CursorForward,
    CursorBack,
    CursorHome,
    CursorPosition { row: u8, col: u8 },

    // Editing
    EraseDisplay,
    EraseLine,
    InsertLine,
    DeleteLine,
    InsertChars,
    DeleteChars,

    // Scrolling
    ScrollUp,
    ScrollDown,
    SetScrollRegion { top: u8, bottom: u8 },

    // Modes
    SetMode { mode: u8 },
    ResetMode { mode: u8 },
    SetPrivateMode { mode: u16 },
    ResetPrivateMode { mode: u16 },

    // Graphics
    SetGraphicRendition { params: Vec<u8> },

    // Tab stops
    SetTabStop,
    ClearTabStop,
    ClearAllTabStops,

    // Character sets
    SetCharsetG0,
    SetCharsetG1,
    ShiftIn,
    ShiftOut,

    // Save/Restore
    SaveCursor,
    RestoreCursor,
    SaveCursorAndAttrs,
    RestoreCursorAndAttrs,

    // Screen
    AlternateScreenOn,
    AlternateScreenOff,

    // OSC sequences
    SetTitle { title: Vec<u8> },
    SetIconName { name: Vec<u8> },
    SetColor { idx: u8, r: u8, g: u8, b: u8 },

    // DCS sequences
    Sixel { data: Vec<u8> },

    // Special
    BracketedPasteOn,
    BracketedPasteOff,
    ApplicationCursorOn,
    ApplicationCursorOff,
}

impl EscapeType {
    fn to_bytes(&self) -> Vec<u8> {
        match self {
            // Cursor movement
            EscapeType::CursorUp => b"\x1b[A".to_vec(),
            EscapeType::CursorDown => b"\x1b[B".to_vec(),
            EscapeType::CursorForward => b"\x1b[C".to_vec(),
            EscapeType::CursorBack => b"\x1b[D".to_vec(),
            EscapeType::CursorHome => b"\x1b[H".to_vec(),
            EscapeType::CursorPosition { row, col } => {
                format!("\x1b[{};{}H", row, col).into_bytes()
            }

            // Editing
            EscapeType::EraseDisplay => b"\x1b[2J".to_vec(),
            EscapeType::EraseLine => b"\x1b[2K".to_vec(),
            EscapeType::InsertLine => b"\x1b[L".to_vec(),
            EscapeType::DeleteLine => b"\x1b[M".to_vec(),
            EscapeType::InsertChars => b"\x1b[@".to_vec(),
            EscapeType::DeleteChars => b"\x1b[P".to_vec(),

            // Scrolling
            EscapeType::ScrollUp => b"\x1b[S".to_vec(),
            EscapeType::ScrollDown => b"\x1b[T".to_vec(),
            EscapeType::SetScrollRegion { top, bottom } => {
                format!("\x1b[{};{}r", top, bottom).into_bytes()
            }

            // Modes
            EscapeType::SetMode { mode } => format!("\x1b[{}h", mode).into_bytes(),
            EscapeType::ResetMode { mode } => format!("\x1b[{}l", mode).into_bytes(),
            EscapeType::SetPrivateMode { mode } => format!("\x1b[?{}h", mode).into_bytes(),
            EscapeType::ResetPrivateMode { mode } => format!("\x1b[?{}l", mode).into_bytes(),

            // Graphics
            EscapeType::SetGraphicRendition { params } => {
                let param_str: Vec<String> = params.iter().map(|p| p.to_string()).collect();
                format!("\x1b[{}m", param_str.join(";")).into_bytes()
            }

            // Tab stops
            EscapeType::SetTabStop => b"\x1bH".to_vec(),
            EscapeType::ClearTabStop => b"\x1b[g".to_vec(),
            EscapeType::ClearAllTabStops => b"\x1b[3g".to_vec(),

            // Character sets
            EscapeType::SetCharsetG0 => b"\x1b(B".to_vec(),
            EscapeType::SetCharsetG1 => b"\x1b)0".to_vec(),
            EscapeType::ShiftIn => b"\x0f".to_vec(),
            EscapeType::ShiftOut => b"\x0e".to_vec(),

            // Save/Restore
            EscapeType::SaveCursor => b"\x1b7".to_vec(),
            EscapeType::RestoreCursor => b"\x1b8".to_vec(),
            EscapeType::SaveCursorAndAttrs => b"\x1b[s".to_vec(),
            EscapeType::RestoreCursorAndAttrs => b"\x1b[u".to_vec(),

            // Screen
            EscapeType::AlternateScreenOn => b"\x1b[?1049h".to_vec(),
            EscapeType::AlternateScreenOff => b"\x1b[?1049l".to_vec(),

            // OSC sequences
            EscapeType::SetTitle { title } => {
                let mut bytes = b"\x1b]0;".to_vec();
                bytes.extend_from_slice(title);
                bytes.push(0x07);
                bytes
            }
            EscapeType::SetIconName { name } => {
                let mut bytes = b"\x1b]1;".to_vec();
                bytes.extend_from_slice(name);
                bytes.push(0x07);
                bytes
            }
            EscapeType::SetColor { idx, r, g, b } => {
                format!("\x1b]4;{};rgb:{:02x}/{:02x}/{:02x}\x07", idx, r, g, b).into_bytes()
            }

            // DCS sequences
            EscapeType::Sixel { data } => {
                let mut bytes = b"\x1bPq".to_vec();
                bytes.extend_from_slice(data);
                bytes.extend_from_slice(b"\x1b\\");
                bytes
            }

            // Special
            EscapeType::BracketedPasteOn => b"\x1b[?2004h".to_vec(),
            EscapeType::BracketedPasteOff => b"\x1b[?2004l".to_vec(),
            EscapeType::ApplicationCursorOn => b"\x1b[?1h".to_vec(),
            EscapeType::ApplicationCursorOff => b"\x1b[?1l".to_vec(),
        }
    }
}

/// Execute a command on a terminal and verify invariants.
fn execute_command(terminal: &mut Terminal, cmd: &TerminalCommand) {
    match cmd {
        TerminalCommand::Feed { data } => {
            terminal.process(data);
        }
        TerminalCommand::FeedEscape { escape_type } => {
            terminal.process(&escape_type.to_bytes());
        }
        TerminalCommand::Resize { rows, cols } => {
            // Clamp to reasonable values
            let rows = (*rows).max(1).min(500);
            let cols = (*cols).max(1).min(1000);
            terminal.resize(rows, cols);
        }
        TerminalCommand::GetCursor => {
            let _cursor = terminal.cursor();
        }
        TerminalCommand::GetTitle => {
            let _title = terminal.title();
        }
        TerminalCommand::GetModes => {
            let _modes = terminal.modes();
        }
        TerminalCommand::Reset => {
            terminal.reset();
        }
        TerminalCommand::ScrollDown { lines } => {
            let lines = (*lines).min(1000) as i32;
            terminal.scroll_display(-lines);
        }
        TerminalCommand::ScrollUp { lines } => {
            let lines = (*lines).min(1000) as i32;
            terminal.scroll_display(lines);
        }
    }

    // Verify invariants after each command
    verify_invariants(terminal);
}

/// Verify terminal invariants hold after an operation.
fn verify_invariants(terminal: &Terminal) {
    let cursor = terminal.cursor();
    let cols = terminal.cols();
    let rows = terminal.rows();

    // Cursor must be within bounds (or at cols for wrap pending)
    assert!(
        cursor.col <= cols,
        "Cursor col {} > cols {}",
        cursor.col,
        cols
    );
    assert!(
        cursor.row < rows,
        "Cursor row {} >= rows {}",
        cursor.row,
        rows
    );

    // Terminal dimensions must be valid
    assert!(cols >= 1, "Cols must be >= 1, got {}", cols);
    assert!(rows >= 1, "Rows must be >= 1, got {}", rows);
}

fuzz_target!(|data: &[u8]| {
    // === Phase 1: Raw data feeding ===
    {
        let mut terminal = Terminal::new(24, 80);
        terminal.process(data);
        verify_invariants(&terminal);
    }

    // === Phase 2: Structured command sequences ===
    if let Ok(commands) = <Vec<TerminalCommand> as Arbitrary>::arbitrary(
        &mut Unstructured::new(data)
    ) {
        let mut terminal = Terminal::new(24, 80);

        for cmd in commands.iter().take(100) {  // Limit command count
            execute_command(&mut terminal, cmd);
        }
    }

    // === Phase 3: Interleaved raw data and commands ===
    if data.len() >= 10 {
        let mut terminal = Terminal::new(24, 80);

        // Split data into chunks
        let chunks: Vec<&[u8]> = data.chunks(data.len() / 5 + 1).collect();

        for chunk in chunks.iter() {
            // Feed raw data
            terminal.process(chunk);
            verify_invariants(&terminal);

            // Do some operations based on chunk content
            if !chunk.is_empty() {
                match chunk[0] % 6 {
                    0 => {
                        let rows = 10 + (chunk[0] % 100) as u16;
                        let cols = 20 + (chunk[0] % 200) as u16;
                        terminal.resize(rows, cols);
                    }
                    1 => { terminal.reset(); }
                    2 => {
                        terminal.scroll_display(chunk[0] as i32 % 50);
                    }
                    3..=5 => {
                        // Just get state
                        let _ = terminal.cursor();
                        let _ = terminal.title();
                        let _ = terminal.modes();
                    }
                    _ => {}
                }
                verify_invariants(&terminal);
            }
        }
    }

    // === Phase 4: Rapid resize testing ===
    if data.len() >= 4 {
        let mut terminal = Terminal::new(24, 80);

        // Fill with content
        terminal.process(b"Test content\r\nLine 2\r\nLine 3\r\n");

        for i in 0..data.len().min(50) {
            let rows = 5 + (data[i] % 100) as u16;
            let cols = 10 + (data[(i + 1) % data.len()] % 200) as u16;

            terminal.resize(rows, cols);
            verify_invariants(&terminal);

            // Feed more data after resize
            terminal.process(b"After resize\r\n");
            verify_invariants(&terminal);
        }
    }

    // === Phase 5: Alternate screen stress ===
    if data.len() >= 2 {
        let mut terminal = Terminal::new(24, 80);

        // Fill main screen
        terminal.process(b"Main screen content\r\n");

        for &byte in data.iter().take(100) {
            if byte % 2 == 0 {
                terminal.process(b"\x1b[?1049h");  // Enter alternate screen
                terminal.process(b"Alternate screen\r\n");
            } else {
                terminal.process(b"\x1b[?1049l");  // Exit alternate screen
            }
            verify_invariants(&terminal);
        }

        // Ensure we're back on main screen
        terminal.process(b"\x1b[?1049l");
        verify_invariants(&terminal);
    }

    // === Phase 6: Scrollback stress ===
    if data.len() >= 4 {
        let mut terminal = Terminal::new(24, 80);

        // Generate lots of lines to create scrollback
        for i in 0..100 {
            terminal.process(format!("Line {}: Some content here\r\n", i).as_bytes());
        }

        // Scroll through scrollback
        for i in 0..data.len().min(20) {
            let scroll_amount = (data[i] as i32) - 128;  // Range -128 to 127
            terminal.scroll_display(scroll_amount);
            verify_invariants(&terminal);
        }
    }
});
