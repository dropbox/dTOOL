//! Codex-style TUI example demonstrating inky components.
//!
//! This example shows how to compose Markdown, ChatView, DiffView,
//! StatusBar, and Input into a cohesive AI assistant interface similar
//! to OpenAI's Codex CLI.
//!
//! Layout:
//! ```text
//! ┌─────────────────────────────────────┐
//! │ Codex TUI Example                   │  <- Header
//! ├─────────────────────────────────────┤
//! │ You                                 │
//! │ Fix the null check bug in main.rs   │
//! │                                     │
//! │ Assistant                           │  <- ChatView
//! │ I'll fix the null check by using... │
//! │                                     │
//! │ src/main.rs (+1/-1)                 │
//! │  1  fn main() {                     │  <- DiffView
//! │  2 -    if x == null {              │
//! │  2 +    if let Some(x) = x {        │
//! │  3      process(x);                 │
//! ├─────────────────────────────────────┤
//! │ ● Ready                             │  <- StatusBar
//! ├─────────────────────────────────────┤
//! │ > Enter your message...             │  <- Input
//! └─────────────────────────────────────┘
//! ```
//!
//! Controls:
//! - Type in the input field
//! - Enter: Submit message (demo cycles through states)
//! - Ctrl+d: Toggle diff view
//! - Ctrl+q / Escape: Quit

use inky::components::{
    ChatMessage, ChatView, DiffLine, DiffView, MessageRole, StatusBar, StatusState,
};
use inky::prelude::*;
use std::time::Duration;

/// Application state for the Codex TUI
struct CodexState {
    /// Chat messages
    messages: Signal<Vec<ChatMessage>>,
    /// Current input text
    input_text: Signal<String>,
    /// Current status state
    status: Signal<StatusState>,
    /// Status message
    status_message: Signal<String>,
    /// Whether to show the diff view
    show_diff: Signal<bool>,
    /// Timer for spinner animation
    timer: IntervalHandle,
    /// Last observed timer tick for spinner animation
    last_tick: Signal<u64>,
}

impl CodexState {
    fn new() -> Self {
        // Start with a sample conversation
        let initial_messages = vec![
            ChatMessage::new(MessageRole::User, "Fix the null check bug in main.rs"),
            ChatMessage::new(
                MessageRole::Assistant,
                "I'll fix the null check by using Rust's `Option` type properly. \
                 The issue is that the code is checking for `null` which isn't idiomatic Rust.\n\n\
                 Here's what I'm changing:\n\
                 - Replace `if x == null` with `if let Some(x) = x`\n\
                 - This properly handles the `Option<T>` pattern",
            ),
        ];

        Self {
            messages: use_signal(initial_messages),
            input_text: use_signal(String::new()),
            status: use_signal(StatusState::Idle),
            status_message: use_signal("Ready".to_string()),
            show_diff: use_signal(true),
            timer: use_interval(Duration::from_millis(100)),
            last_tick: use_signal(0),
        }
    }

    /// Handle user input submission
    fn submit_input(&self) {
        let text = self.input_text.get();
        if text.is_empty() {
            return;
        }

        // Add user message
        let user_msg = ChatMessage::new(MessageRole::User, text.clone());
        self.messages.update(|msgs| msgs.push(user_msg));

        // Clear input
        self.input_text.set(String::new());

        // Cycle through states for demo
        let current_status = self.status.get();
        match current_status {
            StatusState::Idle => {
                self.status.set(StatusState::Thinking);
                self.status_message.set("Analyzing request...".to_string());
            }
            StatusState::Thinking => {
                self.status.set(StatusState::Executing);
                self.status_message.set("Applying changes...".to_string());
            }
            StatusState::Executing => {
                // Add assistant response
                let response = ChatMessage::new(
                    MessageRole::Assistant,
                    "Done! I've applied the changes. The code now properly handles \
                     the `Option` type using Rust's pattern matching.",
                );
                self.messages.update(|msgs| msgs.push(response));
                self.status.set(StatusState::Idle);
                self.status_message.set("Ready".to_string());
            }
            StatusState::Error => {
                self.status.set(StatusState::Idle);
                self.status_message.set("Ready".to_string());
            }
        }
    }
}

/// Build the header section
fn build_header(width: u16) -> Node {
    BoxNode::new()
        .width(width)
        .height(1)
        .padding_xy(1.0, 0.0)
        .child(
            TextNode::new("Codex TUI Example")
                .bold()
                .color(Color::BrightCyan),
        )
        .into()
}

