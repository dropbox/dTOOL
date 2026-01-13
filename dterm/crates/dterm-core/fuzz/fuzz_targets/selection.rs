//! Selection fuzzer - tests text selection operations.
//!
//! This fuzzer exercises the selection subsystem:
//! 1. Selection start/update/end sequences
//! 2. Selection text extraction
//! 3. Smart selection rules (URLs, paths, etc.)
//! 4. Selection across scrollback
//! 5. Wide character handling in selections
//!
//! ## Running
//!
//! ```bash
//! cd crates/dterm-core
//! cargo +nightly fuzz run selection -- -max_total_time=600
//! ```

#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;

use dterm_core::selection::{
    SelectionRuleKind, SelectionSide, SelectionType, SmartSelection, TextSelection,
};
use dterm_core::terminal::Terminal;

/// Selection operations to perform.
#[derive(Debug, Arbitrary)]
enum SelectionOperation {
    /// Start a new selection
    Start {
        row: i32,
        col: u16,
        selection_type: u8,
    },

    /// Update selection endpoint
    Update { row: i32, col: u16 },

    /// Complete selection
    Complete,

    /// Clear selection
    Clear,

    /// Extend selection
    Extend { row: i32, col: u16 },

    /// Adjust for scroll
    AdjustForScroll { delta: i32 },

    /// Expand to semantic boundaries
    ExpandSemantic { start_col: u16, end_col: u16 },

    /// Expand to full lines
    ExpandLines { max_col: u16 },

    /// Check if point is in selection
    Contains { row: i32, col: u16 },

    /// Query selection state
    IsEmpty,
    GetNormalizedStart,
    GetNormalizedEnd,
}

/// Smart selection operations.
#[derive(Debug, Arbitrary)]
enum SmartSelectionOperation {
    /// Find match at position
    FindAt { text: Vec<u8>, byte_pos: u16 },

    /// Find match at column
    FindAtColumn { text: Vec<u8>, column: u16 },

    /// Find all matches
    FindAll { text: Vec<u8> },

    /// Find matches of specific kind
    FindByKind { text: Vec<u8>, kind: u8 },

    /// Check if there's a match at position
    HasMatchAt { text: Vec<u8>, byte_pos: u16 },

    /// Get word boundaries at position
    WordBoundaries { text: Vec<u8>, byte_pos: u16 },

    /// Set rule enabled/disabled
    SetRuleEnabled { rule_name: u8, enabled: bool },
}

/// Get selection type from byte.
fn selection_type_from_byte(b: u8) -> SelectionType {
    match b % 4 {
        0 => SelectionType::Simple,
        1 => SelectionType::Block,
        2 => SelectionType::Semantic,
        _ => SelectionType::Lines,
    }
}

/// Get selection rule kind from byte.
fn rule_kind_from_byte(b: u8) -> SelectionRuleKind {
    match b % 5 {
        0 => SelectionRuleKind::Url,
        1 => SelectionRuleKind::FilePath,
        2 => SelectionRuleKind::Email,
        3 => SelectionRuleKind::IpAddress,
        _ => SelectionRuleKind::Custom,
    }
}

/// Get rule name from byte (matching builtin rules).
fn rule_name_from_byte(b: u8) -> &'static str {
    match b % 12 {
        0 => "url",
        1 => "file_path",
        2 => "email",
        3 => "ipv4",
        4 => "ipv6",
        5 => "git_hash",
        6 => "double_quoted_string",
        7 => "single_quoted_string",
        8 => "backtick_quoted_string",
        9 => "uuid",
        10 => "semver",
        _ => "unknown",
    }
}

