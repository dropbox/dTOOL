//! UI rendering for the TUI
//!
//! Provides the visual layout and rendering logic using Ratatui.

/// Spinner frames for animated processing indicator
const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::{
    AgentStatus, App, AppMode, ApprovalRequestType, ChatMessage, MessageRole, NotificationStyle,
};
use crate::markdown::render_markdown;
use crate::wrap::wrap_text;

/// Search highlight state for rendering messages
struct SearchHighlight<'a> {
    /// The search query (case-insensitive matching)
    query: &'a str,
    /// Whether this message is the current match
    is_current_match: bool,
}

/// Highlight search matches in text, preserving the base style
fn highlight_search_in_text<'a>(
    text: &str,
    highlight: &SearchHighlight<'_>,
    base_style: Style,
) -> Vec<Span<'a>> {
    if highlight.query.is_empty() {
        return vec![Span::styled(text.to_string(), base_style)];
    }

    let query_lower = highlight.query.to_lowercase();
    let text_lower = text.to_lowercase();
    let mut spans = Vec::new();
    let mut last_end = 0;

    // Find all matches
    for (start, _) in text_lower.match_indices(&query_lower) {
        // Add text before the match
        if start > last_end {
            spans.push(Span::styled(text[last_end..start].to_string(), base_style));
        }

        // Add the highlighted match (preserve original case)
        let match_end = start + highlight.query.len();
        let highlight_style = if highlight.is_current_match {
            // Current match: bold yellow background
            Style::default().fg(Color::Black).bg(Color::Yellow).bold()
        } else {
            // Other matches: yellow text
            base_style.fg(Color::Yellow).bold()
        };
        spans.push(Span::styled(
            text[start..match_end].to_string(),
            highlight_style,
        ));
        last_end = match_end;
    }

    // Add remaining text after last match
    if last_end < text.len() {
        spans.push(Span::styled(text[last_end..].to_string(), base_style));
    }

    // Return at least an empty span if text was empty
    if spans.is_empty() {
        vec![Span::styled(text.to_string(), base_style)]
    } else {
        spans
    }
}

/// Build spans for text with specific character positions highlighted.
///
/// Used for fuzzy match highlighting in command popup.
fn build_highlighted_spans<'a>(
    text: &str,
    highlight_positions: &[usize],
    base_style: Style,
    highlight_style: Style,
) -> Vec<Span<'a>> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() || highlight_positions.is_empty() {
        return vec![Span::styled(text.to_string(), base_style)];
    }

    let mut spans = Vec::new();
    let mut current_run = String::new();
    let mut in_highlight = false;

    for (idx, &ch) in chars.iter().enumerate() {
        let should_highlight = highlight_positions.contains(&idx);

        if should_highlight != in_highlight {
            // Style change - flush current run
            if !current_run.is_empty() {
                let style = if in_highlight {
                    highlight_style
                } else {
                    base_style
                };
                spans.push(Span::styled(std::mem::take(&mut current_run), style));
            }
            in_highlight = should_highlight;
        }
        current_run.push(ch);
    }

    // Flush remaining run
    if !current_run.is_empty() {
        let style = if in_highlight {
            highlight_style
        } else {
            base_style
        };
        spans.push(Span::styled(current_run, style));
    }

    if spans.is_empty() {
        vec![Span::styled(text.to_string(), base_style)]
    } else {
        spans
    }
}

/// Calculate input area height based on content
fn calculate_input_height(app: &App) -> u16 {
    // Base height: 1 line + 2 for borders
    // Add extra lines for multi-line input (max 10 lines to avoid taking too much space)
    let line_count = app.input_line_count();
    let content_height = line_count.min(10) as u16;
    content_height + 2 // +2 for borders
}

/// Render the entire UI
pub fn render(frame: &mut Frame, app: &App) {
    // Calculate dynamic input height
    let input_height = calculate_input_height(app);

    // Main layout: header, chat area, input area, status bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),            // Header
            Constraint::Min(5),               // Chat area (min 5 to always show some)
            Constraint::Length(input_height), // Input area (dynamic)
            Constraint::Length(1),            // Status bar
        ])
        .split(frame.area());

    render_header(frame, app, chunks[0]);
    render_chat(frame, app, chunks[1]);
    render_input(frame, app, chunks[2]);
    render_status(frame, app, chunks[3]);

    // Render command popup on top of input area if visible
    if app.command_popup.visible {
        render_command_popup(frame, app, chunks[2]);
    }

    // Render help overlay on top if visible
    if app.show_help {
        render_help_overlay(frame);
    }

    // Render approval overlay on top if visible
    if app.approval_overlay.visible {
        render_approval_overlay(frame, app);
    }

    // Render notification toast if present
    if let Some(ref notification) = app.notification {
        render_notification(frame, notification);
    }
}

/// Get a display-friendly working directory path.
///
/// - Resolves "." to the actual current directory
/// - Replaces home directory with ~
/// - Truncates to fit within max_width, preserving the final path component
fn format_working_dir(working_dir: &str, max_width: usize) -> String {
    use std::path::Path;

    // Resolve "." to actual path
    let path_str = if working_dir == "." {
        std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string())
    } else {
        working_dir.to_string()
    };

    // Replace home directory with ~ using HOME env var
    let display_path = if let Ok(home) = std::env::var("HOME") {
        if path_str.starts_with(&home) {
            path_str.replacen(&home, "~", 1)
        } else {
            path_str
        }
    } else {
        path_str
    };

    // Truncate if too long, preserving final component
    if display_path.len() <= max_width {
        display_path
    } else {
        // Try to keep the last path component visible
        let path = Path::new(&display_path);
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            let available = max_width.saturating_sub(file_name.len() + 4); // 4 for ".../""
            if available > 0 && file_name.len() + 4 < max_width {
                format!(".../{}", file_name)
            } else {
                // Just truncate from the start
                format!(
                    "...{}",
                    &display_path[display_path
                        .len()
                        .saturating_sub(max_width.saturating_sub(3))..]
                )
            }
        } else {
            format!(
                "...{}",
                &display_path[display_path
                    .len()
                    .saturating_sub(max_width.saturating_sub(3))..]
            )
        }
    }
}

/// Estimate context window size for a model based on its name.
///
/// Returns the context window size in tokens. Uses common model prefixes
/// to determine the appropriate context window size.
fn estimate_context_window(model: &str) -> usize {
    let model_lower = model.to_lowercase();

    // Claude models (Anthropic)
    if model_lower.contains("claude-3") || model_lower.contains("claude-2") {
        return 200_000; // Claude 3/2 have 200k context
    }
    if model_lower.contains("claude") {
        return 100_000; // Older Claude models
    }

    // GPT-4 class models (OpenAI)
    if model_lower.contains("gpt-4o") || model_lower.contains("gpt-4-turbo") {
        return 128_000; // GPT-4o and Turbo have 128k
    }
    if model_lower.contains("gpt-4") {
        return 128_000; // Standard GPT-4 context
    }

    // GPT-3.5 models
    if model_lower.contains("gpt-3.5-turbo-16k") {
        return 16_000;
    }
    if model_lower.contains("gpt-3.5") {
        return 4_000; // Standard GPT-3.5
    }

    // Codex models
    if model_lower.contains("codex") {
        return 200_000;
    }

    // O-series reasoning models
    if model_lower.starts_with("o1")
        || model_lower.starts_with("o3")
        || model_lower.starts_with("o4")
    {
        return 200_000;
    }

    // Default for unknown models
    128_000
}

/// Format token display with context percentage.
///
/// Shows tokens in compact form (e.g., "~1.2k") with percentage of context window.
fn format_token_display(tokens: usize, context_size: usize) -> String {
    let percentage = if context_size > 0 {
        (tokens as f64 / context_size as f64 * 100.0).min(100.0)
    } else {
        0.0
    };

    let token_str = if tokens >= 1000 {
        format!("~{:.1}k", tokens as f64 / 1000.0)
    } else {
        format!("~{}", tokens)
    };

    let context_str = if context_size >= 1_000_000 {
        format!("{:.1}M", context_size as f64 / 1_000_000.0)
    } else if context_size >= 1000 {
        format!("{}k", context_size / 1000)
    } else {
        format!("{}", context_size)
    };

    format!("{}/{} ({:.0}%)", token_str, context_str, percentage)
}

/// Format token display with context percentage and color-coded style.
///
/// Returns the formatted string and an appropriate style based on usage level:
/// - Green (DarkGray): 0-50% usage - plenty of context remaining
/// - Yellow: 50-80% usage - context getting limited
/// - Red: 80%+ usage - context nearly full
fn format_token_display_styled(tokens: usize, context_size: usize) -> (String, Style) {
    let text = format_token_display(tokens, context_size);

    let percentage = if context_size > 0 {
        (tokens as f64 / context_size as f64 * 100.0).min(100.0)
    } else {
        0.0
    };

    let style = if percentage >= 80.0 {
        Style::default().fg(Color::Red)
    } else if percentage >= 50.0 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    (text, style)
}

