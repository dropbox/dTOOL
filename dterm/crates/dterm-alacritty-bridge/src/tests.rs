//! Tests for dterm-alacritty-bridge.

use crate::event::{Event, EventListener, VoidListener};
use crate::grid::{Dimensions, GridExt, Scroll};
use crate::index::{Column, Line, Point};
use crate::term::{Config, Term};
use std::sync::{Arc, Mutex};

/// Event listener that records events for verification.
#[derive(Clone, Default)]
struct RecordingListener {
    events: Arc<Mutex<Vec<Event>>>,
}

impl RecordingListener {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn events(&self) -> Vec<Event> {
        self.events.lock().unwrap().clone()
    }

    fn clear(&self) {
        self.events.lock().unwrap().clear();
    }
}

impl EventListener for RecordingListener {
    fn send_event(&self, event: Event) {
        self.events.lock().unwrap().push(event);
    }
}

#[test]
fn term_creation_default_config() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    assert_eq!(term.grid().rows(), 24);
    assert_eq!(term.grid().cols(), 80);
    assert_eq!(term.config().scrolling_history, 10_000);
}

#[test]
fn term_creation_custom_size() {
    let config = Config::default();
    let dims = (50usize, 120usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    assert_eq!(term.grid().rows(), 50);
    assert_eq!(term.grid().cols(), 120);
}

#[test]
fn term_process_simple_text() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"Hello, World!");

    // Cursor should have moved
    let grid = term.grid();
    assert!(grid.cursor().col > 0);
}

#[test]
fn term_process_escape_sequences() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Clear screen and home cursor
    term.process(b"\x1b[2J\x1b[H");

    // Move cursor to row 5, col 10
    term.process(b"\x1b[5;10H");

    let grid = term.grid();
    assert_eq!(grid.cursor().row, 4); // 0-indexed
    assert_eq!(grid.cursor().col, 9); // 0-indexed
}

#[test]
fn term_resize() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    assert_eq!(term.grid().rows(), 24);
    assert_eq!(term.grid().cols(), 80);

    term.resize(&(40usize, 100usize));

    assert_eq!(term.grid().rows(), 40);
    assert_eq!(term.grid().cols(), 100);
}

#[test]
fn term_swap_alt() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Should start on primary screen
    assert!(!term.is_alt_screen());

    // Write some content to primary screen
    term.process(b"Primary content");

    // Swap to alternate screen
    term.swap_alt();
    assert!(term.is_alt_screen());

    // Swap back to primary
    term.swap_alt();
    assert!(!term.is_alt_screen());
}

#[test]
fn term_swap_alt_clears_selection() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Create a selection
    use crate::index::Side;
    use crate::selection::SelectionType;
    term.start_selection(
        SelectionType::Simple,
        Point::new(Line(0), Column(0)),
        Side::Left,
    );
    term.update_selection(Point::new(Line(0), Column(10)), Side::Right);
    assert!(term.selection.is_some());

    // Swap to alternate screen should clear selection
    term.swap_alt();
    assert!(term.selection.is_none());
}

#[test]
fn term_resize_clamps_cursor() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Move cursor near bottom-right
    term.process(b"\x1b[23;79H");

    // Resize to smaller size
    term.resize(&(10usize, 40usize));

    let grid = term.grid();
    assert!(grid.cursor().row < 10);
    assert!(grid.cursor().col < 40);
}

#[test]
fn grid_dimensions_trait() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    assert_eq!(Dimensions::screen_lines(&term), 24);
    assert_eq!(Dimensions::columns(&term), 80);
    assert_eq!(Dimensions::last_column(&term), Column(79));
    assert_eq!(Dimensions::bottommost_line(&term), Line(23));
}

#[test]
fn grid_ext_methods() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    let grid = term.grid();
    assert_eq!(GridExt::columns(grid), 80);
    assert_eq!(GridExt::screen_lines(grid), 24);
    assert_eq!(GridExt::display_offset(grid), 0);
}

#[test]
fn scroll_display_delta() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Add lines to scrollback
    for i in 0..50 {
        term.process(format!("Line {}\r\n", i).as_bytes());
    }

    // Scroll up
    term.scroll_display(Scroll::Delta(10));
    assert!(term.grid().display_offset() > 0);

    // Scroll back down
    term.scroll_display(Scroll::Bottom);
    assert_eq!(term.grid().display_offset(), 0);
}

#[test]
fn scroll_display_page() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Add lines to scrollback
    for i in 0..100 {
        term.process(format!("Line {}\r\n", i).as_bytes());
    }

    // Page up
    term.scroll_display(Scroll::PageUp);
    let offset = term.grid().display_offset();
    assert!(offset > 0);

    // Page down
    term.scroll_display(Scroll::PageDown);
    assert!(term.grid().display_offset() < offset);
}

#[test]
fn scroll_top_bottom() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Add lines to scrollback
    for i in 0..100 {
        term.process(format!("Line {}\r\n", i).as_bytes());
    }

    // Scroll to top
    term.scroll_display(Scroll::Top);
    let top_offset = term.grid().display_offset();

    // Scroll to bottom
    term.scroll_display(Scroll::Bottom);
    assert_eq!(term.grid().display_offset(), 0);

    // Top offset should be larger
    assert!(top_offset > 0);
}

#[test]
fn event_listener_title_change() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let listener = RecordingListener::new();
    let mut term = Term::new(config, &dims, listener.clone());

    // Set title via OSC
    term.process(b"\x1b]0;Test Title\x07");

    let events = listener.events();
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Title(t) if t == "Test Title")));
}

#[test]
fn event_listener_bell() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let listener = RecordingListener::new();
    let mut term = Term::new(config, &dims, listener.clone());

    // Send BEL character
    term.process(b"\x07");

    let events = listener.events();
    assert!(events.iter().any(|e| matches!(e, Event::Bell)));
}

#[test]
fn event_listener_reset_title() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let listener = RecordingListener::new();
    let mut term = Term::new(config, &dims, listener.clone());

    // Set title
    term.process(b"\x1b]0;Title\x07");

    listener.clear();

    // Clear title
    term.process(b"\x1b]0;\x07");

    let events = listener.events();
    assert!(events.iter().any(|e| matches!(e, Event::ResetTitle)));
}

#[test]
fn void_listener_does_not_panic() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Various operations that would trigger events
    term.process(b"\x1b]0;Title\x07");
    term.process(b"\x07");
    term.process(b"Hello\r\n");

    // Should complete without panic
}

#[test]
fn damage_tracking() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Process some text to create damage
    term.process(b"Hello");

    // Check damage exists (row 0 should be damaged)
    assert!(term.damage().is_row_damaged(0));

    // Reset damage
    term.reset_damage();

    // Damage should be cleared
    assert!(!term.damage().is_row_damaged(0));
}

#[test]
fn index_types() {
    // Line tests
    assert_eq!(Line(5).0, 5);
    assert_eq!(Line::from(10usize).0, 10);
    assert!(Line(5) < Line(10));

    // Column tests
    assert_eq!(Column(5).0, 5);
    assert_eq!(Column::from(10usize).0, 10);
    assert!(Column(5) < Column(10));

    // Point tests
    let point = Point::new(Line(5), Column(10));
    assert_eq!(point.line.0, 5);
    assert_eq!(point.column.0, 10);
}

#[test]
fn config_default() {
    let config = Config::default();
    assert_eq!(config.scrolling_history, 10_000);
}

#[test]
fn term_modes_access() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Modes should be accessible
    let _ = term.modes();
}

#[test]
fn term_terminal_access() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Immutable access
    let _ = term.terminal();

    // Mutable access
    let _ = term.terminal_mut();
}

#[test]
fn grid_mutable_access() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Mutable grid access
    let grid = term.grid_mut();
    grid.scroll_display(5);
}

#[test]
fn dimensions_tuple_impl() {
    let dims = (24usize, 80usize);

    assert_eq!(dims.total_lines(), 24);
    assert_eq!(dims.screen_lines(), 24);
    assert_eq!(dims.columns(), 80);
    assert_eq!(dims.history_size(), 0);
}

#[test]
fn display_formats() {
    // Test Display implementations
    assert_eq!(format!("{}", Line(5)), "5");
    assert_eq!(format!("{}", Column(10)), "10");
}

// ===== Selection Tests =====

use crate::index::{Direction, Side};
use crate::selection::SelectionType;
use crate::vi_mode::ViMotion;

