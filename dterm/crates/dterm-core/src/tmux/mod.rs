//! # tmux Control Mode (-CC) Protocol Implementation
//!
//! This module implements tmux's control mode protocol for native tmux integration.
//! When tmux is started with `-CC`, it outputs structured notifications that allow
//! the terminal emulator to create native windows/tabs for tmux panes.
//!
//! ## Protocol Overview
//!
//! All output from tmux in control mode is line-based:
//! - Notifications start with `%` (e.g., `%session-changed $0 default`)
//! - Command responses are wrapped in `%begin`/`%end` or `%begin`/`%error` blocks
//! - Output data is octal-escaped for binary safety
//!
//! ## ID Sigils
//!
//! - `$` - Session ID (e.g., `$0`)
//! - `@` - Window ID (e.g., `@1`)
//! - `%` - Pane ID (e.g., `%0`)
//!
//! ## Example Session
//!
//! ```text
//! %begin 1234567890 0 0
//! 0: 1 windows (created ...) (attached)
//! %end 1234567890 0 0
//! %session-changed $0 default
//! %output %0 Hello, world!\n
//! ```
//!
//! ## References
//!
//! - tmux manual: `man tmux` (search for "control mode")
//! - iTerm2: `TmuxGateway.m`, `TmuxController.m`
//! - WezTerm: `wezterm-escape-parser/src/tmux_cc/`
//! - Ghostty: `src/terminal/tmux/`

mod parser;

pub use parser::{
    TmuxBlockEvent, TmuxCommandNum, TmuxControlParser, TmuxLayoutInfo, TmuxLayoutNode,
    TmuxNotification, TmuxPaneId, TmuxParseState, TmuxSessionId, TmuxTimestamp, TmuxWindowId,
};

/// Callback for tmux control mode events.
///
/// Implement this trait to receive parsed tmux notifications.
pub trait TmuxEventSink {
    /// Called when a tmux notification is received.
    fn on_notification(&mut self, notification: &TmuxNotification);

    /// Called when a command response block starts.
    fn on_block_start(&mut self, timestamp: TmuxTimestamp, cmd_num: TmuxCommandNum, flags: u32);

    /// Called when a command response block ends successfully.
    fn on_block_end(&mut self, timestamp: TmuxTimestamp, cmd_num: TmuxCommandNum, flags: u32);

    /// Called when a command response block ends with an error.
    fn on_block_error(&mut self, timestamp: TmuxTimestamp, cmd_num: TmuxCommandNum, flags: u32);

    /// Called with data lines within a command response block.
    fn on_block_data(&mut self, line: &str);

    /// Called when the tmux session exits.
    fn on_exit(&mut self, reason: Option<&str>);
}

/// State machine for tmux control mode integration.
///
/// This manages the connection lifecycle and coordinates between the parser
/// and the terminal emulator.
#[derive(Debug)]
pub struct TmuxControlMode {
    /// Parser for tmux protocol
    parser: TmuxControlParser,
    /// Whether control mode is active
    active: bool,
    /// Current session ID (if attached)
    session_id: Option<TmuxSessionId>,
    /// Current session name
    session_name: Option<String>,
    /// Command counter for tracking responses
    command_counter: TmuxCommandNum,
    /// Pending output for the current pane
    pending_output: Vec<(TmuxPaneId, Vec<u8>)>,
}

impl Default for TmuxControlMode {
    fn default() -> Self {
        Self::new()
    }
}

impl TmuxControlMode {
    /// Create a new tmux control mode handler.
    #[must_use]
    pub fn new() -> Self {
        Self {
            parser: TmuxControlParser::new(),
            active: false,
            session_id: None,
            session_name: None,
            command_counter: TmuxCommandNum(0),
            pending_output: Vec::new(),
        }
    }

    /// Check if control mode is active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Activate control mode (called when tmux -CC is detected).
    pub fn activate(&mut self) {
        self.active = true;
        self.parser.reset();
    }

    /// Deactivate control mode.
    pub fn deactivate(&mut self) {
        self.active = false;
        self.session_id = None;
        self.session_name = None;
        self.pending_output.clear();
    }

    /// Get the current session ID.
    #[must_use]
    pub fn session_id(&self) -> Option<TmuxSessionId> {
        self.session_id
    }

    /// Get the current session name.
    #[must_use]
    pub fn session_name(&self) -> Option<&str> {
        self.session_name.as_deref()
    }

    /// Process input data from tmux.
    ///
    /// Returns events that should be handled by the terminal.
    pub fn process<S: TmuxEventSink>(&mut self, data: &[u8], sink: &mut S) {
        if !self.active {
            return;
        }

        for event in self.parser.parse(data) {
            match event {
                TmuxBlockEvent::Notification(notification) => {
                    self.handle_notification(&notification, sink);
                    sink.on_notification(&notification);
                }
                TmuxBlockEvent::BlockStart {
                    timestamp,
                    cmd_num,
                    flags,
                } => {
                    sink.on_block_start(timestamp, cmd_num, flags);
                }
                TmuxBlockEvent::BlockEnd {
                    timestamp,
                    cmd_num,
                    flags,
                } => {
                    sink.on_block_end(timestamp, cmd_num, flags);
                }
                TmuxBlockEvent::BlockError {
                    timestamp,
                    cmd_num,
                    flags,
                } => {
                    sink.on_block_error(timestamp, cmd_num, flags);
                }
                TmuxBlockEvent::BlockData(line) => {
                    sink.on_block_data(&line);
                }
            }
        }
    }