/// Render the header
fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    // Format working directory to fit
    let max_dir_width = (area.width as usize).saturating_sub(50); // Leave room for title and warning
    let working_dir = format_working_dir(&app.config.working_dir, max_dir_width);

    // Check context usage for warning
    let tokens = app.estimated_tokens();
    let context_size = estimate_context_window(&app.model);
    let percentage = if context_size > 0 {
        (tokens as f64 / context_size as f64 * 100.0).min(100.0)
    } else {
        0.0
    };

    // Build title spans
    let mut spans = vec![
        Span::styled("Codex DashFlow", Style::default().bold().fg(Color::Magenta)),
        Span::raw(" - "),
        Span::styled(&app.session_id, Style::default().dim()),
        Span::raw(" @ "),
        Span::styled(working_dir, Style::default().fg(Color::Blue)),
    ];

    // Add context warning if usage is high
    if percentage >= 80.0 {
        spans.push(Span::styled(
            " ⚠ CONTEXT FULL - use /compact",
            Style::default().fg(Color::Red).bold(),
        ));
    } else if percentage >= 60.0 {
        spans.push(Span::styled(
            " ⚠ Context High - try /compact",
            Style::default().fg(Color::Yellow),
        ));
    }

    let title = Line::from(spans);

    let header = Paragraph::new(title)
        .block(Block::default().borders(Borders::BOTTOM))
        .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(header, area);
}

/// Render the chat history with scroll support
fn render_chat(frame: &mut Frame, app: &App, area: Rect) {
    use ratatui::widgets::ListState;

    // Calculate available width for message text (minus borders and some padding)
    let available_width = area.width.saturating_sub(4) as usize;

    // Determine current match message index (if any)
    let current_match_msg_idx = app
        .current_match
        .and_then(|idx| app.search_matches.get(idx).copied());

    let items: Vec<ListItem> = app
        .messages
        .iter()
        .enumerate()
        .map(|(idx, msg)| {
            // Check if this message matches the search and if it's the current match
            let highlight = if !app.search_query.is_empty() && app.search_matches.contains(&idx) {
                Some(SearchHighlight {
                    query: &app.search_query,
                    is_current_match: current_match_msg_idx == Some(idx),
                })
            } else {
                None
            };
            render_message(msg, available_width, highlight.as_ref())
        })
        .collect();
    let total_messages = items.len();

    // Calculate visible height (area height minus borders)
    let _visible_height = area.height.saturating_sub(2) as usize;

    // Clamp scroll offset to valid range
    let max_scroll = total_messages.saturating_sub(1);
    let scroll_offset = app.scroll_offset.min(max_scroll);

    // Determine which message to select (from the bottom, accounting for scroll)
    let selected = if total_messages > 0 {
        Some(
            total_messages
                .saturating_sub(1)
                .saturating_sub(scroll_offset),
        )
    } else {
        None
    };

    let scroll_indicator = if scroll_offset > 0 {
        format!(" Chat [{} more below] ", scroll_offset)
    } else {
        " Chat ".to_string()
    };

    let chat = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(scroll_indicator),
    );

    // Use ListState to control scroll position
    let mut state = ListState::default();
    state.select(selected);

    frame.render_stateful_widget(chat, area, &mut state);
}

/// Check if a message content indicates an error
fn is_error_message(content: &str) -> bool {
    let content_lower = content.to_lowercase();
    content_lower.starts_with("error:")
        || content_lower.starts_with("error -")
        || content_lower.starts_with("failed:")
        || content_lower.starts_with("failure:")
        || content_lower.contains("error occurred")
        || content_lower.contains("connection failed")
        || content_lower.contains("timeout")
        || content_lower.contains("authentication failed")
}

/// Render a single chat message with word wrapping and optional search highlighting
fn render_message(
    msg: &ChatMessage,
    width: usize,
    highlight: Option<&SearchHighlight<'_>>,
) -> ListItem<'static> {
    // Check if this is an error message (for System/Tool messages)
    let is_error = matches!(msg.role, MessageRole::System | MessageRole::Tool)
        && is_error_message(&msg.content);

    let (prefix, base_style) = match msg.role {
        MessageRole::User => ("You: ", Style::default().fg(Color::Cyan)),
        MessageRole::Assistant => ("Agent: ", Style::default().fg(Color::Magenta)),
        MessageRole::System => {
            if is_error {
                ("✗ Error: ", Style::default().fg(Color::Red))
            } else {
                ("System: ", Style::default().dim())
            }
        }
        MessageRole::Tool => {
            // Parse tool message to determine display style:
            // - "Calling tool: X" -> in-progress indicator
            // - "✓ X: output" -> success (already has checkmark)
            // - "✗ X: output" or error -> failure
            if is_error || msg.content.starts_with("✗") {
                // Error/failure - show in red, no prefix (message has ✗ already)
                ("", Style::default().fg(Color::Red))
            } else if msg.content.starts_with("Calling tool:") {
                // In-progress tool call - show with hourglass
                ("⏳ ", Style::default().fg(Color::Yellow))
            } else if msg.content.starts_with("✓") {
                // Success - show in green, no prefix (message has ✓ already)
                ("", Style::default().fg(Color::Green))
            } else {
                // Generic tool message
                ("🔧 ", Style::default().fg(Color::DarkGray))
            }
        }
    };

    // Use markdown rendering for assistant messages
    if matches!(msg.role, MessageRole::Assistant) {
        let mut text = render_markdown(&msg.content);
        // Add prefix to first line
        if let Some(first_line) = text.lines.first_mut() {
            first_line
                .spans
                .insert(0, Span::styled(prefix.to_string(), base_style));
        } else {
            text.lines
                .push(Line::from(Span::styled(prefix.to_string(), base_style)));
        }

        // Apply search highlighting to markdown text if needed
        if let Some(hl) = highlight {
            text = highlight_text_with_search(&text, hl);
        }

        // Apply word wrapping
        let wrapped = wrap_text(&text, width);
        ListItem::new(wrapped)
    } else {
        // For error messages, strip the redundant "Error:" prefix if present
        let content = if is_error && msg.content.to_lowercase().starts_with("error:") {
            msg.content
                .trim_start_matches("Error:")
                .trim_start_matches("error:")
                .trim()
        } else {
            &msg.content
        };

        // Build lines with optional search highlighting
        let content_with_prefix = format!("{}{}", prefix, content);
        let lines: Vec<Line> = if let Some(hl) = highlight {
            content_with_prefix
                .lines()
                .map(|line| {
                    let spans = highlight_search_in_text(line, hl, base_style);
                    Line::from(spans)
                })
                .collect()
        } else {
            content_with_prefix
                .lines()
                .map(|line| Line::from(Span::styled(line.to_string(), base_style)))
                .collect()
        };
        let text = Text::from(lines);
        // Apply word wrapping
        let wrapped = wrap_text(&text, width);
        ListItem::new(wrapped)
    }
}

/// Apply search highlighting to a Text widget (for markdown content)
fn highlight_text_with_search<'a>(
    text: &Text<'a>,
    highlight: &SearchHighlight<'_>,
) -> Text<'static> {
    let query_lower = highlight.query.to_lowercase();

    let new_lines: Vec<Line<'static>> = text
        .lines
        .iter()
        .map(|line| {
            // Combine all spans in this line to check for matches
            let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            let line_lower = line_text.to_lowercase();

            // If no match in this line, just clone it with owned strings
            if !line_lower.contains(&query_lower) {
                let spans: Vec<Span<'static>> = line
                    .spans
                    .iter()
                    .map(|s| Span::styled(s.content.to_string(), s.style))
                    .collect();
                return Line::from(spans);
            }

            // There's a match - we need to re-render with highlighting
            // For simplicity, we merge all spans and re-apply highlighting
            // This loses some original styling but highlights correctly
            let base_style = line.spans.first().map(|s| s.style).unwrap_or_default();
            let spans = highlight_search_in_text(&line_text, highlight, base_style);
            Line::from(spans)
        })
        .collect();

    Text::from(new_lines)
}