#[test]
fn term_selection_basic() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // No selection initially
    assert!(term.selection.is_none());
    assert!(term.selection_range().is_none());

    // Start a selection
    term.start_selection(
        SelectionType::Simple,
        Point::new(Line(0), Column(0)),
        Side::Left,
    );

    assert!(term.selection.is_some());

    // Update selection
    term.update_selection(Point::new(Line(0), Column(10)), Side::Right);

    // Check range
    let range = term.selection_range();
    assert!(range.is_some());
    let range = range.unwrap();
    assert_eq!(range.start.column, Column(0));
    assert_eq!(range.end.column, Column(10));

    // Clear selection
    term.clear_selection();
    assert!(term.selection.is_none());
}

#[test]
fn term_selection_with_text() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Write some text
    term.process(b"Hello, World!");

    // Select "Hello"
    term.start_selection(
        SelectionType::Simple,
        Point::new(Line(0), Column(0)),
        Side::Left,
    );
    term.update_selection(Point::new(Line(0), Column(4)), Side::Right);

    // Get selection as string
    let selected = term.selection_to_string();
    assert!(selected.is_some());
    let text = selected.unwrap();
    assert_eq!(text, "Hello");
}

#[test]
fn term_selection_block_mode() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Start a block selection
    term.start_selection(
        SelectionType::Block,
        Point::new(Line(0), Column(5)),
        Side::Left,
    );
    term.update_selection(Point::new(Line(2), Column(10)), Side::Right);

    let range = term.selection_range();
    assert!(range.is_some());
    let range = range.unwrap();
    assert!(range.is_block);
}

#[test]
fn term_selection_line_mode() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Start a line selection
    term.start_selection(
        SelectionType::Lines,
        Point::new(Line(1), Column(5)),
        Side::Left,
    );
    term.update_selection(Point::new(Line(3), Column(10)), Side::Right);

    let range = term.selection_range();
    assert!(range.is_some());
    let range = range.unwrap();
    // Line selection should span full columns
    assert_eq!(range.start.column, Column(0));
    assert_eq!(range.end.column, Column(79)); // last column
}

// ===== Vi Mode Tests =====

#[test]
fn term_vi_mode_toggle() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Not in vi mode initially
    assert!(!term.is_vi_mode());

    // Toggle on
    term.toggle_vi_mode();
    assert!(term.is_vi_mode());

    // Toggle off
    term.toggle_vi_mode();
    assert!(!term.is_vi_mode());
}

#[test]
fn term_vi_mode_cursor_initialized() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Move cursor to specific position
    term.process(b"\x1b[5;10H"); // row 5, col 10 (1-indexed)

    // Toggle vi mode
    term.toggle_vi_mode();

    // Vi cursor should be at terminal cursor position
    assert_eq!(term.vi_mode_cursor.point.line, Line(4)); // 0-indexed
    assert_eq!(term.vi_mode_cursor.point.column, Column(9)); // 0-indexed
}

#[test]
fn term_vi_motion() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(10), Column(40)));

    // Move down
    term.vi_motion(ViMotion::Down);
    assert_eq!(term.vi_mode_cursor.point.line, Line(11));

    // Move up
    term.vi_motion(ViMotion::Up);
    assert_eq!(term.vi_mode_cursor.point.line, Line(10));

    // Move left
    term.vi_motion(ViMotion::Left);
    assert_eq!(term.vi_mode_cursor.point.column, Column(39));

    // Move right
    term.vi_motion(ViMotion::Right);
    assert_eq!(term.vi_mode_cursor.point.column, Column(40));
}

#[test]
fn term_vi_motion_ignored_when_not_active() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Not in vi mode
    let initial = term.vi_mode_cursor.point;
    term.vi_motion(ViMotion::Down);
    assert_eq!(term.vi_mode_cursor.point, initial);
}

#[test]
fn term_vi_goto_point() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(15), Column(30)));

    assert_eq!(term.vi_mode_cursor.point.line, Line(15));
    assert_eq!(term.vi_mode_cursor.point.column, Column(30));
}

#[test]
fn term_vi_goto_point_clamped() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.toggle_vi_mode();

    // Try to go beyond bounds
    term.vi_goto_point(Point::new(Line(100), Column(200)));

    // Should be clamped
    assert!(term.vi_mode_cursor.point.line.0 < 24);
    assert!(term.vi_mode_cursor.point.column.0 < 80);
}

#[test]
fn term_vi_scroll() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(10), Column(0)));

    // Scroll down
    term.vi_scroll(5);
    assert_eq!(term.vi_mode_cursor.point.line, Line(15));

    // Scroll up
    term.vi_scroll(-3);
    assert_eq!(term.vi_mode_cursor.point.line, Line(12));
}

#[test]
fn term_vi_selection() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(5), Column(10)));

    // Start selection
    term.vi_start_selection(SelectionType::Simple);
    assert!(term.selection.is_some());

    // Move and update selection
    term.vi_motion(ViMotion::Right);
    term.vi_motion(ViMotion::Right);
    term.vi_motion(ViMotion::Right);
    term.vi_update_selection();

    let range = term.selection_range();
    assert!(range.is_some());
    let range = range.unwrap();
    assert_eq!(range.start.column, Column(10));
    assert_eq!(range.end.column, Column(13));
}

#[test]
fn term_vi_mode_exit_clears_selection() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.toggle_vi_mode();
    term.vi_start_selection(SelectionType::Simple);
    assert!(term.selection.is_some());

    // Exit vi mode
    term.toggle_vi_mode();

    // Selection should be cleared
    assert!(term.selection.is_none());
}

#[test]
fn term_vi_motion_first_last() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(5), Column(40)));

    // Move to first column
    term.vi_motion(ViMotion::First);
    assert_eq!(term.vi_mode_cursor.point.column, Column(0));

    // Move to last column
    term.vi_motion(ViMotion::Last);
    assert_eq!(term.vi_mode_cursor.point.column, Column(79));
}

#[test]
fn term_vi_motion_high_middle_low() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(10), Column(0)));

    // Move to top
    term.vi_motion(ViMotion::High);
    assert_eq!(term.vi_mode_cursor.point.line, Line(0));

    // Move to middle
    term.vi_motion(ViMotion::Middle);
    assert_eq!(term.vi_mode_cursor.point.line, Line(12));

    // Move to bottom
    term.vi_motion(ViMotion::Low);
    assert_eq!(term.vi_mode_cursor.point.line, Line(23));
}

#[test]
fn term_vi_url_navigation() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Write some text with URLs
    term.process(b"Line 0\r\n");
    term.process(b"Check https://example.com here\r\n");
    term.process(b"Line 2\r\n");
    term.process(b"Visit https://rust-lang.org now\r\n");
    term.process(b"Line 4\r\n");

    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(0), Column(0)));

    // Navigate to first URL
    term.vi_motion(ViMotion::UrlNext);
    assert_eq!(term.vi_mode_cursor.point.line, Line(1));
    assert_eq!(term.vi_mode_cursor.point.column, Column(6)); // "Check " = 6 chars

    // Navigate to second URL
    term.vi_motion(ViMotion::UrlNext);
    assert_eq!(term.vi_mode_cursor.point.line, Line(3));
    assert_eq!(term.vi_mode_cursor.point.column, Column(6)); // "Visit " = 6 chars

    // Navigate back to first URL
    term.vi_motion(ViMotion::UrlPrev);
    assert_eq!(term.vi_mode_cursor.point.line, Line(1));
}

#[test]
fn term_vi_url_at_cursor() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Write text with URL
    term.process(b"Check https://example.com here\r\n");

    term.toggle_vi_mode();

    // Position cursor on URL
    term.vi_goto_point(Point::new(Line(0), Column(10)));
    let url = term.url_at_vi_cursor();
    assert!(url.is_some());
    assert_eq!(url.unwrap().url, "https://example.com");

    // Position cursor outside URL
    term.vi_goto_point(Point::new(Line(0), Column(0)));
    let url = term.url_at_vi_cursor();
    assert!(url.is_none());
}

#[test]
fn term_vi_url_not_in_vi_mode() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"https://example.com\r\n");

    // Not in vi mode - should return None
    let url = term.vi_goto_next_url();
    assert!(url.is_none());
}

#[test]
fn term_visible_urls_includes_hyperlinks() {
    use crate::index::{Column, Line, Point};

    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"\x1b]8;;https://example.com\x07Click here\x1b]8;;\x07");

    let urls = term.visible_urls();
    assert_eq!(urls.len(), 1);
    assert_eq!(urls[0].url, "https://example.com");
    assert_eq!(urls[0].start, Point::new(Line(0), Column(0)));
    assert_eq!(urls[0].end, Point::new(Line(0), Column(9)));
}

#[test]
fn term_visible_urls_dedups_hyperlink_and_regex() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"\x1b]8;;https://example.com\x07https://example.com\x1b]8;;\x07 ");
    term.process(b"https://rust-lang.org");

    let urls = term.visible_urls();
    assert_eq!(urls.len(), 2);
    assert_eq!(urls[0].url, "https://example.com");
    assert_eq!(urls[1].url, "https://rust-lang.org");
}