    /// Handle a tmux notification internally.
    fn handle_notification<S: TmuxEventSink>(
        &mut self,
        notification: &TmuxNotification,
        sink: &mut S,
    ) {
        match notification {
            TmuxNotification::SessionChanged { session_id, name } => {
                self.session_id = Some(*session_id);
                self.session_name = Some(name.clone());
            }
            TmuxNotification::Exit { reason } => {
                sink.on_exit(reason.as_deref());
                self.deactivate();
            }
            _ => {}
        }
    }

    /// Get the next command number for tracking.
    #[must_use]
    pub fn next_command_num(&mut self) -> TmuxCommandNum {
        let num = self.command_counter;
        self.command_counter = TmuxCommandNum(self.command_counter.0.wrapping_add(1));
        num
    }

    /// Format a command to send to tmux.
    ///
    /// The command will be properly formatted for control mode.
    #[must_use]
    pub fn format_command(&self, command: &str) -> String {
        // In control mode, commands are sent as-is with a newline
        format!("{}\n", command)
    }

    /// Build a `send-keys` command for the given pane.
    #[must_use]
    pub fn send_keys_command(pane_id: TmuxPaneId, keys: &str) -> String {
        // Escape special characters
        let escaped = keys.replace('\\', "\\\\").replace('"', "\\\"");
        format!("send-keys -t %{} \"{}\"", pane_id.0, escaped)
    }

    /// Build a `list-panes` command.
    #[must_use]
    pub fn list_panes_command() -> &'static str {
        "list-panes -a -F \"#{pane_id} #{window_id} #{session_id} #{pane_width} #{pane_height} #{pane_left} #{pane_top} #{pane_active} #{pane_tty}\""
    }

    /// Build a `list-windows` command.
    #[must_use]
    pub fn list_windows_command() -> &'static str {
        "list-windows -a -F \"#{window_id} #{session_id} #{window_name} #{window_width} #{window_height} #{window_layout} #{window_active}\""
    }

    /// Build a `capture-pane` command.
    #[must_use]
    pub fn capture_pane_command(pane_id: TmuxPaneId, start: i32, end: i32) -> String {
        format!("capture-pane -t %{} -p -S {} -E {}", pane_id.0, start, end)
    }

    /// Build a `resize-pane` command.
    #[must_use]
    pub fn resize_pane_command(pane_id: TmuxPaneId, width: u16, height: u16) -> String {
        format!("resize-pane -t %{} -x {} -y {}", pane_id.0, width, height)
    }

    /// Build a `new-window` command.
    #[must_use]
    pub fn new_window_command(name: Option<&str>) -> String {
        match name {
            Some(n) => format!("new-window -n \"{}\"", n.replace('"', "\\\"")),
            None => "new-window".to_string(),
        }
    }

    /// Build a `split-window` command.
    #[must_use]
    pub fn split_window_command(horizontal: bool) -> &'static str {
        if horizontal {
            "split-window -h"
        } else {
            "split-window -v"
        }
    }

    /// Build a `select-pane` command.
    #[must_use]
    pub fn select_pane_command(pane_id: TmuxPaneId) -> String {
        format!("select-pane -t %{}", pane_id.0)
    }

    /// Build a `kill-pane` command.
    #[must_use]
    pub fn kill_pane_command(pane_id: TmuxPaneId) -> String {
        format!("kill-pane -t %{}", pane_id.0)
    }

    /// Enable pause mode (tmux 3.2+).
    ///
    /// This allows flow control by pausing pane output.
    #[must_use]
    pub fn enable_pause_mode_command(timeout_ms: u32) -> String {
        format!("refresh-client -f pause-after={}", timeout_ms)
    }

    /// Continue paused pane output (tmux 3.2+).
    #[must_use]
    pub fn continue_pane_command(pane_id: TmuxPaneId) -> String {
        format!("refresh-client -A %{}", pane_id.0)
    }
}