/// Render the input area
fn render_input(frame: &mut Frame, app: &App, area: Rect) {
    // Handle search mode specially
    if matches!(app.mode, AppMode::Search) {
        let match_info = if app.search_matches.is_empty() {
            if app.search_query.is_empty() {
                String::new()
            } else {
                " (no matches)".to_string()
            }
        } else if let Some(idx) = app.current_match {
            format!(" ({}/{})", idx + 1, app.search_matches.len())
        } else {
            String::new()
        };

        let search_text = format!("/{}{}", app.search_query, match_info);
        let search_style = if app.search_matches.is_empty() && !app.search_query.is_empty() {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Yellow)
        };

        let search_input = Paragraph::new(search_text.as_str())
            .style(search_style)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Search [Enter:confirm Esc:cancel ↑↓:navigate] "),
            );

        frame.render_widget(search_input, area);

        // Show cursor at end of search query
        frame.set_cursor_position((area.x + app.search_query.len() as u16 + 2, area.y + 1));
        return;
    }

    let (input_style, title) = match app.mode {
        AppMode::Normal => (
            Style::default(),
            " Input [i:insert /:search q:quit j/k:scroll] ",
        ),
        AppMode::Insert => (
            Style::default().fg(Color::Cyan),
            " Input [Ctrl+Enter:send Enter:newline Esc:normal] ",
        ),
        AppMode::Processing => (Style::default().dim(), " Input [Processing...] "),
        AppMode::Search => unreachable!(), // Handled above
    };

    // Show position/length indicator based on input type
    let line_count = app.input_line_count();
    let char_count = app.input.chars().count();
    let title_with_info = if line_count > 1 {
        // Multi-line: show Ln:Col
        let (row, col) = app.cursor_row_col();
        format!(
            "{} (Ln:{}/{} Col:{})",
            title.trim(),
            row + 1, // 1-indexed for user display
            line_count,
            col + 1 // 1-indexed for user display
        )
    } else if char_count > 0 {
        // Single-line with content: show word and character count
        let word_count = app.input.split_whitespace().count();
        if word_count > 1 {
            format!(
                "{} ({} words, {} chars)",
                title.trim(),
                word_count,
                char_count
            )
        } else {
            format!("{} ({} chars)", title.trim(), char_count)
        }
    } else {
        title.to_string()
    };

    // Calculate visible width for input area (inside borders)
    let visible_width = area.width.saturating_sub(2) as usize;

    // Selection style (blue background with white text for good contrast)
    let selection_style = Style::default().bg(Color::Blue).fg(Color::White);

    // Determine display text - show placeholder if input is empty
    let (display_text, display_style, use_horizontal_scroll) = if app.input.is_empty() {
        // Show placeholder text based on mode
        let placeholder = match app.mode {
            AppMode::Insert => "Type a message... (/ for commands, Ctrl+Enter to send)",
            AppMode::Normal => "Press 'i' to enter insert mode",
            AppMode::Processing => "",
            AppMode::Search => "", // Search mode handled above
        };
        (placeholder.to_string(), input_style.dim(), false)
    } else {
        // Use horizontal scrolling for single-line input that exceeds visible width
        let is_single_line = app.input_line_count() == 1;
        let exceeds_width = app.input.chars().count() > visible_width;

        if is_single_line && exceeds_width {
            // Determine which scroll indicators are needed
            let has_left = app.input_scroll_offset > 0;
            let total_chars = app.input.chars().count();
            let has_right = app.input_scroll_offset + visible_width < total_chars;

            // Calculate effective visible width accounting for indicators
            let indicator_count = if has_left { 1 } else { 0 } + if has_right { 1 } else { 0 };
            let effective_width = visible_width.saturating_sub(indicator_count);

            let (visible_text, _cursor_offset) = app.visible_input_slice(effective_width);

            // Build display with appropriate indicators
            let display = match (has_left, has_right) {
                (true, true) => format!("◀{}▶", visible_text),
                (true, false) => format!("◀{}", visible_text),
                (false, true) => format!("{}▶", visible_text),
                (false, false) => visible_text.to_string(),
            };
            (display, input_style, true)
        } else {
            (app.input.clone(), input_style, false)
        }
    };

    // Build the paragraph - use wrapping only for multi-line input
    // Handle selection highlighting by creating styled spans
    let has_selection = app.has_selection();
    let selection_range = app.selection_range();

    let input = if use_horizontal_scroll {
        // For horizontal scrolling, create styled text if there's a selection
        let text = if has_selection && !app.input.is_empty() {
            let (sel_start, sel_end) = selection_range.unwrap();
            // Adjust selection range for scroll offset
            let scroll = app.input_scroll_offset;
            let has_left = scroll > 0;
            let total_chars = app.input.chars().count();
            let has_right = scroll + visible_width < total_chars;
            let indicator_count = if has_left { 1 } else { 0 } + if has_right { 1 } else { 0 };
            let effective_width = visible_width.saturating_sub(indicator_count);

            // Get visible char range
            let vis_start = scroll;
            let vis_end = (scroll + effective_width).min(total_chars);

            // Build spans for the visible portion
            let mut spans: Vec<Span> = Vec::new();

            // Left indicator
            if has_left {
                spans.push(Span::raw("◀"));
            }

            // Iterate through visible characters
            let chars: Vec<char> = app.input.chars().collect();
            for (idx, &ch) in chars
                .iter()
                .enumerate()
                .skip(vis_start)
                .take(vis_end - vis_start)
            {
                let in_selection = idx >= sel_start && idx < sel_end;
                if in_selection {
                    spans.push(Span::styled(ch.to_string(), selection_style));
                } else {
                    spans.push(Span::styled(ch.to_string(), display_style));
                }
            }

            // Right indicator
            if has_right {
                spans.push(Span::raw("▶"));
            }

            Text::from(Line::from(spans))
        } else {
            Text::raw(display_text.as_str())
        };

        Paragraph::new(text).style(display_style).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title_with_info.as_str()),
        )
        // No wrap for horizontal scrolling
    } else {
        // For multi-line or simple input, create styled text if there's a selection
        let text = if has_selection && !app.input.is_empty() {
            let (sel_start, sel_end) = selection_range.unwrap();

            // Build lines with selection highlighting
            let mut lines: Vec<Line> = Vec::new();
            let mut current_line_spans: Vec<Span> = Vec::new();
            let chars: Vec<char> = app.input.chars().collect();

            for (idx, &ch) in chars.iter().enumerate() {
                let in_selection = idx >= sel_start && idx < sel_end;
                if ch == '\n' {
                    lines.push(Line::from(current_line_spans.clone()));
                    current_line_spans.clear();
                } else if in_selection {
                    current_line_spans.push(Span::styled(ch.to_string(), selection_style));
                } else {
                    current_line_spans.push(Span::styled(ch.to_string(), display_style));
                }
            }
            // Don't forget the last line
            if !current_line_spans.is_empty() || app.input.ends_with('\n') {
                lines.push(Line::from(current_line_spans));
            }

            Text::from(lines)
        } else {
            Text::raw(display_text.as_str())
        };

        Paragraph::new(text)
            .style(display_style)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title_with_info.as_str()),
            )
            .wrap(Wrap { trim: false })
    };

    frame.render_widget(input, area);

    // Show cursor in insert or normal mode
    if matches!(app.mode, AppMode::Insert | AppMode::Normal) {
        if use_horizontal_scroll {
            // For horizontal scrolling, cursor is relative to scroll offset
            // Recalculate effective width accounting for both indicators
            let has_left = app.input_scroll_offset > 0;
            let total_chars = app.input.chars().count();
            let has_right = app.input_scroll_offset + visible_width < total_chars;
            let indicator_count = if has_left { 1 } else { 0 } + if has_right { 1 } else { 0 };
            let effective_width = visible_width.saturating_sub(indicator_count);

            let (_visible_text, cursor_offset) = app.visible_input_slice(effective_width);
            // Account for left scroll indicator if present
            let left_indicator_offset = if has_left { 1 } else { 0 };
            let cursor_x = area.x + 1 + left_indicator_offset + cursor_offset as u16;
            let cursor_y = area.y + 1;
            frame.set_cursor_position((cursor_x, cursor_y));
        } else {
            // Calculate cursor position for multi-line input
            let (row, col) = app.cursor_row_col();
            // Cursor x: area.x + 1 (border) + col
            // Cursor y: area.y + 1 (border) + row
            let cursor_x = area.x + 1 + col as u16;
            let cursor_y = area.y + 1 + row as u16;

            // Only show cursor if it's within the visible area
            if cursor_y < area.y + area.height - 1 {
                frame.set_cursor_position((cursor_x, cursor_y));
            }
        }
    }
}