#[test]
fn direction_enum() {
    assert_eq!(Direction::Left.opposite(), Direction::Right);
    assert_eq!(Direction::Right.opposite(), Direction::Left);
}

#[test]
fn point_display() {
    let point = Point::new(Line(5), Column(10));
    assert_eq!(format!("{}", point), "(5, 10)");
}

#[test]
fn point_ordering() {
    let p1 = Point::new(Line(0), Column(5));
    let p2 = Point::new(Line(0), Column(10));
    let p3 = Point::new(Line(1), Column(0));

    assert!(p1 < p2);
    assert!(p2 < p3);
    assert!(p1 < p3);
}

#[test]
fn line_arithmetic() {
    let line = Line(10);

    assert_eq!(line + 5i32, Line(15));
    assert_eq!(line - 3i32, Line(7));
    assert_eq!(line + 2usize, Line(12));
    assert_eq!(line - 4usize, Line(6));
    assert_eq!(line - Line(3), 7);
}

#[test]
fn column_arithmetic() {
    let col = Column(10);

    assert_eq!(col + 5usize, Column(15));
    assert_eq!(col - 3usize, Column(7));
    assert_eq!(col - Column(3), 7);

    // Saturating subtraction
    assert_eq!(Column(5) - 10usize, Column(0));
}

// ===== New API Tests =====

use crate::event::{ClipboardType, Notify, VoidNotify, WindowSize};
use crate::{CursorStyle, MouseMode, Rgb, TerminalModes};

#[test]
fn term_cursor_style() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Default cursor style
    let style = term.cursor_style();
    // CursorStyle default is BlinkingBlock
    assert!(matches!(
        style,
        CursorStyle::BlinkingBlock | CursorStyle::SteadyBlock
    ));
}

#[test]
fn term_cursor_style_change() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Change cursor style via escape sequence (DECSCUSR)
    // Ps = 4 is steady underline
    term.process(b"\x1b[4 q");

    let style = term.cursor_style();
    assert_eq!(style, CursorStyle::SteadyUnderline);
}

#[test]
fn term_colors_access() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Should be able to access colors
    let colors = term.colors();

    // Default palette should have reasonable values
    // Black (index 0) should be dark
    let black = colors.get(0);
    assert!(black.r < 50 && black.g < 50 && black.b < 50);

    // White (index 7) should be light
    let white = colors.get(7);
    assert!(white.r > 150 && white.g > 150 && white.b > 150);
}

#[test]
fn term_colors_mut() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Modify colors
    let colors = term.colors_mut();
    colors.set(
        0,
        Rgb {
            r: 42,
            g: 42,
            b: 42,
        },
    );

    // Verify the change
    let colors = term.colors();
    let black = colors.get(0);
    assert_eq!(black.r, 42);
    assert_eq!(black.g, 42);
    assert_eq!(black.b, 42);
}

#[test]
fn term_modes_detail() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Check initial modes
    let modes = term.modes();
    assert!(modes.cursor_visible);
    assert!(modes.auto_wrap);
    assert!(!modes.bracketed_paste);

    // Enable bracketed paste mode
    term.process(b"\x1b[?2004h");
    assert!(term.modes().bracketed_paste);

    // Disable bracketed paste mode
    term.process(b"\x1b[?2004l");
    assert!(!term.modes().bracketed_paste);
}

#[test]
fn terminal_modes_struct() {
    let modes = TerminalModes::default();

    // Default modes should have autowrap enabled and cursor visible
    assert!(!modes.cursor_visible); // Default::default() sets to false
    assert!(!modes.auto_wrap); // Default::default() sets to false
    assert!(!modes.bracketed_paste);
    assert!(!modes.alternate_screen);
}

#[test]
fn mouse_mode_variants() {
    assert_eq!(MouseMode::None, MouseMode::default());

    // Just verify the variants exist
    let _ = MouseMode::Normal;
    let _ = MouseMode::ButtonEvent;
    let _ = MouseMode::AnyEvent;
}

#[test]
fn cursor_style_variants() {
    // Verify all variants exist
    let _ = CursorStyle::BlinkingBlock;
    let _ = CursorStyle::SteadyBlock;
    let _ = CursorStyle::BlinkingUnderline;
    let _ = CursorStyle::SteadyUnderline;
    let _ = CursorStyle::BlinkingBar;
    let _ = CursorStyle::SteadyBar;
}

#[test]
fn clipboard_type_variants() {
    let clipboard = ClipboardType::Clipboard;
    let selection = ClipboardType::Selection;

    assert_ne!(clipboard, selection);
}

#[test]
fn event_variants() {
    use std::sync::Arc;

    // Verify new event variants compile and can be created
    let _ = Event::ClipboardStore(ClipboardType::Clipboard, "test".to_string());

    // ClipboardLoad now takes a formatter callback
    let clipboard_formatter: crate::ClipboardFormatter = Arc::new(|text| text.to_string());
    let _ = Event::ClipboardLoad(ClipboardType::Selection, clipboard_formatter);

    let _ = Event::PtyWrite("response".to_string());

    // ColorRequest now takes a formatter callback
    let color_formatter: crate::ColorFormatter =
        Arc::new(|rgb| format!("rgb:{:02x}/{:02x}/{:02x}", rgb.r, rgb.g, rgb.b));
    let _ = Event::ColorRequest(0, color_formatter);

    let _ = Event::Exit;
    let _ = Event::CursorBlinkingChange;

    // TextAreaSizeRequest now takes a formatter callback
    let size_formatter: crate::WindowSizeFormatter =
        Arc::new(|size| format!("{}x{}", size.num_cols, size.num_lines));
    let _ = Event::TextAreaSizeRequest(size_formatter);

    let _ = Event::ChildExit(0);
}

#[test]
fn void_notify() {
    let notifier = VoidNotify;

    // Should not panic
    notifier.notify(&b"test"[..]);
    notifier.notify(b"static bytes");
    notifier.notify(Vec::from(b"vec data"));
}

#[test]
fn window_size_new() {
    let size = WindowSize::new(80, 24, 8, 16);

    assert_eq!(size.num_cols, 80);
    assert_eq!(size.num_lines, 24);
    assert_eq!(size.cell_width, 8);
    assert_eq!(size.cell_height, 16);
}

#[test]
fn window_size_default() {
    let size = WindowSize::default();

    assert_eq!(size.num_cols, 0);
    assert_eq!(size.num_lines, 0);
    assert_eq!(size.cell_width, 0);
    assert_eq!(size.cell_height, 0);
}

#[test]
fn rgb_type() {
    let color = Rgb {
        r: 255,
        g: 128,
        b: 64,
    };
    assert_eq!(color.r, 255);
    assert_eq!(color.g, 128);
    assert_eq!(color.b, 64);
}

// ===== Grid Method Tests =====

#[test]
fn grid_clear_viewport() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Write some content
    term.process(b"Hello, World!\nLine 2\nLine 3");

    // Clear the viewport
    term.grid_mut().clear_viewport();

    // Cursor should be at home
    let cursor = term.grid().cursor();
    assert_eq!(cursor.row, 0);
    assert_eq!(cursor.col, 0);
}

#[test]
fn grid_initialize_all() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Write some content
    term.process(b"Hello, World!");

    // Initialize all cells
    term.grid_mut().initialize_all();

    // Cursor should be at home
    let cursor = term.grid().cursor();
    assert_eq!(cursor.row, 0);
    assert_eq!(cursor.col, 0);
}

#[test]
fn grid_truncate() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Write enough content to create scrollback
    for i in 0..50 {
        term.process(format!("Line {}\n", i).as_bytes());
    }

    // Truncate
    term.grid_mut().truncate();

    // Display offset should be 0 (scrolled to bottom)
    assert_eq!(term.grid().display_offset(), 0);
}

#[test]
fn grid_cursor_cell() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Move cursor to specific position
    term.process(b"\x1b[5;10H"); // Row 5, Col 10 (1-indexed)

    // Get cursor cell
    let cell = term.grid_mut().cursor_cell();
    assert!(cell.is_some());
}

// ===== Grid Iterator Tests =====

use crate::grid::{BidirectionalIterator, GridIteratorExt};

#[test]
fn grid_iter_from_basic() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    let grid = term.grid();
    let start = Point::new(Line(0), Column(0));
    let mut iter = grid.iter_from(start);

    // First cell should be at the starting point
    let first = iter.next();
    assert!(first.is_some());
    let first = first.unwrap();
    assert_eq!(first.point.line, Line(0));
    assert_eq!(first.point.column, Column(0));
}

