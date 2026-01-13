//! tmux control mode protocol parser.
//!
//! This module parses tmux control mode output into structured events.
//! The protocol is line-based with `%` prefixed notifications.

use std::fmt;

/// tmux session ID (prefixed with `$` in protocol).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TmuxSessionId(pub u32);

impl fmt::Display for TmuxSessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.0)
    }
}

/// tmux window ID (prefixed with `@` in protocol).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TmuxWindowId(pub u32);

impl fmt::Display for TmuxWindowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}", self.0)
    }
}

/// tmux pane ID (prefixed with `%` in protocol).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TmuxPaneId(pub u32);

impl fmt::Display for TmuxPaneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "%{}", self.0)
    }
}

/// tmux timestamp (Unix epoch seconds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TmuxTimestamp(pub u64);

/// tmux command number for response tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TmuxCommandNum(pub u32);

/// Parser state for tmux control mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TmuxParseState {
    /// Waiting for next line.
    Idle,
    /// Inside a `%begin`/`%end` or `%begin`/`%error` block.
    InBlock {
        /// Timestamp from `%begin`.
        timestamp: TmuxTimestamp,
        /// Command number from `%begin`.
        cmd_num: TmuxCommandNum,
        /// Flags from `%begin`.
        flags: u32,
    },
    /// Protocol is broken (invalid input received).
    Broken,
}

/// tmux notification types.
#[derive(Debug, Clone, PartialEq)]
pub enum TmuxNotification {
    /// `%output %<pane_id> <data>` - Pane output (octal-encoded).
    Output {
        /// The pane ID that produced output.
        pane_id: TmuxPaneId,
        /// The output data (octal-decoded).
        data: String,
    },

    /// `%extended-output %<pane_id> <latency> : <data>` - Pane output with latency (tmux 3.2+).
    ExtendedOutput {
        /// The pane ID that produced output.
        pane_id: TmuxPaneId,
        /// The latency in milliseconds.
        latency_ms: u32,
        /// The output data (octal-decoded).
        data: String,
    },

    /// `%layout-change @<window_id> <layout> <visible_layout> <flags>` - Window layout changed.
    LayoutChange {
        /// The window whose layout changed.
        window_id: TmuxWindowId,
        /// Parsed layout information.
        layout: TmuxLayoutInfo,
    },

    /// `%window-add @<window_id>` - Window added.
    WindowAdd {
        /// The new window's ID.
        window_id: TmuxWindowId,
    },

    /// `%window-close @<window_id>` - Window closed.
    WindowClose {
        /// The closed window's ID.
        window_id: TmuxWindowId,
    },

    /// `%window-renamed @<window_id> <name>` - Window renamed.
    WindowRenamed {
        /// The renamed window's ID.
        window_id: TmuxWindowId,
        /// The new window name.
        name: String,
    },

    /// `%session-changed $<session_id> <name>` - Client attached to session.
    SessionChanged {
        /// The session ID.
        session_id: TmuxSessionId,
        /// The session name.
        name: String,
    },

    /// `%sessions-changed` - Session list changed.
    SessionsChanged,

    /// `%window-pane-changed @<window_id> %<pane_id>` - Active pane changed.
    WindowPaneChanged {
        /// The window containing the pane.
        window_id: TmuxWindowId,
        /// The newly active pane.
        pane_id: TmuxPaneId,
    },

    /// `%unlinked-window-add @<window_id>` - Unlinked window added.
    UnlinkedWindowAdd {
        /// The unlinked window's ID.
        window_id: TmuxWindowId,
    },

    /// `%unlinked-window-close @<window_id>` - Unlinked window closed.
    UnlinkedWindowClose {
        /// The closed unlinked window's ID.
        window_id: TmuxWindowId,
    },

    /// `%pause %<pane_id>` - Pane output paused (tmux 3.2+).
    Pause {
        /// The paused pane's ID.
        pane_id: TmuxPaneId,
    },

    /// `%continue %<pane_id>` - Pane output continued (tmux 3.2+).
    Continue {
        /// The continued pane's ID.
        pane_id: TmuxPaneId,
    },

    /// `%exit [reason]` - Session ending.
    Exit {
        /// Optional reason for exit.
        reason: Option<String>,
    },

    /// `%subscription-changed <name> : <value>` - Subscription value changed.
    SubscriptionChanged {
        /// The subscription name.
        name: String,
        /// The new value.
        value: String,
    },

    /// `%client-detached <client_name>` - Client detached.
    ClientDetached {
        /// The detached client's name.
        client_name: String,
    },

    /// `%client-session-changed <client_name> $<session_id> <session_name>` - Client session changed.
    ClientSessionChanged {
        /// The client name.
        client_name: String,
        /// The new session ID.
        session_id: TmuxSessionId,
        /// The new session name.
        session_name: String,
    },