/// Render the status bar
fn render_status(frame: &mut Frame, app: &App, area: Rect) {
    use codex_dashflow_core::AuthStatus;

    let status = match app.mode {
        AppMode::Normal => "NORMAL",
        AppMode::Insert => "INSERT",
        AppMode::Processing => "PROCESSING...",
        AppMode::Search => "SEARCH",
    };

    let turn_info = format!("Turn: {}", app.turn_count);
    let model_info = format!("Model: {}", app.model);

    // Token estimate with context window percentage and color coding
    let tokens = app.estimated_tokens();
    let context_size = estimate_context_window(&app.model);
    let (token_info, token_style) = format_token_display_styled(tokens, context_size);

    // Auth status indicator
    let (auth_icon, auth_style) = match &app.auth_status {
        AuthStatus::ChatGpt { .. } => ("ChatGPT", Style::default().fg(Color::Green)),
        AuthStatus::ApiKey => ("API Key", Style::default().fg(Color::Blue)),
        AuthStatus::EnvApiKey => ("Env", Style::default().fg(Color::Cyan)),
        AuthStatus::NotAuthenticated => ("Not Auth", Style::default().fg(Color::Red)),
    };

    // Approval mode indicator - shows current security/approval preset
    let (approval_icon, approval_style) = match app.config.approval_preset.as_str() {
        "read-only" => ("RO", Style::default().fg(Color::Yellow).bold()),
        "auto" => ("Auto", Style::default().fg(Color::Cyan)),
        "full-access" => ("Full", Style::default().fg(Color::Red).bold()),
        other => (other, Style::default().fg(Color::Gray)),
    };

    // Get current spinner character
    let spinner = SPINNER_FRAMES[app.spinner_frame % SPINNER_FRAMES.len()];

    // Agent status indicator (only show when processing)
    let agent_status_spans = match &app.agent_status {
        AgentStatus::Idle => vec![],
        AgentStatus::Thinking { model } => vec![
            Span::raw(" | "),
            Span::styled(
                format!("{} Thinking ({})", spinner, model),
                Style::default().fg(Color::Yellow),
            ),
        ],
        AgentStatus::ExecutingTool { tool } => vec![
            Span::raw(" | "),
            Span::styled(
                format!("{} Running: {}", spinner, tool),
                Style::default().fg(Color::Magenta),
            ),
        ],
        AgentStatus::Complete { duration_ms } => vec![
            Span::raw(" | "),
            Span::styled(
                format!("✓ Done ({:.1}s)", *duration_ms as f64 / 1000.0),
                Style::default().fg(Color::Green),
            ),
        ],
        AgentStatus::Error { message } => vec![
            Span::raw(" | "),
            Span::styled(
                format!("✗ Error: {}", message),
                Style::default().fg(Color::Red),
            ),
        ],
    };

    // Session metrics spans (show tokens and cost when available)
    let session_metrics_spans = if app.session_metrics.llm_call_count > 0 {
        let total_tokens =
            app.session_metrics.total_input_tokens + app.session_metrics.total_output_tokens;
        let token_display = if total_tokens >= 1_000_000 {
            format!("{:.1}M", total_tokens as f64 / 1_000_000.0)
        } else if total_tokens >= 1_000 {
            format!("{:.1}k", total_tokens as f64 / 1_000.0)
        } else {
            format!("{}", total_tokens)
        };

        let mut metrics_spans = vec![
            Span::raw(" | "),
            Span::styled(
                format!("📊 {} tok", token_display),
                Style::default().fg(Color::Cyan),
            ),
        ];

        // Add cost if available
        if let Some(cost) = app.session_metrics.total_cost_usd {
            let cost_display = if cost >= 1.0 {
                format!("${:.2}", cost)
            } else if cost >= 0.01 {
                format!("${:.3}", cost)
            } else {
                format!("${:.4}", cost)
            };
            metrics_spans.push(Span::raw(" "));
            metrics_spans.push(Span::styled(
                cost_display,
                Style::default().fg(Color::Yellow),
            ));
        }

        metrics_spans
    } else {
        vec![]
    };

    let mut spans = vec![
        Span::styled(
            format!(" {} ", status),
            Style::default().bg(Color::DarkGray).bold(),
        ),
        Span::raw(" "),
        Span::styled(turn_info, Style::default().dim()),
        Span::raw(" | "),
        Span::styled(model_info, Style::default().dim()),
        Span::raw(" | "),
        Span::styled(token_info, token_style),
        Span::raw(" | "),
        Span::styled(auth_icon, auth_style),
        Span::raw(" | "),
        Span::styled(approval_icon, approval_style),
    ];
    spans.extend(agent_status_spans);
    spans.extend(session_metrics_spans);

    let status_line = Line::from(spans);

    let status_bar = Paragraph::new(status_line);
    frame.render_widget(status_bar, area);
}

/// Render the command completion popup above the input area
fn render_command_popup(frame: &mut Frame, app: &App, input_area: Rect) {
    use ratatui::widgets::Clear;

    let commands = app.command_popup.visible_commands();
    if commands.is_empty() {
        return;
    }

    // Calculate popup dimensions
    let popup_height = commands.len() as u16 + 2; // +2 for borders
    let popup_width = input_area.width.saturating_sub(4).min(60); // Max 60, leave margin

    // Position popup above the input area
    let popup_y = input_area.y.saturating_sub(popup_height);
    let popup_x = input_area.x + 1;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    // Build list items with selection and match highlighting
    let items: Vec<ListItem> = commands
        .iter()
        .map(|(cmd, desc, is_selected, match_positions)| {
            let base_style = if *is_selected {
                Style::default().fg(Color::Cyan).bold()
            } else {
                Style::default()
            };
            let highlight_style = if *is_selected {
                Style::default().fg(Color::Yellow).bold()
            } else {
                Style::default().fg(Color::Yellow)
            };

            // Build spans for command with highlighted match positions
            let cmd_spans =
                build_highlighted_spans(cmd, match_positions, base_style, highlight_style);

            // Pad the command to align descriptions
            let cmd_display_len: usize = cmd.chars().count();
            let padding = " ".repeat(12usize.saturating_sub(cmd_display_len));

            let mut spans = cmd_spans;
            spans.push(Span::styled(padding, base_style));
            spans.push(Span::styled(desc.to_string(), base_style.dim()));

            ListItem::new(Line::from(spans))
        })
        .collect();

    // Build title with scroll indicator
    let title = if app.command_popup.has_more_items() {
        format!(
            " Commands ({}/{}) [↑↓:select Tab/Enter:confirm Esc:cancel] ",
            app.command_popup.selected_index + 1,
            app.command_popup.total_count()
        )
    } else {
        " Commands [↑↓:select Tab/Enter:confirm Esc:cancel] ".to_string()
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green))
            .title(title)
            .title_style(Style::default().fg(Color::Green)),
    );

    frame.render_widget(list, popup_area);
}

