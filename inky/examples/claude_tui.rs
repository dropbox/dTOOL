//! Claude Code TUI recreation example.
//!
//! This example demonstrates inky components that mirror Claude Code's interface:
//!
//! - StatusBar with token display and streaming indicator
//! - ThinkingBlock with collapsible reasoning
//! - ToolExecution showing tool calls and results
//! - TodoPanel for task tracking
//! - BackgroundTask indicators
//! - Input with placeholder and focus
//!
//! Layout:
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │ Claude Code TUI                                     │  <- Header
//! ├─────────────────────────────────────────────────────┤
//! │ ◐ Building todo list component                      │  <- Todo badge
//! │                                                     │
//! │ ▶ Thinking... (1,234 chars)                         │  <- ThinkingBlock
//! │                                                     │
//! │ User: Help me refactor the auth module              │  <- Messages
//! │                                                     │
//! │ Assistant: I'll help you refactor the auth module.  │
//! │ Let me analyze the code structure first.            │
//! │                                                     │
//! │ ✓ Read src/auth/mod.rs                              │  <- ToolExecution
//! │   → mod login; mod session; mod token;              │
//! │                                                     │
//! │ & 1 background task                                 │  <- BackgroundTask
//! ├─────────────────────────────────────────────────────┤
//! │ ◐ Thinking... │ 1.2k/0.4k tokens │ 0:45             │  <- TokenStatusBar
//! ├─────────────────────────────────────────────────────┤
//! │ > Enter message...                                  │  <- Input
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! Controls:
//! - Type to compose a message
//! - Enter: Submit (demo cycles through states)
//! - Ctrl+t: Toggle thinking block
//! - Ctrl+q / Escape: Quit

use inky::components::{
    BackgroundTask, BackgroundTaskBadge, BackgroundTaskStatus, StatusState, ThinkingBlock,
    TodoBadge, TodoItem, TodoStatus, TokenStatusBar, ToolExecution, ToolStatus,
};
use inky::prelude::*;
use std::time::Duration;

/// Application state
struct ClaudeState {
    // Messages
    messages: Signal<Vec<(bool, String)>>, // (is_user, content)
    input_text: Signal<String>,

    // Thinking state
    thinking_text: Signal<String>,
    thinking_expanded: Signal<bool>,
    thinking_streaming: Signal<bool>,

    // Tool execution state
    tool_executions: Signal<Vec<(String, ToolStatus, Option<String>)>>,

    // Todo state
    todos: Signal<Vec<TodoItem>>,

    // Background tasks
    background_tasks: Signal<Vec<BackgroundTask>>,

    // Status bar state
    status_state: Signal<StatusState>,
    tokens_in: Signal<u64>,
    tokens_out: Signal<u64>,
    elapsed_secs: Signal<u64>,

    // Animation timer (for potential future spinner animation)
    #[allow(dead_code)]
    timer: IntervalHandle,
}

impl ClaudeState {
    fn new() -> Self {
        // Initial demo messages
        let messages = vec![
            (true, "Help me refactor the auth module".to_string()),
            (
                false,
                "I'll help you refactor the auth module. Let me analyze the code structure first."
                    .to_string(),
            ),
        ];

        // Demo thinking content
        let thinking = "\
I need to understand the current auth module structure before suggesting refactoring.

Key areas to analyze:
1. Login flow and session management
2. Token validation and refresh
3. Password hashing and security
4. OAuth integration points

The current implementation has some issues:
- Tight coupling between login and session
- No async support in token refresh
- Missing rate limiting on login attempts";

        // Demo tool executions
        let tools = vec![
            (
                "Read".to_string(),
                ToolStatus::Success,
                Some("src/auth/mod.rs → mod login; mod session; mod token;".to_string()),
            ),
            (
                "Grep".to_string(),
                ToolStatus::Success,
                Some("Found 3 files with 'validate_token'".to_string()),
            ),
        ];

        // Demo todos
        let todos = vec![
            TodoItem::new("Analyze auth module structure").status(TodoStatus::Completed),
            TodoItem::new("Extract session management")
                .status(TodoStatus::InProgress)
                .active_form("Extracting session management..."),
            TodoItem::new("Add async token refresh").status(TodoStatus::Pending),
            TodoItem::new("Implement rate limiting").status(TodoStatus::Pending),
        ];

        // Demo background task
        let bg_tasks = vec![BackgroundTask::new("cargo test auth")
            .status(BackgroundTaskStatus::Running)
            .output_preview("running 12 tests...")];

        Self {
            messages: use_signal(messages),
            input_text: use_signal(String::new()),
            thinking_text: use_signal(thinking.to_string()),
            thinking_expanded: use_signal(false),
            thinking_streaming: use_signal(false),
            tool_executions: use_signal(tools),
            todos: use_signal(todos),
            background_tasks: use_signal(bg_tasks),
            status_state: use_signal(StatusState::Idle),
            tokens_in: use_signal(1234),
            tokens_out: use_signal(356),
            elapsed_secs: use_signal(45),
            timer: use_interval(Duration::from_millis(100)),
        }
    }