    /// Unknown notification (for forward compatibility).
    Unknown {
        /// The notification name.
        name: String,
        /// The raw arguments string.
        args: String,
    },
}

/// Event from the tmux control mode parser.
#[derive(Debug, Clone, PartialEq)]
pub enum TmuxBlockEvent {
    /// A notification was received.
    Notification(TmuxNotification),

    /// `%begin` - Command response block starting.
    BlockStart {
        /// Server timestamp when the block started.
        timestamp: TmuxTimestamp,
        /// Command number for correlation.
        cmd_num: TmuxCommandNum,
        /// Flags associated with this block.
        flags: u32,
    },

    /// `%end` - Command response block ended successfully.
    BlockEnd {
        /// Server timestamp when the block ended.
        timestamp: TmuxTimestamp,
        /// Command number for correlation.
        cmd_num: TmuxCommandNum,
        /// Flags associated with this block.
        flags: u32,
    },

    /// `%error` - Command response block ended with error.
    BlockError {
        /// Server timestamp when the error occurred.
        timestamp: TmuxTimestamp,
        /// Command number for correlation.
        cmd_num: TmuxCommandNum,
        /// Flags associated with this block.
        flags: u32,
    },

    /// Data line within a command response block.
    BlockData(String),
}

/// Layout information from `%layout-change`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TmuxLayoutInfo {
    /// Raw layout string.
    pub layout_string: String,
    /// Visible layout string.
    pub visible_layout_string: String,
    /// Layout flags.
    pub flags: String,
    /// Parsed layout tree (if parsing succeeded).
    pub root: Option<TmuxLayoutNode>,
}

/// Node in a tmux layout tree.
#[derive(Debug, Clone, PartialEq)]
pub enum TmuxLayoutNode {
    /// A single pane.
    Pane {
        /// Width in cells.
        width: u16,
        /// Height in cells.
        height: u16,
        /// X position.
        x: u16,
        /// Y position.
        y: u16,
        /// The pane ID.
        pane_id: TmuxPaneId,
    },
    /// Horizontal split (side-by-side).
    HorizontalSplit {
        /// Width in cells.
        width: u16,
        /// Height in cells.
        height: u16,
        /// X position.
        x: u16,
        /// Y position.
        y: u16,
        /// Child layout nodes.
        children: Vec<TmuxLayoutNode>,
    },
    /// Vertical split (stacked).
    VerticalSplit {
        /// Width in cells.
        width: u16,
        /// Height in cells.
        height: u16,
        /// X position.
        x: u16,
        /// Y position.
        y: u16,
        /// Child layout nodes.
        children: Vec<TmuxLayoutNode>,
    },
}

/// Parser for tmux control mode protocol.
#[derive(Debug)]
pub struct TmuxControlParser {
    state: TmuxParseState,
    /// Buffer for incomplete lines.
    line_buffer: String,
}

impl Default for TmuxControlParser {
    fn default() -> Self {
        Self::new()
    }
}