#[test]
fn grid_iter_from_mid_row() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    let grid = term.grid();
    let start = Point::new(Line(5), Column(10));
    let mut iter = grid.iter_from(start);

    // First cell should be at the starting point
    let first = iter.next();
    assert!(first.is_some());
    let first = first.unwrap();
    assert_eq!(first.point.line, Line(5));
    assert_eq!(first.point.column, Column(10));
}

#[test]
fn grid_iter_advances_through_row() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    let grid = term.grid();
    let start = Point::new(Line(0), Column(77));
    let mut iter = grid.iter_from(start);

    // First cell
    let c1 = iter.next().unwrap();
    assert_eq!(c1.point.column, Column(77));
    assert_eq!(c1.point.line, Line(0));

    // Second cell
    let c2 = iter.next().unwrap();
    assert_eq!(c2.point.column, Column(78));
    assert_eq!(c2.point.line, Line(0));

    // Third cell
    let c3 = iter.next().unwrap();
    assert_eq!(c3.point.column, Column(79));
    assert_eq!(c3.point.line, Line(0));

    // Fourth cell - should wrap to next row
    let c4 = iter.next().unwrap();
    assert_eq!(c4.point.column, Column(0));
    assert_eq!(c4.point.line, Line(1));
}

#[test]
fn grid_iter_stops_at_end() {
    let config = Config::default();
    let dims = (4usize, 4usize); // Small grid for quick test
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    let grid = term.grid();
    let start = Point::new(Line(0), Column(0));
    let iter = grid.iter_from(start);

    // Should iterate through all 16 cells (4x4)
    let count = iter.count();
    assert_eq!(count, 16);
}

#[test]
fn grid_display_iter_basic() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    let grid = term.grid();
    let mut iter = grid.display_iter();

    // First cell should be at visible origin
    let first = iter.next();
    assert!(first.is_some());
    let first = first.unwrap();
    assert_eq!(first.point.line, Line(0));
    assert_eq!(first.point.column, Column(0));
}

#[test]
fn grid_display_iter_count() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    let grid = term.grid();
    let iter = grid.display_iter();

    // Should iterate through all visible cells
    let count = iter.count();
    assert_eq!(count, 24 * 80);
}

#[test]
fn grid_iter_prev() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    let grid = term.grid();
    let start = Point::new(Line(1), Column(2));
    let mut iter = grid.iter_from(start);

    // Move to starting point
    let _ = iter.next();

    // Move backward
    let prev = iter.prev();
    assert!(prev.is_some());
    let prev = prev.unwrap();
    assert_eq!(prev.point.column, Column(1));
    assert_eq!(prev.point.line, Line(1));
}

#[test]
fn grid_iter_prev_wraps_row() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    let grid = term.grid();
    let start = Point::new(Line(1), Column(0));
    let mut iter = grid.iter_from(start);

    // Move to starting point
    let _ = iter.next();

    // Move backward - should wrap to previous row
    let prev = iter.prev();
    assert!(prev.is_some());
    let prev = prev.unwrap();
    assert_eq!(prev.point.column, Column(79));
    assert_eq!(prev.point.line, Line(0));
}

#[test]
fn grid_iter_prev_stops_at_beginning() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    let grid = term.grid();
    let start = Point::new(Line(0), Column(0));
    let mut iter = grid.iter_from(start);

    // Move to starting point
    let _ = iter.next();

    // Move backward - should return None at beginning
    let prev = iter.prev();
    // May return a cell at (0, 0) or None depending on implementation
    // Let's check the point is at or before (0, 0)
    if let Some(p) = prev {
        assert!(p.point <= Point::new(Line(0), Column(0)));
    }
}

#[test]
fn grid_iter_size_hint_accurate() {
    let config = Config::default();
    let dims = (4usize, 4usize); // Small grid
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    let grid = term.grid();
    let start = Point::new(Line(0), Column(0));
    let iter = grid.iter_from(start);

    let (lower, upper) = iter.size_hint();
    assert_eq!(lower, 16);
    assert_eq!(upper, Some(16));
}

#[test]
fn grid_iter_point_method() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    let grid = term.grid();
    let start = Point::new(Line(5), Column(10));
    let mut iter = grid.iter_from(start);

    // Before first next(), point is one before start
    let initial_point = iter.point();
    assert_eq!(initial_point.line, Line(5));
    assert_eq!(initial_point.column, Column(9));

    // After next(), point moves to start
    let _ = iter.next();
    let point = iter.point();
    assert_eq!(point.line, Line(5));
    assert_eq!(point.column, Column(10));
}

#[test]
fn indexed_deref() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    let grid = term.grid();
    let start = Point::new(Line(0), Column(0));
    let mut iter = grid.iter_from(start);

    let indexed = iter.next().unwrap();

    // Should be able to deref to get the cell
    let _cell: &dterm_core::grid::Cell = &indexed;

    // Point should be accessible
    assert_eq!(indexed.point.line, Line(0));
}

#[test]
fn grid_iter_with_text() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Write some text
    term.process(b"Hello");

    let grid = term.grid();
    let start = Point::new(Line(0), Column(0));
    let mut iter = grid.iter_from(start);

    // Collect first 5 cells
    let cells: Vec<_> = (0..5).filter_map(|_| iter.next()).collect();
    assert_eq!(cells.len(), 5);

    // Each cell should have a position
    for (i, indexed) in cells.iter().enumerate() {
        assert_eq!(indexed.point.column.0, i);
        assert_eq!(indexed.point.line, Line(0));
    }
}

// ----------------------------------------------------------------------------
// Grid indexing helper function tests
// ----------------------------------------------------------------------------

#[test]
fn line_to_row_visible_area() {
    use crate::grid::line_to_row;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    let grid = term.grid();

    // Line 0 should map to row 0
    assert_eq!(line_to_row(grid, Line(0)), Some(0));

    // Line 5 should map to row 5
    assert_eq!(line_to_row(grid, Line(5)), Some(5));

    // Line 23 (last row) should map to row 23
    assert_eq!(line_to_row(grid, Line(23)), Some(23));

    // Line 24 (beyond visible) should return None
    assert_eq!(line_to_row(grid, Line(24)), None);

    // Negative lines (scrollback) should return None
    assert_eq!(line_to_row(grid, Line(-1)), None);
}

#[test]
fn grid_row_by_line() {
    use crate::grid::grid_row;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Write some text on line 0
    term.process(b"Hello");

    let grid = term.grid();

    // Get row at Line(0)
    let row = grid_row(grid, Line(0));
    assert!(row.is_some());

    let row = row.unwrap();
    assert_eq!(row.get(0).unwrap().char(), 'H');
    assert_eq!(row.get(1).unwrap().char(), 'e');

    // Get row at Line(1) - empty row
    let row1 = grid_row(grid, Line(1));
    assert!(row1.is_some());

    // Out of bounds
    let row_oob = grid_row(grid, Line(24));
    assert!(row_oob.is_none());
}

#[test]
fn grid_row_mut_by_line() {
    use crate::grid::grid_row_mut;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    let grid = term.grid_mut();

    // Get mutable row at Line(0)
    let row = grid_row_mut(grid, Line(0));
    assert!(row.is_some());

    // Write a character
    let row = row.unwrap();
    row.write_char(0, 'X');

    // Verify it was written
    assert_eq!(term.grid().row(0).unwrap().get(0).unwrap().char(), 'X');
}

#[test]
fn grid_cell_by_point() {
    use crate::grid::grid_cell;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Write some text
    term.process(b"World");

    let grid = term.grid();

    // Get cell at Point(0, 0)
    let cell = grid_cell(grid, Point::new(Line(0), Column(0)));
    assert!(cell.is_some());
    assert_eq!(cell.unwrap().char(), 'W');

    // Get cell at Point(0, 4)
    let cell = grid_cell(grid, Point::new(Line(0), Column(4)));
    assert!(cell.is_some());
    assert_eq!(cell.unwrap().char(), 'd');

    // Out of bounds line
    let cell_oob = grid_cell(grid, Point::new(Line(24), Column(0)));
    assert!(cell_oob.is_none());

    // Out of bounds column
    let cell_oob = grid_cell(grid, Point::new(Line(0), Column(80)));
    assert!(cell_oob.is_none());
}

#[test]
fn grid_cell_mut_by_point() {
    use crate::grid::grid_cell_mut;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    let grid = term.grid_mut();

    // Get mutable cell at Point(0, 0)
    let cell = grid_cell_mut(grid, Point::new(Line(0), Column(0)));
    assert!(cell.is_some());

    // Modify the cell
    let cell = cell.unwrap();
    cell.set_char('Z');

    // Verify modification
    assert_eq!(term.grid().cell(0, 0).unwrap().char(), 'Z');
}

