//! Focus navigation example demonstrating Tab/Shift+Tab navigation.
//!
//! This example shows how focus management works in inky.
//! Press Tab to move focus forward through the boxes.
//! Press Shift+Tab to move focus backward.
//!
//! Controls:
//! - Tab: Focus next box
//! - Shift+Tab: Focus previous box
//! - q / Escape: Quit

use inky::prelude::*;

/// Application state with focus handles for each box.
struct FocusState {
    /// Focus handles for each box
    handles: Vec<FocusHandle>,
}

impl FocusState {
    fn new() -> Self {
        // Create three focus handles
        let handles = vec![use_focus(), use_focus(), use_focus()];

        // Focus the first box initially
        handles[0].focus();

        Self { handles }
    }

    /// Get the index of the currently focused box, if any.
    fn focused_index(&self) -> Option<usize> {
        self.handles.iter().position(|h| h.is_focused())
    }
}

fn render_box(label: &str, index: usize, is_focused: bool) -> Node {
    let text_color = if is_focused {
        Color::BrightWhite
    } else {
        Color::BrightBlack
    };

    let border_style = if is_focused {
        BorderStyle::Double
    } else {
        BorderStyle::Single
    };

    let mut text_node = TextNode::new(format!("Box {} - {}", index + 1, label))
        .color(text_color)
        .bold();

    if is_focused {
        text_node = text_node.bg(Color::Blue);
    }

    let mut content_box = BoxNode::new()
        .flex_direction(FlexDirection::Column)
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .width(20)
        .height(5)
        .border(border_style)
        .padding(1)
        .child(text_node);

    if is_focused {
        content_box = content_box.child(TextNode::new("[FOCUSED]").color(Color::BrightGreen));
    }

    content_box.into()
}

fn main() -> Result<()> {
    let state = FocusState::new();

    App::new()
        .state(state)
        .render(|ctx| {
            let focused_idx = ctx.state.focused_index();

            // Main container
            BoxNode::new()
                .width(ctx.width())
                .height(ctx.height())
                .flex_direction(FlexDirection::Column)
                .align_items(AlignItems::Center)
                .justify_content(JustifyContent::Center)
                .gap(2.0)
                .child(
                    // Title
                    TextNode::new("Focus Navigation Demo")
                        .color(Color::Cyan)
                        .bold(),
                )
                .child(
                    TextNode::new("Press Tab to cycle forward, Shift+Tab to cycle backward")
                        .color(Color::BrightBlack),
                )
                .child(Spacer::new())
                .child(
                    // Row of focusable boxes
                    BoxNode::new()
                        .flex_direction(FlexDirection::Row)
                        .gap(3.0)
                        .child(render_box("First", 0, focused_idx == Some(0)))
                        .child(render_box("Second", 1, focused_idx == Some(1)))
                        .child(render_box("Third", 2, focused_idx == Some(2))),
                )
                .child(Spacer::new())
                .child(
                    // Status indicator
                    BoxNode::new()
                        .flex_direction(FlexDirection::Column)
                        .align_items(AlignItems::Center)
                        .child(
                            TextNode::new(match focused_idx {
                                Some(i) => format!("Currently focused: Box {}", i + 1),
                                None => "No box focused".to_string(),
                            })
                            .color(Color::Yellow),
                        )
                        // 3 buttons registered as focusable
                        .child(TextNode::new("Registered focusables: 3").color(Color::BrightBlack)),
                )
                .child(Spacer::new())
                .child(
                    TextNode::new("Press 'q' or Escape to quit")
                        .color(Color::BrightBlack)
                        .italic(),
                )
                .into()
        })
        .on_key(|_state, key| {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return true,
                // Tab/Shift+Tab is handled automatically by the App
                _ => {}
            }
            false
        })
        .run()?;

    Ok(())
}