impl TmuxControlParser {
    /// Create a new tmux control mode parser.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: TmuxParseState::Idle,
            line_buffer: String::new(),
        }
    }

    /// Reset the parser state.
    pub fn reset(&mut self) {
        self.state = TmuxParseState::Idle;
        self.line_buffer.clear();
    }

    /// Get the current parser state.
    #[must_use]
    pub fn state(&self) -> TmuxParseState {
        self.state
    }

    /// Parse input data and return events.
    pub fn parse(&mut self, data: &[u8]) -> Vec<TmuxBlockEvent> {
        let mut events = Vec::new();

        // Convert to string, handling invalid UTF-8
        let input = String::from_utf8_lossy(data);

        for ch in input.chars() {
            if ch == '\n' {
                // Process complete line
                if let Some(event) = self.process_line(&self.line_buffer.clone()) {
                    events.push(event);
                }
                self.line_buffer.clear();
            } else if ch != '\r' {
                // Ignore CR, accumulate other characters
                self.line_buffer.push(ch);
            }
        }

        events
    }

    /// Process a complete line.
    fn process_line(&mut self, line: &str) -> Option<TmuxBlockEvent> {
        let line = line.trim_end();

        if line.starts_with('%') {
            // This is a notification or control line
            self.parse_notification_line(line)
        } else {
            // Data line (only valid inside a block)
            match self.state {
                TmuxParseState::InBlock { .. } => Some(TmuxBlockEvent::BlockData(line.to_string())),
                TmuxParseState::Idle => {
                    // Data outside a block - ignore or log
                    None
                }
                TmuxParseState::Broken => None,
            }
        }
    }

    /// Parse a notification line (starts with `%`).
    fn parse_notification_line(&mut self, line: &str) -> Option<TmuxBlockEvent> {
        // Split into notification name and arguments
        let line = &line[1..]; // Skip the `%`

        let (name, args) = match line.find(' ') {
            Some(idx) => (&line[..idx], &line[idx + 1..]),
            None => (line, ""),
        };

        match name {
            "begin" => self.parse_begin(args),
            "end" => self.parse_end(args),
            "error" => self.parse_error(args),
            "output" => self.parse_output(args),
            "extended-output" => self.parse_extended_output(args),
            "layout-change" => self.parse_layout_change(args),
            "window-add" => self.parse_window_add(args),
            "window-close" => self.parse_window_close(args),
            "window-renamed" => self.parse_window_renamed(args),
            "session-changed" => self.parse_session_changed(args),
            "sessions-changed" => Some(TmuxBlockEvent::Notification(
                TmuxNotification::SessionsChanged,
            )),
            "window-pane-changed" => self.parse_window_pane_changed(args),
            "unlinked-window-add" => self.parse_unlinked_window_add(args),
            "unlinked-window-close" => self.parse_unlinked_window_close(args),
            "pause" => self.parse_pause(args),
            "continue" => self.parse_continue(args),
            "exit" => self.parse_exit(args),
            "subscription-changed" => self.parse_subscription_changed(args),
            "client-detached" => self.parse_client_detached(args),
            "client-session-changed" => self.parse_client_session_changed(args),
            _ => Some(TmuxBlockEvent::Notification(TmuxNotification::Unknown {
                name: name.to_string(),
                args: args.to_string(),
            })),
        }
    }

    /// Parse `%begin <timestamp> <cmdnum> <flags>`.
    fn parse_begin(&mut self, args: &str) -> Option<TmuxBlockEvent> {
        let parts: Vec<&str> = args.split_whitespace().collect();
        if parts.len() >= 3 {
            let timestamp = TmuxTimestamp(parts[0].parse().unwrap_or(0));
            let cmd_num = TmuxCommandNum(parts[1].parse().unwrap_or(0));
            let flags = parts[2].parse().unwrap_or(0);

            self.state = TmuxParseState::InBlock {
                timestamp,
                cmd_num,
                flags,
            };

            Some(TmuxBlockEvent::BlockStart {
                timestamp,
                cmd_num,
                flags,
            })
        } else {
            None
        }
    }

    /// Parse `%end <timestamp> <cmdnum> <flags>`.
    fn parse_end(&mut self, args: &str) -> Option<TmuxBlockEvent> {
        let parts: Vec<&str> = args.split_whitespace().collect();
        if parts.len() >= 3 {
            let timestamp = TmuxTimestamp(parts[0].parse().unwrap_or(0));
            let cmd_num = TmuxCommandNum(parts[1].parse().unwrap_or(0));
            let flags = parts[2].parse().unwrap_or(0);

            self.state = TmuxParseState::Idle;

            Some(TmuxBlockEvent::BlockEnd {
                timestamp,
                cmd_num,
                flags,
            })
        } else {
            None
        }
    }

    /// Parse `%error <timestamp> <cmdnum> <flags>`.
    fn parse_error(&mut self, args: &str) -> Option<TmuxBlockEvent> {
        let parts: Vec<&str> = args.split_whitespace().collect();
        if parts.len() >= 3 {
            let timestamp = TmuxTimestamp(parts[0].parse().unwrap_or(0));
            let cmd_num = TmuxCommandNum(parts[1].parse().unwrap_or(0));
            let flags = parts[2].parse().unwrap_or(0);

            self.state = TmuxParseState::Idle;

            Some(TmuxBlockEvent::BlockError {
                timestamp,
                cmd_num,
                flags,
            })
        } else {
            None
        }
    }

    /// Parse `%output %<pane_id> <data>`.
    fn parse_output(&mut self, args: &str) -> Option<TmuxBlockEvent> {
        // Format: %<pane_id> <data>
        if let Some(rest) = args.strip_prefix('%') {
            if let Some(idx) = rest.find(' ') {
                let pane_id = TmuxPaneId(rest[..idx].parse().unwrap_or(0));
                let data = rest[idx + 1..].to_string();
                return Some(TmuxBlockEvent::Notification(TmuxNotification::Output {
                    pane_id,
                    data,
                }));
            }
        }
        None
    }

    /// Parse `%extended-output %<pane_id> <latency> : <data>`.
    fn parse_extended_output(&mut self, args: &str) -> Option<TmuxBlockEvent> {
        if let Some(rest) = args.strip_prefix('%') {
            let parts: Vec<&str> = rest.splitn(3, ' ').collect();
            if parts.len() >= 3 {
                let pane_id = TmuxPaneId(parts[0].parse().unwrap_or(0));
                let latency_ms = parts[1].parse().unwrap_or(0);
                // Skip the " : " separator
                let data = if parts[2].starts_with(": ") {
                    &parts[2][2..]
                } else {
                    parts[2]
                };
                return Some(TmuxBlockEvent::Notification(
                    TmuxNotification::ExtendedOutput {
                        pane_id,
                        latency_ms,
                        data: data.to_string(),
                    },
                ));
            }
        }
        None
    }

    /// Parse `%layout-change @<window_id> <layout> [<visible_layout>] [<flags>]`.
    fn parse_layout_change(&mut self, args: &str) -> Option<TmuxBlockEvent> {
        if let Some(rest) = args.strip_prefix('@') {
            let parts: Vec<&str> = rest.splitn(4, ' ').collect();
            if !parts.is_empty() {
                let window_id = TmuxWindowId(parts[0].parse().unwrap_or(0));
                let layout_string = (*parts.get(1).unwrap_or(&"")).to_owned();
                let visible_layout_string = (*parts.get(2).unwrap_or(&"")).to_owned();
                let flags = (*parts.get(3).unwrap_or(&"")).to_owned();

                // Try to parse the layout string
                let root = parse_layout_string(&layout_string);

                return Some(TmuxBlockEvent::Notification(
                    TmuxNotification::LayoutChange {
                        window_id,
                        layout: TmuxLayoutInfo {
                            layout_string,
                            visible_layout_string,
                            flags,
                            root,
                        },
                    },
                ));
            }
        }
        None
    }

    /// Parse `%window-add @<window_id>`.
    fn parse_window_add(&mut self, args: &str) -> Option<TmuxBlockEvent> {
        if let Some(rest) = args.strip_prefix('@') {
            let window_id = TmuxWindowId(rest.trim().parse().unwrap_or(0));
            return Some(TmuxBlockEvent::Notification(TmuxNotification::WindowAdd {
                window_id,
            }));
        }
        None
    }

    /// Parse `%window-close @<window_id>`.
    fn parse_window_close(&mut self, args: &str) -> Option<TmuxBlockEvent> {
        if let Some(rest) = args.strip_prefix('@') {
            let window_id = TmuxWindowId(rest.trim().parse().unwrap_or(0));
            return Some(TmuxBlockEvent::Notification(
                TmuxNotification::WindowClose { window_id },
            ));
        }
        None
    }

    /// Parse `%window-renamed @<window_id> <name>`.
    fn parse_window_renamed(&mut self, args: &str) -> Option<TmuxBlockEvent> {
        if let Some(rest) = args.strip_prefix('@') {
            if let Some(idx) = rest.find(' ') {
                let window_id = TmuxWindowId(rest[..idx].parse().unwrap_or(0));
                let name = rest[idx + 1..].to_string();
                return Some(TmuxBlockEvent::Notification(
                    TmuxNotification::WindowRenamed { window_id, name },
                ));
            }
        }
        None
    }

    /// Parse `%session-changed $<session_id> <name>`.
    fn parse_session_changed(&mut self, args: &str) -> Option<TmuxBlockEvent> {
        if let Some(rest) = args.strip_prefix('$') {
            if let Some(idx) = rest.find(' ') {
                let session_id = TmuxSessionId(rest[..idx].parse().unwrap_or(0));
                let name = rest[idx + 1..].to_string();
                return Some(TmuxBlockEvent::Notification(
                    TmuxNotification::SessionChanged { session_id, name },
                ));
            }
        }
        None
    }

    /// Parse `%window-pane-changed @<window_id> %<pane_id>`.
    fn parse_window_pane_changed(&mut self, args: &str) -> Option<TmuxBlockEvent> {
        if let Some(rest) = args.strip_prefix('@') {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 2 {
                let window_id = TmuxWindowId(parts[0].parse().unwrap_or(0));
                if let Some(pane_str) = parts[1].strip_prefix('%') {
                    let pane_id = TmuxPaneId(pane_str.parse().unwrap_or(0));
                    return Some(TmuxBlockEvent::Notification(
                        TmuxNotification::WindowPaneChanged { window_id, pane_id },
                    ));
                }
            }
        }
        None
    }

    /// Parse `%unlinked-window-add @<window_id>`.
    fn parse_unlinked_window_add(&mut self, args: &str) -> Option<TmuxBlockEvent> {
        if let Some(rest) = args.strip_prefix('@') {
            let window_id = TmuxWindowId(rest.trim().parse().unwrap_or(0));
            return Some(TmuxBlockEvent::Notification(
                TmuxNotification::UnlinkedWindowAdd { window_id },
            ));
        }
        None
    }

    /// Parse `%unlinked-window-close @<window_id>`.
    fn parse_unlinked_window_close(&mut self, args: &str) -> Option<TmuxBlockEvent> {
        if let Some(rest) = args.strip_prefix('@') {
            let window_id = TmuxWindowId(rest.trim().parse().unwrap_or(0));
            return Some(TmuxBlockEvent::Notification(
                TmuxNotification::UnlinkedWindowClose { window_id },
            ));
        }
        None
    }

    /// Parse `%pause %<pane_id>`.
    fn parse_pause(&mut self, args: &str) -> Option<TmuxBlockEvent> {
        if let Some(rest) = args.strip_prefix('%') {
            let pane_id = TmuxPaneId(rest.trim().parse().unwrap_or(0));
            return Some(TmuxBlockEvent::Notification(TmuxNotification::Pause {
                pane_id,
            }));
        }
        None
    }

    /// Parse `%continue %<pane_id>`.
    fn parse_continue(&mut self, args: &str) -> Option<TmuxBlockEvent> {
        if let Some(rest) = args.strip_prefix('%') {
            let pane_id = TmuxPaneId(rest.trim().parse().unwrap_or(0));
            return Some(TmuxBlockEvent::Notification(TmuxNotification::Continue {
                pane_id,
            }));
        }
        None
    }

    /// Parse `%exit [reason]`.
    #[allow(clippy::unnecessary_wraps)]
    fn parse_exit(&mut self, args: &str) -> Option<TmuxBlockEvent> {
        let reason = if args.is_empty() {
            None
        } else {
            Some(args.to_string())
        };
        Some(TmuxBlockEvent::Notification(TmuxNotification::Exit {
            reason,
        }))
    }

    /// Parse `%subscription-changed <name> : <value>`.
    fn parse_subscription_changed(&mut self, args: &str) -> Option<TmuxBlockEvent> {
        if let Some(idx) = args.find(" : ") {
            let name = args[..idx].to_string();
            let value = args[idx + 3..].to_string();
            return Some(TmuxBlockEvent::Notification(
                TmuxNotification::SubscriptionChanged { name, value },
            ));
        }
        None
    }

    /// Parse `%client-detached <client_name>`.
    #[allow(clippy::unnecessary_wraps)]
    fn parse_client_detached(&mut self, args: &str) -> Option<TmuxBlockEvent> {
        Some(TmuxBlockEvent::Notification(
            TmuxNotification::ClientDetached {
                client_name: args.to_string(),
            },
        ))
    }

    /// Parse `%client-session-changed <client_name> $<session_id> <session_name>`.
    fn parse_client_session_changed(&mut self, args: &str) -> Option<TmuxBlockEvent> {
        let parts: Vec<&str> = args.splitn(3, ' ').collect();
        if parts.len() >= 3 {
            let client_name = parts[0].to_string();
            if let Some(sid_str) = parts[1].strip_prefix('$') {
                let session_id = TmuxSessionId(sid_str.parse().unwrap_or(0));
                let session_name = parts[2].to_string();
                return Some(TmuxBlockEvent::Notification(
                    TmuxNotification::ClientSessionChanged {
                        client_name,
                        session_id,
                        session_name,
                    },
                ));
            }
        }
        None
    }
}

