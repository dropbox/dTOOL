//! VirtualList example demonstrating efficient rendering of long lists.
#![allow(clippy::future_not_send)] // Example runs on current_thread runtime
//!
//! This example shows how to use VirtualList for:
//! - Rendering 1000+ items without performance degradation
//! - Keyboard navigation (arrows, PageUp/Down, Home/End)
//! - Mouse scroll support
//! - Variable and fixed item heights
//!
//! This is essential for chat history (like Codex TUI) with thousands of messages.
//!
//! Run with:
//! ```bash
//! cargo run --example virtual_list --features async
//! ```
//!
//! Controls:
//! - Up/Down arrows: Scroll by 1 item
//! - PageUp/PageDown: Scroll by page
//! - Home/End: Jump to top/bottom
//! - Mouse wheel: Scroll
//! - Tab: Toggle between fixed/variable height mode
//! - q/Escape: Quit

use inky::components::{VirtualList, VirtualScrollbarVisibility};
use inky::prelude::*;
use std::sync::Arc;

/// Sample message for demonstration.
#[derive(Clone)]
struct Message {
    id: usize,
    role: &'static str,
    content: String,
    lines: u16,
}

impl Message {
    fn new(id: usize) -> Self {
        let (role, content, lines) = match id % 4 {
            0 => ("user", format!("This is user message #{}", id), 1),
            1 => (
                "assistant",
                format!(
                    "Here's a longer assistant response for message #{}.\n\
                     It spans multiple lines to show variable height support.",
                    id
                ),
                3,
            ),
            2 => ("user", format!("Follow-up question #{}", id), 1),
            _ => (
                "assistant",
                format!(
                    "Response #{} with code:\n\
                     ```rust\n\
                     fn example() {{\n\
                     println!(\"Hello!\");\n\
                     }}\n\
                     ```",
                    id
                ),
                6,
            ),
        };

        Self {
            id,
            role,
            content,
            lines,
        }
    }

    fn render(&self) -> Node {
        let role_color = match self.role {
            "user" => Color::BrightBlue,
            "assistant" => Color::BrightGreen,
            _ => Color::White,
        };

        BoxNode::new()
            .flex_direction(FlexDirection::Column)
            .width(Dimension::Percent(100.0))
            .child(
                TextNode::new(format!("  [{}] #{}", self.role, self.id))
                    .color(role_color)
                    .bold(),
            )
            .child(TextNode::new(format!("  {}", &self.content)).color(Color::White))
            .into()
    }
}

/// Application state.
struct AppState {
    /// Our virtual list state.
    list: VirtualList,
    /// All messages (could be thousands).
    messages: Arc<Vec<Message>>,
    /// Use variable heights?
    variable_height: bool,
    /// Viewport height (set from render context).
    viewport_height: u16,
}

impl AppState {
    fn new(message_count: usize) -> Self {
        // Generate sample messages
        let messages: Arc<Vec<Message>> = Arc::new((0..message_count).map(Message::new).collect());

        // Create virtual list with the exact API requested by Codex porter:
        // VirtualList::new()
        //     .item_count(messages.len())
        //     .item_height(3)
        //     .render_item(|index| render_message(&messages[index]))
        //     .viewport_height(20)
        let list = VirtualList::new(messages.len())
            .item_height(3) // Fixed height initially
            .viewport_height(20)
            .overscan(3) // Render 3 extra items above/below for smooth scrolling
            .scrollbar(VirtualScrollbarVisibility::Auto);

        Self {
            list,
            messages,
            variable_height: false,
            viewport_height: 20,
        }
    }

    fn rebuild_list(&mut self) {
        let offset = self.list.get_scroll_offset();

        if self.variable_height {
            // Use variable heights based on message content
            let heights: Vec<u16> = self.messages.iter().map(|m| m.lines + 1).collect();
            self.list = VirtualList::new(self.messages.len())
                .variable_height(move |idx| heights.get(idx).copied().unwrap_or(2))
                .viewport_height(self.viewport_height)
                .overscan(3)
                .scroll_offset(offset)
                .scrollbar(VirtualScrollbarVisibility::Auto);
        } else {
            // Fixed height mode
            self.list = VirtualList::new(self.messages.len())
                .item_height(3)
                .viewport_height(self.viewport_height)
                .overscan(3)
                .scroll_offset(offset)
                .scrollbar(VirtualScrollbarVisibility::Auto);
        }
    }