    fn submit_input(&self) {
        let text = self.input_text.get();
        if text.is_empty() {
            return;
        }

        // Add user message
        self.messages.update(|msgs| {
            msgs.push((true, text.clone()));
        });
        self.input_text.set(String::new());

        // Cycle status state to demo
        let current = self.status_state.get();
        let next = match current {
            StatusState::Idle => StatusState::Thinking,
            StatusState::Thinking => StatusState::Executing,
            StatusState::Executing => StatusState::Idle,
            StatusState::Error => StatusState::Idle,
        };
        self.status_state.set(next);

        // Toggle streaming for demo
        self.thinking_streaming.update(|s| *s = !*s);

        // Update tokens for demo
        self.tokens_in.update(|t| *t += 50);
        self.tokens_out.update(|t| *t += 25);
    }
}

fn build_header(width: u16) -> Node {
    BoxNode::new()
        .width(width)
        .height(1)
        .padding_xy(1.0, 0.0)
        .flex_direction(FlexDirection::Row)
        .justify_content(JustifyContent::SpaceBetween)
        .child(
            TextNode::new("Claude Code TUI")
                .bold()
                .color(Color::BrightMagenta),
        )
        .child(TextNode::new("inky demo").dim())
        .into()
}

fn build_todo_badge(todos: &[TodoItem]) -> Node {
    // Count statuses
    let in_progress = todos
        .iter()
        .filter(|t| t.get_status() == TodoStatus::InProgress)
        .count();
    let completed = todos
        .iter()
        .filter(|t| t.get_status() == TodoStatus::Completed)
        .count();

    TodoBadge::new()
        .todo_count(todos.len())
        .in_progress(in_progress)
        .completed(completed)
        .detailed(true)
        .to_node()
}

fn build_thinking_block(text: &str, expanded: bool, streaming: bool) -> Node {
    ThinkingBlock::new()
        .content(text)
        .expanded(expanded)
        .streaming(streaming)
        .to_node()
}

fn build_messages(messages: &[(bool, String)]) -> Node {
    let mut container = BoxNode::new()
        .flex_direction(FlexDirection::Column)
        .gap(1.0);

    for (is_user, content) in messages {
        let role = if *is_user { "User" } else { "Assistant" };
        let color = if *is_user {
            Color::BrightBlue
        } else {
            Color::BrightGreen
        };

        let msg_box = BoxNode::new()
            .flex_direction(FlexDirection::Column)
            .child(TextNode::new(role).bold().color(color))
            .child(TextNode::new(content));

        container = container.child(msg_box);
    }

    container.into()
}

fn build_tool_execution(name: &str, status: ToolStatus, output: Option<&str>) -> Node {
    let mut tool = ToolExecution::new(name).status(status);

    if let Some(out) = output {
        tool = tool.output_preview(out);
    }

    tool.to_node()
}

fn build_tool_executions(tools: &[(String, ToolStatus, Option<String>)]) -> Node {
    let mut container = BoxNode::new()
        .flex_direction(FlexDirection::Column)
        .gap(1.0);

    for (name, status, output) in tools {
        container = container.child(build_tool_execution(name, *status, output.as_deref()));
    }

    container.into()
}