/// Parse a tmux layout string into a tree structure.
///
/// Layout string format: `WIDTHxHEIGHT,X,Y{...}` or `WIDTHxHEIGHT,X,Y[...]` or `WIDTHxHEIGHT,X,Y,PANE_ID`
///
/// Examples:
/// - Single pane: `80x24,0,0,0`
/// - Horizontal split: `160x50,0,0{80x50,0,0,0,80x50,81,0,1}`
/// - Vertical split: `80x50,0,0[80x25,0,0,0,80x24,0,26,1]`
///
/// The parsing strategy:
/// 1. Read WIDTHxHEIGHT,X,Y (4 numbers)
/// 2. If next char is `{` or `[`, this is a split - parse children recursively
/// 3. If next char is `,DIGIT`, this is a pane with ID - read the 5th number
/// 4. Otherwise this is a pane with ID 0
fn parse_layout_string(layout: &str) -> Option<TmuxLayoutNode> {
    let mut chars = layout.chars().peekable();
    parse_layout_node(&mut chars)
}

fn parse_layout_node(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<TmuxLayoutNode> {
    // Parse the first 4 numbers: WIDTHxHEIGHT,X,Y
    let width = parse_number(chars)?;
    expect_char(chars, 'x')?;
    let height = parse_number(chars)?;
    expect_char(chars, ',')?;
    let x = parse_number(chars)?;
    expect_char(chars, ',')?;
    let y = parse_number(chars)?;

    // Now check what comes next
    match chars.peek() {
        Some('{') => {
            // Horizontal split
            chars.next();
            let children = parse_layout_children(chars, '}');
            Some(TmuxLayoutNode::HorizontalSplit {
                width,
                height,
                x,
                y,
                children,
            })
        }
        Some('[') => {
            // Vertical split
            chars.next();
            let children = parse_layout_children(chars, ']');
            Some(TmuxLayoutNode::VerticalSplit {
                width,
                height,
                x,
                y,
                children,
            })
        }
        Some(',') => {
            // Single pane with ID - consume comma and read pane ID
            chars.next();
            let pane_id = parse_number(chars).unwrap_or(0);
            Some(TmuxLayoutNode::Pane {
                width,
                height,
                x,
                y,
                pane_id: TmuxPaneId(u32::from(pane_id)),
            })
        }
        _ => {
            // Single pane with default ID
            Some(TmuxLayoutNode::Pane {
                width,
                height,
                x,
                y,
                pane_id: TmuxPaneId(0),
            })
        }
    }
}

/// Parse a number from the iterator.
fn parse_number(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<u16> {
    let mut num_str = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            num_str.push(chars.next().unwrap());
        } else {
            break;
        }
    }
    if num_str.is_empty() {
        None
    } else {
        num_str.parse().ok()
    }
}