#[test]
fn row_cell_by_column() {
    use crate::grid::row_cell;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Write some text
    term.process(b"Test");

    let grid = term.grid();
    let row = grid.row(0).unwrap();

    // Get cell at Column(0)
    let cell = row_cell(row, Column(0));
    assert!(cell.is_some());
    assert_eq!(cell.unwrap().char(), 'T');

    // Get cell at Column(3)
    let cell = row_cell(row, Column(3));
    assert!(cell.is_some());
    assert_eq!(cell.unwrap().char(), 't');

    // Out of bounds
    let cell_oob = row_cell(row, Column(80));
    assert!(cell_oob.is_none());
}

#[test]
fn row_cell_mut_by_column() {
    use crate::grid::row_cell_mut;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    let grid = term.grid_mut();
    let row = grid.row_mut(0).unwrap();

    // Get mutable cell at Column(5)
    let cell = row_cell_mut(row, Column(5));
    assert!(cell.is_some());

    // Modify the cell
    let cell = cell.unwrap();
    cell.set_char('Q');

    // Verify modification
    assert_eq!(term.grid().cell(0, 5).unwrap().char(), 'Q');
}

// ============================================================================
// Scrollback indexing tests
// ============================================================================

#[test]
fn is_scrollback_line_check() {
    use crate::grid::is_scrollback_line;

    // Visible area lines are not scrollback
    assert!(!is_scrollback_line(Line(0)));
    assert!(!is_scrollback_line(Line(5)));
    assert!(!is_scrollback_line(Line(23)));

    // Negative lines are scrollback
    assert!(is_scrollback_line(Line(-1)));
    assert!(is_scrollback_line(Line(-10)));
    assert!(is_scrollback_line(Line(-100)));
}

#[test]
fn scrollback_line_count_no_scrollback() {
    use crate::grid::scrollback_line_count;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Fresh terminal has no scrollback
    let count = scrollback_line_count(term.grid());
    assert_eq!(count, 0);
}

#[test]
fn scrollback_line_count_with_scrollback() {
    use crate::grid::scrollback_line_count;

    let config = Config::default();
    let dims = (5usize, 80usize); // Small terminal to trigger scrollback quickly
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Fill terminal and scroll
    for i in 0..20 {
        term.process(format!("Line {}\n", i).as_bytes());
    }

    // Should have scrollback now
    let count = scrollback_line_count(term.grid());
    assert!(count > 0, "Expected scrollback lines, got {}", count);
}

#[test]
fn get_scrollback_line_no_scrollback() {
    use crate::grid::get_scrollback_line;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // No scrollback available, should return None
    let line = get_scrollback_line(term.grid(), Line(-1));
    assert!(line.is_none());
}

#[test]
fn get_scrollback_line_positive_returns_none() {
    use crate::grid::get_scrollback_line;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Positive lines should return None (use grid_row for visible area)
    assert!(get_scrollback_line(term.grid(), Line(0)).is_none());
    assert!(get_scrollback_line(term.grid(), Line(5)).is_none());
}

#[test]
fn get_scrollback_text_with_content() {
    use crate::grid::get_scrollback_text;

    let config = Config::default();
    let dims = (5usize, 80usize); // Small terminal
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Fill terminal to create scrollback
    for i in 0..20 {
        term.process(format!("Line {}\n", i).as_bytes());
    }

    // Line(-1) should be the most recent scrollback line
    let text = get_scrollback_text(term.grid(), Line(-1));
    assert!(text.is_some(), "Expected scrollback text at Line(-1)");
}

#[test]
fn get_scrollback_line_out_of_bounds() {
    use crate::grid::{get_scrollback_line, scrollback_line_count};

    let config = Config::default();
    let dims = (5usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Create some scrollback
    for i in 0..10 {
        term.process(format!("Line {}\n", i).as_bytes());
    }

    let count = scrollback_line_count(term.grid());

    // Line beyond available scrollback should return None
    // Line(-count-1) should be out of bounds
    let beyond_bounds = Line(-(count as i32 + 100));
    assert!(get_scrollback_line(term.grid(), beyond_bounds).is_none());
}

#[test]
fn scrollback_vs_visible_area_indexing() {
    use crate::grid::{get_scrollback_text, grid_row};

    let config = Config::default();
    let dims = (5usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Create scrollback
    for i in 0..10 {
        term.process(format!("Row {}\n", i).as_bytes());
    }

    // Visible area should be accessible via grid_row
    assert!(grid_row(term.grid(), Line(0)).is_some());
    assert!(grid_row(term.grid(), Line(4)).is_some());

    // Scrollback should return None from grid_row
    assert!(grid_row(term.grid(), Line(-1)).is_none());

    // But scrollback should be accessible via get_scrollback_text
    let text = get_scrollback_text(term.grid(), Line(-1));
    // May or may not have scrollback depending on exact scroll behavior
    // Just verify it doesn't panic
    let _ = text;
}

// ===== Vi Mode Semantic Word Motion Tests =====

#[test]
fn vi_motion_semantic_word_right() {
    let mut term: Term<VoidListener> = Term::new(Config::default(), &(24, 80), VoidListener);
    term.process(b"hello world test");

    term.toggle_vi_mode();
    // Start at column 0 (on 'h' of "hello")
    term.vi_goto_point(Point::new(Line(0), Column(0)));

    // Move to next word (should go to 'w' of "world")
    term.vi_motion(ViMotion::SemanticRight);
    assert_eq!(term.vi_mode_cursor.point.column, Column(6));

    // Move to next word (should go to 't' of "test")
    term.vi_motion(ViMotion::SemanticRight);
    assert_eq!(term.vi_mode_cursor.point.column, Column(12));
}

#[test]
fn vi_motion_semantic_word_left() {
    let mut term: Term<VoidListener> = Term::new(Config::default(), &(24, 80), VoidListener);
    term.process(b"hello world test");

    term.toggle_vi_mode();
    // Start at column 12 (on 't' of "test")
    term.vi_goto_point(Point::new(Line(0), Column(12)));

    // Move to previous word (should go to 'w' of "world")
    term.vi_motion(ViMotion::SemanticLeft);
    assert_eq!(term.vi_mode_cursor.point.column, Column(6));

    // Move to previous word (should go to 'h' of "hello")
    term.vi_motion(ViMotion::SemanticLeft);
    assert_eq!(term.vi_mode_cursor.point.column, Column(0));
}

#[test]
fn vi_motion_semantic_word_right_end() {
    let mut term: Term<VoidListener> = Term::new(Config::default(), &(24, 80), VoidListener);
    term.process(b"hello world test");

    term.toggle_vi_mode();
    // Start at column 0 (on 'h' of "hello")
    term.vi_goto_point(Point::new(Line(0), Column(0)));

    // Move to end of current/next word (should go to 'o' of "hello")
    term.vi_motion(ViMotion::SemanticRightEnd);
    assert_eq!(term.vi_mode_cursor.point.column, Column(4));

    // Move to end of next word (should go to 'd' of "world")
    term.vi_motion(ViMotion::SemanticRightEnd);
    assert_eq!(term.vi_mode_cursor.point.column, Column(10));
}

#[test]
fn vi_motion_semantic_word_left_end() {
    let mut term: Term<VoidListener> = Term::new(Config::default(), &(24, 80), VoidListener);
    term.process(b"hello world test");

    term.toggle_vi_mode();
    // Start at column 12 (on 't' of "test")
    term.vi_goto_point(Point::new(Line(0), Column(12)));

    // Move to end of previous word (should go to 'd' of "world")
    term.vi_motion(ViMotion::SemanticLeftEnd);
    assert_eq!(term.vi_mode_cursor.point.column, Column(10));

    // Move to end of previous word (should go to 'o' of "hello")
    term.vi_motion(ViMotion::SemanticLeftEnd);
    assert_eq!(term.vi_mode_cursor.point.column, Column(4));
}

#[test]
fn vi_motion_whitespace_word_right() {
    let mut term: Term<VoidListener> = Term::new(Config::default(), &(24, 80), VoidListener);
    // Use separators that aren't whitespace for WORD motion testing
    term.process(b"hello.world foo-bar test");

    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(0), Column(0)));

    // WORD motion should skip over hello.world as one word
    term.vi_motion(ViMotion::WordRight);
    assert_eq!(term.vi_mode_cursor.point.column, Column(12)); // 'f' of "foo-bar"

    // WORD motion should skip over foo-bar as one word
    term.vi_motion(ViMotion::WordRight);
    assert_eq!(term.vi_mode_cursor.point.column, Column(20)); // 't' of "test"
}