/// Decode octal-escaped tmux output.
///
/// tmux encodes binary data using backslash + 3 octal digits.
/// For example, `\012` represents a newline.
#[must_use]
pub fn decode_octal_output(input: &str) -> Vec<u8> {
    let mut result = Vec::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            // Check for octal escape (3 digits)
            let mut octal = String::new();
            for _ in 0..3 {
                if let Some(&digit) = chars.peek() {
                    if digit.is_ascii_digit() && digit < '8' {
                        octal.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
            }

            if octal.len() == 3 {
                // Parse as octal
                if let Ok(byte) = u8::from_str_radix(&octal, 8) {
                    result.push(byte);
                } else {
                    // Invalid octal, output as-is
                    result.push(b'\\');
                    result.extend(octal.bytes());
                }
            } else if octal.is_empty() {
                // Check for common escapes
                match chars.peek() {
                    Some('n') => {
                        chars.next();
                        result.push(b'\n');
                    }
                    Some('r') => {
                        chars.next();
                        result.push(b'\r');
                    }
                    Some('t') => {
                        chars.next();
                        result.push(b'\t');
                    }
                    Some('\\') => {
                        chars.next();
                        result.push(b'\\');
                    }
                    _ => {
                        result.push(b'\\');
                    }
                }
            } else {
                // Partial octal, output as-is
                result.push(b'\\');
                result.extend(octal.bytes());
            }
        } else {
            // Regular character - convert to UTF-8 bytes
            let mut buf = [0u8; 4];
            let encoded = c.encode_utf8(&mut buf);
            result.extend_from_slice(encoded.as_bytes());
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_octal_simple() {
        // Newline
        assert_eq!(decode_octal_output("\\012"), b"\n");
        // Carriage return
        assert_eq!(decode_octal_output("\\015"), b"\r");
        // Tab
        assert_eq!(decode_octal_output("\\011"), b"\t");
        // Space
        assert_eq!(decode_octal_output("\\040"), b" ");
        // Escape
        assert_eq!(decode_octal_output("\\033"), b"\x1b");
    }

    #[test]
    fn test_decode_octal_mixed() {
        assert_eq!(decode_octal_output("Hello\\012World"), b"Hello\nWorld");
        assert_eq!(decode_octal_output("Tab\\011here"), b"Tab\there");
    }

    #[test]
    fn test_decode_octal_backslash_escapes() {
        assert_eq!(decode_octal_output("\\n"), b"\n");
        assert_eq!(decode_octal_output("\\r"), b"\r");
        assert_eq!(decode_octal_output("\\t"), b"\t");
        assert_eq!(decode_octal_output("\\\\"), b"\\");
    }

    #[test]
    fn test_decode_octal_plain_text() {
        assert_eq!(decode_octal_output("Hello, world!"), b"Hello, world!");
        assert_eq!(decode_octal_output(""), b"");
    }

    #[test]
    fn test_decode_octal_high_bytes() {
        // 0xFF = 377 octal
        assert_eq!(decode_octal_output("\\377"), vec![0xFF]);
        // 0x80 = 200 octal
        assert_eq!(decode_octal_output("\\200"), vec![0x80]);
    }

    #[test]
    fn test_tmux_control_mode_new() {
        let mode = TmuxControlMode::new();
        assert!(!mode.is_active());
        assert!(mode.session_id().is_none());
        assert!(mode.session_name().is_none());
    }

    #[test]
    fn test_tmux_control_mode_activate() {
        let mut mode = TmuxControlMode::new();
        mode.activate();
        assert!(mode.is_active());

        mode.deactivate();
        assert!(!mode.is_active());
    }

    #[test]
    fn test_tmux_command_formatting() {
        let mode = TmuxControlMode::new();
        assert_eq!(mode.format_command("list-panes"), "list-panes\n");
    }

    #[test]
    fn test_send_keys_command() {
        let cmd = TmuxControlMode::send_keys_command(TmuxPaneId(0), "ls -la");
        assert_eq!(cmd, "send-keys -t %0 \"ls -la\"");

        // Test escaping
        let cmd = TmuxControlMode::send_keys_command(TmuxPaneId(1), "echo \"hello\"");
        assert_eq!(cmd, "send-keys -t %1 \"echo \\\"hello\\\"\"");
    }

    #[test]
    fn test_resize_pane_command() {
        let cmd = TmuxControlMode::resize_pane_command(TmuxPaneId(2), 80, 24);
        assert_eq!(cmd, "resize-pane -t %2 -x 80 -y 24");
    }

    #[test]
    fn test_new_window_command() {
        assert_eq!(TmuxControlMode::new_window_command(None), "new-window");
        assert_eq!(
            TmuxControlMode::new_window_command(Some("test")),
            "new-window -n \"test\""
        );
    }

    #[test]
    fn test_split_window_command() {
        assert_eq!(
            TmuxControlMode::split_window_command(true),
            "split-window -h"
        );
        assert_eq!(
            TmuxControlMode::split_window_command(false),
            "split-window -v"
        );
    }

    #[test]
    fn test_capture_pane_command() {
        let cmd = TmuxControlMode::capture_pane_command(TmuxPaneId(0), -100, 100);
        assert_eq!(cmd, "capture-pane -t %0 -p -S -100 -E 100");
    }

    #[test]
    fn test_pause_mode_commands() {
        assert_eq!(
            TmuxControlMode::enable_pause_mode_command(1000),
            "refresh-client -f pause-after=1000"
        );
        assert_eq!(
            TmuxControlMode::continue_pane_command(TmuxPaneId(3)),
            "refresh-client -A %3"
        );
    }
}