/// Expect and consume a specific character.
fn expect_char(chars: &mut std::iter::Peekable<std::str::Chars>, expected: char) -> Option<()> {
    if chars.peek() == Some(&expected) {
        chars.next();
        Some(())
    } else {
        None
    }
}

fn parse_layout_children(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    end_char: char,
) -> Vec<TmuxLayoutNode> {
    let mut children = Vec::new();

    loop {
        // Check for end
        if chars.peek() == Some(&end_char) {
            chars.next();
            break;
        }

        // No more chars
        if chars.peek().is_none() {
            break;
        }

        // Try to parse a child node
        if let Some(child) = parse_layout_node(chars) {
            children.push(child);
        }

        // After parsing a child, check for comma separator or end
        match chars.peek() {
            Some(',') => {
                chars.next();
            }
            Some(c) if *c == end_char => {
                // Will be handled at loop start
            }
            _ => break,
        }
    }

    children
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_session_changed() {
        let mut parser = TmuxControlParser::new();
        let events = parser.parse(b"%session-changed $0 default\n");
        assert_eq!(events.len(), 1);
        match &events[0] {
            TmuxBlockEvent::Notification(TmuxNotification::SessionChanged { session_id, name }) => {
                assert_eq!(session_id.0, 0);
                assert_eq!(name, "default");
            }
            _ => panic!("Expected SessionChanged notification"),
        }
    }

    #[test]
    fn test_parse_output() {
        let mut parser = TmuxControlParser::new();
        let events = parser.parse(b"%output %0 Hello\\012World\n");
        assert_eq!(events.len(), 1);
        match &events[0] {
            TmuxBlockEvent::Notification(TmuxNotification::Output { pane_id, data }) => {
                assert_eq!(pane_id.0, 0);
                assert_eq!(data, "Hello\\012World");
            }
            _ => panic!("Expected Output notification"),
        }
    }

    #[test]
    fn test_parse_window_add() {
        let mut parser = TmuxControlParser::new();
        let events = parser.parse(b"%window-add @1\n");
        assert_eq!(events.len(), 1);
        match &events[0] {
            TmuxBlockEvent::Notification(TmuxNotification::WindowAdd { window_id }) => {
                assert_eq!(window_id.0, 1);
            }
            _ => panic!("Expected WindowAdd notification"),
        }
    }

    #[test]
    fn test_parse_window_close() {
        let mut parser = TmuxControlParser::new();
        let events = parser.parse(b"%window-close @2\n");
        assert_eq!(events.len(), 1);
        match &events[0] {
            TmuxBlockEvent::Notification(TmuxNotification::WindowClose { window_id }) => {
                assert_eq!(window_id.0, 2);
            }
            _ => panic!("Expected WindowClose notification"),
        }
    }

    #[test]
    fn test_parse_window_renamed() {
        let mut parser = TmuxControlParser::new();
        let events = parser.parse(b"%window-renamed @3 my-window\n");
        assert_eq!(events.len(), 1);
        match &events[0] {
            TmuxBlockEvent::Notification(TmuxNotification::WindowRenamed { window_id, name }) => {
                assert_eq!(window_id.0, 3);
                assert_eq!(name, "my-window");
            }
            _ => panic!("Expected WindowRenamed notification"),
        }
    }

    #[test]
    fn test_parse_window_pane_changed() {
        let mut parser = TmuxControlParser::new();
        let events = parser.parse(b"%window-pane-changed @1 %5\n");
        assert_eq!(events.len(), 1);
        match &events[0] {
            TmuxBlockEvent::Notification(TmuxNotification::WindowPaneChanged {
                window_id,
                pane_id,
            }) => {
                assert_eq!(window_id.0, 1);
                assert_eq!(pane_id.0, 5);
            }
            _ => panic!("Expected WindowPaneChanged notification"),
        }
    }

    #[test]
    fn test_parse_sessions_changed() {
        let mut parser = TmuxControlParser::new();
        let events = parser.parse(b"%sessions-changed\n");
        assert_eq!(events.len(), 1);
        match &events[0] {
            TmuxBlockEvent::Notification(TmuxNotification::SessionsChanged) => {}
            _ => panic!("Expected SessionsChanged notification"),
        }
    }

    #[test]
    fn test_parse_exit() {
        let mut parser = TmuxControlParser::new();
        let events = parser.parse(b"%exit\n");
        assert_eq!(events.len(), 1);
        match &events[0] {
            TmuxBlockEvent::Notification(TmuxNotification::Exit { reason }) => {
                assert!(reason.is_none());
            }
            _ => panic!("Expected Exit notification"),
        }

        let events = parser.parse(b"%exit server exited\n");
        assert_eq!(events.len(), 1);
        match &events[0] {
            TmuxBlockEvent::Notification(TmuxNotification::Exit { reason }) => {
                assert_eq!(reason.as_deref(), Some("server exited"));
            }
            _ => panic!("Expected Exit notification"),
        }
    }

    #[test]
    fn test_parse_pause_continue() {
        let mut parser = TmuxControlParser::new();

        let events = parser.parse(b"%pause %0\n");
        assert_eq!(events.len(), 1);
        match &events[0] {
            TmuxBlockEvent::Notification(TmuxNotification::Pause { pane_id }) => {
                assert_eq!(pane_id.0, 0);
            }
            _ => panic!("Expected Pause notification"),
        }

        let events = parser.parse(b"%continue %0\n");
        assert_eq!(events.len(), 1);
        match &events[0] {
            TmuxBlockEvent::Notification(TmuxNotification::Continue { pane_id }) => {
                assert_eq!(pane_id.0, 0);
            }
            _ => panic!("Expected Continue notification"),
        }
    }

    #[test]
    fn test_parse_begin_end_block() {
        let mut parser = TmuxControlParser::new();

        let events = parser.parse(b"%begin 1234567890 0 0\n");
        assert_eq!(events.len(), 1);
        match &events[0] {
            TmuxBlockEvent::BlockStart {
                timestamp,
                cmd_num,
                flags,
            } => {
                assert_eq!(timestamp.0, 1234567890);
                assert_eq!(cmd_num.0, 0);
                assert_eq!(*flags, 0);
            }
            _ => panic!("Expected BlockStart"),
        }

        assert_eq!(
            parser.state(),
            TmuxParseState::InBlock {
                timestamp: TmuxTimestamp(1234567890),
                cmd_num: TmuxCommandNum(0),
                flags: 0,
            }
        );

        let events = parser.parse(b"some data line\n");
        assert_eq!(events.len(), 1);
        match &events[0] {
            TmuxBlockEvent::BlockData(line) => {
                assert_eq!(line, "some data line");
            }
            _ => panic!("Expected BlockData"),
        }

        let events = parser.parse(b"%end 1234567890 0 0\n");
        assert_eq!(events.len(), 1);
        match &events[0] {
            TmuxBlockEvent::BlockEnd {
                timestamp,
                cmd_num,
                flags,
            } => {
                assert_eq!(timestamp.0, 1234567890);
                assert_eq!(cmd_num.0, 0);
                assert_eq!(*flags, 0);
            }
            _ => panic!("Expected BlockEnd"),
        }

        assert_eq!(parser.state(), TmuxParseState::Idle);
    }

    #[test]
    fn test_parse_error_block() {
        let mut parser = TmuxControlParser::new();

        parser.parse(b"%begin 1234567890 1 0\n");
        let events = parser.parse(b"%error 1234567890 1 0\n");

        assert_eq!(events.len(), 1);
        match &events[0] {
            TmuxBlockEvent::BlockError {
                timestamp,
                cmd_num,
                flags,
            } => {
                assert_eq!(timestamp.0, 1234567890);
                assert_eq!(cmd_num.0, 1);
                assert_eq!(*flags, 0);
            }
            _ => panic!("Expected BlockError"),
        }

        assert_eq!(parser.state(), TmuxParseState::Idle);
    }

    #[test]
    fn test_parse_extended_output() {
        let mut parser = TmuxControlParser::new();
        let events = parser.parse(b"%extended-output %0 100 : test data\n");
        assert_eq!(events.len(), 1);
        match &events[0] {
            TmuxBlockEvent::Notification(TmuxNotification::ExtendedOutput {
                pane_id,
                latency_ms,
                data,
            }) => {
                assert_eq!(pane_id.0, 0);
                assert_eq!(*latency_ms, 100);
                assert_eq!(data, "test data");
            }
            _ => panic!("Expected ExtendedOutput notification"),
        }
    }

    #[test]
    fn test_parse_subscription_changed() {
        let mut parser = TmuxControlParser::new();
        let events = parser.parse(b"%subscription-changed my_var : new_value\n");
        assert_eq!(events.len(), 1);
        match &events[0] {
            TmuxBlockEvent::Notification(TmuxNotification::SubscriptionChanged { name, value }) => {
                assert_eq!(name, "my_var");
                assert_eq!(value, "new_value");
            }
            _ => panic!("Expected SubscriptionChanged notification"),
        }
    }

    #[test]
    fn test_parse_client_detached() {
        let mut parser = TmuxControlParser::new();
        let events = parser.parse(b"%client-detached /dev/pts/1\n");
        assert_eq!(events.len(), 1);
        match &events[0] {
            TmuxBlockEvent::Notification(TmuxNotification::ClientDetached { client_name }) => {
                assert_eq!(client_name, "/dev/pts/1");
            }
            _ => panic!("Expected ClientDetached notification"),
        }
    }

    #[test]
    fn test_parse_client_session_changed() {
        let mut parser = TmuxControlParser::new();
        let events = parser.parse(b"%client-session-changed /dev/pts/0 $1 mysession\n");
        assert_eq!(events.len(), 1);
        match &events[0] {
            TmuxBlockEvent::Notification(TmuxNotification::ClientSessionChanged {
                client_name,
                session_id,
                session_name,
            }) => {
                assert_eq!(client_name, "/dev/pts/0");
                assert_eq!(session_id.0, 1);
                assert_eq!(session_name, "mysession");
            }
            _ => panic!("Expected ClientSessionChanged notification"),
        }
    }

    #[test]
    fn test_parse_unknown_notification() {
        let mut parser = TmuxControlParser::new();
        let events = parser.parse(b"%future-notification some args\n");
        assert_eq!(events.len(), 1);
        match &events[0] {
            TmuxBlockEvent::Notification(TmuxNotification::Unknown { name, args }) => {
                assert_eq!(name, "future-notification");
                assert_eq!(args, "some args");
            }
            _ => panic!("Expected Unknown notification"),
        }
    }

    #[test]
    fn test_parse_layout_single_pane() {
        let layout = parse_layout_string("80x24,0,0,0");
        match layout {
            Some(TmuxLayoutNode::Pane {
                width,
                height,
                x,
                y,
                pane_id,
            }) => {
                assert_eq!(width, 80);
                assert_eq!(height, 24);
                assert_eq!(x, 0);
                assert_eq!(y, 0);
                assert_eq!(pane_id.0, 0);
            }
            _ => panic!("Expected Pane node"),
        }
    }

    #[test]
    fn test_parse_layout_horizontal_split() {
        let layout = parse_layout_string("160x50,0,0{80x50,0,0,0,80x50,81,0,1}");
        match layout {
            Some(TmuxLayoutNode::HorizontalSplit {
                width,
                height,
                children,
                ..
            }) => {
                assert_eq!(width, 160);
                assert_eq!(height, 50);
                assert_eq!(children.len(), 2);
            }
            _ => panic!("Expected HorizontalSplit node"),
        }
    }

    #[test]
    fn test_parse_layout_vertical_split() {
        let layout = parse_layout_string("80x50,0,0[80x25,0,0,0,80x24,0,26,1]");
        match layout {
            Some(TmuxLayoutNode::VerticalSplit {
                width,
                height,
                children,
                ..
            }) => {
                assert_eq!(width, 80);
                assert_eq!(height, 50);
                assert_eq!(children.len(), 2);
            }
            _ => panic!("Expected VerticalSplit node"),
        }
    }

    #[test]
    fn test_parser_state_tracking() {
        let mut parser = TmuxControlParser::new();
        assert_eq!(parser.state(), TmuxParseState::Idle);

        parser.parse(b"%begin 1234 0 0\n");
        assert!(matches!(parser.state(), TmuxParseState::InBlock { .. }));

        parser.parse(b"%end 1234 0 0\n");
        assert_eq!(parser.state(), TmuxParseState::Idle);
    }

    #[test]
    fn test_parser_reset() {
        let mut parser = TmuxControlParser::new();
        parser.parse(b"%begin 1234 0 0\n");
        parser.reset();
        assert_eq!(parser.state(), TmuxParseState::Idle);
    }

    #[test]
    fn test_multiple_events_single_parse() {
        let mut parser = TmuxControlParser::new();
        let events = parser.parse(b"%session-changed $0 test\n%window-add @0\n");
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_partial_line_buffering() {
        let mut parser = TmuxControlParser::new();

        // Send partial line
        let events = parser.parse(b"%session-changed $0 ");
        assert!(events.is_empty());

        // Complete the line
        let events = parser.parse(b"test\n");
        assert_eq!(events.len(), 1);
    }
}