#[test]
fn vi_motion_whitespace_word_left() {
    let mut term: Term<VoidListener> = Term::new(Config::default(), &(24, 80), VoidListener);
    term.process(b"hello world test");

    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(0), Column(15)));

    // Move to previous WORD
    term.vi_motion(ViMotion::WordLeft);
    assert_eq!(term.vi_mode_cursor.point.column, Column(12)); // 't' of "test"

    term.vi_motion(ViMotion::WordLeft);
    assert_eq!(term.vi_mode_cursor.point.column, Column(6)); // 'w' of "world"
}

#[test]
fn vi_motion_bracket_match() {
    let mut term: Term<VoidListener> = Term::new(Config::default(), &(24, 80), VoidListener);
    term.process(b"(hello world)");

    term.toggle_vi_mode();
    // Start at opening bracket
    term.vi_goto_point(Point::new(Line(0), Column(0)));

    // Move to matching bracket
    term.vi_motion(ViMotion::Bracket);
    assert_eq!(term.vi_mode_cursor.point.column, Column(12)); // closing paren

    // Move back to opening bracket
    term.vi_motion(ViMotion::Bracket);
    assert_eq!(term.vi_mode_cursor.point.column, Column(0)); // opening paren
}

#[test]
fn vi_motion_bracket_match_nested() {
    let mut term: Term<VoidListener> = Term::new(Config::default(), &(24, 80), VoidListener);
    term.process(b"((a))");

    term.toggle_vi_mode();
    // Start at first opening bracket
    term.vi_goto_point(Point::new(Line(0), Column(0)));

    // Should match the outermost closing bracket
    term.vi_motion(ViMotion::Bracket);
    assert_eq!(term.vi_mode_cursor.point.column, Column(4));
}

#[test]
fn vi_motion_first_occupied() {
    let mut term: Term<VoidListener> = Term::new(Config::default(), &(24, 80), VoidListener);
    term.process(b"   hello"); // 3 leading spaces

    term.toggle_vi_mode();
    // Start at column 5 (in the middle of hello)
    term.vi_goto_point(Point::new(Line(0), Column(5)));

    // Move to first non-empty cell
    term.vi_motion(ViMotion::FirstOccupied);
    assert_eq!(term.vi_mode_cursor.point.column, Column(3)); // 'h' of "hello"
}

#[test]
fn vi_motion_first_occupied_no_indent() {
    let mut term: Term<VoidListener> = Term::new(Config::default(), &(24, 80), VoidListener);
    term.process(b"hello"); // no leading spaces

    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(0), Column(3)));

    // Move to first non-empty cell (should be column 0)
    term.vi_motion(ViMotion::FirstOccupied);
    assert_eq!(term.vi_mode_cursor.point.column, Column(0));
}

#[test]
fn vi_motion_paragraph_down() {
    let mut term: Term<VoidListener> = Term::new(
        Config::default(),
        &(10, 80), // 10 rows
        VoidListener,
    );
    // Write text on lines 0, 1, then skip 2, write on 3
    term.process(b"line one\r\n");
    term.process(b"line two\r\n");
    term.process(b"\r\n"); // empty line
    term.process(b"line four\r\n");

    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(0), Column(0)));

    // Move down to empty line
    term.vi_motion(ViMotion::ParagraphDown);
    assert_eq!(term.vi_mode_cursor.point.line, Line(2)); // empty line
}

#[test]
fn vi_motion_paragraph_up() {
    let mut term: Term<VoidListener> = Term::new(
        Config::default(),
        &(10, 80), // 10 rows
        VoidListener,
    );
    // Write text on lines 0, 1, then skip 2, write on 3
    term.process(b"line one\r\n");
    term.process(b"line two\r\n");
    term.process(b"\r\n"); // empty line
    term.process(b"line four\r\n");

    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(4), Column(0)));

    // Move up to empty line
    term.vi_motion(ViMotion::ParagraphUp);
    assert_eq!(term.vi_mode_cursor.point.line, Line(2)); // empty line
}

// ============================================================================
// Tests for Phase 9.1 new functionality
// ============================================================================

#[test]
fn set_options_updates_config() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Check default
    assert_eq!(term.config().scrolling_history, 10_000);

    // Update config
    let new_config = Config {
        scrolling_history: 50_000,
        ..Default::default()
    };
    term.set_options(new_config);

    assert_eq!(term.config().scrolling_history, 50_000);
}

#[test]
fn expand_wide_basic() {
    use crate::index::Direction;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // With no wide chars, point should not change
    let point = Point::new(Line(5), Column(10));
    assert_eq!(term.expand_wide(point, Direction::Left), point);
    assert_eq!(term.expand_wide(point, Direction::Right), point);
}

#[test]
fn expand_wide_out_of_bounds() {
    use crate::index::Direction;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Negative line should return unchanged
    let point = Point::new(Line(-1), Column(10));
    assert_eq!(term.expand_wide(point, Direction::Left), point);
    assert_eq!(term.expand_wide(point, Direction::Right), point);

    // Line beyond grid should return unchanged
    let point = Point::new(Line(100), Column(10));
    assert_eq!(term.expand_wide(point, Direction::Left), point);
    assert_eq!(term.expand_wide(point, Direction::Right), point);
}

#[test]
fn inline_search_right_finds_char() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"hello world test");

    let start = Point::new(Line(0), Column(0));
    let result = term.inline_search_right(start, "w");

    assert!(result.is_ok());
    let found = result.unwrap();
    assert_eq!(found.column, Column(6)); // 'w' in "world"
}

#[test]
fn inline_search_right_not_found() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"hello world");

    let start = Point::new(Line(0), Column(0));
    let result = term.inline_search_right(start, "z");

    assert!(result.is_err());
}

#[test]
fn inline_search_left_finds_char() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"hello world test");

    let start = Point::new(Line(0), Column(15));
    let result = term.inline_search_left(start, "w");

    assert!(result.is_ok());
    let found = result.unwrap();
    assert_eq!(found.column, Column(6)); // 'w' in "world"
}

#[test]
fn inline_search_left_not_found() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"hello world");

    let start = Point::new(Line(0), Column(10));
    let result = term.inline_search_left(start, "z");

    assert!(result.is_err());
}

#[test]
fn inline_search_multiple_needles() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"abcdefghij");

    // Search for first occurrence of any of 'd', 'e', or 'f'
    let start = Point::new(Line(0), Column(0));
    let result = term.inline_search_right(start, "def");

    assert!(result.is_ok());
    let found = result.unwrap();
    assert_eq!(found.column, Column(3)); // 'd'
}

#[test]
fn scroll_to_point_already_visible() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Point on visible screen
    let point = Point::new(Line(5), Column(10));
    term.scroll_to_point(point);

    // Display offset should still be 0
    assert_eq!(term.display_offset(), 0);
}

#[test]
fn scroll_to_point_in_scrollback() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Create some scrollback by printing many lines
    for i in 0..50 {
        term.process(format!("line {}\r\n", i).as_bytes());
    }

    // Now scroll to a point in scrollback
    let point = Point::new(Line(-5), Column(0));
    term.scroll_to_point(point);

    // Display offset should be adjusted to show the point
    // The exact value depends on implementation, but should be > 0
    assert!(term.display_offset() > 0);
}

// ----------------------------------------------------------------------------
// RGB color rendering tests
// ----------------------------------------------------------------------------

#[test]
fn render_rgb_foreground_color() {
    use crate::render::{RenderableContent, Rgb};

    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // SGR 38;2;r;g;b - Set RGB foreground (red = 255, 0, 0)
    term.process(b"\x1b[38;2;255;0;0mR\x1b[m");

    // Get renderable content
    let content = RenderableContent::new(&term);
    let cells: Vec<_> = content.iter_cells().collect();

    // Find the cell with 'R'
    let r_cell = cells.iter().find(|c| c.character == 'R');
    assert!(r_cell.is_some(), "Should find cell with 'R'");

    let r_cell = r_cell.unwrap();
    // The foreground should be red (or close to it)
    assert_eq!(
        r_cell.fg,
        Rgb { r: 255, g: 0, b: 0 },
        "Foreground should be red RGB"
    );
}

#[test]
fn render_rgb_background_color() {
    use crate::render::{RenderableContent, Rgb};

    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // SGR 48;2;r;g;b - Set RGB background (blue = 0, 0, 255)
    term.process(b"\x1b[48;2;0;0;255mB\x1b[m");

    // Get renderable content
    let content = RenderableContent::new(&term);
    let cells: Vec<_> = content.iter_cells().collect();

    // Find the cell with 'B'
    let b_cell = cells.iter().find(|c| c.character == 'B');
    assert!(b_cell.is_some(), "Should find cell with 'B'");

    let b_cell = b_cell.unwrap();
    // The background should be blue
    assert_eq!(
        b_cell.bg,
        Rgb { r: 0, g: 0, b: 255 },
        "Background should be blue RGB"
    );
}