    fn toggle_height_mode(&mut self) {
        self.variable_height = !self.variable_height;
        self.rebuild_list();
    }

    /// Build the virtual list node with render callback.
    fn build_list(&self) -> Node {
        // Clone Arc for the closure
        let messages = Arc::clone(&self.messages);

        VirtualList::new(self.messages.len())
            .item_height(3)
            .viewport_height(self.viewport_height)
            .scroll_offset(self.list.get_scroll_offset())
            .overscan(3)
            .scrollbar(VirtualScrollbarVisibility::Auto)
            .render_item(move |idx| messages[idx].render())
            .into()
    }
}

#[cfg(feature = "async")]
fn main() -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to create runtime: {}", e))?;

    rt.block_on(async_main())
}

#[cfg(feature = "async")]
async fn async_main() -> Result<()> {
    // Create 1000 messages to demonstrate virtualization
    let state = AppState::new(1000);

    let app = AsyncApp::new()
        .state(state)
        .render(|ctx| {
            let state = ctx.state;

            // Update viewport height if terminal size changed
            // (Note: in a real app you'd handle resize events)
            let (start, end) = state.list.visible_range();
            let list_node = state.build_list();

            BoxNode::new()
                .width(ctx.width())
                .height(ctx.height())
                .flex_direction(FlexDirection::Column)
                .child(
                    // Header
                    BoxNode::new()
                        .width(Dimension::Percent(100.0))
                        .padding_xy(1.0, 0.0)
                        .border(BorderStyle::Rounded)
                        .child(
                            TextNode::new(format!(
                                "VirtualList Demo - {} messages, showing {}-{} ({})",
                                state.messages.len(),
                                start,
                                end,
                                if state.variable_height {
                                    "variable height"
                                } else {
                                    "fixed height"
                                }
                            ))
                            .color(Color::BrightCyan)
                            .bold(),
                        ),
                )
                .child(
                    // Virtual list
                    BoxNode::new()
                        .flex_grow(1.0)
                        .width(Dimension::Percent(100.0))
                        .border(BorderStyle::Single)
                        .child(list_node),
                )
                .child(
                    // Status bar
                    BoxNode::new()
                        .width(Dimension::Percent(100.0))
                        .padding_xy(1.0, 0.0)
                        .flex_direction(FlexDirection::Row)
                        .child(
                            TextNode::new(format!(
                                "Offset: {} / {}",
                                state.list.get_scroll_offset(),
                                state.messages.len().saturating_sub(20)
                            ))
                            .color(Color::BrightYellow),
                        )
                        .child(Spacer::new())
                        .child(
                            TextNode::new(
                                "[↑↓] Scroll  [PgUp/Dn] Page  [Tab] Toggle height  [q] Quit",
                            )
                            .color(Color::BrightBlack),
                        ),
                )
                .into()
        })
        .on_key(|state, event| {
            match event.code {
                KeyCode::Char('q') | KeyCode::Esc => return true,
                KeyCode::Up => state.list.scroll_up(1),
                KeyCode::Down => state.list.scroll_down(1),
                KeyCode::PageUp => state.list.page_up(),
                KeyCode::PageDown => state.list.page_down(),
                KeyCode::Home => state.list.scroll_to_top(),
                KeyCode::End => state.list.scroll_to_bottom(),
                KeyCode::Tab => state.toggle_height_mode(),
                _ => {}
            }
            false
        })
        .on_mouse(|state, event| {
            match event.kind {
                MouseEventKind::ScrollUp => state.list.scroll_up(3),
                MouseEventKind::ScrollDown => state.list.scroll_down(3),
                _ => {}
            }
            false
        });

    app.run_async().await?;
    Ok(())
}

#[cfg(not(feature = "async"))]
fn main() {
    eprintln!("This example requires the `async` feature.");
    eprintln!("Run with: cargo run --example virtual_list --features async");
    std::process::exit(1);
}