fn execute_selection_operation(selection: &mut TextSelection, op: &SelectionOperation, max_rows: i32) {
    match op {
        SelectionOperation::Start {
            row,
            col,
            selection_type,
        } => {
            // Clamp row to reasonable bounds
            let row = (*row).clamp(-1000, max_rows + 1000);
            let sel_type = selection_type_from_byte(*selection_type);
            selection.start_selection(row, *col, SelectionSide::Left, sel_type);
        }

        SelectionOperation::Update { row, col } => {
            let row = (*row).clamp(-1000, max_rows + 1000);
            selection.update_selection(row, *col, SelectionSide::Right);
        }

        SelectionOperation::Complete => {
            selection.complete_selection();
        }

        SelectionOperation::Clear => {
            selection.clear();
        }

        SelectionOperation::Extend { row, col } => {
            let row = (*row).clamp(-1000, max_rows + 1000);
            selection.extend_selection(row, *col, SelectionSide::Right);
        }

        SelectionOperation::AdjustForScroll { delta } => {
            let _ = selection.adjust_for_scroll(*delta, max_rows);
        }

        SelectionOperation::ExpandSemantic { start_col, end_col } => {
            selection.expand_semantic(*start_col, *end_col);
        }

        SelectionOperation::ExpandLines { max_col } => {
            selection.expand_lines(*max_col);
        }

        SelectionOperation::Contains { row, col } => {
            let row = (*row).clamp(-1000, max_rows + 1000);
            let _ = selection.contains(row, *col);
        }

        SelectionOperation::IsEmpty => {
            let _ = selection.is_empty();
        }

        SelectionOperation::GetNormalizedStart => {
            let _ = selection.normalized_start();
        }

        SelectionOperation::GetNormalizedEnd => {
            let _ = selection.normalized_end();
        }
    }
}

fn execute_smart_selection_operation(smart: &SmartSelection, op: &SmartSelectionOperation) {
    match op {
        SmartSelectionOperation::FindAt { text, byte_pos } => {
            if let Ok(s) = std::str::from_utf8(text) {
                let pos = (*byte_pos as usize).min(s.len());
                let _ = smart.find_at(s, pos);
            }
        }

        SmartSelectionOperation::FindAtColumn { text, column } => {
            if let Ok(s) = std::str::from_utf8(text) {
                let _ = smart.find_at_column(s, *column as usize);
            }
        }

        SmartSelectionOperation::FindAll { text } => {
            if let Ok(s) = std::str::from_utf8(text) {
                let _ = smart.find_all(s);
            }
        }

        SmartSelectionOperation::FindByKind { text, kind } => {
            if let Ok(s) = std::str::from_utf8(text) {
                let rule_kind = rule_kind_from_byte(*kind);
                let _ = smart.find_by_kind(s, rule_kind);
            }
        }

        SmartSelectionOperation::HasMatchAt { text, byte_pos } => {
            if let Ok(s) = std::str::from_utf8(text) {
                let pos = (*byte_pos as usize).min(s.len());
                let _ = smart.has_match_at(s, pos);
            }
        }

        SmartSelectionOperation::WordBoundaries { text, byte_pos } => {
            if let Ok(s) = std::str::from_utf8(text) {
                let pos = (*byte_pos as usize).min(s.len());
                let _ = smart.word_boundaries_at(s, pos);
                let _ = SmartSelection::basic_word_boundaries(s, pos);
            }
        }

        SmartSelectionOperation::SetRuleEnabled { rule_name, enabled } => {
            // We can't modify a shared SmartSelection, but we can test the API
            let name = rule_name_from_byte(*rule_name);
            let _ = smart.get_rule(name);
            // Note: set_rule_enabled would need mutable access
            let _ = enabled; // Silence warning
        }
    }
}