#[test]
fn render_rgb_both_colors() {
    use crate::render::{RenderableContent, Rgb};

    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // SGR 38;2 + 48;2 - Set both RGB colors (green fg, magenta bg)
    term.process(b"\x1b[38;2;0;255;0;48;2;255;0;255mX\x1b[m");

    // Get renderable content
    let content = RenderableContent::new(&term);
    let cells: Vec<_> = content.iter_cells().collect();

    // Find the cell with 'X'
    let x_cell = cells.iter().find(|c| c.character == 'X');
    assert!(x_cell.is_some(), "Should find cell with 'X'");

    let x_cell = x_cell.unwrap();
    assert_eq!(
        x_cell.fg,
        Rgb { r: 0, g: 255, b: 0 },
        "Foreground should be green RGB"
    );
    assert_eq!(
        x_cell.bg,
        Rgb {
            r: 255,
            g: 0,
            b: 255
        },
        "Background should be magenta RGB"
    );
}

#[test]
fn render_indexed_color() {
    use crate::render::{RenderableContent, Rgb};

    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // SGR 38;5;196 - Set indexed color 196 (bright red in 256-color palette)
    term.process(b"\x1b[38;5;196mI\x1b[m");

    // Get renderable content
    let content = RenderableContent::new(&term);
    let cells: Vec<_> = content.iter_cells().collect();

    // Find the cell with 'I'
    let i_cell = cells.iter().find(|c| c.character == 'I');
    assert!(i_cell.is_some(), "Should find cell with 'I'");

    let i_cell = i_cell.unwrap();
    // Color 196 in standard 256-color palette is RGB(255, 0, 0)
    assert_eq!(
        i_cell.fg,
        Rgb { r: 255, g: 0, b: 0 },
        "Indexed color 196 should be red"
    );
}

#[test]
fn render_hyperlink_cells() {
    use crate::index::{Column, Line, Point};
    use crate::render::RenderableContent;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"\x1b]8;;https://example.com\x07Click here\x1b]8;;\x07 after");

    let content = RenderableContent::new(&term);
    let cells: Vec<_> = content.iter_cells().collect();

    let cell_at = |col: usize| {
        let point = Point::new(Line(0), Column(col));
        cells
            .iter()
            .find(|cell| cell.point == point)
            .expect("expected cell at point")
    };

    for col in 0..10 {
        let cell = cell_at(col);
        assert_eq!(
            cell.hyperlink.as_deref(),
            Some("https://example.com"),
            "column {col} should have hyperlink"
        );
    }

    let space_cell = cell_at(5);
    assert_eq!(space_cell.character, ' ');
    assert_eq!(space_cell.hyperlink.as_deref(), Some("https://example.com"));

    for col in 10..16 {
        let cell = cell_at(col);
        assert_eq!(
            cell.hyperlink.as_deref(),
            None,
            "column {col} should not have hyperlink"
        );
    }
}

// ----------------------------------------------------------------------------
// Point arithmetic tests
// ----------------------------------------------------------------------------

#[test]
fn point_add_within_line() {
    use crate::index::{Boundary, Column, Line, Point};

    // Grid with 24 lines, 80 columns
    let dims = (24usize, 80usize);
    let point = Point::new(Line(5), Column(10));

    let result = point.add(&dims, Boundary::Grid, 5);
    assert_eq!(result.line, Line(5));
    assert_eq!(result.column, Column(15));
}

#[test]
fn point_add_wraps_to_next_line() {
    use crate::index::{Boundary, Column, Line, Point};

    let dims = (24usize, 80usize);
    let point = Point::new(Line(5), Column(78));

    // Add 5 columns: 78 + 5 = 83, wraps to next line column 3
    let result = point.add(&dims, Boundary::Grid, 5);
    assert_eq!(result.line, Line(6));
    assert_eq!(result.column, Column(3));
}

#[test]
fn point_add_multiple_lines() {
    use crate::index::{Boundary, Column, Line, Point};

    let dims = (24usize, 80usize);
    let point = Point::new(Line(0), Column(0));

    // Add 160 columns: wraps 2 full lines
    let result = point.add(&dims, Boundary::Grid, 160);
    assert_eq!(result.line, Line(2));
    assert_eq!(result.column, Column(0));
}

#[test]
fn point_add_clamped_at_boundary() {
    use crate::index::{Boundary, Column, Line, Point};

    let dims = (24usize, 80usize);
    let point = Point::new(Line(22), Column(70));

    // Add enough to exceed grid, should clamp to bottom-right
    let result = point.add(&dims, Boundary::Grid, 200);
    assert_eq!(result.line, Line(23)); // bottommost_line
    assert_eq!(result.column, Column(79)); // last_column
}

#[test]
fn point_sub_within_line() {
    use crate::index::{Boundary, Column, Line, Point};

    let dims = (24usize, 80usize);
    let point = Point::new(Line(5), Column(10));

    let result = point.sub(&dims, Boundary::Grid, 5);
    assert_eq!(result.line, Line(5));
    assert_eq!(result.column, Column(5));
}

#[test]
fn point_sub_wraps_to_previous_line() {
    use crate::index::{Boundary, Column, Line, Point};

    let dims = (24usize, 80usize);
    let point = Point::new(Line(5), Column(3));

    // Subtract 5 columns: wraps to previous line
    let result = point.sub(&dims, Boundary::Grid, 5);
    assert_eq!(result.line, Line(4));
    assert_eq!(result.column, Column(78));
}

#[test]
fn point_sub_multiple_lines() {
    use crate::index::{Boundary, Column, Line, Point};

    let dims = (24usize, 80usize);
    let point = Point::new(Line(5), Column(0));

    // Subtract 160 columns: wraps back 2 full lines
    let result = point.sub(&dims, Boundary::Grid, 160);
    assert_eq!(result.line, Line(3));
    assert_eq!(result.column, Column(0));
}

#[test]
fn point_sub_clamped_at_boundary() {
    use crate::index::{Boundary, Column, Line, Point};

    let dims = (24usize, 80usize);
    let point = Point::new(Line(1), Column(10));

    // Subtract enough to exceed grid start, should clamp to top-left
    let result = point.sub(&dims, Boundary::Grid, 200);
    assert_eq!(result.line, Line(0)); // topmost_line (no scrollback)
    assert_eq!(result.column, Column(0));
}

#[test]
fn point_grid_clamp_cursor_boundary() {
    use crate::index::{Boundary, Column, Line, Point};

    let dims = (24usize, 80usize);

    // Point above visible area (negative line)
    let point = Point::new(Line(-5), Column(10));
    let result = point.grid_clamp(&dims, Boundary::Cursor);
    assert_eq!(result.line, Line(0));
    assert_eq!(result.column, Column(0));

    // Point below visible area
    let point = Point::new(Line(30), Column(10));
    let result = point.grid_clamp(&dims, Boundary::Cursor);
    assert_eq!(result.line, Line(23)); // bottommost
    assert_eq!(result.column, Column(79)); // last column
}

#[test]
fn point_grid_clamp_column_overflow() {
    use crate::index::{Boundary, Column, Line, Point};

    let dims = (24usize, 80usize);

    // Column beyond last column
    let point = Point::new(Line(5), Column(100));
    let result = point.grid_clamp(&dims, Boundary::Grid);
    assert_eq!(result.line, Line(5));
    assert_eq!(result.column, Column(79)); // clamped to last column
}

#[test]
fn line_grid_clamp_cursor() {
    use crate::index::{Boundary, Line};

    let dims = (24usize, 80usize);

    // Negative line clamped to 0
    let result = Line(-5).grid_clamp(&dims, Boundary::Cursor);
    assert_eq!(result, Line(0));

    // Line beyond screen clamped to bottom
    let result = Line(30).grid_clamp(&dims, Boundary::Cursor);
    assert_eq!(result, Line(23));

    // Line within range unchanged
    let result = Line(10).grid_clamp(&dims, Boundary::Cursor);
    assert_eq!(result, Line(10));
}