/// Render the help overlay
fn render_help_overlay(frame: &mut Frame) {
    use ratatui::widgets::Clear;

    let help_text = vec![
        Line::from(Span::styled(
            " Keyboard Shortcuts ",
            Style::default().bold().fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Normal Mode:",
            Style::default().bold().fg(Color::Yellow),
        )),
        Line::from("  i      Enter insert mode"),
        Line::from("  a      Append after cursor"),
        Line::from("  A      Append at end of line"),
        Line::from("  I      Insert at beginning"),
        Line::from("  q      Quit application"),
        Line::from("  ?      Toggle this help"),
        Line::from(""),
        Line::from(Span::styled(
            "Navigation:",
            Style::default().bold().fg(Color::Yellow),
        )),
        Line::from("  h/l    Move cursor left/right"),
        Line::from("  j/k    Scroll chat down/up"),
        Line::from("  g/G    Go to top/bottom of chat"),
        Line::from("  0/$    Go to start/end of line"),
        Line::from("  PgUp/PgDn  Scroll 10 messages"),
        Line::from(""),
        Line::from(Span::styled(
            "Search:",
            Style::default().bold().fg(Color::Yellow),
        )),
        Line::from("  /      Start search"),
        Line::from("  n/N    Next/prev match"),
        Line::from(""),
        Line::from(Span::styled(
            "Editing:",
            Style::default().bold().fg(Color::Yellow),
        )),
        Line::from("  x      Delete char under cursor"),
        Line::from("  d      Clear entire line"),
        Line::from(""),
        Line::from(Span::styled(
            "Insert Mode:",
            Style::default().bold().fg(Color::Yellow),
        )),
        Line::from("  Esc       Return to Normal mode"),
        Line::from("  Ctrl+Enter Submit input"),
        Line::from("  Enter     Add new line"),
        Line::from("  Tab       Complete /command"),
        Line::from("  @filename Mention file (fuzzy search)"),
        Line::from("  ↑/↓       Move in multi-line / history"),
        Line::from("  Ctrl+P/N  Previous/next in history"),
        Line::from("  Ctrl+Z    Undo input change"),
        Line::from("  Ctrl+Y    Redo input change"),
        Line::from("  Ctrl+A    Select all text"),
        Line::from("  Ctrl+C    Copy selection"),
        Line::from("  Ctrl+X    Cut selection"),
        Line::from("  Ctrl+V    Paste from clipboard"),
        Line::from("  Home/End  Line start/end"),
        Line::from("  Ctrl+Home/End Document start/end"),
        Line::from("  Shift+←/→ Select text left/right"),
        Line::from("  Shift+↑/↓ Select text up/down"),
        Line::from("  Shift+Home/End Select to line boundary"),
        Line::from("  Ctrl/Alt+←/→ Move by word"),
        Line::from("  Shift+Ctrl/Alt+←/→ Select by word"),
        Line::from("  Ctrl/Alt+Bksp Delete word left"),
        Line::from("  Ctrl/Alt+Del Delete word right"),
        Line::from("  Ctrl+U    Delete to line start"),
        Line::from("  Ctrl+K    Delete to line end"),
        Line::from(""),
        Line::from(Span::styled(
            "Global:",
            Style::default().bold().fg(Color::Yellow),
        )),
        Line::from("  Ctrl+C Quit (any mode)"),
        Line::from("  Ctrl+D Quit (when input empty)"),
        Line::from("  Ctrl+M Cycle approval mode"),
        Line::from("  Esc    Dismiss notification"),
        Line::from(""),
        Line::from(Span::styled(
            "Slash Commands:",
            Style::default().bold().fg(Color::Yellow),
        )),
        Line::from("  /help     Show this help"),
        Line::from("  /quit     Quit application"),
        Line::from("  /new      Start new session"),
        Line::from("  /clear    Clear chat history"),
        Line::from("  /compact  Reduce context size"),
        Line::from("  /undo     Undo last turn"),
        Line::from("  /status   Show session info"),
        Line::from("  /tokens   Show token usage"),
        Line::from("  /model    Show/change model"),
        Line::from("  /mode     Change approval mode"),
        Line::from("  /stop     Cancel running task"),
        Line::from("  /export   Export conversation"),
        Line::from("  /diff     Show git diff"),
        Line::from("  /resume   Resume prior session"),
        Line::from(""),
        Line::from(Span::styled(
            "Press ?, Esc, or q to close",
            Style::default().dim(),
        )),
    ];

    // Calculate centered popup area (60% width, constrained height)
    let popup_width = (frame.area().width as f32 * 0.6).min(50.0) as u16;
    let popup_width = popup_width.max(30); // Minimum width of 30
    let ideal_height = help_text.len() as u16 + 2; // +2 for borders
    let popup_height = ideal_height.min(frame.area().height.saturating_sub(2)); // Leave margin
    let popup_x = (frame.area().width.saturating_sub(popup_width)) / 2;
    let popup_y = (frame.area().height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let help_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Help ")
        .title_style(Style::default().bold().fg(Color::Cyan));

    let help_widget = Paragraph::new(help_text)
        .block(help_block)
        .wrap(Wrap { trim: false });

    frame.render_widget(help_widget, popup_area);
}

/// Render the approval overlay for tool approval dialogs
fn render_approval_overlay(frame: &mut Frame, app: &App) {
    use ratatui::widgets::Clear;

    let request = match &app.approval_overlay.request {
        Some(r) => r,
        None => return,
    };

    // Build the overlay content
    let mut lines = vec![
        Line::from(Span::styled(
            format!(
                " {} Approval Required ",
                request.request_type.display_name()
            ),
            Style::default().bold().fg(Color::Yellow),
        )),
        Line::from(""),
    ];

    // Display the tool/command being executed
    match &request.request_type {
        ApprovalRequestType::Shell { command } => {
            lines.push(Line::from(vec![
                Span::styled("Command: ", Style::default().fg(Color::Cyan)),
                Span::styled(format!("$ {}", command), Style::default().fg(Color::White)),
            ]));
        }
        ApprovalRequestType::FileWrite { path } => {
            lines.push(Line::from(vec![
                Span::styled("File: ", Style::default().fg(Color::Cyan)),
                Span::styled(path.clone(), Style::default().fg(Color::White)),
            ]));
        }
        ApprovalRequestType::Tool { tool, args } => {
            lines.push(Line::from(vec![
                Span::styled("Tool: ", Style::default().fg(Color::Cyan)),
                Span::styled(tool.clone(), Style::default().fg(Color::White)),
            ]));
            if !args.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("Args: ", Style::default().fg(Color::Cyan)),
                    Span::styled(args.clone(), Style::default().fg(Color::Gray)),
                ]));
            }
        }
    }

    // Display reason if provided
    if let Some(reason) = &request.reason {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Reason: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                reason.clone(),
                Style::default().fg(Color::DarkGray).italic(),
            ),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Choose an action:",
        Style::default().bold(),
    )));
    lines.push(Line::from(""));

    // Render options
    for (label, is_selected, hotkey) in app.approval_overlay.options() {
        let marker = if is_selected { "▸ " } else { "  " };
        let style = if is_selected {
            Style::default().fg(Color::Cyan).bold()
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(vec![
            Span::styled(marker, style),
            Span::styled(format!("[{}] ", hotkey), Style::default().fg(Color::Yellow)),
            Span::styled(label, style),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press y/a/n or Enter to confirm, Esc to cancel",
        Style::default().dim(),
    )));

    // Calculate centered popup area
    let popup_width = (frame.area().width as f32 * 0.7).min(60.0) as u16;
    let popup_width = popup_width.max(40); // Minimum width of 40
    let popup_height = (lines.len() as u16 + 2).min(frame.area().height.saturating_sub(4)); // +2 for borders
    let popup_x = (frame.area().width.saturating_sub(popup_width)) / 2;
    let popup_y = (frame.area().height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Tool Approval ")
        .title_style(Style::default().bold().fg(Color::Yellow));

    let widget = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(widget, popup_area);
}

/// Render a transient notification toast at the top-right of the screen
fn render_notification(frame: &mut Frame, notification: &crate::app::Notification) {
    use ratatui::widgets::Clear;

    // Determine color based on notification style
    let (fg_color, border_color) = match notification.style {
        NotificationStyle::Info => (Color::Cyan, Color::Cyan),
        NotificationStyle::Success => (Color::Green, Color::Green),
        NotificationStyle::Warning => (Color::Yellow, Color::Yellow),
        NotificationStyle::Error => (Color::Red, Color::Red),
    };

    // Calculate notification dimensions
    let text_width = notification.text.chars().count() as u16;
    let notif_width = text_width
        .saturating_add(4)
        .min(frame.area().width.saturating_sub(4)); // +4 for padding and borders
    let notif_height = 3u16; // Single line + borders

    // Position at top-right corner with some margin
    let notif_x = frame.area().width.saturating_sub(notif_width + 2);
    let notif_y = 1; // Below header area

    let notif_area = Rect::new(notif_x, notif_y, notif_width, notif_height);

    // Clear the area behind the notification
    frame.render_widget(Clear, notif_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let text = Paragraph::new(Span::styled(
        &notification.text,
        Style::default().fg(fg_color).bold(),
    ))
    .block(block);

    frame.render_widget(text, notif_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, AppConfig, AppMode, ChatMessage, MessageRole};
    use insta::assert_snapshot;
    use ratatui::{backend::TestBackend, Terminal};

    /// Create a test app with default configuration
    fn create_test_app() -> App {
        let config = AppConfig {
            session_id: Some("test-session-12345".to_string()),
            model: "gpt-4o-mini".to_string(),
            use_mock_llm: true, // Use mock mode for consistent auth status in tests
            ..Default::default()
        };
        App::new(config)
    }

    /// Helper to render the full UI and snapshot it
    fn snapshot_full_ui(name: &str, app: &App) {
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| {
                render(frame, app);
            })
            .unwrap();
        assert_snapshot!(name, terminal.backend());
    }

    /// Helper to render just the header and snapshot it
    fn snapshot_header(name: &str, app: &App) {
        let mut terminal = Terminal::new(TestBackend::new(80, 3)).unwrap();
        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 80, 3);
                render_header(frame, app, area);
            })
            .unwrap();
        assert_snapshot!(name, terminal.backend());
    }

    /// Helper to render just the status bar and snapshot it
    fn snapshot_status(name: &str, app: &App) {
        let mut terminal = Terminal::new(TestBackend::new(80, 1)).unwrap();
        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 80, 1);
                render_status(frame, app, area);
            })
            .unwrap();
        assert_snapshot!(name, terminal.backend());
    }

    /// Helper to render just the input area and snapshot it
    fn snapshot_input(name: &str, app: &App) {
        let mut terminal = Terminal::new(TestBackend::new(80, 3)).unwrap();
        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 80, 3);
                render_input(frame, app, area);
            })
            .unwrap();
        assert_snapshot!(name, terminal.backend());
    }

    /// Helper to render just the chat area and snapshot it
    fn snapshot_chat(name: &str, app: &App) {
        let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 80, 10);
                render_chat(frame, app, area);
            })
            .unwrap();
        assert_snapshot!(name, terminal.backend());
    }

    // === Full UI Tests ===

    #[test]
    fn test_full_ui_initial_state() {
        let app = create_test_app();
        snapshot_full_ui("full_ui_initial_state", &app);
    }

    #[test]
    fn test_full_ui_with_input() {
        let mut app = create_test_app();
        app.input = "Hello, how can you help me?".to_string();
        app.cursor_position = app.input.len();
        snapshot_full_ui("full_ui_with_input", &app);
    }

    #[test]
    fn test_full_ui_processing_mode() {
        let mut app = create_test_app();
        app.mode = AppMode::Processing;
        snapshot_full_ui("full_ui_processing_mode", &app);
    }

    // === Header Tests ===

    #[test]
    fn test_header_default() {
        let app = create_test_app();
        snapshot_header("header_default", &app);
    }

    #[test]
    fn test_header_with_custom_working_dir() {
        let config = AppConfig {
            session_id: Some("test-session".to_string()),
            working_dir: "/tmp/project".to_string(),
            ..Default::default()
        };
        let app = App::new(config);
        snapshot_header("header_custom_working_dir", &app);
    }

    // === format_working_dir Tests ===

    #[test]
    fn test_format_working_dir_short_path() {
        let result = super::format_working_dir("/tmp/test", 50);
        assert_eq!(result, "/tmp/test");
    }

    #[test]
    fn test_format_working_dir_home_replacement() {
        // Set HOME env var for test
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        let path = format!("{}/projects/my-app", home);
        let result = super::format_working_dir(&path, 50);
        assert!(
            result.starts_with('~'),
            "Expected path to start with ~, got: {}",
            result
        );
        assert!(result.contains("projects/my-app"));
    }

    #[test]
    fn test_format_working_dir_truncation() {
        let long_path =
            "/very/long/path/that/should/be/truncated/to/fit/within/max/width/directory";
        let result = super::format_working_dir(long_path, 20);
        assert!(
            result.len() <= 20,
            "Expected length <= 20, got: {} ({})",
            result.len(),
            result
        );
        assert!(
            result.starts_with("..."),
            "Expected truncated path to start with ..."
        );
    }

    #[test]
    fn test_format_working_dir_preserves_final_component() {
        let path = "/some/path/to/my_project";
        let result = super::format_working_dir(path, 20);
        // Should try to keep "my_project" visible
        assert!(
            result.contains("my_project"),
            "Expected final component preserved: {}",
            result
        );
    }

    #[test]
    fn test_format_working_dir_dot_resolution() {
        // "." should resolve to current directory
        let result = super::format_working_dir(".", 100);
        // It should not be "." anymore (resolved to actual path)
        assert_ne!(result, ".", "Expected '.' to be resolved to actual path");
    }

    // === Context Window Tests ===

    #[test]
    fn test_estimate_context_window_gpt4() {
        assert_eq!(super::estimate_context_window("gpt-4o-mini"), 128_000);
        assert_eq!(super::estimate_context_window("gpt-4-turbo"), 128_000);
        assert_eq!(super::estimate_context_window("gpt-4"), 128_000);
    }

    #[test]
    fn test_estimate_context_window_claude() {
        assert_eq!(super::estimate_context_window("claude-3-opus"), 200_000);
        assert_eq!(super::estimate_context_window("claude-3-sonnet"), 200_000);
        assert_eq!(super::estimate_context_window("claude-2"), 200_000);
    }

    #[test]
    fn test_estimate_context_window_gpt35() {
        assert_eq!(super::estimate_context_window("gpt-3.5-turbo"), 4_000);
        assert_eq!(super::estimate_context_window("gpt-3.5-turbo-16k"), 16_000);
    }

    #[test]
    fn test_estimate_context_window_default() {
        // Unknown models should default to 128k
        assert_eq!(super::estimate_context_window("unknown-model"), 128_000);
    }

    #[test]
    fn test_format_token_display_small() {
        let result = super::format_token_display(500, 128_000);
        assert!(result.contains("~500"), "Expected ~500 in: {}", result);
        assert!(result.contains("128k"), "Expected 128k in: {}", result);
        assert!(result.contains("0%"), "Expected 0% in: {}", result);
    }

    #[test]
    fn test_format_token_display_thousands() {
        let result = super::format_token_display(5000, 128_000);
        assert!(result.contains("~5.0k"), "Expected ~5.0k in: {}", result);
        assert!(result.contains("128k"), "Expected 128k in: {}", result);
        // 5000/128000 = ~4%
        assert!(result.contains("4%"), "Expected 4% in: {}", result);
    }

    #[test]
    fn test_format_token_display_high_percentage() {
        let result = super::format_token_display(100_000, 128_000);
        // 100000/128000 = ~78%
        assert!(result.contains("78%"), "Expected 78% in: {}", result);
    }

    #[test]
    fn test_format_token_display_large_context() {
        let result = super::format_token_display(1000, 1_000_000);
        assert!(result.contains("1.0M"), "Expected 1.0M in: {}", result);
    }

    #[test]
    fn test_format_token_display_styled_low_usage() {
        // <50% should be DarkGray
        let (_, style) = super::format_token_display_styled(10_000, 128_000);
        // 10k/128k = ~8% - should be DarkGray
        assert_eq!(style, Style::default().fg(Color::DarkGray));
    }

    #[test]
    fn test_format_token_display_styled_medium_usage() {
        // 50-80% should be Yellow
        let (_, style) = super::format_token_display_styled(70_000, 128_000);
        // 70k/128k = ~55% - should be Yellow
        assert_eq!(style, Style::default().fg(Color::Yellow));
    }

    #[test]
    fn test_format_token_display_styled_high_usage() {
        // >80% should be Red
        let (_, style) = super::format_token_display_styled(110_000, 128_000);
        // 110k/128k = ~86% - should be Red
        assert_eq!(style, Style::default().fg(Color::Red));
    }

    #[test]
    fn test_format_token_display_styled_returns_text() {
        // Should return the same text as format_token_display
        let (text, _) = super::format_token_display_styled(5000, 128_000);
        let expected = super::format_token_display(5000, 128_000);
        assert_eq!(text, expected);
    }

    // === Status Bar Tests ===

    #[test]
    fn test_status_normal_mode() {
        let mut app = create_test_app();
        app.mode = AppMode::Normal;
        snapshot_status("status_normal_mode", &app);
    }

    #[test]
    fn test_status_insert_mode() {
        let app = create_test_app();
        // App starts in Insert mode by default
        snapshot_status("status_insert_mode", &app);
    }

    #[test]
    fn test_status_processing_mode() {
        let mut app = create_test_app();
        app.mode = AppMode::Processing;
        snapshot_status("status_processing_mode", &app);
    }

    #[test]
    fn test_status_with_turns() {
        let mut app = create_test_app();
        app.turn_count = 5;
        snapshot_status("status_with_turns", &app);
    }

    // === Approval Mode Indicator Tests ===

    #[test]
    fn test_status_approval_mode_auto() {
        let config = AppConfig {
            approval_preset: "auto".to_string(),
            use_mock_llm: true,
            ..Default::default()
        };
        let app = App::new(config);
        snapshot_status("status_approval_auto", &app);
    }

    #[test]
    fn test_status_approval_mode_read_only() {
        let config = AppConfig {
            approval_preset: "read-only".to_string(),
            use_mock_llm: true,
            ..Default::default()
        };
        let app = App::new(config);
        snapshot_status("status_approval_read_only", &app);
    }

    #[test]
    fn test_status_approval_mode_full_access() {
        let config = AppConfig {
            approval_preset: "full-access".to_string(),
            use_mock_llm: true,
            ..Default::default()
        };
        let app = App::new(config);
        snapshot_status("status_approval_full_access", &app);
    }

    // === Input Area Tests ===

    #[test]
    fn test_input_empty() {
        let app = create_test_app();
        snapshot_input("input_empty", &app);
    }

    #[test]
    fn test_input_with_text() {
        let mut app = create_test_app();
        app.input = "Create a new file called test.rs".to_string();
        app.cursor_position = app.input.len();
        snapshot_input("input_with_text", &app);
    }

    #[test]
    fn test_input_processing_style() {
        let mut app = create_test_app();
        app.input = "Processing...".to_string();
        app.mode = AppMode::Processing;
        snapshot_input("input_processing_style", &app);
    }

    #[test]
    fn test_input_placeholder_normal_mode() {
        // Empty input in Normal mode should show placeholder "Press 'i' to enter insert mode"
        let mut app = create_test_app();
        app.input.clear();
        app.mode = AppMode::Normal;
        snapshot_input("input_placeholder_normal", &app);
    }

    #[test]
    fn test_input_placeholder_insert_mode() {
        // Empty input in Insert mode should show placeholder about typing
        let mut app = create_test_app();
        app.input.clear();
        app.mode = AppMode::Insert;
        snapshot_input("input_placeholder_insert", &app);
    }

    #[test]
    fn test_input_placeholder_processing_mode() {
        // Empty input in Processing mode should show no placeholder (just empty)
        let mut app = create_test_app();
        app.input.clear();
        app.mode = AppMode::Processing;
        snapshot_input("input_placeholder_processing", &app);
    }

    #[test]
    fn test_input_no_placeholder_with_text() {
        // Input with text should not show placeholder
        let mut app = create_test_app();
        app.input = "Some text".to_string();
        app.cursor_position = app.input.len();
        app.mode = AppMode::Insert;
        snapshot_input("input_no_placeholder_with_text", &app);
    }

    #[test]
    fn test_input_scroll_right_indicator() {
        // Long input at start should show right indicator only
        let mut app = create_test_app();
        app.input = "a".repeat(100); // Longer than visible width
        app.cursor_position = 5;
        app.input_scroll_offset = 0; // At start
        app.mode = AppMode::Insert;
        snapshot_input("input_scroll_right_indicator", &app);
    }

    #[test]
    fn test_input_scroll_left_indicator() {
        // Long input scrolled to end should show left indicator only
        let mut app = create_test_app();
        app.input = "a".repeat(100);
        app.cursor_position = 100;
        app.input_scroll_offset = 30; // Scrolled right
        app.mode = AppMode::Insert;
        snapshot_input("input_scroll_left_indicator", &app);
    }

    #[test]
    fn test_input_scroll_both_indicators() {
        // Long input in middle should show both indicators
        let mut app = create_test_app();
        app.input = "a".repeat(200); // Very long input
        app.cursor_position = 100; // In the middle
        app.input_scroll_offset = 50; // Scrolled to middle
        app.mode = AppMode::Insert;
        snapshot_input("input_scroll_both_indicators", &app);
    }

    // === Chat Area Tests ===

    #[test]
    fn test_chat_with_welcome_message() {
        let app = create_test_app();
        snapshot_chat("chat_with_welcome_message", &app);
    }

    #[test]
    fn test_chat_with_conversation() {
        let mut app = create_test_app();
        app.messages.push(ChatMessage {
            role: MessageRole::User,
            content: "List the files in the current directory".to_string(),
        });
        app.messages.push(ChatMessage {
            role: MessageRole::Tool,
            content: "Calling tool: shell".to_string(),
        });
        app.messages.push(ChatMessage {
            role: MessageRole::Tool,
            content: "✓ shell: Cargo.toml\nREADME.md\nsrc/".to_string(),
        });
        app.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: "The current directory contains Cargo.toml, README.md, and a src directory."
                .to_string(),
        });
        snapshot_chat("chat_with_conversation", &app);
    }

    #[test]
    fn test_chat_with_error_message() {
        let mut app = create_test_app();
        app.messages.push(ChatMessage {
            role: MessageRole::System,
            content: "Error: Connection timeout".to_string(),
        });
        snapshot_chat("chat_with_error_message", &app);
    }

    // === Render Message Tests ===

    #[test]
    fn test_render_user_message() {
        let msg = ChatMessage {
            role: MessageRole::User,
            content: "Hello".to_string(),
        };
        let item = render_message(&msg, 80, None);
        // Verify it creates a valid ListItem with at least 1 line height
        assert!(item.height() >= 1);
    }

    #[test]
    fn test_render_assistant_message() {
        let msg = ChatMessage {
            role: MessageRole::Assistant,
            content: "Hi there, how can I help?".to_string(),
        };
        let item = render_message(&msg, 80, None);
        assert!(item.height() >= 1);
    }

    #[test]
    fn test_render_system_message() {
        let msg = ChatMessage {
            role: MessageRole::System,
            content: "System initialized".to_string(),
        };
        let item = render_message(&msg, 80, None);
        assert!(item.height() >= 1);
    }

    #[test]
    fn test_render_tool_message() {
        let msg = ChatMessage {
            role: MessageRole::Tool,
            content: "Tool output here".to_string(),
        };
        let item = render_message(&msg, 80, None);
        assert!(item.height() >= 1);
    }

    #[test]
    fn test_render_multiline_message() {
        let msg = ChatMessage {
            role: MessageRole::Assistant,
            content: "Line 1\nLine 2\nLine 3".to_string(),
        };
        let item = render_message(&msg, 80, None);
        // Should have height for 3 lines
        assert!(item.height() >= 3);
    }

    #[test]
    fn test_render_assistant_with_markdown() {
        let msg = ChatMessage {
            role: MessageRole::Assistant,
            content: "Here is some **bold** and *italic* text with `code`.".to_string(),
        };
        let item = render_message(&msg, 80, None);
        assert!(item.height() >= 1);
    }

    #[test]
    fn test_render_assistant_with_code_block() {
        let msg = ChatMessage {
            role: MessageRole::Assistant,
            content: "Here is code:\n```rust\nfn main() {}\n```".to_string(),
        };
        let item = render_message(&msg, 80, None);
        // Code block should produce multiple lines
        assert!(item.height() >= 2);
    }

    // === Spinner Tests ===

    #[test]
    fn test_spinner_frames_constant() {
        // Verify SPINNER_FRAMES has reasonable length (compile-time check via const)
        const _: () = assert!(SPINNER_FRAMES.len() >= 4); // At least 4 frames for smooth animation

        // Verify all frames are printable Unicode characters
        for &frame in SPINNER_FRAMES {
            assert!(frame.is_ascii() || frame.len_utf8() <= 4);
        }
    }

    #[test]
    fn test_spinner_cycles_through_all_frames() {
        // Verify modulo indexing works correctly
        for i in 0..SPINNER_FRAMES.len() * 2 {
            let frame = SPINNER_FRAMES[i % SPINNER_FRAMES.len()];
            assert!(frame != '\0'); // No null characters
        }
    }

    #[test]
    fn test_spinner_in_status_thinking() {
        let mut app = create_test_app();
        app.mode = AppMode::Processing;
        app.agent_status = AgentStatus::Thinking {
            model: "gpt-4".to_string(),
        };

        // Test at different spinner frames
        app.spinner_frame = 0;
        snapshot_status("status_thinking_frame_0", &app);

        app.spinner_frame = 5;
        snapshot_status("status_thinking_frame_5", &app);
    }

    #[test]
    fn test_spinner_in_status_executing() {
        let mut app = create_test_app();
        app.mode = AppMode::Processing;
        app.agent_status = AgentStatus::ExecutingTool {
            tool: "shell".to_string(),
        };
        app.spinner_frame = 3;
        snapshot_status("status_executing_tool", &app);
    }

    #[test]
    fn test_status_complete_no_spinner() {
        let mut app = create_test_app();
        app.agent_status = AgentStatus::Complete { duration_ms: 1234 };
        app.spinner_frame = 7; // Should not affect display
        snapshot_status("status_complete", &app);
    }

    #[test]
    fn test_status_error_no_spinner() {
        let mut app = create_test_app();
        app.agent_status = AgentStatus::Error {
            message: "Connection failed".to_string(),
        };
        app.spinner_frame = 2; // Should not affect display
        snapshot_status("status_error", &app);
    }

    // === Error Message Detection Tests ===

    #[test]
    fn test_is_error_message_with_error_prefix() {
        assert!(is_error_message("Error: Something went wrong"));
        assert!(is_error_message("error: lowercase error"));
        assert!(is_error_message("ERROR: UPPERCASE"));
        assert!(is_error_message("Error - with dash"));
    }

    #[test]
    fn test_is_error_message_with_failed_prefix() {
        assert!(is_error_message("Failed: Operation failed"));
        assert!(is_error_message("Failure: Critical failure"));
    }

    #[test]
    fn test_is_error_message_with_keywords() {
        assert!(is_error_message("An error occurred during processing"));
        assert!(is_error_message("Connection failed to server"));
        assert!(is_error_message("Request timeout after 30s"));
        assert!(is_error_message("Authentication failed"));
    }

    #[test]
    fn test_is_error_message_non_errors() {
        assert!(!is_error_message("Welcome to the application"));
        assert!(!is_error_message("Operation completed successfully"));
        assert!(!is_error_message("Processing your request"));
        assert!(!is_error_message("Tool: shell"));
    }

    #[test]
    fn test_render_system_error_message() {
        let msg = ChatMessage {
            role: MessageRole::System,
            content: "Error: Connection timeout".to_string(),
        };
        let item = render_message(&msg, 80, None);
        assert!(item.height() >= 1);
    }

    #[test]
    fn test_render_tool_error_message() {
        let msg = ChatMessage {
            role: MessageRole::Tool,
            content: "Error: Command failed with exit code 1".to_string(),
        };
        let item = render_message(&msg, 80, None);
        assert!(item.height() >= 1);
    }

    #[test]
    fn test_render_system_normal_message() {
        let msg = ChatMessage {
            role: MessageRole::System,
            content: "Welcome to Codex DashFlow".to_string(),
        };
        let item = render_message(&msg, 80, None);
        assert!(item.height() >= 1);
    }

    #[test]
    fn test_chat_with_error_styling() {
        let mut app = create_test_app();
        app.messages.push(ChatMessage {
            role: MessageRole::System,
            content: "Error: Network connection failed".to_string(),
        });
        snapshot_chat("chat_with_styled_error", &app);
    }

    #[test]
    fn test_chat_with_tool_error_styling() {
        let mut app = create_test_app();
        app.messages.push(ChatMessage {
            role: MessageRole::Tool,
            content: "Error: Permission denied".to_string(),
        });
        snapshot_chat("chat_with_tool_error", &app);
    }

    // === Help Overlay Tests ===

    #[test]
    fn test_help_overlay_hidden_by_default() {
        let app = create_test_app();
        assert!(!app.show_help);
    }

    #[test]
    fn test_help_overlay_toggle() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        assert!(!app.show_help);
        app.show_help = true;
        assert!(app.show_help);
        app.show_help = false;
        assert!(!app.show_help);
    }

    #[test]
    fn test_full_ui_with_help_overlay() {
        let mut app = create_test_app();
        app.show_help = true;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, &app))
            .expect("Failed to draw");

        assert_snapshot!("full_ui_with_help_overlay", terminal.backend());
    }

    #[test]
    fn test_help_overlay_content() {
        // Test that help overlay renders without panic on small terminals
        let mut app = create_test_app();
        app.show_help = true;

        let backend = TestBackend::new(40, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let result = terminal.draw(|frame| render(frame, &app));
        assert!(result.is_ok());
    }

    // === Word Wrap Tests ===

    #[test]
    fn test_chat_with_long_message_wrapped() {
        let mut app = create_test_app();
        // Add a message that exceeds typical terminal width
        app.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: "This is a very long message that should wrap to multiple lines when rendered in the chat area. It contains enough text to exceed the typical terminal width of 80 characters and demonstrate word wrapping functionality.".to_string(),
        });
        // Render in narrow terminal to force wrapping
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, &app))
            .expect("Failed to draw");
        assert_snapshot!("chat_with_wrapped_message", terminal.backend());
    }

    #[test]
    fn test_render_message_with_narrow_width() {
        let msg = ChatMessage {
            role: MessageRole::User,
            content: "Hello world this is a test message".to_string(),
        };
        // Use narrow width to force wrapping
        let item = render_message(&msg, 20, None);
        // Should have multiple lines due to wrapping
        assert!(item.height() >= 2);
    }

    #[test]
    fn test_render_message_preserves_newlines() {
        let msg = ChatMessage {
            role: MessageRole::Assistant,
            content: "Line 1\nLine 2\nLine 3".to_string(),
        };
        let item = render_message(&msg, 80, None);
        // Original newlines should be preserved
        assert!(item.height() >= 3);
    }

    // === Search Highlight Tests ===

    #[test]
    fn test_highlight_search_in_text_no_match() {
        let highlight = SearchHighlight {
            query: "xyz",
            is_current_match: false,
        };
        let spans = highlight_search_in_text("Hello world", &highlight, Style::default());
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content.as_ref(), "Hello world");
    }

    #[test]
    fn test_highlight_search_in_text_single_match() {
        let highlight = SearchHighlight {
            query: "world",
            is_current_match: false,
        };
        let spans = highlight_search_in_text("Hello world", &highlight, Style::default());
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content.as_ref(), "Hello ");
        assert_eq!(spans[1].content.as_ref(), "world");
    }

    #[test]
    fn test_highlight_search_in_text_multiple_matches() {
        let highlight = SearchHighlight {
            query: "o",
            is_current_match: false,
        };
        let spans = highlight_search_in_text("Hello world", &highlight, Style::default());
        // H-e-l-l-[o]- -w-[o]-r-l-d
        // Should produce: "Hell", "o", " w", "o", "rld"
        assert_eq!(spans.len(), 5);
        assert_eq!(spans[0].content.as_ref(), "Hell");
        assert_eq!(spans[1].content.as_ref(), "o");
        assert_eq!(spans[2].content.as_ref(), " w");
        assert_eq!(spans[3].content.as_ref(), "o");
        assert_eq!(spans[4].content.as_ref(), "rld");
    }

    #[test]
    fn test_highlight_search_case_insensitive() {
        let highlight = SearchHighlight {
            query: "HELLO",
            is_current_match: false,
        };
        let spans = highlight_search_in_text("Hello world", &highlight, Style::default());
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content.as_ref(), "Hello"); // Preserves original case
        assert_eq!(spans[1].content.as_ref(), " world");
    }

    #[test]
    fn test_highlight_search_empty_query() {
        let highlight = SearchHighlight {
            query: "",
            is_current_match: false,
        };
        let spans = highlight_search_in_text("Hello world", &highlight, Style::default());
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content.as_ref(), "Hello world");
    }

    #[test]
    fn test_highlight_search_current_match_style() {
        let current = SearchHighlight {
            query: "test",
            is_current_match: true,
        };
        let other = SearchHighlight {
            query: "test",
            is_current_match: false,
        };

        let current_spans = highlight_search_in_text("test message", &current, Style::default());
        let other_spans = highlight_search_in_text("test message", &other, Style::default());

        // Both should have 2 spans
        assert_eq!(current_spans.len(), 2);
        assert_eq!(other_spans.len(), 2);

        // The highlighted span should have different styles
        assert_ne!(current_spans[0].style, other_spans[0].style);
    }

    #[test]
    fn test_render_message_with_highlight() {
        let msg = ChatMessage {
            role: MessageRole::User,
            content: "Hello world this is a test".to_string(),
        };
        let highlight = SearchHighlight {
            query: "world",
            is_current_match: true,
        };
        let item = render_message(&msg, 80, Some(&highlight));
        assert!(item.height() >= 1);
    }

    #[test]
    fn test_chat_with_search_highlight() {
        let mut app = create_test_app();
        app.messages.push(ChatMessage {
            role: MessageRole::User,
            content: "Hello world".to_string(),
        });
        app.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: "The world is full of wonders.".to_string(),
        });

        // Set up search state
        app.search_query = "world".to_string();
        app.search_matches = vec![1, 2]; // Both messages match
        app.current_match = Some(1); // Second match is current

        let backend = TestBackend::new(80, 15);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, &app))
            .expect("Failed to draw");
        assert_snapshot!("chat_with_search_highlight", terminal.backend());
    }

    #[test]
    fn test_chat_with_search_no_matches() {
        let mut app = create_test_app();
        app.messages.push(ChatMessage {
            role: MessageRole::User,
            content: "Hello world".to_string(),
        });

        // Search with no matches
        app.search_query = "xyz123".to_string();
        app.search_matches = vec![];
        app.current_match = None;

        let backend = TestBackend::new(80, 15);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, &app))
            .expect("Failed to draw");
        // Should render without panic even with no matches
    }

    // Tests for build_highlighted_spans

    #[test]
    fn test_build_highlighted_spans_empty_positions() {
        let base_style = Style::default();
        let highlight_style = Style::default().fg(Color::Yellow);

        let spans = super::build_highlighted_spans("/help", &[], base_style, highlight_style);

        // Should return single span with base style when no positions to highlight
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "/help");
    }

    #[test]
    fn test_build_highlighted_spans_consecutive_positions() {
        let base_style = Style::default();
        let highlight_style = Style::default().fg(Color::Yellow);

        // Highlight positions 0, 1, 2 (first three characters)
        let spans =
            super::build_highlighted_spans("/help", &[0, 1, 2], base_style, highlight_style);

        // Should produce two spans: highlighted "/he" and non-highlighted "lp"
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "/he");
        assert_eq!(spans[1].content, "lp");
    }

    #[test]
    fn test_build_highlighted_spans_with_gaps() {
        let base_style = Style::default();
        let highlight_style = Style::default().fg(Color::Yellow);

        // Highlight positions 0, 2, 4 (non-consecutive) in "/help"
        let spans =
            super::build_highlighted_spans("/help", &[0, 2, 4], base_style, highlight_style);

        // Should produce multiple alternating spans
        // /=highlight, h=base, e=highlight, l=base, p=highlight
        assert!(spans.len() >= 3);

        // Verify the highlighted characters are correct by checking content
        let combined: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(combined, "/help");
    }

    #[test]
    fn test_build_highlighted_spans_all_highlighted() {
        let base_style = Style::default();
        let highlight_style = Style::default().fg(Color::Yellow);

        // Highlight all positions
        let spans =
            super::build_highlighted_spans("/quit", &[0, 1, 2, 3, 4], base_style, highlight_style);

        // Should return single highlighted span
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "/quit");
    }

    #[test]
    fn test_build_highlighted_spans_empty_text() {
        let base_style = Style::default();
        let highlight_style = Style::default().fg(Color::Yellow);

        let spans = super::build_highlighted_spans("", &[0, 1], base_style, highlight_style);

        // Empty text should return single empty span
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "");
    }
}