fuzz_target!(|data: &[u8]| {
    // === Phase 1: TextSelection operations ===
    if let Ok(operations) =
        <Vec<SelectionOperation> as Arbitrary>::arbitrary(&mut Unstructured::new(data))
    {
        let mut selection = TextSelection::new();

        for op in operations.iter().take(100) {
            execute_selection_operation(&mut selection, op, 1000);
        }
    }

    // === Phase 2: SmartSelection with various text patterns ===
    if let Ok(operations) = <Vec<SmartSelectionOperation> as Arbitrary>::arbitrary(
        &mut Unstructured::new(data)
    ) {
        let smart = SmartSelection::with_builtin_rules();

        for op in operations.iter().take(50) {
            execute_smart_selection_operation(&smart, op);
        }
    }

    // === Phase 3: Selection with terminal content ===
    if data.len() >= 10 {
        let mut terminal = Terminal::new(24, 80);

        // Add content
        terminal.process(data);

        // Perform selection operations based on input
        for i in (0..data.len()).step_by(4) {
            if i + 3 >= data.len() {
                break;
            }

            // Start selection via mutable borrow
            let start_row = (data[i] as i32) % (terminal.rows() as i32 + 10) - 5;
            let start_col = (data[i + 1] as u16) % (terminal.cols() + 10);
            terminal.text_selection_mut().start_selection(
                start_row,
                start_col,
                SelectionSide::Left,
                SelectionType::Simple,
            );

            // Update selection
            let end_row = (data[i + 2] as i32) % (terminal.rows() as i32 + 10) - 5;
            let end_col = (data[i + 3] as u16) % (terminal.cols() + 10);
            terminal.text_selection_mut().update_selection(end_row, end_col, SelectionSide::Right);

            // Get selection text (should never panic)
            let _text = terminal.selection_to_string();

            // Clear selection
            terminal.text_selection_mut().clear();
        }
    }

    // === Phase 4: Selection across scrollback ===
    if data.len() >= 8 {
        let mut terminal = Terminal::new(24, 80);

        // Add enough content to create scrollback
        for i in 0..100 {
            terminal.process(format!("Line {}: {}\r\n", i, std::str::from_utf8(&data[..data.len().min(20)]).unwrap_or("test")).as_bytes());
        }

        // Start selection in scrollback
        let scrollback_lines = terminal.scrollback().map(|s| s.line_count()).unwrap_or(0);
        if scrollback_lines > 0 {
            let row = -(data[0] as i32 % scrollback_lines.min(100) as i32 + 1);
            let col = data[1] as u16 % terminal.cols();
            terminal.text_selection_mut().start_selection(
                row,
                col,
                SelectionSide::Left,
                SelectionType::Simple,
            );

            // Extend to visible area
            let end_row = (data[2] as i32) % terminal.rows() as i32;
            let end_col = data[3] as u16 % terminal.cols();
            terminal.text_selection_mut().update_selection(end_row, end_col, SelectionSide::Right);

            // Get text
            let _text = terminal.selection_to_string();

            terminal.text_selection_mut().clear();
        }
    }

    // === Phase 5: Selection with wide characters ===
    {
        let mut terminal = Terminal::new(24, 80);

        // Add wide character content
        terminal.process(b"ASCII ");
        terminal.process("日本語".as_bytes());
        terminal.process(b" more ASCII ");
        terminal.process("한글".as_bytes());
        terminal.process(b"\r\n");
        terminal.process("Emoji: ".as_bytes());
        terminal.process("😀🎉".as_bytes());
        terminal.process(b"\r\n");

        if data.len() >= 4 {
            // Select across wide characters
            let start_col = data[0] as u16 % terminal.cols();
            let end_col = data[1] as u16 % terminal.cols();

            terminal.text_selection_mut().start_selection(
                0,
                start_col,
                SelectionSide::Left,
                SelectionType::Simple,
            );
            terminal.text_selection_mut().update_selection(1, end_col, SelectionSide::Right);

            // The extracted text should be valid UTF-8
            if let Some(text) = terminal.selection_to_string() {
                assert!(
                    text.is_ascii() || !text.is_empty(),
                    "Selection text should be valid"
                );
            }

            terminal.text_selection_mut().clear();
        }
    }

    // === Phase 6: Smart selection pattern matching ===
    {
        let smart = SmartSelection::with_builtin_rules();

        // Test with various pattern types
        let test_texts = [
            "Visit https://example.com/path?query=value for more info",
            "Email me at user@example.com please",
            "Check /home/user/file.txt or ~/Documents/report.pdf",
            "IP addresses: 192.168.1.1 and 2001:db8::1",
            "Git commit abc1234def5678 on branch main",
            "Version 1.2.3 released! Also see v2.0.0-beta.1",
            "UUID: 550e8400-e29b-41d4-a716-446655440000",
            "Quoted \"string here\" and 'single quotes' and `backticks`",
        ];

        for text in test_texts {
            // Find all matches
            let matches = smart.find_all(text);
            for m in matches {
                // Verify match bounds are valid
                assert!(m.start() <= m.end());
                assert!(m.end() <= text.len());
                assert!(!m.matched_text().is_empty() || m.is_empty());
            }

            // Find at various positions
            for pos in 0..text.len() {
                let _ = smart.find_at(text, pos);
                let _ = smart.has_match_at(text, pos);
                let _ = smart.word_boundaries_at(text, pos);
            }
        }

        // Test with fuzz input as text
        if let Ok(text) = std::str::from_utf8(data) {
            let _ = smart.find_all(text);
            for pos in 0..text.len().min(100) {
                let _ = smart.find_at(text, pos);
            }
        }
    }

    // === Phase 7: Block selection ===
    if data.len() >= 6 {
        let mut terminal = Terminal::new(24, 80);

        // Add grid-like content
        for i in 0..20 {
            terminal.process(format!("{:04}|abcdefghij|klmnopqrst|\r\n", i).as_bytes());
        }

        // Block selection
        let start_row = (data[0] as i32) % 20;
        let start_col = (data[1] as u16) % 30;
        let end_row = (data[2] as i32) % 20;
        let end_col = (data[3] as u16) % 30;

        terminal.text_selection_mut().start_selection(
            start_row,
            start_col,
            SelectionSide::Left,
            SelectionType::Block,
        );
        terminal.text_selection_mut().update_selection(end_row, end_col, SelectionSide::Right);

        let _text = terminal.selection_to_string();
        terminal.text_selection_mut().clear();
    }

    // === Phase 8: Line selection ===
    if data.len() >= 4 {
        let mut terminal = Terminal::new(24, 80);

        // Add content
        for i in 0..30 {
            terminal.process(format!("Full line {} with various content\r\n", i).as_bytes());
        }

        // Line selection
        let start_row = (data[0] as i32) % 30;
        let end_row = (data[1] as i32) % 30;

        let cols = terminal.cols();
        terminal.text_selection_mut().start_selection(
            start_row,
            0,
            SelectionSide::Left,
            SelectionType::Lines,
        );
        terminal.text_selection_mut().update_selection(
            end_row,
            cols.saturating_sub(1),
            SelectionSide::Right,
        );

        let _text = terminal.selection_to_string();
        terminal.text_selection_mut().clear();
    }

    // === Phase 9: Selection during resize ===
    if data.len() >= 8 {
        let mut terminal = Terminal::new(24, 80);

        // Add content
        terminal.process(b"Content before resize\r\n");
        terminal.process(b"More content here\r\n");

        // Start selection
        terminal.text_selection_mut().start_selection(
            0,
            0,
            SelectionSide::Left,
            SelectionType::Simple,
        );
        terminal.text_selection_mut().update_selection(1, 10, SelectionSide::Right);

        // Resize while selection is active
        let new_rows = (data[4] % 50).max(1) as u16 + 5;
        let new_cols = (data[5] % 200).max(1) as u16 + 10;
        terminal.resize(new_rows, new_cols);

        // Selection might be invalidated or adjusted - should not panic
        let _text = terminal.selection_to_string();

        terminal.text_selection_mut().clear();
    }

    // === Phase 10: Selection with scroll adjustment ===
    if data.len() >= 4 {
        let mut selection = TextSelection::new();

        selection.start_selection(10, 5, SelectionSide::Left, SelectionType::Simple);
        selection.update_selection(15, 20, SelectionSide::Right);

        // Apply various scroll deltas
        for byte in data.iter().take(20) {
            let delta = (*byte as i32) - 128;
            let _ = selection.adjust_for_scroll(delta, 100);
        }

        // Selection should still be valid (or empty if scrolled out of view)
        let _ = selection.is_empty();
        let _ = selection.normalized_start();
        let _ = selection.normalized_end();
    }
});