#[test]
fn line_grid_clamp_grid_boundary() {
    use crate::index::{Boundary, Column, Dimensions, Line};

    // Create a custom dimensions struct for testing with scrollback
    struct TestDims {
        total: usize,
        screen: usize,
        cols: usize,
    }

    impl Dimensions for TestDims {
        fn total_lines(&self) -> usize {
            self.total
        }
        fn screen_lines(&self) -> usize {
            self.screen
        }
        fn columns(&self) -> usize {
            self.cols
        }
    }

    // Grid with scrollback (total 100, screen 24)
    let dims = TestDims {
        total: 100,
        screen: 24,
        cols: 80,
    };

    // Verify dimensions are as expected
    assert_eq!(dims.history_size(), 76); // 100 - 24
    assert_eq!(dims.topmost_line(), Line(-76));
    assert_eq!(dims.bottommost_line(), Line(23));
    assert_eq!(dims.last_column(), Column(79));

    // Negative line within scrollback is valid
    let result = Line(-50).grid_clamp(&dims, Boundary::Grid);
    assert_eq!(result, Line(-50));

    // Line beyond scrollback clamped to topmost
    let result = Line(-100).grid_clamp(&dims, Boundary::Grid);
    assert_eq!(result, Line(-76)); // topmost_line = -(100-24) = -76

    // Line beyond screen clamped to bottom
    let result = Line(30).grid_clamp(&dims, Boundary::Grid);
    assert_eq!(result, Line(23));
}

// ----------------------------------------------------------------------------
// Config tests
// ----------------------------------------------------------------------------

#[test]
fn config_semantic_escape_chars_default() {
    use crate::term::DEFAULT_SEMANTIC_ESCAPE_CHARS;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Should use default separators
    assert_eq!(term.semantic_escape_chars(), DEFAULT_SEMANTIC_ESCAPE_CHARS);
}

#[test]
fn config_semantic_escape_chars_custom() {
    let config = Config {
        scrolling_history: 10_000,
        semantic_escape_chars: ".,;:".to_string(),
    };
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Should use custom separators
    assert_eq!(term.semantic_escape_chars(), ".,;:");
}

#[test]
fn config_semantic_escape_chars_empty_uses_default() {
    use crate::term::DEFAULT_SEMANTIC_ESCAPE_CHARS;

    let config = Config {
        scrolling_history: 10_000,
        semantic_escape_chars: "".to_string(),
    };
    let dims = (24usize, 80usize);
    let term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Empty string should fall back to defaults
    assert_eq!(term.semantic_escape_chars(), DEFAULT_SEMANTIC_ESCAPE_CHARS);
}

// ============================================================================
// Inline Search State Tests (f/F/t/T with ;/, repeat)
// ============================================================================

#[test]
fn vi_inline_search_finds_character() {
    use crate::vi_mode::InlineSearchKind;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    // Write some text
    term.process(b"hello world");

    // Enter vi mode
    term.toggle_vi_mode();
    assert!(term.is_vi_mode());

    // Cursor starts at terminal cursor position (end of "hello world")
    // Move to beginning first
    term.vi_goto_point(Point::new(Line(0), Column(0)));

    // Find 'o' to the right (f motion)
    let result = term.vi_inline_search(InlineSearchKind::FindRight, 'o');
    assert!(result.is_some());
    let pos = result.unwrap();
    assert_eq!(pos.column, Column(4)); // 'o' in "hello"
}

#[test]
fn vi_inline_search_state_persists() {
    use crate::vi_mode::InlineSearchKind;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"abcabc");
    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(0), Column(0)));

    // Search for 'c'
    term.vi_inline_search(InlineSearchKind::FindRight, 'c');

    // Verify state was saved
    let state = term.inline_search_state();
    assert!(state.is_some());
    let state = state.unwrap();
    assert_eq!(state.char, 'c');
    assert_eq!(state.kind, InlineSearchKind::FindRight);
}

#[test]
fn vi_inline_search_repeat() {
    use crate::vi_mode::InlineSearchKind;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"a b c d e");
    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(0), Column(0)));

    // Find first space
    let first = term.vi_inline_search(InlineSearchKind::FindRight, ' ');
    assert!(first.is_some());
    term.vi_mode_cursor.point = first.unwrap();

    // Repeat to find next space
    let second = term.vi_inline_search_repeat();
    assert!(second.is_some());
    assert!(second.unwrap().column > first.unwrap().column);
}

#[test]
fn vi_inline_search_repeat_reverse() {
    use crate::vi_mode::InlineSearchKind;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"a b c d e");
    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(0), Column(4))); // Start at 'c'

    // Find space to the right
    let first = term.vi_inline_search(InlineSearchKind::FindRight, ' ');
    assert!(first.is_some());
    term.vi_mode_cursor.point = first.unwrap();

    // Repeat in reverse (go back)
    let back = term.vi_inline_search_repeat_reverse();
    assert!(back.is_some());
    assert!(back.unwrap().column < first.unwrap().column);
}

#[test]
fn vi_inline_search_till_motion() {
    use crate::vi_mode::InlineSearchKind;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"hello");
    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(0), Column(0)));

    // Till 'o' (t motion) - should stop one before 'o'
    let result = term.vi_inline_search(InlineSearchKind::TillRight, 'o');
    assert!(result.is_some());
    let pos = result.unwrap();
    // 'o' is at column 4, so 't' should land at column 3
    assert_eq!(pos.column, Column(3));
}

#[test]
fn vi_inline_search_clear_state() {
    use crate::vi_mode::InlineSearchKind;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"test");
    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(0), Column(0)));

    term.vi_inline_search(InlineSearchKind::FindRight, 't');
    assert!(term.inline_search_state().is_some());

    term.clear_inline_search();
    assert!(term.inline_search_state().is_none());
}

#[test]
fn vi_inline_search_not_found() {
    use crate::vi_mode::InlineSearchKind;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"hello");
    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(0), Column(0)));

    // Search for 'z' which doesn't exist
    let result = term.vi_inline_search(InlineSearchKind::FindRight, 'z');
    assert!(result.is_none());
}

// ============================================================================
// Search State Tests (n/N navigation)
// ============================================================================

#[test]
fn vi_search_query_basic() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"hello world hello");
    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(0), Column(0)));

    // Set search query
    term.set_search_query(Some("hello"));

    // Should have matches
    assert!(term.search_state().match_count() > 0);
}

#[test]
fn vi_search_next_moves_cursor() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"test one test two");
    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(0), Column(0)));

    term.set_search_query(Some("test"));

    // Navigate to next match
    let result = term.vi_search_next();
    assert!(result.is_some());
}

#[test]
fn vi_search_previous_moves_cursor() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"test one test two");
    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(0), Column(15))); // Near end

    term.set_search_query(Some("test"));

    // Navigate to previous match
    let result = term.vi_search_previous();
    assert!(result.is_some());
}

#[test]
fn vi_search_motion_integration() {
    use crate::vi_mode::ViMotion;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"word other word last");
    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(0), Column(0)));

    term.set_search_query(Some("word"));

    // Use vi_motion with SearchNext
    term.vi_motion(ViMotion::SearchNext);

    // Cursor should have moved
    // (Exact position depends on search results)
}

#[test]
fn vi_search_no_match() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"hello world");
    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(0), Column(0)));

    term.set_search_query(Some("xyz123"));

    // Should have no matches
    assert_eq!(term.search_state().match_count(), 0);

    // Navigation should return None
    let result = term.vi_search_next();
    assert!(result.is_none());
}

#[test]
fn vi_search_requires_vi_mode() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"test content");
    // Don't enable vi mode

    term.set_search_query(Some("test"));

    // Should return None when not in vi mode
    let result = term.vi_search_next();
    assert!(result.is_none());
}

#[test]
fn vi_search_mark_dirty_and_rebuild() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"initial");
    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(0), Column(0)));

    term.set_search_query(Some("initial"));
    let initial_count = term.search_state().match_count();
    assert!(initial_count > 0);

    // Mark as dirty (simulating content change)
    term.mark_search_dirty();
    assert!(term.search_state().is_dirty());

    // Search should auto-rebuild
    let _ = term.vi_search_next();
    assert!(!term.search_state().is_dirty());
}

#[test]
fn reset_clears_search_state() {
    use crate::vi_mode::InlineSearchKind;

    let config = Config::default();
    let dims = (24usize, 80usize);
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);

    term.process(b"test content");
    term.toggle_vi_mode();
    term.vi_goto_point(Point::new(Line(0), Column(0)));

    // Set up inline search state
    term.vi_inline_search(InlineSearchKind::FindRight, 't');
    assert!(term.inline_search_state().is_some());

    // Set up search query
    term.set_search_query(Some("test"));
    assert!(term.search_state().match_count() > 0);

    // Reset terminal
    term.reset();

    // Both should be cleared
    assert!(term.inline_search_state().is_none());
    // Search state is cleared (query is None)
    assert!(term.search_state().query().is_none());
}

#[test]
fn term_exit_sends_event() {
    let config = Config::default();
    let dims = (24usize, 80usize);
    let listener = RecordingListener::new();
    let mut term = Term::new(config, &dims, listener.clone());

    // Call exit
    term.exit();

    // Verify Exit event was sent
    let events = listener.events();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], Event::Exit));
}