/// Build the chat view section
fn build_chat(messages: &[ChatMessage], height: u16) -> Node {
    let view = ChatView::new()
        .messages(messages.iter().cloned())
        .show_timestamps(false);

    BoxNode::new()
        .flex_grow(1.0)
        .height(height)
        .padding_xy(1.0, 0.0)
        .flex_direction(FlexDirection::Column)
        .child(view.to_node())
        .into()
}

/// Build the diff view section showing code changes
fn build_diff() -> Node {
    let diff = DiffView::new()
        .file_path("src/main.rs")
        .line(DiffLine::context(1, "fn main() {"))
        .line(DiffLine::delete(2, "    if x == null {"))
        .line(DiffLine::add(2, "    if let Some(x) = x {"))
        .line(DiffLine::context(3, "        process(x);"))
        .line(DiffLine::context(4, "    }"))
        .line(DiffLine::context(5, "}"));

    BoxNode::new()
        .padding_xy(1.0, 1.0)
        .flex_direction(FlexDirection::Column)
        .child(diff.to_node())
        .into()
}

/// Build the status bar
fn build_status_bar(state: StatusState, message: &str, tick_delta: usize, width: u16) -> Node {
    let mut status = StatusBar::new().state(state).message(message);

    // Advance spinner based on new timer ticks since last render
    for _ in 0..tick_delta {
        status.tick();
    }

    BoxNode::new()
        .width(width)
        .height(1)
        .padding_xy(1.0, 0.0)
        .child(status.to_node())
        .into()
}

/// Build the input section
fn build_input(text: &str, width: u16) -> Node {
    let input = Input::new()
        .value(text)
        .placeholder("Enter your message...")
        .width(width.saturating_sub(4));

    BoxNode::new()
        .width(width)
        .height(1)
        .padding_xy(1.0, 0.0)
        .flex_direction(FlexDirection::Row)
        .child(TextNode::new("> ").color(Color::BrightGreen))
        .child(Node::from(input))
        .into()
}

/// Build a horizontal separator line
fn build_separator(width: u16) -> Node {
    let line = "─".repeat(width as usize);
    TextNode::new(line).dim().into()
}

fn main() -> Result<()> {
    let state = CodexState::new();

    App::new()
        .state(state)
        .alt_screen(true)
        .render(|ctx| {
            let width = ctx.width();
            let height = ctx.height();

            // Get current state values
            let messages = ctx.state.messages.get();
            let input_text = ctx.state.input_text.get();
            let status = ctx.state.status.get();
            let status_message = ctx.state.status_message.get();
            let show_diff = ctx.state.show_diff.get();

            // Advance spinner based on timer ticks since last render
            let tick = ctx.state.timer.get();
            let last_tick = ctx.state.last_tick.get();
            let tick_delta = tick.saturating_sub(last_tick) as usize;
            ctx.state.last_tick.set(tick);

            // Calculate available height for chat
            // Header: 1, Separator: 1, Diff: ~8, Separator: 1, Status: 1, Input: 1
            let diff_height = if show_diff { 8 } else { 0 };
            let fixed_height = 1 + 1 + diff_height + 1 + 1 + 1; // header + sep + diff + sep + status + input
            let chat_height = height.saturating_sub(fixed_height);

            // Build the main layout
            let mut layout = BoxNode::new()
                .width(width)
                .height(height)
                .flex_direction(FlexDirection::Column);

            // Add header
            layout = layout.child(build_header(width));
            layout = layout.child(build_separator(width));

            // Add chat view
            layout = layout.child(build_chat(&messages, chat_height));

            // Add diff view if showing
            if show_diff {
                layout = layout.child(build_diff());
            }

            // Add separator before status
            layout = layout.child(build_separator(width));

            // Add status bar
            layout = layout.child(build_status_bar(status, &status_message, tick_delta, width));

            // Add input field
            layout = layout.child(build_input(&input_text, width));

            layout.into()
        })
        .on_key(|state, key| {
            match key.code {
                KeyCode::Enter => {
                    state.submit_input();
                }
                KeyCode::Char('q') if key.modifiers.ctrl => {
                    return true;
                }
                KeyCode::Esc => {
                    return true;
                }
                KeyCode::Char('d') if key.modifiers.ctrl => {
                    // Toggle diff view
                    state.show_diff.update(|show| *show = !*show);
                }
                KeyCode::Backspace => {
                    state.input_text.update(|text| {
                        text.pop();
                    });
                }
                KeyCode::Char(c) => {
                    state.input_text.update(|text| text.push(c));
                }
                _ => {}
            }
            false
        })
        .run()?;

    Ok(())
}