fn build_background_tasks(tasks: &[BackgroundTask]) -> Node {
    if tasks.is_empty() {
        return BoxNode::new().into();
    }

    // Count running tasks
    let running = tasks.iter().filter(|t| t.is_running()).count();

    BoxNode::new()
        .flex_direction(FlexDirection::Row)
        .gap(2.0)
        .child(
            BackgroundTaskBadge::new()
                .count(running)
                .show_spinner(true)
                .to_node(),
        )
        .into()
}

fn build_status_bar(
    state: StatusState,
    tokens_in: u64,
    tokens_out: u64,
    elapsed_secs: u64,
    _width: u16,
) -> Node {
    TokenStatusBar::new()
        .state(state)
        .tokens(tokens_in, tokens_out)
        .elapsed(Duration::from_secs(elapsed_secs))
        .elapsed_threshold(Duration::from_secs(0)) // Always show elapsed
        .to_node()
}

fn build_input(text: &str, focused: bool, width: u16) -> Node {
    let input = Input::new()
        .value(text)
        .placeholder("Enter your message... (Enter to send, Ctrl+t toggle thinking)")
        .width(width.saturating_sub(4))
        .focused(focused);

    BoxNode::new()
        .width(width)
        .height(1)
        .padding_xy(1.0, 0.0)
        .flex_direction(FlexDirection::Row)
        .child(TextNode::new("> ").color(Color::BrightGreen))
        .child(Node::from(input))
        .into()
}

fn build_separator(width: u16) -> Node {
    TextNode::new("─".repeat(width as usize)).dim().into()
}

fn main() -> Result<()> {
    let state = ClaudeState::new();

    App::new()
        .state(state)
        .alt_screen(true)
        .render(|ctx| {
            let width = ctx.width();
            let height = ctx.height();

            // Get state values
            let messages = ctx.state.messages.get();
            let input_text = ctx.state.input_text.get();
            let thinking_text = ctx.state.thinking_text.get();
            let thinking_expanded = ctx.state.thinking_expanded.get();
            let thinking_streaming = ctx.state.thinking_streaming.get();
            let tool_executions = ctx.state.tool_executions.get();
            let todos = ctx.state.todos.get();
            let background_tasks = ctx.state.background_tasks.get();
            let status_state = ctx.state.status_state.get();
            let tokens_in = ctx.state.tokens_in.get();
            let tokens_out = ctx.state.tokens_out.get();
            let elapsed_secs = ctx.state.elapsed_secs.get();

            // Build main layout
            let mut layout = BoxNode::new()
                .width(width)
                .height(height)
                .flex_direction(FlexDirection::Column);

            // Header
            layout = layout.child(build_header(width));
            layout = layout.child(build_separator(width));

            // Main content area
            let mut content = BoxNode::new()
                .flex_direction(FlexDirection::Column)
                .flex_grow(1.0)
                .padding_xy(1.0, 0.0)
                .gap(1.0);

            // Todo badge
            content = content.child(build_todo_badge(&todos));

            // Thinking block
            if !thinking_text.is_empty() || thinking_streaming {
                content = content.child(build_thinking_block(
                    &thinking_text,
                    thinking_expanded,
                    thinking_streaming,
                ));
            }

            // Messages
            content = content.child(build_messages(&messages));

            // Tool executions
            if !tool_executions.is_empty() {
                content = content.child(build_tool_executions(&tool_executions));
            }

            // Background tasks
            if !background_tasks.is_empty() {
                content = content.child(build_background_tasks(&background_tasks));
            }

            layout = layout.child(content);

            // Status bar
            layout = layout.child(build_separator(width));
            layout = layout.child(build_status_bar(
                status_state,
                tokens_in,
                tokens_out,
                elapsed_secs,
                width,
            ));

            // Input
            layout = layout.child(build_input(&input_text, true, width));

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
                KeyCode::Char('t') if key.modifiers.ctrl => {
                    // Toggle thinking block expanded
                    state.thinking_expanded.update(|e| *e = !*e);
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
